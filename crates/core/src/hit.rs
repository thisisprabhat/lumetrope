//! Hit testing and layer geometry for canvas interaction and state-machine
//! pointer routing.

use crate::anim::eval1;
use crate::geom::{self, Affine, Path};
use crate::model::{Animation, Layer, Shape};
use crate::render::shapes;
use crate::scene;
use crate::text::{self, FontRegistry};

/// Geometry of one layer in its composition's space.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    pub id: String,
    /// Bounds in layer space: [min_x, min_y, max_x, max_y].
    pub local: [f64; 4],
    /// Axis-aligned bounds in composition space.
    pub bounds: [f64; 4],
    /// Local bounds corners (TL, TR, BR, BL) mapped to composition space.
    pub corners: [[f64; 2]; 4],
    /// Layer matrix (a, b, c, d, e, f).
    pub matrix: [f64; 6],
    /// Anchor point in composition space.
    pub anchor: [f64; 2],
}

pub fn layer_id(comp: &str, l: &Layer) -> String {
    format!("{comp}:{}", l.ind.unwrap_or(l.json_index as i64))
}

fn comp_layers<'a>(anim: &'a Animation, comp: &str) -> Option<&'a [Layer]> {
    if comp == "root" {
        Some(&anim.layers)
    } else {
        anim.asset(comp).and_then(|a| a.layers.as_deref())
    }
}

/// Resolve `comp:ind` to (layers slice, index).
pub fn find<'a>(anim: &'a Animation, id: &str) -> Option<(&'a [Layer], usize)> {
    let (comp, ind) = id.rsplit_once(':')?;
    let ind: i64 = ind.parse().ok()?;
    let layers = comp_layers(anim, comp)?;
    let idx = layers.iter().position(|l| l.ind.unwrap_or(l.json_index as i64) == ind)?;
    Some((layers, idx))
}

/// The layer's own geometry in layer space.
pub fn local_paths(anim: &Animation, fonts: &FontRegistry, l: &Layer, t: f64) -> Vec<Path> {
    let lt = scene::layer_time(l, t);
    let rect = |w: f64, h: f64| vec![Path::from_contour(geom::rect([w / 2.0, h / 2.0], [w, h], 0.0, false))];
    match l.ty {
        4 => shapes::collect(&l.shapes, lt, &[]).into_iter().map(|(_, p)| p).collect(),
        1 => rect(l.sw, l.sh),
        0 => rect(l.w, l.h),
        2 => match l.ref_id.as_ref().and_then(|id| anim.asset(id)) {
            Some(a) => rect(a.w, a.h),
            None => vec![],
        },
        5 => match &l.text {
            Some(td) => match td.docs.iter().rev().find(|(ft, _)| *ft <= lt).or(td.docs.first()) {
                Some((_, d)) => text::layout(d, fonts),
                None => vec![],
            },
            None => vec![],
        },
        _ => vec![],
    }
}

fn union_bounds(paths: &[Path]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for p in paths {
        if let Some(r) = p.bounds() {
            b = Some(match b {
                None => r,
                Some(o) => [o[0].min(r[0]), o[1].min(r[1]), o[2].max(r[2]), o[3].max(r[3])],
            });
        }
    }
    b
}

pub fn layer_geometry(anim: &Animation, fonts: &FontRegistry, id: &str, t: f64) -> Option<Geometry> {
    let (layers, idx) = find(anim, id)?;
    let l = &layers[idx];
    let m = scene::layer_matrix(layers, idx, t);
    let local = union_bounds(&local_paths(anim, fonts, l, t)).unwrap_or([0.0; 4]);
    let corners = [
        m.apply([local[0], local[1]]),
        m.apply([local[2], local[1]]),
        m.apply([local[2], local[3]]),
        m.apply([local[0], local[3]]),
    ];
    let xs = corners.iter().map(|c| c[0]);
    let ys = corners.iter().map(|c| c[1]);
    let bounds = [
        xs.clone().fold(f64::INFINITY, f64::min),
        ys.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.fold(f64::NEG_INFINITY, f64::max),
    ];
    let lt = scene::layer_time(l, t);
    let a = crate::anim::eval_or(l.ks.a.as_ref(), lt, &[0.0, 0.0]);
    Some(Geometry {
        id: id.to_string(),
        local,
        bounds,
        corners,
        matrix: [m.a, m.b, m.c, m.d, m.e, m.f],
        anchor: m.apply([a.first().copied().unwrap_or(0.0), a.get(1).copied().unwrap_or(0.0)]),
    })
}

