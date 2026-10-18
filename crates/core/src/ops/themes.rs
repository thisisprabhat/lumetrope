//! Slots, themes and segments (markers).

use serde_json::{json, Map, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, parse_color};
use super::{arg_f64, arg_str, err, escape, opt_bool, opt_f64, opt_str, Cx, OpError, OpResult};

fn prop_target() -> Value {
    json!({"layer": {"type": "string"}, "shape": {"type": "string"}, "property": {"type": "string"}})
}

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_slots",
            description: "List slots (themeable properties) of the active animation with type, current value and how many properties are bound.",
            schema: || obj(json!({}), &[]),
            mutates: false,
            handler: list_slots,
        },
        ToolDef {
            name: "create_slot",
            description: "Expose a property as a themeable slot (dotLottie themes override slots). Binds the property and records its current value as the slot default.",
            schema: || {
                let mut p = prop_target();
                p["slot_id"] = json!({"type": "string"});
                obj(p, &["layer", "property"])
            },
            mutates: true,
            handler: create_slot,
        },
        ToolDef {
            name: "unlink_slot",
            description: "Unbind a property from its slot (the slot is deleted when nothing else uses it).",
            schema: || obj(prop_target(), &["layer", "property"]),
            mutates: true,
            handler: unlink_slot,
        },
        ToolDef {
            name: "update_slot_value",
            description: "Change a slot's default value; every bound property updates.",
            schema: || obj(json!({"slot_id": {"type": "string"}, "value": {}}), &["slot_id", "value"]),
            mutates: true,
            handler: update_slot_value,
        },
        ToolDef {
            name: "list_themes",
            description: "List project themes with their rule counts.",
            schema: || obj(json!({}), &[]),
            mutates: false,
            handler: list_themes,
        },
        ToolDef {
            name: "add_theme",
            description: "Create a theme (a set of slot overrides exported to the dotLottie t/ folder).",
            schema: || obj(json!({"id": {"type": "string"}, "name": {"type": "string"}}), &[]),
            mutates: true,
            handler: add_theme,
        },
        ToolDef {
            name: "update_theme_rule",
            description: "Set (or with remove=true delete) a theme's override for one slot. Give 'value', or 'keyframes' [{frame, value}] for an animated override.",
            schema: || obj(json!({"theme": {"type": "string"}, "slot_id": {"type": "string"}, "value": {}, "keyframes": {"type": "array"}, "type": {"type": "string"}, "remove": {"type": "boolean"}}), &["theme", "slot_id"]),
            mutates: true,
            handler: update_theme_rule,
        },
        ToolDef {
            name: "remove_theme",
            description: "Delete a theme.",
            schema: || obj(json!({"theme": {"type": "string"}}), &["theme"]),
            mutates: true,
            handler: remove_theme,
        },
        ToolDef {
            name: "select_theme",
            description: "Preview the animation with a theme applied (null for no theme).",
            schema: || obj(json!({"theme": {"type": ["string", "null"]}}), &["theme"]),
            mutates: true,
            handler: select_theme,
        },
        ToolDef {
            name: "list_segments",
            description: "List named segments (markers) with start and end frames. State machine playback states play segments by name.",
            schema: || obj(json!({}), &[]),
            mutates: false,
            handler: list_segments,
        },
        ToolDef {
            name: "add_segment",
            description: "Add a named segment (marker) from 'start' to 'end' frame.",
            schema: || obj(json!({"name": {"type": "string"}, "start": {"type": "number"}, "end": {"type": "number"}}), &["name", "start", "end"]),
            mutates: true,
            handler: add_segment,
        },
        ToolDef {
            name: "update_segment",
            description: "Rename or retime a segment.",
            schema: || obj(json!({"name": {"type": "string"}, "new_name": {"type": "string"}, "start": {"type": "number"}, "end": {"type": "number"}}), &["name"]),
            mutates: true,
            handler: update_segment,
        },
        ToolDef {
            name: "remove_segment",
            description: "Delete a segment.",
            schema: || obj(json!({"name": {"type": "string"}}), &["name"]),
            mutates: true,
            handler: remove_segment,
        },
    ]
}

