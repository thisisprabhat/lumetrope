use lumetrope_core::engine::Engine;
use lumetrope_core::project;
use serde_json::json;

fn lottie() -> serde_json::Value {
    json!({"v":"5.12.0","fr":24,"ip":0,"op":48,"w":64,"h":32,"nm":"x","layers":[
        {"ty":4,"nm":"a","ind":1,"ip":0,"op":48,"st":0,"lmeta":{"lock":true},
         "ks":{"p":{"a":0,"k":[32,16]}},
         "shapes":[{"ty":"rc","p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[64,32]},"r":{"a":0,"k":0}},
                   {"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":50}}]}],"customField":42})
}

#[test]
fn project_from_lottie_wraps_main() {
    let p = project::from_lottie(lottie());
    assert_eq!(project::active_id(&p), "main");
    assert_eq!(project::active_anim(&p)["w"], 64);
    assert!(p["stateMachines"].is_object() && p["themes"].is_object());
}

#[test]
fn load_json_reports_info() {
    let mut e = Engine::new();
    let info = e.load_file("a.json", lottie().to_string().as_bytes()).unwrap();
    assert_eq!(info["w"], 64.0);
    assert_eq!(info["fr"], 24.0);
    assert_eq!(info["op"], 48.0);
    assert_eq!(info["layers"], 1);
}

#[test]
fn garbage_input_is_error_not_panic() {
    let mut e = Engine::new();
    assert!(e.load_file("a.json", b"{not json").is_err());
    assert!(e.load_file("a.json", b"[1,2,3]").is_err());
    assert!(e.load_file("a.bin", b"\x00\x01").is_err());
    assert!(e.set_project_json("{}").is_err());
}

#[test]
fn render_returns_straight_alpha_rgba_of_size() {
    let mut e = Engine::new();
    e.load_file("a.json", lottie().to_string().as_bytes()).unwrap();
    let px = e.render_rgba(0.0, 64, 32);
    assert_eq!(px.len(), 64 * 32 * 4);
    let c = &px[(16 * 64 + 32) * 4..(16 * 64 + 32) * 4 + 4];
    assert_eq!(c[0], 255, "straight (not premultiplied) red");
    assert!((120..=135).contains(&c[3]));
}

#[test]
fn export_strips_lmeta_and_keeps_unknown_fields() {
    let mut e = Engine::new();
    e.load_file("a.json", lottie().to_string().as_bytes()).unwrap();
    let out: serde_json::Value = serde_json::from_str(&e.export_lottie()).unwrap();
    assert!(out["layers"][0].get("lmeta").is_none());
    assert_eq!(out["customField"], 42);
}

#[test]
fn set_project_json_bumps_revision_and_roundtrips() {
    let mut e = Engine::new();
    let r0 = e.revision();
    let p = project::from_lottie(lottie());
    e.set_project_json(&p.to_string()).unwrap();
    assert!(e.revision() > r0);
    let back: serde_json::Value = serde_json::from_str(&e.project_json()).unwrap();
    assert_eq!(back, p);
}

#[test]
fn new_project_has_requested_settings() {
    let mut e = Engine::new();
    e.new_project(800.0, 600.0, 60.0, 120.0);
    let i = e.info();
    assert_eq!((i["w"].as_f64(), i["h"].as_f64(), i["fr"].as_f64(), i["op"].as_f64()), (Some(800.0), Some(600.0), Some(60.0), Some(120.0)));
}

#[test]
fn engine_apply_undo_redo() {
    let mut e = Engine::new();
    let r = e.apply("set_scene_settings", &json!({"width": 300}));
    assert_eq!(r["ok"], json!({"ok": true}));
    assert_eq!(e.info()["w"], 300.0);
    assert!(e.can_undo());
    assert!(e.undo());
    assert_eq!(e.info()["w"], 512.0);
    assert!(e.redo());
    assert_eq!(e.info()["w"], 300.0);
    let bad = e.apply("nope", &json!({}));
    assert!(bad["error"].as_str().unwrap().contains("nope"));
    assert!(e.catalog().as_array().unwrap().len() >= 1);
}

#[test]
fn loading_a_file_clears_history() {
    let mut e = Engine::new();
    e.apply("set_scene_settings", &json!({"width": 300}));
    e.load_file("a.json", lottie().to_string().as_bytes()).unwrap();
    assert!(!e.can_undo());
}

#[test]
fn engine_hit_test_and_geometry() {
    let mut e = Engine::new();
    e.load_file("a.json", lottie().to_string().as_bytes()).unwrap();
    assert_eq!(e.hit_test(32.0, 16.0, 0.0).as_deref(), Some("root:1"));
    assert_eq!(e.hit_test(500.0, 500.0, 0.0), None);
    let g = e.layer_geometry("root:1", 0.0);
    assert_eq!(g["bounds"], json!([0.0, 0.0, 64.0, 32.0]));
    assert!(e.layer_geometry("root:99", 0.0).is_null());
}

#[test]
fn batch_groups_many_ops_into_one_undo_step() {
    let mut e = Engine::new();
    e.begin_batch();
    for w in [100, 200, 300] {
        e.apply("set_scene_settings", &json!({"width": w}));
    }
    e.end_batch();
    assert_eq!(e.info()["w"], 300.0);
    assert!(e.undo());
    assert_eq!(e.info()["w"], 512.0);
    assert!(!e.can_undo());
    e.begin_batch();
    e.end_batch();
    assert!(!e.can_undo(), "empty batch records nothing");
}

#[test]
fn view_scene_renders_and_hit_tests_a_nested_scene() {
    let mut e = Engine::new();
    e.new_project(200.0, 100.0, 30.0, 30.0);
    let r = e.apply("create_scene", &json!({"name": "Card", "width": 50, "height": 50}));
    let id = r["ok"]["scene"].as_str().unwrap().to_string();
    e.apply("create_shape", &json!({"type": "rect", "scene": id, "position": [25, 25], "size": [50, 50], "fill": "#00ff00"}));
    assert_eq!(e.render_rgba(0.0, 200, 100)[(50 * 200 + 100) * 4 + 3], 0, "root has nothing");
    e.set_view_scene(&id);
    assert_eq!((e.info()["w"].as_f64(), e.info()["view"].as_str()), (Some(50.0), Some(id.as_str())));
    let px = e.render_rgba(0.0, 50, 50);
    assert_eq!(&px[(25 * 50 + 25) * 4..(25 * 50 + 25) * 4 + 4], &[0, 255, 0, 255]);
    assert_eq!(e.hit_test(25.0, 25.0, 0.0), Some(format!("{id}:1")));
    e.set_view_scene("missing");
    assert_eq!(e.info()["view"], "root", "unknown scenes fall back to root");
}

#[test]
fn render_view_maps_camera_and_clips_to_artboard() {
    let mut e = Engine::new();
    e.load_file("a.json", lottie().to_string().as_bytes()).unwrap();
    // 64×32 red rect filling the comp; camera: scale 2, offset (10, 5) in a 200×100 viewport
    let px = e.render_view_rgba(0.0, 200, 100, 2.0, 10.0, 5.0);
    assert_eq!(px.len(), 200 * 100 * 4);
    let at = |x: usize, y: usize| px[(y * 200 + x) * 4..(y * 200 + x) * 4 + 4].to_vec();
    assert_eq!(at(10 + 64, 5 + 32)[0], 255, "comp centre (32,16) → screen (74,37)");
    assert_eq!(at(5, 2)[3], 0, "outside the artboard is transparent");
    assert_eq!(at(10 + 128 + 3, 20)[3], 0, "right of the artboard is clipped");
}
