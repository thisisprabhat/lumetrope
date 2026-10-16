//! Lenient, typed, read-only view of a Lottie document.
//!
//! The JSON `Value` stays the source of truth (so unknown data round-trips);
//! this model is rebuilt from it whenever the document changes and is what the
//! renderer, hit-tester and state machine read. Missing or ill-typed fields fall
//! back to defaults instead of failing.

use serde_json::{Map, Value};

pub type Vec2 = [f64; 2];

#[derive(Debug, Clone, PartialEq)]
pub struct ModelError(pub String);

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ease {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keyframe<T> {
    pub t: f64,
    pub s: T,
    /// Legacy (bodymovin < 5.5) explicit end value.
    pub e: Option<T>,
    pub i: Option<Ease>,
    pub o: Option<Ease>,
    pub h: bool,
    /// Spatial tangents (position keyframes).
    pub to: Option<Vec<f64>>,
    pub ti: Option<Vec<f64>>,
}

/// A numeric animatable property (scalar, vector or colour).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prop {
    pub animated: bool,
    pub value: Vec<f64>,
    pub keyframes: Vec<Keyframe<Vec<f64>>>,
    pub sid: Option<String>,
    /// `k` was written as a bare number rather than an array.
    pub scalar: bool,
    pub expr: Option<String>,
}

