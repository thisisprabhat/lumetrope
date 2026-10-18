//! Mask and layer-effect tools.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, parse_color, static_prop};
use super::{arg_str, err, opt_bool, opt_f64, opt_str, opt_vec, Cx, OpError, OpResult};
use crate::geom;

const MASK_MODES: &[(&str, &str)] = &[("add", "a"), ("subtract", "s"), ("intersect", "i"), ("lighten", "l"), ("darken", "d"), ("difference", "f"), ("none", "n")];

/// Effect parameter: (key, AE display name, value type code, default).
type Param = (&'static str, &'static str, i64, fn() -> Value);

struct FxDef {
    key: &'static str,
    ty: i64,
    name: &'static str,
    mn: &'static str,
    params: &'static [Param],
}

const EFFECTS: &[FxDef] = &[
    FxDef {
        key: "drop_shadow",
        ty: 25,
        name: "Drop Shadow",
        mn: "ADBE Drop Shadow",
        params: &[
            ("color", "Shadow Color", 2, || json!([0.0, 0.0, 0.0, 1.0])),
            ("opacity", "Opacity", 0, || json!(127.5)),
            ("direction", "Direction", 1, || json!(135.0)),
            ("distance", "Distance", 0, || json!(10.0)),
            ("softness", "Softness", 0, || json!(10.0)),
            ("shadow_only", "Shadow Only", 4, || json!(0)),
        ],
    },
    FxDef {
        key: "gaussian_blur",
        ty: 29,
        name: "Gaussian Blur",
        mn: "ADBE Gaussian Blur 2",
        params: &[("blurriness", "Blurriness", 0, || json!(10.0)), ("dimensions", "Blur Dimensions", 7, || json!(1)), ("repeat_edges", "Repeat Edge Pixels", 4, || json!(0))],
    },
    FxDef {
        key: "fill",
        ty: 21,
        name: "Fill",
        mn: "ADBE Fill",
        params: &[
            ("fill_mask", "Fill Mask", 10, || json!(0)),
            ("all_masks", "All Masks", 4, || json!(0)),
            ("color", "Color", 2, || json!([1.0, 0.0, 0.0, 1.0])),
            ("invert", "Invert", 4, || json!(0)),
            ("horizontal_feather", "Horizontal Feather", 0, || json!(0.0)),
            ("vertical_feather", "Vertical Feather", 0, || json!(0.0)),
            ("opacity", "Opacity", 0, || json!(1.0)),
        ],
    },
    FxDef {
        key: "tint",
        ty: 20,
        name: "Tint",
        mn: "ADBE Tint",
        params: &[("black", "Map Black To", 2, || json!([0.0, 0.0, 0.0, 1.0])), ("white", "Map White To", 2, || json!([1.0, 1.0, 1.0, 1.0])), ("amount", "Amount to Tint", 0, || json!(100.0))],
    },
    FxDef {
        key: "tritone",
        ty: 23,
        name: "Tritone",
        mn: "ADBE Tritone",
        params: &[
            ("highlights", "Highlights", 2, || json!([1.0, 1.0, 1.0, 1.0])),
            ("midtones", "Midtones", 2, || json!([0.5, 0.5, 0.5, 1.0])),
            ("shadows", "Shadows", 2, || json!([0.0, 0.0, 0.0, 1.0])),
            ("blend", "Blend With Original", 0, || json!(0.0)),
        ],
    },
];

fn fx_by_key(k: &str) -> Option<&'static FxDef> {
    EFFECTS.iter().find(|f| f.key == k)
}

fn fx_by_ty(t: i64) -> Option<&'static FxDef> {
    EFFECTS.iter().find(|f| f.ty == t)
}

