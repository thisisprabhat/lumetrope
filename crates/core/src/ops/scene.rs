//! Scene-level tools.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, default_ks, hex, parse_color, static_prop};
use super::{arg_str, err, opt_bool, opt_f64, opt_str, Cx, OpResult};

pub fn defs() -> Vec<ToolDef> {
    vec![
    ToolDef {
        name: "read_scene",
        description: "Read the active animation: size, fps, duration, segments, and every layer of a scene (default root) with its id, name, type, parent, in/out, visibility, lock and shape tree paths. Start here to get layer ids.",
        schema: || obj(json!({"scene": {"type": "string", "description": "Scene id (default 'root')"}}), &[]),
        mutates: false,
        handler: read_scene,
    },
    ToolDef {
        name: "inspect_layer",
        description: "Full raw Lottie JSON of one layer plus its bounds at 'frame' and the list of animated property paths.",
        schema: || obj(json!({"layer": {"type": "string"}, "frame": {"type": "number"}}), &["layer"]),
        mutates: false,
        handler: inspect_layer,
    },
    ToolDef {
        name: "list_scenes",
        description: "List the root scene and every nested scene (precomp) with ids and layer counts.",
        schema: || obj(json!({}), &[]),
        mutates: false,
        handler: list_scenes,
    },
    ToolDef {
        name: "create_scene",
        description: "Create an empty nested scene (precomp). With add_to_root, also place it as a layer on top of the root scene.",
        schema: || obj(json!({"name": {"type": "string"}, "width": {"type": "number"}, "height": {"type": "number"}, "add_to_root": {"type": "boolean"}}), &[]),
        mutates: true,
        handler: create_scene,
    },
    ToolDef {
        name: "set_background",
        description: "Set the preview/export background colour of the active animation (null for transparent). Stored in the dotLottie manifest on export.",
        schema: || obj(json!({"color": {"type": ["string", "array", "null"]}}), &["color"]),
        mutates: true,
        handler: set_background,
    },
    ToolDef {
        name: "list_colors",
        description: "List every unique colour used by fills, strokes, solids and text, with usage counts — useful for recolouring.",
        schema: || obj(json!({}), &[]),
        mutates: false,
        handler: list_colors,
    },
    ToolDef {
        name: "replace_color",
        description: "Replace a colour everywhere (fills, strokes, keyframes, solids, text). 'tolerance' 0..1 also matches near colours.",
        schema: || obj(json!({"from": {"type": ["string", "array"]}, "to": {"type": ["string", "array"]}, "tolerance": {"type": "number"}}), &["from", "to"]),
        mutates: true,
        handler: replace_color,
    },
    ToolDef {
        name: "set_scene_settings",
        description: "Change the active animation's canvas size, frame rate, duration (in frames) or name. Omitted fields are left unchanged.",
        schema: || {
            obj(
                json!({
                    "width": {"type": "number", "exclusiveMinimum": 0},
                    "height": {"type": "number", "exclusiveMinimum": 0},
                    "fps": {"type": "number", "exclusiveMinimum": 0},
                    "duration_frames": {"type": "number", "exclusiveMinimum": 0, "description": "Out point (op) in frames"},
                    "name": {"type": "string"}
                }),
                &[],
            )
        },
        mutates: true,
        handler: set_scene_settings,
    }]
}

const LAYER_TYPES: [&str; 6] = ["precomp", "solid", "image", "null", "shape", "text"];

fn shape_kind(v: &Value) -> &'static str {
    match v.get("ty").and_then(Value::as_str).unwrap_or("") {
        "gr" => "group",
        "rc" => "rect",
        "el" => "ellipse",
        "sr" => if v.get("sy").and_then(Value::as_i64) == Some(2) { "polygon" } else { "star" },
        "sh" => "path",
        "fl" => "fill",
        "st" => "stroke",
        "gf" => "gradient_fill",
        "gs" => "gradient_stroke",
        "tr" => "transform",
        "tm" => "trim_path",
        "rd" => "round_corners",
        "rp" => "repeater",
        _ => "other",
    }
}

