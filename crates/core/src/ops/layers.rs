//! Layer tools: creation, structure, timing, compositing and transforms.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, default_ks, parse_color, static_prop};
use super::{arg_str, err, opt_bool, opt_f64, opt_str, opt_vec, Cx, LayerRef, OpError, OpResult};
use crate::geom::Affine;
use crate::hit;
use crate::model;
use crate::scene as sc;
use crate::text::FontRegistry;

const BLEND_MODES: &[&str] = &[
    "normal", "multiply", "screen", "overlay", "darken", "lighten", "color_dodge", "color_burn", "hard_light", "soft_light", "difference",
    "exclusion", "hue", "saturation", "color", "luminosity", "add",
];

fn layer_arg() -> Value {
    json!({"type": "string", "description": "Layer id from read_scene (e.g. 'root:3') or a unique layer name"})
}

fn color_arg(desc: &str) -> Value {
    json!({"description": desc, "oneOf": [{"type": "string"}, {"type": "array", "items": {"type": "number"}}, {"type": "null"}]})
}

fn vec2(desc: &str) -> Value {
    json!({"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 3, "description": desc})
}

pub fn shape_geometry_props() -> Value {
    json!({
        "type": {"type": "string", "enum": ["rect", "ellipse", "polygon", "star", "path"]},
        "name": {"type": "string"},
        "position": vec2("Centre in scene coordinates (defaults to the scene centre)"),
        "size": vec2("Width and height for rect/ellipse (default 100×100)"),
        "roundness": {"type": "number", "description": "Corner radius (rect) or point roundness % (star/polygon)"},
        "radius": {"type": "number", "description": "Outer radius for star/polygon (default 50)"},
        "inner_radius": {"type": "number", "description": "Inner radius for star (default radius/2)"},
        "points": {"description": "Star/polygon point count, or path vertices [[x,y],...] relative to position"},
        "path": {"type": "object", "description": "Bezier {v, i, o, c} relative to position"},
        "closed": {"type": "boolean"},
        "fill": color_arg("Fill colour; null for no fill (default #c4c4c4)"),
        "stroke": color_arg("Stroke colour (default none)"),
        "stroke_width": {"type": "number"}
    })
}

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "create_shape",
            description: "Create a new shape layer (rect, ellipse, polygon, star or freeform path) on top of the scene, with optional fill and stroke. Returns the new layer id.",
            schema: || {
                let mut p = shape_geometry_props();
                p["scene"] = json!({"type": "string", "description": "Scene/precomp id to add to (default 'root')"});
                obj(p, &["type"])
            },
            mutates: true,
            handler: create_shape,
        },
        ToolDef {
            name: "create_solid",
            description: "Create a solid colour layer covering the scene (or a given size).",
            schema: || obj(json!({"color": color_arg("Solid colour"), "width": {"type": "number"}, "height": {"type": "number"}, "name": {"type": "string"}, "scene": {"type": "string"}}), &["color"]),
            mutates: true,
            handler: create_solid,
        },
        ToolDef {
            name: "create_null",
            description: "Create an invisible null layer, useful as a parent/controller for other layers.",
            schema: || obj(json!({"name": {"type": "string"}, "position": vec2("Position"), "scene": {"type": "string"}}), &[]),
            mutates: true,
            handler: create_null,
        },
        ToolDef {
            name: "duplicate_layer",
            description: "Duplicate a layer (with its animation) directly above the original. Returns the new id.",
            schema: || obj(json!({"layer": layer_arg(), "name": {"type": "string"}}), &["layer"]),
            mutates: true,
            handler: duplicate_layer,
        },
        ToolDef {
            name: "delete_layer",
            description: "Delete a layer. Children parented to it are unparented.",
            schema: || obj(json!({"layer": layer_arg()}), &["layer"]),
            mutates: true,
            handler: delete_layer,
        },
        ToolDef {
            name: "rename_layer",
            description: "Rename a layer. State machine interactions match layers by name, so keep names unique.",
            schema: || obj(json!({"layer": layer_arg(), "name": {"type": "string"}}), &["layer", "name"]),
            mutates: true,
            handler: rename_layer,
        },
        ToolDef {
            name: "reorder_layer",
            description: "Move a layer in the stacking order: give an index (0 = top) or 'to' front/back/up/down.",
            schema: || obj(json!({"layer": layer_arg(), "index": {"type": "integer", "minimum": 0}, "to": {"type": "string", "enum": ["front", "back", "up", "down"]}}), &["layer"]),
            mutates: true,
            handler: reorder_layer,
        },
        ToolDef {
            name: "set_parent",
            description: "Parent a layer to another layer in the same scene (child follows the parent's transform), or unparent with parent=null.",
            schema: || obj(json!({"layer": layer_arg(), "parent": {"type": ["string", "null"]}}), &["layer", "parent"]),
            mutates: true,
            handler: set_parent,
        },
        ToolDef {
            name: "set_layer_visibility",
            description: "Show or hide a layer.",
            schema: || obj(json!({"layer": layer_arg(), "visible": {"type": "boolean"}}), &["layer", "visible"]),
            mutates: true,
            handler: set_layer_visibility,
        },
        ToolDef {
            name: "set_layer_lock",
            description: "Lock or unlock a layer for canvas editing (editor-only flag, not exported).",
            schema: || obj(json!({"layer": layer_arg(), "locked": {"type": "boolean"}}), &["layer", "locked"]),
            mutates: true,
            handler: set_layer_lock,
        },
        ToolDef {
            name: "set_layer_time",
            description: "Set a layer's in/out points, start time offset (frames) and time stretch.",
            schema: || obj(json!({"layer": layer_arg(), "in": {"type": "number"}, "out": {"type": "number"}, "start": {"type": "number"}, "stretch": {"type": "number", "exclusiveMinimum": 0}}), &["layer"]),
            mutates: true,
            handler: set_layer_time,
        },
        ToolDef {
            name: "set_blend_mode",
            description: "Set a layer's blend mode.",
            schema: || obj(json!({"layer": layer_arg(), "mode": {"type": "string", "enum": BLEND_MODES}}), &["layer", "mode"]),
            mutates: true,
            handler: set_blend_mode,
        },
        ToolDef {
            name: "set_matte",
            description: "Use one layer as a track matte for another (alpha, alpha_inverted, luma, luma_inverted). The matte layer is moved directly above the target and hidden from direct rendering.",
            schema: || obj(json!({"layer": layer_arg(), "matte_layer": layer_arg(), "mode": {"type": "string", "enum": ["alpha", "alpha_inverted", "luma", "luma_inverted"]}}), &["layer", "matte_layer"]),
            mutates: true,
            handler: set_matte,
        },
        ToolDef {
            name: "remove_matte",
            description: "Remove a layer's track matte.",
            schema: || obj(json!({"layer": layer_arg()}), &["layer"]),
            mutates: true,
            handler: remove_matte,
        },
        ToolDef {
            name: "precompose_layers",
            description: "Move layers into a new nested scene (precomp) and replace them with one precomp layer. Returns the new scene id and layer id.",
            schema: || obj(json!({"layers": {"type": "array", "items": {"type": "string"}, "minItems": 1}, "name": {"type": "string"}}), &["layers"]),
            mutates: true,
            handler: precompose_layers,
        },
        ToolDef {
            name: "align_layers",
            description: "Align layers' bounding boxes to the scene or to the selection's combined bounds.",
            schema: || obj(json!({"layers": {"type": "array", "items": {"type": "string"}, "minItems": 1}, "align": {"type": "string", "enum": ["left", "center", "right", "top", "middle", "bottom"]}, "to": {"type": "string", "enum": ["scene", "selection"]}, "frame": {"type": "number"}}), &["layers", "align"]),
            mutates: true,
            handler: align_layers,
        },
        ToolDef {
            name: "distribute_layers",
            description: "Evenly space the centres of 3+ layers horizontally or vertically, keeping the outermost ones in place.",
            schema: || obj(json!({"layers": {"type": "array", "items": {"type": "string"}, "minItems": 3}, "axis": {"type": "string", "enum": ["horizontal", "vertical"]}, "frame": {"type": "number"}}), &["layers", "axis"]),
            mutates: true,
            handler: distribute_layers,
        },
        ToolDef {
            name: "paste_layers",
            description: "Insert copies of layer JSON objects (e.g. from inspect_layer) on top of a scene, assigning fresh ids and optionally offsetting them. Parent/matte links inside the pasted set are preserved.",
            schema: || obj(json!({"layers": {"type": "array", "items": {"type": "object"}, "minItems": 1}, "scene": {"type": "string"}, "offset": {"type": "array"}}), &["layers"]),
            mutates: true,
            handler: paste_layers,
        },
        ToolDef {
            name: "nudge_layers",
            description: "Move layers by a scene-space delta (dx, dy) like dragging on the canvas: static positions shift; animated positions get a keyframe at 'frame'. Parented layers are handled.",
            schema: || obj(json!({"layers": {"type": "array", "items": {"type": "string"}}, "dx": {"type": "number"}, "dy": {"type": "number"}, "frame": {"type": "number"}}), &["layers"]),
            mutates: true,
            handler: nudge_layers,
        },
        ToolDef {
            name: "set_transform",
            description: "Set transform properties of a layer (or a shape group via 'shape'). Static properties are replaced; animated ones get a keyframe at 'frame'. Rotation in degrees (clockwise), scale and opacity in percent.",
            schema: || {
                obj(
                    json!({"layer": layer_arg(), "shape": {"type": "string", "description": "Shape group path, e.g. '0'"},
                        "position": vec2("x, y"), "scale": vec2("x%, y%"), "rotation": {"type": "number"}, "opacity": {"type": "number", "minimum": 0, "maximum": 100},
                        "anchor": vec2("Anchor point in layer space"), "skew": {"type": "number"}, "skew_axis": {"type": "number"}, "frame": {"type": "number"}}),
                    &["layer"],
                )
            },
            mutates: true,
            handler: set_transform,
        },
        ToolDef {
            name: "set_anchor_point",
            description: "Move a layer's anchor point to a named position of its bounds (center, top_left, …) or explicit layer-space [x,y], compensating position so the layer does not move visually.",
            schema: || obj(json!({"layer": layer_arg(), "anchor": {"description": "center|top_left|top|top_right|left|right|bottom_left|bottom|bottom_right or [x,y]"}, "frame": {"type": "number"}}), &["layer", "anchor"]),
            mutates: true,
            handler: set_anchor_point,
        },
        ToolDef {
            name: "flip_layer",
            description: "Mirror a layer horizontally or vertically by negating its scale.",
            schema: || obj(json!({"layer": layer_arg(), "axis": {"type": "string", "enum": ["horizontal", "vertical"]}}), &["layer", "axis"]),
            mutates: true,
            handler: flip_layer,
        },
    ]
}

