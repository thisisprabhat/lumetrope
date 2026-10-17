//! Shared helpers for tools: colours, easing presets, property addressing and
//! keyframe editing on raw Lottie JSON.

use serde_json::{json, Value};

use super::{err, Cx, LayerRef, OpError, OpResult};
use crate::anim;
use crate::model;

// ------------------------------------------------------------------ colours

/// Parse `#rgb`, `#rrggbb`, `#rrggbbaa` or `[r,g,b(,a)]` (0..1) into RGBA 0..1.
pub fn parse_color(v: &Value) -> OpResult<[f64; 4]> {
    match v {
        Value::String(s) => {
            let h = s.trim().trim_start_matches('#');
            let h = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect::<String>() } else { h.to_string() };
            if h.len() != 6 && h.len() != 8 {
                return err(format!("invalid colour '{s}'"));
            }
            let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map(|x| x as f64 / 255.0).map_err(|_| OpError(format!("invalid colour '{s}'")));
            Ok([c(0)?, c(2)?, c(4)?, if h.len() == 8 { c(6)? } else { 1.0 }])
        }
        Value::Array(a) if a.len() >= 3 => {
            let n: Vec<f64> = a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect();
            let big = n.iter().take(3).any(|x| *x > 1.0);
            let f = |x: f64| if big { x / 255.0 } else { x };
            Ok([f(n[0]), f(n[1]), f(n[2]), n.get(3).copied().map(|a| if a > 1.0 { a / 255.0 } else { a }).unwrap_or(1.0)])
        }
        _ => err("colour must be a hex string like \"#ff8800\" or [r,g,b] in 0..1"),
    }
}

pub fn hex(c: &[f64]) -> String {
    let g = |i: usize| ((c.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round()) as u8;
    format!("#{:02x}{:02x}{:02x}", g(0), g(1), g(2))
}

// ------------------------------------------------------------------ easing

pub const EASING_PRESETS: &[(&str, [f64; 4])] = &[
    ("linear", [0.0, 0.0, 1.0, 1.0]),
    ("ease", [0.25, 0.1, 0.25, 1.0]),
    ("ease_in", [0.42, 0.0, 1.0, 1.0]),
    ("ease_out", [0.0, 0.0, 0.58, 1.0]),
    ("ease_in_out", [0.42, 0.0, 0.58, 1.0]),
    ("smooth", [0.33, 0.0, 0.67, 1.0]),
    ("ease_in_back", [0.36, 0.0, 0.66, -0.56]),
    ("ease_out_back", [0.34, 1.56, 0.64, 1.0]),
    ("ease_in_out_back", [0.68, -0.6, 0.32, 1.6]),
    ("snappy", [0.2, 0.9, 0.1, 1.0]),
];

/// Easing spec → Some(bezier) or None for hold.
pub fn parse_easing(v: &Value) -> OpResult<Option<[f64; 4]>> {
    match v {
        Value::String(s) if s == "hold" => Ok(None),
        Value::String(s) => EASING_PRESETS.iter().find(|(n, _)| n == s).map(|(_, b)| Some(*b)).ok_or_else(|| {
            OpError(format!("unknown easing '{s}' (use hold, {})", EASING_PRESETS.iter().map(|p| p.0).collect::<Vec<_>>().join(", ")))
        }),
        Value::Array(a) if a.len() == 4 => Ok(Some([0, 1, 2, 3].map(|i| a[i].as_f64().unwrap_or(0.0)))),
        Value::Object(o) => {
            let g = |k: &str| o.get(k).and_then(Value::as_f64).ok_or_else(|| OpError(format!("easing missing '{k}'")));
            Ok(Some([g("x1")?, g("y1")?, g("x2")?, g("y2")?]))
        }
        _ => err("easing must be a preset name, \"hold\", [x1,y1,x2,y2] or {x1,y1,x2,y2}"),
    }
}

pub fn ease_json(b: [f64; 4]) -> (Value, Value) {
    (json!({"x": [b[0]], "y": [b[1]]}), json!({"x": [b[2]], "y": [b[3]]}))
}

pub const DEFAULT_EASE: [f64; 4] = [0.42, 0.0, 0.58, 1.0];

// ------------------------------------------------------------------ addressing

/// Pointer to the shape item addressed by `path` ("0/2/1") inside a layer.
pub fn shape_ptr(layer_ptr: &str, path: &str) -> String {
    let mut p = format!("{layer_ptr}/shapes");
    for (n, seg) in path.split('/').filter(|s| !s.is_empty()).enumerate() {
        if n > 0 {
            p.push_str("/it");
        }
        p.push('/');
        p.push_str(seg);
    }
    p
}

/// Pointer of the transform object for a target: layer `ks`, or a group's `tr` item.
fn transform_ptr(cx: &Cx, target_ptr: &str, kind: &str) -> OpResult<String> {
    if kind == "layer" {
        return Ok(format!("{target_ptr}/ks"));
    }
    let items = cx.get(&format!("{target_ptr}/it")).and_then(Value::as_array).cloned().unwrap_or_default();
    match items.iter().rposition(|i| i.get("ty").and_then(Value::as_str) == Some("tr")) {
        Some(i) => Ok(format!("{target_ptr}/it/{i}")),
        None => err("group has no transform item"),
    }
}

fn friendly(kind: &str, prop: &str) -> Option<&'static str> {
    let t = match prop {
        "position" => "p",
        "anchor" | "anchor_point" => "a",
        "scale" => "s",
        "rotation" => "r",
        "opacity" => "o",
        "skew" => "sk",
        "skewAxis" | "skew_axis" => "sa",
        _ => "",
    };
    let generic = match (kind, prop) {
        ("rc" | "el" | "sr", "position") => "p",
        ("rc" | "el", "size") => "s",
        ("rc", "roundness") => "r",
        ("sr", "points") => "pt",
        ("sr", "outer_radius" | "radius") => "or",
        ("sr", "inner_radius") => "ir",
        ("sr", "outer_roundness" | "roundness") => "os",
        ("sr", "inner_roundness") => "is",
        ("sr", "rotation") => "r",
        ("sh", "path") => "ks",
        ("fl" | "st", "color") => "c",
        ("fl" | "st" | "gf" | "gs", "opacity") => "o",
        ("st" | "gs", "width") => "w",
        ("gf" | "gs", "start") => "s",
        ("gf" | "gs", "end") => "e",
        ("gf" | "gs", "highlight") => "h",
        ("gf" | "gs", "angle") => "a",
        ("tm", "start") => "s",
        ("tm", "end") => "e",
        ("tm", "offset") => "o",
        ("rd", "radius" | "roundness") => "r",
        ("rp", "copies") => "c",
        ("rp", "offset") => "o",
        _ => "",
    };
    let r = if !generic.is_empty() { generic } else { t };
    (!r.is_empty()).then_some(r)
}

