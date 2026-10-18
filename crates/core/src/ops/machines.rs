//! dotLottie state machine authoring tools. Machines live in the project at
//! `/stateMachines/<id>` as spec JSON plus an editor-only `lmeta` (name, node layout).

use serde_json::{json, Map, Value};

use super::catalog::{obj, ToolDef};
use super::util::parse_easing;
use super::{arg_str, err, escape, opt_f64, opt_str, Cx, OpError, OpResult};

pub const INTERACTIONS: &[&str] = &["Click", "PointerDown", "PointerUp", "PointerEnter", "PointerExit", "PointerMove", "OnComplete", "OnLoopComplete"];
pub const ACTIONS: &[&str] = &[
    "OpenUrl", "SetTheme", "Increment", "Decrement", "Toggle", "SetBoolean", "SetString", "SetNumeric", "Fire", "Reset", "SetFrame", "SetProgress", "FireCustomEvent",
];

fn m() -> Value {
    json!({"type": "string", "description": "State machine id"})
}

fn state_fields() -> Value {
    json!({
        "machine": m(), "name": {"type": "string"},
        "animation": {"type": "string", "description": "Animation id (empty = the active animation)"},
        "segment": {"type": ["string", "null"], "description": "Segment (marker) name to play"},
        "loop": {"type": ["boolean", "null"]}, "loop_count": {"type": ["integer", "null"]}, "autoplay": {"type": ["boolean", "null"]},
        "final": {"type": ["boolean", "null"]}, "mode": {"type": ["string", "null"], "enum": ["forward", "reverse", "bounce", "reverse_bounce", null]},
        "speed": {"type": ["number", "null"]}, "background": {"type": ["string", "null"]},
        "entry_actions": {"type": ["array", "null"]}, "exit_actions": {"type": ["array", "null"]},
        "position": {"type": "array", "description": "Node position in the graph editor"}
    })
}

fn transition_fields() -> Value {
    json!({
        "machine": m(), "from": {"type": "string"}, "to": {"type": "string"}, "index": {"type": "integer"},
        "type": {"type": "string", "enum": ["instant", "tweened"]}, "duration": {"type": "number", "description": "Seconds (tweened)"},
        "easing": {"description": "Preset name or [x1,y1,x2,y2] (tweened)"},
        "guards": {"type": "array", "description": "[{input, condition: equal|not_equal|greater_than|greater_than_or_equal|less_than|less_than_or_equal, value (or \"$otherInput\")}] — all must pass; event inputs need only {input}"}
    })
}

