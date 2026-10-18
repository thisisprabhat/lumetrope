//! Import tools: SVG, Lottie (as a nested scene) and images.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, default_ks, static_prop};
use super::{opt_str, opt_vec, Cx, OpError, OpResult};
use crate::svg;

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "import_svg",
            description: "Import SVG markup as a new editable shape layer (paths, fills, strokes and gradients), centred in the scene unless a position is given.",
            schema: || obj(json!({"svg": {"type": "string"}, "name": {"type": "string"}, "position": {"type": "array"}, "scene": {"type": "string"}}), &["svg"]),
            mutates: true,
            handler: import_svg,
        },
        ToolDef {
            name: "import_lottie",
            description: "Import another Lottie animation (JSON string or object) as a nested scene layer; its assets are merged with remapped ids.",
            schema: || obj(json!({"json": {}, "name": {"type": "string"}, "position": {"type": "array"}}), &["json"]),
            mutates: true,
            handler: import_lottie,
        },
        ToolDef {
            name: "import_image",
            description: "Embed a PNG or JPEG (data: URL) as an image asset and add an image layer centred in the scene.",
            schema: || obj(json!({"data": {"type": "string", "description": "data:image/png;base64,…"}, "name": {"type": "string"}, "position": {"type": "array"}, "width": {"type": "number"}, "height": {"type": "number"}}), &["data"]),
            mutates: true,
            handler: import_image,
        },
    ]
}

fn center(cx: &Cx, a: &Value) -> [f64; 2] {
    let (w, h, ..) = util::scene(cx);
    opt_vec(a, "position").map(|p| [p[0], p.get(1).copied().unwrap_or(0.0)]).unwrap_or([w / 2.0, h / 2.0])
}

fn insert_layer(cx: &mut Cx, scene: &str, mut layer: Value) -> OpResult<String> {
    let lp = cx.comp_layers_ptr(scene)?;
    let ind = cx.next_ind(&lp);
    let (.., ip, op) = util::scene(cx);
    layer["ind"] = json!(ind);
    for (k, v) in [("ddd", json!(0)), ("sr", json!(1)), ("ao", json!(0)), ("ip", json!(ip)), ("op", json!(op)), ("st", json!(0)), ("bm", json!(0))] {
        if layer.get(k).is_none() {
            layer[k] = v;
        }
    }
    cx.insert(&lp, 0, layer)?;
    Ok(format!("{scene}:{ind}"))
}

fn import_svg(cx: &mut Cx, a: &Value) -> OpResult {
    let text = a.get("svg").and_then(Value::as_str).ok_or_else(|| OpError("missing 'svg' markup".into()))?;
    let (shapes, size) = svg::to_shapes(text).map_err(OpError)?;
    if shapes.is_empty() {
        return Err(OpError("SVG contains no drawable paths".into()));
    }
    let pos = center(cx, a);
    let mut ks = default_ks(pos);
    ks["a"] = static_prop(json!([size[0] / 2.0, size[1] / 2.0, 0.0]));
    let name = opt_str(a, "name").unwrap_or("SVG");
    let scene = opt_str(a, "scene").unwrap_or("root").to_string();
    let id = insert_layer(cx, &scene, json!({"ty": 4, "nm": name, "ks": ks, "shapes": shapes}))?;
    Ok(json!({"id": id}))
}

