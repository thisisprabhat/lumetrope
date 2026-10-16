use lumetrope_core::anim::{cubic_ease, eval, eval1, eval_shape};
use lumetrope_core::model::{prop, shape_prop};
use serde_json::json;

fn p(v: serde_json::Value) -> lumetrope_core::model::Prop {
    prop(Some(&v)).unwrap()
}

const LIN: &str = r#"{"x":[1],"y":[1]}"#;
const LIN0: &str = r#"{"x":[0],"y":[0]}"#;

fn lin_prop(a: f64, b: f64, t0: f64, t1: f64) -> lumetrope_core::model::Prop {
    let i: serde_json::Value = serde_json::from_str(LIN).unwrap();
    let o: serde_json::Value = serde_json::from_str(LIN0).unwrap();
    p(json!({"a":1,"k":[{"t":t0,"s":[a],"i":i,"o":o},{"t":t1,"s":[b]}]}))
}

#[test]
fn linear_midpoint() {
    assert!((eval(&lin_prop(0.0, 100.0, 0.0, 10.0), 5.0)[0] - 50.0).abs() < 1e-6);
}

#[test]
fn hold_keyframe() {
    let pr = p(json!({"a":1,"k":[{"t":0,"s":[0],"h":1},{"t":10,"s":[100]}]}));
    assert_eq!(eval(&pr, 9.99)[0], 0.0);
    assert_eq!(eval(&pr, 10.0)[0], 100.0);
}

#[test]
fn ease_in_out_is_symmetric() {
    assert!((cubic_ease(0.42, 0.0, 0.58, 1.0, 0.5) - 0.5).abs() < 1e-4);
    assert!(cubic_ease(0.42, 0.0, 0.58, 1.0, 0.25) < 0.25);
    assert_eq!(cubic_ease(0.42, 0.0, 0.58, 1.0, 0.0), 0.0);
    assert_eq!(cubic_ease(0.42, 0.0, 0.58, 1.0, 1.0), 1.0);
}

#[test]
fn before_first_after_last_clamp() {
    let pr = lin_prop(10.0, 20.0, 5.0, 15.0);
    assert_eq!(eval(&pr, 0.0)[0], 10.0);
    assert_eq!(eval(&pr, 100.0)[0], 20.0);
}

#[test]
fn legacy_e_end_value() {
    let pr = p(json!({"a":1,"k":[
        {"t":0,"s":[0],"e":[40],"i":{"x":[1],"y":[1]},"o":{"x":[0],"y":[0]}},
        {"t":10}
    ]}));
    assert!((eval(&pr, 5.0)[0] - 20.0).abs() < 1e-6);
    assert_eq!(eval(&pr, 20.0)[0], 40.0);
}

#[test]
fn spatial_bezier_position() {
    let pr = p(json!({"a":1,"k":[
        {"t":0,"s":[0,0],"to":[0,50],"ti":[0,50],"i":{"x":1,"y":1},"o":{"x":0,"y":0}},
        {"t":10,"s":[100,0]}
    ]}));
    let mid = eval(&pr, 5.0);
    assert!((mid[0] - 50.0).abs() < 2.0, "x {}", mid[0]);
    assert!(mid[1] > 20.0, "curve should bow away from the straight line, y={}", mid[1]);
}

#[test]
fn coincident_keyframes() {
    let pr = p(json!({"a":1,"k":[{"t":5,"s":[1]},{"t":5,"s":[2]},{"t":10,"s":[3]}]}));
    for fr in [0.0, 5.0, 7.5, 10.0] {
        assert!(eval(&pr, fr)[0].is_finite());
    }
}

#[test]
fn static_and_eval1_defaults() {
    assert_eq!(eval(&p(json!({"a":0,"k":[3,4]})), 99.0), vec![3.0, 4.0]);
    assert_eq!(eval1(None, 0.0, 7.0), 7.0);
}

#[test]
fn shape_vertex_interpolation() {
    let sp = shape_prop(Some(&json!({"a":1,"k":[
        {"t":0,"s":[{"c":false,"v":[[0,0],[10,0]],"i":[[0,0],[0,0]],"o":[[0,0],[0,0]]}],"i":{"x":1,"y":1},"o":{"x":0,"y":0}},
        {"t":10,"s":[{"c":false,"v":[[0,10],[10,10]],"i":[[0,0],[0,0]],"o":[[0,0],[0,0]]}]}
    ]}))).unwrap();
    let b = eval_shape(&sp, 5.0);
    assert_eq!(b.v.len(), 2);
    assert!((b.v[1][1] - 5.0).abs() < 1e-6);
}
