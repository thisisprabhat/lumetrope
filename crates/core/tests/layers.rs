use base64::Engine as _;
use lumetrope_core::model;
use lumetrope_core::render::{render, RenderCtx};
use serde_json::{json, Value};

fn st(v: Value) -> Value { json!({"a":0,"k":v}) }
fn ks(p: [f64; 2]) -> Value {
    json!({"p": st(json!(p)), "a": st(json!([0,0])), "s": st(json!([100,100])), "r": st(json!(0)), "o": st(json!(100))})
}
fn doc(layers: Vec<Value>) -> Value { json!({"fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":layers}) }
fn doc_assets(layers: Vec<Value>, assets: Vec<Value>) -> Value {
    json!({"fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":layers,"assets":assets})
}
fn rect_layer(ind: i64, p: [f64; 2], size: [f64; 2], c: [f64; 3]) -> Value {
    json!({"ty":4,"ind":ind,"ip":0,"op":60,"st":0,"ks":ks(p),"shapes":[
        {"ty":"rc","p":st(json!([0,0])),"s":st(json!(size)),"r":st(json!(0))},
        {"ty":"fl","c":st(json!([c[0],c[1],c[2],1])),"o":st(json!(100)),"r":1}]})
}
fn draw_ctx(v: &Value, frame: f64, ctx: &RenderCtx) -> tiny_skia::Pixmap {
    render(&model::parse(v).unwrap(), ctx, frame, 100, 100)
}
fn draw(v: &Value, frame: f64) -> tiny_skia::Pixmap { draw_ctx(v, frame, &RenderCtx::default()) }
fn px(p: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}
fn square_mask(mode: &str, inv: bool, cx: f64, half: f64) -> Value {
    json!({"mode":mode,"inv":inv,"o":st(json!(100)),"pt":st(json!({"c":true,
        "v":[[cx-half,-half+50.0],[cx+half,-half+50.0],[cx+half,half+50.0],[cx-half,half+50.0]],
        "i":[[0,0],[0,0],[0,0],[0,0]],"o":[[0,0],[0,0],[0,0],[0,0]]}))})
}
fn full_red() -> Value { rect_layer(1, [50.0, 50.0], [100.0, 100.0], [1.0, 0.0, 0.0]) }
fn at_origin(mut l: Value) -> Value { l["ks"]["p"] = st(json!([0,0])); l }

#[test]
fn precomp_layer_renders_asset_with_time_offset() {
    let mut inner = rect_layer(1, [50.0, 50.0], [20.0, 20.0], [0.0, 1.0, 0.0]);
    inner["ip"] = json!(5);
    let pc = json!({"ty":0,"ind":1,"refId":"comp1","w":100,"h":100,"ip":0,"op":60,"st":10,"ks":ks([50.0,50.0])});
    let mut pcl = pc.clone();
    pcl["ks"]["a"] = st(json!([50,50]));
    let d = doc_assets(vec![pcl], vec![json!({"id":"comp1","layers":[inner]})]);
    assert_eq!(px(&draw(&d, 12.0), 50, 50)[3], 0, "inner time 2 < ip 5");
    assert_eq!(px(&draw(&d, 16.0), 50, 50), [0, 255, 0, 255]);
}

#[test]
fn time_remap_selects_frame() {
    let mut inner = rect_layer(1, [50.0, 50.0], [20.0, 20.0], [0.0, 0.0, 1.0]);
    inner["ip"] = json!(30);
    let pc = json!({"ty":0,"ind":1,"refId":"c","w":100,"h":100,"ip":0,"op":60,"st":0,
        "ks":{"a":st(json!([50,50])),"p":st(json!([50,50]))},"tm":st(json!(1.5))});
    let d = doc_assets(vec![pc], vec![json!({"id":"c","layers":[inner]})]);
    // tm = 1.5s * 30fps = frame 45 inside → visible even at outer frame 0
    assert_eq!(px(&draw(&d, 0.0), 50, 50), [0, 0, 255, 255]);
}