// ------------------------------------------------------------------ builders

fn color_value(v: &Value) -> OpResult<Value> {
    let c = parse_color(v)?;
    Ok(json!([c[0], c[1], c[2], 1.0]))
}

pub fn fill_item(c: &Value) -> OpResult<Value> {
    Ok(json!({"ty": "fl", "nm": "Fill", "c": static_prop(color_value(c)?), "o": static_prop(json!(100)), "r": 1, "bm": 0}))
}

pub fn stroke_item(c: &Value, w: f64) -> OpResult<Value> {
    Ok(json!({"ty": "st", "nm": "Stroke", "c": static_prop(color_value(c)?), "o": static_prop(json!(100)), "w": static_prop(json!(w)), "lc": 2, "lj": 2, "ml": 4, "bm": 0}))
}

/// Geometry + paint items for a new shape, positioned relative to `origin`.
pub fn shape_items(a: &Value, origin: [f64; 2]) -> OpResult<Vec<Value>> {
    let kind = arg_str(a, "type")?;
    let size = opt_vec(a, "size").unwrap_or_else(|| vec![100.0, 100.0]);
    let (w, h) = (size.first().copied().unwrap_or(100.0), size.get(1).copied().unwrap_or(size[0]));
    let o = json!([origin[0], origin[1]]);
    let geom = match kind {
        "rect" => json!({"ty": "rc", "nm": "Rectangle Path", "d": 1, "p": static_prop(o), "s": static_prop(json!([w, h])), "r": static_prop(json!(opt_f64(a, "roundness").unwrap_or(0.0)))}),
        "ellipse" => json!({"ty": "el", "nm": "Ellipse Path", "d": 1, "p": static_prop(o), "s": static_prop(json!([w, h]))}),
        "star" | "polygon" => {
            let r = opt_f64(a, "radius").unwrap_or(50.0);
            let pts = a.get("points").and_then(Value::as_f64).unwrap_or(5.0);
            let round = opt_f64(a, "roundness").unwrap_or(0.0);
            let mut s = json!({"ty": "sr", "nm": if kind == "star" { "Star Path" } else { "Polygon Path" }, "d": 1, "sy": if kind == "star" { 1 } else { 2 },
                "p": static_prop(o), "pt": static_prop(json!(pts)), "or": static_prop(json!(r)), "os": static_prop(json!(round)), "r": static_prop(json!(0))});
            if kind == "star" {
                s["ir"] = static_prop(json!(opt_f64(a, "inner_radius").unwrap_or(r / 2.0)));
                s["is"] = static_prop(json!(0));
            }
            s
        }
        "path" => {
            let bez = if let Some(p) = a.get("path") {
                util::bezier_value(p)?
            } else {
                let pts = a.get("points").and_then(Value::as_array).ok_or_else(|| OpError("path needs 'points' [[x,y],...] or 'path'".into()))?;
                let v: Vec<Value> = pts.iter().map(|p| json!([p[0].as_f64().unwrap_or(0.0) + origin[0], p[1].as_f64().unwrap_or(0.0) + origin[1]])).collect();
                let n = v.len();
                json!({"c": opt_bool(a, "closed").unwrap_or(true), "v": v, "i": vec![json!([0, 0]); n], "o": vec![json!([0, 0]); n]})
            };
            json!({"ty": "sh", "nm": "Path", "d": 1, "ks": static_prop(bez)})
        }
        other => return err(format!("unknown shape type '{other}' (rect, ellipse, polygon, star, path)")),
    };
    // Earlier items draw on top: geometry, stroke, then fill (stroke above fill).
    let mut items = vec![geom];
    if let Some(c) = a.get("stroke").filter(|c| !c.is_null()) {
        items.push(stroke_item(c, opt_f64(a, "stroke_width").unwrap_or(2.0))?);
    }
    match a.get("fill") {
        Some(Value::Null) => {}
        Some(c) => items.push(fill_item(c)?),
        None if a.get("stroke").is_some() => {}
        None => items.push(fill_item(&json!("#c4c4c4"))?),
    }
    Ok(items)
}

