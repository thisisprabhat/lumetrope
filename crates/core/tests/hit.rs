use lumetrope_core::hit;
use lumetrope_core::model;
use lumetrope_core::text::FontRegistry;
use serde_json::{json, Value};

fn st(v: Value) -> Value { json!({"a":0,"k":v}) }
fn rect_layer(ind: i64, nm: &str, p: [f64; 2], size: [f64; 2]) -> Value {
    json!({"ty":4,"nm":nm,"ind":ind,"ip":0,"op":60,"st":0,
        "ks":{"p":st(json!(p)),"a":st(json!([0,0])),"s":st(json!([100,100])),"r":st(json!(0)),"o":st(json!(100))},
        "shapes":[{"ty":"rc","p":st(json!([0,0])),"s":st(json!(size)),"r":st(json!(0))},{"ty":"fl","c":st(json!([1,0,0,1])),"o":st(json!(100))}]})
}
fn anim(layers: Vec<Value>) -> model::Animation {
    model::parse(&json!({"w":200,"h":200,"fr":30,"op":60,"layers":layers})).unwrap()
}

#[test]
fn hit_topmost_wins() {
    let a = anim(vec![rect_layer(1, "top", [100.0, 100.0], [40.0, 40.0]), rect_layer(2, "bottom", [100.0, 100.0], [100.0, 100.0])]);
    let f = FontRegistry::default();
    assert_eq!(hit::hit_test(&a, &f, 0.0, 100.0, 100.0).as_deref(), Some("root:1"));
    assert_eq!(hit::hit_test(&a, &f, 0.0, 60.0, 60.0).as_deref(), Some("root:2"));
}

#[test]
fn miss_returns_none() {
    let a = anim(vec![rect_layer(1, "a", [100.0, 100.0], [40.0, 40.0])]);
    assert!(hit::hit_test(&a, &FontRegistry::default(), 0.0, 5.0, 5.0).is_none());
}

#[test]
fn hidden_and_out_of_range_layers_are_not_hit() {
    let mut l = rect_layer(1, "a", [100.0, 100.0], [40.0, 40.0]);
    l["hd"] = json!(true);
    assert!(hit::hit_test(&anim(vec![l]), &FontRegistry::default(), 0.0, 100.0, 100.0).is_none());
}

#[test]
fn bounds_follow_scale() {
    let mut l = rect_layer(1, "a", [100.0, 100.0], [40.0, 20.0]);
    l["ks"]["s"] = st(json!([200, 200]));
    let g = hit::layer_geometry(&anim(vec![l]), &FontRegistry::default(), "root:1", 0.0).unwrap();
    let b = g.bounds;
    assert!((b[0] - 60.0).abs() < 1e-6 && (b[2] - 140.0).abs() < 1e-6, "{b:?}");
    assert!((b[1] - 80.0).abs() < 1e-6 && (b[3] - 120.0).abs() < 1e-6, "{b:?}");
}

#[test]
fn geometry_has_rotated_corners() {
    let mut l = rect_layer(1, "a", [100.0, 100.0], [40.0, 20.0]);
    l["ks"]["r"] = st(json!(90));
    let g = hit::layer_geometry(&anim(vec![l]), &FontRegistry::default(), "root:1", 0.0).unwrap();
    // local top-left (-20,-10) rotated 90° clockwise → (110, 80)
    assert!((g.corners[0][0] - 110.0).abs() < 1e-6 && (g.corners[0][1] - 80.0).abs() < 1e-6, "{:?}", g.corners);
    assert_eq!(g.local, [-20.0, -10.0, 20.0, 10.0]);
}

#[test]
fn named_at_includes_precomp_children() {
    let inner = rect_layer(1, "button", [50.0, 50.0], [20.0, 20.0]);
    let a = model::parse(&json!({"w":200,"h":200,"fr":30,"op":60,
        "assets":[{"id":"c1","layers":[inner]}],
        "layers":[{"ty":0,"nm":"group","ind":1,"refId":"c1","w":100,"h":100,"ip":0,"op":60,"st":0,
                   "ks":{"p":st(json!([100,100])),"a":st(json!([50,50]))}}]})).unwrap();
    let names = hit::layers_named_at(&a, &FontRegistry::default(), 0.0, 100.0, 100.0);
    assert!(names.contains(&"button".to_string()) && names.contains(&"group".to_string()), "{names:?}");
    assert!(!hit::layers_named_at(&a, &FontRegistry::default(), 0.0, 60.0, 60.0).contains(&"button".to_string()));
}
