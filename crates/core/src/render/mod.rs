//! Rasteriser entry points (tiny-skia).

pub mod paint;
pub mod shapes;

use std::collections::HashMap;

use tiny_skia::Pixmap;

use crate::geom::Affine;
use crate::model::{Animation, Layer};
use crate::scene;

/// Resources the renderer needs beyond the document itself.
#[derive(Default)]
pub struct RenderCtx {
    /// Decoded image assets keyed by asset id.
    pub images: HashMap<String, Pixmap>,
}

/// Render `anim` at `frame` into a new `w`×`h` pixmap (transparent background),
/// stretching the composition to fill it.
pub fn render(anim: &Animation, ctx: &RenderCtx, frame: f64, w: u32, h: u32) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("non-zero pixmap");
    let base = Affine::scale(w as f64 / anim.w.max(1e-6), h as f64 / anim.h.max(1e-6));
    draw_comp(&mut pm, anim, &anim.layers, ctx, frame, &base, 0);
    pm
}

/// Draw a list of layers (bottom-most last in the array) at composition time `t`.
pub fn draw_comp(canvas: &mut Pixmap, anim: &Animation, layers: &[Layer], ctx: &RenderCtx, t: f64, base: &Affine, depth: u32) {
    if depth > 16 {
        return;
    }
    for idx in (0..layers.len()).rev() {
        let l = &layers[idx];
        if l.td != 0 || !scene::is_visible(l, t) {
            continue;
        }
        draw_layer(canvas, anim, layers, idx, ctx, t, base, depth);
    }
}

/// Draw one layer's content directly onto `canvas`.
#[allow(clippy::too_many_arguments)]
pub fn draw_layer(canvas: &mut Pixmap, anim: &Animation, layers: &[Layer], idx: usize, ctx: &RenderCtx, t: f64, base: &Affine, depth: u32) {
    let _ = (anim, ctx, depth);
    let l = &layers[idx];
    let m = base.mul(&scene::layer_matrix(layers, idx, t));
    let lt = scene::layer_time(l, t);
    let alpha = scene::opacity(&l.ks, lt) as f32;
    if l.ty == 4 {
        shapes::render_items(canvas, &l.shapes, &m, alpha, lt, &[]);
    }
}
