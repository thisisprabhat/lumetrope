//! dotLottie (.lottie) archives: v1 and v2 reading, v2 writing, and theme
//! application (slot overrides) for previews.
//!
//! The project keeps images embedded as data URIs and fonts as base64 so it is
//! self-contained; export moves them into `i/` and `f/`.

use std::io::{Cursor, Read, Write};

use base64::Engine as _;
use serde_json::{json, Map, Value};

use crate::project;

const GENERATOR: &str = concat!("Lumetrope ", env!("CARGO_PKG_VERSION"));

fn mime_for(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.ends_with(".png") {
        "image/png"
    } else if n.ends_with(".jpg") || n.ends_with(".jpeg") {
        "image/jpeg"
    } else if n.ends_with(".webp") {
        "image/webp"
    } else if n.ends_with(".svg") {
        "image/svg+xml"
    } else if n.ends_with(".gif") {
        "image/gif"
    } else {
        "application/octet-stream"
    }
}

fn ext_for_data_uri(uri: &str) -> &'static str {
    match uri.split(';').next().unwrap_or("") {
        "data:image/jpeg" | "data:image/jpg" => "jpg",
        "data:image/webp" => "webp",
        "data:image/svg+xml" => "svg",
        "data:image/gif" => "gif",
        _ => "png",
    }
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn hex_to_u32(h: &str) -> Option<u32> {
    let h = h.trim_start_matches('#');
    if h.len() != 6 && h.len() != 8 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(if h.len() == 6 { (v << 8) | 0xff } else { v })
}

fn u32_to_hex(v: u64) -> String {
    format!("#{:06x}", (v >> 8) & 0xffffff)
}

/// Read a .lottie archive into a project value.
pub fn read(bytes: &[u8]) -> Result<Value, String> {
    read_limited(bytes, MAX_UNPACKED)
}

/// Cap on the total inflated size of an archive (guards against zip bombs).
pub const MAX_UNPACKED: usize = 512 * 1024 * 1024;

