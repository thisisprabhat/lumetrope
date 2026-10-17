//! The project root document: one JSON value holding every animation, theme,
//! state machine and embedded font, so a single undo history covers it all.
//!
//! ```json
//! { "version": 1, "active": "main",
//!   "animations": { "main": <lottie> }, "themes": {}, "stateMachines": {},
//!   "fonts": {}, "settings": {} }
//! ```

use serde_json::{json, Map, Value};

pub const EDITOR_META_KEY: &str = "lmeta";

pub fn is_project(v: &Value) -> bool {
    v.get("animations").map(Value::is_object).unwrap_or(false) && v.get("version").is_some()
}

pub fn is_lottie(v: &Value) -> bool {
    v.is_object() && (v.get("layers").is_some() || (v.get("w").is_some() && v.get("h").is_some()))
}

pub fn from_lottie(anim: Value) -> Value {
    json!({
        "version": 1,
        "active": "main",
        "animations": { "main": anim },
        "themes": {},
        "stateMachines": {},
        "fonts": {},
        "settings": {}
    })
}

pub fn blank_lottie(w: f64, h: f64, fr: f64, frames: f64) -> Value {
    json!({
        "v": "5.12.0", "fr": fr, "ip": 0, "op": frames, "w": w, "h": h, "nm": "Untitled",
        "ddd": 0, "assets": [], "layers": [], "markers": [],
        "meta": { "g": "Lumetrope" }
    })
}

/// Fill in sections a hand-edited or older project may lack.
pub fn normalize(mut p: Value) -> Result<Value, String> {
    if !is_project(&p) {
        return Err("not a Lumetrope project".into());
    }
    let o = p.as_object_mut().unwrap();
    for k in ["themes", "stateMachines", "fonts", "settings"] {
        if !o.get(k).map(Value::is_object).unwrap_or(false) {
            o.insert(k.into(), Value::Object(Map::new()));
        }
    }
    let first = o["animations"].as_object().and_then(|a| a.keys().next().cloned());
    let Some(first) = first else { return Err("project has no animations".into()) };
    let active_ok = o.get("active").and_then(Value::as_str).map(|a| o["animations"].get(a).is_some()).unwrap_or(false);
    if !active_ok {
        o.insert("active".into(), Value::String(first));
    }
    Ok(p)
}

pub fn active_id(p: &Value) -> String {
    p.get("active").and_then(Value::as_str).unwrap_or("main").to_string()
}

pub fn active_anim(p: &Value) -> &Value {
    &p["animations"][active_id(p).as_str()]
}

pub fn active_anim_mut(p: &mut Value) -> &mut Value {
    let id = active_id(p);
    &mut p["animations"][id.as_str()]
}

/// Remove editor-only metadata (any `lmeta` key) recursively.
pub fn strip_editor_meta(v: &mut Value) {
    match v {
        Value::Object(o) => {
            o.remove(EDITOR_META_KEY);
            for (_, x) in o.iter_mut() {
                strip_editor_meta(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip_editor_meta),
        _ => {}
    }
}

/// Build a project from file bytes. Recognises Lumetrope project JSON and Lottie JSON;
/// other formats are handled by their own modules.
pub fn from_json_bytes(bytes: &[u8]) -> Result<Value, String> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("invalid JSON: {e}"))?;
    if is_project(&v) {
        return normalize(v);
    }
    if is_lottie(&v) {
        return Ok(from_lottie(v));
    }
    Err("JSON is neither a Lottie animation nor a Lumetrope project".into())
}