fn new_layer(cx: &mut Cx, scene: &str, ty: i64, name: &str, pos: [f64; 2], extra: Value) -> OpResult<Value> {
    let lp = cx.comp_layers_ptr(scene)?;
    let ind = cx.next_ind(&lp);
    let (_, _, _, ip, op) = util::scene(cx);
    let mut l = json!({"ddd": 0, "ind": ind, "ty": ty, "nm": name, "sr": 1, "ks": default_ks(pos), "ao": 0, "ip": ip, "op": op, "st": 0, "bm": 0});
    if let Value::Object(e) = extra {
        for (k, v) in e {
            l[k] = v;
        }
    }
    cx.insert(&lp, 0, l)?;
    Ok(json!({"id": format!("{scene}:{ind}")}))
}

fn default_name(cx: &Cx, scene: &str, base: &str) -> String {
    let n = cx.comp_layers_ptr(scene).ok().and_then(|p| cx.get(&p)).and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    format!("{base} {}", n + 1)
}

fn scene_arg<'v>(a: &'v Value) -> &'v str {
    opt_str(a, "scene").unwrap_or("root")
}

fn create_shape(cx: &mut Cx, a: &Value) -> OpResult {
    let (w, h, ..) = util::scene(cx);
    let pos = opt_vec(a, "position").map(|p| [p[0], p.get(1).copied().unwrap_or(0.0)]).unwrap_or([w / 2.0, h / 2.0]);
    let items = shape_items(a, [0.0, 0.0])?;
    let scene = scene_arg(a).to_string();
    let label = match arg_str(a, "type")? {
        "rect" => "Rectangle",
        "ellipse" => "Ellipse",
        "polygon" => "Polygon",
        "star" => "Star",
        _ => "Path",
    };
    let name = opt_str(a, "name").map(str::to_string).unwrap_or_else(|| default_name(cx, &scene, label));
    new_layer(cx, &scene, 4, &name, pos, json!({"shapes": items}))
}