// ------------------------------------------------------------------ slots

/// dotLottie theme rule type for a property pointer.
pub fn slot_type(ptr: &str) -> &'static str {
    let key = ptr.rsplit('/').next().unwrap_or("");
    if ptr.ends_with("/g/k") {
        return "Gradient";
    }
    match key {
        "c" | "sc" | "fc" => "Color",
        "p" | "a" => "Position",
        "s" | "e" | "sz" => "Vector",
        _ => "Scalar",
    }
}

/// Pointers of every property bound to `sid` in the active animation.
fn bound(cx: &Cx, sid: &str) -> Vec<String> {
    fn walk(v: &Value, base: &str, sid: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                if o.get("sid").and_then(Value::as_str) == Some(sid) && o.contains_key("k") {
                    out.push(base.to_string());
                }
                for (k, x) in o {
                    if k != "slots" {
                        walk(x, &format!("{base}/{}", escape(k)), sid, out);
                    }
                }
            }
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    walk(x, &format!("{base}/{i}"), sid, out);
                }
            }
            _ => {}
        }
    }
    let mut out = vec![];
    let base = cx.anim_ptr();
    if let Some(o) = cx.anim().as_object() {
        for (k, x) in o {
            if k != "slots" {
                walk(x, &format!("{base}/{}", escape(k)), sid, &mut out);
            }
        }
    }
    out
}

fn list_slots(cx: &mut Cx, _a: &Value) -> OpResult {
    let slots = cx.anim().get("slots").and_then(Value::as_object).cloned().unwrap_or_default();
    Ok(Value::Array(
        slots
            .iter()
            .map(|(id, s)| {
                let b = bound(cx, id);
                let ty = b.first().map(|p| slot_type(p)).unwrap_or("Scalar");
                json!({"id": id, "type": ty, "value": s.pointer("/p/k").cloned().unwrap_or(Value::Null), "bound": b.len()})
            })
            .collect(),
    ))
}

fn create_slot(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let property = arg_str(a, "property")?;
    let ptr = util::prop_ptr(cx, &r, opt_str(a, "shape"), property)?;
    let mut prop = cx.get(&ptr).cloned().ok_or_else(|| OpError(format!("property '{property}' not found")))?;
    let slots = cx.anim().get("slots").and_then(Value::as_object).cloned().unwrap_or_default();
    let sid = match opt_str(a, "slot_id") {
        Some(s) => s.to_string(),
        None => {
            let lname = cx.layer_value(&r)["nm"].as_str().unwrap_or("layer").to_lowercase().replace(|c: char| !c.is_alphanumeric(), "_");
            let base = format!("{lname}_{property}");
            (0..).map(|n| if n == 0 { base.clone() } else { format!("{base}_{n}") }).find(|c| !slots.contains_key(c)).unwrap()
        }
    };
    if sid.is_empty() || !sid.chars().all(|c| c.is_ascii_alphanumeric() || "._- ".contains(c)) {
        return err("slot_id may contain only letters, digits, '.', '_', '-' and spaces");
    }
    if let Some(o) = prop.as_object_mut() {
        o.remove("sid");
    }
    let sp = format!("{}/slots/{}", cx.anim_ptr(), escape(&sid));
    if cx.get(&sp).is_none() {
        cx.set(&sp, json!({"p": prop}))?;
    }
    cx.set(&format!("{ptr}/sid"), json!(sid))?;
    Ok(json!({"slot_id": sid, "type": slot_type(&ptr)}))
}

fn unlink_slot(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let ptr = util::prop_ptr(cx, &r, opt_str(a, "shape"), arg_str(a, "property")?)?;
    let Some(sid) = cx.get(&format!("{ptr}/sid")).and_then(Value::as_str).map(str::to_string) else {
        return err("property is not bound to a slot");
    };
    cx.delete(&format!("{ptr}/sid"))?;
    if bound(cx, &sid).is_empty() {
        let ap = cx.anim_ptr();
        cx.delete(&format!("{ap}/slots/{}", escape(&sid)))?;
        if cx.anim().get("slots").and_then(Value::as_object).map(Map::is_empty).unwrap_or(false) {
            cx.delete(&format!("{ap}/slots"))?;
        }
    }
    Ok(json!({"ok": true}))
}