fn max_stroke(items: &[Shape], t: f64) -> f64 {
    items.iter().fold(0.0, |acc, s| match s {
        Shape::Stroke(x) if !x.c.hd => acc.max(eval1(x.style.w.as_ref(), t, 0.0)),
        Shape::GStroke(x) if !x.c.hd => acc.max(eval1(x.style.w.as_ref(), t, 0.0)),
        Shape::Group(g) if !g.c.hd => acc.max(max_stroke(&g.items, t)),
        _ => acc,
    })
}

fn polyline(c: &geom::Contour) -> Vec<[f64; 2]> {
    let mut pts = vec![];
    for s in c.segments() {
        for k in 0..16 {
            pts.push(geom::cubic_at(&s, k as f64 / 16.0));
        }
    }
    if let Some(last) = c.segments().last() {
        pts.push(last[3]);
    } else if let Some(v) = c.v.first() {
        pts.push(*v);
    }
    pts
}

fn winding(poly: &[[f64; 2]], p: [f64; 2]) -> i32 {
    let mut w = 0;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (p[0] - a[0]) * (b[1] - a[1]);
        if a[1] <= p[1] {
            if b[1] > p[1] && cross > 0.0 {
                w += 1;
            }
        } else if b[1] <= p[1] && cross < 0.0 {
            w -= 1;
        }
    }
    w
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

/// Whether composition-space point `p` lies on the layer (fill or stroke).
fn contains(anim: &Animation, fonts: &FontRegistry, layers: &[Layer], idx: usize, t: f64, base: &Affine, p: [f64; 2]) -> bool {
    let l = &layers[idx];
    let m = base.mul(&scene::layer_matrix(layers, idx, t));
    let Some(inv) = m.invert() else { return false };
    let lp = inv.apply(p);
    let paths = local_paths(anim, fonts, l, t);
    if l.ty != 4 && l.ty != 5 {
        return union_bounds(&paths).map(|b| lp[0] >= b[0] && lp[0] <= b[2] && lp[1] >= b[1] && lp[1] <= b[3]).unwrap_or(false);
    }
    let tol = if l.ty == 4 { max_stroke(&l.shapes, scene::layer_time(l, t)) / 2.0 } else { 0.0 };
    let mut wind = 0;
    for path in &paths {
        for c in &path.contours {
            let poly = polyline(c);
            if poly.len() >= 3 {
                wind += winding(&poly, lp);
            }
            if tol > 0.0 && poly.windows(2).any(|w| seg_dist(lp, w[0], w[1]) <= tol) {
                return true;
            }
        }
    }
    wind != 0
}

/// Topmost visible root layer under (`x`, `y`), as a layer id.
pub fn hit_test(anim: &Animation, fonts: &FontRegistry, t: f64, x: f64, y: f64) -> Option<String> {
    hit_test_in(anim, fonts, "root", t, x, y)
}

/// Like [`hit_test`] for an animation whose root layers belong to composition `comp`.
pub fn hit_test_in(anim: &Animation, fonts: &FontRegistry, comp: &str, t: f64, x: f64, y: f64) -> Option<String> {
    for (idx, l) in anim.layers.iter().enumerate() {
        if l.td != 0 || !scene::is_visible(l, t) || l.ty == 3 {
            continue;
        }
        if contains(anim, fonts, &anim.layers, idx, t, &Affine::IDENTITY, [x, y]) {
            return Some(layer_id(comp, l));
        }
    }
    None
}

/// Names of every visible layer (at any precomp depth) under the point.
pub fn layers_named_at(anim: &Animation, fonts: &FontRegistry, t: f64, x: f64, y: f64) -> Vec<String> {
    let mut out = vec![];
    walk(anim, fonts, &anim.layers, t, &Affine::IDENTITY, [x, y], &mut out, 0);
    out
}

#[allow(clippy::too_many_arguments)]
fn walk(anim: &Animation, fonts: &FontRegistry, layers: &[Layer], t: f64, base: &Affine, p: [f64; 2], out: &mut Vec<String>, depth: u32) {
    if depth > 16 {
        return;
    }
    for (idx, l) in layers.iter().enumerate() {
        if l.td != 0 || !scene::is_visible(l, t) || l.ty == 3 {
            continue;
        }
        if contains(anim, fonts, layers, idx, t, base, p) && !l.nm.is_empty() {
            out.push(l.nm.clone());
        }
        if l.ty == 0 {
            if let Some(inner) = l.ref_id.as_ref().and_then(|id| anim.asset(id)).and_then(|a| a.layers.as_deref()) {
                let m = base.mul(&scene::layer_matrix(layers, idx, t));
                let local = (t - l.st) / if l.sr.abs() > 1e-9 { l.sr } else { 1.0 };
                walk(anim, fonts, inner, local, &m, p, out, depth + 1);
            }
        }
    }
}