/// Resolve a property to the pointer of its `{a,k}` object.
///
/// `shape` is a shape path ("0/1"), `mask:<i>`, or `effect:<i>`; omitted
/// means the layer itself (transform, time remap).
pub fn prop_ptr(cx: &Cx, r: &LayerRef, shape: Option<&str>, property: &str) -> OpResult<String> {
    let lp = r.ptr();
    match shape {
        Some(s) if s.starts_with("mask:") => {
            let i = &s[5..];
            let key = match property {
                "path" | "pt" => "pt",
                "opacity" | "o" => "o",
                "expansion" | "x" => "x",
                _ => return err(format!("mask property must be path, opacity or expansion, not '{property}'")),
            };
            Ok(format!("{lp}/masksProperties/{i}/{key}"))
        }
        Some(s) if s.starts_with("effect:") => {
            let i = &s[7..];
            let vals = cx.get(&format!("{lp}/ef/{i}/ef")).and_then(Value::as_array).cloned().ok_or_else(|| OpError(format!("no effect {i}")))?;
            let j = property.parse::<usize>().ok().or_else(|| {
                vals.iter().position(|v| v.get("nm").and_then(Value::as_str).map(|n| n.eq_ignore_ascii_case(property) || n.to_lowercase().replace(' ', "_") == property).unwrap_or(false))
            });
            match j {
                Some(j) if j < vals.len() => Ok(format!("{lp}/ef/{i}/ef/{j}/v")),
                _ => err(format!("effect {i} has no parameter '{property}'")),
            }
        }
        Some(s) if !s.is_empty() => {
            let sp = shape_ptr(&lp, s);
            let item = cx.get(&sp).ok_or_else(|| OpError(format!("shape '{s}' not found in layer {}", r.id)))?;
            let kind = item.get("ty").and_then(Value::as_str).unwrap_or("").to_string();
            if kind == "gr" {
                let tp = transform_ptr(cx, &sp, "gr")?;
                let key = friendly("tr", property).unwrap_or(property);
                return Ok(format!("{tp}/{key}"));
            }
            if kind == "rp" && matches!(property, "position" | "scale" | "rotation" | "anchor" | "start_opacity" | "end_opacity") {
                let key = match property {
                    "start_opacity" => "so",
                    "end_opacity" => "eo",
                    p => friendly("tr", p).unwrap_or(p),
                };
                return Ok(format!("{sp}/tr/{key}"));
            }
            if (kind == "gf" || kind == "gs") && property == "colors" {
                return Ok(format!("{sp}/g/k"));
            }
            let key = friendly(&kind, property).unwrap_or(property);
            Ok(format!("{sp}/{key}"))
        }
        _ => {
            if property == "time_remap" {
                return Ok(format!("{lp}/tm"));
            }
            if let ("position_x" | "position_y", Some(p)) = (property, cx.get(&format!("{lp}/ks/p"))) {
                if p.get("s").and_then(Value::as_bool) == Some(true) || p.get("s").and_then(Value::as_i64) == Some(1) {
                    return Ok(format!("{lp}/ks/p/{}", if property == "position_x" { "x" } else { "y" }));
                }
            }
            let key = friendly("layer", property).unwrap_or(property);
            Ok(format!("{lp}/ks/{key}"))
        }
    }
}

