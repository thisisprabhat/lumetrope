//! Display-list backend: evaluates the animation exactly like the CPU renderer
//! but records drawing commands (JSON) for a GPU-accelerated Canvas2D executor
//! in the UI. Paths are SVG path data in local space; every command carries its
//! absolute transform (camera included), so the executor never tracks state.
//!
//! Commands:
//! * `{"op":"fill","d","t","rule","paint"}`
//! * `{"op":"stroke","d","t","w","cap","join","ml","dash","doff","paint"}`
//! * `{"op":"image","id","t","w","h","a"}`
//! * `{"op":"group","cmds","alpha","blend","clip","masks","matte","fx"}` — an
//!   offscreen layer for compositing features.
//!
//! Paint: `{"c":[r,g,b,a]}`, `{"lin":[x0,y0,x1,y1],"stops"}` or
//! `{"rad":[fx,fy,cx,cy,r],"stops"}` with stops `[offset,r,g,b,a]` (0..1).

use std::fmt::Write as _;

use serde_json::{json, Value};

use super::paint::{merged_stops, rgb};
use super::shapes::{collect, group_transform, mod_of, repeater_copy, ModSpec};
use crate::anim::{eval1, eval_or, eval_shape};
use crate::geom::{self, Affine, Path};
use crate::model::{Animation, Gradient, Layer, Shape, StrokeStyle};
use crate::scene;
use crate::text::{self, FontRegistry};

fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// SVG path data for a geometry path.
pub fn path_d(p: &Path) -> String {
    let mut s = String::new();
    let n = |v: f64| {
        let r = r3(v);
        if r == r.trunc() {
            format!("{}", r as i64)
        } else {
            format!("{r}")
        }
    };
    for c in &p.contours {
        if c.v.is_empty() || c.v.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) {
            continue;
        }
        let _ = write!(s, "M{} {}", n(c.v[0][0]), n(c.v[0][1]));
        for seg in c.segments() {
            let line = seg[1] == seg[0] && seg[2] == seg[3];
            if line {
                let _ = write!(s, "L{} {}", n(seg[3][0]), n(seg[3][1]));
            } else {
                let _ = write!(s, "C{} {} {} {} {} {}", n(seg[1][0]), n(seg[1][1]), n(seg[2][0]), n(seg[2][1]), n(seg[3][0]), n(seg[3][1]));
            }
        }
        if c.closed {
            s.push('Z');
        }
    }
    s
}

fn tj(m: &Affine) -> Value {
    json!([r3(m.a), r3(m.b), r3(m.c), r3(m.d), r3(m.e), r3(m.f)])
}

fn solid(c: [f32; 3], a: f32) -> Value {
    json!({"c": [c[0] as f64, c[1] as f64, c[2] as f64, (a.clamp(0.0, 1.0) as f64 * 1000.0).round() / 1000.0]})
}

fn gradient_paint(g: &Gradient, t: f64, alpha: f32) -> Option<Value> {
    let stops = merged_stops(g, t, alpha);
    if stops.is_empty() {
        return None;
    }
    let sv: Vec<Value> = stops.iter().map(|(o, c)| json!([*o as f64, c.red() as f64, c.green() as f64, c.blue() as f64, (c.alpha() as f64 * 1000.0).round() / 1000.0])).collect();
    if stops.len() == 1 {
        let c = stops[0].1;
        return Some(json!({"c": [c.red(), c.green(), c.blue(), c.alpha()]}));
    }
    let s = eval_or(g.s.as_ref(), t, &[0.0, 0.0]);
    let e = eval_or(g.e.as_ref(), t, &[0.0, 0.0]);
    let (sx, sy, ex, ey) = (s[0], s.get(1).copied().unwrap_or(0.0), e[0], e.get(1).copied().unwrap_or(0.0));
    if g.t == 2 {
        let r = ((ex - sx).powi(2) + (ey - sy).powi(2)).sqrt();
        if r <= 1e-6 {
            return None;
        }
        let h = (eval1(g.h.as_ref(), t, 0.0) / 100.0).clamp(-0.99, 0.99);
        let ang = (ey - sy).atan2(ex - sx) + eval1(g.a.as_ref(), t, 0.0).to_radians();
        Some(json!({"rad": [r3(sx + h * r * ang.cos()), r3(sy + h * r * ang.sin()), r3(sx), r3(sy), r3(r)], "stops": sv}))
    } else {
        Some(json!({"lin": [r3(sx), r3(sy), r3(ex), r3(ey)], "stops": sv}))
    }
}

