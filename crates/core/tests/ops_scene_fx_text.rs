mod common;
use common::*;
use serde_json::json;

#[test]
fn read_scene_lists_layers_with_ids_types_and_shapes() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "Box"}));
    ok(&mut e, "create_text", json!({"text": "Hi"}));
    let s = ok(&mut e, "read_scene", json!({}));
    assert_eq!((s["width"].as_f64(), s["fps"].as_f64(), s["duration_frames"].as_f64()), (Some(400.0), Some(30.0), Some(60.0)));
    let layers = s["layers"].as_array().unwrap();
    assert_eq!(layers[0]["type"], "text");
    assert_eq!(layers[1]["id"], "root:1");
    assert_eq!(layers[1]["name"], "Box");
    assert_eq!(layers[1]["shapes"][0]["path"], "0");
    assert_eq!(layers[1]["shapes"][0]["type"], "rect");
    let il = ok(&mut e, "inspect_layer", json!({"layer": "Box"}));
    assert_eq!(il["id"], "root:1");
    assert_eq!(il["json"]["nm"], "Box");
    assert!(il["bounds"].is_array());
}

#[test]
fn scenes_create_and_list() {
    let mut e = eng();
    let r = ok(&mut e, "create_scene", json!({"name": "Card", "add_to_root": true}));
    let id = r["scene"].as_str().unwrap().to_string();
    ok(&mut e, "create_shape", json!({"type": "rect", "scene": id}));
    let l = ok(&mut e, "list_scenes", json!({}));
    let card = l.as_array().unwrap().iter().find(|s| s["id"] == json!(id)).unwrap().clone();
    assert_eq!(card["layers"], 1);
    assert_eq!(card["name"], "Card");
    assert_eq!(layer(&e, 0)["ty"], 0);
}

#[test]
fn background_list_and_replace_colors() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "fill": "#ff0000"}));
    ok(&mut e, "create_shape", json!({"type": "ellipse", "fill": "#ff0000", "stroke": "#00ff00"}));
    ok(&mut e, "set_background", json!({"color": "#101010"}));
    assert_eq!(e.project()["settings"]["background"]["main"], "#101010");
    let c = ok(&mut e, "list_colors", json!({}));
    let red = c.as_array().unwrap().iter().find(|x| x["hex"] == "#ff0000").unwrap().clone();
    assert_eq!(red["count"], 2);
    let r = ok(&mut e, "replace_color", json!({"from": "#ff0000", "to": "#0000ff"}));
    assert_eq!(r["replaced"], 2);
    assert!(ok(&mut e, "list_colors", json!({})).as_array().unwrap().iter().all(|x| x["hex"] != "#ff0000"));
}

#[test]
fn masks_add_update_remove() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "size": [200, 200]}));
    let m = ok(&mut e, "add_mask", json!({"layer": "root:1", "shape": "ellipse", "size": [50, 50], "mode": "subtract"}));
    assert_eq!(m["index"], 0);
    let l = layer(&e, 0);
    assert_eq!(l["masksProperties"][0]["mode"], "s");
    assert_eq!(l["hasMask"], true);
    assert_eq!(e.render_rgba(0.0, 400, 300)[(150 * 400 + 200) * 4 + 3], 0, "centre cut out");
    ok(&mut e, "update_mask", json!({"layer": "root:1", "index": 0, "inverted": true, "mode": "add"}));
    assert_eq!(layer(&e, 0)["masksProperties"][0]["inv"], true);
    ok(&mut e, "remove_mask", json!({"layer": "root:1", "index": 0}));
    assert!(layer(&e, 0).get("hasMask").is_none());
}

#[test]
fn effects_add_update_list_remove() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect"}));
    ok(&mut e, "add_effect", json!({"layer": "root:1", "type": "drop_shadow", "params": {"distance": 12, "opacity": 50}}));
    ok(&mut e, "add_effect", json!({"layer": "root:1", "type": "gaussian_blur", "params": {"blurriness": 4}}));
    let fx = layer(&e, 0)["ef"].clone();
    assert_eq!(fx[0]["ty"], 25);
    assert_eq!(fx[0]["ef"][3]["v"]["k"], 12.0);
    assert_eq!(fx[0]["ef"][1]["v"]["k"], 127.5);
    ok(&mut e, "update_effect", json!({"layer": "root:1", "index": 1, "params": {"blurriness": 9}}));
    assert_eq!(layer(&e, 0)["ef"][1]["ef"][0]["v"]["k"], 9.0);
    let list = ok(&mut e, "list_effects", json!({"layer": "root:1"}));
    assert_eq!(list[1]["type"], "gaussian_blur");
    assert_eq!(list[0]["params"]["distance"], 12.0);
    ok(&mut e, "remove_effect", json!({"layer": "root:1", "index": 0}));
    assert_eq!(layer(&e, 0)["ef"].as_array().unwrap().len(), 1);
    ok(&mut e, "set_keyframe", json!({"layer": "root:1", "shape": "effect:0", "property": "blurriness", "frame": 10, "value": 2}));
    assert_eq!(layer(&e, 0)["ef"][0]["ef"][0]["v"]["a"], 1);
    for t in ["fill", "tint", "tritone"] {
        ok(&mut e, "add_effect", json!({"layer": "root:1", "type": t}));
    }
    assert!(fail(&mut e, "add_effect", json!({"layer": "root:1", "type": "glow"})).contains("type"));
}

#[test]
fn text_create_set_and_style() {
    let mut e = eng();
    let r = ok(&mut e, "create_text", json!({"text": "Hello", "font_size": 48, "color": "#ff0000", "justify": "center"}));
    assert_eq!(r["id"], "root:1");
    let l = layer(&e, 0);
    assert_eq!(l["ty"], 5);
    let d = l["t"]["d"]["k"][0]["s"].clone();
    assert_eq!((d["t"].as_str(), d["s"].as_f64(), d["j"].as_i64()), (Some("Hello"), Some(48.0), Some(2)));
    assert_eq!(d["fc"], json!([1.0, 0.0, 0.0]));
    assert!(anim(&e)["fonts"]["list"].as_array().unwrap().iter().any(|f| f["fName"] == d["f"]));
    ok(&mut e, "set_text", json!({"layer": "root:1", "text": "World"}));
    ok(&mut e, "set_text_style", json!({"layer": "root:1", "font_size": 20, "tracking": 50, "stroke_color": "#000000", "stroke_width": 2, "caps": "all_caps"}));
    let d = layer(&e, 0)["t"]["d"]["k"][0]["s"].clone();
    assert_eq!((d["t"].as_str(), d["s"].as_f64(), d["tr"].as_f64(), d["ca"].as_i64()), (Some("World"), Some(20.0), Some(50.0), Some(1)));
    let px = e.render_rgba(0.0, 400, 300);
    assert!(px.chunks(4).filter(|p| p[3] > 128).count() > 50, "text renders");
    assert!(fail(&mut e, "set_text", json!({"layer": "root:1"})).contains("text"));
}
