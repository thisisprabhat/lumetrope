//! Text layer tools.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, default_ks, parse_color};
use super::{arg_str, err, opt_f64, opt_str, opt_vec, Cx, OpError, OpResult};

pub const DEFAULT_FONT: &str = "Poppins-Regular";

fn style_props() -> Value {
    json!({
        "font": {"type": "string", "description": "Font name (registered fonts; default Poppins-Regular)"},
        "font_size": {"type": "number", "exclusiveMinimum": 0},
        "color": {"description": "Fill colour"},
        "stroke_color": {"description": "Stroke colour (null removes)"},
        "stroke_width": {"type": "number"},
        "justify": {"type": "string", "enum": ["left", "center", "right"]},
        "tracking": {"type": "number", "description": "Letter spacing in 1/1000 em"},
        "line_height": {"type": "number"},
        "caps": {"type": "string", "enum": ["regular", "all_caps", "small_caps"]}
    })
}

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "create_text",
            description: "Create a text layer. Position is the text anchor (baseline centre for centred text); give 'box' [w,h] for wrapped paragraph text.",
            schema: || {
                let mut p = style_props();
                for (k, v) in [("text", json!({"type": "string"})), ("position", json!({"type": "array"})), ("name", json!({"type": "string"})), ("box", json!({"type": "array"})), ("scene", json!({"type": "string"}))] {
                    p[k] = v;
                }
                obj(p, &["text"])
            },
            mutates: true,
            handler: create_text,
        },
        ToolDef {
            name: "set_text",
            description: "Change a text layer's content (use \\r or \\n for line breaks).",
            schema: || obj(json!({"layer": {"type": "string"}, "text": {"type": "string"}, "frame": {"type": "number"}}), &["layer", "text"]),
            mutates: true,
            handler: set_text,
        },
        ToolDef {
            name: "set_text_style",
            description: "Change font, size, colours, stroke, justification, tracking, line height or caps of a text layer.",
            schema: || {
                let mut p = style_props();
                p["layer"] = json!({"type": "string"});
                obj(p, &["layer"])
            },
            mutates: true,
            handler: set_text_style,
        },
    ]
}

fn rgb(v: &Value) -> OpResult<Value> {
    let c = parse_color(v)?;
    Ok(json!([c[0], c[1], c[2]]))
}

fn justify_code(s: &str) -> OpResult<i64> {
    match s {
        "left" => Ok(0),
        "right" => Ok(1),
        "center" => Ok(2),
        o => err(format!("justify must be left, center or right, not '{o}'")),
    }
}

fn apply_style(doc: &mut Value, a: &Value) -> OpResult<()> {
    if let Some(f) = opt_str(a, "font") {
        doc["f"] = json!(f);
    }
    if let Some(s) = opt_f64(a, "font_size") {
        if s <= 0.0 {
            return err("font_size must be > 0");
        }
        doc["s"] = json!(s);
        if a.get("line_height").is_none() {
            doc["lh"] = json!(s * 1.2);
        }
    }
    if let Some(c) = a.get("color") {
        doc["fc"] = rgb(c)?;
    }
    match a.get("stroke_color") {
        Some(Value::Null) => {
            if let Some(o) = doc.as_object_mut() {
                o.remove("sc");
                o.remove("sw");
            }
        }
        Some(c) => {
            doc["sc"] = rgb(c)?;
            if doc.get("sw").is_none() {
                doc["sw"] = json!(1.0);
            }
            doc["of"] = json!(false);
        }
        None => {}
    }
    if let Some(w) = opt_f64(a, "stroke_width") {
        doc["sw"] = json!(w);
    }
    if let Some(j) = opt_str(a, "justify") {
        doc["j"] = json!(justify_code(j)?);
    }
    if let Some(t) = opt_f64(a, "tracking") {
        doc["tr"] = json!(t);
    }
    if let Some(l) = opt_f64(a, "line_height") {
        doc["lh"] = json!(l);
    }
    if let Some(c) = opt_str(a, "caps") {
        doc["ca"] = json!(match c {
            "regular" => 0,
            "all_caps" => 1,
            "small_caps" => 2,
            o => return err(format!("unknown caps '{o}'")),
        });
    }
    Ok(())
}