/// User-facing parameter value → stored effect value.
fn to_store(fx: &FxDef, key: &str, v: &Value) -> OpResult<Value> {
    let p = fx.params.iter().find(|p| p.0 == key).ok_or_else(|| OpError(format!("{} has no parameter '{key}' ({})", fx.key, fx.params.iter().map(|p| p.0).collect::<Vec<_>>().join(", "))))?;
    Ok(match (fx.key, key, p.2) {
        (_, _, 2) => json!(parse_color(v)?),
        (_, _, 4) => json!(if v.as_bool().unwrap_or(v.as_f64().unwrap_or(0.0) != 0.0) { 1 } else { 0 }),
        ("gaussian_blur", "dimensions", _) => json!(match v.as_str() {
            Some("horizontal") => 2,
            Some("vertical") => 3,
            Some("both") | None => v.as_i64().unwrap_or(1),
            Some(o) => return err(format!("dimensions must be both, horizontal or vertical, not '{o}'")),
        }),
        ("drop_shadow", "opacity", _) => json!(v.as_f64().ok_or_else(|| OpError("opacity must be a number (0-100)".into()))? * 255.0 / 100.0),
        ("fill", "opacity", _) => json!(v.as_f64().ok_or_else(|| OpError("opacity must be a number (0-100)".into()))? / 100.0),
        _ => json!(v.as_f64().ok_or_else(|| OpError(format!("{key} must be a number")))?),
    })
}

fn from_store(fx: &FxDef, key: &str, v: &Value) -> Value {
    let n = v.as_f64();
    match (fx.key, key) {
        ("drop_shadow", "opacity") => json!(n.unwrap_or(0.0) * 100.0 / 255.0),
        ("fill", "opacity") => json!(n.unwrap_or(0.0) * 100.0),
        _ => v.clone(),
    }
}

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "add_mask",
            description: "Add a mask to a layer: rect/ellipse (position + size in layer space, default the layer bounds) or a bezier path. Modes: add, subtract, intersect, lighten, darken, difference, none.",
            schema: || {
                obj(
                    json!({"layer": {"type": "string"}, "shape": {"type": "string", "enum": ["rect", "ellipse", "path"]}, "position": {"type": "array"}, "size": {"type": "array"},
                        "path": {"type": "object"}, "mode": {"type": "string"}, "inverted": {"type": "boolean"}, "opacity": {"type": "number"}, "name": {"type": "string"}}),
                    &["layer"],
                )
            },
            mutates: true,
            handler: add_mask,
        },
        ToolDef {
            name: "update_mask",
            description: "Change a mask's mode, inversion, opacity, name or path (path keyframes via set_keyframe with shape 'mask:<i>').",
            schema: || obj(json!({"layer": {"type": "string"}, "index": {"type": "integer"}, "mode": {"type": "string"}, "inverted": {"type": "boolean"}, "opacity": {"type": "number"}, "path": {"type": "object"}, "name": {"type": "string"}, "frame": {"type": "number"}}), &["layer", "index"]),
            mutates: true,
            handler: update_mask,
        },
        ToolDef {
            name: "remove_mask",
            description: "Remove a mask from a layer.",
            schema: || obj(json!({"layer": {"type": "string"}, "index": {"type": "integer"}}), &["layer", "index"]),
            mutates: true,
            handler: remove_mask,
        },
        ToolDef {
            name: "add_effect",
            description: "Add a layer effect: drop_shadow (color, opacity %, direction °, distance, softness, shadow_only), gaussian_blur (blurriness, dimensions both|horizontal|vertical), fill (color, opacity %), tint (black, white, amount %), tritone (highlights, midtones, shadows, blend %).",
            schema: || obj(json!({"layer": {"type": "string"}, "type": {"type": "string", "enum": ["drop_shadow", "gaussian_blur", "fill", "tint", "tritone"]}, "params": {"type": "object"}}), &["layer", "type"]),
            mutates: true,
            handler: add_effect,
        },
        ToolDef {
            name: "update_effect",
            description: "Update parameters of a layer effect by index (see list_effects). Animated parameters get a keyframe at 'frame'; 'enabled' toggles the effect.",
            schema: || obj(json!({"layer": {"type": "string"}, "index": {"type": "integer"}, "params": {"type": "object"}, "enabled": {"type": "boolean"}, "frame": {"type": "number"}}), &["layer", "index"]),
            mutates: true,
            handler: update_effect,
        },
        ToolDef {
            name: "remove_effect",
            description: "Remove a layer effect by index.",
            schema: || obj(json!({"layer": {"type": "string"}, "index": {"type": "integer"}}), &["layer", "index"]),
            mutates: true,
            handler: remove_effect,
        },
        ToolDef {
            name: "list_effects",
            description: "List a layer's effects with their parameters.",
            schema: || obj(json!({"layer": {"type": "string"}}), &["layer"]),
            mutates: false,
            handler: list_effects,
        },
    ]
}