fn stroke_fields(style: &StrokeStyle, t: f64) -> Option<Value> {
    let w = eval1(style.w.as_ref(), t, 1.0);
    if !(w > 0.0) || !w.is_finite() {
        return None;
    }
    let mut dash = vec![];
    let mut off = 0.0;
    for d in &style.dashes {
        let v = eval1(d.v.as_ref(), t, 0.0);
        if d.n == "o" {
            off = v;
        } else {
            dash.push(r3(v.max(0.0)));
        }
    }
    if dash.iter().all(|x| *x <= 0.0) {
        dash.clear();
    }
    Some(json!({
        "w": r3(w),
        "cap": match style.lc { 1 => "butt", 3 => "square", _ => "round" },
        "join": match style.lj { 1 => "miter", 3 => "bevel", _ => "round" },
        "ml": style.ml.max(1.0), "dash": dash, "doff": r3(off)
    }))
}

const BLEND: [&str; 17] = [
    "source-over", "multiply", "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn", "hard-light", "soft-light", "difference",
    "exclusion", "hue", "saturation", "color", "luminosity", "lighter",
];

struct Rec<'a> {
    anim: &'a Animation,
    fonts: &'a FontRegistry,
}

impl<'a> Rec<'a> {
    fn union_before(geoms: &[(usize, Path)], k: usize) -> Option<String> {
        let mut p = Path::default();
        for (idx, g) in geoms {
            if *idx < k {
                p.contours.extend(g.contours.iter().cloned());
            }
        }
        let d = path_d(&p);
        (!d.is_empty()).then_some(d)
    }

