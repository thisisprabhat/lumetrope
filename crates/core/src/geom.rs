//! Bezier geometry: Lottie primitives → contours, affine transforms, path length,
//! trim paths and round corners. Tangents are stored relative to their vertex,
//! exactly like Lottie's `{v, i, o}`.

use crate::model::{Bezier, Vec2};

/// Circle approximation constant for cubic beziers.
pub const KAPPA: f64 = 0.5519150244935105707435627;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contour {
    pub v: Vec<Vec2>,
    pub i: Vec<Vec2>,
    pub o: Vec<Vec2>,
    pub closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub contours: Vec<Contour>,
}

/// 2D affine matrix mapping (x, y) → (a·x + c·y + e, b·x + d·y + f).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn translate(x: f64, y: f64) -> Self {
        Affine { e: x, f: y, ..Self::IDENTITY }
    }
    pub fn scale(sx: f64, sy: f64) -> Self {
        Affine { a: sx, d: sy, ..Self::IDENTITY }
    }
    /// Rotation in degrees; positive is clockwise on a y-down screen.
    pub fn rotate(deg: f64) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        Affine { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }
    pub fn shear_x(k: f64) -> Self {
        Affine { c: k, ..Self::IDENTITY }
    }
    /// `self ∘ rhs`: apply `rhs` first, then `self`.
    pub fn mul(&self, r: &Affine) -> Affine {
        Affine {
            a: self.a * r.a + self.c * r.b,
            b: self.b * r.a + self.d * r.b,
            c: self.a * r.c + self.c * r.d,
            d: self.b * r.c + self.d * r.d,
            e: self.a * r.e + self.c * r.f + self.e,
            f: self.b * r.e + self.d * r.f + self.f,
        }
    }
    pub fn apply(&self, p: Vec2) -> Vec2 {
        [self.a * p[0] + self.c * p[1] + self.e, self.b * p[0] + self.d * p[1] + self.f]
    }
    pub fn invert(&self) -> Option<Affine> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-12 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Affine { a, b, c, d, e: -(a * self.e + c * self.f), f: -(b * self.e + d * self.f) })
    }
    /// Lottie transform: T(p) · R(r) · Skew(sk, sa) · S(s/100) · T(-a).
    pub fn from_lottie(anchor: Vec2, pos: Vec2, scale_pct: Vec2, rot: f64, sk: f64, sa: f64) -> Affine {
        let mut m = Affine::translate(pos[0], pos[1]).mul(&Affine::rotate(rot));
        if sk.abs() > 1e-9 {
            let skew = Affine::rotate(-sa).mul(&Affine::shear_x((-sk).to_radians().tan())).mul(&Affine::rotate(sa));
            m = m.mul(&skew);
        }
        m.mul(&Affine::scale(scale_pct[0] / 100.0, scale_pct[1] / 100.0)).mul(&Affine::translate(-anchor[0], -anchor[1]))
    }
    pub fn to_skia(&self) -> tiny_skia::Transform {
        tiny_skia::Transform::from_row(self.a as f32, self.b as f32, self.c as f32, self.d as f32, self.e as f32, self.f as f32)
    }
    /// Approximate uniform scale factor (for stroke widths, blur radii).
    pub fn scale_factor(&self) -> f64 {
        ((self.a * self.d - self.b * self.c).abs()).sqrt()
    }
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: Vec2, b: Vec2) -> Vec2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn lerp(a: Vec2, b: Vec2, t: f64) -> Vec2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}
fn dist(a: Vec2, b: Vec2) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl Contour {
    fn push(&mut self, v: Vec2, i: Vec2, o: Vec2) {
        self.v.push(v);
        self.i.push(i);
        self.o.push(o);
    }

    pub fn reversed(&self) -> Contour {
        let n = self.v.len();
        let mut c = Contour { closed: self.closed, ..Default::default() };
        if n == 0 {
            return c;
        }
        // Keep the first vertex first for closed contours, like lottie-web.
        let order: Vec<usize> = if self.closed { std::iter::once(0).chain((1..n).rev()).collect() } else { (0..n).rev().collect() };
        for k in order {
            c.push(self.v[k], self.o[k], self.i[k]);
        }
        c
    }

    pub fn transform(&self, m: &Affine) -> Contour {
        let mut c = Contour { closed: self.closed, ..Default::default() };
        for k in 0..self.v.len() {
            let v = m.apply(self.v[k]);
            c.push(v, sub(m.apply(add(self.v[k], self.i[k])), v), sub(m.apply(add(self.v[k], self.o[k])), v));
        }
        c
    }

    /// Cubic segments as absolute control points.
    pub fn segments(&self) -> Vec<[Vec2; 4]> {
        let n = self.v.len();
        let mut out = Vec::new();
        if n < 2 {
            return out;
        }
        let count = if self.closed { n } else { n - 1 };
        for k in 0..count {
            let j = (k + 1) % n;
            out.push([self.v[k], add(self.v[k], self.o[k]), add(self.v[j], self.i[j]), self.v[j]]);
        }
        out
    }

    pub fn length(&self) -> f64 {
        self.segments().iter().map(seg_len).sum()
    }
}

