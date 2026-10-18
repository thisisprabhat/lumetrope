//! Shape-item tools (inside shape layers) and paint tools.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::layers::{self, fill_item, shape_geometry_props, stroke_item};
use super::util::{self, default_ks, parse_color, shape_ptr, static_prop};
use super::{arg_str, err, opt_bool, opt_f64, opt_str, opt_vec, Cx, LayerRef, OpError, OpResult};

fn base() -> Value {
    json!({"layer": {"type": "string", "description": "Layer id or unique name"},
           "shape": {"type": "string", "description": "Shape path, e.g. '0' or '0/2' (group children are nested)"}})
}

fn with(mut b: Value, e: Value) -> Value {
    if let (Some(o), Value::Object(x)) = (b.as_object_mut(), e) {
        o.extend(x);
    }
    b
}

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "add_shape",
            description: "Add a shape (rect, ellipse, polygon, star, path) as a new group inside an existing shape layer, on top of its other shapes. 'shape' may name a group to add into. Position is in layer space.",
            schema: || obj(with(base(), shape_geometry_props()), &["layer", "type"]),
            mutates: true,
            handler: add_shape,
        },
        ToolDef {
            name: "remove_shape_item",
            description: "Delete a shape item (geometry, paint, modifier or whole group) from a shape layer.",
            schema: || obj(base(), &["layer", "shape"]),
            mutates: true,
            handler: remove_shape_item,
        },
        ToolDef {
            name: "set_path",
            description: "Replace a path shape's bezier {v:[[x,y]...], i, o, c} (tangents relative to vertices). Animated paths get a keyframe at 'frame'.",
            schema: || obj(with(base(), json!({"path": {"type": "object"}, "frame": {"type": "number"}})), &["layer", "shape", "path"]),
            mutates: true,
            handler: set_path,
        },
        ToolDef {
            name: "add_trim_path",
            description: "Add a trim-paths modifier (start/end in %, offset in degrees) affecting all paths before it in the layer or group. Animate 'end' 0→100 for a draw-on effect.",
            schema: || obj(with(base(), json!({"start": {"type": "number"}, "end": {"type": "number"}, "offset": {"type": "number"}, "individually": {"type": "boolean"}})), &["layer"]),
            mutates: true,
            handler: add_trim_path,
        },
        ToolDef {
            name: "add_round_corners",
            description: "Add a round-corners modifier with the given radius to the layer or group.",
            schema: || obj(with(base(), json!({"radius": {"type": "number", "minimum": 0}})), &["layer", "radius"]),
            mutates: true,
            handler: add_round_corners,
        },
        ToolDef {
            name: "add_repeater",
            description: "Add a repeater that copies everything above it N times, each copy offset by position/rotation/scale, fading from start_opacity to end_opacity.",
            schema: || {
                obj(
                    with(base(), json!({"copies": {"type": "number", "minimum": 1}, "offset": {"type": "number"}, "position": {"type": "array"}, "rotation": {"type": "number"},
                        "scale": {"type": "array"}, "anchor": {"type": "array"}, "start_opacity": {"type": "number"}, "end_opacity": {"type": "number"}, "above": {"type": "boolean"}})),
                    &["layer", "copies"],
                )
            },
            mutates: true,
            handler: add_repeater,
        },
        ToolDef {
            name: "update_shape_item",
            description: "Update a shape item: animatable 'props' by friendly name (size, roundness, color, opacity, width, start, end, …) plus name, hidden, direction (1 or 3), fill_rule (non_zero|even_odd), line_cap (butt|round|square), line_join (miter|round|bevel), miter_limit, trim_mode (simultaneous|individual).",
            schema: || {
                obj(
                    with(base(), json!({"props": {"type": "object"}, "name": {"type": "string"}, "hidden": {"type": "boolean"}, "direction": {"type": "integer"},
                        "fill_rule": {"type": "string"}, "line_cap": {"type": "string"}, "line_join": {"type": "string"}, "miter_limit": {"type": "number"},
                        "trim_mode": {"type": "string"}, "frame": {"type": "number"}})),
                    &["layer", "shape"],
                )
            },
            mutates: true,
            handler: update_shape_item,
        },
        ToolDef {
            name: "reorder_shape",
            description: "Move a shape item to a new index within its parent list (0 = top).",
            schema: || obj(with(base(), json!({"index": {"type": "integer", "minimum": 0}})), &["layer", "shape", "index"]),
            mutates: true,
            handler: reorder_shape,
        },
        ToolDef {
            name: "set_fill",
            description: "Set (or add) the solid fill of a shape layer or group: colour and opacity %. Animated colours get a keyframe at 'frame'.",
            schema: || obj(with(base(), json!({"color": {}, "opacity": {"type": "number"}, "frame": {"type": "number"}})), &["layer"]),
            mutates: true,
            handler: set_fill,
        },
        ToolDef {
            name: "set_stroke",
            description: "Set (or add) the stroke of a shape layer or group: colour, width, opacity, line_cap, line_join, dash [dash, gap].",
            schema: || {
                obj(
                    with(base(), json!({"color": {}, "width": {"type": "number"}, "opacity": {"type": "number"}, "line_cap": {"type": "string", "enum": ["butt", "round", "square"]},
                        "line_join": {"type": "string", "enum": ["miter", "round", "bevel"]}, "dash": {"type": ["array", "null"]}, "frame": {"type": "number"}})),
                    &["layer"],
                )
            },
            mutates: true,
            handler: set_stroke,
        },
        ToolDef {
            name: "set_gradient",
            description: "Give a shape layer or group a linear or radial gradient fill/stroke. stops: [{offset 0..1, color, opacity 0..1?}]. start/end points in layer space default to the shape's bounds.",
            schema: || {
                obj(
                    with(base(), json!({"kind": {"type": "string", "enum": ["fill", "stroke"]}, "type": {"type": "string", "enum": ["linear", "radial"]},
                        "stops": {"type": "array", "minItems": 1}, "start": {"type": "array"}, "end": {"type": "array"}, "highlight": {"type": "number"}, "angle": {"type": "number"}, "width": {"type": "number"}})),
                    &["layer", "stops"],
                )
            },
            mutates: true,
            handler: set_gradient,
        },
        ToolDef {
            name: "remove_paint",
            description: "Remove fills, strokes (solid or gradient) or both from a shape layer or group.",
            schema: || obj(with(base(), json!({"kind": {"type": "string", "enum": ["fill", "stroke", "all"]}})), &["layer", "kind"]),
            mutates: true,
            handler: remove_paint,
        },
    ]
}