fn create_solid(cx: &mut Cx, a: &Value) -> OpResult {
    let (w, h, ..) = util::scene(cx);
    let c = parse_color(a.get("color").unwrap_or(&Value::Null))?;
    let (sw, sh) = (opt_f64(a, "width").unwrap_or(w), opt_f64(a, "height").unwrap_or(h));
    let scene = scene_arg(a).to_string();
    let name = opt_str(a, "name").map(str::to_string).unwrap_or_else(|| default_name(cx, &scene, "Solid"));
    let r = new_layer(cx, &scene, 1, &name, [0.0, 0.0], json!({"sc": util::hex(&c), "sw": sw, "sh": sh}))?;
    Ok(r)
}

fn create_null(cx: &mut Cx, a: &Value) -> OpResult {
    let (w, h, ..) = util::scene(cx);
    let pos = opt_vec(a, "position").map(|p| [p[0], p.get(1).copied().unwrap_or(0.0)]).unwrap_or([w / 2.0, h / 2.0]);
    let scene = scene_arg(a).to_string();
    let name = opt_str(a, "name").map(str::to_string).unwrap_or_else(|| default_name(cx, &scene, "Null"));
    new_layer(cx, &scene, 3, &name, pos, json!({}))
}

fn duplicate_layer(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let mut l = cx.layer_value(&r);
    let ind = cx.next_ind(&r.layers_ptr);
    l["ind"] = json!(ind);
    l["nm"] = json!(opt_str(a, "name").map(str::to_string).unwrap_or_else(|| format!("{} copy", l["nm"].as_str().unwrap_or("Layer"))));
    cx.insert(&r.layers_ptr, r.index, l)?;
    Ok(json!({"id": format!("{}:{ind}", Cx::comp_of(&r))}))
}

