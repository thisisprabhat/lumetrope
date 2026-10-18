mod common;
use common::*;
use serde_json::json;

fn setup() -> lumetrope_core::engine::Engine {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A", "size": [100, 100]}));
    e
}

fn tys(v: &serde_json::Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x["ty"].as_str().unwrap().to_string()).collect()
}

#[test]
fn add_shape_creates_group_on_top() {
    let mut e = setup();
    let r = ok(&mut e, "add_shape", json!({"layer": "A", "type": "ellipse", "position": [30, 0], "size": [20, 20], "fill": "#0000ff"}));
    assert_eq!(r["shape"], "0");
    let g = layer(&e, 0)["shapes"][0].clone();
    assert_eq!(g["ty"], "gr");
    assert_eq!(tys(&g["it"]), vec!["el", "fl", "tr"]);
    let px = e.render_rgba(0.0, 400, 300);
    let i = (150 * 400 + 230) * 4;
    assert_eq!(&px[i..i + 3], &[0, 0, 255]);
}

#[test]
fn modifiers_go_before_group_transform() {
    let mut e = setup();
    ok(&mut e, "add_shape", json!({"layer": "A", "type": "rect"}));
    ok(&mut e, "add_trim_path", json!({"layer": "A", "shape": "0", "end": 50}));
    ok(&mut e, "add_round_corners", json!({"layer": "A", "radius": 8}));
    ok(&mut e, "add_repeater", json!({"layer": "A", "copies": 4, "position": [30, 0]}));
    let l = layer(&e, 0);
    assert_eq!(tys(&l["shapes"][0]["it"]), vec!["rc", "fl", "tm", "tr"]);
    let top = tys(&l["shapes"]);
    assert_eq!(&top[top.len() - 2..], &["rd".to_string(), "rp".to_string()]);
    assert_eq!(l["shapes"][4]["tr"]["p"]["k"], json!([30.0, 0.0]));
}

#[test]
fn set_path_and_update_and_remove_item() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "path", "points": [[0,0],[10,0],[10,10]]}));
    ok(&mut e, "set_path", json!({"layer": "root:1", "shape": "0", "path": {"v": [[0,0],[50,0],[50,50],[0,50]], "c": true}}));
    assert_eq!(layer(&e, 0)["shapes"][0]["ks"]["k"]["v"].as_array().unwrap().len(), 4);
    ok(&mut e, "update_shape_item", json!({"layer": "root:1", "shape": "1", "props": {"opacity": 40}, "name": "Paint", "fill_rule": "even_odd"}));
    let f = layer(&e, 0)["shapes"][1].clone();
    assert_eq!((f["o"]["k"].as_f64(), f["nm"].as_str(), f["r"].as_i64()), (Some(40.0), Some("Paint"), Some(2)));
    ok(&mut e, "remove_shape_item", json!({"layer": "root:1", "shape": "1"}));
    assert_eq!(layer(&e, 0)["shapes"].as_array().unwrap().len(), 1);
    assert!(fail(&mut e, "remove_shape_item", json!({"layer": "root:1", "shape": "7"})).contains("not found"));
}

#[test]
fn reorder_shape_within_parent() {
    let mut e = setup();
    ok(&mut e, "add_shape", json!({"layer": "A", "type": "ellipse", "name": "E"}));
    ok(&mut e, "reorder_shape", json!({"layer": "A", "shape": "0", "index": 2}));
    assert_eq!(layer(&e, 0)["shapes"][2]["nm"], "E");
}

#[test]
fn set_fill_updates_or_adds_and_stroke_goes_above() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "stroke": "#000000", "fill": null}));
    ok(&mut e, "set_fill", json!({"layer": "root:1", "color": "#00ff00", "opacity": 80}));
    let l = layer(&e, 0);
    assert_eq!(tys(&l["shapes"]), vec!["rc", "st", "fl"], "fill added below the stroke");
    assert_eq!(l["shapes"][2]["o"]["k"], 80.0);
    ok(&mut e, "set_fill", json!({"layer": "root:1", "color": "#ff0000"}));
    assert_eq!(layer(&e, 0)["shapes"][2]["c"]["k"], json!([1.0, 0.0, 0.0, 1.0]));
    ok(&mut e, "set_stroke", json!({"layer": "root:1", "width": 9, "line_cap": "square", "dash": [4, 2]}));
    let st = layer(&e, 0)["shapes"][1].clone();
    assert_eq!((st["w"]["k"].as_f64(), st["lc"].as_i64()), (Some(9.0), Some(3)));
    assert_eq!(st["d"].as_array().unwrap().len(), 3);
}

#[test]
fn gradient_replaces_solid_fill_and_renders() {
    let mut e = setup();
    ok(&mut e, "set_gradient", json!({"layer": "A", "kind": "fill", "type": "linear",
        "stops": [{"offset": 0, "color": "#000000"}, {"offset": 1, "color": "#ffffff", "opacity": 0.5}]}));
    let g = layer(&e, 0)["shapes"][1].clone();
    assert_eq!(g["ty"], "gf");
    assert_eq!(g["g"]["p"], 2);
    assert_eq!(g["g"]["k"]["k"], json!([0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 0.5]));
    let px = e.render_rgba(0.0, 400, 300);
    assert!(px[(150 * 400 + 160) * 4] < px[(150 * 400 + 240) * 4]);
}

#[test]
fn remove_paint_kinds() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "stroke": "#000000", "fill": "#ffffff"}));
    ok(&mut e, "remove_paint", json!({"layer": "root:1", "kind": "stroke"}));
    assert_eq!(tys(&layer(&e, 0)["shapes"]), vec!["rc", "fl"]);
    ok(&mut e, "remove_paint", json!({"layer": "root:1", "kind": "all"}));
    assert_eq!(tys(&layer(&e, 0)["shapes"]), vec!["rc"]);
}