pub fn read_limited(bytes: &[u8], max_total: usize) -> Result<Value, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a .lottie (zip) file: {e}"))?;
    let mut files: Vec<(String, Vec<u8>)> = vec![];
    let mut total = 0usize;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| format!("corrupt .lottie: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let mut b = vec![];
        let room = max_total - total;
        (&mut f).take(room as u64 + 1).read_to_end(&mut b).map_err(|e| format!("corrupt .lottie: {e}"))?;
        if b.len() > room {
            return Err(format!(".lottie is too large (over {} MB unpacked)", max_total / (1024 * 1024)));
        }
        total += b.len();
        files.push((f.name().trim_start_matches("./").to_string(), b));
    }
    let get = |name: &str| files.iter().find(|(n, _)| n == name).map(|(_, b)| b.as_slice());
    let manifest: Value = match get("manifest.json") {
        Some(m) => serde_json::from_slice(m).map_err(|e| format!("invalid manifest.json: {e}"))?,
        None => json!({}),
    };
    let v2 = manifest.get("version").and_then(Value::as_str).map(|v| v.starts_with('2')).unwrap_or(false) || files.iter().any(|(n, _)| n.starts_with("a/"));
    let (anim_dir, img_dir) = if v2 { ("a/", "i/") } else { ("animations/", "images/") };

    // Animation ids: from the manifest, falling back to the files present.
    let mut ids: Vec<String> = manifest.get("animations").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.get("id").and_then(Value::as_str).map(str::to_string)).collect()).unwrap_or_default();
    if ids.is_empty() {
        ids = files.iter().filter_map(|(n, _)| n.strip_prefix(anim_dir).and_then(|x| x.strip_suffix(".json")).map(str::to_string)).collect();
    }
    if ids.is_empty() {
        return Err(".lottie contains no animations".into());
    }

    let mut animations = Map::new();
    for id in &ids {
        let raw = get(&format!("{anim_dir}{id}.json")).ok_or_else(|| format!("missing animation file for '{id}'"))?;
        let mut anim: Value = serde_json::from_slice(raw).map_err(|e| format!("animation '{id}' is not valid JSON: {e}"))?;
        // Embed referenced images so the project is self-contained.
        if let Some(assets) = anim.get_mut("assets").and_then(Value::as_array_mut) {
            for a in assets.iter_mut() {
                let (Some(p), embedded) = (a.get("p").and_then(Value::as_str).map(str::to_string), a.get("e").and_then(Value::as_i64) == Some(1)) else { continue };
                if embedded || p.starts_with("data:") || a.get("layers").is_some() {
                    continue;
                }
                let file = p.rsplit('/').next().unwrap_or(&p);
                let found = get(&format!("{img_dir}{file}")).or_else(|| files.iter().find(|(n, _)| n.ends_with(&format!("/{file}"))).map(|(_, b)| b.as_slice()));
                if let Some(b) = found {
                    a["p"] = json!(format!("data:{};base64,{}", mime_for(file), b64(b)));
                    a["u"] = json!("");
                    a["e"] = json!(1);
                }
            }
        }
        animations.insert(id.clone(), anim);
    }

    let mut machines = Map::new();
    let sm_meta: Vec<Value> = manifest.get("stateMachines").and_then(Value::as_array).cloned().unwrap_or_default();
    for (name, b) in files.iter().filter(|(n, _)| n.starts_with("s/") && n.ends_with(".json")) {
        let id = name.trim_start_matches("s/").trim_end_matches(".json").to_string();
        let mut sm: Value = serde_json::from_slice(b).map_err(|e| format!("state machine '{id}' is not valid JSON: {e}"))?;
        let display = sm_meta.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(id.as_str())).and_then(|m| m.get("name").cloned()).unwrap_or(json!(id.clone()));
        if let Some(o) = sm.as_object_mut() {
            o.insert("lmeta".into(), json!({"name": display, "layout": {}}));
        }
        machines.insert(id, sm);
    }

    let mut themes = Map::new();
    let theme_meta: Vec<Value> = manifest.get("themes").and_then(Value::as_array).cloned().unwrap_or_default();
    for (name, b) in files.iter().filter(|(n, _)| n.starts_with("t/") && n.ends_with(".json")) {
        let id = name.trim_start_matches("t/").trim_end_matches(".json").to_string();
        let t: Value = serde_json::from_slice(b).map_err(|e| format!("theme '{id}' is not valid JSON: {e}"))?;
        let display = theme_meta.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(id.as_str())).and_then(|m| m.get("name").cloned()).unwrap_or(json!(id.clone()));
        themes.insert(id, json!({"name": display, "rules": t.get("rules").cloned().unwrap_or(json!([]))}));
    }

    let mut fonts = Map::new();
    for (name, b) in files.iter().filter(|(n, _)| n.starts_with("f/")) {
        let file = name.trim_start_matches("f/");
        let stem = file.rsplit_once('.').map(|x| x.0).unwrap_or(file);
        fonts.insert(stem.to_string(), json!({"file": file, "data": b64(b)}));
    }

    let mut backgrounds = Map::new();
    if let Some(list) = manifest.get("animations").and_then(Value::as_array) {
        for a in list {
            if let (Some(id), Some(bg)) = (a.get("id").and_then(Value::as_str), a.get("background").and_then(Value::as_u64)) {
                backgrounds.insert(id.into(), json!(u32_to_hex(bg)));
            }
        }
    }

    let active = manifest.pointer("/initial/animation").and_then(Value::as_str).filter(|a| animations.contains_key(*a)).map(str::to_string).unwrap_or_else(|| ids[0].clone());
    let mut settings = json!({});
    if !backgrounds.is_empty() {
        settings["background"] = Value::Object(backgrounds);
    }
    if let Some(sm) = manifest.pointer("/initial/stateMachine").and_then(Value::as_str) {
        settings["initial_state_machine"] = json!(sm);
    }
    Ok(json!({
        "version": 1, "active": active, "animations": animations, "themes": themes,
        "stateMachines": machines, "fonts": fonts, "settings": settings
    }))
}