fn layers_of(cx: &Cx, r: &LayerRef) -> Vec<Value> {
    cx.get(&r.layers_ptr).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn ind_of(l: &Value) -> Option<i64> {
    l.get("ind").and_then(Value::as_i64)
}

fn delete_layer(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let removed = cx.remove(&r.layers_ptr, r.index)?;
    if let Some(ind) = ind_of(&removed) {
        for (i, l) in layers_of(cx, &r).iter().enumerate() {
            if l.get("parent").and_then(Value::as_i64) == Some(ind) {
                cx.delete(&format!("{}/{i}/parent", r.layers_ptr))?;
            }
            if l.get("tp").and_then(Value::as_i64) == Some(ind) {
                cx.delete(&format!("{}/{i}/tp", r.layers_ptr))?;
                cx.delete(&format!("{}/{i}/tt", r.layers_ptr))?;
            }
        }
    }
    Ok(json!({"deleted": r.id}))
}

fn rename_layer(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    cx.set(&format!("{}/nm", r.ptr()), json!(arg_str(a, "name")?))?;
    Ok(json!({"id": r.id}))
}

fn reorder_layer(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let n = layers_of(cx, &r).len();
    let target = if let Some(i) = a.get("index").and_then(Value::as_u64) {
        (i as usize).min(n - 1)
    } else {
        match opt_str(a, "to") {
            Some("front") => 0,
            Some("back") => n - 1,
            Some("up") => r.index.saturating_sub(1),
            Some("down") => (r.index + 1).min(n - 1),
            _ => return err("give 'index' or 'to' (front, back, up, down)"),
        }
    };
    if target != r.index {
        let l = cx.remove(&r.layers_ptr, r.index)?;
        cx.insert(&r.layers_ptr, target, l)?;
    }
    Ok(json!({"id": r.id, "index": target}))
}

fn set_parent(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    match a.get("parent") {
        None | Some(Value::Null) => {
            cx.delete(&format!("{}/parent", r.ptr()))?;
        }
        Some(Value::String(p)) => {
            let pr = cx.layer(p)?;
            if pr.layers_ptr != r.layers_ptr {
                return err("parent must be in the same scene");
            }
            if pr.index == r.index {
                return err("a layer cannot parent itself");
            }
            let layers = layers_of(cx, &r);
            let my = ind_of(&layers[r.index]);
            let mut cur = Some(&layers[pr.index]);
            for _ in 0..64 {
                let Some(c) = cur else { break };
                if ind_of(c) == my {
                    return err("parenting would create a cycle");
                }
                cur = c.get("parent").and_then(Value::as_i64).and_then(|pi| layers.iter().find(|l| ind_of(l) == Some(pi)));
            }
            let pind = ind_of(&layers[pr.index]).ok_or_else(|| OpError("parent layer has no index".into()))?;
            cx.set(&format!("{}/parent", r.ptr()), json!(pind))?;
        }
        _ => return err("parent must be a layer id/name or null"),
    }
    Ok(json!({"id": r.id}))
}

fn set_layer_visibility(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let visible = opt_bool(a, "visible").ok_or_else(|| OpError("missing 'visible'".into()))?;
    if visible {
        cx.delete(&format!("{}/hd", r.ptr()))?;
    } else {
        cx.set(&format!("{}/hd", r.ptr()), json!(true))?;
    }
    Ok(json!({"id": r.id, "visible": visible}))
}

fn set_layer_lock(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let locked = opt_bool(a, "locked").ok_or_else(|| OpError("missing 'locked'".into()))?;
    cx.set(&format!("{}/lmeta/lock", r.ptr()), json!(locked))?;
    Ok(json!({"id": r.id, "locked": locked}))
}

fn set_layer_time(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let p = r.ptr();
    let l = cx.layer_value(&r);
    let ip = opt_f64(a, "in").unwrap_or(l["ip"].as_f64().unwrap_or(0.0));
    let op = opt_f64(a, "out").unwrap_or(l["op"].as_f64().unwrap_or(0.0));
    if op <= ip {
        return err("out must be greater than in");
    }
    for (k, key) in [("in", "ip"), ("out", "op"), ("start", "st"), ("stretch", "sr")] {
        if let Some(v) = opt_f64(a, k) {
            if key == "sr" && v <= 0.0 {
                return err("stretch must be > 0");
            }
            cx.set(&format!("{p}/{key}"), json!(v))?;
        }
    }
    Ok(json!({"id": r.id}))
}

fn set_blend_mode(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let m = arg_str(a, "mode")?;
    let bm = BLEND_MODES.iter().position(|x| *x == m).ok_or_else(|| OpError(format!("unknown blend mode '{m}'")))?;
    cx.set(&format!("{}/bm", r.ptr()), json!(bm))?;
    Ok(json!({"id": r.id, "bm": bm}))
}

fn set_matte(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let m = cx.layer(arg_str(a, "matte_layer")?)?;
    if m.layers_ptr != r.layers_ptr || m.index == r.index {
        return err("matte layer must be a different layer in the same scene");
    }
    let tt = match opt_str(a, "mode").unwrap_or("alpha") {
        "alpha" => 1,
        "alpha_inverted" => 2,
        "luma" => 3,
        "luma_inverted" => 4,
        other => return err(format!("unknown matte mode '{other}'")),
    };
    let target_ind = ind_of(&cx.layer_value(&r));
    // Place the matte directly above the target (legacy players use adjacency).
    let ml = cx.remove(&m.layers_ptr, m.index)?;
    let mind = ind_of(&ml).ok_or_else(|| OpError("matte layer has no index".into()))?;
    let ti = layers_of(cx, &r).iter().position(|l| ind_of(l) == target_ind).unwrap_or(0);
    cx.insert(&r.layers_ptr, ti, ml)?;
    let lp = r.layers_ptr.clone();
    cx.set(&format!("{lp}/{ti}/td"), json!(1))?;
    cx.set(&format!("{lp}/{}/tt", ti + 1), json!(tt))?;
    cx.set(&format!("{lp}/{}/tp", ti + 1), json!(mind))?;
    Ok(json!({"id": r.id, "matte": m.id}))
}

fn remove_matte(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let l = cx.layer_value(&r);
    let src = match l.get("tp").and_then(Value::as_i64) {
        Some(tp) => layers_of(cx, &r).iter().position(|x| ind_of(x) == Some(tp)),
        None => r.index.checked_sub(1),
    };
    cx.delete(&format!("{}/tt", r.ptr()))?;
    cx.delete(&format!("{}/tp", r.ptr()))?;
    if let Some(si) = src {
        let sind = ind_of(&layers_of(cx, &r)[si]);
        let still_used = layers_of(cx, &r).iter().any(|x| x.get("tp").and_then(Value::as_i64) == sind && sind.is_some());
        if !still_used {
            cx.delete(&format!("{}/{si}/td", r.layers_ptr))?;
        }
    }
    Ok(json!({"id": r.id}))
}

fn precompose_layers(cx: &mut Cx, a: &Value) -> OpResult {
    let keys = a.get("layers").and_then(Value::as_array).ok_or_else(|| OpError("missing 'layers'".into()))?;
    let mut refs: Vec<LayerRef> = keys.iter().map(|k| cx.layer(k.as_str().unwrap_or(""))).collect::<OpResult<_>>()?;
    if refs.iter().any(|r| r.layers_ptr != refs[0].layers_ptr) {
        return err("all layers must be in the same scene");
    }
    refs.sort_by_key(|r| r.index);
    refs.dedup_by_key(|r| r.index);
    let lp = refs[0].layers_ptr.clone();
    let top = refs[0].index;
    let mut moved = vec![];
    for r in refs.iter().rev() {
        moved.push(cx.remove(&lp, r.index)?);
    }
    moved.reverse();
    let (w, h, fr, ip, op) = util::scene(cx);
    let anim_ptr = cx.anim_ptr();
    let existing: Vec<String> = cx.anim().get("assets").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.get("id").and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
    let id = (1..).map(|n| format!("comp_{n}")).find(|c| !existing.contains(c)).unwrap();
    let name = opt_str(a, "name").unwrap_or("Nested Scene").to_string();
    let n_assets = existing.len();
    cx.insert(&format!("{anim_ptr}/assets"), n_assets, json!({"id": id, "nm": name, "fr": fr, "layers": moved}))?;
    let ind = cx.next_ind(&lp);
    let mut ks = default_ks([w / 2.0, h / 2.0]);
    ks["a"] = static_prop(json!([w / 2.0, h / 2.0, 0.0]));
    cx.insert(&lp, top, json!({"ddd": 0, "ind": ind, "ty": 0, "nm": name, "refId": id, "sr": 1, "ks": ks, "ao": 0, "w": w, "h": h, "ip": ip, "op": op, "st": 0, "bm": 0}))?;
    let comp = cx.comps().into_iter().find(|(_, p)| *p == lp).map(|c| c.0).unwrap_or_else(|| "root".into());
    Ok(json!({"scene": id, "id": format!("{comp}:{ind}")}))
}

// ------------------------------------------------------------------ geometry-based

fn parsed(cx: &Cx) -> model::Animation {
    model::parse(cx.anim()).unwrap_or_default()
}

pub fn geometry(cx: &Cx, id: &str, frame: f64) -> OpResult<hit::Geometry> {
    hit::layer_geometry(&parsed(cx), &FontRegistry::default(), id, frame).ok_or_else(|| OpError(format!("layer '{id}' not found")))
}

/// Convert a composition-space delta into the layer's parent space.
fn parent_delta(cx: &Cx, r: &LayerRef, dx: f64, dy: f64, frame: f64) -> (f64, f64) {
    let anim = parsed(cx);
    let (comp, _) = r.id.rsplit_once(':').unwrap_or(("root", ""));
    let layers: &[model::Layer] = if comp == "root" { &anim.layers } else { anim.asset(comp).and_then(|a| a.layers.as_deref()).unwrap_or(&[]) };
    if let Some(pid) = layers.get(r.index).and_then(|l| l.parent) {
        if let Some(pi) = layers.iter().position(|l| l.ind == Some(pid)) {
            if let Some(inv) = sc::layer_matrix(layers, pi, frame).invert() {
                let d = Affine { e: 0.0, f: 0.0, ..inv }.apply([dx, dy]);
                return (d[0], d[1]);
            }
        }
    }
    (dx, dy)
}

fn is_split(cx: &Cx, p_ptr: &str) -> bool {
    cx.get(p_ptr).and_then(|p| p.get("s")).map(|s| s.as_bool().unwrap_or(false) || s.as_i64() == Some(1)).unwrap_or(false)
}

/// Move a layer by a composition-space delta (all position keyframes shift).
pub fn translate(cx: &mut Cx, r: &LayerRef, dx: f64, dy: f64, frame: f64) -> OpResult<()> {
    let (ldx, ldy) = parent_delta(cx, r, dx, dy, frame);
    let base = format!("{}/ks/p", r.ptr());
    if cx.get(&base).is_none() {
        cx.set(&base, static_prop(json!([0, 0, 0])))?;
    }
    if is_split(cx, &base) {
        shift_prop(cx, &format!("{base}/x"), &[ldx])?;
        shift_prop(cx, &format!("{base}/y"), &[ldy])
    } else {
        shift_prop(cx, &base, &[ldx, ldy])
    }
}

/// Move a layer like a canvas drag: static positions shift, animated ones get a keyframe at `frame`.
pub fn nudge(cx: &mut Cx, r: &LayerRef, dx: f64, dy: f64, frame: f64) -> OpResult<()> {
    let (ldx, ldy) = parent_delta(cx, r, dx, dy, frame);
    let base = format!("{}/ks/p", r.ptr());
    let targets: Vec<(String, Vec<f64>)> = if is_split(cx, &base) { vec![(format!("{base}/x"), vec![ldx]), (format!("{base}/y"), vec![ldy])] } else { vec![(base.clone(), vec![ldx, ldy])] };
    for (ptr, d) in targets {
        if cx.get(&ptr).map(util::is_animated).unwrap_or(false) {
            let mut v = util::read_value(cx, &ptr, frame);
            for (i, x) in d.iter().enumerate() {
                if let Some(c) = v.get_mut(i) {
                    *c += x;
                }
            }
            util::upsert_keyframe(cx, &ptr, frame, &json!(v), None)?;
        } else {
            if cx.get(&ptr).is_none() {
                cx.set(&ptr, static_prop(json!([0, 0, 0])))?;
            }
            shift_prop(cx, &ptr, &d)?;
        }
    }
    Ok(())
}

fn nudge_layers(cx: &mut Cx, a: &Value) -> OpResult {
    let frame = util::frame_arg(a).unwrap_or(0.0).round();
    let (dx, dy) = (opt_f64(a, "dx").unwrap_or(0.0), opt_f64(a, "dy").unwrap_or(0.0));
    let keys: Vec<String> = a.get("layers").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    for k in &keys {
        let r = cx.layer(k)?;
        nudge(cx, &r, dx, dy, frame)?;
    }
    Ok(json!({"moved": keys.len()}))
}

/// Add `d` to a property's static value or to every keyframe value.
pub fn shift_prop(cx: &mut Cx, ptr: &str, d: &[f64]) -> OpResult<()> {
    let mut p = cx.get(ptr).cloned().unwrap_or_else(|| static_prop(json!(vec![0.0; d.len()])));
    let add = |v: &mut Value| match v {
        Value::Number(n) => *v = json!(n.as_f64().unwrap_or(0.0) + d[0]),
        Value::Array(a) => {
            for (i, x) in a.iter_mut().enumerate().take(d.len()) {
                *x = json!(x.as_f64().unwrap_or(0.0) + d[i]);
            }
        }
        _ => {}
    };
    if util::is_animated(&p) {
        for kf in p["k"].as_array_mut().unwrap() {
            for key in ["s", "e"] {
                if let Some(v) = kf.get_mut(key) {
                    add(v);
                }
            }
        }
    } else if let Some(k) = p.get_mut("k") {
        add(k);
    }
    cx.set(ptr, p)
}

fn align_layers(cx: &mut Cx, a: &Value) -> OpResult {
    let frame = util::frame_arg(a).unwrap_or(0.0);
    let keys: Vec<String> = a.get("layers").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let refs: Vec<LayerRef> = keys.iter().map(|k| cx.layer(k)).collect::<OpResult<_>>()?;
    let geoms: Vec<hit::Geometry> = refs.iter().map(|r| geometry(cx, &r.id, frame)).collect::<OpResult<_>>()?;
    let (w, h, ..) = util::scene(cx);
    let to_sel = opt_str(a, "to") == Some("selection") && refs.len() > 1;
    let area = if to_sel {
        geoms.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, g| [b[0].min(g.bounds[0]), b[1].min(g.bounds[1]), b[2].max(g.bounds[2]), b[3].max(g.bounds[3])])
    } else {
        [0.0, 0.0, w, h]
    };
    let align = arg_str(a, "align")?;
    for (r, g) in refs.iter().zip(&geoms) {
        let b = g.bounds;
        let (dx, dy) = match align {
            "left" => (area[0] - b[0], 0.0),
            "right" => (area[2] - b[2], 0.0),
            "center" => ((area[0] + area[2]) / 2.0 - (b[0] + b[2]) / 2.0, 0.0),
            "top" => (0.0, area[1] - b[1]),
            "bottom" => (0.0, area[3] - b[3]),
            "middle" => (0.0, (area[1] + area[3]) / 2.0 - (b[1] + b[3]) / 2.0),
            other => return err(format!("unknown align '{other}'")),
        };
        if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
            translate(cx, r, dx, dy, frame)?;
        }
    }
    Ok(json!({"aligned": refs.len()}))
}