fn interaction_fields() -> Value {
    json!({
        "machine": m(), "index": {"type": "integer"},
        "type": {"type": "string", "enum": INTERACTIONS},
        "layer": {"type": ["string", "null"], "description": "Layer name the pointer interaction is limited to"},
        "state": {"type": "string", "description": "State name (OnComplete / OnLoopComplete)"},
        "actions": {"type": "array", "description": "[{type: toggle|increment|decrement|set_boolean|set_numeric|set_string|fire|reset|set_frame|set_progress|set_theme|open_url|fire_custom_event, input, value, url, target}]"}
    })
}

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef { name: "list_state_machines", description: "List the project's state machines.", schema: || obj(json!({}), &[]), mutates: false, handler: list },
        ToolDef { name: "add_state_machine", description: "Create an empty state machine. Returns its id.", schema: || obj(json!({"id": {"type": "string"}, "name": {"type": "string"}}), &[]), mutates: true, handler: add_machine },
        ToolDef { name: "remove_state_machine", description: "Delete a state machine.", schema: || obj(json!({"machine": m()}), &["machine"]), mutates: true, handler: remove_machine },
        ToolDef { name: "rename_state_machine", description: "Rename a state machine.", schema: || obj(json!({"machine": m(), "name": {"type": "string"}}), &["machine", "name"]), mutates: true, handler: rename_machine },
        ToolDef { name: "read_state_machine", description: "Return a state machine as dotLottie spec JSON (initial, states, interactions, inputs).", schema: || obj(json!({"machine": m()}), &["machine"]), mutates: false, handler: read_machine },
        ToolDef {
            name: "add_state",
            description: "Add a state: a playback state (plays the animation or a named segment with loop/mode/speed) or the single global state (type 'global') whose transitions are checked from any state. The first playback state becomes initial.",
            schema: || {
                let mut f = state_fields();
                f["type"] = json!({"type": "string", "enum": ["playback", "global"]});
                obj(f, &["machine", "name"])
            },
            mutates: true,
            handler: add_state,
        },
        ToolDef {
            name: "update_state",
            description: "Change a state's settings (null removes an optional field). Renaming updates transitions, interactions and the initial state.",
            schema: || {
                let mut f = state_fields();
                f["new_name"] = json!({"type": "string"});
                obj(f, &["machine", "name"])
            },
            mutates: true,
            handler: update_state,
        },
        ToolDef { name: "remove_state", description: "Delete a state and every transition/interaction that refers to it.", schema: || obj(json!({"machine": m(), "name": {"type": "string"}}), &["machine", "name"]), mutates: true, handler: remove_state },
        ToolDef { name: "set_initial_state", description: "Choose the state the machine starts in.", schema: || obj(json!({"machine": m(), "name": {"type": "string"}}), &["machine", "name"]), mutates: true, handler: set_initial },
        ToolDef { name: "add_transition", description: "Add a transition from one state to another, optionally guarded by input conditions and tweened over a duration.", schema: || obj(transition_fields(), &["machine", "from", "to"]), mutates: true, handler: add_transition },
        ToolDef { name: "update_transition", description: "Update the transition at 'index' in state 'from'.", schema: || obj(transition_fields(), &["machine", "from", "index"]), mutates: true, handler: update_transition },
        ToolDef { name: "remove_transition", description: "Delete the transition at 'index' in state 'from'.", schema: || obj(json!({"machine": m(), "from": {"type": "string"}, "index": {"type": "integer"}}), &["machine", "from", "index"]), mutates: true, handler: remove_transition },
        ToolDef {
            name: "add_input",
            description: "Add an input variable: boolean, numeric, string or event (events have no value).",
            schema: || obj(json!({"machine": m(), "name": {"type": "string"}, "type": {"type": "string", "enum": ["boolean", "numeric", "string", "event"]}, "value": {}}), &["machine", "name", "type"]),
            mutates: true,
            handler: add_input,
        },
        ToolDef {
            name: "update_input",
            description: "Rename an input or change its type/default value. Renames update guards and actions.",
            schema: || obj(json!({"machine": m(), "name": {"type": "string"}, "new_name": {"type": "string"}, "type": {"type": "string"}, "value": {}}), &["machine", "name"]),
            mutates: true,
            handler: update_input,
        },
        ToolDef { name: "remove_input", description: "Delete an input along with guards and actions that use it.", schema: || obj(json!({"machine": m(), "name": {"type": "string"}}), &["machine", "name"]), mutates: true, handler: remove_input },
        ToolDef { name: "add_interaction", description: "Wire a user/playback event (Click, PointerDown/Up/Enter/Exit/Move on an optional layer, OnComplete/OnLoopComplete of a state) to actions.", schema: || obj(interaction_fields(), &["machine", "type", "actions"]), mutates: true, handler: add_interaction },
        ToolDef { name: "update_interaction", description: "Update the interaction at 'index'.", schema: || obj(interaction_fields(), &["machine", "index"]), mutates: true, handler: update_interaction },
        ToolDef { name: "remove_interaction", description: "Delete the interaction at 'index'.", schema: || obj(json!({"machine": m(), "index": {"type": "integer"}}), &["machine", "index"]), mutates: true, handler: remove_interaction },
    ]
}

// ------------------------------------------------------------------ helpers

fn mptr(cx: &Cx, a: &Value) -> OpResult<String> {
    let id = arg_str(a, "machine")?;
    let p = format!("/stateMachines/{}", escape(id));
    if cx.get(&p).is_none() {
        return err(format!("no state machine '{id}'"));
    }
    Ok(p)
}

