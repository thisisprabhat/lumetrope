use lumetrope_core::ops::{self, catalog, History, Tx};
use lumetrope_core::project;
use serde_json::json;

fn proj() -> serde_json::Value {
    project::from_lottie(project::blank_lottie(512.0, 512.0, 30.0, 90.0))
}

#[test]
fn tx_set_insert_remove_and_undo_restore_exact_json() {
    let mut root = proj();
    let before = root.clone();
    let mut tx = Tx::new("t");
    tx.set(&mut root, "/animations/main/w", json!(100)).unwrap();
    tx.set(&mut root, "/animations/main/meta/new/deep", json!(1)).unwrap();
    tx.insert(&mut root, "/animations/main/layers", 0, json!({"ty":3,"ind":1})).unwrap();
    tx.insert(&mut root, "/animations/main/layers", 1, json!({"ty":3,"ind":2})).unwrap();
    let removed = tx.remove(&mut root, "/animations/main/layers", 0).unwrap();
    assert_eq!(removed["ind"], 1);
    assert_eq!(root["animations"]["main"]["layers"][0]["ind"], 2);
    tx.undo(&mut root);
    assert_eq!(root, before);
    tx.redo(&mut root);
    assert_eq!(root["animations"]["main"]["w"], 100);
    assert_eq!(root["animations"]["main"]["meta"]["new"]["deep"], 1);
}

#[test]
fn undo_restores_exact_json() {
    let mut root = proj();
    let before = root.clone();
    let mut h = History::default();
    ops::apply(&mut root, &mut h, "set_scene_settings", &json!({"width": 800, "height": 600, "fps": 60, "duration_frames": 120, "name": "Hero"})).unwrap();
    let a = &root["animations"]["main"];
    assert_eq!((a["w"].as_f64(), a["h"].as_f64(), a["fr"].as_f64(), a["op"].as_f64()), (Some(800.0), Some(600.0), Some(60.0), Some(120.0)));
    assert_eq!(a["nm"], "Hero");
    assert!(h.undo(&mut root).is_some());
    assert_eq!(root, before);
}

#[test]
fn redo_reapplies() {
    let mut root = proj();
    let mut h = History::default();
    ops::apply(&mut root, &mut h, "set_scene_settings", &json!({"width": 10})).unwrap();
    let after = root.clone();
    h.undo(&mut root);
    assert!(h.redo(&mut root).is_some());
    assert_eq!(root, after);
}

#[test]
fn new_op_clears_redo() {
    let mut root = proj();
    let mut h = History::default();
    ops::apply(&mut root, &mut h, "set_scene_settings", &json!({"width": 10})).unwrap();
    h.undo(&mut root);
    assert!(h.can_redo());
    ops::apply(&mut root, &mut h, "set_scene_settings", &json!({"width": 20})).unwrap();
    assert!(!h.can_redo());
}

#[test]
fn unknown_tool_errors() {
    let mut root = proj();
    let mut h = History::default();
    let e = ops::apply(&mut root, &mut h, "no_such_tool", &json!({})).unwrap_err();
    assert!(e.to_string().contains("no_such_tool"));
}

#[test]
fn failed_op_rolls_back_and_records_nothing() {
    let mut root = proj();
    let before = root.clone();
    let mut h = History::default();
    assert!(ops::apply(&mut root, &mut h, "set_scene_settings", &json!({"width": -5})).is_err());
    assert_eq!(root, before);
    assert!(!h.can_undo());
}

#[test]
fn catalog_names_unique_and_schemas_are_objects() {
    let tools = catalog::tools();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
    let n = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), n, "duplicate tool names");
    for t in &tools {
        let s = (t.schema)();
        assert_eq!(s["type"], "object", "{} schema", t.name);
        assert!(!t.description.is_empty());
    }
    let json = catalog::catalog_json();
    assert!(json.as_array().unwrap().iter().any(|t| t["name"] == "set_scene_settings" && t["inputSchema"]["type"] == "object"));
}

#[test]
fn history_is_capped() {
    let mut root = proj();
    let mut h = History::with_cap(3);
    for w in 1..=5 {
        ops::apply(&mut root, &mut h, "set_scene_settings", &json!({"width": w})).unwrap();
    }
    let mut n = 0;
    while h.undo(&mut root).is_some() {
        n += 1;
    }
    assert_eq!(n, 3);
}
