//! Motion presets: ready-made keyframe recipes applied to a layer's transform.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::util::{self, read_value, upsert_keyframe, EASING_PRESETS};
use super::{arg_str, err, opt_f64, Cx, LayerRef, OpError, OpResult};

pub const PRESETS: &[&str] = &[
    "fade_in", "fade_out", "scale_in", "scale_out", "pop", "slide_in_left", "slide_in_right", "slide_in_up", "slide_in_down", "slide_out_left", "slide_out_right",
    "slide_out_up", "slide_out_down", "spin", "bounce", "pulse", "shake", "wiggle", "float", "blink",
];

pub fn defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "apply_motion_preset",
            description: "Animate a layer with a preset (fade_in/out, scale_in/out, pop, slide_in/out_left/right/up/down, spin, bounce, pulse, shake, wiggle, float, blink) starting at start_frame for duration_frames. Values are relative to the layer's current transform; 'intensity' scales distances/angles.",
            schema: || {
                obj(
                    json!({"layer": {"type": "string"}, "preset": {"type": "string", "enum": PRESETS}, "start_frame": {"type": "number"}, "duration_frames": {"type": "number"}, "intensity": {"type": "number"}}),
                    &["layer", "preset"],
                )
            },
            mutates: true,
            handler: apply_motion_preset,
        },
        ToolDef {
            name: "stagger_layers",
            description: "Apply the same motion preset to several layers, each starting 'delay_frames' after the previous (in the order given).",
            schema: || {
                obj(
                    json!({"layers": {"type": "array", "items": {"type": "string"}}, "preset": {"type": "string", "enum": PRESETS}, "start_frame": {"type": "number"}, "delay_frames": {"type": "number"}, "duration_frames": {"type": "number"}, "intensity": {"type": "number"}}),
                    &["layers", "preset"],
                )
            },
            mutates: true,
            handler: stagger_layers,
        },
    ]
}

fn ease(name: &str) -> Option<Option<[f64; 4]>> {
    if name == "hold" {
        return Some(None);
    }
    EASING_PRESETS.iter().find(|p| p.0 == name).map(|p| Some(p.1))
}

/// (property, [(relative time 0..1, value, easing of the following segment)])
type Track = (&'static str, Vec<(f64, Vec<f64>, &'static str)>);

fn tracks(preset: &str, p: &[f64], s: &[f64], r: f64, o: f64, k: f64) -> OpResult<Vec<Track>> {
    let (px, py) = (p[0], p.get(1).copied().unwrap_or(0.0));
    let (sx, sy) = (s[0], s.get(1).copied().unwrap_or(s[0]));
    let sc = |f: f64| vec![sx * f, sy * f];
    let pos = |dx: f64, dy: f64| vec![px + dx * k, py + dy * k];
    let d = 160.0;
    let slide = |dx: f64, dy: f64, out: bool| -> Vec<Track> {
        if out {
            vec![("position", vec![(0.0, pos(0.0, 0.0), "ease_in"), (1.0, pos(dx, dy), "linear")]), ("opacity", vec![(0.0, vec![o], "ease_in"), (1.0, vec![0.0], "linear")])]
        } else {
            vec![("position", vec![(0.0, pos(dx, dy), "ease_out"), (1.0, pos(0.0, 0.0), "linear")]), ("opacity", vec![(0.0, vec![0.0], "ease_out"), (1.0, vec![o], "linear")])]
        }
    };
    Ok(match preset {
        "fade_in" => vec![("opacity", vec![(0.0, vec![0.0], "ease_in_out"), (1.0, vec![o], "linear")])],
        "fade_out" => vec![("opacity", vec![(0.0, vec![o], "ease_in_out"), (1.0, vec![0.0], "linear")])],
        "scale_in" => vec![("scale", vec![(0.0, sc(0.0), "ease_out_back"), (1.0, sc(1.0), "linear")])],
        "scale_out" => vec![("scale", vec![(0.0, sc(1.0), "ease_in_back"), (1.0, sc(0.0), "linear")])],
        "pop" => vec![
            ("scale", vec![(0.0, sc(0.0), "ease_out"), (0.6, sc(1.0 + 0.15 * k), "ease_in_out"), (1.0, sc(1.0), "linear")]),
            ("opacity", vec![(0.0, vec![0.0], "ease_out"), (0.3, vec![o], "linear")]),
        ],
        "slide_in_left" => slide(-d, 0.0, false),
        "slide_in_right" => slide(d, 0.0, false),
        "slide_in_up" => slide(0.0, d, false),
        "slide_in_down" => slide(0.0, -d, false),
        "slide_out_left" => slide(-d, 0.0, true),
        "slide_out_right" => slide(d, 0.0, true),
        "slide_out_up" => slide(0.0, -d, true),
        "slide_out_down" => slide(0.0, d, true),
        "spin" => vec![("rotation", vec![(0.0, vec![r], "ease_in_out"), (1.0, vec![r + 360.0 * k], "linear")])],
        "bounce" => vec![("position", vec![(0.0, pos(0.0, 0.0), "ease_out"), (0.45, pos(0.0, -40.0), "ease_in"), (0.9, pos(0.0, 0.0), "ease_out"), (1.0, pos(0.0, 0.0), "linear")])],
        "pulse" => vec![("scale", vec![(0.0, sc(1.0), "ease_in_out"), (0.5, sc(1.0 + 0.1 * k), "ease_in_out"), (1.0, sc(1.0), "linear")])],
        "shake" => vec![(
            "position",
            [0.0, 10.0, -10.0, 8.0, -8.0, 4.0, 0.0].iter().enumerate().map(|(i, x)| (i as f64 / 6.0, pos(*x, 0.0), "ease_in_out")).collect(),
        )],
        "wiggle" => vec![("rotation", [0.0, 8.0, -8.0, 5.0, -5.0, 0.0].iter().enumerate().map(|(i, x)| (i as f64 / 5.0, vec![r + x * k], "ease_in_out")).collect())],
        "float" => vec![("position", vec![(0.0, pos(0.0, 0.0), "smooth"), (0.5, pos(0.0, -12.0), "smooth"), (1.0, pos(0.0, 0.0), "linear")])],
        "blink" => vec![("opacity", vec![(0.0, vec![o], "hold"), (0.5, vec![0.0], "hold"), (1.0, vec![o], "linear")])],
        other => return err(format!("unknown preset '{other}' (valid: {})", PRESETS.join(", "))),
    })
}