fn update_slot_value(cx: &mut Cx, a: &Value) -> OpResult {
    let sid = arg_str(a, "slot_id")?;
    let sp = format!("{}/slots/{}/p", cx.anim_ptr(), escape(sid));
    if cx.get(&sp).is_none() {
        return err(format!("no slot '{sid}'"));
    }
    let v = a.get("value").ok_or_else(|| OpError("missing 'value'".into()))?;
    let targets = bound(cx, sid);
    let colorish = targets.first().map(|p| slot_type(p) == "Color").unwrap_or(false);
    let v = if colorish { json!(parse_color(v)?) } else { v.clone() };
    // Slot values are static overrides; replace any animation on the bound properties.
    for p in std::iter::once(sp.clone()).chain(targets) {
        let mut prop = cx.get(&p).cloned().unwrap_or_else(|| json!({}));
        let probe = json!({"a": 0, "k": prop.get("k").filter(|_| !util::is_animated(&prop)).cloned().unwrap_or(json!([]))});
        prop["a"] = json!(0);
        prop["k"] = probe["k"].clone();
        cx.set(&p, prop)?;
        util::write_value(cx, &p, &v, None)?;
    }
    Ok(json!({"ok": true}))
}

// ------------------------------------------------------------------ themes

fn list_themes(cx: &mut Cx, _a: &Value) -> OpResult {
    let t = cx.root.get("themes").and_then(Value::as_object).cloned().unwrap_or_default();
    Ok(Value::Array(t.iter().map(|(id, th)| json!({"id": id, "name": th.get("name"), "rules": th.get("rules").and_then(Value::as_array).map(Vec::len).unwrap_or(0)})).collect()))
}

fn add_theme(cx: &mut Cx, a: &Value) -> OpResult {
    let existing = cx.root.get("themes").and_then(Value::as_object).cloned().unwrap_or_default();
    let name = opt_str(a, "name").unwrap_or("Theme");
    let id = match opt_str(a, "id") {
        Some(i) => i.to_string(),
        None => {
            let base: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            (0..).map(|n| if n == 0 { base.clone() } else { format!("{base}_{n}") }).find(|c| !existing.contains_key(c)).unwrap()
        }
    };
    if existing.contains_key(&id) {
        return err(format!("theme '{id}' already exists"));
    }
    cx.set(&format!("/themes/{}", escape(&id)), json!({"name": name, "rules": []}))?;
    Ok(json!({"theme": id}))
}

fn theme_ptr(cx: &Cx, id: &str) -> OpResult<String> {
    let p = format!("/themes/{}", escape(id));
    if cx.get(&p).is_none() {
        return err(format!("no theme '{id}'"));
    }
    Ok(p)
}

fn update_theme_rule(cx: &mut Cx, a: &Value) -> OpResult {
    let tp = theme_ptr(cx, arg_str(a, "theme")?)?;
    let sid = arg_str(a, "slot_id")?;
    let rules = cx.get(&format!("{tp}/rules")).and_then(Value::as_array).cloned().unwrap_or_default();
    let existing = rules.iter().position(|r| r.get("id").and_then(Value::as_str) == Some(sid));
    if opt_bool(a, "remove").unwrap_or(false) {
        if let Some(i) = existing {
            cx.remove(&format!("{tp}/rules"), i)?;
        }
        return Ok(json!({"ok": true}));
    }
    let ty = match opt_str(a, "type") {
        Some(t) => t.to_string(),
        None => bound(cx, sid).first().map(|p| slot_type(p).to_string()).unwrap_or_else(|| "Scalar".into()),
    };
    let conv = |v: &Value| -> OpResult<Value> { if ty == "Color" { Ok(json!(parse_color(v)?)) } else { Ok(v.clone()) } };
    let mut rule = json!({"id": sid, "type": ty});
    if let Some(kfs) = a.get("keyframes").and_then(Value::as_array) {
        let mut out = vec![];
        for k in kfs {
            let frame = k.get("frame").and_then(Value::as_f64).ok_or_else(|| OpError("keyframes need 'frame'".into()))?;
            out.push(json!({"frame": frame, "value": conv(k.get("value").unwrap_or(&Value::Null))?}));
        }
        rule["keyframes"] = Value::Array(out);
    } else {
        rule["value"] = conv(a.get("value").ok_or_else(|| OpError("give 'value' or 'keyframes'".into()))?)?;
    }
    match existing {
        Some(i) => cx.set(&format!("{tp}/rules/{i}"), rule)?,
        None => {
            cx.insert(&format!("{tp}/rules"), rules.len(), rule)?;
        }
    }
    Ok(json!({"ok": true}))
}