impl Path {
    pub fn from_contour(c: Contour) -> Path {
        Path { contours: vec![c] }
    }
    pub fn transform(&self, m: &Affine) -> Path {
        Path { contours: self.contours.iter().map(|c| c.transform(m)).collect() }
    }
    pub fn length(&self) -> f64 {
        self.contours.iter().map(Contour::length).sum()
    }
    pub fn extend(&mut self, other: Path) {
        self.contours.extend(other.contours);
    }

    pub fn to_skia(&self) -> Option<tiny_skia::Path> {
        let mut pb = tiny_skia::PathBuilder::new();
        let mut any = false;
        for c in &self.contours {
            if c.v.is_empty() || c.v.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
                continue;
            }
            pb.move_to(c.v[0][0] as f32, c.v[0][1] as f32);
            for s in c.segments() {
                pb.cubic_to(s[1][0] as f32, s[1][1] as f32, s[2][0] as f32, s[2][1] as f32, s[3][0] as f32, s[3][1] as f32);
            }
            if c.closed {
                pb.close();
            }
            any = true;
        }
        if !any {
            return None;
        }
        pb.finish()
    }

    /// Axis-aligned bounds of the control polygon: [min_x, min_y, max_x, max_y].
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for c in &self.contours {
            for s in c.segments() {
                for t in 0..=8 {
                    let p = cubic_at(&s, t as f64 / 8.0);
                    b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
                }
            }
            if c.v.len() == 1 {
                let p = c.v[0];
                b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
            }
        }
        b[0].is_finite().then_some(b)
    }
}

pub fn cubic_at(s: &[Vec2; 4], t: f64) -> Vec2 {
    let mt = 1.0 - t;
    let c = [mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t];
    [
        c[0] * s[0][0] + c[1] * s[1][0] + c[2] * s[2][0] + c[3] * s[3][0],
        c[0] * s[0][1] + c[1] * s[1][1] + c[2] * s[2][1] + c[3] * s[3][1],
    ]
}

const SAMPLES: usize = 24;

fn seg_table(s: &[Vec2; 4]) -> [f64; SAMPLES + 1] {
    let mut t = [0.0; SAMPLES + 1];
    let mut prev = s[0];
    for k in 1..=SAMPLES {
        let p = cubic_at(s, k as f64 / SAMPLES as f64);
        t[k] = t[k - 1] + dist(prev, p);
        prev = p;
    }
    t
}

fn seg_len(s: &[Vec2; 4]) -> f64 {
    seg_table(s)[SAMPLES]
}

/// Parameter t at which the arc length from the segment start equals `l`.
fn t_at_len(s: &[Vec2; 4], l: f64) -> f64 {
    let tab = seg_table(s);
    if tab[SAMPLES] <= 1e-12 {
        return 0.0;
    }
    let l = l.clamp(0.0, tab[SAMPLES]);
    let idx = tab.partition_point(|&x| x < l).clamp(1, SAMPLES);
    let (a, b) = (tab[idx - 1], tab[idx]);
    let f = if b > a { (l - a) / (b - a) } else { 0.0 };
    ((idx - 1) as f64 + f) / SAMPLES as f64
}

/// Sub-segment of a cubic between parameters t0..t1 (de Casteljau).
fn split(s: &[Vec2; 4], t0: f64, t1: f64) -> [Vec2; 4] {
    let left = |s: &[Vec2; 4], t: f64| -> [Vec2; 4] {
        let ab = lerp(s[0], s[1], t);
        let bc = lerp(s[1], s[2], t);
        let cd = lerp(s[2], s[3], t);
        let abc = lerp(ab, bc, t);
        let bcd = lerp(bc, cd, t);
        [s[0], ab, abc, lerp(abc, bcd, t)]
    };
    let right = |s: &[Vec2; 4], t: f64| -> [Vec2; 4] {
        let ab = lerp(s[0], s[1], t);
        let bc = lerp(s[1], s[2], t);
        let cd = lerp(s[2], s[3], t);
        let abc = lerp(ab, bc, t);
        let bcd = lerp(bc, cd, t);
        [lerp(abc, bcd, t), bcd, cd, s[3]]
    };
    let r = right(s, t0);
    if t0 >= 1.0 {
        return [s[3]; 4];
    }
    let t = ((t1 - t0) / (1.0 - t0)).clamp(0.0, 1.0);
    left(&r, t)
}