#[test]
fn solid_layer_fills_size() {
    let l = json!({"ty":1,"ind":1,"ip":0,"op":60,"st":0,"sc":"#00ff00","sw":40,"sh":40,"ks":ks([0.0,0.0])});
    let p = draw(&doc(vec![l]), 0.0);
    assert_eq!(px(&p, 20, 20), [0, 255, 0, 255]);
    assert_eq!(px(&p, 60, 60)[3], 0);
}

#[test]
fn image_layer_draws_png_data_uri() {
    let mut img = tiny_skia::Pixmap::new(10, 10).unwrap();
    img.fill(tiny_skia::Color::from_rgba8(0, 0, 255, 255));
    let uri = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(img.encode_png().unwrap()));
    let a = model::parse(&doc_assets(
        vec![json!({"ty":2,"ind":1,"refId":"img","ip":0,"op":60,"st":0,"ks":ks([0.0,0.0])})],
        vec![json!({"id":"img","w":50,"h":50,"u":"","p":uri,"e":1})],
    )).unwrap();
    let ctx = RenderCtx::from_animation(&a);
    let p = render(&a, &ctx, 0.0, 100, 100);
    assert_eq!(px(&p, 25, 25), [0, 0, 255, 255]);
    assert_eq!(px(&p, 75, 75)[3], 0);
}

#[test]
fn mask_add_clips() {
    let mut l = full_red();
    l["masksProperties"] = json!([square_mask("a", false, 50.0, 10.0)]);
    l["hasMask"] = json!(true);
    let l = at_origin_shift(l);
    let p = draw(&doc(vec![l]), 0.0);
    assert_eq!(px(&p, 50, 50), [255, 0, 0, 255]);
    assert_eq!(px(&p, 10, 10)[3], 0);
}

// The mask path is in layer space; place the layer so layer space == comp space.
fn at_origin_shift(mut l: Value) -> Value {
    l["ks"]["p"] = st(json!([0,0]));
    l["shapes"][0]["p"] = st(json!([50,50]));
    l
}

#[test]
fn mask_subtract_cuts_hole() {
    let mut l = full_red();
    l["masksProperties"] = json!([square_mask("s", false, 50.0, 10.0)]);
    let p = draw(&doc(vec![at_origin_shift(l)]), 0.0);
    assert_eq!(px(&p, 50, 50)[3], 0);
    assert_eq!(px(&p, 10, 10), [255, 0, 0, 255]);
}

#[test]
fn mask_inverted() {
    let mut l = full_red();
    l["masksProperties"] = json!([square_mask("a", true, 50.0, 10.0)]);
    let p = draw(&doc(vec![at_origin_shift(l)]), 0.0);
    assert_eq!(px(&p, 50, 50)[3], 0);
    assert_eq!(px(&p, 10, 10), [255, 0, 0, 255]);
}

#[test]
fn alpha_matte_clips() {
    let mut matte = rect_layer(1, [50.0, 50.0], [20.0, 20.0], [1.0, 1.0, 1.0]);
    matte["td"] = json!(1);
    let mut content = full_red();
    content["ind"] = json!(2);
    content["tt"] = json!(1);
    let p = draw(&doc(vec![matte, content]), 0.0);
    assert_eq!(px(&p, 50, 50), [255, 0, 0, 255]);
    assert_eq!(px(&p, 10, 10)[3], 0, "outside the matte");
}

#[test]
fn luma_matte() {
    let matte = json!({"ty":4,"ind":1,"td":1,"ip":0,"op":60,"st":0,"ks":ks([50.0,50.0]),"shapes":[
        {"ty":"gr","it":[{"ty":"rc","p":st(json!([-25,0])),"s":st(json!([50,100])),"r":st(json!(0))},
                         {"ty":"fl","c":st(json!([1,1,1,1])),"o":st(json!(100))},{"ty":"tr"}]},
        {"ty":"gr","it":[{"ty":"rc","p":st(json!([25,0])),"s":st(json!([50,100])),"r":st(json!(0))},
                         {"ty":"fl","c":st(json!([0,0,0,1])),"o":st(json!(100))},{"ty":"tr"}]}]});
    let mut content = full_red();
    content["ind"] = json!(2);
    content["tt"] = json!(3);
    let p = draw(&doc(vec![matte, content]), 0.0);
    assert_eq!(px(&p, 25, 50), [255, 0, 0, 255]);
    assert_eq!(px(&p, 75, 50)[3], 0);
}

