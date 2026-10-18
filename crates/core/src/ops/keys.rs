//! Keyframe tools.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, is_animated, parse_easing, read_value, upsert_keyframe};
use super::{arg_f64, arg_str, err, opt_f64, opt_str, Cx, OpError, OpResult};

fn target() -> Value {
    json!({
        "layer": {"type": "string", "description": "Layer id or unique name"},
        "shape": {"type": "string", "description": "Optional shape path ('0', '0/1'), 'mask:<i>' or 'effect:<i>'"},
        "property": {"type": "string", "description": "position, scale, rotation, opacity, anchor, skew, skew_axis, color, width, size, roundness, start, end, offset, path, … or a raw Lottie key"}
    })
}

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(b), Value::Object(e)) = (base.as_object_mut(), extra) {
        b.extend(e);
    }
    base
}

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "set_keyframe",
            description: "Add or replace a keyframe on one property at an integer frame. A static property becomes animated. Optional easing (preset name, 'hold', or [x1,y1,x2,y2]) applies to the segment starting at this keyframe.",
            schema: || obj(with(target(), json!({"frame": {"type": "number", "minimum": 0}, "value": {"description": "Number, [x,y], colour hex, or bezier {v,i,o,c} for paths"}, "easing": {}})), &["layer", "property", "frame", "value"]),
            mutates: true,
            handler: set_keyframe,
        },
        ToolDef {
            name: "remove_keyframe",
            description: "Remove the keyframe at a frame. When one keyframe or none remains the property becomes static.",
            schema: || obj(with(target(), json!({"frame": {"type": "number"}})), &["layer", "property", "frame"]),
            mutates: true,
            handler: remove_keyframe,
        },
        ToolDef {
            name: "set_keyframe_easing",
            description: "Set the easing of the segment that starts at the keyframe on 'frame'. Presets: linear, ease, ease_in, ease_out, ease_in_out, smooth, ease_in_back, ease_out_back, ease_in_out_back, snappy, hold — or a cubic bezier [x1,y1,x2,y2].",
            schema: || obj(with(target(), json!({"frame": {"type": "number"}, "easing": {}})), &["layer", "property", "frame", "easing"]),
            mutates: true,
            handler: set_keyframe_easing,
        },
        ToolDef {
            name: "get_property",
            description: "Read one property at a frame: evaluated value, whether it is animated, whether a keyframe sits on this frame, and all keyframe frames.",
            schema: || obj(with(target(), json!({"frame": {"type": "number"}})), &["layer", "property"]),
            mutates: false,
            handler: get_property,
        },
        ToolDef {
            name: "read_keyframes",
            description: "List keyframes (frame, value, easing) of one property, or of every animated property of a layer when 'property' is omitted.",
            schema: || obj(target(), &["layer"]),
            mutates: false,
            handler: read_keyframes,
        },
        ToolDef {
            name: "clear_animation",
            description: "Remove all keyframes of a property, keeping its value at 'frame' (default the first keyframe) as a static value.",
            schema: || obj(with(target(), json!({"frame": {"type": "number"}})), &["layer", "property"]),
            mutates: true,
            handler: clear_animation,
        },
        ToolDef {
            name: "shift_keyframes",
            description: "Move keyframes in time by 'by' frames (negative = earlier). Applies to one property, or all animated properties of the layer; optionally only keyframes at or after 'from_frame'.",
            schema: || obj(with(target(), json!({"by": {"type": "number"}, "from_frame": {"type": "number"}})), &["layer", "by"]),
            mutates: true,
            handler: shift_keyframes,
        },
        ToolDef {
            name: "retime_keyframes",
            description: "Scale keyframe timing around a pivot frame (scale 2 = twice as slow, 0.5 = twice as fast).",
            schema: || obj(with(target(), json!({"scale": {"type": "number", "exclusiveMinimum": 0}, "pivot": {"type": "number"}})), &["layer", "scale"]),
            mutates: true,
            handler: retime_keyframes,
        },
        ToolDef {
            name: "copy_animation",
            description: "Copy a layer's transform animation (all animated transform properties, or the listed ones) onto another layer, optionally offset in time.",
            schema: || {
                obj(
                    json!({"from": {"type": "string"}, "to": {"type": "string"}, "properties": {"type": "array", "items": {"type": "string"}}, "offset_frames": {"type": "number"}}),
                    &["from", "to"],
                )
            },
            mutates: true,
            handler: copy_animation,
        },
    ]
}