fn apply(cx: &mut Cx, r: &LayerRef, preset: &str, start: f64, dur: f64, k: f64) -> OpResult<usize> {
    if dur <= 0.0 {
        return err("duration_frames must be > 0");
    }
    let lp = r.ptr();
    let p = read_value(cx, &format!("{lp}/ks/p"), start);
    let p = if p.is_empty() { vec![0.0, 0.0] } else { p };
    let s = read_value(cx, &format!("{lp}/ks/s"), start);
    let s = if s.is_empty() { vec![100.0, 100.0] } else { s };
    let rot = read_value(cx, &format!("{lp}/ks/r"), start).first().copied().unwrap_or(0.0);
    let o = read_value(cx, &format!("{lp}/ks/o"), start).first().copied().unwrap_or(100.0);
    let mut n = 0;
    for (prop, keys) in tracks(preset, &p, &s, rot, o, k)? {
        let ptr = util::prop_ptr(cx, r, None, prop)?;
        for (t, v, e) in keys {
            let frame = (start + t * dur).round();
            upsert_keyframe(cx, &ptr, frame, &json!(v), ease(e))?;
            n += 1;
        }
    }
    Ok(n)
}

fn apply_motion_preset(cx: &mut Cx, a: &Value) -> OpResult {
    let r = cx.layer(arg_str(a, "layer")?)?;
    let n = apply(cx, &r, arg_str(a, "preset")?, opt_f64(a, "start_frame").unwrap_or(0.0), opt_f64(a, "duration_frames").unwrap_or(20.0), opt_f64(a, "intensity").unwrap_or(1.0))?;
    Ok(json!({"keyframes": n}))
}

fn stagger_layers(cx: &mut Cx, a: &Value) -> OpResult {
    let names: Vec<String> = a.get("layers").and_then(Value::as_array).ok_or_else(|| OpError("missing 'layers'".into()))?.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
    let preset = arg_str(a, "preset")?.to_string();
    let (start, delay, dur, k) = (opt_f64(a, "start_frame").unwrap_or(0.0), opt_f64(a, "delay_frames").unwrap_or(4.0), opt_f64(a, "duration_frames").unwrap_or(20.0), opt_f64(a, "intensity").unwrap_or(1.0));
    let mut total = 0;
    for (i, name) in names.iter().enumerate() {
        let r = cx.layer(name)?;
        total += apply(cx, &r, &preset, start + delay * i as f64, dur, k)?;
    }
    Ok(json!({"layers": names.len(), "keyframes": total}))
}
