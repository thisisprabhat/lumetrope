use lumetrope_core::model::{self, Shape};
use serde_json::json;

fn minimal() -> serde_json::Value {
    json!({
        "v": "5.12.0", "fr": 30, "ip": 0, "op": 60, "w": 512, "h": 512, "nm": "t",
        "layers": [{
            "ty": 4, "nm": "box", "ind": 1, "ip": 0, "op": 60, "st": 0,
            "ks": { "p": {"a":0,"k":[256,256]}, "a": {"a":0,"k":[0,0]}, "s": {"a":0,"k":[100,100]},
                    "r": {"a":0,"k":0}, "o": {"a":0,"k":100} },
            "shapes": [
                {"ty":"rc","nm":"r","p":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0}},
                {"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":100}}
            ]
        }]
    })
}

#[test]
fn parses_minimal_doc() {
    let a = model::parse(&minimal()).unwrap();
    assert_eq!((a.w, a.h), (512.0, 512.0));
    assert_eq!(a.fr, 30.0);
    assert_eq!(a.layers.len(), 1);
    let l = &a.layers[0];
    assert_eq!(l.ty, 4);
    assert_eq!(l.shapes.len(), 2);
    assert!(matches!(l.shapes[0], Shape::Rect { .. }));
    assert!(matches!(l.shapes[1], Shape::Fill { .. }));
    assert_eq!(l.ks.p.as_ref().unwrap().value, vec![256.0, 256.0]);
}

#[test]
fn parses_legacy_and_scalar_forms() {
    let mut v = minimal();
    v["layers"][0]["ks"]["r"] = json!({"a":0,"k":45});
    v["layers"][0]["ks"]["o"] = json!({"a":1,"k":[
        {"t":0,"s":[0],"e":[100],"i":{"x":[0.5],"y":[1]},"o":{"x":[0.5],"y":[0]}},
        {"t":10}
    ]});
    let a = model::parse(&v).unwrap();
    let r = a.layers[0].ks.r.as_ref().unwrap();
    assert_eq!(r.value, vec![45.0]);
    assert!(r.scalar);
    let o = a.layers[0].ks.o.as_ref().unwrap();
    assert!(o.animated);
    assert_eq!(o.keyframes.len(), 2);
    assert_eq!(o.keyframes[0].e.as_deref(), Some(&[100.0][..]));
}

#[test]
fn missing_layers_is_empty_not_error() {
    let a = model::parse(&json!({"w": 10, "h": 10})).unwrap();
    assert!(a.layers.is_empty());
    assert_eq!(a.fr, 30.0);
}

#[test]
fn non_object_is_error() {
    assert!(model::parse(&json!([1, 2])).is_err());
}

#[test]
fn split_position_and_unknown_shape_kept() {
    let mut v = minimal();
    v["layers"][0]["ks"]["p"] = json!({"s": true, "x": {"a":0,"k":10}, "y": {"a":0,"k":20}});
    v["layers"][0]["shapes"].as_array_mut().unwrap().push(json!({"ty":"zz","nm":"zig"}));
    let a = model::parse(&v).unwrap();
    let ks = &a.layers[0].ks;
    assert!(ks.p.is_none());
    assert_eq!(ks.px.as_ref().unwrap().value, vec![10.0]);
    assert_eq!(ks.py.as_ref().unwrap().value, vec![20.0]);
    assert!(matches!(a.layers[0].shapes[2], Shape::Unknown { .. }));
}