fn resolve(cx: &Cx, a: &Value) -> OpResult<String> {
    let r = cx.layer(arg_str(a, "layer")?)?;
    util::prop_ptr(cx, &r, opt_str(a, "shape"), arg_str(a, "property")?)
}

fn kf_index(p: &Value, frame: f64) -> Option<usize> {
    p.get("k")?.as_array()?.iter().position(|k| (k.get("t").and_then(Value::as_f64).unwrap_or(f64::NAN) - frame).abs() < 1e-6)
}

fn set_keyframe(cx: &mut Cx, a: &Value) -> OpResult {
    let ptr = resolve(cx, a)?;
    let frame = arg_f64(a, "frame")?;
    if frame < 0.0 {
        return err("frame must be >= 0");
    }
    let value = a.get("value").ok_or_else(|| OpError("missing 'value'".into()))?;
    let easing = match a.get("easing") {
        Some(e) => Some(parse_easing(e)?),
        None => None,
    };
    upsert_keyframe(cx, &ptr, frame.round(), value, easing)?;
    Ok(json!({"ok": true, "frame": frame.round()}))
}

fn remove_keyframe(cx: &mut Cx, a: &Value) -> OpResult {
    let ptr = resolve(cx, a)?;
    let frame = arg_f64(a, "frame")?;
    let p = cx.get(&ptr).cloned().unwrap_or(Value::Null);
    if !is_animated(&p) {
        return err("property is not animated");
    }
    let i = kf_index(&p, frame).ok_or_else(|| OpError(format!("no keyframe at frame {frame}")))?;
    let mut kfs = p["k"].as_array().cloned().unwrap();
    let removed = kfs.remove(i);
    let mut np = p.clone();
    if kfs.len() <= 1 {
        let keep = kfs.first().unwrap_or(&removed);
        let s = keep.get("s").cloned().unwrap_or(json!(0));
        np["a"] = json!(0);
        np["k"] = static_from_s(&ptr, &s, &p);
    } else {
        np["k"] = Value::Array(kfs);
    }
    cx.set(&ptr, np)?;
    Ok(json!({"ok": true}))
}

/// Static `k` from a keyframe `s` value (unwrapping path arrays and 1-element scalars).
fn static_from_s(ptr: &str, s: &Value, _orig: &Value) -> Value {
    let key = ptr.rsplit('/').next().unwrap_or("");
    match s {
        Value::Array(a) if matches!(key, "ks" | "pt") && a.first().map(Value::is_object).unwrap_or(false) => a[0].clone(),
        Value::Array(a) if a.len() == 1 && matches!(key, "r" | "o" | "w" | "sk" | "sa" | "pt" | "or" | "ir" | "os" | "is" | "e" | "c" | "h") && a[0].is_number() => a[0].clone(),
        other => other.clone(),
    }
}

fn set_keyframe_easing(cx: &mut Cx, a: &Value) -> OpResult {
    let ptr = resolve(cx, a)?;
    let frame = arg_f64(a, "frame")?;
    let easing = parse_easing(a.get("easing").unwrap_or(&Value::Null))?;
    let p = cx.get(&ptr).cloned().unwrap_or(Value::Null);
    if !is_animated(&p) {
        return err("property is not animated");
    }
    let i = kf_index(&p, frame).ok_or_else(|| OpError(format!("no keyframe at frame {frame}")))?;
    let mut kf = p["k"][i].clone();
    let o = kf.as_object_mut().unwrap();
    match easing {
        Some(b) => {
            let (eo, ei) = util::ease_json(b);
            o.remove("h");
            o.insert("o".into(), eo);
            o.insert("i".into(), ei);
        }
        None => {
            o.insert("h".into(), json!(1));
        }
    }
    cx.set(&format!("{ptr}/k/{i}"), kf)?;
    Ok(json!({"ok": true}))
}