/// Extract the piece of `c` between absolute lengths `from..to` as an open contour.
fn sub_contour(c: &Contour, from: f64, to: f64) -> Option<Contour> {
    if to - from <= 1e-9 {
        return None;
    }
    let mut out = Contour::default();
    let mut acc = 0.0;
    for s in c.segments() {
        let l = seg_len(&s);
        let (s0, s1) = (acc, acc + l);
        acc = s1;
        if s1 <= from || s0 >= to || l <= 1e-12 {
            continue;
        }
        let t0 = if from > s0 { t_at_len(&s, from - s0) } else { 0.0 };
        let t1 = if to < s1 { t_at_len(&s, to - s0) } else { 1.0 };
        let p = split(&s, t0, t1);
        if out.v.is_empty() {
            out.push(p[0], [0.0, 0.0], sub(p[1], p[0]));
        } else {
            let last = out.v.len() - 1;
            out.o[last] = sub(p[1], p[0]);
        }
        out.push(p[3], sub(p[2], p[3]), [0.0, 0.0]);
    }
    (out.v.len() >= 2).then_some(out)
}

/// Trim `paths` like a Lottie trim-path modifier. `start`/`end` are percentages,
/// `offset_deg` is the offset in degrees (360 = one full length).
pub fn trim(paths: &[Path], start: f64, end: f64, offset_deg: f64, individually: bool) -> Vec<Path> {
    if individually {
        return paths.iter().flat_map(|p| trim(std::slice::from_ref(p), start, end, offset_deg, false)).collect();
    }
    let (mut s, mut e) = (start / 100.0, end / 100.0);
    if s > e {
        std::mem::swap(&mut s, &mut e);
    }
    if (e - s).abs() < 1e-9 {
        return paths.iter().map(|_| Path::default()).collect();
    }
    if s <= 0.0 && e >= 1.0 {
        return paths.to_vec();
    }
    let o = (offset_deg / 360.0).rem_euclid(1.0);
    let (s, e) = (s + o, e + o);
    let total: f64 = paths.iter().map(Path::length).sum();
    let mut ranges = vec![];
    if e <= 1.0 {
        ranges.push((s, e));
    } else if s >= 1.0 {
        ranges.push((s - 1.0, e - 1.0));
    } else {
        ranges.push((s, 1.0));
        ranges.push((0.0, e - 1.0));
    }
    let mut out = Vec::new();
    let mut acc = 0.0;
    for p in paths {
        let mut np = Path::default();
        for c in &p.contours {
            let l = c.length();
            for &(a, b) in &ranges {
                let (from, to) = (a * total - acc, b * total - acc);
                if to <= 0.0 || from >= l {
                    continue;
                }
                if let Some(sc) = sub_contour(c, from.max(0.0), to.min(l)) {
                    np.contours.push(sc);
                }
            }
            acc += l;
        }
        out.push(np);
    }
    out
}

/// Rectangle centred at `c`, Lottie vertex order (top-right, clockwise).
pub fn rect(c: Vec2, size: Vec2, round: f64, reversed: bool) -> Contour {
    let (hw, hh) = (size[0] / 2.0, size[1] / 2.0);
    let r = round.max(0.0).min(hw.abs()).min(hh.abs());
    let mut k = Contour { closed: true, ..Default::default() };
    let z = [0.0, 0.0];
    if r <= 1e-9 {
        k.push([c[0] + hw, c[1] - hh], z, z);
        k.push([c[0] + hw, c[1] + hh], z, z);
        k.push([c[0] - hw, c[1] + hh], z, z);
        k.push([c[0] - hw, c[1] - hh], z, z);
    } else {
        let t = r * KAPPA;
        let (x0, x1, y0, y1) = (c[0] - hw, c[0] + hw, c[1] - hh, c[1] + hh);
        k.push([x1, y0 + r], [0.0, -t], z);
        k.push([x1, y1 - r], z, [0.0, t]);
        k.push([x1 - r, y1], [t, 0.0], z);
        k.push([x0 + r, y1], z, [-t, 0.0]);
        k.push([x0, y1 - r], [0.0, t], z);
        k.push([x0, y0 + r], z, [0.0, -t]);
        k.push([x0 + r, y0], [-t, 0.0], z);
        k.push([x1 - r, y0], z, [t, 0.0]);
    }
    if reversed {
        k.reversed()
    } else {
        k
    }
}