fn distribute_layers(cx: &mut Cx, a: &Value) -> OpResult {
    let frame = util::frame_arg(a).unwrap_or(0.0);
    let keys: Vec<String> = a.get("layers").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if keys.len() < 3 {
        return err("distribute needs at least 3 layers");
    }
    let horiz = arg_str(a, "axis")? == "horizontal";
    let mut items: Vec<(LayerRef, f64)> = vec![];
    for k in &keys {
        let r = cx.layer(k)?;
        let b = geometry(cx, &r.id, frame)?.bounds;
        items.push((r, if horiz { (b[0] + b[2]) / 2.0 } else { (b[1] + b[3]) / 2.0 }));
    }
    items.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal));
    let (first, last) = (items[0].1, items[items.len() - 1].1);
    let step = (last - first) / (items.len() - 1) as f64;
    for (n, (r, c)) in items.iter().enumerate() {
        let d = first + step * n as f64 - c;
        if d.abs() > 1e-9 {
            if horiz {
                translate(cx, r, d, 0.0, frame)?;
            } else {
                translate(cx, r, 0.0, d, frame)?;
            }
        }
    }
    Ok(json!({"distributed": items.len()}))
}

fn set_transform(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let shape = opt_str(a, "shape");
    let frame = util::frame_arg(a);
    let mut n = 0;
    for prop in ["position", "scale", "rotation", "opacity", "anchor", "skew", "skew_axis"] {
        if let Some(v) = a.get(prop) {
            let ptr = util::prop_ptr(cx, &r, shape, prop)?;
            util::write_value(cx, &ptr, v, frame)?;
            n += 1;
        }
    }
    if n == 0 {
        return err("give at least one of position, scale, rotation, opacity, anchor, skew, skew_axis");
    }
    Ok(json!({"id": r.id, "updated": n}))
}

