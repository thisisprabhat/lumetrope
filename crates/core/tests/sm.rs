use lumetrope_core::model;
use lumetrope_core::sm::{InputValue, Machine, PointerKind, SmEvent};
use serde_json::{json, Value};

/// 30 fps, 100 frames, segments a=[0,10] b=[20,40] c=[50,60].
fn anim() -> model::Animation {
    model::parse(&json!({"fr": 30, "ip": 0, "op": 100, "w": 100, "h": 100, "layers": [],
        "markers": [{"cm": "a", "tm": 0, "dr": 10}, {"cm": "b", "tm": 20, "dr": 20}, {"cm": "c", "tm": 50, "dr": 10}]})).unwrap()
}

fn st(name: &str, seg: &str, extra: Value, transitions: Value) -> Value {
    let mut s = json!({"name": name, "type": "PlaybackState", "animation": "", "segment": seg, "autoplay": true, "transitions": transitions});
    for (k, v) in extra.as_object().unwrap() {
        s[k] = v.clone();
    }
    s
}

fn machine(def: Value) -> Machine {
    let mut m = Machine::new(&def, &anim()).unwrap();
    m.start();
    m
}

fn tick_secs(m: &mut Machine, secs: f64) -> Vec<SmEvent> {
    let mut ev = vec![];
    let steps = (secs * 60.0).round() as usize;
    for _ in 0..steps {
        ev.extend(m.tick(1.0 / 60.0));
    }
    ev
}

fn guard_to(to: &str, guards: Value) -> Value {
    json!([{"type": "Transition", "toState": to, "guards": guards}])
}

#[test]
fn initial_state_plays_segment() {
    let mut m = machine(json!({"initial": "A", "states": [st("A", "b", json!({"loop": true}), json!([]))]}));
    let s = m.snapshot();
    assert_eq!((s.state.as_str(), s.frame), ("A", 20.0));
    assert!(s.playing);
    m.tick(0.5);
    assert!((m.snapshot().frame - 35.0).abs() < 1e-6);
}

#[test]
fn numeric_guard_greater_than() {
    let def = json!({"initial": "A", "inputs": [{"type": "Numeric", "name": "n", "value": 0}], "states": [
        st("A", "a", json!({}), guard_to("B", json!([{"type": "Numeric", "inputName": "n", "conditionType": "GreaterThan", "compareTo": 5}]))),
        st("B", "b", json!({}), json!([]))]});
    let mut m = machine(def);
    m.set_input("n", InputValue::Num(5.0));
    assert_eq!(m.snapshot().state, "A");
    m.set_input("n", InputValue::Num(6.0));
    assert_eq!(m.snapshot().state, "B");
}

#[test]
fn input_reference_compare_to() {
    let def = json!({"initial": "A", "inputs": [{"type": "Numeric", "name": "x", "value": 1}, {"type": "Numeric", "name": "y", "value": 3}], "states": [
        st("A", "a", json!({}), guard_to("B", json!([{"type": "Numeric", "inputName": "x", "conditionType": "Equal", "compareTo": "$y"}]))),
        st("B", "b", json!({}), json!([]))]});
    let mut m = machine(def);
    m.set_input("x", InputValue::Num(3.0));
    assert_eq!(m.snapshot().state, "B");
}

#[test]
fn event_input_consumed_after_run() {
    let def = json!({"initial": "A", "inputs": [{"type": "Event", "name": "go"}], "states": [
        st("A", "a", json!({}), guard_to("B", json!([{"type": "Event", "inputName": "go"}]))),
        st("B", "b", json!({}), guard_to("A", json!([{"type": "Event", "inputName": "go"}])))]});
    let mut m = machine(def);
    m.fire("go");
    assert_eq!(m.snapshot().state, "B", "one event moves exactly one step");
    m.tick(0.1);
    assert_eq!(m.snapshot().state, "B", "event was consumed");
    m.fire("go");
    assert_eq!(m.snapshot().state, "A");
}

#[test]
fn global_state_transitions_first() {
    let def = json!({"initial": "A", "inputs": [{"type": "Boolean", "name": "b", "value": false}], "states": [
        st("A", "a", json!({}), guard_to("B", json!([{"type": "Boolean", "inputName": "b", "conditionType": "Equal", "compareTo": true}]))),
        st("B", "b", json!({}), json!([])),
        st("C", "c", json!({}), json!([])),
        {"name": "G", "type": "GlobalState", "transitions": guard_to("C", json!([{"type": "Boolean", "inputName": "b", "conditionType": "Equal", "compareTo": true}]))}]});
    let mut m = machine(def);
    m.set_input("b", InputValue::Bool(true));
    assert_eq!(m.snapshot().state, "C");
}

