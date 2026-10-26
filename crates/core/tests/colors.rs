mod common;
use common::*;
use serde_json::{json, Value};

fn hexes(e: &mut lumetrope_core::engine::Engine) -> Vec<String> {
    ok(e, "list_colors", json!({})).as_array().unwrap().iter().map(|c| c["hex"].as_str().unwrap().to_string()).collect()
}

fn items_of_type<'a>(v: &'a Value, ty: &str, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(o) => {
            if o.get("ty").and_then(Value::as_str) == Some(ty) {
                out.push(v);
            }
            o.values().for_each(|x| items_of_type(x, ty, out));
        }
        Value::Array(a) => a.iter().for_each(|x| items_of_type(x, ty, out)),
        _ => {}
    }
}

#[test]
fn list_colors_includes_gradient_stops_and_effect_colours() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "fill": "#ff0000"}));
    ok(&mut e, "set_gradient", json!({"layer": "root:1", "stops": [{"offset": 0, "color": "#112233"}, {"offset": 1, "color": "#445566"}]}));
    ok(&mut e, "add_effect", json!({"layer": "root:1", "type": "fill", "params": {"color": "#00ff00"}}));
    let h = hexes(&mut e);
    for c in ["#112233", "#445566", "#00ff00"] {
        assert!(h.contains(&c.to_string()), "{c} missing from {h:?}");
    }
}

#[test]
fn apply_color_map_swaps_simultaneously_and_is_one_undo_step() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "fill": "#ff0000"}));
    ok(&mut e, "create_shape", json!({"type": "ellipse", "fill": "#0000ff", "stroke": "#ff0000"}));
    ok(&mut e, "set_gradient", json!({"layer": "root:1", "kind": "stroke", "stops": [{"offset": 0, "color": "#ff0000"}, {"offset": 1, "color": "#0000ff", "opacity": 0.5}]}));
    let r = ok(&mut e, "apply_color_map", json!({"map": {"#ff0000": "#0000ff", "#0000ff": "#ff0000"}}));
    assert!(r["replaced"].as_u64().unwrap() >= 4);
    let mut grads = vec![];
    let a = anim(&e);
    items_of_type(&a, "gs", &mut grads);
    let k: Vec<f64> = grads[0]["g"]["k"]["k"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    // Stops swapped (not cascaded to all one colour); opacity stops untouched.
    assert_eq!(&k[1..4], &[0.0, 0.0, 1.0]);
    assert_eq!(&k[5..8], &[1.0, 0.0, 0.0]);
    assert_eq!(k[k.len() - 1], 0.5);
    let mut set = hexes(&mut e);
    set.sort();
    assert_eq!(set, vec!["#0000ff", "#ff0000"]);
    assert!(e.undo());
    assert!(hexes(&mut e).contains(&"#ff0000".to_string()));
    let fills: Vec<String> = {
        let a = anim(&e);
        let mut f = vec![];
        items_of_type(&a, "fl", &mut f);
        f.iter().map(|x| lumetrope_core::ops::util::hex(&x["c"]["k"].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect::<Vec<_>>())).collect()
    };
    assert!(fills.contains(&"#ff0000".to_string()) && fills.contains(&"#0000ff".to_string()));
}

#[test]
fn apply_color_map_keeps_alpha_and_rejects_bad_maps() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "fill": "#ff0000"}));
    ok(&mut e, "apply_color_map", json!({"map": {"#f00": "#00ff00"}}));
    let mut f = vec![];
    let a = anim(&e);
    items_of_type(&a, "fl", &mut f);
    let c: Vec<f64> = f[0]["c"]["k"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert_eq!(&c[..3], &[0.0, 1.0, 0.0]);
    assert_eq!(c.len(), 4);
    fail(&mut e, "apply_color_map", json!({"map": ["#fff"]}));
    fail(&mut e, "apply_color_map", json!({"map": {"#fff": "nope"}}));
}