// ------------------------------------------------------------------ values

/// Coerce a user value for property key `key` into a numeric vector.
pub fn coerce(key: &str, v: &Value, existing_dims: usize) -> OpResult<Vec<f64>> {
    if key == "c" || key.ends_with("/c") || (v.is_string()) {
        let c = parse_color(v)?;
        return Ok(c.to_vec());
    }
    let mut out: Vec<f64> = match v {
        Value::Number(n) => vec![n.as_f64().unwrap_or(0.0)],
        Value::Array(a) => a.iter().map(|x| x.as_f64().ok_or_else(|| OpError("values must be numbers".into()))).collect::<OpResult<_>>()?,
        _ => return err("value must be a number or an array of numbers"),
    };
    // Lottie transforms conventionally carry a 3rd component.
    if existing_dims == 3 && out.len() == 2 {
        out.push(if key == "s" { 100.0 } else { 0.0 });
    }
    if existing_dims == 0 && out.len() == 2 && matches!(key, "p" | "a") {
        out.push(0.0);
    }
    if existing_dims == 0 && out.len() == 2 && key == "s" {
        out.push(100.0);
    }
    Ok(out)
}

fn last_key(ptr: &str) -> &str {
    ptr.rsplit('/').next().unwrap_or("")
}

fn is_bezier_prop(ptr: &str) -> bool {
    matches!(last_key(ptr), "ks" | "pt") && !ptr.ends_with("/ks/ks")
}

pub fn is_animated(p: &Value) -> bool {
    p.get("k").and_then(Value::as_array).and_then(|a| a.first()).map(|f| f.is_object() && f.get("t").is_some()).unwrap_or(false)
}

/// Evaluate the property at `ptr` at `frame` as numbers (empty if missing).
pub fn read_value(cx: &Cx, ptr: &str, frame: f64) -> Vec<f64> {
    cx.get(ptr).and_then(|p| model::prop(Some(p))).map(|p| anim::eval(&p, frame)).unwrap_or_default()
}

fn static_k(existing: Option<&Value>, vals: &[f64]) -> Value {
    let scalar = existing.and_then(|p| p.get("k")).map(Value::is_number).unwrap_or(false);
    if vals.len() == 1 && (scalar || existing.is_none()) {
        json!(vals[0])
    } else {
        json!(vals)
    }
}

/// Convert legacy (`e` end-value) keyframes to the modern form.
fn modernize(kfs: &mut [Value]) {
    for n in 0..kfs.len() {
        if let Some(e) = kfs[n].get("e").cloned() {
            if let Some(next) = kfs.get_mut(n + 1) {
                if next.get("s").is_none() {
                    next["s"] = e;
                }
            }
            if let Some(o) = kfs[n].as_object_mut() {
                o.remove("e");
            }
        }
    }
}

/// Write a value: static properties are replaced; animated ones get a keyframe at `frame`.
pub fn write_value(cx: &mut Cx, ptr: &str, value: &Value, frame: Option<f64>) -> OpResult<()> {
    let existing = cx.get(ptr).cloned();
    if existing.as_ref().map(is_animated).unwrap_or(false) {
        return upsert_keyframe(cx, ptr, frame.unwrap_or(0.0), value, None);
    }
    if is_bezier_prop(ptr) {
        let mut p = existing.unwrap_or_else(|| json!({"a": 0}));
        p["a"] = json!(0);
        p["k"] = bezier_value(value)?;
        return cx.set(ptr, p);
    }
    let dims = existing.as_ref().and_then(|p| p.get("k")).and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    let vals = coerce(last_key(ptr), value, dims)?;
    let mut p = existing.clone().unwrap_or_else(|| json!({"a": 0}));
    p["a"] = json!(0);
    p["k"] = static_k(existing.as_ref(), &vals);
    cx.set(ptr, p)
}

