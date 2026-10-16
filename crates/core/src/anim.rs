//! Keyframe evaluation: cubic-bezier easing, holds, legacy end values and
//! spatial (motion-path) position interpolation.

use crate::model::{Bezier, Ease, Keyframe, Prop, ShapeProp};

/// Evaluate a CSS-style cubic bezier easing curve (P0=(0,0), P3=(1,1)) at `x`.
pub fn cubic_ease(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let (x1, x2) = (x1.clamp(0.0, 1.0), x2.clamp(0.0, 1.0));
    if (x1 - y1).abs() < 1e-9 && (x2 - y2).abs() < 1e-9 {
        return x;
    }
    let bx = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * x1 + 3.0 * (1.0 - t) * t * t * x2 + t * t * t;
    let by = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * y1 + 3.0 * (1.0 - t) * t * t * y2 + t * t * t;
    let dx = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * x1 + 6.0 * (1.0 - t) * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
    let mut t = x;
    for _ in 0..8 {
        let err = bx(t) - x;
        if err.abs() < 1e-7 {
            return by(t);
        }
        let d = dx(t);
        if d.abs() < 1e-6 {
            break;
        }
        t -= err / d;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    t = x;
    for _ in 0..60 {
        let v = bx(t);
        if (v - x).abs() < 1e-7 {
            break;
        }
        if v < x {
            lo = t;
        } else {
            hi = t;
        }
        t = (lo + hi) / 2.0;
    }
    by(t)
}

fn ease_dim(o: Option<&Ease>, i: Option<&Ease>, d: usize, u: f64) -> f64 {
    let pick = |v: &[f64], dflt: f64| v.get(d).or_else(|| v.first()).copied().unwrap_or(dflt);
    let (x1, y1) = o.map(|e| (pick(&e.x, 0.0), pick(&e.y, 0.0))).unwrap_or((0.0, 0.0));
    let (x2, y2) = i.map(|e| (pick(&e.x, 1.0), pick(&e.y, 1.0))).unwrap_or((1.0, 1.0));
    cubic_ease(x1, y1, x2, y2, u)
}

/// Locate the keyframe segment for `frame`: Ok(index of segment start, progress) or
/// Err(index of the keyframe whose value applies outright).
fn locate<T>(kfs: &[Keyframe<T>], frame: f64) -> Result<(usize, f64), usize> {
    if frame < kfs[0].t {
        return Err(0);
    }
    for n in 0..kfs.len() - 1 {
        let (k0, k1) = (&kfs[n], &kfs[n + 1]);
        if frame < k1.t {
            let dt = k1.t - k0.t;
            if dt <= 0.0 {
                return Err(n + 1);
            }
            return Ok((n, (frame - k0.t) / dt));
        }
    }
    Err(kfs.len() - 1)
}

fn end_value<'a>(kfs: &'a [Keyframe<Vec<f64>>], n: usize) -> &'a [f64] {
    if let Some(e) = &kfs[n].e {
        return e;
    }
    match kfs.get(n + 1) {
        Some(k1) if !k1.s.is_empty() => &k1.s,
        _ => &kfs[n].s,
    }
}

fn settled(kfs: &[Keyframe<Vec<f64>>], n: usize) -> Vec<f64> {
    if !kfs[n].s.is_empty() {
        return kfs[n].s.clone();
    }
    // Legacy files end with a bare `{t}` keyframe: use the previous end value.
    (0..n).rev().find_map(|m| kfs[m].e.clone().or_else(|| (!kfs[m].s.is_empty()).then(|| kfs[m].s.clone()))).unwrap_or_default()
}

/// Evaluate a numeric property at `frame` (composition frames).
pub fn eval(p: &Prop, frame: f64) -> Vec<f64> {
    if !p.animated || p.keyframes.is_empty() {
        return p.value.clone();
    }
    let kfs = &p.keyframes;
    let (n, u) = match locate(kfs, frame) {
        Ok(x) => x,
        Err(n) => return settled(kfs, n),
    };
    let k0 = &kfs[n];
    if k0.h {
        return settled(kfs, n);
    }
    let a = &k0.s;
    let b = end_value(kfs, n);
    let dims = a.len().min(b.len());
    if dims >= 2 {
        let nonzero = |v: &Option<Vec<f64>>| v.as_ref().map(|t| t.iter().any(|x| x.abs() > 1e-9)).unwrap_or(false);
        if nonzero(&k0.to) || nonzero(&k0.ti) {
            let e = ease_dim(k0.o.as_ref(), k0.i.as_ref(), 0, u);
            return spatial(a, b, k0.to.as_deref().unwrap_or(&[]), k0.ti.as_deref().unwrap_or(&[]), e);
        }
    }
    let mut out = Vec::with_capacity(b.len().max(a.len()));
    for d in 0..dims {
        let e = ease_dim(k0.o.as_ref(), k0.i.as_ref(), d, u);
        out.push(a[d] + (b[d] - a[d]) * e);
    }
    out.extend_from_slice(&b[dims..]);
    out
}