fn arr(cx: &Cx, p: &str) -> Vec<Value> {
    cx.get(p).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn state_index(cx: &Cx, mp: &str, name: &str) -> OpResult<usize> {
    arr(cx, &format!("{mp}/states")).iter().position(|s| s.get("name").and_then(Value::as_str) == Some(name)).ok_or_else(|| OpError(format!("no state '{name}'")))
}

fn camel(s: &str) -> String {
    s.split('_').map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect()
}

fn input_type(cx: &Cx, mp: &str, name: &str) -> OpResult<String> {
    arr(cx, &format!("{mp}/inputs"))
        .iter()
        .find(|i| i.get("name").and_then(Value::as_str) == Some(name))
        .and_then(|i| i.get("type").and_then(Value::as_str).map(str::to_string))
        .ok_or_else(|| OpError(format!("no input named '{name}'")))
}

fn guards(cx: &Cx, mp: &str, v: &Value) -> OpResult<Vec<Value>> {
    let list = v.as_array().ok_or_else(|| OpError("guards must be an array".into()))?;
    list.iter()
        .map(|g| {
            let name = g.get("input").or_else(|| g.get("inputName")).and_then(Value::as_str).ok_or_else(|| OpError("each guard needs 'input'".into()))?;
            let ty = input_type(cx, mp, name)?;
            if ty == "Event" {
                return Ok(json!({"type": "Event", "inputName": name}));
            }
            let cond = g.get("condition").or_else(|| g.get("conditionType")).and_then(Value::as_str).unwrap_or("equal");
            let cond = if cond.contains('_') || cond.chars().next().map(char::is_lowercase).unwrap_or(false) { camel(cond) } else { cond.to_string() };
            let allowed: &[&str] = if ty == "Numeric" { &["Equal", "NotEqual", "GreaterThan", "GreaterThanOrEqual", "LessThan", "LessThanOrEqual"] } else { &["Equal", "NotEqual"] };
            if !allowed.contains(&cond.as_str()) {
                return err(format!("condition '{cond}' is not valid for a {ty} input"));
            }
            let value = g.get("value").or_else(|| g.get("compareTo")).cloned().ok_or_else(|| OpError(format!("guard on '{name}' needs a value")))?;
            let is_ref = value.as_str().map(|s| s.starts_with('$')).unwrap_or(false);
            let ok_ty = is_ref
                || match ty.as_str() {
                    "Numeric" => value.is_number(),
                    "Boolean" => value.is_boolean(),
                    _ => value.is_string(),
                };
            if !ok_ty {
                return err(format!("guard value for {ty} input '{name}' has the wrong type"));
            }
            Ok(json!({"type": ty, "inputName": name, "conditionType": cond, "compareTo": value}))
        })
        .collect()
}

fn actions(v: &Value) -> OpResult<Vec<Value>> {
    let list = v.as_array().ok_or_else(|| OpError("actions must be an array".into()))?;
    list.iter()
        .map(|a| {
            let o = a.as_object().ok_or_else(|| OpError("each action must be an object".into()))?;
            let t = o.get("type").and_then(Value::as_str).ok_or_else(|| OpError("each action needs 'type'".into()))?;
            let t = if ACTIONS.contains(&t) { t.to_string() } else { camel(t) };
            if !ACTIONS.contains(&t.as_str()) {
                return err(format!("unknown action type '{}' (valid: {})", o["type"].as_str().unwrap_or(""), ACTIONS.join(", ")));
            }
            let mut out = Map::new();
            out.insert("type".into(), json!(t));
            for (k, v) in o {
                match k.as_str() {
                    "type" => {}
                    "input" | "inputName" => {
                        out.insert("inputName".into(), v.clone());
                    }
                    other => {
                        out.insert(other.into(), v.clone());
                    }
                }
            }
            let needs_input = matches!(t.as_str(), "Increment" | "Decrement" | "Toggle" | "SetBoolean" | "SetString" | "SetNumeric" | "Fire" | "Reset");
            if needs_input && !out.contains_key("inputName") {
                return err(format!("{t} needs 'input'"));
            }
            if t == "OpenUrl" && !out.contains_key("url") {
                return err("OpenUrl needs 'url'");
            }
            Ok(Value::Object(out))
        })
        .collect()
}

fn apply_state_fields(s: &mut Value, a: &Value) -> OpResult<()> {
    let o = s.as_object_mut().unwrap();
    let global = o.get("type").and_then(Value::as_str) == Some("GlobalState");
    let mut set = |key: &str, v: Option<Value>| match v {
        Some(Value::Null) => {
            o.remove(key);
        }
        Some(v) => {
            o.insert(key.into(), v);
        }
        None => {}
    };
    if !global {
        set("animation", a.get("animation").cloned());
        set("segment", a.get("segment").cloned());
        set("loop", a.get("loop").cloned());
        set("loopCount", a.get("loop_count").cloned());
        set("autoplay", a.get("autoplay").cloned());
        set("final", a.get("final").cloned());
        set("speed", a.get("speed").cloned());
        let mode = match a.get("mode") {
            Some(Value::String(m)) => Some(json!(match m.as_str() {
                "forward" | "Forward" => "Forward",
                "reverse" | "Reverse" => "Reverse",
                "bounce" | "Bounce" => "Bounce",
                "reverse_bounce" | "ReverseBounce" => "ReverseBounce",
                other => return err(format!("unknown mode '{other}'")),
            })),
            other => other.cloned(),
        };
        set("mode", mode);
        let bg = match a.get("background") {
            Some(Value::String(h)) => {
                let c = super::util::parse_color(&json!(h))?;
                let u = ((c[0] * 255.0) as u32) << 24 | ((c[1] * 255.0) as u32) << 16 | ((c[2] * 255.0) as u32) << 8 | (c[3] * 255.0) as u32;
                Some(json!(u))
            }
            other => other.cloned(),
        };
        set("backgroundColor", bg);
    }
    for (arg, key) in [("entry_actions", "entryActions"), ("exit_actions", "exitActions")] {
        match a.get(arg) {
            Some(Value::Null) => set(key, Some(Value::Null)),
            Some(v) => set(key, Some(Value::Array(actions(v)?))),
            None => {}
        }
    }
    Ok(())
}

fn build_transition(cx: &Cx, mp: &str, a: &Value, base: Option<Value>) -> OpResult<Value> {
    let mut t = base.unwrap_or_else(|| json!({"type": "Transition"}));
    if let Some(to) = opt_str(a, "to") {
        state_index(cx, mp, to)?;
        t["toState"] = json!(to);
    }
    let tweened = match opt_str(a, "type") {
        Some("tweened") | Some("Tweened") => true,
        Some("instant") | Some("Transition") => false,
        Some(o) => return err(format!("transition type must be instant or tweened, not '{o}'")),
        None => t["type"] == "Tweened",
    };
    let o = t.as_object_mut().unwrap();
    if tweened {
        o.insert("type".into(), json!("Tweened"));
        let d = opt_f64(a, "duration").or_else(|| o.get("duration").and_then(Value::as_f64)).unwrap_or(0.5);
        if d <= 0.0 {
            return err("duration must be > 0");
        }
        o.insert("duration".into(), json!(d));
        let e = match a.get("easing") {
            Some(e) => parse_easing(e)?.unwrap_or([0.0, 0.0, 1.0, 1.0]),
            None => o.get("easing").and_then(Value::as_array).map(|v| [0, 1, 2, 3].map(|i| v.get(i).and_then(Value::as_f64).unwrap_or(0.0))).unwrap_or([0.42, 0.0, 0.58, 1.0]),
        };
        o.insert("easing".into(), json!(e));
    } else {
        o.insert("type".into(), json!("Transition"));
        o.remove("duration");
        o.remove("easing");
    }
    match a.get("guards") {
        Some(Value::Null) => {
            o.remove("guards");
        }
        Some(g) => {
            let gs = guards(cx, mp, g)?;
            if gs.is_empty() {
                o.remove("guards");
            } else {
                o.insert("guards".into(), Value::Array(gs));
            }
        }
        None => {}
    }
    Ok(t)
}

fn interaction(a: &Value, base: Option<Value>) -> OpResult<Value> {
    let mut it = base.unwrap_or_else(|| json!({}));
    if let Some(t) = opt_str(a, "type") {
        let t = INTERACTIONS.iter().find(|x| x.eq_ignore_ascii_case(t) || x.eq_ignore_ascii_case(&camel(t))).ok_or_else(|| OpError(format!("unknown interaction type '{t}' (valid: {})", INTERACTIONS.join(", "))))?;
        it["type"] = json!(t);
    }
    let o = it.as_object_mut().unwrap();
    match a.get("layer") {
        Some(Value::Null) => {
            o.remove("layerName");
        }
        Some(Value::String(l)) => {
            o.insert("layerName".into(), json!(l));
        }
        _ => {}
    }
    if let Some(s) = opt_str(a, "state") {
        o.insert("stateName".into(), json!(s));
    }
    if let Some(acts) = a.get("actions") {
        o.insert("actions".into(), Value::Array(actions(acts)?));
    }
    let ty = o.get("type").and_then(Value::as_str).unwrap_or("").to_string();
    if matches!(ty.as_str(), "OnComplete" | "OnLoopComplete") {
        o.remove("layerName");
        if !o.contains_key("stateName") {
            return err(format!("{ty} needs 'state'"));
        }
    } else {
        o.remove("stateName");
    }
    if !o.contains_key("actions") {
        o.insert("actions".into(), json!([]));
    }
    Ok(it)
}

/// Spec JSON without editor metadata.
pub fn clean(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("lmeta");
    }
    v
}

