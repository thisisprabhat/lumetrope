mod common;
use base64::Engine as _;
use common::*;
use serde_json::json;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50">
  <rect x="0" y="0" width="50" height="50" fill="#ff0000"/>
  <circle cx="75" cy="25" r="20" fill="none" stroke="#0000ff" stroke-width="4"/>
  <defs><linearGradient id="g"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs>
  <path d="M60 0 L100 0 L100 10 Z" fill="url(#g)"/>
</svg>"##;

#[test]
fn import_svg_creates_shape_layer() {
    let mut e = eng();
    let r = ok(&mut e, "import_svg", json!({"svg": SVG, "name": "Logo"}));
    assert_eq!(r["id"], "root:1");
    let l = layer(&e, 0);
    assert_eq!((l["ty"].as_i64(), l["nm"].as_str()), (Some(4), Some("Logo")));
    let s = l.to_string();
    assert!(s.contains("\"gf\"") && s.contains("\"st\"") && s.contains("\"fl\""), "has gradient, stroke and fill");
    // centred in the 400×300 scene: rect occupies x 150..200, y 125..175
    let px = e.render_rgba(0.0, 400, 300);
    let i = (150 * 400 + 175) * 4;
    assert_eq!(&px[i..i + 4], &[255, 0, 0, 255]);
    assert!(fail(&mut e, "import_svg", json!({"svg": "<not svg"})).to_lowercase().contains("svg"));
}

#[test]
fn import_lottie_as_nested_scene_with_remapped_assets() {
    let mut e = eng();
    let inner = json!({"v":"5.12.0","fr":25,"ip":0,"op":50,"w":100,"h":100,"nm":"Spinner",
        "assets":[{"id":"comp_1","layers":[]}],
        "layers":[{"ty":0,"ind":1,"refId":"comp_1","ip":0,"op":50,"st":0,"w":100,"h":100,"ks":{}},
                  {"ty":4,"ind":2,"ip":0,"op":50,"st":0,"ks":{"p":{"a":0,"k":[50,50]}},"shapes":[{"ty":"rc","p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[40,40]},"r":{"a":0,"k":0}},{"ty":"fl","c":{"a":0,"k":[0,1,0,1]},"o":{"a":0,"k":100}}]}]});
    ok(&mut e, "create_scene", json!({"name": "existing"}));
    let r = ok(&mut e, "import_lottie", json!({"json": inner.to_string()}));
    let a = anim(&e);
    let assets = a["assets"].as_array().unwrap();
    let ids: Vec<&str> = assets.iter().map(|x| x["id"].as_str().unwrap()).collect();
    let mut dedup = ids.clone();
    dedup.sort();
    dedup.dedup();
    assert_eq!(dedup.len(), ids.len(), "asset ids stay unique: {ids:?}");
    let scene = r["scene"].as_str().unwrap();
    let sc = assets.iter().find(|x| x["id"] == scene).unwrap();
    assert_eq!(sc["fr"], 25.0);
    assert_ne!(sc["layers"][0]["refId"], "comp_1", "inner refIds remapped");
    assert_eq!(layer(&e, 0)["ty"], 0);
    let px = e.render_rgba(0.0, 400, 300);
    assert_eq!(&px[(150 * 400 + 200) * 4..(150 * 400 + 200) * 4 + 4], &[0, 255, 0, 255]);
}

#[test]
fn import_image_adds_asset_and_layer() {
    let mut img = tiny_skia::Pixmap::new(20, 10).unwrap();
    img.fill(tiny_skia::Color::from_rgba8(255, 0, 255, 255));
    let uri = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(img.encode_png().unwrap()));
    let mut e = eng();
    ok(&mut e, "import_image", json!({"data": uri, "name": "Pic"}));
    let a = anim(&e);
    assert_eq!((a["assets"][0]["w"].as_f64(), a["assets"][0]["h"].as_f64()), (Some(20.0), Some(10.0)));
    assert_eq!(layer(&e, 0)["ty"], 2);
    let px = e.render_rgba(0.0, 400, 300);
    assert_eq!(&px[(150 * 400 + 200) * 4..(150 * 400 + 200) * 4 + 4], &[255, 0, 255, 255]);
    assert!(fail(&mut e, "import_image", json!({"data": "data:image/png;base64,AAAA"})).contains("image"));
}

#[test]
fn motion_preset_fade_in_writes_opacity_keys() {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A"}));
    ok(&mut e, "apply_motion_preset", json!({"layer": "A", "preset": "fade_in", "start_frame": 5, "duration_frames": 10}));
    let o = layer(&e, 0)["ks"]["o"]["k"].clone();
    assert_eq!((o[0]["t"].as_f64(), o[0]["s"].clone()), (Some(5.0), json!([0.0])));
    assert_eq!((o[1]["t"].as_f64(), o[1]["s"].clone()), (Some(15.0), json!([100.0])));
}

#[test]
fn every_preset_applies() {
    let presets = ["fade_in", "fade_out", "scale_in", "scale_out", "pop", "slide_in_left", "slide_in_right", "slide_in_up", "slide_in_down",
        "slide_out_left", "slide_out_right", "slide_out_up", "slide_out_down", "spin", "bounce", "pulse", "shake", "wiggle", "float", "blink"];
    for p in presets {
        let mut e = eng();
        ok(&mut e, "create_shape", json!({"type": "rect", "name": "A"}));
        ok(&mut e, "apply_motion_preset", json!({"layer": "A", "preset": p}));
        let r = ok(&mut e, "read_keyframes", json!({"layer": "A"}));
        assert!(!r["animated"].as_array().unwrap().is_empty(), "{p} animates something");
    }
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "A"}));
    assert!(fail(&mut e, "apply_motion_preset", json!({"layer": "A", "preset": "teleport"})).contains("teleport"));
}

#[test]
fn stagger_offsets_each_layer() {
    let mut e = eng();
    for n in ["A", "B", "C"] {
        ok(&mut e, "create_shape", json!({"type": "rect", "name": n}));
    }
    ok(&mut e, "stagger_layers", json!({"layers": ["A", "B", "C"], "preset": "fade_in", "start_frame": 0, "delay_frames": 4, "duration_frames": 10}));
    let first_t = |idx: usize| layer(&e, idx)["ks"]["o"]["k"][0]["t"].as_f64().unwrap();
    assert_eq!((first_t(2), first_t(1), first_t(0)), (0.0, 4.0, 8.0));
}