/// Make sure the animation's font list has an entry for `name`.
fn ensure_font(cx: &mut Cx, name: &str) -> OpResult<()> {
    let fp = format!("{}/fonts/list", cx.anim_ptr());
    let list = cx.get(&fp).and_then(Value::as_array).cloned().unwrap_or_default();
    if list.iter().any(|f| f.get("fName").and_then(Value::as_str) == Some(name)) {
        return Ok(());
    }
    let (family, style) = name.split_once('-').unwrap_or((name, "Regular"));
    cx.insert(&fp, list.len(), json!({"fName": name, "fFamily": family, "fStyle": style, "ascent": 75}))?;
    Ok(())
}

fn create_text(cx: &mut Cx, a: &Value) -> OpResult {
    let text = arg_str(a, "text")?;
    let (w, h, _, ip, op) = util::scene(cx);
    let pos = opt_vec(a, "position").map(|p| [p[0], p.get(1).copied().unwrap_or(0.0)]).unwrap_or([w / 2.0, h / 2.0]);
    let size = opt_f64(a, "font_size").unwrap_or(48.0);
    let mut doc = json!({"s": size, "f": DEFAULT_FONT, "t": text.replace('\n', "\r"), "j": 2, "tr": 0, "lh": size * 1.2, "ls": 0, "fc": [0.0, 0.0, 0.0]});
    apply_style(&mut doc, a)?;
    if let Some(b) = opt_vec(a, "box") {
        let (bw, bh) = (b[0], b.get(1).copied().unwrap_or(b[0]));
        doc["sz"] = json!([bw, bh]);
        doc["ps"] = json!([-bw / 2.0, -bh / 2.0]);
    }
    let font = doc["f"].as_str().unwrap_or(DEFAULT_FONT).to_string();
    ensure_font(cx, &font)?;
    let scene = opt_str(a, "scene").unwrap_or("root").to_string();
    let lp = cx.comp_layers_ptr(&scene)?;
    let ind = cx.next_ind(&lp);
    let name = opt_str(a, "name").map(str::to_string).unwrap_or_else(|| text.chars().take(24).collect());
    let layer = json!({"ddd": 0, "ind": ind, "ty": 5, "nm": name, "sr": 1, "ks": default_ks(pos), "ao": 0, "ip": ip, "op": op, "st": 0, "bm": 0,
        "t": {"d": {"k": [{"s": doc, "t": 0}]}, "p": {}, "m": {"g": 1, "a": {"a": 0, "k": [0, 0]}}, "a": []}});
    cx.insert(&lp, 0, layer)?;
    Ok(json!({"id": format!("{scene}:{ind}")}))
}

fn docs_ptr(cx: &Cx, a: &Value) -> OpResult<(String, usize)> {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let dp = format!("{}/t/d/k", r.ptr());
    let n = cx.get(&dp).and_then(Value::as_array).map(Vec::len).ok_or_else(|| OpError(format!("layer {} is not a text layer", r.id)))?;
    Ok((dp, n))
}

fn set_text(cx: &mut Cx, a: &Value) -> OpResult {
    let text = a.get("text").and_then(Value::as_str).ok_or_else(|| OpError("missing string argument 'text'".into()))?;
    let (dp, n) = docs_ptr(cx, a)?;
    let frame = opt_f64(a, "frame").unwrap_or(0.0);
    let docs = cx.get(&dp).and_then(Value::as_array).cloned().unwrap_or_default();
    let i = docs.iter().rposition(|d| d.get("t").and_then(Value::as_f64).unwrap_or(0.0) <= frame).unwrap_or(0).min(n.saturating_sub(1));
    cx.set(&format!("{dp}/{i}/s/t"), json!(text.replace('\n', "\r")))?;
    Ok(json!({"ok": true}))
}

fn set_text_style(cx: &mut Cx, a: &Value) -> OpResult {
    let (dp, n) = docs_ptr(cx, a)?;
    for i in 0..n {
        let mut doc = cx.get(&format!("{dp}/{i}/s")).cloned().unwrap_or_else(|| json!({}));
        apply_style(&mut doc, a)?;
        cx.set(&format!("{dp}/{i}/s"), doc)?;
    }
    if let Some(f) = opt_str(a, "font") {
        ensure_font(cx, f)?;
    }
    Ok(json!({"ok": true}))
}
