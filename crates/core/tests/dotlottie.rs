mod common;
use base64::Engine as _;
use common::*;
use lumetrope_core::dotlottie;
use lumetrope_core::engine::Engine;
use serde_json::{json, Value};
use std::io::Write;

fn png_uri() -> String {
    let mut img = tiny_skia::Pixmap::new(4, 4).unwrap();
    img.fill(tiny_skia::Color::from_rgba8(0, 0, 255, 255));
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(img.encode_png().unwrap()))
}

fn rich_engine() -> Engine {
    let mut e = eng();
    ok(&mut e, "create_shape", json!({"type": "rect", "name": "Button", "fill": "#ff0000", "size": [400, 300]}));
    ok(&mut e, "import_image", json!({"data": png_uri(), "name": "Pic"}));
    ok(&mut e, "set_layer_lock", json!({"layer": "Pic", "locked": true}));
    ok(&mut e, "create_slot", json!({"layer": "Button", "shape": "1", "property": "color", "slot_id": "brand"}));
    let t = ok(&mut e, "add_theme", json!({"name": "Dark", "id": "dark"}));
    assert_eq!(t["theme"], "dark");
    ok(&mut e, "update_theme_rule", json!({"theme": "dark", "slot_id": "brand", "value": "#00ff00"}));
    ok(&mut e, "add_segment", json!({"name": "idle", "start": 0, "end": 30}));
    ok(&mut e, "add_state_machine", json!({"id": "sm1", "name": "Main"}));
    ok(&mut e, "add_input", json!({"machine": "sm1", "name": "on", "type": "boolean"}));
    ok(&mut e, "add_state", json!({"machine": "sm1", "name": "Idle", "segment": "idle"}));
    ok(&mut e, "set_background", json!({"color": "#102030"}));
    e
}

fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut b = vec![];
            std::io::copy(&mut f, &mut b).unwrap();
            (f.name().to_string(), b)
        })
        .collect()
}

#[test]
fn export_strips_lmeta_and_validates_manifest() {
    let e = rich_engine();
    let bytes = dotlottie::write(e.project()).unwrap();
    let files = unzip(&bytes);
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert!(names.contains(&"manifest.json") && names.contains(&"a/main.json") && names.contains(&"s/sm1.json") && names.contains(&"t/dark.json"), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("i/")), "image moved to i/: {names:?}");
    let manifest: Value = serde_json::from_slice(&files.iter().find(|f| f.0 == "manifest.json").unwrap().1).unwrap();
    assert_eq!(manifest["version"], "2");
    for k in manifest.as_object().unwrap().keys() {
        assert!(["version", "generator", "animations", "themes", "stateMachines", "initial"].contains(&k.as_str()), "unexpected manifest key {k}");
    }
    assert_eq!(manifest["animations"][0]["id"], "main");
    assert_eq!(manifest["animations"][0]["background"], 0x102030ff_u32);
    assert_eq!(manifest["stateMachines"][0], json!({"id": "sm1", "name": "Main"}));
    assert_eq!(manifest["themes"][0], json!({"id": "dark", "name": "Dark"}));
    for (name, b) in &files {
        let s = String::from_utf8_lossy(b);
        if name.ends_with(".json") {
            assert!(!s.contains("lmeta"), "{name} leaks editor metadata");
        }
    }
    let anim: Value = serde_json::from_slice(&files.iter().find(|f| f.0 == "a/main.json").unwrap().1).unwrap();
    let img = anim["assets"].as_array().unwrap().iter().find(|a| a.get("w").is_some() && a.get("layers").is_none()).unwrap();
    assert_eq!((img["u"].as_str(), img["e"].as_i64()), (Some("/i/"), Some(0)));
    let theme: Value = serde_json::from_slice(&files.iter().find(|f| f.0 == "t/dark.json").unwrap().1).unwrap();
    assert_eq!(theme["rules"][0]["id"], "brand");
}

