//! Per-layer pipeline: content (shape, solid, image, precomp), then effects,
//! masks and track mattes on an offscreen buffer, composited with the layer's
//! blend mode.

use tiny_skia::{BlendMode, FillRule, Mask, MaskType, Pixmap, PixmapPaint, Transform};

use super::{effects, paint, shapes, RenderCtx};
use crate::anim::{eval1, eval_shape};
use crate::geom::{self, Affine, Path};
use crate::model::{Animation, Layer};
use crate::scene;

fn blend(bm: i64) -> BlendMode {
    match bm {
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        6 => BlendMode::ColorDodge,
        7 => BlendMode::ColorBurn,
        8 => BlendMode::HardLight,
        9 => BlendMode::SoftLight,
        10 => BlendMode::Difference,
        11 => BlendMode::Exclusion,
        12 => BlendMode::Hue,
        13 => BlendMode::Saturation,
        14 => BlendMode::Color,
        15 => BlendMode::Luminosity,
        16 => BlendMode::Plus,
        _ => BlendMode::SourceOver,
    }
}

fn parse_hex(s: &str) -> [f32; 3] {
    let h = s.trim_start_matches('#');
    let g = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0;
    [g(0), g(2), g(4)]
}

/// Index of the layer providing this layer's track matte.
fn matte_source(layers: &[Layer], idx: usize) -> Option<usize> {
    match layers[idx].tp {
        Some(ind) => layers.iter().position(|l| l.ind == Some(ind)),
        None => idx.checked_sub(1),
    }
}

/// Draw `layers[idx]` (with masks, mattes, effects and blending) onto `canvas`.
#[allow(clippy::too_many_arguments)]
pub fn draw_layer(canvas: &mut Pixmap, anim: &Animation, layers: &[Layer], idx: usize, ctx: &RenderCtx, t: f64, base: &Affine, depth: u32) {
    let l = &layers[idx];
    let lt = scene::layer_time(l, t);
    let has_masks = l.masks.iter().any(|m| m.mode != "n");
    let has_fx = l.ef.iter().any(|e| e.enabled && matches!(e.ty, 20 | 21 | 23 | 25 | 29));
    let offscreen = has_masks || has_fx || l.tt != 0 || l.bm != 0 || l.ty == 0;
    if !offscreen {
        draw_content(canvas, anim, layers, idx, ctx, t, base, depth);
        return;
    }
    let Some(mut pm) = Pixmap::new(canvas.width(), canvas.height()) else { return };
    draw_content(&mut pm, anim, layers, idx, ctx, t, base, depth);
    if has_fx {
        effects::apply(&mut pm, &l.ef, lt, base.scale_factor());
    }
    let m = base.mul(&scene::layer_matrix(layers, idx, t));
    if has_masks {
        if let Some(mask) = layer_mask(l, lt, &m, pm.width(), pm.height()) {
            pm.apply_mask(&mask);
        }
    }
    if l.tt != 0 {
        if let Some(src) = matte_source(layers, idx) {
            let Some(mut mp) = Pixmap::new(canvas.width(), canvas.height()) else { return };
            if scene::is_visible(&layers[src], t) || layers[src].hd {
                // A matte source is hidden from direct drawing but still used as a matte.
                draw_layer(&mut mp, anim, layers, src, ctx, t, base, depth + 1);
            }
            let kind = if l.tt >= 3 { MaskType::Luminance } else { MaskType::Alpha };
            let mut mask = Mask::from_pixmap(mp.as_ref(), kind);
            if l.tt == 2 || l.tt == 4 {
                for v in mask.data_mut() {
                    *v = 255 - *v;
                }
            }
            pm.apply_mask(&mask);
        }
    }
    let paint = PixmapPaint { blend_mode: blend(l.bm), ..Default::default() };
    canvas.draw_pixmap(0, 0, pm.as_ref(), &paint, Transform::identity(), None);
}