/// Rewrite every asset id / refId in `v` through `map`.
fn remap_refs(v: &mut Value, map: &[(String, String)]) {
    match v {
        Value::Object(o) => {
            for key in ["refId", "id"] {
                if let Some(Value::String(s)) = o.get(key) {
                    if let Some((_, n)) = map.iter().find(|(old, _)| old == s) {
                        if key == "refId" || o.contains_key("layers") || o.contains_key("p") {
                            o.insert(key.into(), json!(n));
                        }
                    }
                }
            }
            for (_, x) in o.iter_mut() {
                remap_refs(x, map);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| remap_refs(x, map)),
        _ => {}
    }
}

fn import_lottie(cx: &mut Cx, a: &Value) -> OpResult {
    let src: Value = match a.get("json") {
        Some(Value::String(s)) => serde_json::from_str(s).map_err(|e| OpError(format!("invalid Lottie JSON: {e}")))?,
        Some(v @ Value::Object(_)) => v.clone(),
        _ => return Err(OpError("missing 'json'".into())),
    };
    let model = crate::model::parse(&src).map_err(|e| OpError(e.0))?;
    let existing: Vec<String> = cx.anim().get("assets").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.get("id").and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
    let mut taken = existing.clone();
    let fresh = |base: &str, taken: &mut Vec<String>| -> String {
        let id = (1..).map(|n| format!("{base}_{n}")).find(|c| !taken.contains(c)).unwrap();
        taken.push(id.clone());
        id
    };
    let mut map = vec![];
    let mut assets = src.get("assets").and_then(Value::as_array).cloned().unwrap_or_default();
    for asset in &assets {
        if let Some(id) = asset.get("id").and_then(Value::as_str) {
            map.push((id.to_string(), fresh("imported", &mut taken)));
        }
    }
    for asset in assets.iter_mut() {
        remap_refs(asset, &map);
    }
    let mut layers = src.get("layers").cloned().unwrap_or(json!([]));
    remap_refs(&mut layers, &map);
    let scene_id = fresh("comp", &mut taken);
    let name = opt_str(a, "name").map(str::to_string).unwrap_or_else(|| if model.nm.is_empty() { "Imported".into() } else { model.nm.clone() });
    let ap = format!("{}/assets", cx.anim_ptr());
    let mut n = existing.len();
    for asset in assets {
        cx.insert(&ap, n, asset)?;
        n += 1;
    }
    cx.insert(&ap, n, json!({"id": scene_id, "nm": name, "fr": model.fr, "w": model.w, "h": model.h, "layers": layers}))?;
    if let Some(list) = src.pointer("/fonts/list").and_then(Value::as_array) {
        let fp = format!("{}/fonts/list", cx.anim_ptr());
        for f in list {
            let have = cx.get(&fp).and_then(Value::as_array).cloned().unwrap_or_default();
            if !have.iter().any(|x| x.get("fName") == f.get("fName")) {
                cx.insert(&fp, have.len(), f.clone())?;
            }
        }
    }
    let pos = center(cx, a);
    let mut ks = default_ks(pos);
    ks["a"] = static_prop(json!([model.w / 2.0, model.h / 2.0, 0.0]));
    let id = insert_layer(cx, "root", json!({"ty": 0, "nm": name, "refId": scene_id, "w": model.w, "h": model.h, "ks": ks}))?;
    Ok(json!({"id": id, "scene": scene_id}))
}

fn import_image(cx: &mut Cx, a: &Value) -> OpResult {
    let data = a.get("data").and_then(Value::as_str).ok_or_else(|| OpError("missing 'data' (data: URL)".into()))?;
    let bytes = crate::render::decode_data_uri(data).ok_or_else(|| OpError("data must be a base64 data: URL of an image".into()))?;
    let img = crate::render::decode_image(&bytes).ok_or_else(|| OpError("could not decode image (PNG or JPEG expected)".into()))?;
    let (w, h) = (a.get("width").and_then(Value::as_f64).unwrap_or(img.width() as f64), a.get("height").and_then(Value::as_f64).unwrap_or(img.height() as f64));
    let existing: Vec<String> = cx.anim().get("assets").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.get("id").and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
    let id = (1..).map(|n| format!("image_{n}")).find(|c| !existing.contains(c)).unwrap();
    let ap = format!("{}/assets", cx.anim_ptr());
    cx.insert(&ap, existing.len(), json!({"id": id, "w": w, "h": h, "u": "", "p": data, "e": 1}))?;
    let pos = center(cx, a);
    let mut ks = default_ks(pos);
    ks["a"] = static_prop(json!([w / 2.0, h / 2.0, 0.0]));
    let name = opt_str(a, "name").unwrap_or("Image");
    let lid = insert_layer(cx, "root", json!({"ty": 2, "nm": name, "refId": id, "ks": ks}))?;
    Ok(json!({"id": lid, "asset": id}))
}