fn set_anchor_point(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let frame = util::frame_arg(a).unwrap_or(0.0);
    let g = geometry(cx, &r.id, frame)?;
    let lb = g.local;
    let new_a = match a.get("anchor") {
        Some(Value::String(s)) => {
            let (cx_, cy) = ((lb[0] + lb[2]) / 2.0, (lb[1] + lb[3]) / 2.0);
            match s.as_str() {
                "center" => [cx_, cy],
                "top_left" => [lb[0], lb[1]],
                "top" => [cx_, lb[1]],
                "top_right" => [lb[2], lb[1]],
                "left" => [lb[0], cy],
                "right" => [lb[2], cy],
                "bottom_left" => [lb[0], lb[3]],
                "bottom" => [cx_, lb[3]],
                "bottom_right" => [lb[2], lb[3]],
                other => return err(format!("unknown anchor '{other}'")),
            }
        }
        Some(v) => {
            let p = util::coerce("a", v, 2)?;
            [p[0], p.get(1).copied().unwrap_or(0.0)]
        }
        None => return err("missing 'anchor'"),
    };
    let anim = parsed(cx);
    let (comp, _) = r.id.rsplit_once(':').unwrap_or(("root", ""));
    let layers: &[model::Layer] = if comp == "root" { &anim.layers } else { anim.asset(comp).and_then(|x| x.layers.as_deref()).unwrap_or(&[]) };
    let l = &layers[r.index];
    let own = sc::transform_matrix(&l.ks, sc::layer_time(l, frame));
    let old_a = crate::anim::eval_or(l.ks.a.as_ref(), sc::layer_time(l, frame), &[0.0, 0.0]);
    let from = own.apply([old_a[0], old_a.get(1).copied().unwrap_or(0.0)]);
    let to = own.apply(new_a);
    let ptr = format!("{}/ks/a", r.ptr());
    if util::is_animated(cx.get(&ptr).unwrap_or(&Value::Null)) {
        return err("anchor point is animated; edit its keyframes instead");
    }
    util::write_value(cx, &ptr, &json!([new_a[0], new_a[1]]), None)?;
    let p_ptr = format!("{}/ks/p", r.ptr());
    if cx.get(&p_ptr).and_then(|p| p.get("s")).is_some() {
        shift_prop(cx, &format!("{p_ptr}/x"), &[to[0] - from[0]])?;
        shift_prop(cx, &format!("{p_ptr}/y"), &[to[1] - from[1]])?;
    } else {
        shift_prop(cx, &p_ptr, &[to[0] - from[0], to[1] - from[1]])?;
    }
    Ok(json!({"id": r.id, "anchor": new_a}))
}