// ------------------------------------------------------------------ handlers

fn list(cx: &mut Cx, _a: &Value) -> OpResult {
    let ms = cx.root.get("stateMachines").and_then(Value::as_object).cloned().unwrap_or_default();
    Ok(Value::Array(
        ms.iter()
            .map(|(id, sm)| {
                json!({"id": id, "name": sm.pointer("/lmeta/name").cloned().unwrap_or(json!(id)), "states": sm.get("states").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
                "inputs": sm.get("inputs").and_then(Value::as_array).map(Vec::len).unwrap_or(0), "initial": sm.get("initial")})
            })
            .collect(),
    ))
}

fn add_machine(cx: &mut Cx, a: &Value) -> OpResult {
    let existing = cx.root.get("stateMachines").and_then(Value::as_object).cloned().unwrap_or_default();
    let n = existing.len() + 1;
    let id = match opt_str(a, "id") {
        Some(i) => i.to_string(),
        None => (n..).map(|k| format!("StateMachine{k}")).find(|c| !existing.contains_key(c)).unwrap(),
    };
    if existing.contains_key(&id) {
        return err(format!("state machine '{id}' already exists"));
    }
    let name = opt_str(a, "name").map(str::to_string).unwrap_or_else(|| id.clone());
    cx.set(&format!("/stateMachines/{}", escape(&id)), json!({"initial": "", "states": [], "interactions": [], "inputs": [], "lmeta": {"name": name, "layout": {}}}))?;
    Ok(json!({"machine": id}))
}