fn add(w: &mut zip::ZipWriter<Cursor<Vec<u8>>>, name: &str, data: &[u8]) -> Result<(), String> {
    let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    w.start_file(name, o).map_err(|e| e.to_string())?;
    w.write_all(data).map_err(|e| e.to_string())
}

fn safe_id(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || "._- ".contains(c) { c } else { '_' }).collect()
}

/// Write the project as a dotLottie v2 archive.
pub fn write(project: &Value) -> Result<Vec<u8>, String> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut manifest = json!({"version": "2", "generator": GENERATOR, "animations": []});
    let animations = project.get("animations").and_then(Value::as_object).ok_or("project has no animations")?;
    let mut image_n = 0;
    for (id, anim) in animations {
        let mut anim = anim.clone();
        project::strip_editor_meta(&mut anim);
        if let Some(assets) = anim.get_mut("assets").and_then(Value::as_array_mut) {
            for a in assets.iter_mut() {
                let Some(p) = a.get("p").and_then(Value::as_str).map(str::to_string) else { continue };
                if !p.starts_with("data:") || a.get("layers").is_some() {
                    continue;
                }
                let Some(bytes) = crate::render::decode_data_uri(&p) else { continue };
                image_n += 1;
                let aid = a.get("id").and_then(Value::as_str).map(safe_id).unwrap_or_else(|| format!("image_{image_n}"));
                let file = format!("{aid}.{}", ext_for_data_uri(&p));
                add(&mut w, &format!("i/{file}"), &bytes)?;
                a["u"] = json!("/i/");
                a["p"] = json!(file);
                a["e"] = json!(0);
            }
        }
        let fid = safe_id(id);
        add(&mut w, &format!("a/{fid}.json"), anim.to_string().as_bytes())?;
        let mut entry = json!({"id": fid});
        if let Some(bg) = project.pointer(&format!("/settings/background/{}", crate::ops::escape(id))).and_then(Value::as_str).and_then(hex_to_u32) {
            entry["background"] = json!(bg);
        }
        manifest["animations"].as_array_mut().unwrap().push(entry);
    }
    if let Some(themes) = project.get("themes").and_then(Value::as_object).filter(|t| !t.is_empty()) {
        let mut list = vec![];
        for (id, t) in themes {
            let fid = safe_id(id);
            add(&mut w, &format!("t/{fid}.json"), json!({"rules": t.get("rules").cloned().unwrap_or(json!([]))}).to_string().as_bytes())?;
            let mut e = json!({"id": fid});
            if let Some(n) = t.get("name") {
                e["name"] = n.clone();
            }
            list.push(e);
        }
        manifest["themes"] = Value::Array(list);
    }
    if let Some(sms) = project.get("stateMachines").and_then(Value::as_object).filter(|t| !t.is_empty()) {
        let mut list = vec![];
        for (id, sm) in sms {
            let fid = safe_id(id);
            let name = sm.pointer("/lmeta/name").cloned().unwrap_or(json!(id));
            let mut clean = sm.clone();
            project::strip_editor_meta(&mut clean);
            add(&mut w, &format!("s/{fid}.json"), clean.to_string().as_bytes())?;
            list.push(json!({"id": fid, "name": name}));
        }
        manifest["stateMachines"] = Value::Array(list);
    }
    if let Some(fonts) = project.get("fonts").and_then(Value::as_object) {
        for (name, f) in fonts {
            let Some(data) = f.get("data").and_then(Value::as_str).and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()) else { continue };
            let file = f.get("file").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("{}.ttf", safe_id(name)));
            add(&mut w, &format!("f/{file}"), &data)?;
        }
    }
    let active = project::active_id(project);
    let mut initial = json!({"animation": safe_id(&active)});
    if let Some(sm) = project.pointer("/settings/initial_state_machine").and_then(Value::as_str) {
        initial["stateMachine"] = json!(sm);
    }
    manifest["initial"] = initial;
    add(&mut w, "manifest.json", manifest.to_string().as_bytes())?;
    Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
}

