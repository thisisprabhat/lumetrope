use lumetrope_core::project;
use lumetrope_core::vcs::Repo;
use serde_json::json;

fn proj(layers: Vec<serde_json::Value>, w: f64) -> Vec<u8> {
    let mut l = project::blank_lottie(w, 100.0, 30.0, 60.0);
    l["layers"] = json!(layers);
    project::from_lottie(l).to_string().into_bytes()
}

fn layer(ind: i64, name: &str) -> serde_json::Value {
    json!({"ty": 3, "ind": ind, "nm": name, "ks": {}})
}

#[test]
fn commit_and_log_order() {
    let mut r = Repo::new();
    let a = r.commit(&proj(vec![], 100.0), "first", "me", 1.0).unwrap();
    let b = r.commit(&proj(vec![layer(1, "A")], 100.0), "second", "me", 2.0).unwrap();
    let log = r.log();
    assert_eq!(log.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), vec![b.clone(), a.clone()]);
    assert_eq!(log[0].message, "second");
    assert_eq!(log[0].parents, vec![a]);
}

#[test]
fn identical_content_dedups_and_unchanged_commit_is_refused() {
    let mut r = Repo::new();
    r.commit(&proj(vec![], 100.0), "one", "me", 1.0).unwrap();
    let err = r.commit(&proj(vec![], 100.0), "again", "me", 2.0).unwrap_err();
    assert!(err.contains("no changes"));
    assert_eq!(r.object_count(), 1);
}

#[test]
fn checkout_restores_bytes() {
    let mut r = Repo::new();
    let v1 = proj(vec![layer(1, "A")], 100.0);
    let a = r.commit(&v1, "a", "me", 1.0).unwrap();
    r.commit(&proj(vec![], 100.0), "b", "me", 2.0).unwrap();
    assert_eq!(r.checkout(&a).unwrap(), v1);
    assert!(r.checkout("deadbeef").is_err());
}

#[test]
fn branch_switch() {
    let mut r = Repo::new();
    let base = r.commit(&proj(vec![], 100.0), "base", "me", 1.0).unwrap();
    r.create_branch("experiment", None).unwrap();
    assert_eq!(r.current_branch(), "experiment");
    let v2 = proj(vec![layer(1, "X")], 100.0);
    r.commit(&v2, "try", "me", 2.0).unwrap();
    let main = r.switch("main").unwrap();
    assert_eq!(main, Some(proj(vec![], 100.0)));
    assert_eq!(r.log().len(), 1);
    assert_eq!(r.log()[0].id, base);
    assert_eq!(r.switch("experiment").unwrap(), Some(v2));
    assert!(r.create_branch("experiment", None).is_err());
    assert!(r.switch("nope").is_err());
    let b = r.branches();
    assert!(b.iter().any(|(n, _)| n == "main") && b.iter().any(|(n, _)| n == "experiment"));
}

#[test]
fn diff_detects_added_layer_and_settings() {
    let mut r = Repo::new();
    let a = r.commit(&proj(vec![layer(1, "A")], 100.0), "a", "me", 1.0).unwrap();
    let mut l = project::blank_lottie(200.0, 100.0, 30.0, 60.0);
    l["layers"] = json!([layer(1, "A renamed"), layer(2, "B")]);
    let mut p = project::from_lottie(l);
    p["stateMachines"]["m"] = json!({"initial": "x", "states": []});
    let b = r.commit(&p.to_string().into_bytes(), "b", "me", 2.0).unwrap();
    let d = r.diff(&a, &b).unwrap();
    assert_eq!(d["layers_added"], json!(["B"]));
    assert_eq!(d["layers_changed"], json!(["A renamed"]));
    assert_eq!(d["layers_removed"], json!([]));
    assert_eq!(d["settings_changed"], json!(["w"]));
    assert_eq!(d["state_machines_changed"], json!(["m"]));
}

#[test]
fn serialize_roundtrip() {
    let mut r = Repo::new();
    let v = proj(vec![layer(1, "A")], 100.0);
    let a = r.commit(&v, "a", "me", 1.0).unwrap();
    r.create_branch("b2", None).unwrap();
    let bytes = r.to_bytes();
    let r2 = Repo::from_bytes(&bytes).unwrap();
    assert_eq!(r2.current_branch(), "b2");
    assert_eq!(r2.checkout(&a).unwrap(), v);
    assert_eq!(r2.log().len(), 1);
    assert!(Repo::from_bytes(b"junk").is_err());
}