fn remove_machine(cx: &mut Cx, a: &Value) -> OpResult {
    let p = mptr(cx, a)?;
    cx.delete(&p)?;
    Ok(json!({"ok": true}))
}

fn rename_machine(cx: &mut Cx, a: &Value) -> OpResult {
    let p = mptr(cx, a)?;
    cx.set(&format!("{p}/lmeta/name"), json!(arg_str(a, "name")?))?;
    Ok(json!({"ok": true}))
}

fn read_machine(cx: &mut Cx, a: &Value) -> OpResult {
    let p = mptr(cx, a)?;
    Ok(clean(cx.get(&p).unwrap()))
}

fn add_state(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let name = arg_str(a, "name")?;
    if name.is_empty() {
        return err("state name cannot be empty");
    }
    if state_index(cx, &mp, name).is_ok() {
        return err(format!("state '{name}' already exists"));
    }
    let states = arr(cx, &format!("{mp}/states"));
    let global = matches!(opt_str(a, "type"), Some("global") | Some("GlobalState"));
    if global && states.iter().any(|s| s["type"] == "GlobalState") {
        return err("a machine can only have one global state");
    }
    let mut s = if global {
        json!({"name": name, "type": "GlobalState", "transitions": []})
    } else {
        json!({"name": name, "type": "PlaybackState", "animation": "", "transitions": []})
    };
    apply_state_fields(&mut s, a)?;
    cx.insert(&format!("{mp}/states"), states.len(), s)?;
    if !global && cx.get(&format!("{mp}/initial")).and_then(Value::as_str).map(str::is_empty).unwrap_or(true) {
        cx.set(&format!("{mp}/initial"), json!(name))?;
    }
    if let Some(p) = a.get("position").filter(|p| p.is_array()) {
        cx.set(&format!("{mp}/lmeta/layout/{}", escape(name)), p.clone())?;
    }
    Ok(json!({"ok": true}))
}

