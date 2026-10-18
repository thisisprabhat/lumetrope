mod common;
use common::*;
use serde_json::json;

fn setup() -> lumetrope_core::engine::Engine {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A"}));
    e
}

#[test]
fn set_keyframe_animates_static_prop_and_renders_between() {
    let mut e = setup();
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "opacity", "frame": 0, "value": 0}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "opacity", "frame": 20, "value": 100, "easing": "linear"}));
    let o = layer(&e, 0)["ks"]["o"].clone();
    assert_eq!(o["a"], 1);
    assert_eq!(o["k"][0]["s"], json!([0.0]));
    let mid = e.render_rgba(10.0, 400, 300)[(150 * 400 + 200) * 4 + 3];
    assert!(mid > 100 && mid < 160, "mid alpha {mid}");
}

#[test]
fn keyframes_stay_sorted_and_replace_same_frame() {
    let mut e = setup();
    for (f, v) in [(30, 3), (0, 1), (15, 2), (15, 9)] {
        ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "rotation", "frame": f, "value": v}));
    }
    let k = layer(&e, 0)["ks"]["r"]["k"].clone();
    let ts: Vec<f64> = k.as_array().unwrap().iter().map(|x| x["t"].as_f64().unwrap()).collect();
    assert_eq!(ts, vec![0.0, 15.0, 30.0]);
    assert_eq!(k[1]["s"], json!([9.0]));
}

#[test]
fn remove_last_keyframe_makes_static() {
    let mut e = setup();
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "rotation", "frame": 0, "value": 10}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "rotation", "frame": 10, "value": 20}));
    ok(&mut e, "remove_keyframe", json!({"layer": "A", "property": "rotation", "frame": 10}));
    let r = layer(&e, 0)["ks"]["r"].clone();
    assert_eq!(r["a"], 0);
    assert_eq!(r["k"], json!(10.0));
    assert!(fail(&mut e, "remove_keyframe", json!({"layer": "A", "property": "rotation", "frame": 0})).contains("not animated"));
}

#[test]
fn easing_preset_ease_in_out_writes_bezier_and_hold() {
    let mut e = setup();
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "scale", "frame": 0, "value": [100, 100]}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "scale", "frame": 10, "value": [50, 50]}));
    ok(&mut e, "set_keyframe_easing", json!({"layer": "A", "property": "scale", "frame": 0, "easing": "ease_in_out"}));
    let k0 = layer(&e, 0)["ks"]["s"]["k"][0].clone();
    assert_eq!(k0["o"], json!({"x": [0.42], "y": [0.0]}));
    assert_eq!(k0["i"], json!({"x": [0.58], "y": [1.0]}));
    ok(&mut e, "set_keyframe_easing", json!({"layer": "A", "property": "scale", "frame": 0, "easing": "hold"}));
    assert_eq!(layer(&e, 0)["ks"]["s"]["k"][0]["h"], 1);
    assert!(fail(&mut e, "set_keyframe_easing", json!({"layer": "A", "property": "scale", "frame": 0, "easing": "wobbly"})).contains("unknown easing"));
}

#[test]
fn read_keyframes_lists_all_animated() {
    let mut e = setup();
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "position", "frame": 0, "value": [0, 0]}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "shape": "1", "property": "color", "frame": 5, "value": "#ff0000"}));
    let r = ok(&mut e, "read_keyframes", json!({"layer": "A"}));
    let paths: Vec<String> = r["animated"].as_array().unwrap().iter().map(|x| x["path"].as_str().unwrap().to_string()).collect();
    assert!(paths.contains(&"ks/p".to_string()) && paths.contains(&"shapes/1/c".to_string()), "{paths:?}");
    let one = ok(&mut e, "read_keyframes", json!({"layer": "A", "shape": "1", "property": "color"}));
    assert_eq!(one["keyframes"][0]["value"], json!([1.0, 0.0, 0.0, 1.0]));
}

#[test]
fn clear_shift_retime_copy() {
    let mut e = setup();
    ok(&mut e, "create_shape", json!({"type": "ellipse", "name": "B"}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "rotation", "frame": 10, "value": 0}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "rotation", "frame": 20, "value": 90}));
    ok(&mut e, "shift_keyframes", json!({"layer": "A", "by": 5}));
    ok(&mut e, "retime_keyframes", json!({"layer": "A", "property": "rotation", "scale": 2, "pivot": 15}));
    let ts: Vec<f64> = layer(&e, 1)["ks"]["r"]["k"].as_array().unwrap().iter().map(|k| k["t"].as_f64().unwrap()).collect();
    assert_eq!(ts, vec![15.0, 35.0]);
    ok(&mut e, "copy_animation", json!({"from": "A", "to": "B", "offset_frames": 10}));
    assert_eq!(layer(&e, 0)["ks"]["r"]["k"][0]["t"], 25.0);
    ok(&mut e, "clear_animation", json!({"layer": "A", "property": "rotation", "frame": 35}));
    assert_eq!(layer(&e, 1)["ks"]["r"], json!({"a": 0, "k": 90.0}));
}

#[test]
fn path_keyframes_on_shape() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "path", "points": [[0,0],[10,0],[5,10]]}));
    ok(&mut e, "set_keyframe", json!({"layer": "root:1", "shape": "0", "property": "path", "frame": 0, "value": {"v": [[0,0],[10,0],[5,10]]}}));
    ok(&mut e, "set_keyframe", json!({"layer": "root:1", "shape": "0", "property": "path", "frame": 10, "value": {"v": [[0,0],[20,0],[5,20]]}}));
    let k = layer(&e, 0)["shapes"][0]["ks"]["k"].clone();
    assert_eq!(k[1]["s"][0]["v"][1], json!([20, 0]));
}

#[test]
fn get_property_reports_value_and_keys() {
    let mut e = setup();
    let st = ok(&mut e, "get_property", json!({"layer": "A", "property": "position", "frame": 0}));
    assert_eq!(st["value"], json!([200.0, 150.0, 0.0]));
    assert_eq!(st["animated"], false);
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "rotation", "frame": 0, "value": 0, "easing": "linear"}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "rotation", "frame": 10, "value": 100}));
    let r = ok(&mut e, "get_property", json!({"layer": "A", "property": "rotation", "frame": 5}));
    assert_eq!((r["value"][0].as_f64(), r["animated"].as_bool(), r["keyed"].as_bool()), (Some(50.0), Some(true), Some(false)));
    assert_eq!(r["keyframes"], json!([0.0, 10.0]));
    assert_eq!(ok(&mut e, "get_property", json!({"layer": "A", "property": "rotation", "frame": 10}))["keyed"], true);
}