#[test]
fn entry_exit_order() {
    let def = json!({"initial": "A", "inputs": [{"type": "Event", "name": "go"}, {"type": "String", "name": "log", "value": ""}], "states": [
        st("A", "a", json!({"exitActions": [{"type": "FireCustomEvent", "value": "exit A"}]}), guard_to("B", json!([{"type": "Event", "inputName": "go"}]))),
        st("B", "b", json!({"entryActions": [{"type": "FireCustomEvent", "value": "enter B"}]}), json!([]))]});
    let mut m = machine(def);
    let ev = m.fire("go");
    let custom: Vec<String> = ev.iter().filter_map(|e| if let SmEvent::CustomEvent(s) = e { Some(s.clone()) } else { None }).collect();
    assert_eq!(custom, vec!["exit A", "enter B"]);
    let pos = |pred: &dyn Fn(&SmEvent) -> bool| ev.iter().position(|e| pred(e)).unwrap();
    assert!(pos(&|e| matches!(e, SmEvent::StateExited(s) if s == "A")) < pos(&|e| matches!(e, SmEvent::StateEntered(s) if s == "B")));
}

#[test]
fn tweened_transition_blocks_until_done() {
    let def = json!({"initial": "A", "inputs": [{"type": "Event", "name": "go"}], "states": [
        st("A", "a", json!({}), json!([{"type": "Tweened", "toState": "B", "duration": 0.5, "easing": [0, 0, 1, 1], "guards": [{"type": "Event", "inputName": "go"}]}])),
        st("B", "c", json!({}), guard_to("A", json!([{"type": "Event", "inputName": "go"}])))]});
    let mut m = machine(def);
    m.fire("go");
    assert!(m.snapshot().tweening);
    assert_eq!(m.snapshot().state, "A");
    m.fire("go");
    m.tick(0.25);
    let f = m.snapshot().frame;
    assert!(f > 0.0 && f < 50.0, "tweening frame between states: {f}");
    tick_secs(&mut m, 0.3);
    let s = m.snapshot();
    assert_eq!(s.state, "B");
    assert!(!s.tweening);
}

#[test]
fn on_complete_interaction() {
    let def = json!({"initial": "A", "inputs": [{"type": "Boolean", "name": "done", "value": false}], "states": [
        st("A", "a", json!({}), guard_to("B", json!([{"type": "Boolean", "inputName": "done", "conditionType": "Equal", "compareTo": true}]))),
        st("B", "b", json!({"loop": true}), json!([]))],
        "interactions": [{"type": "OnComplete", "stateName": "A", "actions": [{"type": "SetBoolean", "inputName": "done", "value": true}]}]});
    let mut m = machine(def);
    tick_secs(&mut m, 0.2);
    assert_eq!(m.snapshot().state, "A");
    let ev = tick_secs(&mut m, 0.3);
    assert!(ev.iter().any(|e| matches!(e, SmEvent::Complete)));
    assert_eq!(m.snapshot().state, "B");
}

#[test]
fn click_on_named_layer_only() {
    let def = json!({"initial": "A", "inputs": [{"type": "Numeric", "name": "n", "value": 0}], "states": [st("A", "a", json!({}), json!([]))],
        "interactions": [{"type": "Click", "layerName": "Button", "actions": [{"type": "Increment", "inputName": "n"}]},
                         {"type": "PointerDown", "actions": [{"type": "Increment", "inputName": "n", "value": 10}]}]});
    let mut m = machine(def);
    m.pointer(PointerKind::Click, &["Other".to_string()]);
    assert_eq!(m.input("n"), Some(InputValue::Num(0.0)));
    m.pointer(PointerKind::Click, &["Button".to_string()]);
    assert_eq!(m.input("n"), Some(InputValue::Num(1.0)));
    m.pointer(PointerKind::Down, &[]);
    assert_eq!(m.input("n"), Some(InputValue::Num(11.0)));
}

#[test]
fn pointer_enter_exit_per_layer() {
    let def = json!({"initial": "A", "inputs": [{"type": "Boolean", "name": "h", "value": false}], "states": [st("A", "a", json!({}), json!([]))],
        "interactions": [{"type": "PointerEnter", "layerName": "Card", "actions": [{"type": "SetBoolean", "inputName": "h", "value": true}]},
                         {"type": "PointerExit", "layerName": "Card", "actions": [{"type": "SetBoolean", "inputName": "h", "value": false}]}]});
    let mut m = machine(def);
    m.pointer(PointerKind::Move, &["Card".to_string()]);
    assert_eq!(m.input("h"), Some(InputValue::Bool(true)));
    m.pointer(PointerKind::Move, &["Card".to_string()]);
    assert_eq!(m.input("h"), Some(InputValue::Bool(true)));
    m.pointer(PointerKind::Move, &[]);
    assert_eq!(m.input("h"), Some(InputValue::Bool(false)));
}