    fn items(&self, out: &mut Vec<Value>, items: &[Shape], m: &Affine, alpha: f32, t: f64, inherited: &[ModSpec]) {
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
                    if let Some(d) = Self::union_before(&geoms, k) {
                        let a = alpha * (eval1(f.o.as_ref(), t, 100.0) / 100.0) as f32;
                        let c = rgb(&eval_or(f.color.as_ref(), t, &[0.0, 0.0, 0.0]));
                        out.push(json!({"op": "fill", "d": d, "t": tj(m), "rule": if f.rule == 2 { "evenodd" } else { "nonzero" }, "paint": solid(c, a)}));
                    }
                }
                Shape::GFill(f) => {
                    if let Some(d) = Self::union_before(&geoms, k) {
                        let a = alpha * (eval1(f.o.as_ref(), t, 100.0) / 100.0) as f32;
                        if let Some(p) = gradient_paint(&f.g, t, a) {
                            out.push(json!({"op": "fill", "d": d, "t": tj(m), "rule": if f.rule == 2 { "evenodd" } else { "nonzero" }, "paint": p}));
                        }
                    }
                }
                Shape::Stroke(st) => {
                    if let (Some(d), Some(mut cmd)) = (Self::union_before(&geoms, k), stroke_fields(&st.style, t)) {
                        let a = alpha * (eval1(st.o.as_ref(), t, 100.0) / 100.0) as f32;
                        let c = rgb(&eval_or(st.color.as_ref(), t, &[0.0, 0.0, 0.0]));
                        cmd["op"] = json!("stroke");
                        cmd["d"] = json!(d);
                        cmd["t"] = tj(m);
                        cmd["paint"] = solid(c, a);
                        out.push(cmd);
                    }
                }
                Shape::GStroke(st) => {
                    if let (Some(d), Some(mut cmd)) = (Self::union_before(&geoms, k), stroke_fields(&st.style, t)) {
                        let a = alpha * (eval1(st.o.as_ref(), t, 100.0) / 100.0) as f32;
                        if let Some(p) = gradient_paint(&st.g, t, a) {
                            cmd["op"] = json!("stroke");
                            cmd["d"] = json!(d);
                            cmd["t"] = tj(m);
                            cmd["paint"] = p;
                            out.push(cmd);
                        }
                    }
                }
                Shape::Group(g) => {
                    let (gm, ga) = group_transform(&g.items, t);
                    let mut child_mods: Vec<ModSpec> = items[k + 1..].iter().filter_map(|s| mod_of(s, t)).collect();
                    child_mods.extend_from_slice(inherited);
                    self.items(out, &g.items, &m.mul(&gm), alpha * ga, t, &child_mods);
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
                self.items(out, &items[..r], &m.mul(&cm), alpha * ca, t, &child_mods);
            }
        }
    }

    fn comp(&self, layers: &[Layer], t: f64, base: &Affine, depth: u32) -> Vec<Value> {
        let mut out = vec![];
        if depth > 16 {
            return out;
        }
        for idx in (0..layers.len()).rev() {
            let l = &layers[idx];
            if l.td != 0 || !scene::is_visible(l, t) {
                continue;
            }
            self.layer(&mut out, layers, idx, t, base, depth, false);
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn layer(&self, out: &mut Vec<Value>, layers: &[Layer], idx: usize, t: f64, base: &Affine, depth: u32, as_matte: bool) {
        let l = &layers[idx];
        let m = base.mul(&scene::layer_matrix(layers, idx, t));
        let lt = scene::layer_time(l, t);
        let alpha = scene::opacity(&l.ks, lt) as f32;
        let mut content = vec![];
        let mut group_alpha = 1.0f32;
        let mut clip = Value::Null;
        if alpha > 0.0 {
            match l.ty {
                4 => self.items(&mut content, &l.shapes, &m, alpha, lt, &[]),
                1 => {
                    let c = super::layers_hex(l.sc.as_deref().unwrap_or("#000000"));
                    let d = path_d(&Path::from_contour(geom::rect([l.sw / 2.0, l.sh / 2.0], [l.sw, l.sh], 0.0, false)));
                    content.push(json!({"op": "fill", "d": d, "t": tj(&m), "rule": "nonzero", "paint": solid(c, alpha)}));
                }
                2 => {
                    if let Some(a) = l.ref_id.as_ref().and_then(|id| self.anim.asset(id)) {
                        content.push(json!({"op": "image", "id": a.id, "t": tj(&m), "w": a.w, "h": a.h, "a": alpha}));
                    }
                }
                0 => {
                    if let Some(asset) = l.ref_id.as_ref().and_then(|id| self.anim.asset(id)) {
                        if let Some(inner) = &asset.layers {
                            let local = match &l.tm {
                                Some(tm) => eval1(Some(tm), lt, 0.0) * asset.fr.unwrap_or(self.anim.fr),
                                None => (t - l.st) / if l.sr.abs() > 1e-9 { l.sr } else { 1.0 },
                            };
                            content = self.comp(inner, local, &m, depth + 1);
                            group_alpha = alpha;
                            if l.w > 0.0 && l.h > 0.0 {
                                clip = json!({"d": path_d(&Path::from_contour(geom::rect([l.w / 2.0, l.h / 2.0], [l.w, l.h], 0.0, false))), "t": tj(&m)});
                            }
                        }
                    }
                }
                5 => self.text(&mut content, l, lt, &m, alpha),
                _ => {}
            }
        }
        let masks: Vec<Value> = l
            .masks
            .iter()
            .filter(|mk| mk.mode != "n")
            .map(|mk| {
                let d = mk.pt.as_ref().map(|pt| path_d(&Path::from_contour(geom::from_bezier(&eval_shape(pt, lt))))).unwrap_or_default();
                json!({"d": d, "t": tj(&m), "mode": mk.mode, "inv": mk.inv, "o": eval1(mk.o.as_ref(), lt, 100.0) / 100.0})
            })
            .collect();
        let fx = self.effects(l, lt, base.scale_factor());
        let matte = if l.tt != 0 && !as_matte {
            let src = match l.tp {
                Some(ind) => layers.iter().position(|x| x.ind == Some(ind)),
                None => idx.checked_sub(1),
            };
            src.map(|si| {
                let mut mc = vec![];
                if scene::is_visible(&layers[si], t) || layers[si].hd {
                    self.layer(&mut mc, layers, si, t, base, depth + 1, true);
                }
                json!({"mode": match l.tt { 2 => "alpha_inv", 3 => "luma", 4 => "luma_inv", _ => "alpha" }, "cmds": mc})
            })
        } else {
            None
        };
        let needs_group = !masks.is_empty() || !fx.is_empty() || matte.is_some() || l.bm != 0 || l.ty == 0;
        if !needs_group {
            out.extend(content);
            return;
        }
        let mut g = json!({"op": "group", "cmds": content, "alpha": group_alpha, "blend": BLEND.get(l.bm as usize).copied().unwrap_or("source-over")});
        if !clip.is_null() {
            g["clip"] = clip;
        }
        if !masks.is_empty() {
            g["masks"] = Value::Array(masks);
        }
        if let Some(mt) = matte {
            g["matte"] = mt;
        }
        if !fx.is_empty() {
            g["fx"] = Value::Array(fx);
        }
        out.push(g);
    }

    fn effects(&self, l: &Layer, t: f64, scale: f64) -> Vec<Value> {
        let mut out = vec![];
        for e in l.ef.iter().filter(|e| e.enabled) {
            let val = |i: usize, d: f64| e.values.get(i).map(|v| eval1(v.v.as_ref(), t, d)).unwrap_or(d);
            let col = |i: usize, d: [f64; 3]| {
                let c = rgb(&e.values.get(i).map(|v| eval_or(v.v.as_ref(), t, &d)).unwrap_or_else(|| d.to_vec()));
                json!([c[0], c[1], c[2]])
            };
            match e.ty {
                29 => out.push(json!({"type": "blur", "sigma": r3(val(0, 0.0) * scale / 2.0), "dims": val(1, 1.0) as i64})),
                25 => {
                    let dir = val(2, 135.0).to_radians();
                    let dist = val(3, 0.0) * scale;
                    out.push(json!({"type": "shadow", "color": col(0, [0.0; 3]), "opacity": r3((val(1, 127.5) / 255.0).clamp(0.0, 1.0)),
                        "dx": r3(dir.sin() * dist), "dy": r3(-dir.cos() * dist), "blur": r3(val(4, 0.0) * scale), "only": val(5, 0.0) != 0.0}));
                }
                21 => out.push(json!({"type": "fill", "color": col(2, [1.0, 0.0, 0.0]), "opacity": val(6, 1.0).clamp(0.0, 1.0)})),
                20 => out.push(json!({"type": "tint", "black": col(0, [0.0; 3]), "white": col(1, [1.0; 3]), "amount": val(2, 100.0) / 100.0})),
                23 => out.push(json!({"type": "tritone", "hi": col(0, [1.0; 3]), "mid": col(1, [0.5; 3]), "lo": col(2, [0.0; 3]), "amount": 1.0 - val(3, 0.0) / 100.0})),
                _ => {}
            }
        }
        out
    }

    fn text(&self, out: &mut Vec<Value>, l: &Layer, t: f64, m: &Affine, alpha: f32) {
        let Some(td) = &l.text else { return };
        let Some((_, doc)) = td.docs.iter().rev().find(|(ft, _)| *ft <= t).or(td.docs.first()) else { return };
        let mut all = Path::default();
        for g in text::layout(doc, self.fonts) {
            all.extend(g);
        }
        let d = path_d(&all);
        if d.is_empty() {
            return;
        }
        let stroke = |out: &mut Vec<Value>| {
            if let (Some(c), true) = (&doc.stroke, doc.stroke_width > 0.0) {
                out.push(json!({"op": "stroke", "d": d, "t": tj(m), "w": doc.stroke_width, "cap": "round", "join": "round", "ml": 4, "dash": [], "doff": 0, "paint": solid(rgb(c), alpha)}));
            }
        };
        let fill = |out: &mut Vec<Value>| {
            if let Some(c) = &doc.fill {
                out.push(json!({"op": "fill", "d": d, "t": tj(m), "rule": "nonzero", "paint": solid(rgb(c), alpha)}));
            }
        };
        // Commands draw in order (later on top), unlike Lottie item order.
        if doc.stroke_over_fill {
            fill(out);
            stroke(out);
        } else {
            stroke(out);
            fill(out);
        }
    }
}

/// Record the display list for `anim` at `frame` with camera/base transform `base`.
pub fn record(anim: &Animation, fonts: &FontRegistry, frame: f64, base: &Affine) -> Value {
    let r = Rec { anim, fonts };
    Value::Array(r.comp(&anim.layers, frame, base, 0))
}