fn rename_refs(v: &mut Value, old: &str, new: &str) {
    if let Some(states) = v.get_mut("states").and_then(Value::as_array_mut) {
        for s in states {
            if let Some(ts) = s.get_mut("transitions").and_then(Value::as_array_mut) {
                for t in ts {
                    if t["toState"] == old {
                        t["toState"] = json!(new);
                    }
                }
            }
        }
    }
    if let Some(is) = v.get_mut("interactions").and_then(Value::as_array_mut) {
        for i in is {
            if i.get("stateName").and_then(Value::as_str) == Some(old) {
                i["stateName"] = json!(new);
            }
        }
    }
    if v["initial"] == old {
        v["initial"] = json!(new);
    }
    if let Some(l) = v.pointer_mut("/lmeta/layout").and_then(Value::as_object_mut) {
        if let Some(p) = l.remove(old) {
            l.insert(new.into(), p);
        }
    }
}

fn update_state(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let name = arg_str(a, "name")?;
    let i = state_index(cx, &mp, name)?;
    let mut sm = cx.get(&mp).cloned().unwrap();
    let mut s = sm["states"][i].clone();
    apply_state_fields(&mut s, a)?;
    sm["states"][i] = s;
    if let Some(nn) = opt_str(a, "new_name").filter(|n| *n != name) {
        if sm["states"].as_array().unwrap().iter().any(|s| s["name"] == nn) {
            return err(format!("state '{nn}' already exists"));
        }
        sm["states"][i]["name"] = json!(nn);
        rename_refs(&mut sm, name, nn);
    }
    if let Some(p) = a.get("position").filter(|p| p.is_array()) {
        let nm = sm["states"][i]["name"].as_str().unwrap_or(name).to_string();
        sm["lmeta"]["layout"][nm] = p.clone();
    }
    cx.set(&mp, sm)?;
    Ok(json!({"ok": true}))
}

fn remove_state(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let name = arg_str(a, "name")?;
    let i = state_index(cx, &mp, name)?;
    let mut sm = cx.get(&mp).cloned().unwrap();
    sm["states"].as_array_mut().unwrap().remove(i);
    for s in sm["states"].as_array_mut().unwrap() {
        if let Some(ts) = s.get_mut("transitions").and_then(Value::as_array_mut) {
            ts.retain(|t| t["toState"] != name);
        }
    }
    if let Some(is) = sm.get_mut("interactions").and_then(Value::as_array_mut) {
        is.retain(|x| x.get("stateName").and_then(Value::as_str) != Some(name));
    }
    if sm["initial"] == name {
        let next = sm["states"].as_array().unwrap().iter().find(|s| s["type"] == "PlaybackState").map(|s| s["name"].clone()).unwrap_or(json!(""));
        sm["initial"] = next;
    }
    if let Some(l) = sm.pointer_mut("/lmeta/layout").and_then(Value::as_object_mut) {
        l.remove(name);
    }
    cx.set(&mp, sm)?;
    Ok(json!({"ok": true}))
}

