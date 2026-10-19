mod common;
use common::*;
use serde_json::json;

#[test]
fn create_shape_adds_layer_on_top_with_defaults() {
    let mut e = eng();
    let a = ok(&mut e, "create_shape", json!({"type": "rect", "name": "Box"}));
    let b = ok(&mut e, "create_shape", json!({"type": "ellipse", "position": [10, 20], "size": [30, 40], "fill": "#ff0000"}));
    assert_eq!(a["id"], "root:1");
    assert_eq!(b["id"], "root:2");
    let top = layer(&e, 0);
    assert_eq!(top["ind"], 2);
    assert_eq!(top["ks"]["p"]["k"], json!([10.0, 20.0, 0.0]));
    assert_eq!(top["shapes"][0]["ty"], "el");
    assert_eq!(top["shapes"][1]["c"]["k"], json!([1.0, 0.0, 0.0, 1.0]));
    let box_ = layer(&e, 1);
    assert_eq!(box_["nm"], "Box");
    assert_eq!(box_["ks"]["p"]["k"], json!([200.0, 150.0, 0.0]), "defaults to scene centre");
    assert_eq!(box_["op"], 60.0);
    assert!(e.render_rgba(0.0, 400, 300)[(150 * 400 + 200) * 4 + 3] > 0, "visible");
}

#[test]
fn create_shape_star_polygon_path() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "star", "points": 6, "radius": 50, "inner_radius": 20}));
    ok(&mut e, "create_shape", json!({"type": "polygon", "points": 3, "radius": 40, "stroke": "#000000", "stroke_width": 4, "fill": null}));
    ok(&mut e, "create_shape", json!({"type": "path", "points": [[0, 0], [50, 0], [25, 40]], "closed": true}));
    assert_eq!(layer(&e, 2)["shapes"][0]["sy"], 1);
    assert_eq!(layer(&e, 1)["shapes"][0]["sy"], 2);
    assert_eq!(layer(&e, 1)["shapes"][1]["ty"], "st");
    assert_eq!(layer(&e, 0)["shapes"][0]["ks"]["k"]["v"].as_array().unwrap().len(), 3);
    assert!(fail(&mut e, "create_shape", json!({"type": "blob"})).contains("type"));
}

#[test]
fn duplicate_rename_delete() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A"}));
    let d = ok(&mut e, "duplicate_layer", json!({"layer": "root:1"}));
    assert_eq!(d["id"], "root:2");
    assert_eq!(layer(&e, 0)["nm"], "A copy");
    ok(&mut e, "rename_layer", json!({"layer": "root:2", "name": "B"}));
    assert_eq!(layer(&e, 0)["nm"], "B");
    ok(&mut e, "set_parent", json!({"layer": "B", "parent": "A"}));
    assert_eq!(layer(&e, 0)["parent"], 1);
    ok(&mut e, "delete_layer", json!({"layer": "A"}));
    assert_eq!(anim(&e)["layers"].as_array().unwrap().len(), 1);
    assert!(layer(&e, 0).get("parent").is_none(), "orphaned child is unparented");
}

#[test]
fn reorder_visibility_lock_time_blend() {
    let mut e = eng();
    for n in ["A", "B", "C"] {
        ok(&mut e, "create_shape", json!({"type": "rect", "name": n}));
    }
    ok(&mut e, "reorder_layer", json!({"layer": "C", "to": "back"}));
    assert_eq!(layer(&e, 2)["nm"], "C");
    ok(&mut e, "reorder_layer", json!({"layer": "C", "index": 0}));
    assert_eq!(layer(&e, 0)["nm"], "C");
    ok(&mut e, "set_layer_visibility", json!({"layer": "A", "visible": false}));
    assert_eq!(layer(&e, 2)["hd"], true);
    ok(&mut e, "set_layer_lock", json!({"layer": "A", "locked": true}));
    assert_eq!(layer(&e, 2)["lmeta"]["lock"], true);
    ok(&mut e, "set_layer_time", json!({"layer": "A", "in": 5, "out": 40, "start": 2}));
    assert_eq!((layer(&e, 2)["ip"].as_f64(), layer(&e, 2)["op"].as_f64(), layer(&e, 2)["st"].as_f64()), (Some(5.0), Some(40.0), Some(2.0)));
    ok(&mut e, "set_blend_mode", json!({"layer": "A", "mode": "multiply"}));
    assert_eq!(layer(&e, 2)["bm"], 1);
    assert!(fail(&mut e, "set_blend_mode", json!({"layer": "A", "mode": "weird"})).contains("mode"));
}