#[test]
fn increment_decrement_toggle_actions() {
    let def = json!({"initial": "A", "inputs": [{"type": "Numeric", "name": "n", "value": 5}, {"type": "Numeric", "name": "step", "value": 2}, {"type": "Boolean", "name": "b", "value": false}, {"type": "String", "name": "s", "value": "x"}],
        "states": [st("A", "a", json!({}), json!([]))],
        "interactions": [{"type": "Click", "actions": [{"type": "Increment", "inputName": "n", "value": "$step"}, {"type": "Decrement", "inputName": "n"}, {"type": "Toggle", "inputName": "b"}, {"type": "SetString", "inputName": "s", "value": "y"}]},
                         {"type": "PointerUp", "actions": [{"type": "Reset", "inputName": "n"}, {"type": "Reset", "inputName": "s"}]}]});
    let mut m = machine(def);
    m.pointer(PointerKind::Click, &[]);
    assert_eq!((m.input("n"), m.input("b"), m.input("s")), (Some(InputValue::Num(6.0)), Some(InputValue::Bool(true)), Some(InputValue::Str("y".into()))));
    m.pointer(PointerKind::Up, &[]);
    assert_eq!((m.input("n"), m.input("s")), (Some(InputValue::Num(5.0)), Some(InputValue::Str("x".into()))));
}

#[test]
fn set_progress_and_set_frame_move_frame() {
    let def = json!({"initial": "A", "states": [st("A", "b", json!({"autoplay": false}), json!([]))],
        "interactions": [{"type": "Click", "actions": [{"type": "SetProgress", "value": 0.5}]}, {"type": "PointerDown", "actions": [{"type": "SetFrame", "value": 33}]}]});
    let mut m = machine(def);
    assert!(!m.snapshot().playing);
    m.pointer(PointerKind::Click, &[]);
    assert!((m.snapshot().frame - 30.0).abs() < 1e-6, "50% of segment b [20,40]");
    m.pointer(PointerKind::Down, &[]);
    assert_eq!(m.snapshot().frame, 33.0);
}

#[test]
fn bounce_mode_reverses() {
    let mut m = machine(json!({"initial": "A", "states": [st("A", "b", json!({"mode": "Bounce", "loop": true}), json!([]))]}));
    m.tick(0.6); // 18 frames forward from 20 → hits 40 after 20/30 s
    m.tick(0.2);
    let f = m.snapshot().frame;
    assert!(f < 40.0 && f > 30.0, "came back from the end: {f}");
    let mut r = machine(json!({"initial": "A", "states": [st("A", "b", json!({"mode": "Reverse"}), json!([]))]}));
    assert_eq!(r.snapshot().frame, 40.0);
    r.tick(0.1);
    assert!((r.snapshot().frame - 37.0).abs() < 1e-6);
}

#[test]
fn loop_count_then_complete() {
    let mut m = machine(json!({"initial": "A", "states": [st("A", "a", json!({"loop": true, "loopCount": 2}), json!([]))]}));
    let ev = tick_secs(&mut m, 1.0);
    let loops = ev.iter().filter(|e| matches!(e, SmEvent::LoopComplete)).count();
    assert_eq!(loops, 2);
    assert!(ev.iter().any(|e| matches!(e, SmEvent::Complete)));
    assert!(!m.snapshot().playing);
}

#[test]
fn ping_pong_is_bounded() {
    let def = json!({"initial": "A", "inputs": [{"type": "Boolean", "name": "b", "value": false}], "states": [
        st("A", "a", json!({}), guard_to("B", json!([{"type": "Boolean", "inputName": "b", "conditionType": "Equal", "compareTo": true}]))),
        st("B", "b", json!({}), guard_to("A", json!([{"type": "Boolean", "inputName": "b", "conditionType": "Equal", "compareTo": true}])))]});
    let mut m = machine(def);
    m.set_input("b", InputValue::Bool(true)); // must return, not hang
    let s = m.snapshot();
    assert!(s.state == "A" || s.state == "B");
}

#[test]
fn missing_initial_is_error() {
    assert!(Machine::new(&json!({"initial": "Nope", "states": []}), &anim()).is_err());
    assert!(Machine::new(&json!([1, 2]), &anim()).is_err());
}

#[test]
fn toggle_sample_click_sun_goes_to_night() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/lottiefiles/toogle-E7UafdXhQK.lottie");
    let Ok(bytes) = std::fs::read(&path) else { return };
    let p = lumetrope_core::dotlottie::read(&bytes).unwrap();
    let a = model::parse(&p["animations"]["Main Scene"]).unwrap();
    let mut m = Machine::new(&p["stateMachines"]["StateMachine1"], &a).unwrap();
    m.start();
    assert_eq!(m.snapshot().state, "Day Idle");
    m.pointer(PointerKind::Click, &["Sun".to_string()]);
    assert_eq!(m.snapshot().state, "Day to Night");
    for _ in 0..600 {
        m.tick(1.0 / 60.0);
    }
    assert_eq!(m.snapshot().state, "Night Idle");
    assert_eq!(m.input("toggle"), Some(InputValue::Bool(true)));
}
