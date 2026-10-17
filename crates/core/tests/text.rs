use lumetrope_core::model::{self, TextDoc};
use lumetrope_core::render::{render, RenderCtx};
use lumetrope_core::text::{layout, FontRegistry};
use serde_json::json;

fn doc(text: &str, justify: i64, font: &str) -> TextDoc {
    TextDoc { text: text.into(), size: 20.0, font: font.into(), justify, line_height: 24.0, fill: Some(vec![0.0, 0.0, 0.0]), ..Default::default() }
}

fn bounds(paths: &[lumetrope_core::geom::Path]) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in paths {
        if let Some(r) = p.bounds() {
            b = [b[0].min(r[0]), b[1].min(r[1]), b[2].max(r[2]), b[3].max(r[3])];
        }
    }
    b
}

#[test]
fn text_renders_ink_pixels() {
    let v = json!({"w":100,"h":100,"fr":30,"op":30,"layers":[{"ty":5,"ind":1,"ip":0,"op":30,"st":0,
        "ks":{"p":{"a":0,"k":[10,60]}},
        "t":{"d":{"k":[{"t":0,"s":{"t":"Hi","s":40,"f":"Poppins","j":0,"tr":0,"lh":48,"fc":[1,0,0]}}]}}}]});
    let a = model::parse(&v).unwrap();
    let p = render(&a, &RenderCtx::default(), 0.0, 100, 100);
    let ink = p.pixels().iter().filter(|c| c.alpha() > 128).count();
    assert!(ink > 100, "ink pixels {ink}");
}

#[test]
fn center_justify_centers_bbox() {
    let paths = layout(&doc("Hello", 2, "Poppins"), &FontRegistry::default());
    let b = bounds(&paths);
    assert!(((b[0] + b[2]) / 2.0).abs() < 3.0, "center {:?}", b);
    let left = bounds(&layout(&doc("Hello", 0, "Poppins"), &FontRegistry::default()));
    assert!(left[0] >= -1.0, "left aligned starts at 0: {:?}", left);
}

#[test]
fn unknown_font_falls_back() {
    assert!(!layout(&doc("Ab", 0, "NoSuchFont"), &FontRegistry::default()).is_empty());
}

#[test]
fn second_line_is_below_first() {
    let paths = layout(&doc("A\rA", 0, ""), &FontRegistry::default());
    assert_eq!(paths.len(), 2);
    let (a, b) = (paths[0].bounds().unwrap(), paths[1].bounds().unwrap());
    assert!((b[1] - a[1] - 24.0).abs() < 0.5, "line height applied: {a:?} {b:?}");
}

#[test]
fn add_font_rejects_garbage() {
    let mut r = FontRegistry::default();
    assert!(!r.add("x", vec![1, 2, 3]));
}