// ------------------------------------------------------------------ themes

/// Lottie property JSON for a theme rule (or None if unsupported here).
fn rule_prop(rule: &Value, current: &Value) -> Option<Value> {
    let ty = rule.get("type").and_then(Value::as_str).unwrap_or("");
    let to_s = |v: &Value| -> Value {
        match (ty, v) {
            ("Gradient", Value::Array(stops)) if stops.first().map(Value::is_object).unwrap_or(false) => {
                let mut k = vec![];
                for s in stops {
                    k.push(s.get("offset").cloned().unwrap_or(json!(0)));
                    for c in s.get("color").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().take(3) {
                        k.push(c);
                    }
                }
                Value::Array(k)
            }
            (_, Value::Number(_)) => json!([v]),
            _ => v.clone(),
        }
    };
    if let Some(kfs) = rule.get("keyframes").and_then(Value::as_array) {
        let mut out = vec![];
        for (n, k) in kfs.iter().enumerate() {
            let mut kf = json!({"t": k.get("frame").cloned().unwrap_or(json!(0)), "s": to_s(k.get("value").unwrap_or(&Value::Null))});
            if k.get("hold").and_then(Value::as_bool).unwrap_or(false) {
                kf["h"] = json!(1);
            } else if n + 1 < kfs.len() {
                kf["o"] = k.get("outTangent").cloned().unwrap_or(json!({"x": [0.33], "y": [0.0]}));
                kf["i"] = k.get("inTangent").cloned().unwrap_or(json!({"x": [0.67], "y": [1.0]}));
            }
            out.push(kf);
        }
        return Some(json!({"a": 1, "k": out}));
    }
    let v = rule.get("value")?;
    if matches!(ty, "Image" | "Text") {
        return None;
    }
    let k = match (v, current.get("k")) {
        (Value::Number(_), Some(Value::Number(_))) => v.clone(),
        _ => match to_s(v) {
            Value::Array(a) if a.len() == 1 && current.get("k").map(Value::is_number).unwrap_or(false) => a[0].clone(),
            other => other,
        },
    };
    Some(json!({"a": 0, "k": k}))
}

/// Apply slot defaults and (optionally) a theme's rules to a Lottie animation in place.
pub fn apply_theme(anim: &mut Value, theme: Option<&Value>, anim_id: &str) {
    let slots = anim.get("slots").and_then(Value::as_object).cloned().unwrap_or_default();
    let rules: Vec<Value> = theme
        .and_then(|t| t.get("rules"))
        .and_then(Value::as_array)
        .map(|r| {
            r.iter()
                .filter(|rule| rule.get("animations").and_then(Value::as_array).map(|ids| ids.iter().any(|x| x.as_str() == Some(anim_id))).unwrap_or(true))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if slots.is_empty() && rules.is_empty() {
        return;
    }
    fn walk(v: &mut Value, slots: &Map<String, Value>, rules: &[Value]) {
        match v {
            Value::Object(o) => {
                if let Some(sid) = o.get("sid").and_then(Value::as_str).map(str::to_string) {
                    if o.contains_key("k") {
                        let current = Value::Object(o.clone());
                        let replacement = rules
                            .iter()
                            .find(|r| r.get("id").and_then(Value::as_str) == Some(sid.as_str()))
                            .and_then(|r| rule_prop(r, &current))
                            .or_else(|| slots.get(&sid).and_then(|s| s.get("p")).cloned());
                        if let Some(Value::Object(rep)) = replacement {
                            for key in ["a", "k"] {
                                if let Some(x) = rep.get(key) {
                                    o.insert(key.into(), x.clone());
                                }
                            }
                        }
                        return;
                    }
                }
                for (k, x) in o.iter_mut() {
                    if k != "slots" {
                        walk(x, slots, rules);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| walk(x, slots, rules)),
            _ => {}
        }
    }
    walk(anim, &slots, &rules);
}