fn mask_mode(m: &str) -> OpResult<&'static str> {
    MASK_MODES.iter().find(|x| x.0 == m || x.1 == m).map(|x| x.1).ok_or_else(|| OpError(format!("unknown mask mode '{m}'")))
}

fn index(a: &Value) -> OpResult<usize> {
    a.get("index").and_then(Value::as_u64).map(|i| i as usize).ok_or_else(|| OpError("missing 'index'".into()))
}

fn add_mask(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let local = super::layers::geometry(cx, &r.id, 0.0).map(|g| g.local).unwrap_or([0.0, 0.0, 100.0, 100.0]);
    let center = opt_vec(a, "position").map(|p| [p[0], p.get(1).copied().unwrap_or(0.0)]).unwrap_or([(local[0] + local[2]) / 2.0, (local[1] + local[3]) / 2.0]);
    let size = opt_vec(a, "size").map(|s| [s[0], s.get(1).copied().unwrap_or(s[0])]).unwrap_or([local[2] - local[0], local[3] - local[1]]);
    let path = match (opt_str(a, "shape").unwrap_or("rect"), a.get("path")) {
        (_, Some(p)) => util::bezier_value(p)?,
        ("ellipse", None) => {
            let c = geom::ellipse(center, size, false);
            json!({"c": true, "v": c.v, "i": c.i, "o": c.o})
        }
        ("rect", None) => {
            let c = geom::rect(center, size, 0.0, false);
            json!({"c": true, "v": c.v, "i": c.i, "o": c.o})
        }
        (other, None) => return err(format!("unknown mask shape '{other}'")),
    };
    let lp = r.ptr();
    let n = cx.get(&format!("{lp}/masksProperties")).and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    let m = json!({"inv": opt_bool(a, "inverted").unwrap_or(false), "mode": mask_mode(opt_str(a, "mode").unwrap_or("add"))?,
        "pt": static_prop(path), "o": static_prop(json!(opt_f64(a, "opacity").unwrap_or(100.0))), "x": static_prop(json!(0)),
        "nm": opt_str(a, "name").map(str::to_string).unwrap_or_else(|| format!("Mask {}", n + 1))});
    cx.insert(&format!("{lp}/masksProperties"), n, m)?;
    cx.set(&format!("{lp}/hasMask"), json!(true))?;
    Ok(json!({"layer": r.id, "index": n}))
}

fn update_mask(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let i = index(a)?;
    let mp = format!("{}/masksProperties/{i}", r.ptr());
    if cx.get(&mp).is_none() {
        return err(format!("layer has no mask {i}"));
    }
    if let Some(m) = opt_str(a, "mode") {
        cx.set(&format!("{mp}/mode"), json!(mask_mode(m)?))?;
    }
    if let Some(v) = opt_bool(a, "inverted") {
        cx.set(&format!("{mp}/inv"), json!(v))?;
    }
    if let Some(n) = opt_str(a, "name") {
        cx.set(&format!("{mp}/nm"), json!(n))?;
    }
    let frame = util::frame_arg(a);
    if let Some(o) = a.get("opacity") {
        util::write_value(cx, &format!("{mp}/o"), o, frame)?;
    }
    if let Some(p) = a.get("path") {
        util::write_value(cx, &format!("{mp}/pt"), p, frame)?;
    }
    Ok(json!({"ok": true}))
}