fn remove_theme(cx: &mut Cx, a: &Value) -> OpResult {
    let id = arg_str(a, "theme")?;
    let tp = theme_ptr(cx, id)?;
    cx.delete(&tp)?;
    if cx.get("/settings/preview_theme").and_then(Value::as_str) == Some(id) {
        cx.delete("/settings/preview_theme")?;
    }
    Ok(json!({"ok": true}))
}

fn select_theme(cx: &mut Cx, a: &Value) -> OpResult {
    match a.get("theme").and_then(Value::as_str) {
        Some(id) => {
            theme_ptr(cx, id)?;
            cx.set("/settings/preview_theme", json!(id))?;
        }
        None => cx.delete("/settings/preview_theme")?,
    }
    Ok(json!({"ok": true}))
}

// ------------------------------------------------------------------ segments

fn markers(cx: &Cx) -> Vec<Value> {
    cx.anim().get("markers").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn marker_index(cx: &Cx, name: &str) -> OpResult<usize> {
    markers(cx).iter().position(|m| m.get("cm").and_then(Value::as_str) == Some(name)).ok_or_else(|| OpError(format!("no segment '{name}'")))
}

fn list_segments(cx: &mut Cx, _a: &Value) -> OpResult {
    Ok(Value::Array(
        markers(cx)
            .iter()
            .map(|m| {
                let t = m.get("tm").and_then(Value::as_f64).unwrap_or(0.0);
                json!({"name": m.get("cm"), "start": t, "end": t + m.get("dr").and_then(Value::as_f64).unwrap_or(0.0)})
            })
            .collect(),
    ))
}

fn add_segment(cx: &mut Cx, a: &Value) -> OpResult {
    let name = arg_str(a, "name")?;
    let (s, e) = (arg_f64(a, "start")?, arg_f64(a, "end")?);
    if e <= s {
        return err("end must be after start");
    }
    if marker_index(cx, name).is_ok() {
        return err(format!("segment '{name}' already exists"));
    }
    let mp = format!("{}/markers", cx.anim_ptr());
    let n = markers(cx).len();
    cx.insert(&mp, n, json!({"cm": name, "tm": s, "dr": e - s}))?;
    Ok(json!({"ok": true}))
}

fn update_segment(cx: &mut Cx, a: &Value) -> OpResult {
    let i = marker_index(cx, arg_str(a, "name")?)?;
    let m = markers(cx)[i].clone();
    let s0 = m.get("tm").and_then(Value::as_f64).unwrap_or(0.0);
    let e0 = s0 + m.get("dr").and_then(Value::as_f64).unwrap_or(0.0);
    let (s, e) = (opt_f64(a, "start").unwrap_or(s0), opt_f64(a, "end").unwrap_or(e0));
    if e <= s {
        return err("end must be after start");
    }
    let name = opt_str(a, "new_name").unwrap_or(m["cm"].as_str().unwrap_or(""));
    if opt_str(a, "new_name").is_some() && marker_index(cx, name).is_ok() {
        return err(format!("segment '{name}' already exists"));
    }
    let mp = format!("{}/markers/{i}", cx.anim_ptr());
    cx.set(&mp, json!({"cm": name, "tm": s, "dr": e - s}))?;
    Ok(json!({"ok": true}))
}

fn remove_segment(cx: &mut Cx, a: &Value) -> OpResult {
    let i = marker_index(cx, arg_str(a, "name")?)?;
    let mp = format!("{}/markers", cx.anim_ptr());
    cx.remove(&mp, i)?;
    Ok(json!({"ok": true}))
}