#[test]
fn roundtrip_v2_project() {
    let e = rich_engine();
    let bytes = dotlottie::write(e.project()).unwrap();
    let mut e2 = Engine::new();
    e2.load_file("x.lottie", &bytes).unwrap();
    let p = e2.project();
    assert_eq!(p["stateMachines"]["sm1"]["states"][0]["name"], "Idle");
    assert_eq!(p["stateMachines"]["sm1"]["lmeta"]["name"], "Main");
    assert_eq!(p["themes"]["dark"]["name"], "Dark");
    assert_eq!(p["themes"]["dark"]["rules"][0]["id"], "brand");
    assert_eq!(p["settings"]["background"]["main"], "#102030");
    let a = &p["animations"]["main"];
    assert_eq!(a["markers"][0]["cm"], "idle");
    assert!(a["assets"].as_array().unwrap().iter().any(|x| x["p"].as_str().map(|s| s.starts_with("data:image/png")).unwrap_or(false)), "image re-embedded");
    let px = e2.render_rgba(0.0, 400, 300);
    assert_eq!(&px[(150 * 400 + 200) * 4..(150 * 400 + 200) * 4 + 4], &[0, 0, 255, 255], "image layer renders after round trip");
}

#[test]
fn reads_v1_manifest() {
    let mut buf = std::io::Cursor::new(vec![]);
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default();
        w.start_file("manifest.json", o).unwrap();
        w.write_all(br#"{"version":"1.0","generator":"x","animations":[{"id":"hello","speed":1}]}"#).unwrap();
        w.start_file("animations/hello.json", o).unwrap();
        w.write_all(br#"{"v":"5.7.0","fr":24,"ip":0,"op":10,"w":64,"h":64,"layers":[]}"#).unwrap();
        w.finish().unwrap();
    }
    let mut e = Engine::new();
    let info = e.load_file("old.lottie", &buf.into_inner()).unwrap();
    assert_eq!(info["active"], "hello");
    assert_eq!(info["w"], 64.0);
}

#[test]
fn garbage_lottie_is_error() {
    let mut e = Engine::new();
    assert!(e.load_file("x.lottie", b"PK\x03\x04garbage").is_err());
    assert!(e.load_file("x.lottie", b"not a zip").is_err());
}

#[test]
fn theme_color_rule_overrides_slot() {
    let mut e = rich_engine();
    ok(&mut e, "delete_layer", json!({"layer": "Pic"}));
    let red = e.render_rgba(0.0, 400, 300)[(150 * 400 + 200) * 4..(150 * 400 + 200) * 4 + 3].to_vec();
    assert_eq!(red, vec![255, 0, 0]);
    ok(&mut e, "select_theme", json!({"theme": "dark"}));
    let green = e.render_rgba(0.0, 400, 300)[(150 * 400 + 200) * 4..(150 * 400 + 200) * 4 + 3].to_vec();
    assert_eq!(green, vec![0, 255, 0]);
}

#[test]
fn theme_keyframes_rule_animates_slot() {
    let mut e = rich_engine();
    ok(&mut e, "delete_layer", json!({"layer": "Pic"}));
    ok(&mut e, "update_theme_rule", json!({"theme": "dark", "slot_id": "brand", "keyframes": [{"frame": 0, "value": "#000000"}, {"frame": 10, "value": "#ffffff"}]}));
    ok(&mut e, "select_theme", json!({"theme": "dark"}));
    let at = |e: &mut Engine, f: f64| e.render_rgba(f, 400, 300)[(150 * 400 + 200) * 4];
    assert!(at(&mut e, 0.0) < 10);
    assert!(at(&mut e, 10.0) > 245);
}

#[test]
fn lottiefiles_samples_load_and_render() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/lottiefiles");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    let mut n = 0;
    for f in entries.flatten() {
        let p = f.path();
        if p.extension().and_then(|x| x.to_str()) != Some("lottie") {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let mut e = Engine::new();
        let info = e.load_file(p.file_name().unwrap().to_str().unwrap(), &bytes).unwrap_or_else(|err| panic!("{}: {err}", p.display()));
        assert!(e.project()["stateMachines"].as_object().map(|m| !m.is_empty()).unwrap_or(false), "{} has state machines", p.display());
        let (w, h) = ((info["w"].as_f64().unwrap() / 4.0).max(1.0) as u32, (info["h"].as_f64().unwrap() / 4.0).max(1.0) as u32);
        let px = e.render_rgba(info["ip"].as_f64().unwrap(), w, h);
        assert_eq!(px.len(), (w * h * 4) as usize);
        let back = dotlottie::write(e.project()).unwrap();
        Engine::new().load_file("rt.lottie", &back).unwrap_or_else(|err| panic!("round trip {}: {err}", p.display()));
        n += 1;
    }
    eprintln!("loaded {n} LottieFiles samples");
}