#[test]
fn matte_set_and_remove() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "Content"}));
    ok(&mut e, "create_shape", json!({"type": "ellipse", "name": "Mask"}));
    ok(&mut e, "reorder_layer", json!({"layer": "Mask", "to": "back"}));
    ok(&mut e, "set_matte", json!({"layer": "Content", "matte_layer": "Mask", "mode": "luma"}));
    let ls = anim(&e)["layers"].clone();
    let ci = ls.as_array().unwrap().iter().position(|l| l["nm"] == "Content").unwrap();
    assert_eq!(ls[ci - 1]["nm"], "Mask", "matte source placed directly above");
    assert_eq!(ls[ci - 1]["td"], 1);
    assert_eq!(ls[ci]["tt"], 3);
    assert_eq!(ls[ci]["tp"], 2);
    ok(&mut e, "remove_matte", json!({"layer": "Content"}));
    let ls = anim(&e)["layers"].clone();
    assert!(ls.as_array().unwrap().iter().all(|l| l.get("tt").is_none() && l.get("td").is_none()));
}

#[test]
fn precompose_moves_layers_to_asset() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A"}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "B"}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "C"}));
    let r = ok(&mut e, "precompose_layers", json!({"layers": ["A", "B"], "name": "Group"}));
    let a = anim(&e);
    assert_eq!(a["layers"].as_array().unwrap().len(), 2);
    let pc = a["layers"].as_array().unwrap().iter().find(|l| l["ty"] == 0).unwrap().clone();
    assert_eq!(pc["nm"], "Group");
    let asset = a["assets"].as_array().unwrap().iter().find(|x| x["id"] == pc["refId"]).unwrap().clone();
    assert_eq!(asset["layers"].as_array().unwrap().len(), 2);
    assert_eq!(r["scene"], pc["refId"]);
    assert!(e.render_rgba(0.0, 400, 300)[(150 * 400 + 200) * 4 + 3] > 0);
}

#[test]
fn transform_static_then_keyframed() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect"}));
    ok(&mut e, "set_transform", json!({"layer": "root:1", "position": [50, 60], "rotation": 45, "opacity": 50, "scale": [200, 100]}));
    let l = layer(&e, 0);
    assert_eq!(l["ks"]["p"]["k"], json!([50.0, 60.0, 0.0]));
    assert_eq!(l["ks"]["r"]["k"], json!(45.0));
    assert_eq!(l["ks"]["s"]["k"], json!([200.0, 100.0, 100.0]));
    ok(&mut e, "set_keyframe", json!({"layer": "root:1", "property": "rotation", "frame": 10, "value": 90}));
    ok(&mut e, "set_transform", json!({"layer": "root:1", "rotation": 120, "frame": 20}));
    let r = layer(&e, 0)["ks"]["r"].clone();
    assert_eq!(r["a"], 1);
    assert_eq!(r["k"].as_array().unwrap().len(), 2, "set_transform on an animated property adds a keyframe");
}

