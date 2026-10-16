use lumetrope_core::model;
use lumetrope_core::render::{render, RenderCtx};
use serde_json::{json, Value};

fn st(v: Value) -> Value {
    json!({"a":0,"k":v})
}

fn ks(p: [f64; 2]) -> Value {
    json!({"p": st(json!(p)), "a": st(json!([0,0])), "s": st(json!([100,100])), "r": st(json!(0)), "o": st(json!(100))})
}

fn doc(layers: Vec<Value>) -> Value {
    json!({"v":"5.12.0","fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":layers})
}

fn shape_layer(ind: i64, p: [f64; 2], shapes: Vec<Value>) -> Value {
    json!({"ty":4,"ind":ind,"ip":0,"op":60,"st":0,"ks":ks(p),"shapes":shapes})
}

fn rect(w: f64, h: f64) -> Value {
    json!({"ty":"rc","p":st(json!([0,0])),"s":st(json!([w,h])),"r":st(json!(0))})
}

fn fill(c: [f64; 3]) -> Value {
    json!({"ty":"fl","c":st(json!([c[0],c[1],c[2],1])),"o":st(json!(100)),"r":1})
}

fn draw(v: &Value, frame: f64) -> tiny_skia::Pixmap {
    let a = model::parse(v).unwrap();
    render(&a, &RenderCtx::default(), frame, 100, 100)
}

fn px(p: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn red_rect_center_pixel_red() {
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![rect(40.0, 40.0), fill([1.0, 0.0, 0.0])])]), 0.0);
    assert_eq!(px(&p, 50, 50), [255, 0, 0, 255]);
    assert_eq!(px(&p, 5, 5)[3], 0);
}

#[test]
fn fill_after_two_shapes_paints_both() {
    let left = json!({"ty":"rc","p":st(json!([-30,0])),"s":st(json!([20,20])),"r":st(json!(0))});
    let right = json!({"ty":"rc","p":st(json!([30,0])),"s":st(json!([20,20])),"r":st(json!(0))});
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![left, right, fill([0.0, 0.0, 1.0])])]), 0.0);
    assert_eq!(px(&p, 20, 50), [0, 0, 255, 255]);
    assert_eq!(px(&p, 80, 50), [0, 0, 255, 255]);
    assert_eq!(px(&p, 50, 50)[3], 0);
}

#[test]
fn earlier_paint_is_on_top() {
    // fill listed first is drawn above the fill listed second
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![rect(40.0, 40.0), fill([0.0, 1.0, 0.0]), fill([1.0, 0.0, 0.0])])]), 0.0);
    assert_eq!(px(&p, 50, 50), [0, 255, 0, 255]);
}

#[test]
fn stroke_width_covers_edge() {
    let stroke = json!({"ty":"st","c":st(json!([0,0,0,1])),"o":st(json!(100)),"w":st(json!(6)),"lc":2,"lj":2});
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![rect(40.0, 40.0), stroke])]), 0.0);
    assert_eq!(px(&p, 30, 50)[3], 255, "left edge stroked");
    assert_eq!(px(&p, 50, 50)[3], 0, "no fill");
}

#[test]
fn group_transform_moves_shape() {
    let g = json!({"ty":"gr","it":[rect(10.0,10.0), fill([1.0,0.0,0.0]),
        {"ty":"tr","p":st(json!([20,0])),"a":st(json!([0,0])),"s":st(json!([100,100])),"r":st(json!(0)),"o":st(json!(100))}]});
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![g])]), 0.0);
    assert_eq!(px(&p, 70, 50), [255, 0, 0, 255]);
    assert_eq!(px(&p, 50, 50)[3], 0);
}

#[test]
fn linear_gradient_left_dark_right_light() {
    let gf = json!({"ty":"gf","t":1,"o":st(json!(100)),"r":1,
        "s":st(json!([-50,0])),"e":st(json!([50,0])),
        "g":{"p":2,"k":st(json!([0,0,0,0, 1,1,1,1]))}});
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![rect(100.0, 100.0), gf])]), 0.0);
    assert!(px(&p, 5, 50)[0] < 40);
    assert!(px(&p, 95, 50)[0] > 215);
}

#[test]
fn radial_gradient_center() {
    let gf = json!({"ty":"gf","t":2,"o":st(json!(100)),"r":1,
        "s":st(json!([0,0])),"e":st(json!([50,0])),"h":st(json!(0)),"a":st(json!(0)),
        "g":{"p":2,"k":st(json!([0,1,1,1, 1,0,0,0]))}});
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![rect(100.0, 100.0), gf])]), 0.0);
    assert!(px(&p, 50, 50)[0] > 240);
    assert!(px(&p, 98, 50)[0] < 20);
}

#[test]
fn gradient_alpha_stops() {
    let gf = json!({"ty":"gf","t":1,"o":st(json!(100)),"r":1,
        "s":st(json!([-50,0])),"e":st(json!([50,0])),
        "g":{"p":2,"k":st(json!([0,1,0,0, 1,1,0,0, 0,1, 1,0]))}});
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![rect(100.0, 100.0), gf])]), 0.0);
    assert!(px(&p, 3, 50)[3] > 240);
    assert!(px(&p, 97, 50)[3] < 15);
}