fn describe(p: &Value) -> Value {
    if !is_animated(p) {
        return json!({"animated": false, "value": p.get("k").cloned().unwrap_or(Value::Null)});
    }
    let kfs: Vec<Value> = p["k"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            let easing = if k.get("h").and_then(Value::as_i64) == Some(1) || k.get("h").and_then(Value::as_bool) == Some(true) {
                json!("hold")
            } else {
                let g = |e: &str, c: &str| k.get(e).and_then(|x| x.get(c)).and_then(|v| v.as_array().and_then(|a| a.first()).or(Some(v))).and_then(Value::as_f64);
                match (g("o", "x"), g("o", "y"), g("i", "x"), g("i", "y")) {
                    (Some(x1), Some(y1), Some(x2), Some(y2)) => json!([x1, y1, x2, y2]),
                    _ => Value::Null,
                }
            };
            json!({"frame": k.get("t"), "value": k.get("s"), "easing": easing})
        })
        .collect();
    json!({"animated": true, "keyframes": kfs})
}

fn read_keyframes(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    if opt_str(a, "property").is_some() {
        let ptr = resolve(cx, a)?;
        return Ok(describe(cx.get(&ptr).unwrap_or(&Value::Null)));
    }
    let lp = r.ptr();
    let mut paths = vec![];
    util::animated_props(&cx.layer_value(&r), "", &mut paths);
    let out: Vec<Value> = paths.iter().map(|p| json!({"path": p.trim_start_matches('/'), "keyframes": describe(cx.get(&format!("{lp}{p}")).unwrap())["keyframes"]})).collect();
    Ok(json!({"layer": r.id, "animated": out}))
}

fn clear_animation(cx: &mut Cx, a: &Value) -> OpResult {
    let ptr = resolve(cx, a)?;
    let p = cx.get(&ptr).cloned().unwrap_or(Value::Null);
    if !is_animated(&p) {
        return Ok(json!({"ok": true, "note": "already static"}));
    }
    let key = ptr.rsplit('/').next().unwrap_or("");
    let k = if matches!(key, "ks" | "pt") {
        let frame = opt_f64(a, "frame").unwrap_or(p["k"][0]["t"].as_f64().unwrap_or(0.0));
        let sp = crate::model::shape_prop(Some(&p)).unwrap_or_default();
        let b = crate::anim::eval_shape(&sp, frame);
        json!({"c": b.c, "v": b.v, "i": b.i, "o": b.o})
    } else {
        let frame = opt_f64(a, "frame").unwrap_or(p["k"][0]["t"].as_f64().unwrap_or(0.0));
        let v = read_value(cx, &ptr, frame);
        if v.len() == 1 {
            json!(v[0])
        } else {
            json!(v)
        }
    };
    let mut np = p.clone();
    np["a"] = json!(0);
    np["k"] = k;
    cx.set(&ptr, np)?;
    Ok(json!({"ok": true}))
}

fn target_ptrs(cx: &Cx, a: &Value) -> OpResult<Vec<String>> {
    if opt_str(a, "property").is_some() {
        return Ok(vec![resolve(cx, a)?]);
    }
    let r = cx.layer(arg_str(a, "layer")?)?;
    let mut paths = vec![];
    util::animated_props(&cx.layer_value(&r), "", &mut paths);
    Ok(paths.into_iter().map(|p| format!("{}{p}", r.ptr())).collect())
}

