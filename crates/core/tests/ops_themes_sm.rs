mod common;
use common::*;
use serde_json::json;

fn setup() -> lumetrope_core::engine::Engine {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "Button", "fill": "#ff0000"}));
    e
}

#[test]
fn slots_create_update_list_unlink() {
    let mut e = setup();
    let s = ok(&mut e, "create_slot", json!({"layer": "Button", "shape": "1", "property": "color", "slot_id": "brand"}));
    assert_eq!(s["slot_id"], "brand");
    assert_eq!(layer(&e, 0)["shapes"][1]["c"]["sid"], "brand");
    assert!(anim(&e)["slots"]["brand"]["p"]["k"].is_array());
    ok(&mut e, "update_slot_value", json!({"slot_id": "brand", "value": "#0000ff"}));
    assert_eq!(layer(&e, 0)["shapes"][1]["c"]["k"], json!([0.0, 0.0, 1.0, 1.0]));
    assert_eq!(anim(&e)["slots"]["brand"]["p"]["k"], json!([0.0, 0.0, 1.0, 1.0]));
    let l = ok(&mut e, "list_slots", json!({}));
    assert_eq!(l[0]["id"], "brand");
    assert_eq!(l[0]["type"], "Color");
    assert_eq!(l[0]["bound"], 1);
    ok(&mut e, "unlink_slot", json!({"layer": "Button", "shape": "1", "property": "color"}));
    assert!(layer(&e, 0)["shapes"][1]["c"].get("sid").is_none());
    assert!(anim(&e).get("slots").map(|s| s.get("brand").is_none()).unwrap_or(true));
}

#[test]
fn themes_and_rules() {
    let mut e = setup();
    ok(&mut e, "create_slot", json!({"layer": "Button", "shape": "1", "property": "color", "slot_id": "brand"}));
    let t = ok(&mut e, "add_theme", json!({"name": "Dark"}));
    let id = t["theme"].as_str().unwrap().to_string();
    ok(&mut e, "update_theme_rule", json!({"theme": id, "slot_id": "brand", "value": "#00ff00"}));
    let th = e.project()["themes"][id.as_str()].clone();
    assert_eq!(th["name"], "Dark");
    assert_eq!(th["rules"][0], json!({"id": "brand", "type": "Color", "value": [0.0, 1.0, 0.0, 1.0]}));
    ok(&mut e, "select_theme", json!({"theme": id}));
    assert_eq!(e.project()["settings"]["preview_theme"], json!(id));
    assert_eq!(ok(&mut e, "list_themes", json!({}))[0]["rules"], 1);
    ok(&mut e, "update_theme_rule", json!({"theme": id, "slot_id": "brand", "remove": true}));
    assert_eq!(e.project()["themes"][id.as_str()]["rules"].as_array().unwrap().len(), 0);
    ok(&mut e, "remove_theme", json!({"theme": id}));
    assert!(e.project()["themes"].get(id.as_str()).is_none());
    assert!(e.project()["settings"].get("preview_theme").is_none());
}

#[test]
fn segments_crud() {
    let mut e = setup();
    ok(&mut e, "add_segment", json!({"name": "idle", "start": 0, "end": 30}));
    ok(&mut e, "add_segment", json!({"name": "hover", "start": 30, "end": 60}));
    assert!(fail(&mut e, "add_segment", json!({"name": "idle", "start": 0, "end": 5})).contains("exists"));
    assert!(fail(&mut e, "add_segment", json!({"name": "bad", "start": 10, "end": 5})).contains("end"));
    ok(&mut e, "update_segment", json!({"name": "hover", "new_name": "over", "end": 50}));
    assert_eq!(anim(&e)["markers"][1], json!({"cm": "over", "tm": 30.0, "dr": 20.0}));
    let l = ok(&mut e, "list_segments", json!({}));
    assert_eq!(l[1], json!({"name": "over", "start": 30.0, "end": 50.0}));
    ok(&mut e, "remove_segment", json!({"name": "idle"}));
    assert_eq!(anim(&e)["markers"].as_array().unwrap().len(), 1);
}

