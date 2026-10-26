//! Colour tools: list the palette, replace colours, apply a whole colour
//! scheme at once, and the black-and-white "outlines only" look.

use serde_json::{json, Map, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, hex, parse_color, static_prop};
use super::{err, opt_f64, Cx, OpResult};

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_colors",
            description: "List every unique colour used by fills, strokes, gradient stops, solids, text and effect colours, with usage counts — useful for recolouring.",
            schema: || obj(json!({}), &[]),
            mutates: false,
            handler: list_colors,
        },
        ToolDef {
            name: "replace_color",
            description: "Replace a colour everywhere (fills, strokes, gradients, keyframes, solids, text, effects). 'tolerance' 0..1 also matches near colours.",
            schema: || obj(json!({"from": {"type": ["string", "array"]}, "to": {"type": ["string", "array"]}, "tolerance": {"type": "number"}}), &["from", "to"]),
            mutates: true,
            handler: replace_color,
        },
        ToolDef {
            name: "apply_color_map",
            description: "Recolour with a whole mapping at once, e.g. {\"#ff0000\": \"#223344\", \"#00ff00\": \"#ffcc00\"} (keys as reported by list_colors). Every swap happens simultaneously, so chains like A→B, B→C don't cascade. Alpha is kept. One undo step.",
            schema: || {
                obj(
                    json!({"map": {"type": "object", "additionalProperties": {"type": ["string", "array"]}, "description": "from colour (hex) → to colour"}}),
                    &["map"],
                )
            },
            mutates: true,
            handler: apply_color_map,
        },
        ToolDef {
            name: "outline_only",
            description: "Black-and-white line-art look: every fill and gradient becomes a single-colour stroke outline (existing strokes are recoloured), text and effect colours take the line colour, solids and the background take the background colour. Layer blend modes reset to normal and blur / drop-shadow effects are switched off. Matte source layers are left alone so masking keeps working.",
            schema: || {
                obj(
                    json!({
                        "line": {"type": ["string", "array"], "description": "Outline colour (default #000000)"},
                        "background": {"type": ["string", "array", "null"], "description": "Background colour (default #ffffff; null leaves the background unchanged)"},
                        "width": {"type": "number", "exclusiveMinimum": 0, "description": "Width of outlines replacing fills (default 2)"}
                    }),
                    &[],
                )
            },
            mutates: true,
            handler: outline_only,
        },
    ]
}

// ------------------------------------------------------------------ sites

#[derive(Clone, Copy, PartialEq)]
enum Site {
    /// Animatable colour property `{a, k}`.
    Prop,
    /// Hex string (solid layer colour).
    Hex,
    /// Plain `[r, g, b]` array (text document).
    Rgb,
    /// Gradient `{p, k}` with this many colour stops.
    Grad(usize),
}