fn remove_mask(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let i = index(a)?;
    let lp = r.ptr();
    cx.remove(&format!("{lp}/masksProperties"), i)?;
    if cx.get(&format!("{lp}/masksProperties")).and_then(Value::as_array).map(Vec::is_empty).unwrap_or(true) {
        cx.delete(&format!("{lp}/masksProperties"))?;
        cx.delete(&format!("{lp}/hasMask"))?;
    }
    Ok(json!({"ok": true}))
}

fn add_effect(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let kind = arg_str(a, "type")?;
    let fx = fx_by_key(kind).ok_or_else(|| OpError(format!("unknown effect type '{kind}' (drop_shadow, gaussian_blur, fill, tint, tritone)")))?;
    let params = a.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut vals = vec![];
    for (i, p) in fx.params.iter().enumerate() {
        let v = match params.get(p.0) {
            Some(x) => to_store(fx, p.0, x)?,
            None => (p.3)(),
        };
        vals.push(json!({"ty": p.2, "nm": p.1, "mn": format!("{} {}", fx.mn, i + 1), "ix": i + 1, "v": static_prop(v)}));
    }
    if let Some(k) = params.keys().find(|k| !fx.params.iter().any(|p| p.0 == k.as_str())) {
        return err(format!("{kind} has no parameter '{k}'"));
    }
    let lp = format!("{}/ef", r.ptr());
    let n = cx.get(&lp).and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    cx.insert(&lp, n, json!({"ty": fx.ty, "nm": fx.name, "np": vals.len() + 1, "mn": fx.mn, "ix": n + 1, "en": 1, "ef": vals}))?;
    Ok(json!({"layer": r.id, "index": n}))
}

fn effect_at(cx: &Cx, lp: &str, i: usize) -> OpResult<(&'static FxDef, String)> {
    let ep = format!("{lp}/ef/{i}");
    let ty = cx.get(&format!("{ep}/ty")).and_then(Value::as_i64).ok_or_else(|| OpError(format!("layer has no effect {i}")))?;
    let fx = fx_by_ty(ty).ok_or_else(|| OpError(format!("effect type {ty} is not editable")))?;
    Ok((fx, ep))
}

fn update_effect(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let (fx, ep) = effect_at(cx, &r.ptr(), index(a)?)?;
    let frame = util::frame_arg(a);
    if let Some(params) = a.get("params").and_then(Value::as_object) {
        for (k, v) in params {
            let j = fx.params.iter().position(|p| p.0 == k).ok_or_else(|| OpError(format!("{} has no parameter '{k}'", fx.key)))?;
            let stored = to_store(fx, k, v)?;
            util::write_value(cx, &format!("{ep}/ef/{j}/v"), &stored, frame)?;
        }
    }
    if let Some(en) = opt_bool(a, "enabled") {
        cx.set(&format!("{ep}/en"), json!(if en { 1 } else { 0 }))?;
    }
    Ok(json!({"ok": true}))
}

fn remove_effect(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    cx.remove(&format!("{}/ef", r.ptr()), index(a)?)?;
    Ok(json!({"ok": true}))
}

fn list_effects(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let efs = cx.get(&format!("{}/ef", r.ptr())).and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(Value::Array(
        efs.iter()
            .enumerate()
            .map(|(i, e)| {
                let ty = e.get("ty").and_then(Value::as_i64).unwrap_or(0);
                let fx = fx_by_ty(ty);
                let mut params = serde_json::Map::new();
                if let Some(fx) = fx {
                    for (j, p) in fx.params.iter().enumerate() {
                        if let Some(v) = e.pointer(&format!("/ef/{j}/v")) {
                            let k = v.get("k").cloned().unwrap_or(Value::Null);
                            params.insert(p.0.into(), if util::is_animated(v) { json!("animated") } else { from_store(fx, p.0, &k) });
                        }
                    }
                }
                json!({"index": i, "type": fx.map(|f| f.key).unwrap_or("unknown"), "name": e.get("nm"), "enabled": e.get("en").and_then(Value::as_i64).unwrap_or(1) != 0, "params": params})
            })
            .collect(),
    ))
}
