//! Shape-tree rendering with Lottie's ordering rules:
//! * a paint applies to every geometry item before it in its group, including
//!   geometry inside earlier nested groups;
//! * modifiers (trim, round corners) apply to geometry before them, and are
//!   inherited by nested groups for those groups' own paints;
//! * earlier items are drawn on top of later ones;
//! * a repeater replicates everything before it.

use tiny_skia::Pixmap;

use super::paint;
use crate::anim::{eval1, eval_or, eval_shape};
use crate::geom::{self, Affine, Path};
use crate::model::{Repeater, Shape, Transform};
use crate::scene;

#[derive(Clone, Debug)]
pub enum ModSpec {
    Trim { s: f64, e: f64, o: f64, individually: bool },
    Round(f64),
}

fn mod_of(item: &Shape, t: f64) -> Option<ModSpec> {
    match item {
        Shape::Trim(x) if !x.c.hd => Some(ModSpec::Trim {
            s: eval1(x.s.as_ref(), t, 0.0),
            e: eval1(x.e.as_ref(), t, 100.0),
            o: eval1(x.o.as_ref(), t, 0.0),
            individually: x.m == 2,
        }),
        Shape::Round(x) if !x.c.hd => Some(ModSpec::Round(eval1(x.r.as_ref(), t, 0.0))),
        _ => None,
    }
}

fn apply_mod(geoms: &mut [(usize, Path)], m: &ModSpec) {
    match m {
        ModSpec::Trim { s, e, o, individually } => {
            let paths: Vec<Path> = geoms.iter().map(|g| g.1.clone()).collect();
            let out = geom::trim(&paths, *s, *e, *o, *individually);
            for (g, p) in geoms.iter_mut().zip(out) {
                g.1 = p;
            }
        }
        ModSpec::Round(r) => {
            for g in geoms.iter_mut() {
                g.1 = Path { contours: g.1.contours.iter().map(|c| geom::round_corners(c, *r)).collect() };
            }
        }
    }
}

/// Geometry produced by a single primitive item, in its group's space.
pub fn primitive(item: &Shape, t: f64) -> Option<Path> {
    if item.common().hd {
        return None;
    }
    let v2 = |v: Vec<f64>, d: f64| [v.first().copied().unwrap_or(d), v.get(1).copied().unwrap_or(d)];
    let c = match item {
        Shape::Rect(r) => geom::rect(
            v2(eval_or(r.p.as_ref(), t, &[0.0, 0.0]), 0.0),
            v2(eval_or(r.s.as_ref(), t, &[0.0, 0.0]), 0.0),
            eval1(r.r.as_ref(), t, 0.0),
            r.d == 3,
        ),
        Shape::Ellipse(e) => geom::ellipse(v2(eval_or(e.p.as_ref(), t, &[0.0, 0.0]), 0.0), v2(eval_or(e.s.as_ref(), t, &[0.0, 0.0]), 0.0), e.d == 3),
        Shape::Star(s) => geom::star(
            s.sy,
            v2(eval_or(s.p.as_ref(), t, &[0.0, 0.0]), 0.0),
            eval1(s.pt.as_ref(), t, 5.0),
            eval1(s.or.as_ref(), t, 0.0),
            eval1(s.ir.as_ref(), t, 0.0),
            eval1(s.os.as_ref(), t, 0.0),
            eval1(s.is.as_ref(), t, 0.0),
            eval1(s.r.as_ref(), t, 0.0),
            s.d == 3,
        ),
        Shape::Path(p) => {
            let c = geom::from_bezier(&eval_shape(p.ks.as_ref()?, t));
            if p.d == 3 {
                c.reversed()
            } else {
                c
            }
        }
        _ => return None,
    };
    Some(Path::from_contour(c))
}

/// Matrix and opacity of the group transform item within `items`.
pub fn group_transform(items: &[Shape], t: f64) -> (Affine, f32) {
    match items.iter().rev().find_map(|s| if let Shape::Transform(tr) = s { Some(&tr.tr) } else { None }) {
        Some(tr) => (scene::transform_matrix(tr, t), scene::opacity(tr, t) as f32),
        None => (Affine::IDENTITY, 1.0),
    }
}

fn repeater_copy(r: &Repeater, t: f64, i: f64) -> (Affine, f32, usize) {
    let tr: &Transform = &r.tr;
    let a = eval_or(tr.a.as_ref(), t, &[0.0, 0.0]);
    let p = eval_or(tr.p.as_ref(), t, &[0.0, 0.0]);
    let s = eval_or(tr.s.as_ref(), t, &[100.0, 100.0]);
    let g = |v: &[f64], k: usize, d: f64| v.get(k).copied().unwrap_or(d);
    let rot = eval1(tr.r.as_ref(), t, 0.0);
    let n = eval1(r.copies.as_ref(), t, 1.0).floor().max(0.0) as usize;
    let k = i + eval1(r.offset.as_ref(), t, 0.0);
    let (sx, sy) = ((g(&s, 0, 100.0) / 100.0).powf(k), (g(&s, 1, g(&s, 0, 100.0)) / 100.0).powf(k));
    let (ax, ay) = (g(&a, 0, 0.0), g(&a, 1, 0.0));
    let m = Affine::translate(g(&p, 0, 0.0) * k, g(&p, 1, 0.0) * k)
        .mul(&Affine::translate(ax, ay))
        .mul(&Affine::rotate(rot * k))
        .mul(&Affine::scale(sx, sy))
        .mul(&Affine::translate(-ax, -ay));
    let so = eval1(tr.so.as_ref(), t, 100.0) / 100.0;
    let eo = eval1(tr.eo.as_ref(), t, 100.0) / 100.0;
    let f = if n > 1 { i / (n - 1) as f64 } else { 0.0 };
    (m, (so + (eo - so) * f).clamp(0.0, 1.0) as f32, n)
}