fn color_sites(v: &Value, base: &str, out: &mut Vec<(String, Site)>) {
    match v {
        Value::Object(o) => {
            match o.get("ty") {
                Some(Value::String(t)) if t == "fl" || t == "st" => out.push((format!("{base}/c"), Site::Prop)),
                Some(Value::String(t)) if t == "gf" || t == "gs" => {
                    if let Some(n) = o.get("g").and_then(|g| g.get("p")).and_then(Value::as_u64) {
                        out.push((format!("{base}/g"), Site::Grad(n as usize)));
                    }
                }
                Some(Value::Number(n)) if n.as_i64() == Some(1) && o.contains_key("sc") => out.push((format!("{base}/sc"), Site::Hex)),
                // Effect colour value (effect value type 2); layers also have ty 2 but always carry `ks`.
                Some(Value::Number(n)) if n.as_i64() == Some(2) && o.contains_key("v") && !o.contains_key("ks") => out.push((format!("{base}/v"), Site::Prop)),
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

type Recolor<'a> = dyn FnMut([f64; 3]) -> Option<[f64; 3]> + 'a;

/// Offer the RGB triple at `a[off..off+3]` to `f`; write back its replacement.
fn map_triple(a: &mut Value, off: usize, f: &mut Recolor) -> bool {
    let Some(a) = a.as_array_mut() else { return false };
    if a.len() < off + 3 {
        return false;
    }
    let c = [0, 1, 2].map(|i| a[off + i].as_f64().unwrap_or(0.0));
    match f(c) {
        Some(n) => {
            for i in 0..3 {
                a[off + i] = json!(n[i]);
            }
            true
        }
        None => false,
    }
}

fn map_prop(p: &mut Value, offsets: &[usize], f: &mut Recolor) -> bool {
    let mut changed = false;
    if util::is_animated(p) {
        for k in p["k"].as_array_mut().unwrap() {
            for key in ["s", "e"] {
                if let Some(x) = k.get_mut(key) {
                    for &o in offsets {
                        changed |= map_triple(x, o, f);
                    }
                }
            }
        }
    } else if let Some(x) = p.get_mut("k") {
        for &o in offsets {
            changed |= map_triple(x, o, f);
        }
    }
    changed
}

/// Visit every colour stored at a site (static values, keyframes, gradient stops).
fn map_site(v: &mut Value, kind: Site, f: &mut Recolor) -> bool {
    match kind {
        Site::Prop => map_prop(v, &[0], f),
        Site::Grad(n) => {
            let offsets: Vec<usize> = (0..n).map(|i| i * 4 + 1).collect();
            v.get_mut("k").map(|p| map_prop(p, &offsets, f)).unwrap_or(false)
        }
        Site::Rgb => map_triple(v, 0, f),
        Site::Hex => {
            let Some(c) = v.as_str().and_then(|s| parse_color(&json!(s)).ok()) else { return false };
            match f([c[0], c[1], c[2]]) {
                Some(n) => {
                    *v = json!(hex(&n));
                    true
                }
                None => false,
            }
        }
    }
}

fn all_sites(cx: &Cx) -> Vec<(String, Site)> {
    let mut sites = vec![];
    let anim_ptr = cx.anim_ptr();
    color_sites(cx.anim(), &anim_ptr, &mut sites);
    // Colour properties bound to a slot render the slot's value, so recolour those slots too.
    let mut slots: Vec<String> = vec![];
    for (ptr, kind) in &sites {
        if *kind == Site::Prop {
            if let Some(sid) = cx.get(ptr).and_then(|p| p.get("sid")).and_then(Value::as_str) {
                if !slots.iter().any(|s| s == sid) && cx.anim().pointer(&format!("/slots/{}/p", super::escape(sid))).is_some() {
                    slots.push(sid.to_string());
                }
            }
        }
    }
    sites.extend(slots.iter().map(|sid| (format!("{anim_ptr}/slots/{}/p", super::escape(sid)), Site::Prop)));
    sites
}

/// Rewrite every site through `f`; returns how many sites changed.
fn recolor(cx: &mut Cx, f: &mut Recolor) -> OpResult<usize> {
    let mut n = 0;
    for (ptr, kind) in all_sites(cx) {
        let Some(mut v) = cx.get(&ptr).cloned() else { continue };
        if map_site(&mut v, kind, f) {
            cx.set(&ptr, v)?;
            n += 1;
        }
    }
    Ok(n)
}

// ------------------------------------------------------------------ tools

fn list_colors(cx: &mut Cx, _a: &Value) -> OpResult {
    let mut counts: Vec<(String, usize)> = vec![];
    for (ptr, kind) in all_sites(cx) {
        let Some(mut v) = cx.get(&ptr).cloned() else { continue };
        let mut seen: Vec<String> = vec![];
        map_site(&mut v, kind, &mut |c| {
            let h = hex(&c);
            if !seen.contains(&h) {
                seen.push(h);
            }
            None
        });
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
    let replaced = recolor(cx, &mut |c| (0..3).all(|i| (c[i] - from[i]).abs() <= tol + 1e-9).then_some([to[0], to[1], to[2]]))?;
    Ok(json!({"replaced": replaced}))
}

fn apply_color_map(cx: &mut Cx, a: &Value) -> OpResult {
    let mut map: Vec<(String, [f64; 3])> = vec![];
    match a.get("map") {
        Some(Value::Object(o)) => {
            for (k, v) in o {
                let from = parse_color(&json!(k))?;
                let to = parse_color(v)?;
                map.push((hex(&from), [to[0], to[1], to[2]]));
            }
        }
        _ => return err("'map' must be an object of {\"#from\": \"#to\"}"),
    }
    let replaced = recolor(cx, &mut |c| {
        let h = hex(&c);
        map.iter().find(|(k, _)| *k == h).map(|(_, to)| *to)
    })?;
    Ok(json!({"replaced": replaced}))
}

fn kind(it: &Value) -> &str {
    it.get("ty").and_then(Value::as_str).unwrap_or("")
}

struct Outline {
    color: Value,
    width: f64,
    fills: usize,
    strokes: usize,
}

impl Outline {
    fn stroke(&self, from: &Value) -> Value {
        let mut st = Map::new();
        st.insert("ty".into(), json!("st"));
        st.insert("nm".into(), from.get("nm").cloned().unwrap_or(json!("Outline")));
        st.insert("c".into(), self.color.clone());
        st.insert("o".into(), from.get("o").cloned().unwrap_or_else(|| static_prop(json!(100))));
        st.insert("w".into(), from.get("w").cloned().unwrap_or_else(|| static_prop(json!(self.width))));
        for (k, d) in [("lc", json!(2)), ("lj", json!(2)), ("ml", json!(4)), ("bm", json!(0))] {
            st.insert(k.into(), from.get(k).cloned().unwrap_or(d));
        }
        for k in ["d", "hd", "mn"] {
            if let Some(v) = from.get(k) {
                st.insert(k.into(), v.clone());
            }
        }
        Value::Object(st)
    }

    /// Fills become one stroke per shape list (placed at the last fill, so it
    /// styles every shape the fills did); strokes take the line colour.
    fn items(&mut self, items: &[Value]) -> Vec<Value> {
        let has_stroke = items.iter().any(|it| matches!(kind(it), "st" | "gs"));
        let last_fill = items.iter().rposition(|it| matches!(kind(it), "fl" | "gf"));
        let first_fill = items.iter().position(|it| matches!(kind(it), "fl" | "gf"));
        let mut out = vec![];
        for (i, it) in items.iter().enumerate() {
            match kind(it) {
                "gr" => {
                    let mut g = it.clone();
                    if let Some(inner) = it.get("it").and_then(Value::as_array) {
                        g["it"] = Value::Array(self.items(inner));
                    }
                    out.push(g);
                }
                "fl" | "gf" => {
                    self.fills += 1;
                    if !has_stroke && Some(i) == last_fill {
                        let src = first_fill.map(|f| &items[f]).unwrap_or(it);
                        let mut st = self.stroke(&json!({"nm": "Outline", "o": src.get("o").cloned().unwrap_or_else(|| static_prop(json!(100)))}));
                        if let Some(h) = it.get("hd") {
                            st["hd"] = h.clone();
                        }
                        out.push(st);
                    }
                }
                "st" => {
                    self.strokes += 1;
                    let mut st = it.clone();
                    st["c"] = self.color.clone();
                    out.push(st);
                }
                "gs" => {
                    self.strokes += 1;
                    out.push(self.stroke(it));
                }
                _ => out.push(it.clone()),
            }
        }
        out
    }
}

fn outline_only(cx: &mut Cx, a: &Value) -> OpResult {
    let line = parse_color(a.get("line").unwrap_or(&json!("#000000")))?;
    let bg = match a.get("background") {
        None => Some(parse_color(&json!("#ffffff"))?),
        Some(Value::Null) => None,
        Some(c) => Some(parse_color(c)?),
    };
    let width = opt_f64(a, "width").unwrap_or(2.0);
    if !(width > 0.0) {
        return err("width must be > 0");
    }
    let rgb = json!([line[0], line[1], line[2]]);
    let mut o = Outline { color: static_prop(json!([line[0], line[1], line[2], 1])), width, fills: 0, strokes: 0 };
    for (_, lp) in cx.comps() {
        let layers = cx.get(&lp).and_then(Value::as_array).cloned().unwrap_or_default();
        for (i, l) in layers.iter().enumerate() {
            // Matte sources keep their fills: their alpha decides what the matted layer shows.
            if l.get("td").and_then(Value::as_i64).unwrap_or(0) != 0 {
                continue;
            }
            let ptr = format!("{lp}/{i}");
            match l.get("ty").and_then(Value::as_i64) {
                Some(4) => {
                    if let Some(shapes) = l.get("shapes").and_then(Value::as_array) {
                        let next = o.items(shapes);
                        if &next != shapes {
                            cx.set(&format!("{ptr}/shapes"), Value::Array(next))?;
                        }
                    }
                }
                Some(1) => {
                    if let Some(bg) = bg {
                        cx.set(&format!("{ptr}/sc"), json!(hex(&bg)))?;
                    }
                }
                Some(5) => {
                    let docs = l.pointer("/t/d/k").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
                    for j in 0..docs {
                        let dp = format!("{ptr}/t/d/k/{j}/s");
                        cx.set(&format!("{dp}/fc"), rgb.clone())?;
                        if cx.get(&format!("{dp}/sc")).is_some() {
                            cx.set(&format!("{dp}/sc"), rgb.clone())?;
                        }
                    }
                }
                _ => {}
            }
            // Screen/add-style blending would wash black lines out to nothing.
            if l.get("bm").and_then(Value::as_i64).unwrap_or(0) != 0 {
                cx.set(&format!("{ptr}/bm"), json!(0))?;
            }
            if let Some(effects) = l.get("ef").and_then(Value::as_array) {
                for (e, fx) in effects.iter().enumerate() {
                    // Shadows and blurs turn crisp lines into grey haze.
                    if matches!(fx.get("ty").and_then(Value::as_i64), Some(25 | 29)) && fx.get("en").and_then(Value::as_i64) != Some(0) {
                        cx.set(&format!("{ptr}/ef/{e}/en"), json!(0))?;
                    }
                    for (j, v) in fx.get("ef").and_then(Value::as_array).into_iter().flatten().enumerate() {
                        if v.get("ty").and_then(Value::as_i64) == Some(2) {
                            cx.set(&format!("{ptr}/ef/{e}/ef/{j}/v"), o.color.clone())?;
                        }
                    }
                }
            }
        }
    }
    if let Some(bg) = bg {
        let id = crate::project::active_id(cx.root);
        cx.set(&format!("/settings/background/{}", super::escape(&id)), json!(hex(&bg)))?;
    }
    Ok(json!({"fills": o.fills, "strokes": o.strokes}))
}