fn map_times(cx: &mut Cx, ptrs: &[String], f: impl Fn(f64) -> Option<f64>) -> OpResult<usize> {
    let mut n = 0;
    for ptr in ptrs {
        let mut p = cx.get(ptr).cloned().unwrap_or(Value::Null);
        if !is_animated(&p) {
            continue;
        }
        for kf in p["k"].as_array_mut().unwrap() {
            let t = kf.get("t").and_then(Value::as_f64).unwrap_or(0.0);
            if let Some(nt) = f(t) {
                kf["t"] = json!(nt.round());
                n += 1;
            }
        }
        p["k"].as_array_mut().unwrap().sort_by(|x, y| x["t"].as_f64().partial_cmp(&y["t"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
        cx.set(ptr, p)?;
    }
    Ok(n)
}

fn shift_keyframes(cx: &mut Cx, a: &Value) -> OpResult {
    let by = arg_f64(a, "by")?;
    let from = opt_f64(a, "from_frame").unwrap_or(f64::NEG_INFINITY);
    let ptrs = target_ptrs(cx, a)?;
    let n = map_times(cx, &ptrs, |t| (t >= from).then_some((t + by).max(0.0)))?;
    Ok(json!({"moved": n}))
}

fn retime_keyframes(cx: &mut Cx, a: &Value) -> OpResult {
    let s = arg_f64(a, "scale")?;
    if s <= 0.0 {
        return err("scale must be > 0");
    }
    let pivot = opt_f64(a, "pivot").unwrap_or(0.0);
    let ptrs = target_ptrs(cx, a)?;
    let n = map_times(cx, &ptrs, |t| Some((pivot + (t - pivot) * s).max(0.0)))?;
    Ok(json!({"retimed": n}))
}

fn copy_animation(cx: &mut Cx, a: &Value) -> OpResult {
    let from = cx.layer(arg_str(a, "from")?)?;
    let to = cx.layer(arg_str(a, "to")?)?;
    let offset = opt_f64(a, "offset_frames").unwrap_or(0.0);
    let wanted: Option<Vec<String>> = a.get("properties").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect());
    let mut n = 0;
    for (name, key) in [("position", "p"), ("anchor", "a"), ("scale", "s"), ("rotation", "r"), ("opacity", "o"), ("skew", "sk"), ("skew_axis", "sa")] {
        if let Some(w) = &wanted {
            if !w.iter().any(|x| x == name || x == key) {
                continue;
            }
        }
        let src = format!("{}/ks/{key}", from.ptr());
        let Some(mut p) = cx.get(&src).cloned() else { continue };
        if wanted.is_none() && !is_animated(&p) {
            continue;
        }
        if is_animated(&p) && offset != 0.0 {
            for kf in p["k"].as_array_mut().unwrap() {
                kf["t"] = json!(kf["t"].as_f64().unwrap_or(0.0) + offset);
            }
        }
        cx.set(&format!("{}/ks/{key}", to.ptr()), p)?;
        n += 1;
    }
    Ok(json!({"copied": n}))
}

fn get_property(cx: &mut Cx, a: &Value) -> OpResult {
    let ptr = resolve(cx, a)?;
    let frame = opt_f64(a, "frame").unwrap_or(0.0);
    let p = cx.get(&ptr).cloned().unwrap_or(Value::Null);
    if p.is_null() {
        return Ok(json!({"exists": false}));
    }
    let key = ptr.rsplit('/').next().unwrap_or("");
    let value = if matches!(key, "ks" | "pt") {
        let sp = crate::model::shape_prop(Some(&p)).unwrap_or_default();
        let b = crate::anim::eval_shape(&sp, frame);
        json!({"c": b.c, "v": b.v, "i": b.i, "o": b.o})
    } else {
        json!(read_value(cx, &ptr, frame))
    };
    let frames: Vec<f64> = if is_animated(&p) { p["k"].as_array().unwrap().iter().filter_map(|k| k.get("t").and_then(Value::as_f64)).collect() } else { vec![] };
    Ok(json!({"exists": true, "value": value, "animated": is_animated(&p), "keyed": frames.iter().any(|f| (f - frame).abs() < 1e-6),
        "keyframes": frames, "slot": p.get("sid").cloned().unwrap_or(Value::Null)}))
}