fn layer_mask(l: &Layer, t: f64, m: &Affine, w: u32, h: u32) -> Option<Mask> {
    let n = (w * h) as usize;
    let mut acc: Vec<f32> = Vec::new();
    for mk in l.masks.iter().filter(|m| m.mode != "n") {
        let mut cov = vec![0.0f32; n];
        if let Some(pt) = &mk.pt {
            let c = geom::from_bezier(&eval_shape(pt, t));
            if let Some(path) = Path::from_contour(c).to_skia() {
                if let Some(mut mm) = Mask::new(w, h) {
                    mm.fill_path(&path, FillRule::Winding, true, m.to_skia());
                    let o = (eval1(mk.o.as_ref(), t, 100.0) / 100.0) as f32;
                    for (c, v) in cov.iter_mut().zip(mm.data()) {
                        *c = *v as f32 / 255.0 * o;
                    }
                }
            }
        }
        if mk.inv {
            for c in cov.iter_mut() {
                *c = 1.0 - *c;
            }
        }
        if acc.is_empty() {
            let start = if matches!(mk.mode.as_str(), "a" | "l" | "f") { 0.0 } else { 1.0 };
            acc = vec![start; n];
        }
        for (a, c) in acc.iter_mut().zip(&cov) {
            *a = match mk.mode.as_str() {
                "a" => *a + c - *a * c,
                "s" => *a * (1.0 - c),
                "i" => *a * c,
                "l" => a.max(*c),
                "d" => a.min(*c),
                "f" => (*a - c).abs(),
                _ => *a,
            };
        }
    }
    if acc.is_empty() {
        return None;
    }
    let mut mask = Mask::new(w, h)?;
    for (d, a) in mask.data_mut().iter_mut().zip(acc) {
        *d = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    Some(mask)
}

/// Draw the layer's own content with its transform and opacity.
#[allow(clippy::too_many_arguments)]
fn draw_content(canvas: &mut Pixmap, anim: &Animation, layers: &[Layer], idx: usize, ctx: &RenderCtx, t: f64, base: &Affine, depth: u32) {
    let l = &layers[idx];
    let m = base.mul(&scene::layer_matrix(layers, idx, t));
    let lt = scene::layer_time(l, t);
    let alpha = scene::opacity(&l.ks, lt) as f32;
    if alpha <= 0.0 {
        return;
    }
    match l.ty {
        4 => shapes::render_items(canvas, &l.shapes, &m, alpha, lt, &[]),
        1 => {
            let c = parse_hex(l.sc.as_deref().unwrap_or("#000000"));
            if let Some(path) = Path::from_contour(geom::rect([l.sw / 2.0, l.sh / 2.0], [l.sw, l.sh], 0.0, false)).to_skia() {
                paint::fill(canvas, &path, &paint::solid_paint(c, alpha), 1, &m);
            }
        }
        2 => {
            let Some(id) = &l.ref_id else { return };
            let (Some(img), Some(asset)) = (ctx.images.get(id), anim.asset(id)) else { return };
            let (aw, ah) = (if asset.w > 0.0 { asset.w } else { img.width() as f64 }, if asset.h > 0.0 { asset.h } else { img.height() as f64 });
            let fit = m.mul(&Affine::scale(aw / img.width() as f64, ah / img.height() as f64));
            let paint = PixmapPaint { opacity: alpha, quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
            canvas.draw_pixmap(0, 0, img.as_ref(), &paint, fit.to_skia(), None);
        }
        0 => {
            let Some(asset) = l.ref_id.as_ref().and_then(|id| anim.asset(id)) else { return };
            let Some(inner_layers) = &asset.layers else { return };
            let local = match &l.tm {
                Some(tm) => eval1(Some(tm), lt, 0.0) * asset.fr.unwrap_or(anim.fr),
                None => (t - l.st) / if l.sr.abs() > 1e-9 { l.sr } else { 1.0 },
            };
            let Some(mut pm) = Pixmap::new(canvas.width(), canvas.height()) else { return };
            super::draw_comp(&mut pm, anim, inner_layers, ctx, local, &m, depth + 1);
            // Clip to the precomp's bounds.
            if l.w > 0.0 && l.h > 0.0 {
                if let Some(path) = Path::from_contour(geom::rect([l.w / 2.0, l.h / 2.0], [l.w, l.h], 0.0, false)).to_skia() {
                    if let Some(mut clip) = Mask::new(pm.width(), pm.height()) {
                        clip.fill_path(&path, FillRule::Winding, true, m.to_skia());
                        pm.apply_mask(&clip);
                    }
                }
            }
            let paint = PixmapPaint { opacity: alpha, ..Default::default() };
            canvas.draw_pixmap(0, 0, pm.as_ref(), &paint, Transform::identity(), None);
        }
        5 => draw_text(canvas, l, ctx, lt, &m, alpha),
        _ => {}
    }
}

fn draw_text(canvas: &mut Pixmap, l: &Layer, ctx: &RenderCtx, t: f64, m: &Affine, alpha: f32) {
    let Some(td) = &l.text else { return };
    let Some(doc) = td.docs.iter().rev().find(|(ft, _)| *ft <= t).or(td.docs.first()).map(|(_, d)| d) else { return };
    let glyphs = crate::text::layout(doc, &ctx.fonts);
    let mut all = Path::default();
    for g in glyphs {
        all.extend(g);
    }
    let Some(path) = all.to_skia() else { return };
    let fill = |canvas: &mut Pixmap| {
        if let Some(c) = &doc.fill {
            paint::fill(canvas, &path, &paint::solid_paint(paint::rgb(c), alpha), 1, m);
        }
    };
    let stroke = |canvas: &mut Pixmap| {
        if let (Some(c), true) = (&doc.stroke, doc.stroke_width > 0.0) {
            let spec = tiny_skia::Stroke { width: doc.stroke_width as f32, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
            paint::stroke(canvas, &path, &paint::solid_paint(paint::rgb(c), alpha), &spec, m);
        }
    };
    if doc.stroke_over_fill {
        fill(canvas);
        stroke(canvas);
    } else {
        stroke(canvas);
        fill(canvas);
    }
}