#[test]
fn anchor_center_keeps_visual_position() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "path", "points": [[0, 0], [100, 0], [100, 50], [0, 50]], "closed": true, "position": [10, 10]}));
    let before = e.layer_geometry("root:1", 0.0)["bounds"].clone();
    ok(&mut e, "set_anchor_point", json!({"layer": "root:1", "anchor": "center"}));
    assert_eq!(layer(&e, 0)["ks"]["a"]["k"], json!([50.0, 25.0, 0.0]));
    assert_eq!(e.layer_geometry("root:1", 0.0)["bounds"], before);
}

#[test]
fn align_and_distribute() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A", "position": [100, 100], "size": [20, 20]}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "B", "position": [150, 200], "size": [40, 40]}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "C", "position": [380, 50], "size": [20, 20]}));
    ok(&mut e, "align_layers", json!({"layers": ["A"], "align": "left", "to": "scene"}));
    assert_eq!(e.layer_geometry("root:1", 0.0)["bounds"][0], 0.0);
    ok(&mut e, "align_layers", json!({"layers": ["A", "B"], "align": "top", "to": "selection"}));
    assert_eq!(e.layer_geometry("root:1", 0.0)["bounds"][1], 90.0);
    assert_eq!(e.layer_geometry("root:2", 0.0)["bounds"][1], 90.0);
    ok(&mut e, "distribute_layers", json!({"layers": ["A", "B", "C"], "axis": "horizontal"}));
    let cx = |id: &str, e: &mut lumetrope_core::engine::Engine| {
        let b = e.layer_geometry(id, 0.0)["bounds"].clone();
        (b[0].as_f64().unwrap() + b[2].as_f64().unwrap()) / 2.0
    };
    let (a, b, c) = (cx("root:1", &mut e), cx("root:2", &mut e), cx("root:3", &mut e));
    assert!(((b - a) - (c - b)).abs() < 1e-6, "{a} {b} {c}");
}

#[test]
fn flip_negates_scale() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect"}));
    ok(&mut e, "flip_layer", json!({"layer": "root:1", "axis": "horizontal"}));
    assert_eq!(layer(&e, 0)["ks"]["s"]["k"], json!([-100.0, 100.0, 100.0]));
}

#[test]
fn null_and_solid() {
    let mut e = eng();
    let n = ok(&mut e, "create_null", json!({"name": "Ctrl"}));
    let s = ok(&mut e, "create_solid", json!({"color": "#00ff00"}));
    assert_eq!(layer(&e, 1)["ty"], 3);
    assert_eq!(layer(&e, 0)["sc"], "#00ff00");
    assert_eq!(layer(&e, 0)["sw"], 400.0);
    assert_ne!(n["id"], s["id"]);
}

#[test]
fn nudge_static_animated_and_parented() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "S", "position": [100, 100]}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A", "position": [100, 100]}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "position", "frame": 0, "value": [100, 100]}));
    ok(&mut e, "set_keyframe", json!({"layer": "A", "property": "position", "frame": 20, "value": [200, 100]}));
    ok(&mut e, "create_null", json!({"name": "P", "position": [0, 0]}));
    ok(&mut e, "set_transform", json!({"layer": "P", "scale": [200, 200]}));
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "C", "position": [10, 10]}));
    ok(&mut e, "set_parent", json!({"layer": "C", "parent": "P"}));
    ok(&mut e, "nudge_layers", json!({"layers": ["S", "A", "C"], "dx": 10, "dy": -4, "frame": 10}));
    let get = |e: &mut lumetrope_core::engine::Engine, l: &str, f: f64| ok(e, "get_property", json!({"layer": l, "property": "position", "frame": f}))["value"].clone();
    assert_eq!(get(&mut e, "S", 0.0), json!([110.0, 96.0, 0.0]));
    assert_eq!(get(&mut e, "A", 10.0)[0].as_f64().unwrap().round(), 160.0, "animated layer keyed at frame 10");
    assert_eq!(get(&mut e, "A", 0.0)[0], 100.0, "other keyframes untouched");
    assert_eq!(get(&mut e, "C", 0.0), json!([15.0, 8.0, 0.0]), "delta halved by the 200% parent");
}