fn flip_layer(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let i = match arg_str(a, "axis")? {
        "horizontal" => 0,
        "vertical" => 1,
        other => return err(format!("unknown axis '{other}'")),
    };
    let ptr = format!("{}/ks/s", r.ptr());
    let mut p = cx.get(&ptr).cloned().unwrap_or_else(|| static_prop(json!([100.0, 100.0, 100.0])));
    let flip = |v: &mut Value| {
        if let Some(x) = v.get_mut(i) {
            *x = json!(-x.as_f64().unwrap_or(100.0));
        }
    };
    if util::is_animated(&p) {
        for kf in p["k"].as_array_mut().unwrap() {
            if let Some(s) = kf.get_mut("s") {
                flip(s);
            }
        }
    } else if let Some(k) = p.get_mut("k") {
        flip(k);
    }
    cx.set(&ptr, p)?;
    Ok(json!({"id": r.id}))
}

fn paste_layers(cx: &mut Cx, a: &Value) -> OpResult {
    let src = a.get("layers").and_then(Value::as_array).ok_or_else(|| OpError("missing 'layers'".into()))?;
    if src.iter().any(|l| l.get("ty").and_then(Value::as_i64).is_none()) {
        return err("each pasted item must be a Lottie layer object with a numeric 'ty'");
    }
    let scene = scene_arg(a).to_string();
    let lp = cx.comp_layers_ptr(&scene)?;
    let existing: Vec<i64> = cx.get(&lp).and_then(Value::as_array).map(|v| v.iter().filter_map(ind_of).collect()).unwrap_or_default();
    let mut next = cx.next_ind(&lp);
    let mut map: Vec<(i64, i64)> = vec![];
    let mut layers: Vec<Value> = src.clone();
    for l in layers.iter_mut() {
        let new = next;
        next += 1;
        if let Some(old) = ind_of(l) {
            map.push((old, new));
        }
        l["ind"] = json!(new);
    }
    let remap = |old: i64| map.iter().find(|m| m.0 == old).map(|m| m.1);
    for l in layers.iter_mut() {
        let o = l.as_object_mut().unwrap();
        if let Some(p) = o.get("parent").and_then(Value::as_i64) {
            match remap(p) {
                Some(n) => {
                    o.insert("parent".into(), json!(n));
                }
                None if existing.contains(&p) => {}
                None => {
                    o.remove("parent");
                }
            }
        }
        if let Some(tp) = o.get("tp").and_then(Value::as_i64) {
            match remap(tp) {
                Some(n) => {
                    o.insert("tp".into(), json!(n));
                }
                None => {
                    o.remove("tp");
                    o.remove("tt");
                }
            }
        }
    }
    let mut ids = vec![];
    for (i, l) in layers.into_iter().enumerate() {
        ids.push(format!("{scene}:{}", l["ind"]));
        cx.insert(&lp, i, l)?;
    }
    if let Some(off) = opt_vec(a, "offset").filter(|o| o.len() >= 2 && (o[0] != 0.0 || o[1] != 0.0)) {
        for id in &ids {
            let r = cx.layer(id)?;
            if cx.layer_value(&r).get("parent").is_none() {
                translate(cx, &r, off[0], off[1], 0.0)?;
            }
        }
    }
    Ok(json!({"ids": ids}))
}
