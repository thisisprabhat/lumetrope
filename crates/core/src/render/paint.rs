//! Fills, strokes, dashes and gradients.

use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, Pixmap, Point, RadialGradient, Shader, SpreadMode,
    Stroke, StrokeDash,
};

use crate::anim::{eval1, eval_or};
use crate::geom::Affine;
use crate::model::{Gradient, StrokeStyle};

/// Normalise a Lottie colour (0..1, or legacy 0..255) to RGB in 0..1.
pub fn rgb(v: &[f64]) -> [f32; 3] {
    let big = v.iter().take(3).any(|x| *x > 1.0);
    let g = |i: usize| {
        let x = v.get(i).copied().unwrap_or(0.0);
        (if big { x / 255.0 } else { x }).clamp(0.0, 1.0) as f32
    };
    [g(0), g(1), g(2)]
}

pub enum Brush {
    Solid([f32; 3]),
    Gradient(Shader<'static>),
}

pub fn solid_paint(c: [f32; 3], alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba(c[0], c[1], c[2], alpha.clamp(0.0, 1.0)).unwrap_or(Color::BLACK));
    p.anti_alias = true;
    p
}

pub(crate) fn merged_stops(g: &Gradient, t: f64, alpha: f32) -> Vec<(f32, Color)> {
    let raw = eval_or(g.colors.as_ref(), t, &[]);
    let n = g.count.min(raw.len() / 4);
    let colors: Vec<(f64, [f64; 3])> = (0..n).map(|i| (raw[i * 4], [raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]])).collect();
    let rest = &raw[n * 4..];
    let alphas: Vec<(f64, f64)> = rest.chunks_exact(2).map(|c| (c[0], c[1])).collect();
    if colors.is_empty() {
        return vec![];
    }
    let sample = |stops: &[(f64, f64)], x: f64| -> f64 {
        if stops.is_empty() {
            return 1.0;
        }
        if x <= stops[0].0 {
            return stops[0].1;
        }
        for w in stops.windows(2) {
            if x <= w[1].0 {
                let d = w[1].0 - w[0].0;
                let f = if d > 0.0 { (x - w[0].0) / d } else { 1.0 };
                return w[0].1 + (w[1].1 - w[0].1) * f;
            }
        }
        stops[stops.len() - 1].1
    };
    let color_at = |x: f64| -> [f64; 3] {
        let ch = |k: usize| {
            let pts: Vec<(f64, f64)> = colors.iter().map(|(o, c)| (*o, c[k])).collect();
            sample(&pts, x)
        };
        [ch(0), ch(1), ch(2)]
    };
    let mut offsets: Vec<f64> = colors.iter().map(|c| c.0).chain(alphas.iter().map(|a| a.0)).collect();
    offsets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    offsets.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    offsets
        .into_iter()
        .map(|o| {
            let c = rgb(&color_at(o));
            let a = (sample(&alphas, o) as f32 * alpha).clamp(0.0, 1.0);
            (o.clamp(0.0, 1.0) as f32, Color::from_rgba(c[0], c[1], c[2], a).unwrap_or(Color::TRANSPARENT))
        })
        .collect()
}

/// Build a gradient shader in the shape's local space.
pub fn gradient_shader(g: &Gradient, t: f64, alpha: f32) -> Option<Shader<'static>> {
    let stops = merged_stops(g, t, alpha);
    if stops.is_empty() {
        return None;
    }
    if stops.len() == 1 {
        return Some(Shader::SolidColor(stops[0].1));
    }
    let stops: Vec<GradientStop> = stops.into_iter().map(|(o, c)| GradientStop::new(o, c)).collect();
    let s = eval_or(g.s.as_ref(), t, &[0.0, 0.0]);
    let e = eval_or(g.e.as_ref(), t, &[0.0, 0.0]);
    let (sx, sy) = (s.first().copied().unwrap_or(0.0), s.get(1).copied().unwrap_or(0.0));
    let (ex, ey) = (e.first().copied().unwrap_or(0.0), e.get(1).copied().unwrap_or(0.0));
    if g.t == 2 {
        let r = ((ex - sx).powi(2) + (ey - sy).powi(2)).sqrt();
        if r <= 1e-6 {
            return None;
        }
        let h = (eval1(g.h.as_ref(), t, 0.0) / 100.0).clamp(-0.99, 0.99);
        let ang = (ey - sy).atan2(ex - sx) + eval1(g.a.as_ref(), t, 0.0).to_radians();
        let focal = Point::from_xy((sx + h * r * ang.cos()) as f32, (sy + h * r * ang.sin()) as f32);
        RadialGradient::new(focal, Point::from_xy(sx as f32, sy as f32), r as f32, stops, SpreadMode::Pad, tiny_skia::Transform::identity())
    } else {
        LinearGradient::new(Point::from_xy(sx as f32, sy as f32), Point::from_xy(ex as f32, ey as f32), stops, SpreadMode::Pad, tiny_skia::Transform::identity())
    }
}

pub fn brush_paint(b: &Brush, alpha: f32) -> Paint<'static> {
    match b {
        Brush::Solid(c) => solid_paint(*c, alpha),
        Brush::Gradient(sh) => {
            let mut p = Paint::default();
            p.shader = sh.clone();
            p.anti_alias = true;
            p
        }
    }
}

pub fn fill(canvas: &mut Pixmap, path: &tiny_skia::Path, paint: &Paint, rule: i64, m: &Affine) {
    let fr = if rule == 2 { FillRule::EvenOdd } else { FillRule::Winding };
    canvas.fill_path(path, paint, fr, m.to_skia(), None);
}

pub fn stroke_spec(style: &StrokeStyle, t: f64) -> Option<Stroke> {
    let w = eval1(style.w.as_ref(), t, 1.0);
    if !(w > 0.0) || !w.is_finite() {
        return None;
    }
    let mut s = Stroke {
        width: w as f32,
        line_cap: match style.lc {
            1 => LineCap::Butt,
            3 => LineCap::Square,
            _ => LineCap::Round,
        },
        line_join: match style.lj {
            1 => LineJoin::Miter,
            3 => LineJoin::Bevel,
            _ => LineJoin::Round,
        },
        miter_limit: style.ml.max(1.0) as f32,
        dash: None,
    };
    if !style.dashes.is_empty() {
        let mut arr = vec![];
        let mut offset = 0.0f32;
        for d in &style.dashes {
            let v = eval1(d.v.as_ref(), t, 0.0) as f32;
            if d.n == "o" {
                offset = v;
            } else {
                arr.push(v.max(0.0));
            }
        }
        if arr.len() % 2 == 1 {
            let copy = arr.clone();
            arr.extend(copy);
        }
        s.dash = StrokeDash::new(arr, offset);
    }
    Some(s)
}

pub fn stroke(canvas: &mut Pixmap, path: &tiny_skia::Path, paint: &Paint, spec: &Stroke, m: &Affine) {
    canvas.stroke_path(path, paint, spec, m.to_skia(), None);
}