#[test]
fn blend_multiply() {
    let mut top = rect_layer(1, [50.0, 50.0], [100.0, 100.0], [0.0, 1.0, 1.0]);
    top["bm"] = json!(1);
    let bottom = rect_layer(2, [50.0, 50.0], [100.0, 100.0], [1.0, 1.0, 0.0]);
    let p = draw(&doc(vec![top, bottom]), 0.0);
    assert_eq!(px(&p, 50, 50), [0, 255, 0, 255]);
}

fn fx(ty: i64, values: Vec<Value>) -> Value {
    json!({"ty":ty,"en":1,"ef":values.into_iter().map(|v| json!({"ty":0,"v":st(v)})).collect::<Vec<_>>()})
}

#[test]
fn drop_shadow_offsets_dark_pixels() {
    let mut l = rect_layer(1, [40.0, 40.0], [20.0, 20.0], [1.0, 1.0, 1.0]);
    // color, opacity(0-255), direction(deg, 180 = down), distance, softness
    l["ef"] = json!([fx(25, vec![json!([0,0,0,1]), json!(255), json!(180), json!(20), json!(0)])]);
    let p = draw(&doc(vec![l]), 0.0);
    assert_eq!(px(&p, 40, 40), [255, 255, 255, 255], "layer itself unchanged");
    let s = px(&p, 40, 62);
    assert!(s[3] > 200 && s[0] < 30, "shadow below: {s:?}");
}

#[test]
fn gaussian_blur_softens_edge() {
    let mut l = rect_layer(1, [50.0, 50.0], [40.0, 40.0], [1.0, 0.0, 0.0]);
    l["ef"] = json!([fx(29, vec![json!(10), json!(1), json!(0)])]);
    let p = draw(&doc(vec![l]), 0.0);
    let edge = px(&p, 30, 50)[3];
    assert!(edge > 40 && edge < 215, "edge alpha softened: {edge}");
    assert!(px(&p, 24, 50)[3] > 0, "blur spreads outside");
}

#[test]
fn tint_maps_to_gray_range() {
    let mut l = rect_layer(1, [50.0, 50.0], [100.0, 100.0], [1.0, 1.0, 1.0]);
    l["ef"] = json!([fx(20, vec![json!([0,0,0,1]), json!([0,0,1,1]), json!(100)])]);
    assert_eq!(px(&draw(&doc(vec![l]), 0.0), 50, 50), [0, 0, 255, 255]);
}

#[test]
fn fill_effect_recolors() {
    let mut l = rect_layer(1, [50.0, 50.0], [100.0, 100.0], [1.0, 0.0, 0.0]);
    l["ef"] = json!([fx(21, vec![json!(0), json!(0), json!([0,1,0,1]), json!(0), json!(0), json!(0), json!(1)])]);
    assert_eq!(px(&draw(&doc(vec![l]), 0.0), 50, 50), [0, 255, 0, 255]);
}

#[test]
fn tritone_shadows_for_black() {
    let mut l = rect_layer(1, [50.0, 50.0], [100.0, 100.0], [0.0, 0.0, 0.0]);
    l["ef"] = json!([fx(23, vec![json!([1,1,1,1]), json!([0.5,0.5,0.5,1]), json!([1,0,0,1]), json!(0)])]);
    assert_eq!(px(&draw(&doc(vec![l]), 0.0), 50, 50), [255, 0, 0, 255]);
}

#[test]
fn unused_helper_compiles() { let _ = at_origin(json!({"ks":{}})); }