#[test]
fn outline_only_turns_fills_into_strokes() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "fill": "#ff0000"}));
    ok(&mut e, "create_shape", json!({"type": "ellipse", "fill": "#00ff00", "stroke": "#0000ff"}));
    ok(&mut e, "create_solid", json!({"color": "#336699"}));
    ok(&mut e, "create_text", json!({"text": "Hi"}));
    let r = ok(&mut e, "outline_only", json!({}));
    assert_eq!(r["fills"], 2);
    let a = anim(&e);
    let (mut fl, mut st) = (vec![], vec![]);
    items_of_type(&a, "fl", &mut fl);
    items_of_type(&a, "st", &mut st);
    assert!(fl.is_empty(), "fills left: {fl:?}");
    // The rect gains one outline; the ellipse keeps its single (recoloured) stroke.
    assert_eq!(st.len(), 2);
    for s in &st {
        assert_eq!(&s["c"]["k"].as_array().unwrap()[..3], &[json!(0.0), json!(0.0), json!(0.0)]);
    }
    let mut set = hexes(&mut e);
    set.sort();
    assert_eq!(set, vec!["#000000", "#ffffff"]);
    assert_eq!(e.project()["settings"]["background"]["main"], "#ffffff");
    e.undo();
    assert!(hexes(&mut e).contains(&"#ff0000".to_string()));
}

#[test]
fn outline_only_inverted_and_skips_matte_sources() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "fill": "#ff0000", "name": "Matte"}));
    ok(&mut e, "create_shape", json!({"type": "ellipse", "fill": "#00ff00", "name": "Art"}));
    ok(&mut e, "set_matte", json!({"layer": "Art", "matte_layer": "Matte", "mode": "alpha"}));
    ok(&mut e, "outline_only", json!({"line": "#ffffff", "background": "#000000", "width": 3}));
    let a = anim(&e);
    let matte = a["layers"].as_array().unwrap().iter().find(|l| l["nm"] == "Matte").unwrap().clone();
    let art = a["layers"].as_array().unwrap().iter().find(|l| l["nm"] == "Art").unwrap().clone();
    let (mut mf, mut ast) = (vec![], vec![]);
    items_of_type(&matte, "fl", &mut mf);
    items_of_type(&art, "st", &mut ast);
    assert_eq!(mf.len(), 1, "matte source keeps its fill");
    assert_eq!(ast[0]["w"]["k"], 3.0);
    assert_eq!(&ast[0]["c"]["k"].as_array().unwrap()[..3], &[json!(1.0), json!(1.0), json!(1.0)]);
}

#[test]
fn recolouring_reaches_slot_bound_colours() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "fill": "#ff0000", "name": "Box"}));
    ok(&mut e, "create_slot", json!({"layer": "Box", "shape": "1", "property": "color", "slot_id": "brand"}));
    let a = anim(&e);
    assert!(a["slots"]["brand"].is_object(), "slot created: {}", a["slots"]);
    ok(&mut e, "apply_color_map", json!({"map": {"#ff0000": "#00ff00"}}));
    let a = anim(&e);
    assert_eq!(&a["slots"]["brand"]["p"]["k"].as_array().unwrap()[..3], &[json!(0.0), json!(1.0), json!(0.0)]);
}

#[test]
fn outline_only_resets_blend_modes_and_disables_blur() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "ellipse", "fill": "#3366ff", "name": "Glow"}));
    ok(&mut e, "set_blend_mode", json!({"layer": "Glow", "mode": "screen"}));
    ok(&mut e, "add_effect", json!({"layer": "Glow", "type": "gaussian_blur"}));
    assert_ne!(layer(&e, 0)["bm"], 0);
    ok(&mut e, "outline_only", json!({}));
    let l = layer(&e, 0);
    assert_eq!(l["bm"], 0);
    assert_eq!(l["ef"][0]["en"], 0);
}