impl Prop {
    pub fn fixed(value: Vec<f64>) -> Self {
        Prop { value, ..Default::default() }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bezier {
    pub c: bool,
    pub v: Vec<Vec2>,
    pub i: Vec<Vec2>,
    pub o: Vec<Vec2>,
}

impl Bezier {
    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapeProp {
    pub animated: bool,
    pub value: Bezier,
    pub keyframes: Vec<Keyframe<Bezier>>,
    pub sid: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transform {
    pub a: Option<Prop>,
    pub p: Option<Prop>,
    pub px: Option<Prop>,
    pub py: Option<Prop>,
    pub s: Option<Prop>,
    pub r: Option<Prop>,
    pub o: Option<Prop>,
    pub sk: Option<Prop>,
    pub sa: Option<Prop>,
    /// Repeater start/end opacity.
    pub so: Option<Prop>,
    pub eo: Option<Prop>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Common {
    pub nm: String,
    pub hd: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub c: Common,
    pub items: Vec<Shape>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub c: Common,
    pub p: Option<Prop>,
    pub s: Option<Prop>,
    pub r: Option<Prop>,
    pub d: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ellipse {
    pub c: Common,
    pub p: Option<Prop>,
    pub s: Option<Prop>,
    pub d: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Star {
    pub c: Common,
    /// 1 = star, 2 = polygon.
    pub sy: i64,
    pub p: Option<Prop>,
    pub pt: Option<Prop>,
    pub or: Option<Prop>,
    pub ir: Option<Prop>,
    pub os: Option<Prop>,
    pub is: Option<Prop>,
    pub r: Option<Prop>,
    pub d: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathShape {
    pub c: Common,
    pub ks: Option<ShapeProp>,
    pub d: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fill {
    pub c: Common,
    pub color: Option<Prop>,
    pub o: Option<Prop>,
    /// 1 = non-zero, 2 = even-odd.
    pub rule: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dash {
    /// "d" dash, "g" gap, "o" offset.
    pub n: String,
    pub v: Option<Prop>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StrokeStyle {
    pub w: Option<Prop>,
    /// 1 butt, 2 round, 3 square.
    pub lc: i64,
    /// 1 miter, 2 round, 3 bevel.
    pub lj: i64,
    pub ml: f64,
    pub dashes: Vec<Dash>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stroke {
    pub c: Common,
    pub color: Option<Prop>,
    pub o: Option<Prop>,
    pub style: StrokeStyle,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gradient {
    /// 1 = linear, 2 = radial.
    pub t: i64,
    pub s: Option<Prop>,
    pub e: Option<Prop>,
    pub h: Option<Prop>,
    pub a: Option<Prop>,
    /// Number of colour stops.
    pub count: usize,
    pub colors: Option<Prop>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GFill {
    pub c: Common,
    pub g: Gradient,
    pub o: Option<Prop>,
    pub rule: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GStroke {
    pub c: Common,
    pub g: Gradient,
    pub o: Option<Prop>,
    pub style: StrokeStyle,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapeTransform {
    pub c: Common,
    pub tr: Transform,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trim {
    pub c: Common,
    pub s: Option<Prop>,
    pub e: Option<Prop>,
    pub o: Option<Prop>,
    /// 1 = simultaneously, 2 = individually.
    pub m: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Round {
    pub c: Common,
    pub r: Option<Prop>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Repeater {
    pub c: Common,
    pub copies: Option<Prop>,
    pub offset: Option<Prop>,
    /// 1 = above, 2 = below.
    pub m: i64,
    pub tr: Transform,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Group(Group),
    Rect(Rect),
    Ellipse(Ellipse),
    Star(Star),
    Path(PathShape),
    Fill(Fill),
    Stroke(Stroke),
    GFill(GFill),
    GStroke(GStroke),
    Transform(ShapeTransform),
    Trim(Trim),
    Round(Round),
    Repeater(Repeater),
    Unknown { ty: String, c: Common },
}

impl Shape {
    pub fn common(&self) -> &Common {
        match self {
            Shape::Group(x) => &x.c,
            Shape::Rect(x) => &x.c,
            Shape::Ellipse(x) => &x.c,
            Shape::Star(x) => &x.c,
            Shape::Path(x) => &x.c,
            Shape::Fill(x) => &x.c,
            Shape::Stroke(x) => &x.c,
            Shape::GFill(x) => &x.c,
            Shape::GStroke(x) => &x.c,
            Shape::Transform(x) => &x.c,
            Shape::Trim(x) => &x.c,
            Shape::Round(x) => &x.c,
            Shape::Repeater(x) => &x.c,
            Shape::Unknown { c, .. } => c,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mask {
    /// a add, s subtract, i intersect, l lighten, d darken, f difference, n none.
    pub mode: String,
    pub pt: Option<ShapeProp>,
    pub o: Option<Prop>,
    pub inv: bool,
    pub x: Option<Prop>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectValue {
    pub ty: i64,
    pub nm: String,
    pub v: Option<Prop>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effect {
    pub ty: i64,
    pub nm: String,
    pub enabled: bool,
    pub values: Vec<EffectValue>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextDoc {
    pub text: String,
    pub size: f64,
    pub font: String,
    /// 0 left, 1 right, 2 center, 3.. justify variants.
    pub justify: i64,
    pub tracking: f64,
    pub line_height: f64,
    pub baseline: f64,
    pub fill: Option<Vec<f64>>,
    pub stroke: Option<Vec<f64>>,
    pub stroke_width: f64,
    pub stroke_over_fill: bool,
    pub box_size: Option<Vec2>,
    pub box_pos: Option<Vec2>,
    /// 0 regular, 1 all caps, 2 small caps.
    pub caps: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextData {
    /// (start frame, document) pairs; text documents hold, never interpolate.
    pub docs: Vec<(f64, TextDoc)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layer {
    pub ty: i64,
    pub nm: String,
    pub ind: Option<i64>,
    pub parent: Option<i64>,
    pub ip: f64,
    pub op: f64,
    pub st: f64,
    pub sr: f64,
    pub ks: Transform,
    pub ao: bool,
    pub hd: bool,
    pub bm: i64,
    pub tt: i64,
    pub tp: Option<i64>,
    pub td: i64,
    pub masks: Vec<Mask>,
    pub ef: Vec<Effect>,
    pub ref_id: Option<String>,
    pub w: f64,
    pub h: f64,
    pub tm: Option<Prop>,
    pub sc: Option<String>,
    pub sw: f64,
    pub sh: f64,
    pub shapes: Vec<Shape>,
    pub text: Option<TextData>,
    /// Index of this layer in its JSON `layers` array.
    pub json_index: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Asset {
    pub id: String,
    pub nm: String,
    pub layers: Option<Vec<Layer>>,
    pub w: f64,
    pub h: f64,
    pub u: String,
    pub p: String,
    pub fr: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marker {
    pub name: String,
    pub time: f64,
    pub duration: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontDef {
    pub name: String,
    pub family: String,
    pub style: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Animation {
    pub nm: String,
    pub fr: f64,
    pub ip: f64,
    pub op: f64,
    pub w: f64,
    pub h: f64,
    pub layers: Vec<Layer>,
    pub assets: Vec<Asset>,
    pub markers: Vec<Marker>,
    pub slots: Map<String, Value>,
    pub fonts: Vec<FontDef>,
}

impl Animation {
    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }
}

// ---------------------------------------------------------------- helpers

pub(crate) fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Array(a) if a.len() == 1 => num(&a[0]),
        _ => None,
    }
}

fn f(o: &Value, k: &str, d: f64) -> f64 {
    o.get(k).and_then(num).unwrap_or(d)
}

fn i(o: &Value, k: &str, d: i64) -> i64 {
    o.get(k).and_then(num).map(|x| x as i64).unwrap_or(d)
}

fn s(o: &Value, k: &str) -> String {
    o.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn b(o: &Value, k: &str) -> bool {
    o.get(k).and_then(num).map(|x| x != 0.0).unwrap_or(false)
}

fn nums(v: &Value) -> Option<Vec<f64>> {
    match v {
        Value::Number(_) => num(v).map(|x| vec![x]),
        Value::Array(a) => a.iter().map(num).collect(),
        _ => None,
    }
}

fn vec2s(v: Option<&Value>) -> Vec<Vec2> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| {
                    let n = nums(p).unwrap_or_default();
                    [n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0)]
                })
                .collect()
        })
        .unwrap_or_default()
}

fn common(o: &Value) -> Common {
    Common { nm: s(o, "nm"), hd: b(o, "hd") }
}

fn ease(v: Option<&Value>) -> Option<Ease> {
    let v = v?;
    Some(Ease { x: v.get("x").and_then(nums).unwrap_or_default(), y: v.get("y").and_then(nums).unwrap_or_default() })
}

fn is_keyframe_list(k: &Value) -> bool {
    k.as_array().map(|a| a.first().map(|x| x.is_object() && x.get("t").is_some()).unwrap_or(false)).unwrap_or(false)
}

/// Parse a numeric animatable property.
pub fn prop(v: Option<&Value>) -> Option<Prop> {
    let v = v?;
    let (k, obj) = match v.get("k") {
        Some(k) => (k, true),
        None if v.is_number() || v.is_array() => (v, false),
        None => return None,
    };
    let mut p = Prop {
        sid: if obj { v.get("sid").and_then(Value::as_str).map(str::to_string) } else { None },
        expr: if obj { v.get("x").and_then(Value::as_str).map(str::to_string) } else { None },
        ..Default::default()
    };
    if is_keyframe_list(k) {
        p.animated = true;
        for kf in k.as_array().unwrap() {
            p.keyframes.push(Keyframe {
                t: f(kf, "t", 0.0),
                s: kf.get("s").and_then(nums).unwrap_or_default(),
                e: kf.get("e").and_then(nums),
                i: ease(kf.get("i")),
                o: ease(kf.get("o")),
                h: b(kf, "h"),
                to: kf.get("to").and_then(nums),
                ti: kf.get("ti").and_then(nums),
            });
        }
        p.value = p.keyframes.first().map(|k| k.s.clone()).unwrap_or_default();
    } else {
        p.scalar = k.is_number();
        p.value = nums(k).unwrap_or_default();
    }
    Some(p)
}

pub fn bezier(v: &Value) -> Bezier {
    let v = match v {
        Value::Array(a) if !a.is_empty() => &a[0],
        _ => v,
    };
    let vs = vec2s(v.get("v"));
    let n = vs.len();
    let mut ins = vec2s(v.get("i"));
    let mut outs = vec2s(v.get("o"));
    ins.resize(n, [0.0, 0.0]);
    outs.resize(n, [0.0, 0.0]);
    Bezier { c: b(v, "c"), v: vs, i: ins, o: outs }
}

pub fn shape_prop(v: Option<&Value>) -> Option<ShapeProp> {
    let v = v?;
    let k = v.get("k")?;
    let mut p = ShapeProp { sid: v.get("sid").and_then(Value::as_str).map(str::to_string), ..Default::default() };
    if is_keyframe_list(k) {
        p.animated = true;
        for kf in k.as_array().unwrap() {
            p.keyframes.push(Keyframe {
                t: f(kf, "t", 0.0),
                s: kf.get("s").map(bezier).unwrap_or_default(),
                e: kf.get("e").map(bezier),
                i: ease(kf.get("i")),
                o: ease(kf.get("o")),
                h: b(kf, "h"),
                to: None,
                ti: None,
            });
        }
        p.value = p.keyframes.first().map(|k| k.s.clone()).unwrap_or_default();
    } else {
        p.value = bezier(k);
    }
    Some(p)
}

pub fn transform(v: Option<&Value>) -> Transform {
    let Some(v) = v else { return Transform::default() };
    let mut t = Transform {
        a: prop(v.get("a")),
        s: prop(v.get("s")),
        r: prop(v.get("r")).or_else(|| prop(v.get("rz"))),
        o: prop(v.get("o")),
        sk: prop(v.get("sk")),
        sa: prop(v.get("sa")),
        so: prop(v.get("so")),
        eo: prop(v.get("eo")),
        ..Default::default()
    };
    match v.get("p") {
        Some(p) if p.get("s").map(|x| num(x) == Some(1.0)).unwrap_or(false) => {
            t.px = prop(p.get("x"));
            t.py = prop(p.get("y"));
        }
        p => t.p = prop(p),
    }
    t
}

fn stroke_style(o: &Value) -> StrokeStyle {
    StrokeStyle {
        w: prop(o.get("w")),
        lc: i(o, "lc", 2),
        lj: i(o, "lj", 2),
        ml: f(o, "ml", 4.0),
        dashes: o
            .get("d")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|d| Dash { n: s(d, "n"), v: prop(d.get("v")) }).collect())
            .unwrap_or_default(),
    }
}

fn gradient(o: &Value) -> Gradient {
    let g = o.get("g");
    Gradient {
        t: i(o, "t", 1),
        s: prop(o.get("s")),
        e: prop(o.get("e")),
        h: prop(o.get("h")),
        a: prop(o.get("a")),
        count: g.map(|g| i(g, "p", 0).max(0) as usize).unwrap_or(0),
        colors: g.and_then(|g| prop(g.get("k"))),
    }
}

pub fn shape(o: &Value) -> Shape {
    let c = common(o);
    match o.get("ty").and_then(Value::as_str).unwrap_or("") {
        "gr" => Shape::Group(Group { c, items: shapes(o.get("it")) }),
        "rc" => Shape::Rect(Rect { c, p: prop(o.get("p")), s: prop(o.get("s")), r: prop(o.get("r")), d: i(o, "d", 1) }),
        "el" => Shape::Ellipse(Ellipse { c, p: prop(o.get("p")), s: prop(o.get("s")), d: i(o, "d", 1) }),
        "sr" => Shape::Star(Star {
            c,
            sy: i(o, "sy", 1),
            p: prop(o.get("p")),
            pt: prop(o.get("pt")),
            or: prop(o.get("or")),
            ir: prop(o.get("ir")),
            os: prop(o.get("os")),
            is: prop(o.get("is")),
            r: prop(o.get("r")),
            d: i(o, "d", 1),
        }),
        "sh" => Shape::Path(PathShape { c, ks: shape_prop(o.get("ks")), d: i(o, "d", 1) }),
        "fl" => Shape::Fill(Fill { c, color: prop(o.get("c")), o: prop(o.get("o")), rule: i(o, "r", 1) }),
        "st" => Shape::Stroke(Stroke { c, color: prop(o.get("c")), o: prop(o.get("o")), style: stroke_style(o) }),
        "gf" => Shape::GFill(GFill { c, g: gradient(o), o: prop(o.get("o")), rule: i(o, "r", 1) }),
        "gs" => Shape::GStroke(GStroke { c, g: gradient(o), o: prop(o.get("o")), style: stroke_style(o) }),
        "tr" => Shape::Transform(ShapeTransform { c, tr: transform(Some(o)) }),
        "tm" => Shape::Trim(Trim { c, s: prop(o.get("s")), e: prop(o.get("e")), o: prop(o.get("o")), m: i(o, "m", 1) }),
        "rd" => Shape::Round(Round { c, r: prop(o.get("r")) }),
        "rp" => Shape::Repeater(Repeater {
            c,
            copies: prop(o.get("c")),
            offset: prop(o.get("o")),
            m: i(o, "m", 1),
            tr: transform(o.get("tr")),
        }),
        other => Shape::Unknown { ty: other.to_string(), c },
    }
}

pub fn shapes(v: Option<&Value>) -> Vec<Shape> {
    v.and_then(Value::as_array).map(|a| a.iter().map(shape).collect()).unwrap_or_default()
}

fn text_doc(d: &Value) -> TextDoc {
    let pair = |k: &str| {
        d.get(k).and_then(nums).filter(|n| n.len() >= 2).map(|n| [n[0], n[1]])
    };
    TextDoc {
        text: s(d, "t"),
        size: f(d, "s", 32.0),
        font: s(d, "f"),
        justify: i(d, "j", 0),
        tracking: f(d, "tr", 0.0),
        line_height: f(d, "lh", f(d, "s", 32.0) * 1.2),
        baseline: f(d, "ls", 0.0),
        fill: d.get("fc").and_then(nums),
        stroke: d.get("sc").and_then(nums),
        stroke_width: f(d, "sw", 0.0),
        stroke_over_fill: b(d, "of"),
        box_size: pair("sz"),
        box_pos: pair("ps"),
        caps: i(d, "ca", 0),
    }
}

fn text_data(t: Option<&Value>) -> Option<TextData> {
    let k = t?.get("d")?.get("k")?;
    let docs = match k {
        Value::Array(a) => a.iter().map(|kf| (f(kf, "t", 0.0), kf.get("s").map(text_doc).unwrap_or_default())).collect(),
        Value::Object(_) => vec![(0.0, text_doc(k))],
        _ => vec![],
    };
    Some(TextData { docs })
}

fn effects(v: Option<&Value>) -> Vec<Effect> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|e| Effect {
                    ty: i(e, "ty", 0),
                    nm: s(e, "nm"),
                    enabled: e.get("en").and_then(num).map(|x| x != 0.0).unwrap_or(true),
                    values: e
                        .get("ef")
                        .and_then(Value::as_array)
                        .map(|vs| vs.iter().map(|x| EffectValue { ty: i(x, "ty", 0), nm: s(x, "nm"), v: prop(x.get("v")) }).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn masks(v: Option<&Value>) -> Vec<Mask> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|m| Mask {
                    mode: m.get("mode").and_then(Value::as_str).unwrap_or("a").to_string(),
                    pt: shape_prop(m.get("pt")),
                    o: prop(m.get("o")),
                    inv: b(m, "inv"),
                    x: prop(m.get("x")),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn layer(o: &Value, json_index: usize, default_op: f64) -> Layer {
    Layer {
        ty: i(o, "ty", 3),
        nm: s(o, "nm"),
        ind: o.get("ind").and_then(num).map(|x| x as i64),
        parent: o.get("parent").and_then(num).map(|x| x as i64),
        ip: f(o, "ip", 0.0),
        op: f(o, "op", default_op),
        st: f(o, "st", 0.0),
        sr: f(o, "sr", 1.0),
        ks: transform(o.get("ks")),
        ao: b(o, "ao"),
        hd: b(o, "hd"),
        bm: i(o, "bm", 0),
        tt: i(o, "tt", 0),
        tp: o.get("tp").and_then(num).map(|x| x as i64),
        td: i(o, "td", 0),
        masks: masks(o.get("masksProperties")),
        ef: effects(o.get("ef")),
        ref_id: o.get("refId").and_then(Value::as_str).map(str::to_string),
        w: f(o, "w", 0.0),
        h: f(o, "h", 0.0),
        tm: prop(o.get("tm")),
        sc: o.get("sc").and_then(Value::as_str).map(str::to_string),
        sw: f(o, "sw", 0.0),
        sh: f(o, "sh", 0.0),
        shapes: shapes(o.get("shapes")),
        text: text_data(o.get("t")),
        json_index,
    }
}

pub fn layers(v: Option<&Value>, default_op: f64) -> Vec<Layer> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().enumerate().map(|(n, l)| layer(l, n, default_op)).collect())
        .unwrap_or_default()
}

/// Build the typed model from a Lottie JSON value.
pub fn parse(v: &Value) -> Result<Animation, ModelError> {
    if !v.is_object() {
        return Err(ModelError("Lottie document must be a JSON object".into()));
    }
    let op = f(v, "op", 60.0);
    Ok(Animation {
        nm: s(v, "nm"),
        fr: f(v, "fr", 30.0).max(1e-3),
        ip: f(v, "ip", 0.0),
        op,
        w: f(v, "w", 512.0),
        h: f(v, "h", 512.0),
        layers: layers(v.get("layers"), op),
        assets: v
            .get("assets")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|x| Asset {
                        id: s(x, "id"),
                        nm: s(x, "nm"),
                        layers: x.get("layers").map(|l| layers(Some(l), op)),
                        w: f(x, "w", 0.0),
                        h: f(x, "h", 0.0),
                        u: s(x, "u"),
                        p: s(x, "p"),
                        fr: x.get("fr").and_then(num),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        markers: v
            .get("markers")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|m| Marker { name: s(m, "cm"), time: f(m, "tm", 0.0), duration: f(m, "dr", 0.0) }).collect())
            .unwrap_or_default(),
        slots: v.get("slots").and_then(Value::as_object).cloned().unwrap_or_default(),
        fonts: v
            .get("fonts")
            .and_then(|x| x.get("list"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|x| FontDef { name: s(x, "fName"), family: s(x, "fFamily"), style: s(x, "fStyle"), path: s(x, "fPath") })
                    .collect()
            })
            .unwrap_or_default(),
    })
}