#[test]
fn state_machine_authoring() {
    let mut e = setup();
    let m = ok(&mut e, "add_state_machine", json!({"name": "Toggle"}));
    let id = m["machine"].as_str().unwrap().to_string();
    ok(&mut e, "add_input", json!({"machine": id, "name": "on", "type": "boolean", "value": false}));
    ok(&mut e, "add_input", json!({"machine": id, "name": "count", "type": "numeric", "value": 0}));
    ok(&mut e, "add_state", json!({"machine": id, "name": "Off", "segment": "idle", "loop": true, "autoplay": true}));
    ok(&mut e, "add_state", json!({"machine": id, "name": "On", "mode": "reverse", "speed": 2}));
    ok(&mut e, "add_state", json!({"machine": id, "name": "Global", "type": "global"}));
    ok(&mut e, "add_transition", json!({"machine": id, "from": "Off", "to": "On", "guards": [{"input": "on", "condition": "equal", "value": true}]}));
    ok(&mut e, "add_transition", json!({"machine": id, "from": "On", "to": "Off", "type": "tweened", "duration": 0.5, "easing": "ease_in_out",
        "guards": [{"input": "count", "condition": "greater_than", "value": "$count"}]}));
    assert!(fail(&mut e, "add_transition", json!({"machine": id, "from": "Off", "to": "Nowhere"})).contains("Nowhere"));
    assert!(fail(&mut e, "add_transition", json!({"machine": id, "from": "Off", "to": "On", "guards": [{"input": "missing", "condition": "equal", "value": 1}]})).contains("missing"));
    ok(&mut e, "add_interaction", json!({"machine": id, "type": "Click", "layer": "Button", "actions": [{"type": "toggle", "input": "on"}, {"type": "Increment", "inputName": "count", "value": 1}]}));
    ok(&mut e, "add_interaction", json!({"machine": id, "type": "OnComplete", "state": "On", "actions": [{"type": "fire_custom_event", "value": "done"}]}));
    let sm = ok(&mut e, "read_state_machine", json!({"machine": id}));
    assert_eq!(sm["initial"], "Off");
    let states = sm["states"].as_array().unwrap();
    assert_eq!(states[0], json!({"name": "Off", "type": "PlaybackState", "animation": "", "transitions": [
        {"type": "Transition", "toState": "On", "guards": [{"type": "Boolean", "inputName": "on", "conditionType": "Equal", "compareTo": true}]}],
        "segment": "idle", "loop": true, "autoplay": true}));
    assert_eq!(states[1]["mode"], "Reverse");
    assert_eq!(states[1]["transitions"][0]["type"], "Tweened");
    assert_eq!(states[1]["transitions"][0]["easing"], json!([0.42, 0.0, 0.58, 1.0]));
    assert_eq!(states[1]["transitions"][0]["guards"][0]["compareTo"], "$count");
    assert_eq!(states[2]["type"], "GlobalState");
    assert_eq!(sm["interactions"][0], json!({"type": "Click", "layerName": "Button", "actions": [{"type": "Toggle", "inputName": "on"}, {"type": "Increment", "inputName": "count", "value": 1}]}));
    assert_eq!(sm["interactions"][1]["stateName"], "On");
    assert_eq!(sm["inputs"][0], json!({"type": "Boolean", "name": "on", "value": false}));
    assert!(sm.get("lmeta").is_none(), "read_state_machine returns the clean spec JSON");

    ok(&mut e, "update_state", json!({"machine": id, "name": "On", "new_name": "Active"}));
    let sm = ok(&mut e, "read_state_machine", json!({"machine": id}));
    assert_eq!(sm["states"][0]["transitions"][0]["toState"], "Active", "renames propagate to transitions");
    assert_eq!(sm["interactions"][1]["stateName"], "Active");
    ok(&mut e, "set_initial_state", json!({"machine": id, "name": "Active"}));
    ok(&mut e, "remove_state", json!({"machine": id, "name": "Off"}));
    let sm = ok(&mut e, "read_state_machine", json!({"machine": id}));
    assert_eq!(sm["states"].as_array().unwrap().len(), 2);
    assert!(sm["states"][0]["transitions"].as_array().unwrap().is_empty(), "transitions into removed state are dropped");
    ok(&mut e, "add_transition", json!({"machine": id, "from": "Active", "to": "Global"}));
    ok(&mut e, "remove_transition", json!({"machine": id, "from": "Active", "index": 0}));
    assert!(ok(&mut e, "read_state_machine", json!({"machine": id}))["states"][0]["transitions"].as_array().unwrap().is_empty());
    ok(&mut e, "update_input", json!({"machine": id, "name": "count", "value": 5}));
    ok(&mut e, "remove_input", json!({"machine": id, "name": "count"}));
    ok(&mut e, "remove_interaction", json!({"machine": id, "index": 0}));
    ok(&mut e, "rename_state_machine", json!({"machine": id, "name": "Switch"}));
    let list = ok(&mut e, "list_state_machines", json!({}));
    assert_eq!(list[0]["name"], "Switch");
    ok(&mut e, "remove_state_machine", json!({"machine": id}));
    assert!(ok(&mut e, "list_state_machines", json!({})).as_array().unwrap().is_empty());
}

#[test]
fn update_transition_and_interaction() {
    let mut e = setup();
    let id = ok(&mut e, "add_state_machine", json!({}))["machine"].as_str().unwrap().to_string();
    ok(&mut e, "add_input", json!({"machine": id, "name": "go", "type": "event"}));
    ok(&mut e, "add_state", json!({"machine": id, "name": "A"}));
    ok(&mut e, "add_state", json!({"machine": id, "name": "B"}));
    ok(&mut e, "add_transition", json!({"machine": id, "from": "A", "to": "B"}));
    ok(&mut e, "update_transition", json!({"machine": id, "from": "A", "index": 0, "type": "tweened", "duration": 1, "easing": [0, 0, 1, 1], "guards": [{"input": "go"}]}));
    let sm = ok(&mut e, "read_state_machine", json!({"machine": id}));
    assert_eq!(sm["states"][0]["transitions"][0], json!({"type": "Tweened", "toState": "B", "duration": 1.0, "easing": [0.0, 0.0, 1.0, 1.0], "guards": [{"type": "Event", "inputName": "go"}]}));
    ok(&mut e, "add_interaction", json!({"machine": id, "type": "PointerDown", "actions": [{"type": "fire", "input": "go"}]}));
    ok(&mut e, "update_interaction", json!({"machine": id, "index": 0, "type": "PointerUp", "layer": "Button"}));
    let sm = ok(&mut e, "read_state_machine", json!({"machine": id}));
    assert_eq!(sm["interactions"][0], json!({"type": "PointerUp", "layerName": "Button", "actions": [{"type": "Fire", "inputName": "go"}]}));
    assert!(fail(&mut e, "add_interaction", json!({"machine": id, "type": "Swipe", "actions": []})).contains("Swipe"));
    assert!(fail(&mut e, "add_interaction", json!({"machine": id, "type": "Click", "actions": [{"type": "explode"}]})).contains("explode"));
}