fn shape_tree(items: &[Value], prefix: &str, out: &mut Vec<Value>) {
    for (i, it) in items.iter().enumerate() {
        let path = if prefix.is_empty() { i.to_string() } else { format!("{prefix}/{i}") };
        let kind = shape_kind(it);
        if kind == "transform" {
            continue;
        }
        out.push(json!({"path": path, "type": kind, "name": it.get("nm").cloned().unwrap_or(Value::Null), "hidden": it.get("hd").cloned().unwrap_or(json!(false))}));
        if kind == "group" {
            if let Some(c) = it.get("it").and_then(Value::as_array) {
                shape_tree(c, &path, out);
            }
        }
    }
}

pub fn layer_summary(comp: &str, l: &Value, layers: &[Value]) -> Value {
    let ind = l.get("ind").and_then(Value::as_i64).unwrap_or(0);
    let ty = l.get("ty").and_then(Value::as_i64).unwrap_or(3);
    let parent = l.get("parent").and_then(Value::as_i64).filter(|p| layers.iter().any(|x| x.get("ind").and_then(Value::as_i64) == Some(*p)));
    let mut s = json!({
        "id": format!("{comp}:{ind}"), "name": l.get("nm").cloned().unwrap_or(json!("")),
        "type": LAYER_TYPES.get(ty as usize).copied().unwrap_or("unknown"),
        "parent": parent.map(|p| json!(format!("{comp}:{p}"))).unwrap_or(Value::Null),
        "in": l.get("ip"), "out": l.get("op"),
        "hidden": l.get("hd").and_then(Value::as_bool).unwrap_or(false),
        "locked": l.pointer("/lmeta/lock").and_then(Value::as_bool).unwrap_or(false),
    });
    if let Some(tt) = l.get("tt").and_then(Value::as_i64) {
        s["matte_mode"] = json!(["none", "alpha", "alpha_inverted", "luma", "luma_inverted"].get(tt as usize).copied().unwrap_or("alpha"));
    }
    if l.get("td").and_then(Value::as_i64).unwrap_or(0) != 0 {
        s["is_matte"] = json!(true);
    }
    if ty == 0 {
        s["scene"] = l.get("refId").cloned().unwrap_or(Value::Null);
    }
    if ty == 4 {
        let mut shapes = vec![];
        shape_tree(l.get("shapes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]), "", &mut shapes);
        s["shapes"] = Value::Array(shapes);
    }
    if ty == 5 {
        s["text"] = l.pointer("/t/d/k/0/s/t").cloned().unwrap_or(Value::Null);
    }
    if l.get("ef").and_then(Value::as_array).map(|e| !e.is_empty()).unwrap_or(false) {
        s["effects"] = json!(l["ef"].as_array().unwrap().len());
    }
    if l.get("masksProperties").and_then(Value::as_array).map(|e| !e.is_empty()).unwrap_or(false) {
        s["masks"] = json!(l["masksProperties"].as_array().unwrap().len());
    }
    s
}

fn read_scene(cx: &mut Cx, a: &Value) -> OpResult {
    let comp = opt_str(a, "scene").unwrap_or("root").to_string();
    let lp = cx.comp_layers_ptr(&comp)?;
    let layers = cx.get(&lp).and_then(Value::as_array).cloned().unwrap_or_default();
    let (w, h, fr, ip, op) = util::scene(cx);
    let an = cx.anim();
    let markers: Vec<Value> = an.get("markers").and_then(Value::as_array).map(|m| {
        m.iter().map(|x| json!({"name": x.get("cm"), "start": x.get("tm"), "end": x.get("tm").and_then(Value::as_f64).unwrap_or(0.0) + x.get("dr").and_then(Value::as_f64).unwrap_or(0.0)})).collect()
    }).unwrap_or_default();
    Ok(json!({
        "name": an.get("nm"), "width": w, "height": h, "fps": fr, "ip": ip, "op": op, "duration_frames": op - ip,
        "scene": comp, "segments": markers,
        "layers": layers.iter().map(|l| layer_summary(&comp, l, &layers)).collect::<Vec<_>>(),
        "scenes": list_scenes(cx, &json!({}))?,
    }))
}

fn inspect_layer(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let frame = opt_f64(a, "frame").unwrap_or(0.0);
    let l = cx.layer_value(&r);
    let mut animated = vec![];
    util::animated_props(&l, "", &mut animated);
    let g = super::layers::geometry(cx, &r.id, frame).ok();
    Ok(json!({"id": r.id, "json": l, "animated": animated.iter().map(|p| p.trim_start_matches('/')).collect::<Vec<_>>(),
        "bounds": g.as_ref().map(|g| json!(g.bounds)).unwrap_or(Value::Null),
        "local_bounds": g.as_ref().map(|g| json!(g.local)).unwrap_or(Value::Null)}))
}

fn list_scenes(cx: &mut Cx, _a: &Value) -> OpResult {
    let (w, h, ..) = util::scene(cx);
    let mut out = vec![];
    for (id, lp) in cx.comps() {
        let n = cx.get(&lp).and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        let name = if id == "root" {
            cx.anim().get("nm").cloned().unwrap_or(json!("Main Scene"))
        } else {
            cx.anim()["assets"].as_array().and_then(|a| a.iter().find(|x| x.get("id").and_then(Value::as_str) == Some(id.as_str()))).and_then(|x| x.get("nm").cloned()).unwrap_or(json!(id.clone()))
        };
        let mut s = json!({"id": id, "name": name, "layers": n});
        if s["id"] == "root" {
            s["width"] = json!(w);
            s["height"] = json!(h);
        }
        out.push(s);
    }
    Ok(Value::Array(out))
}

fn create_scene(cx: &mut Cx, a: &Value) -> OpResult {
    let (w, h, fr, ip, op) = util::scene(cx);
    let (sw, sh) = (opt_f64(a, "width").unwrap_or(w), opt_f64(a, "height").unwrap_or(h));
    let existing: Vec<String> = cx.anim().get("assets").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.get("id").and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
    let id = (1..).map(|n| format!("comp_{n}")).find(|c| !existing.contains(c)).unwrap();
    let name = opt_str(a, "name").unwrap_or("Nested Scene").to_string();
    let ap = format!("{}/assets", cx.anim_ptr());
    cx.insert(&ap, existing.len(), json!({"id": id, "nm": name, "fr": fr, "w": sw, "h": sh, "layers": []}))?;
    let mut out = json!({"scene": id});
    if opt_bool(a, "add_to_root").unwrap_or(false) {
        let lp = cx.comp_layers_ptr("root")?;
        let ind = cx.next_ind(&lp);
        let mut ks = default_ks([w / 2.0, h / 2.0]);
        ks["a"] = static_prop(json!([sw / 2.0, sh / 2.0, 0.0]));
        cx.insert(&lp, 0, json!({"ddd": 0, "ind": ind, "ty": 0, "nm": name, "refId": id, "sr": 1, "ks": ks, "ao": 0, "w": sw, "h": sh, "ip": ip, "op": op, "st": 0, "bm": 0}))?;
        out["id"] = json!(format!("root:{ind}"));
    }
    Ok(out)
}

fn set_background(cx: &mut Cx, a: &Value) -> OpResult {
    let id = crate::project::active_id(cx.root);
    let ptr = format!("/settings/background/{}", super::escape(&id));
    match a.get("color") {
        None | Some(Value::Null) => cx.delete(&ptr)?,
        Some(c) => {
            let c = parse_color(c)?;
            cx.set(&ptr, json!(hex(&c)))?;
        }
    }
    Ok(json!({"ok": true}))
}

#[derive(Clone, Copy, PartialEq)]
enum Site {
    Prop,
    Hex,
    Rgb,
}

fn color_sites(v: &Value, base: &str, out: &mut Vec<(String, Site)>) {
    match v {
        Value::Object(o) => {
            match o.get("ty") {
                Some(Value::String(t)) if t == "fl" || t == "st" => out.push((format!("{base}/c"), Site::Prop)),
                Some(Value::Number(n)) if n.as_i64() == Some(1) && o.contains_key("sc") => out.push((format!("{base}/sc"), Site::Hex)),
                _ => {}
            }
            if o.get("t").map(Value::is_string).unwrap_or(false) && o.contains_key("s") && o.contains_key("f") {
                for k in ["fc", "sc"] {
                    if o.contains_key(k) {
                        out.push((format!("{base}/{k}"), Site::Rgb));
                    }
                }
            }
            for (k, x) in o {
                color_sites(x, &format!("{base}/{}", super::escape(k)), out);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                color_sites(x, &format!("{base}/{i}"), out);
            }
        }
        _ => {}
    }
}

fn hex_to_rgb(h: &str) -> Vec<f64> {
    parse_color(&json!(h)).map(|c| c[..3].to_vec()).unwrap_or_default()
}

/// Every colour value (as RGB 0..1) found at a site, with the JSON path of each.
fn site_colors(v: &Value, kind: Site) -> Vec<(String, Vec<f64>)> {
    let nums = |x: &Value| -> Vec<f64> { x.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default() };
    match kind {
        Site::Hex => v.as_str().map(|h| vec![(String::new(), hex_to_rgb(h))]).unwrap_or_default(),
        Site::Rgb => vec![(String::new(), nums(v))],
        Site::Prop => {
            if util::is_animated(v) {
                v["k"].as_array().unwrap().iter().enumerate().flat_map(|(i, k)| {
                    ["s", "e"].into_iter().filter_map(move |key| k.get(key).map(|x| (format!("/k/{i}/{key}"), nums(x))))
                }).collect()
            } else {
                vec![("/k".to_string(), v.get("k").map(nums).unwrap_or_default())]
            }
        }
    }
}

fn all_sites(cx: &Cx) -> Vec<(String, Site)> {
    let mut sites = vec![];
    color_sites(cx.anim(), &cx.anim_ptr(), &mut sites);
    sites
}

fn list_colors(cx: &mut Cx, _a: &Value) -> OpResult {
    let mut counts: Vec<(String, usize)> = vec![];
    for (ptr, kind) in all_sites(cx) {
        let Some(v) = cx.get(&ptr) else { continue };
        let mut seen = vec![];
        for (_, c) in site_colors(v, kind) {
            if c.len() >= 3 {
                let h = hex(&c);
                if !seen.contains(&h) {
                    seen.push(h);
                }
            }
        }
        for h in seen {
            match counts.iter_mut().find(|x| x.0 == h) {
                Some(x) => x.1 += 1,
                None => counts.push((h, 1)),
            }
        }
    }
    Ok(Value::Array(counts.into_iter().map(|(h, n)| json!({"hex": h, "count": n})).collect()))
}

fn replace_color(cx: &mut Cx, a: &Value) -> OpResult {
    let from = parse_color(a.get("from").unwrap_or(&Value::Null))?;
    let to = parse_color(a.get("to").unwrap_or(&Value::Null))?;
    let tol = opt_f64(a, "tolerance").unwrap_or(0.004).max(0.0);
    let close = |c: &[f64]| c.len() >= 3 && (0..3).all(|i| (c[i] - from[i]).abs() <= tol + 1e-9);
    let mut replaced = 0;
    for (ptr, kind) in all_sites(cx) {
        let Some(v) = cx.get(&ptr).cloned() else { continue };
        let mut hit = false;
        for (sub, c) in site_colors(&v, kind) {
            if !close(&c) {
                continue;
            }
            hit = true;
            let target = format!("{ptr}{sub}");
            let new = match kind {
                Site::Hex => json!(hex(&to)),
                _ => {
                    let mut n = vec![to[0], to[1], to[2]];
                    if c.len() > 3 {
                        n.push(c[3]);
                    }
                    json!(n)
                }
            };
            cx.set(&target, new)?;
        }
        if hit {
            replaced += 1;
        }
    }
    Ok(json!({"replaced": replaced}))
}

fn set_scene_settings(cx: &mut Cx, a: &Value) -> OpResult {
    let p = cx.anim_ptr();
    for (arg, key) in [("width", "w"), ("height", "h"), ("fps", "fr"), ("duration_frames", "op")] {
        if let Some(v) = opt_f64(a, arg) {
            if !(v > 0.0) {
                return err(format!("{arg} must be > 0"));
            }
            cx.set(&format!("{p}/{key}"), json!(v))?;
        }
    }
    if let Some(n) = opt_str(a, "name") {
        cx.set(&format!("{p}/nm"), json!(n))?;
    }
    Ok(json!({"ok": true}))
}
