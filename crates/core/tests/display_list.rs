use lumetrope_core::geom::Affine;
use lumetrope_core::model;
use lumetrope_core::render::list::record;
use lumetrope_core::text::FontRegistry;
use serde_json::{json, Value};

fn st(v: Value) -> Value {
    json!({"a":0,"k":v})
}
fn rect_layer(ind: i64, p: [f64; 2], c: [f64; 3]) -> Value {
    json!({"ty":4,"nm":format!("L{ind}"),"ind":ind,"ip":0,"op":60,"st":0,"ks":{"p":st(json!(p)),"o":st(json!(100))},"shapes":[
        {"ty":"rc","p":st(json!([0,0])),"s":st(json!([20,10])),"r":st(json!(0))},
        {"ty":"fl","c":st(json!([c[0],c[1],c[2],1])),"o":st(json!(50))}]})
}
fn rec(doc: Value, base: Affine) -> Vec<Value> {
    let a = model::parse(&doc).unwrap();
    record(&a, &FontRegistry::default(), 0.0, &base).as_array().unwrap().clone()
}
fn doc(layers: Vec<Value>) -> Value {
    json!({"w":100,"h":100,"fr":30,"op":60,"layers":layers})
}

#[test]
fn simple_shape_becomes_one_fill_with_absolute_transform() {
    let cmds = rec(doc(vec![rect_layer(1, [50.0, 50.0], [1.0, 0.0, 0.0])]), Affine::scale(2.0, 2.0));
    assert_eq!(cmds.len(), 1);
    let c = &cmds[0];
    assert_eq!(c["op"], "fill");
    assert_eq!(c["t"], json!([2.0, 0.0, 0.0, 2.0, 100.0, 100.0]));
    assert_eq!(c["paint"]["c"], json!([1.0, 0.0, 0.0, 0.5]), "fill opacity folded into colour alpha");
    assert!(c["d"].as_str().unwrap().starts_with("M10 -5"), "{}", c["d"]);
    assert_eq!(c["rule"], "nonzero");
}

#[test]
fn draw_order_bottom_layer_first() {
    let cmds = rec(doc(vec![rect_layer(1, [10.0, 10.0], [1.0, 0.0, 0.0]), rect_layer(2, [20.0, 20.0], [0.0, 0.0, 1.0])]), Affine::IDENTITY);
    assert_eq!(cmds[0]["paint"]["c"][2], 1.0, "blue (bottom layer) first");
    assert_eq!(cmds[1]["paint"]["c"][0], 1.0);
}

#[test]
fn stroke_and_gradient_paints() {
    let l = json!({"ty":4,"ind":1,"ip":0,"op":60,"st":0,"ks":{},"shapes":[
        {"ty":"el","p":st(json!([0,0])),"s":st(json!([10,10]))},
        {"ty":"st","c":st(json!([0,0,0,1])),"o":st(json!(100)),"w":st(json!(3)),"lc":1,"lj":3,"ml":4,"d":[{"n":"d","v":st(json!(4))},{"n":"g","v":st(json!(2))}]},
        {"ty":"gf","t":1,"o":st(json!(100)),"s":st(json!([-5,0])),"e":st(json!([5,0])),"g":{"p":2,"k":st(json!([0,1,0,0, 1,0,0,1]))}}]});
    let cmds = rec(doc(vec![l]), Affine::IDENTITY);
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0]["op"], "fill");
    assert_eq!(cmds[0]["paint"]["lin"], json!([-5.0, 0.0, 5.0, 0.0]));
    assert_eq!(cmds[0]["paint"]["stops"].as_array().unwrap().len(), 2);
    assert_eq!(cmds[1]["op"], "stroke");
    assert_eq!((cmds[1]["w"].as_f64(), cmds[1]["cap"].as_str(), cmds[1]["join"].as_str()), (Some(3.0), Some("butt"), Some("bevel")));
    assert_eq!(cmds[1]["dash"], json!([4.0, 2.0]));
}

#[test]
fn matte_mask_blend_and_effects_make_groups() {
    let mut matte = rect_layer(1, [50.0, 50.0], [1.0, 1.0, 1.0]);
    matte["td"] = json!(1);
    let mut content = rect_layer(2, [50.0, 50.0], [1.0, 0.0, 0.0]);
    content["tt"] = json!(3);
    content["bm"] = json!(1);
    content["masksProperties"] = json!([{"mode":"s","inv":false,"o":st(json!(100)),"pt":st(json!({"c":true,"v":[[0,0],[5,0],[5,5]],"i":[[0,0],[0,0],[0,0]],"o":[[0,0],[0,0],[0,0]]}))}]);
    content["ef"] = json!([{"ty":29,"en":1,"ef":[{"ty":0,"v":st(json!(8))},{"ty":7,"v":st(json!(1))}]}]);
    let cmds = rec(doc(vec![matte, content]), Affine::scale(2.0, 2.0));
    assert_eq!(cmds.len(), 1, "matte source is not drawn on its own");
    let g = &cmds[0];
    assert_eq!(g["op"], "group");
    assert_eq!(g["blend"], "multiply");
    assert_eq!(g["matte"]["mode"], "luma");
    assert_eq!(g["matte"]["cmds"].as_array().unwrap().len(), 1);
    assert_eq!(g["masks"][0]["mode"], "s");
    assert_eq!(g["fx"][0]["type"], "blur");
    assert_eq!(g["fx"][0]["sigma"], 8.0, "blurriness 8 → σ 4 comp px → 8 device px at 2×");
}

#[test]
fn precomp_is_clipped_group_and_text_is_paths() {
    let inner = rect_layer(1, [10.0, 10.0], [0.0, 1.0, 0.0]);
    let d = json!({"w":100,"h":100,"fr":30,"op":60,"assets":[{"id":"c","layers":[inner]}],"layers":[
        {"ty":0,"ind":1,"refId":"c","w":50,"h":50,"ip":0,"op":60,"st":0,"ks":{}},
        {"ty":5,"ind":2,"ip":0,"op":60,"st":0,"ks":{},"t":{"d":{"k":[{"t":0,"s":{"t":"A","s":20,"f":"x","fc":[0,0,0]}}]}}}]});
    let cmds = rec(d, Affine::IDENTITY);
    let g = cmds.iter().find(|c| c["op"] == "group").unwrap();
    assert!(g["clip"]["d"].as_str().unwrap().contains("50"));
    assert_eq!(g["cmds"].as_array().unwrap().len(), 1);
    assert!(cmds.iter().any(|c| c["op"] == "fill" && c["paint"]["c"] == json!([0.0, 0.0, 0.0, 1.0])), "text glyph fill");
}