/// Ellipse centred at `c`; starts at the top, clockwise.
pub fn ellipse(c: Vec2, size: Vec2, reversed: bool) -> Contour {
    let (rx, ry) = (size[0] / 2.0, size[1] / 2.0);
    let (cx, cy) = (rx * KAPPA, ry * KAPPA);
    let mut k = Contour { closed: true, ..Default::default() };
    k.push([c[0], c[1] - ry], [-cx, 0.0], [cx, 0.0]);
    k.push([c[0] + rx, c[1]], [0.0, -cy], [0.0, cy]);
    k.push([c[0], c[1] + ry], [cx, 0.0], [-cx, 0.0]);
    k.push([c[0] - rx, c[1]], [0.0, cy], [0.0, -cy]);
    if reversed {
        k.reversed()
    } else {
        k
    }
}

/// Star (`sy`=1) or polygon (`sy`=2). Roundness values are percentages; `rot` in degrees.
#[allow(clippy::too_many_arguments)]
pub fn star(sy: i64, c: Vec2, points: f64, outer_r: f64, inner_r: f64, outer_round: f64, inner_round: f64, rot: f64, reversed: bool) -> Contour {
    let polygon = sy == 2;
    let n = points.floor().max(0.0) as usize * if polygon { 1 } else { 2 };
    let mut k = Contour { closed: true, ..Default::default() };
    if n == 0 {
        return k;
    }
    let dir = if reversed { -1.0 } else { 1.0 };
    let step = std::f64::consts::TAU / n as f64;
    let mut ang = -std::f64::consts::FRAC_PI_2 + rot.to_radians();
    let mut long = true;
    for _ in 0..n {
        let (rad, round) = if long || polygon { (outer_r, outer_round / 100.0) } else { (inner_r, inner_round / 100.0) };
        let perim = std::f64::consts::TAU * rad / (n as f64 * if polygon { 4.0 } else { 2.0 });
        let (x, y) = (rad * ang.cos(), rad * ang.sin());
        let len = (x * x + y * y).sqrt();
        let (ox, oy) = if len > 0.0 { (y / len, -x / len) } else { (0.0, 0.0) };
        let t = perim * round * dir;
        k.push([x + c[0], y + c[1]], [ox * t, oy * t], [-ox * t, -oy * t]);
        long = !long;
        ang += step * dir;
    }
    k
}

pub fn from_bezier(b: &Bezier) -> Contour {
    Contour { v: b.v.clone(), i: b.i.clone(), o: b.o.clone(), closed: b.c }
}

pub fn to_bezier(c: &Contour) -> Bezier {
    Bezier { c: c.closed, v: c.v.clone(), i: c.i.clone(), o: c.o.clone() }
}

/// Round sharp corners (vertices without tangents) with radius `r`.
pub fn round_corners(c: &Contour, r: f64) -> Contour {
    let n = c.v.len();
    if r <= 0.0 || n < 2 {
        return c.clone();
    }
    let mut out = Contour { closed: c.closed, ..Default::default() };
    let z = [0.0, 0.0];
    for k in 0..n {
        let sharp = c.i[k] == z && c.o[k] == z;
        let has_prev = c.closed || k > 0;
        let has_next = c.closed || k + 1 < n;
        if !sharp || !has_prev || !has_next {
            out.push(c.v[k], c.i[k], c.o[k]);
            continue;
        }
        let v = c.v[k];
        let prev = c.v[(k + n - 1) % n];
        let next = c.v[(k + 1) % n];
        let (dp, dn) = (dist(v, prev), dist(v, next));
        if dp < 1e-9 || dn < 1e-9 {
            out.push(v, z, z);
            continue;
        }
        let a = lerp(v, prev, (r.min(dp / 2.0)) / dp);
        let b = lerp(v, next, (r.min(dn / 2.0)) / dn);
        let ka = 1.0 - KAPPA;
        out.push(a, z, [(v[0] - a[0]) * (1.0 - ka), (v[1] - a[1]) * (1.0 - ka)]);
        out.push(b, [(v[0] - b[0]) * (1.0 - ka), (v[1] - b[1]) * (1.0 - ka)], z);
    }
    out
}