#[test]
fn layer_opacity_halves_alpha() {
    let mut l = shape_layer(1, [50.0, 50.0], vec![rect(40.0, 40.0), fill([1.0, 0.0, 0.0])]);
    l["ks"]["o"] = st(json!(50));
    let p = draw(&doc(vec![l]), 0.0);
    let a = px(&p, 50, 50)[3];
    assert!((120..=135).contains(&a), "alpha {a}");
}

#[test]
fn parenting_follows_parent_position() {
    let parent = json!({"ty":3,"ind":1,"ip":0,"op":60,"st":0,"ks":ks([30.0, 0.0])});
    let mut child = shape_layer(2, [20.0, 50.0], vec![rect(10.0, 10.0), fill([1.0, 0.0, 0.0])]);
    child["parent"] = json!(1);
    let p = draw(&doc(vec![child, parent]), 0.0);
    assert_eq!(px(&p, 50, 50), [255, 0, 0, 255]);
    assert_eq!(px(&p, 20, 50)[3], 0);
}

#[test]
fn layer_outside_ip_op_not_drawn() {
    let mut l = shape_layer(1, [50.0, 50.0], vec![rect(40.0, 40.0), fill([1.0, 0.0, 0.0])]);
    l["ip"] = json!(10);
    assert_eq!(px(&draw(&doc(vec![l.clone()]), 5.0), 50, 50)[3], 0);
    assert_eq!(px(&draw(&doc(vec![l]), 10.0), 50, 50)[3], 255);
}

#[test]
fn hidden_layer_not_drawn() {
    let mut l = shape_layer(1, [50.0, 50.0], vec![rect(40.0, 40.0), fill([1.0, 0.0, 0.0])]);
    l["hd"] = json!(true);
    assert_eq!(px(&draw(&doc(vec![l]), 0.0), 50, 50)[3], 0);
}

#[test]
fn animated_position_moves() {
    let mut l = shape_layer(1, [0.0, 0.0], vec![rect(10.0, 10.0), fill([1.0, 0.0, 0.0])]);
    l["ks"]["p"] = json!({"a":1,"k":[
        {"t":0,"s":[10,50],"i":{"x":1,"y":1},"o":{"x":0,"y":0}},{"t":10,"s":[90,50]}]});
    let p = draw(&doc(vec![l]), 5.0);
    assert_eq!(px(&p, 50, 50), [255, 0, 0, 255]);
}

#[test]
fn trim_path_draws_half_stroke() {
    // horizontal open path from x=10 to x=90, trimmed to first half
    let path = json!({"ty":"sh","ks":st(json!({"c":false,"v":[[-40,0],[40,0]],"i":[[0,0],[0,0]],"o":[[0,0],[0,0]]}))});
    let trim = json!({"ty":"tm","s":st(json!(0)),"e":st(json!(50)),"o":st(json!(0)),"m":1});
    let stroke = json!({"ty":"st","c":st(json!([0,0,0,1])),"o":st(json!(100)),"w":st(json!(4)),"lc":1,"lj":1});
    let p = draw(&doc(vec![shape_layer(1, [50.0, 50.0], vec![path, trim, stroke])]), 0.0);
    assert_eq!(px(&p, 30, 50)[3], 255);
    assert_eq!(px(&p, 70, 50)[3], 0);
}

#[test]
fn repeater_three_copies() {
    let rp = json!({"ty":"rp","c":st(json!(3)),"o":st(json!(0)),"m":1,
        "tr":{"p":st(json!([30,0])),"a":st(json!([0,0])),"s":st(json!([100,100])),"r":st(json!(0)),
              "so":st(json!(100)),"eo":st(json!(100))}});
    let p = draw(&doc(vec![shape_layer(1, [20.0, 50.0], vec![rect(10.0, 10.0), fill([1.0, 0.0, 0.0]), rp])]), 0.0);
    for x in [20, 50, 80] {
        assert_eq!(px(&p, x, 50), [255, 0, 0, 255], "copy at x={x}");
    }
    assert_eq!(px(&p, 35, 50)[3], 0);
}

#[test]
fn degenerate_shapes_do_not_panic() {
    let shapes = vec![
        rect(0.0, 0.0),
        json!({"ty":"el","p":st(json!([0,0])),"s":st(json!([0,0]))}),
        json!({"ty":"sh","ks":st(json!({"c":true,"v":[[1,1]],"i":[[0,0]],"o":[[0,0]]}))}),
        json!({"ty":"sh","ks":st(json!({}))}),
        json!({"ty":"sr","sy":1,"p":st(json!([0,0])),"pt":st(json!(0)),"or":st(json!(0)),"ir":st(json!(0)),"os":st(json!(0)),"is":st(json!(0)),"r":st(json!(0))}),
        json!({"ty":"tm","s":st(json!(50)),"e":st(json!(50)),"o":st(json!(0)),"m":1}),
        json!({"ty":"st","c":st(json!([0,0,0,1])),"o":st(json!(100)),"w":st(json!(0)),"d":[{"n":"d","v":st(json!(0))},{"n":"g","v":st(json!(0))}]}),
        fill([1.0, 0.0, 0.0]),
        json!({"ty":"gf","t":1,"o":st(json!(100)),"s":st(json!([0,0])),"e":st(json!([0,0])),"g":{"p":0,"k":st(json!([]))}}),
    ];
    let mut l = shape_layer(1, [50.0, 50.0], shapes);
    l["ks"]["s"] = st(json!([0, 0]));
    let _ = draw(&doc(vec![l, json!({"ty":4})]), 0.0);
    let _ = draw(&json!({"w":0,"h":0}), 0.0);
}
