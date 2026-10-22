//! The demo files shipped in `samples/` load, render, and (for the
//! interactive ones) respond to their state machines. Regenerate them with
//! `cargo test -p lumetrope-core --test demos -- --ignored write_demo_samples`.

use lumetrope_core::engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples")
}

fn load(name: &str) -> Engine {
    let bytes = std::fs::read(samples_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut e = Engine::new();
    e.load_file(name, &bytes).unwrap_or_else(|err| panic!("{name}: {err}"));
    e
}

fn painted(e: &mut Engine, frame: f64) -> usize {
    e.render_rgba(frame, 128, 128).chunks(4).filter(|p| p[3] > 0).count()
}

#[test]
fn every_demo_renders() {
    for name in ["demo-loader.json", "demo-button.lottie", "demo-toggle.lottie"] {
        let mut e = load(name);
        assert!(painted(&mut e, 10.0) > 200, "{name} renders almost nothing");
    }
}

#[test]
fn button_demo_spins_when_clicked() {
    let mut e = load("demo-button.lottie");
    let id = e.project()["stateMachines"].as_object().unwrap().keys().next().unwrap().clone();
    assert_eq!(e.sm_start(&id).unwrap()["state"], "Idle");
    e.sm_tick(1.0); // let the button pop in
    assert_eq!(e.sm_pointer("click", 10.0, 10.0)["state"], "Idle");
    assert_eq!(e.sm_pointer("click", 200.0, 150.0)["state"], "Spin");
}

#[test]
fn toggle_demo_flips_both_ways() {
    let mut e = load("demo-toggle.lottie");
    let id = e.project()["stateMachines"].as_object().unwrap().keys().next().unwrap().clone();
    assert_eq!(e.sm_start(&id).unwrap()["state"], "Off");
    assert_eq!(e.sm_pointer("click", 200.0, 150.0)["state"], "On");
    e.sm_tick(1.0);
    assert_eq!(e.sm_pointer("click", 200.0, 150.0)["state"], "Off");
}

fn ok(e: &mut Engine, tool: &str, args: Value) -> Value {
    let r = e.apply(tool, &args);
    assert!(r.get("error").is_none(), "{tool}: {r}");
    r["ok"].clone()
}

#[test]
#[ignore]
fn write_demo_samples() {
    let dir = samples_dir();

    // 1. Loader: a trimmed arc that sweeps round — plain Lottie JSON.
    let mut e = Engine::new();
    e.new_project(256.0, 256.0, 30.0, 60.0);
    ok(&mut e, "set_scene_settings", json!({"name": "Loader"}));
    ok(&mut e, "create_shape", json!({"type": "ellipse", "name": "Arc", "position": [128, 128], "size": [140, 140], "stroke": "#ffb020", "stroke_width": 16}));
    ok(&mut e, "set_stroke", json!({"layer": "Arc", "line_cap": "round"}));
    let tm = ok(&mut e, "add_trim_path", json!({"layer": "Arc", "start": 0, "end": 30}))["shape"].clone();
    ok(&mut e, "set_keyframe", json!({"layer": "Arc", "property": "rotation", "frame": 0, "value": 0, "easing": "linear"}));
    ok(&mut e, "set_keyframe", json!({"layer": "Arc", "property": "rotation", "frame": 60, "value": 360}));
    ok(&mut e, "set_keyframe", json!({"layer": "Arc", "shape": tm, "property": "end", "frame": 0, "value": 10, "easing": "ease_in_out"}));
    ok(&mut e, "set_keyframe", json!({"layer": "Arc", "shape": tm, "property": "end", "frame": 30, "value": 75, "easing": "ease_in_out"}));
    ok(&mut e, "set_keyframe", json!({"layer": "Arc", "shape": tm, "property": "end", "frame": 60, "value": 10}));
    std::fs::write(dir.join("demo-loader.json"), e.export_lottie()).unwrap();

    // 2. Button: pops in, spins when clicked.
    let mut e = Engine::new();
    e.new_project(400.0, 300.0, 30.0, 60.0);
    ok(&mut e, "set_scene_settings", json!({"name": "Interactive button"}));
    ok(&mut e, "create_solid", json!({"color": "#14213d", "name": "Background"}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "Button", "position": [200, 150], "size": [180, 70], "roundness": 35, "fill": "#fca311"}));
    ok(&mut e, "create_text", json!({"text": "Press me", "position": [200, 160], "font_size": 26, "color": "#14213d", "name": "Label"}));
    ok(&mut e, "apply_motion_preset", json!({"layer": "Button", "preset": "pop", "start_frame": 0, "duration_frames": 20}));
    ok(&mut e, "add_segment", json!({"name": "intro", "start": 0, "end": 20}));
    ok(&mut e, "add_segment", json!({"name": "pressed", "start": 20, "end": 40}));
    ok(&mut e, "set_keyframe", json!({"layer": "Button", "property": "rotation", "frame": 20, "value": 0}));
    ok(&mut e, "set_keyframe", json!({"layer": "Button", "property": "rotation", "frame": 40, "value": 360, "easing": "ease_in_out"}));
    let m = ok(&mut e, "add_state_machine", json!({"name": "Button"}))["machine"].clone();
    ok(&mut e, "add_input", json!({"machine": m, "name": "press", "type": "event"}));
    ok(&mut e, "add_state", json!({"machine": m, "name": "Idle", "segment": "intro"}));
    ok(&mut e, "add_state", json!({"machine": m, "name": "Spin", "segment": "pressed"}));
    ok(&mut e, "add_transition", json!({"machine": m, "from": "Idle", "to": "Spin", "guards": [{"input": "press"}]}));
    ok(&mut e, "add_transition", json!({"machine": m, "from": "Spin", "to": "Idle", "guards": [{"input": "press"}]}));
    ok(&mut e, "add_interaction", json!({"machine": m, "type": "Click", "layer": "Button", "actions": [{"type": "fire", "input": "press"}]}));
    std::fs::write(dir.join("demo-button.lottie"), e.export_dotlottie().unwrap()).unwrap();

    // 3. Toggle: a switch whose knob slides, driven by a boolean input.
    let mut e = Engine::new();
    e.new_project(400.0, 300.0, 30.0, 30.0);
    ok(&mut e, "set_scene_settings", json!({"name": "Toggle switch"}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "Track", "position": [200, 150], "size": [160, 80], "roundness": 40, "fill": "#2a2f38"}));
    ok(&mut e, "create_shape", json!({"type": "ellipse", "name": "Knob", "position": [160, 150], "size": [64, 64], "fill": "#f4f4f4"}));
    ok(&mut e, "set_keyframe", json!({"layer": "Knob", "property": "position", "frame": 0, "value": [160, 150], "easing": "ease_in_out"}));
    ok(&mut e, "set_keyframe", json!({"layer": "Knob", "property": "position", "frame": 15, "value": [240, 150]}));
    ok(&mut e, "add_segment", json!({"name": "off", "start": 0, "end": 1}));
    ok(&mut e, "add_segment", json!({"name": "on", "start": 0, "end": 15}));
    let m = ok(&mut e, "add_state_machine", json!({"name": "Toggle"}))["machine"].clone();
    ok(&mut e, "add_input", json!({"machine": m, "name": "on", "type": "boolean", "value": false}));
    ok(&mut e, "add_state", json!({"machine": m, "name": "Off", "segment": "off", "loop": false}));
    ok(&mut e, "add_state", json!({"machine": m, "name": "On", "segment": "on", "loop": false}));
    ok(&mut e, "add_transition", json!({"machine": m, "from": "Off", "to": "On", "guards": [{"input": "on", "condition": "Equal", "value": true}]}));
    ok(&mut e, "add_transition", json!({"machine": m, "from": "On", "to": "Off", "guards": [{"input": "on", "condition": "Equal", "value": false}]}));
    ok(&mut e, "add_interaction", json!({"machine": m, "type": "Click", "layer": "Track", "actions": [{"type": "toggle", "input": "on"}]}));
    std::fs::write(dir.join("demo-toggle.lottie"), e.export_dotlottie().unwrap()).unwrap();
}