pub fn bezier_value(v: &Value) -> OpResult<Value> {
    let o = v.as_object().ok_or_else(|| OpError("path must be {v:[[x,y]...], i?, o?, c?}".into()))?;
    let pts = o.get("v").and_then(Value::as_array).ok_or_else(|| OpError("path needs 'v' vertices".into()))?;
    let n = pts.len();
    let tangents = |k: &str| -> Value { o.get(k).cloned().unwrap_or_else(|| json!(vec![json!([0, 0]); n])) };
    Ok(json!({"c": o.get("c").and_then(Value::as_bool).unwrap_or(true), "v": pts, "i": tangents("i"), "o": tangents("o")}))
}

/// Insert or replace the keyframe at `frame` (converting a static property).
pub fn upsert_keyframe(cx: &mut Cx, ptr: &str, frame: f64, value: &Value, easing: Option<Option<[f64; 4]>>) -> OpResult<()> {
    let existing = cx.get(ptr).cloned().unwrap_or_else(|| json!({"a": 0, "k": 0}));
    let bez = is_bezier_prop(ptr);
    let dims = if is_animated(&existing) {
        existing["k"][0]["s"].as_array().map(Vec::len).unwrap_or(0)
    } else {
        existing.get("k").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
    };
    let s = if bez { json!([bezier_value(value)?]) } else { json!(coerce(last_key(ptr), value, dims)?) };
    let mut kfs: Vec<Value> = if is_animated(&existing) { existing["k"].as_array().cloned().unwrap_or_default() } else { vec![] };
    modernize(&mut kfs);
    let ease = easing.unwrap_or(Some(DEFAULT_EASE));
    let mut kf = json!({"t": frame, "s": s});
    match ease {
        Some(b) => {
            let (o, i) = ease_json(b);
            kf["o"] = o;
            kf["i"] = i;
        }
        None => kf["h"] = json!(1),
    }
    match kfs.iter().position(|k| (k.get("t").and_then(Value::as_f64).unwrap_or(0.0) - frame).abs() < 1e-6) {
        Some(i) => {
            kfs[i]["s"] = kf["s"].clone();
            if easing.is_some() {
                kfs[i] = kf;
            }
        }
        None => {
            let at = kfs.iter().position(|k| k.get("t").and_then(Value::as_f64).unwrap_or(0.0) > frame).unwrap_or(kfs.len());
            kfs.insert(at, kf);
        }
    }
    let mut p = existing;
    p["a"] = json!(1);
    p["k"] = Value::Array(kfs);
    cx.set(ptr, p)
}

pub fn frame_arg(a: &Value) -> Option<f64> {
    a.get("frame").and_then(Value::as_f64)
}

/// All animated property pointers within `v` (relative paths from `base`).
pub fn animated_props(v: &Value, base: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if is_animated(v) {
                out.push(base.to_string());
                return;
            }
            for (k, x) in o {
                animated_props(x, &format!("{base}/{}", super::escape(k)), out);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                animated_props(x, &format!("{base}/{i}"), out);
            }
        }
        _ => {}
    }
}

/// Standard static transform block.
pub fn default_ks(p: [f64; 2]) -> Value {
    json!({
        "o": {"a": 0, "k": 100},
        "r": {"a": 0, "k": 0},
        "p": {"a": 0, "k": [p[0], p[1], 0.0]},
        "a": {"a": 0, "k": [0.0, 0.0, 0.0]},
        "s": {"a": 0, "k": [100.0, 100.0, 100.0]}
    })
}

pub fn static_prop(v: Value) -> Value {
    json!({"a": 0, "k": v})
}

/// Scene (canvas) size and time range of the active animation.
pub fn scene(cx: &Cx) -> (f64, f64, f64, f64, f64) {
    let a = cx.anim();
    let g = |k: &str, d: f64| a.get(k).and_then(Value::as_f64).unwrap_or(d);
    (g("w", 512.0), g("h", 512.0), g("fr", 30.0), g("ip", 0.0), g("op", 60.0))
}