/// Pointer of the item list a tool operates on: the layer's `shapes` or a group's `it`.
fn container(cx: &Cx, r: &LayerRef, shape: Option<&str>) -> OpResult<String> {
    match shape.filter(|s| !s.is_empty()) {
        None => {
            if cx.layer_value(r).get("ty").and_then(Value::as_i64) != Some(4) {
                return err(format!("layer {} is not a shape layer", r.id));
            }
            Ok(format!("{}/shapes", r.ptr()))
        }
        Some(s) => {
            let p = shape_ptr(&r.ptr(), s);
            match cx.get(&p).and_then(|v| v.get("ty")).and_then(Value::as_str) {
                Some("gr") => Ok(format!("{p}/it")),
                Some(_) => err(format!("shape '{s}' is not a group")),
                None => err(format!("shape '{s}' not found")),
            }
        }
    }
}

fn items(cx: &Cx, ptr: &str) -> Vec<Value> {
    cx.get(ptr).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn ty(v: &Value) -> &str {
    v.get("ty").and_then(Value::as_str).unwrap_or("")
}

/// Index just before the group transform (or the end).
fn end_index(list: &[Value]) -> usize {
    list.iter().rposition(|i| ty(i) == "tr").unwrap_or(list.len())
}

fn child_path(shape: Option<&str>, idx: usize) -> String {
    match shape.filter(|s| !s.is_empty()) {
        Some(s) => format!("{s}/{idx}"),
        None => idx.to_string(),
    }
}

fn add_shape(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let shape = opt_str(a, "shape");
    let cp = container(cx, &r, shape)?;
    let pos = opt_vec(a, "position").map(|p| [p[0], p.get(1).copied().unwrap_or(0.0)]).unwrap_or([0.0, 0.0]);
    let mut it = layers::shape_items(a, pos)?;
    let mut tr = default_ks([0.0, 0.0]);
    tr["ty"] = json!("tr");
    tr["p"] = static_prop(json!([0.0, 0.0]));
    tr["a"] = static_prop(json!([0.0, 0.0]));
    tr["s"] = static_prop(json!([100.0, 100.0]));
    it.push(tr);
    let n = items(cx, &cp).iter().filter(|i| ty(i) == "gr").count() + 1;
    let name = opt_str(a, "name").map(str::to_string).unwrap_or_else(|| format!("Shape {n}"));
    cx.insert(&cp, 0, json!({"ty": "gr", "nm": name, "np": it.len(), "it": it}))?;
    Ok(json!({"layer": r.id, "shape": child_path(shape, 0)}))
}

fn locate(cx: &Cx, r: &LayerRef, path: &str) -> OpResult<(String, usize)> {
    let full = shape_ptr(&r.ptr(), path);
    if cx.get(&full).is_none() {
        return err(format!("shape '{path}' not found in {}", r.id));
    }
    let (parent, idx) = full.rsplit_once('/').unwrap();
    Ok((parent.to_string(), idx.parse().map_err(|_| OpError("bad shape path".into()))?))
}

fn remove_shape_item(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let (parent, idx) = locate(cx, &r, arg_str(a, "shape")?)?;
    cx.remove(&parent, idx)?;
    Ok(json!({"ok": true}))
}

fn set_path(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let s = arg_str(a, "shape")?;
    let sp = shape_ptr(&r.ptr(), s);
    if cx.get(&sp).map(ty) != Some("sh") {
        return err(format!("shape '{s}' is not a path"));
    }
    util::write_value(cx, &format!("{sp}/ks"), a.get("path").unwrap_or(&Value::Null), util::frame_arg(a))?;
    Ok(json!({"ok": true}))
}

fn add_modifier(cx: &mut Cx, a: &Value, item: Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let shape = opt_str(a, "shape");
    let cp = container(cx, &r, shape)?;
    let at = end_index(&items(cx, &cp));
    let at = cx.insert(&cp, at, item)?;
    Ok(json!({"layer": r.id, "shape": child_path(shape, at)}))
}

fn add_trim_path(cx: &mut Cx, a: &Value) -> OpResult {
    let item = json!({"ty": "tm", "nm": "Trim Paths", "s": static_prop(json!(opt_f64(a, "start").unwrap_or(0.0))),
        "e": static_prop(json!(opt_f64(a, "end").unwrap_or(100.0))), "o": static_prop(json!(opt_f64(a, "offset").unwrap_or(0.0))),
        "m": if opt_bool(a, "individually").unwrap_or(false) { 2 } else { 1 }});
    add_modifier(cx, a, item)
}

fn add_round_corners(cx: &mut Cx, a: &Value) -> OpResult {
    let r = opt_f64(a, "radius").unwrap_or(10.0);
    if r < 0.0 {
        return err("radius must be >= 0");
    }
    add_modifier(cx, a, json!({"ty": "rd", "nm": "Round Corners", "r": static_prop(json!(r))}))
}

fn add_repeater(cx: &mut Cx, a: &Value) -> OpResult {
    let copies = opt_f64(a, "copies").unwrap_or(3.0);
    if copies < 1.0 {
        return err("copies must be >= 1");
    }
    let v2 = |k: &str, d: [f64; 2]| -> Value {
        let v = opt_vec(a, k).unwrap_or_else(|| d.to_vec());
        json!([v[0], v.get(1).copied().unwrap_or(v[0])])
    };
    let item = json!({"ty": "rp", "nm": "Repeater", "c": static_prop(json!(copies)), "o": static_prop(json!(opt_f64(a, "offset").unwrap_or(0.0))),
        "m": if opt_bool(a, "above").unwrap_or(true) { 1 } else { 2 },
        "tr": {"ty": "tr", "p": static_prop(v2("position", [0.0, 0.0])), "a": static_prop(v2("anchor", [0.0, 0.0])), "s": static_prop(v2("scale", [100.0, 100.0])),
               "r": static_prop(json!(opt_f64(a, "rotation").unwrap_or(0.0))),
               "so": static_prop(json!(opt_f64(a, "start_opacity").unwrap_or(100.0))), "eo": static_prop(json!(opt_f64(a, "end_opacity").unwrap_or(100.0)))}});
    add_modifier(cx, a, item)
}

fn update_shape_item(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let s = arg_str(a, "shape")?;
    let sp = shape_ptr(&r.ptr(), s);
    if cx.get(&sp).is_none() {
        return err(format!("shape '{s}' not found"));
    }
    let frame = util::frame_arg(a);
    if let Some(props) = a.get("props").and_then(Value::as_object) {
        for (k, v) in props {
            let ptr = util::prop_ptr(cx, &r, Some(s), k)?;
            util::write_value(cx, &ptr, v, frame)?;
        }
    }
    if let Some(n) = opt_str(a, "name") {
        cx.set(&format!("{sp}/nm"), json!(n))?;
    }
    if let Some(h) = opt_bool(a, "hidden") {
        cx.set(&format!("{sp}/hd"), json!(h))?;
    }
    if let Some(d) = a.get("direction").and_then(Value::as_i64) {
        cx.set(&format!("{sp}/d"), json!(d))?;
    }
    let pick = |k: &str, opts: &[(&str, i64)]| -> OpResult<Option<i64>> {
        match opt_str(a, k) {
            None => Ok(None),
            Some(v) => opts.iter().find(|o| o.0 == v).map(|o| Some(o.1)).ok_or_else(|| OpError(format!("invalid {k} '{v}'"))),
        }
    };
    if let Some(v) = pick("fill_rule", &[("non_zero", 1), ("even_odd", 2)])? {
        cx.set(&format!("{sp}/r"), json!(v))?;
    }
    if let Some(v) = pick("line_cap", &[("butt", 1), ("round", 2), ("square", 3)])? {
        cx.set(&format!("{sp}/lc"), json!(v))?;
    }
    if let Some(v) = pick("line_join", &[("miter", 1), ("round", 2), ("bevel", 3)])? {
        cx.set(&format!("{sp}/lj"), json!(v))?;
    }
    if let Some(v) = pick("trim_mode", &[("simultaneous", 1), ("individual", 2)])? {
        cx.set(&format!("{sp}/m"), json!(v))?;
    }
    if let Some(v) = opt_f64(a, "miter_limit") {
        cx.set(&format!("{sp}/ml"), json!(v))?;
    }
    Ok(json!({"ok": true}))
}

fn reorder_shape(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let (parent, idx) = locate(cx, &r, arg_str(a, "shape")?)?;
    let n = items(cx, &parent).len();
    let to = (a.get("index").and_then(Value::as_u64).unwrap_or(0) as usize).min(n - 1);
    let v = cx.remove(&parent, idx)?;
    cx.insert(&parent, to, v)?;
    Ok(json!({"ok": true, "index": to}))
}

// ------------------------------------------------------------------ paint

fn find_paint(list: &[Value], kinds: &[&str]) -> Option<usize> {
    list.iter().position(|i| kinds.contains(&ty(i)))
}

fn set_fill(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let cp = container(cx, &r, opt_str(a, "shape"))?;
    let list = items(cx, &cp);
    let frame = util::frame_arg(a);
    let idx = match find_paint(&list, &["fl"]) {
        Some(i) => i,
        None => {
            // Below every existing paint (later = drawn underneath), before the group transform.
            let at = list.iter().rposition(|i| matches!(ty(i), "fl" | "st" | "gf" | "gs")).map(|i| i + 1).unwrap_or(end_index(&list).min(
                list.iter().rposition(|i| matches!(ty(i), "rc" | "el" | "sr" | "sh" | "gr")).map(|i| i + 1).unwrap_or(0),
            ));
            let at = at.min(end_index(&list));
            cx.insert(&cp, at, fill_item(a.get("color").unwrap_or(&json!("#c4c4c4")))?)?
        }
    };
    let ip = format!("{cp}/{idx}");
    if let Some(c) = a.get("color") {
        util::write_value(cx, &format!("{ip}/c"), &json!(parse_color(c)?), frame)?;
    }
    if let Some(o) = a.get("opacity") {
        util::write_value(cx, &format!("{ip}/o"), o, frame)?;
    }
    Ok(json!({"layer": r.id, "shape": child_path(opt_str(a, "shape"), idx)}))
}

fn set_stroke(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let cp = container(cx, &r, opt_str(a, "shape"))?;
    let list = items(cx, &cp);
    let frame = util::frame_arg(a);
    let idx = match find_paint(&list, &["st"]) {
        Some(i) => i,
        None => {
            // Above existing paints so the stroke is not covered by the fill.
            let at = find_paint(&list, &["fl", "gf", "gs"]).unwrap_or(end_index(&list));
            cx.insert(&cp, at, stroke_item(a.get("color").unwrap_or(&json!("#000000")), opt_f64(a, "width").unwrap_or(2.0))?)?
        }
    };
    let ip = format!("{cp}/{idx}");
    if let Some(c) = a.get("color") {
        util::write_value(cx, &format!("{ip}/c"), &json!(parse_color(c)?), frame)?;
    }
    for (k, key) in [("width", "w"), ("opacity", "o")] {
        if let Some(v) = a.get(k) {
            util::write_value(cx, &format!("{ip}/{key}"), v, frame)?;
        }
    }
    let codes = |k: &str, opts: &[(&str, i64)]| opt_str(a, k).and_then(|v| opts.iter().find(|o| o.0 == v)).map(|o| o.1);
    if let Some(v) = codes("line_cap", &[("butt", 1), ("round", 2), ("square", 3)]) {
        cx.set(&format!("{ip}/lc"), json!(v))?;
    }
    if let Some(v) = codes("line_join", &[("miter", 1), ("round", 2), ("bevel", 3)]) {
        cx.set(&format!("{ip}/lj"), json!(v))?;
    }
    match a.get("dash") {
        Some(Value::Null) => cx.delete(&format!("{ip}/d"))?,
        Some(Value::Array(d)) if d.len() >= 2 => {
            let g = |i: usize| d[i].as_f64().unwrap_or(0.0);
            cx.set(
                &format!("{ip}/d"),
                json!([{"n": "d", "nm": "dash", "v": static_prop(json!(g(0)))}, {"n": "g", "nm": "gap", "v": static_prop(json!(g(1)))},
                       {"n": "o", "nm": "offset", "v": static_prop(json!(d.get(2).and_then(Value::as_f64).unwrap_or(0.0)))}]),
            )?;
        }
        Some(_) => return err("dash must be [dash, gap(, offset)] or null"),
        None => {}
    }
    Ok(json!({"layer": r.id, "shape": child_path(opt_str(a, "shape"), idx)}))
}

fn set_gradient(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let shape = opt_str(a, "shape");
    let cp = container(cx, &r, shape)?;
    let stroke = opt_str(a, "kind") == Some("stroke");
    let radial = opt_str(a, "type") == Some("radial");
    let stops = a.get("stops").and_then(Value::as_array).filter(|s| !s.is_empty()).ok_or_else(|| OpError("stops must be a non-empty array".into()))?;
    let mut colors = vec![];
    let mut alphas = vec![];
    let mut has_alpha = false;
    for s in stops {
        let off = s.get("offset").and_then(Value::as_f64).unwrap_or(0.0).clamp(0.0, 1.0);
        let c = parse_color(s.get("color").unwrap_or(&Value::Null))?;
        let op = s.get("opacity").and_then(Value::as_f64).unwrap_or(c[3]);
        has_alpha |= op < 1.0;
        colors.extend([off, c[0], c[1], c[2]]);
        alphas.extend([off, op]);
    }
    if has_alpha {
        colors.extend(alphas);
    }
    let local = layers::geometry(cx, &r.id, 0.0).map(|g| g.local).unwrap_or([-50.0, -50.0, 50.0, 50.0]);
    let (mx, my) = ((local[0] + local[2]) / 2.0, (local[1] + local[3]) / 2.0);
    let (ds, de) = if radial { ([mx, my], [local[2], my]) } else { ([local[0], my], [local[2], my]) };
    let pt = |k: &str, d: [f64; 2]| -> Value {
        let v = opt_vec(a, k).unwrap_or_else(|| d.to_vec());
        json!([v[0], v.get(1).copied().unwrap_or(0.0)])
    };
    let list = items(cx, &cp);
    let kinds: &[&str] = if stroke { &["st", "gs"] } else { &["fl", "gf"] };
    let existing = find_paint(&list, kinds);
    let mut item = json!({"ty": if stroke { "gs" } else { "gf" }, "nm": if stroke { "Gradient Stroke" } else { "Gradient Fill" },
        "t": if radial { 2 } else { 1 }, "o": static_prop(json!(100)), "r": 1, "bm": 0,
        "s": static_prop(pt("start", ds)), "e": static_prop(pt("end", de)),
        "g": {"p": stops.len(), "k": static_prop(json!(colors))}});
    if radial {
        item["h"] = static_prop(json!(opt_f64(a, "highlight").unwrap_or(0.0)));
        item["a"] = static_prop(json!(opt_f64(a, "angle").unwrap_or(0.0)));
    }
    if stroke {
        let prev = existing.map(|i| list[i].clone()).unwrap_or(Value::Null);
        item["w"] = prev.get("w").cloned().unwrap_or_else(|| static_prop(json!(opt_f64(a, "width").unwrap_or(2.0))));
        for k in ["lc", "lj", "ml", "d"] {
            item[k] = prev.get(k).cloned().unwrap_or(match k {
                "lc" | "lj" => json!(2),
                "ml" => json!(4),
                _ => json!([]),
            });
        }
        if let Some(w) = opt_f64(a, "width") {
            item["w"] = static_prop(json!(w));
        }
    }
    let idx = match existing {
        Some(i) => {
            if let Some(o) = list[i].get("o") {
                item["o"] = o.clone();
            }
            cx.set(&format!("{cp}/{i}"), item)?;
            i
        }
        None => cx.insert(&cp, end_index(&list), item)?,
    };
    Ok(json!({"layer": r.id, "shape": child_path(shape, idx)}))
}

fn remove_paint(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let cp = container(cx, &r, opt_str(a, "shape"))?;
    let kinds: &[&str] = match arg_str(a, "kind")? {
        "fill" => &["fl", "gf"],
        "stroke" => &["st", "gs"],
        "all" => &["fl", "gf", "st", "gs"],
        other => return err(format!("unknown kind '{other}'")),
    };
    let list = items(cx, &cp);
    let mut n = 0;
    for i in (0..list.len()).rev() {
        if kinds.contains(&ty(&list[i])) {
            cx.remove(&cp, i)?;
            n += 1;
        }
    }
    Ok(json!({"removed": n}))
}