fn set_initial(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let name = arg_str(a, "name")?;
    let i = state_index(cx, &mp, name)?;
    if arr(cx, &format!("{mp}/states"))[i]["type"] == "GlobalState" {
        return err("the global state cannot be the initial state");
    }
    cx.set(&format!("{mp}/initial"), json!(name))?;
    Ok(json!({"ok": true}))
}

fn add_transition(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let from = state_index(cx, &mp, arg_str(a, "from")?)?;
    arg_str(a, "to")?;
    let t = build_transition(cx, &mp, a, None)?;
    let tp = format!("{mp}/states/{from}/transitions");
    let n = arr(cx, &tp).len();
    cx.insert(&tp, n, t)?;
    Ok(json!({"index": n}))
}

fn transition_ptr(cx: &Cx, mp: &str, a: &Value) -> OpResult<String> {
    let from = state_index(cx, mp, arg_str(a, "from")?)?;
    let idx = a.get("index").and_then(Value::as_u64).ok_or_else(|| OpError("missing 'index'".into()))? as usize;
    let p = format!("{mp}/states/{from}/transitions/{idx}");
    if cx.get(&p).is_none() {
        return err(format!("no transition {idx} from that state"));
    }
    Ok(p)
}

fn update_transition(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let tp = transition_ptr(cx, &mp, a)?;
    let t = build_transition(cx, &mp, a, cx.get(&tp).cloned())?;
    cx.set(&tp, t)?;
    Ok(json!({"ok": true}))
}

fn remove_transition(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let tp = transition_ptr(cx, &mp, a)?;
    let (parent, idx) = tp.rsplit_once('/').unwrap();
    cx.remove(parent, idx.parse().unwrap())?;
    Ok(json!({"ok": true}))
}

fn input_value(ty: &str, v: Option<&Value>) -> OpResult<Option<Value>> {
    Ok(match ty {
        "Event" => None,
        "Boolean" => Some(json!(v.map(|x| x.as_bool().ok_or_else(|| OpError("boolean input needs true/false".into()))).transpose()?.unwrap_or(false))),
        "Numeric" => Some(json!(v.map(|x| x.as_f64().ok_or_else(|| OpError("numeric input needs a number".into()))).transpose()?.unwrap_or(0.0))),
        _ => Some(json!(v.map(|x| x.as_str().map(str::to_string).ok_or_else(|| OpError("string input needs a string".into()))).transpose()?.unwrap_or_default())),
    })
}

fn parse_input_type(t: &str) -> OpResult<&'static str> {
    match t.to_lowercase().as_str() {
        "boolean" | "bool" => Ok("Boolean"),
        "numeric" | "number" => Ok("Numeric"),
        "string" => Ok("String"),
        "event" => Ok("Event"),
        o => err(format!("input type must be boolean, numeric, string or event, not '{o}'")),
    }
}

fn add_input(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let name = arg_str(a, "name")?;
    let ty = parse_input_type(arg_str(a, "type")?)?;
    let inputs = arr(cx, &format!("{mp}/inputs"));
    if inputs.iter().any(|i| i["name"] == name) {
        return err(format!("input '{name}' already exists"));
    }
    let mut inp = json!({"type": ty, "name": name});
    if let Some(v) = input_value(ty, a.get("value"))? {
        inp["value"] = v;
    }
    cx.insert(&format!("{mp}/inputs"), inputs.len(), inp)?;
    Ok(json!({"ok": true}))
}

fn walk_input_refs(sm: &mut Value, f: &mut dyn FnMut(&mut Value) -> bool) {
    // f returns false to drop the guard/action.
    if let Some(states) = sm.get_mut("states").and_then(Value::as_array_mut) {
        for s in states {
            if let Some(ts) = s.get_mut("transitions").and_then(Value::as_array_mut) {
                for t in ts {
                    if let Some(gs) = t.get_mut("guards").and_then(Value::as_array_mut) {
                        gs.retain_mut(|g| f(g));
                    }
                }
            }
            for k in ["entryActions", "exitActions"] {
                if let Some(acts) = s.get_mut(k).and_then(Value::as_array_mut) {
                    acts.retain_mut(|x| f(x));
                }
            }
        }
    }
    if let Some(is) = sm.get_mut("interactions").and_then(Value::as_array_mut) {
        for i in is {
            if let Some(acts) = i.get_mut("actions").and_then(Value::as_array_mut) {
                acts.retain_mut(|x| f(x));
            }
        }
    }
}

