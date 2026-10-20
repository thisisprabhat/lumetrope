//! Rasteriser entry points (tiny-skia).

pub mod effects;
pub mod layers;
pub mod list;
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
    pub fonts: crate::text::FontRegistry,
}

impl RenderCtx {
    /// Decode every embedded (`data:` URI) image asset of `anim`.
    pub fn from_animation(anim: &Animation) -> Self {
        let mut ctx = RenderCtx::default();
        ctx.load_embedded(anim);
        ctx
    }

    pub fn load_embedded(&mut self, anim: &Animation) {
        for a in &anim.assets {
            if a.layers.is_some() || self.images.contains_key(&a.id) {
                continue;
            }
            if let Some(bytes) = decode_data_uri(&a.p) {
                self.add_image(&a.id, &bytes);
            }
        }
    }

    /// Register image bytes (PNG or JPEG) for an asset id; returns false if undecodable.
    pub fn add_image(&mut self, id: &str, bytes: &[u8]) -> bool {
        match decode_image(bytes) {
            Some(pm) => {
                self.images.insert(id.to_string(), pm);
                true
            }
            None => false,
        }
    }
}

pub fn decode_data_uri(p: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let rest = p.strip_prefix("data:")?;
    let (_, data) = rest.split_once(";base64,")?;
    base64::engine::general_purpose::STANDARD.decode(data.trim()).ok()
}

pub fn decode_image(bytes: &[u8]) -> Option<Pixmap> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Pixmap::decode_png(bytes).ok();
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut dec = jpeg_decoder::Decoder::new(bytes);
        let data = dec.decode().ok()?;
        let info = dec.info()?;
        let (w, h) = (info.width as u32, info.height as u32);
        let mut pm = Pixmap::new(w, h)?;
        let n = (w * h) as usize;
        let px = pm.data_mut();
        for i in 0..n {
            let (r, g, b) = match info.pixel_format {
                jpeg_decoder::PixelFormat::RGB24 => (data[i * 3], data[i * 3 + 1], data[i * 3 + 2]),
                jpeg_decoder::PixelFormat::L8 => (data[i], data[i], data[i]),
                jpeg_decoder::PixelFormat::L16 => (data[i * 2], data[i * 2], data[i * 2]),
                jpeg_decoder::PixelFormat::CMYK32 => {
                    let (c, m, y, k) = (data[i * 4] as u32, data[i * 4 + 1] as u32, data[i * 4 + 2] as u32, data[i * 4 + 3] as u32);
                    ((c * k / 255) as u8, (m * k / 255) as u8, (y * k / 255) as u8)
                }
            };
            px[i * 4..i * 4 + 4].copy_from_slice(&[r, g, b, 255]);
        }
        return Some(pm);
    }
    None
}

/// Render `anim` at `frame` into a new `w`×`h` pixmap (transparent background),
/// stretching the composition to fill it.
pub fn render(anim: &Animation, ctx: &RenderCtx, frame: f64, w: u32, h: u32) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("non-zero pixmap");
    let base = Affine::scale(w as f64 / anim.w.max(1e-6), h as f64 / anim.h.max(1e-6));
    draw_comp(&mut pm, anim, &anim.layers, ctx, frame, &base, 0);
    pm
}

/// Render through a camera: screen = comp × `scale` + (`tx`, `ty`), into a `w`×`h`
/// viewport. Content outside the composition bounds is clipped away.
#[allow(clippy::too_many_arguments)]
pub fn render_view(anim: &Animation, ctx: &RenderCtx, frame: f64, w: u32, h: u32, scale: f64, tx: f64, ty: f64) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("non-zero pixmap");
    let base = Affine { a: scale, b: 0.0, c: 0.0, d: scale, e: tx, f: ty };
    // Skip work entirely when the artboard is off-screen.
    let (x0, y0, x1, y1) = (tx, ty, tx + anim.w * scale, ty + anim.h * scale);
    if x1 <= 0.0 || y1 <= 0.0 || x0 >= w as f64 || y0 >= h as f64 {
        return pm;
    }
    draw_comp(&mut pm, anim, &anim.layers, ctx, frame, &base, 0);
    if let Some(rect) = tiny_skia::Rect::from_ltrb(x0.max(0.0) as f32, y0.max(0.0) as f32, x1.min(w as f64) as f32, y1.min(h as f64) as f32) {
        let mut clip = tiny_skia::Mask::new(pm.width(), pm.height()).expect("mask");
        clip.fill_path(&tiny_skia::PathBuilder::from_rect(rect), tiny_skia::FillRule::Winding, false, tiny_skia::Transform::identity());
        pm.apply_mask(&clip);
    }
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
        layers::draw_layer(canvas, anim, layers, idx, ctx, t, base, depth);
    }
}


/// Parse a `#rrggbb` solid-layer colour to 0..1 RGB.
pub fn layers_hex(s: &str) -> [f32; 3] {
    let h = s.trim_start_matches('#');
    let g = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0;
    [g(0), g(2), g(4)]
}