/// All geometry of a group (nested groups flattened) tagged with the index of
/// the top-level item it came from, with modifiers applied.
pub fn collect(items: &[Shape], t: f64, inherited: &[ModSpec]) -> Vec<(usize, Path)> {
    let mut geoms: Vec<(usize, Path)> = vec![];
    for (k, item) in items.iter().enumerate() {
        if item.common().hd {
            continue;
        }
        match item {
            Shape::Group(g) => {
                let (gm, _) = group_transform(&g.items, t);
                for (_, p) in collect(&g.items, t, &[]) {
                    geoms.push((k, p.transform(&gm)));
                }
            }
            Shape::Repeater(r) => {
                let (_, _, n) = repeater_copy(r, t, 0.0);
                let mut out = vec![];
                for i in 0..n {
                    let (m, _, _) = repeater_copy(r, t, i as f64);
                    out.extend(geoms.iter().map(|(idx, p)| (*idx, p.transform(&m))));
                }
                geoms = out;
            }
            _ => {
                if let Some(m) = mod_of(item, t) {
                    apply_mod(&mut geoms, &m);
                } else if let Some(p) = primitive(item, t) {
                    geoms.push((k, p));
                }
            }
        }
    }
    for m in inherited {
        apply_mod(&mut geoms, m);
    }
    geoms
}

fn union_before(geoms: &[(usize, Path)], k: usize) -> Option<tiny_skia::Path> {
    let mut p = Path::default();
    for (idx, g) in geoms {
        if *idx < k {
            p.contours.extend(g.contours.iter().cloned());
        }
    }
    p.to_skia()
}

/// Render the items of one group.
pub fn render_items(canvas: &mut Pixmap, items: &[Shape], m: &Affine, alpha: f32, t: f64, inherited: &[ModSpec]) {
    if alpha <= 0.0 {
        return;
    }
    let geoms = collect(items, t, inherited);
    let rep = items.iter().position(|s| matches!(s, Shape::Repeater(r) if !r.c.hd));
    let lo = rep.map(|r| r + 1).unwrap_or(0);
    for k in (lo..items.len()).rev() {
        let item = &items[k];
        if item.common().hd {
            continue;
        }
        match item {
            Shape::Fill(f) => {
                if let Some(path) = union_before(&geoms, k) {
                    let c = paint::rgb(&eval_or(f.color.as_ref(), t, &[0.0, 0.0, 0.0]));
                    let a = alpha * (eval1(f.o.as_ref(), t, 100.0) / 100.0) as f32;
                    paint::fill(canvas, &path, &paint::solid_paint(c, a), f.rule, m);
                }
            }
            Shape::GFill(f) => {
                if let Some(path) = union_before(&geoms, k) {
                    let a = alpha * (eval1(f.o.as_ref(), t, 100.0) / 100.0) as f32;
                    if let Some(sh) = paint::gradient_shader(&f.g, t, a) {
                        paint::fill(canvas, &path, &paint::brush_paint(&paint::Brush::Gradient(sh), a), f.rule, m);
                    }
                }
            }
            Shape::Stroke(s) => {
                if let (Some(path), Some(spec)) = (union_before(&geoms, k), paint::stroke_spec(&s.style, t)) {
                    let c = paint::rgb(&eval_or(s.color.as_ref(), t, &[0.0, 0.0, 0.0]));
                    let a = alpha * (eval1(s.o.as_ref(), t, 100.0) / 100.0) as f32;
                    paint::stroke(canvas, &path, &paint::solid_paint(c, a), &spec, m);
                }
            }
            Shape::GStroke(s) => {
                if let (Some(path), Some(spec)) = (union_before(&geoms, k), paint::stroke_spec(&s.style, t)) {
                    let a = alpha * (eval1(s.o.as_ref(), t, 100.0) / 100.0) as f32;
                    if let Some(sh) = paint::gradient_shader(&s.g, t, a) {
                        paint::stroke(canvas, &path, &paint::brush_paint(&paint::Brush::Gradient(sh), a), &spec, m);
                    }
                }
            }
            Shape::Group(g) => {
                let (gm, ga) = group_transform(&g.items, t);
                let mut child_mods: Vec<ModSpec> = items[k + 1..].iter().filter_map(|s| mod_of(s, t)).collect();
                child_mods.extend_from_slice(inherited);
                render_items(canvas, &g.items, &m.mul(&gm), alpha * ga, t, &child_mods);
            }
            _ => {}
        }
    }
    if let Some(r) = rep {
        let Shape::Repeater(rp) = &items[r] else { return };
        let (_, _, n) = repeater_copy(rp, t, 0.0);
        let mut child_mods: Vec<ModSpec> = items[r + 1..].iter().filter_map(|s| mod_of(s, t)).collect();
        child_mods.extend_from_slice(inherited);
        let order: Vec<usize> = if rp.m == 2 { (0..n).rev().collect() } else { (0..n).collect() };
        for i in order {
            let (cm, ca, _) = repeater_copy(rp, t, i as f64);
            render_items(canvas, &items[..r], &m.mul(&cm), alpha * ca, t, &child_mods);
        }
    }
}