fn update_input(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let name = arg_str(a, "name")?;
    let mut sm = cx.get(&mp).cloned().unwrap();
    let i = sm["inputs"].as_array().and_then(|v| v.iter().position(|x| x["name"] == name)).ok_or_else(|| OpError(format!("no input '{name}'")))?;
    let ty = match opt_str(a, "type") {
        Some(t) => parse_input_type(t)?.to_string(),
        None => sm["inputs"][i]["type"].as_str().unwrap_or("Numeric").to_string(),
    };
    let mut inp = json!({"type": ty, "name": name});
    let old_val = sm["inputs"][i].get("value").cloned();
    let v = if a.get("value").is_some() { a.get("value").cloned() } else if opt_str(a, "type").is_none() { old_val } else { None };
    if let Some(v) = input_value(&ty, v.as_ref())? {
        inp["value"] = v;
    }
    if let Some(nn) = opt_str(a, "new_name").filter(|n| *n != name) {
        if sm["inputs"].as_array().unwrap().iter().any(|x| x["name"] == nn) {
            return err(format!("input '{nn}' already exists"));
        }
        inp["name"] = json!(nn);
        let (old, new) = (format!("${name}"), format!("${nn}"));
        walk_input_refs(&mut sm, &mut |g| {
            if g.get("inputName").and_then(Value::as_str) == Some(name) {
                g["inputName"] = json!(nn);
            }
            for k in ["compareTo", "value"] {
                if g.get(k).and_then(Value::as_str) == Some(old.as_str()) {
                    g[k] = json!(new);
                }
            }
            true
        });
    }
    sm["inputs"][i] = inp;
    cx.set(&mp, sm)?;
    Ok(json!({"ok": true}))
}

fn remove_input(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let name = arg_str(a, "name")?;
    let mut sm = cx.get(&mp).cloned().unwrap();
    let i = sm["inputs"].as_array().and_then(|v| v.iter().position(|x| x["name"] == name)).ok_or_else(|| OpError(format!("no input '{name}'")))?;
    sm["inputs"].as_array_mut().unwrap().remove(i);
    walk_input_refs(&mut sm, &mut |g| g.get("inputName").and_then(Value::as_str) != Some(name));
    cx.set(&mp, sm)?;
    Ok(json!({"ok": true}))
}

fn add_interaction(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let it = interaction(a, None)?;
    if let Some(s) = it.get("stateName").and_then(Value::as_str) {
        state_index(cx, &mp, s)?;
    }
    let ip = format!("{mp}/interactions");
    let n = arr(cx, &ip).len();
    cx.insert(&ip, n, it)?;
    Ok(json!({"index": n}))
}

fn interaction_ptr(cx: &Cx, mp: &str, a: &Value) -> OpResult<String> {
    let idx = a.get("index").and_then(Value::as_u64).ok_or_else(|| OpError("missing 'index'".into()))?;
    let p = format!("{mp}/interactions/{idx}");
    if cx.get(&p).is_none() {
        return err(format!("no interaction {idx}"));
    }
    Ok(p)
}

fn update_interaction(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let ip = interaction_ptr(cx, &mp, a)?;
    let it = interaction(a, cx.get(&ip).cloned())?;
    cx.set(&ip, it)?;
    Ok(json!({"ok": true}))
}

fn remove_interaction(cx: &mut Cx, a: &Value) -> OpResult {
    let mp = mptr(cx, a)?;
    let ip = interaction_ptr(cx, &mp, a)?;
    let (parent, idx) = ip.rsplit_once('/').unwrap();
    cx.remove(parent, idx.parse().unwrap())?;
    Ok(json!({"ok": true}))
}
