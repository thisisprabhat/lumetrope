//! SVG → Lottie shape items (via usvg). Every SVG path becomes a shape group
//! in absolute SVG coordinates with its own fill/stroke; SVG paint order is
//! reversed because earlier Lottie items draw on top.

use serde_json::{json, Value};

fn st(v: Value) -> Value {
    json!({"a": 0, "k": v})
}

fn map(t: &usvg::Transform, x: f32, y: f32) -> [f64; 2] {
    [(t.sx * x + t.kx * y + t.tx) as f64, (t.ky * x + t.sy * y + t.ty) as f64]
}

fn rgb(c: usvg::Color) -> [f64; 3] {
    [c.red as f64 / 255.0, c.green as f64 / 255.0, c.blue as f64 / 255.0]
}

/// Convert a usvg path into Lottie `sh` items (one per subpath).
fn contours(path: &usvg::Path, t: &usvg::Transform) -> Vec<Value> {
    let mut out = vec![];
    let (mut v, mut ins, mut outs): (Vec<[f64; 2]>, Vec<[f64; 2]>, Vec<[f64; 2]>) = (vec![], vec![], vec![]);
    let mut closed = false;
    let flush = |v: &mut Vec<[f64; 2]>, i: &mut Vec<[f64; 2]>, o: &mut Vec<[f64; 2]>, closed: &mut bool, out: &mut Vec<Value>| {
        if v.len() >= 2 {
            // A closing segment that returns to the start duplicates the first vertex.
            if *closed && v.len() > 2 {
                let (a, b) = (v[0], v[v.len() - 1]);
                if (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6 {
                    let last_in = i.pop().unwrap();
                    v.pop();
                    o.pop();
                    i[0] = last_in;
                }
            }
            out.push(json!({"ty": "sh", "nm": "Path", "d": 1, "ks": st(json!({"c": *closed, "v": v.clone(), "i": i.clone(), "o": o.clone()}))}));
        }
        v.clear();
        i.clear();
        o.clear();
        *closed = false;
    };
    for seg in path.data().segments() {
        use usvg::tiny_skia_path::PathSegment as S;
        match seg {
            S::MoveTo(p) => {
                flush(&mut v, &mut ins, &mut outs, &mut closed, &mut out);
                v.push(map(t, p.x, p.y));
                ins.push([0.0, 0.0]);
                outs.push([0.0, 0.0]);
            }
            S::LineTo(p) => {
                v.push(map(t, p.x, p.y));
                ins.push([0.0, 0.0]);
                outs.push([0.0, 0.0]);
            }
            S::QuadTo(q, p) => {
                let p0 = *v.last().unwrap_or(&[0.0, 0.0]);
                let (q, p) = (map(t, q.x, q.y), map(t, p.x, p.y));
                let c1 = [p0[0] + 2.0 / 3.0 * (q[0] - p0[0]), p0[1] + 2.0 / 3.0 * (q[1] - p0[1])];
                let c2 = [p[0] + 2.0 / 3.0 * (q[0] - p[0]), p[1] + 2.0 / 3.0 * (q[1] - p[1])];
                if let Some(o) = outs.last_mut() {
                    *o = [c1[0] - p0[0], c1[1] - p0[1]];
                }
                v.push(p);
                ins.push([c2[0] - p[0], c2[1] - p[1]]);
                outs.push([0.0, 0.0]);
            }
            S::CubicTo(a, b, p) => {
                let p0 = *v.last().unwrap_or(&[0.0, 0.0]);
                let (c1, c2, p) = (map(t, a.x, a.y), map(t, b.x, b.y), map(t, p.x, p.y));
                if let Some(o) = outs.last_mut() {
                    *o = [c1[0] - p0[0], c1[1] - p0[1]];
                }
                v.push(p);
                ins.push([c2[0] - p[0], c2[1] - p[1]]);
                outs.push([0.0, 0.0]);
            }
            S::Close => {
                closed = true;
                flush(&mut v, &mut ins, &mut outs, &mut closed, &mut out);
            }
        }
    }
    flush(&mut v, &mut ins, &mut outs, &mut closed, &mut out);
    out
}

fn gradient(paint: &usvg::Paint, t: &usvg::Transform, alpha: f64, stroke: Option<(f64, i64, i64, f64)>) -> Option<Value> {
    let (stops, kind, s, e, h) = match paint {
        usvg::Paint::LinearGradient(g) => {
            let gt = t.pre_concat(g.transform());
            (g.stops(), 1, map(&gt, g.x1(), g.y1()), map(&gt, g.x2(), g.y2()), 0.0)
        }
        usvg::Paint::RadialGradient(g) => {
            let gt = t.pre_concat(g.transform());
            let c = map(&gt, g.cx(), g.cy());
            let edge = map(&gt, g.cx() + g.r().get(), g.cy());
            let f = map(&gt, g.fx(), g.fy());
            let r = ((edge[0] - c[0]).powi(2) + (edge[1] - c[1]).powi(2)).sqrt().max(1e-6);
            let h = (((f[0] - c[0]).powi(2) + (f[1] - c[1]).powi(2)).sqrt() / r * 100.0).min(99.0);
            (g.stops(), 2, c, edge, h)
        }
        _ => return None,
    };
    let mut k = vec![];
    for s in stops {
        let c = rgb(s.color());
        k.extend([s.offset().get() as f64, c[0], c[1], c[2]]);
    }
    if stops.iter().any(|s| s.opacity().get() < 1.0) {
        for s in stops {
            k.extend([s.offset().get() as f64, s.opacity().get() as f64]);
        }
    }
    let mut item = json!({"ty": if stroke.is_some() { "gs" } else { "gf" }, "nm": "Gradient", "t": kind, "o": st(json!(alpha * 100.0)), "r": 1,
        "s": st(json!(s)), "e": st(json!(e)), "g": {"p": stops.len(), "k": st(json!(k))}});
    if kind == 2 {
        item["h"] = st(json!(h));
        item["a"] = st(json!(0));
    }
    if let Some((w, lc, lj, ml)) = stroke {
        item["w"] = st(json!(w));
        item["lc"] = json!(lc);
        item["lj"] = json!(lj);
        item["ml"] = json!(ml);
    }
    Some(item)
}

fn paint_items(p: &usvg::Path, t: &usvg::Transform, opacity: f64) -> Vec<Value> {
    let scale = ((t.sx * t.sy - t.kx * t.ky).abs() as f64).sqrt();
    let mut items = vec![];
    let stroke = p.stroke().and_then(|s| {
        let a = opacity * s.opacity().get() as f64;
        let lc = match s.linecap() {
            usvg::LineCap::Butt => 1,
            usvg::LineCap::Round => 2,
            usvg::LineCap::Square => 3,
        };
        let lj = match s.linejoin() {
            usvg::LineJoin::Round => 2,
            usvg::LineJoin::Bevel => 3,
            _ => 1,
        };
        let w = s.width().get() as f64 * scale;
        let ml = s.miterlimit().get() as f64;
        match s.paint() {
            usvg::Paint::Color(c) => {
                let c = rgb(*c);
                let mut it = json!({"ty": "st", "nm": "Stroke", "c": st(json!([c[0], c[1], c[2], 1])), "o": st(json!(a * 100.0)), "w": st(json!(w)), "lc": lc, "lj": lj, "ml": ml});
                if let Some(d) = s.dasharray() {
                    let mut ds = vec![];
                    for (i, v) in d.iter().enumerate() {
                        ds.push(json!({"n": if i % 2 == 0 { "d" } else { "g" }, "v": st(json!(*v as f64 * scale))}));
                    }
                    ds.push(json!({"n": "o", "v": st(json!(s.dashoffset() as f64 * scale))}));
                    it["d"] = json!(ds);
                }
                Some(it)
            }
            other => gradient(other, t, a, Some((w, lc, lj, ml))),
        }
    });
    let fill = p.fill().and_then(|f| {
        let a = opacity * f.opacity().get() as f64;
        let rule = if f.rule() == usvg::FillRule::EvenOdd { 2 } else { 1 };
        match f.paint() {
            usvg::Paint::Color(c) => {
                let c = rgb(*c);
                Some(json!({"ty": "fl", "nm": "Fill", "c": st(json!([c[0], c[1], c[2], 1])), "o": st(json!(a * 100.0)), "r": rule}))
            }
            other => gradient(other, t, a, None).map(|mut g| {
                g["r"] = json!(rule);
                g
            }),
        }
    });
    // Earlier = on top. SVG default paints fill, then stroke over it.
    match p.paint_order() {
        usvg::PaintOrder::FillAndStroke => items.extend(stroke.into_iter().chain(fill)),
        usvg::PaintOrder::StrokeAndFill => items.extend(fill.into_iter().chain(stroke)),
    }
    items
}

fn walk(g: &usvg::Group, opacity: f64, out: &mut Vec<Value>) {
    let opacity = opacity * g.opacity().get() as f64;
    for n in g.children() {
        match n {
            usvg::Node::Group(c) => walk(c, opacity, out),
            usvg::Node::Path(p) if p.is_visible() => {
                let t = p.abs_transform();
                let mut it = contours(p, &t);
                if it.is_empty() {
                    continue;
                }
                it.extend(paint_items(p, &t, opacity));
                it.push(json!({"ty": "tr", "p": st(json!([0, 0])), "a": st(json!([0, 0])), "s": st(json!([100, 100])), "r": st(json!(0)), "o": st(json!(100))}));
                out.push(json!({"ty": "gr", "nm": p.id().is_empty().then(|| "Path".to_string()).unwrap_or_else(|| p.id().to_string()), "it": it}));
            }
            _ => {}
        }
    }
}

/// Parse SVG text into Lottie shape groups (top-most first) and the SVG size.
pub fn to_shapes(svg: &str) -> Result<(Vec<Value>, [f64; 2]), String> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| format!("invalid SVG: {e}"))?;
    let mut groups = vec![];
    walk(tree.root(), 1.0, &mut groups);
    groups.reverse();
    let s = tree.size();
    Ok((groups, [s.width() as f64, s.height() as f64]))
}
