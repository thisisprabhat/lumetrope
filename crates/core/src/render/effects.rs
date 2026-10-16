//! Layer effects on an offscreen RGBA pixmap: drop shadow (25), gaussian blur
//! (29), fill (21), tint (20), tritone (23). Values are read by index, matching
//! the After Effects/Lottie effect layouts.

use tiny_skia::{Pixmap, PixmapPaint, Transform};

use super::paint::rgb;
use crate::anim::{eval1, eval_or};
use crate::model::Effect;

fn val(e: &Effect, i: usize, t: f64, d: f64) -> f64 {
    e.values.get(i).map(|v| eval1(v.v.as_ref(), t, d)).unwrap_or(d)
}

fn col(e: &Effect, i: usize, t: f64, d: [f64; 3]) -> [f32; 3] {
    rgb(&e.values.get(i).map(|v| eval_or(v.v.as_ref(), t, &d)).unwrap_or_else(|| d.to_vec()))
}

/// Apply every enabled effect in order. `scale` converts composition units to pixels.
pub fn apply(pm: &mut Pixmap, effects: &[Effect], t: f64, scale: f64) {
    for e in effects.iter().filter(|e| e.enabled) {
        match e.ty {
            25 => drop_shadow(pm, e, t, scale),
            29 => {
                let sigma = val(e, 0, t, 0.0) * scale / 2.0;
                let dims = val(e, 1, t, 1.0) as i64;
                blur(pm, sigma, dims != 3, dims != 2);
            }
            21 => recolor(pm, |_, _, _| col(e, 2, t, [1.0, 0.0, 0.0]), (val(e, 6, t, 1.0) as f32).clamp(0.0, 1.0)),
            20 => {
                let (b, w, amt) = (col(e, 0, t, [0.0; 3]), col(e, 1, t, [1.0; 3]), (val(e, 2, t, 100.0) / 100.0) as f32);
                map_luma(pm, amt, |l| mix(b, w, l));
            }
            23 => {
                let (hi, mid, sh) = (col(e, 0, t, [1.0; 3]), col(e, 1, t, [0.5; 3]), col(e, 2, t, [0.0; 3]));
                let amt = 1.0 - (val(e, 3, t, 0.0) / 100.0) as f32;
                map_luma(pm, amt, |l| if l < 0.5 { mix(sh, mid, l * 2.0) } else { mix(mid, hi, (l - 0.5) * 2.0) });
            }
            _ => {}
        }
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Replace each pixel colour (keeping alpha), blended by `amount`.
fn recolor(pm: &mut Pixmap, f: impl Fn(f32, f32, f32) -> [f32; 3], amount: f32) {
    for px in pm.data_mut().chunks_exact_mut(4) {
        let a = px[3] as f32 / 255.0;
        if a <= 0.0 {
            continue;
        }
        let (r, g, b) = (px[0] as f32 / 255.0 / a, px[1] as f32 / 255.0 / a, px[2] as f32 / 255.0 / a);
        let c = f(r, g, b);
        let out = mix([r, g, b], c, amount);
        for k in 0..3 {
            px[k] = (out[k].clamp(0.0, 1.0) * a * 255.0).round() as u8;
        }
    }
}

fn map_luma(pm: &mut Pixmap, amount: f32, f: impl Fn(f32) -> [f32; 3]) {
    recolor(pm, |r, g, b| f(0.2126 * r + 0.7152 * g + 0.0722 * b), amount);
}

fn drop_shadow(pm: &mut Pixmap, e: &Effect, t: f64, scale: f64) {
    let color = col(e, 0, t, [0.0; 3]);
    let opacity = (val(e, 1, t, 127.5) / 255.0).clamp(0.0, 1.0) as f32;
    let dir = val(e, 2, t, 135.0).to_radians();
    let dist = val(e, 3, t, 0.0) * scale;
    let soft = val(e, 4, t, 0.0) * scale;
    let only = val(e, 5, t, 0.0) != 0.0;
    let Some(mut sh) = Pixmap::new(pm.width(), pm.height()) else { return };
    for (d, s) in sh.data_mut().chunks_exact_mut(4).zip(pm.data().chunks_exact(4)) {
        let a = s[3] as f32 / 255.0 * opacity;
        for k in 0..3 {
            d[k] = (color[k] * a * 255.0).round() as u8;
        }
        d[3] = (a * 255.0).round() as u8;
    }
    if soft > 0.0 {
        blur(&mut sh, soft / 2.0, true, true);
    }
    let Some(mut out) = Pixmap::new(pm.width(), pm.height()) else { return };
    let (dx, dy) = ((dir.sin() * dist) as f32, (-dir.cos() * dist) as f32);
    out.draw_pixmap(0, 0, sh.as_ref(), &PixmapPaint::default(), Transform::from_translate(dx, dy), None);
    if !only {
        out.draw_pixmap(0, 0, pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
    *pm = out;
}

/// Approximate gaussian blur with three box passes per axis.
pub fn blur(pm: &mut Pixmap, sigma: f64, horizontal: bool, vertical: bool) {
    if !(sigma > 0.25) {
        return;
    }
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    // Box sizes for 3 passes (Kovesi).
    let n = 3.0;
    let wl = ((12.0 * sigma * sigma / n + 1.0).sqrt()).floor() as i64;
    let wl = if wl % 2 == 0 { wl - 1 } else { wl }.max(1);
    let m = ((12.0 * sigma * sigma - n * (wl * wl) as f64 - 4.0 * n * wl as f64 - 3.0 * n) / (-4.0 * wl as f64 - 4.0)).round() as i64;
    let radii: Vec<usize> = (0..3).map(|i| (if i < m { wl } else { wl + 2 } as usize - 1) / 2).collect();
    let mut buf: Vec<f32> = pm.data().iter().map(|&x| x as f32).collect();
    let mut tmp = buf.clone();
    for &r in &radii {
        if r == 0 {
            continue;
        }
        if horizontal {
            box_pass(&buf, &mut tmp, w, h, r, true);
            std::mem::swap(&mut buf, &mut tmp);
        }
        if vertical {
            box_pass(&buf, &mut tmp, w, h, r, false);
            std::mem::swap(&mut buf, &mut tmp);
        }
    }
    for (d, s) in pm.data_mut().iter_mut().zip(buf) {
        *d = s.round().clamp(0.0, 255.0) as u8;
    }
    // Keep premultiplied invariant (colour ≤ alpha) after rounding.
    for px in pm.data_mut().chunks_exact_mut(4) {
        for k in 0..3 {
            px[k] = px[k].min(px[3]);
        }
    }
}

fn box_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, horiz: bool) {
    let (outer, inner) = if horiz { (h, w) } else { (w, h) };
    let idx = |o: usize, i: usize| if horiz { (o * w + i) * 4 } else { (i * w + o) * 4 };
    let norm = 1.0 / (2 * r + 1) as f32;
    for o in 0..outer {
        for c in 0..4 {
            let mut acc = 0.0f32;
            // transparent outside the image
            for i in 0..=r.min(inner.saturating_sub(1)) {
                acc += src[idx(o, i) + c];
            }
            for i in 0..inner {
                dst[idx(o, i) + c] = acc * norm;
                let add = i + r + 1;
                if add < inner {
                    acc += src[idx(o, add) + c];
                }
                if i >= r {
                    acc -= src[idx(o, i - r) + c];
                }
            }
        }
    }
}