/// Point at arc-length fraction `e` along the cubic from `a` to `b` with tangents.
fn spatial(a: &[f64], b: &[f64], to: &[f64], ti: &[f64], e: f64) -> Vec<f64> {
    let g = |v: &[f64], d: usize| v.get(d).copied().unwrap_or(0.0);
    let p0 = [a[0], a[1]];
    let p1 = [a[0] + g(to, 0), a[1] + g(to, 1)];
    let p2 = [b[0] + g(ti, 0), b[1] + g(ti, 1)];
    let p3 = [b[0], b[1]];
    let at = |t: f64| {
        let mt = 1.0 - t;
        let c = [mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t];
        [c[0] * p0[0] + c[1] * p1[0] + c[2] * p2[0] + c[3] * p3[0], c[0] * p0[1] + c[1] * p1[1] + c[2] * p2[1] + c[3] * p3[1]]
    };
    const N: usize = 48;
    let mut lens = [0.0f64; N + 1];
    let mut prev = p0;
    for s in 1..=N {
        let q = at(s as f64 / N as f64);
        lens[s] = lens[s - 1] + ((q[0] - prev[0]).powi(2) + (q[1] - prev[1]).powi(2)).sqrt();
        prev = q;
    }
    let total = lens[N];
    let t = if total <= 1e-9 {
        e
    } else {
        let target = e.clamp(0.0, 1.0) * total;
        let idx = lens.partition_point(|&l| l < target).clamp(1, N);
        let (l0, l1) = (lens[idx - 1], lens[idx]);
        let f = if l1 > l0 { (target - l0) / (l1 - l0) } else { 0.0 };
        ((idx - 1) as f64 + f) / N as f64
    };
    let pt = at(t);
    let mut out = vec![pt[0], pt[1]];
    for d in 2..a.len().min(b.len()) {
        out.push(a[d] + (b[d] - a[d]) * e);
    }
    out
}

/// Evaluate an optional property, falling back to `default`.
pub fn eval_or(p: Option<&Prop>, frame: f64, default: &[f64]) -> Vec<f64> {
    match p {
        Some(p) => {
            let v = eval(p, frame);
            if v.is_empty() {
                default.to_vec()
            } else {
                v
            }
        }
        None => default.to_vec(),
    }
}

/// Evaluate the first component of an optional property.
pub fn eval1(p: Option<&Prop>, frame: f64, default: f64) -> f64 {
    p.and_then(|p| eval(p, frame).first().copied()).unwrap_or(default)
}

fn lerp_bezier(a: &Bezier, b: &Bezier, e: f64) -> Bezier {
    if a.v.len() != b.v.len() {
        return a.clone();
    }
    let l = |x: &[[f64; 2]], y: &[[f64; 2]]| {
        x.iter().zip(y).map(|(p, q)| [p[0] + (q[0] - p[0]) * e, p[1] + (q[1] - p[1]) * e]).collect()
    };
    Bezier { c: a.c, v: l(&a.v, &b.v), i: l(&a.i, &b.i), o: l(&a.o, &b.o) }
}

/// Evaluate a path property at `frame`.
pub fn eval_shape(p: &ShapeProp, frame: f64) -> Bezier {
    if !p.animated || p.keyframes.is_empty() {
        return p.value.clone();
    }
    let kfs = &p.keyframes;
    let settled_b = |n: usize| -> Bezier {
        if !kfs[n].s.is_empty() {
            return kfs[n].s.clone();
        }
        (0..n).rev().find_map(|m| kfs[m].e.clone().or_else(|| (!kfs[m].s.is_empty()).then(|| kfs[m].s.clone()))).unwrap_or_default()
    };
    let (n, u) = match locate(kfs, frame) {
        Ok(x) => x,
        Err(n) => return settled_b(n),
    };
    let k0 = &kfs[n];
    if k0.h {
        return settled_b(n);
    }
    let b = match &k0.e {
        Some(e) => e.clone(),
        None => kfs.get(n + 1).filter(|k| !k.s.is_empty()).map(|k| k.s.clone()).unwrap_or_else(|| k0.s.clone()),
    };
    lerp_bezier(&k0.s, &b, ease_dim(k0.o.as_ref(), k0.i.as_ref(), 0, u))
}
