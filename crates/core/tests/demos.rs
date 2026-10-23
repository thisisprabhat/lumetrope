//! The demo files shipped in `samples/` load, render, and (for the
//! interactive ones) respond to their state machines. Regenerate them with
//! `cargo test -p lumetrope-core --test demos -- --ignored write_demo_samples`.

use lumetrope_core::engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples")
}

fn load(name: &str) -> Engine {
    let bytes = std::fs::read(samples_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut e = Engine::new();
    e.load_file(name, &bytes).unwrap_or_else(|err| panic!("{name}: {err}"));
    e
}

fn painted(e: &mut Engine, frame: f64) -> usize {
    e.render_rgba(frame, 128, 128).chunks(4).filter(|p| p[3] > 0).count()
}

const DEMOS: [&str; 4] = ["demo-bloom.lottie", "demo-day-night.lottie", "demo-buddy.lottie", "demo-groove.lottie"];

fn machine_id(e: &Engine) -> String {
    e.project()["stateMachines"].as_object().unwrap().keys().next().unwrap().clone()
}

fn state(v: &Value) -> String {
    v["state"].as_str().unwrap_or_default().to_string()
}

/// Advance the machine by `secs` in small steps, returning the final snapshot.
fn run_for(e: &mut Engine, secs: f64) -> Value {
    let mut snap = Value::Null;
    for _ in 0..(secs * 60.0) as usize {
        snap = e.sm_tick(1.0 / 60.0);
    }
    snap
}

#[test]
fn every_demo_renders() {
    for name in DEMOS {
        let mut e = load(name);
        assert!(painted(&mut e, 10.0) > 10_000, "{name} renders almost nothing");
    }
}

#[test]
fn shipped_files_match_the_generators() {
    for (name, doc) in [("demo-bloom.lottie", bloom()), ("demo-day-night.lottie", day_night()), ("demo-buddy.lottie", buddy()), ("demo-groove.lottie", groove())] {
        let mut shipped = load(name);
        let mut fresh = doc.0;
        for f in [0.0, 30.0, 70.0] {
            assert_eq!(shipped.render_rgba(f, 64, 64), fresh.render_rgba(f, 64, 64), "{name}@{f} differs from its generator; regenerate with write_demo_samples");
        }
    }
}

#[test]
fn bloom_bursts_and_retheme() {
    let mut e = load("demo-bloom.lottie");
    let id = machine_id(&e);
    assert_eq!(state(&e.sm_start(&id).unwrap()), "Idle");
    run_for(&mut e, 0.2);
    assert_eq!(state(&e.sm_pointer("click", 5.0, 5.0)), "Idle", "background is not clickable");
    assert_eq!(state(&e.sm_pointer("click", 240.0, 240.0)), "Bloom");
    assert_eq!(state(&run_for(&mut e, 2.0)), "Idle", "the burst settles back to idle");
    let snap = e.sm_pointer("click", 284.0, 432.0);
    assert_eq!(state(&snap), "Bloom", "a swatch also triggers the burst");
    assert!(snap.to_string().contains("mint"), "swatch switches the theme: {snap}");
}

#[test]
fn day_night_cycles_when_the_sun_and_moon_are_tapped() {
    let mut e = load("demo-day-night.lottie");
    let id = machine_id(&e);
    assert_eq!(state(&e.sm_start(&id).unwrap()), "Day");
    run_for(&mut e, 0.3);
    assert_eq!(state(&e.sm_pointer("click", 120.0, 118.0)), "Dusk");
    assert_eq!(state(&run_for(&mut e, 1.6)), "Night");
    assert_eq!(state(&e.sm_pointer("click", 365.0, 90.0)), "Dawn");
    assert_eq!(state(&run_for(&mut e, 1.6)), "Day");
}

#[test]
fn buddy_reacts_to_hover_and_petting() {
    let mut e = load("demo-buddy.lottie");
    let id = machine_id(&e);
    assert_eq!(state(&e.sm_start(&id).unwrap()), "Idle");
    run_for(&mut e, 0.2);
    e.sm_pointer("move", 5.0, 5.0);
    assert_eq!(state(&e.sm_pointer("move", 200.0, 230.0)), "Perk", "hover perks Buddy up");
    assert_eq!(state(&run_for(&mut e, 0.6)), "Curious");
    assert_eq!(state(&e.sm_pointer("move", 5.0, 5.0)), "Relax", "leaving relaxes");
    assert_eq!(state(&run_for(&mut e, 0.6)), "Idle");
    e.sm_pointer("move", 200.0, 230.0);
    assert_eq!(state(&e.sm_pointer("click", 200.0, 230.0)), "Happy", "petting works from any state");
    run_for(&mut e, 2.0);
    assert_ne!(state(&e.sm_snapshot()), "Happy", "the hop finishes");
}

#[test]
fn groove_plays_and_stops() {
    let mut e = load("demo-groove.lottie");
    let id = machine_id(&e);
    assert_eq!(state(&e.sm_start(&id).unwrap()), "Paused");
    assert_eq!(state(&e.sm_pointer("click", 370.0, 330.0)), "Starting");
    assert_eq!(state(&run_for(&mut e, 1.0)), "Playing");
    run_for(&mut e, 1.5);
    assert_eq!(state(&e.sm_snapshot()), "Playing", "keeps looping");
    assert_eq!(state(&e.sm_pointer("click", 370.0, 330.0)), "Stopping");
    assert_eq!(state(&run_for(&mut e, 1.5)), "Paused");
}

/// Thin wrapper over the engine's op API, the same tools an AI agent drives through the MCP server.
struct Doc(Engine);

impl Doc {
    fn new(name: &str, w: f64, h: f64, fps: f64, frames: f64) -> Doc {
        let mut e = Engine::new();
        e.new_project(w, h, fps, frames);
        let mut d = Doc(e);
        d.op("set_scene_settings", json!({"name": name}));
        d
    }

    fn op(&mut self, tool: &str, args: Value) -> Value {
        let r = self.0.apply(tool, &args);
        assert!(r.get("error").is_none(), "{tool} {args}: {r}");
        r["ok"].clone()
    }

    /// Keyframe a layer (or, with `shape`, a shape item) property.
    fn key(&mut self, layer: &str, shape: Option<&str>, prop: &str, frame: f64, value: Value, easing: &str) {
        let mut a = json!({"layer": layer, "property": prop, "frame": frame, "value": value, "easing": easing});
        if let Some(s) = shape {
            a["shape"] = json!(s);
        }
        self.op("set_keyframe", a);
    }

    fn track(&mut self, layer: &str, prop: &str, easing: &str, pts: &[(f64, Value)]) {
        for (f, v) in pts {
            self.key(layer, None, prop, *f, v.clone(), easing);
        }
    }

    fn shape_track(&mut self, layer: &str, shape: &str, prop: &str, easing: &str, pts: &[(f64, Value)]) {
        for (f, v) in pts {
            self.key(layer, Some(shape), prop, *f, v.clone(), easing);
        }
    }

    /// Cross-fade a fill colour over time.
    fn recolor(&mut self, layer: &str, shape: &str, pts: &[(f64, &str)]) {
        for (f, c) in pts {
            self.key(layer, Some(shape), "color", *f, json!(c), "ease_in_out");
        }
    }

    fn shape(&mut self, args: Value) -> String {
        self.op("create_shape", args)["id"].as_str().unwrap().to_string()
    }

    fn segment(&mut self, name: &str, start: f64, end: f64) {
        self.op("add_segment", json!({"name": name, "start": start, "end": end}));
    }

    fn glow(&mut self, layer: &str, blur: f64) {
        self.op("add_effect", json!({"layer": layer, "type": "gaussian_blur", "params": {"blurriness": blur}}));
    }

    fn solid(&mut self, name: &str, color: &str) {
        self.op("create_solid", json!({"color": color, "name": name}));
    }

    fn text(&mut self, name: &str, text: &str, pos: [f64; 2], size: f64, color: &str, tracking: f64) {
        self.op("create_text", json!({"text": text, "name": name, "position": pos, "font_size": size, "color": color, "tracking": tracking, "justify": "center"}));
    }

    fn machine(&mut self, name: &str) -> Value {
        self.op("add_state_machine", json!({"name": name}))["machine"].clone()
    }

    fn state(&mut self, m: &Value, name: &str, segment: &str, looping: bool) {
        self.op("add_state", json!({"machine": m, "name": name, "segment": segment, "loop": looping}));
    }

    fn go(&mut self, m: &Value, from: &str, to: &str, guards: Value) {
        let mut a = json!({"machine": m, "from": from, "to": to});
        if !guards.is_null() {
            a["guards"] = guards;
        }
        self.op("add_transition", a);
    }

    fn click(&mut self, m: &Value, layer: &str, actions: Value) {
        self.op("add_interaction", json!({"machine": m, "type": "Click", "layer": layer, "actions": actions}));
    }
}

fn pt(f: f64, v: Value) -> (f64, Value) {
    (f, v)
}

fn scale(s: f64) -> Value {
    json!([s, s])
}

fn sy(x: f64, y: f64) -> Value {
    json!([x, y])
}

// -------------------------------------------------------------------------
// 1. Bloom: a generative mandala. Click a swatch to re-theme it, click the
//    flower to make it burst open.
// -------------------------------------------------------------------------
fn bloom() -> Doc {
    let mut d = Doc::new("Bloom", 480.0, 480.0, 30.0, 105.0);
    d.segment("idle", 0.0, 60.0);
    d.segment("burst", 60.0, 105.0);
    d.solid("Background", "#070815");

    // Soft halo behind everything.
    d.shape(json!({"type": "ellipse", "name": "Halo", "position": [240, 240], "size": [330, 330], "fill": "#1b50ff"}));
    d.glow("Halo", 70.0);

    // (name, petal size, offset from centre, copies, degrees per copy, colour, opacity)
    let rings = [
        ("Outer Petals", [44.0, 128.0], 118.0, 12.0, 30.0, "#2f7bff", 82.0, "outer"),
        ("Mid Petals", [38.0, 96.0], 76.0, 8.0, 45.0, "#35e0ff", 78.0, "mid"),
    ];
    for (name, size, off, copies, step, color, op, slot) in rings {
        d.shape(json!({"type": "ellipse", "name": name, "position": [240, 240], "size": size, "fill": color}));
        d.op("update_shape_item", json!({"layer": name, "shape": "0", "props": {"position": [0, -off]}}));
        d.op("add_repeater", json!({"layer": name, "copies": copies, "rotation": step}));
        d.op("set_blend_mode", json!({"layer": name, "mode": "screen"}));
        d.op("set_transform", json!({"layer": name, "opacity": op}));
        d.op("create_slot", json!({"layer": name, "shape": "1", "property": "color", "slot_id": slot}));
    }
    d.shape(json!({"type": "star", "name": "Inner Petals", "position": [240, 240], "radius": 50, "inner_radius": 22, "points": 6, "roundness": 40, "fill": "#b9f3ff"}));
    d.op("set_blend_mode", json!({"layer": "Inner Petals", "mode": "screen"}));
    d.op("create_slot", json!({"layer": "Inner Petals", "shape": "1", "property": "color", "slot_id": "inner"}));

    d.shape(json!({"type": "ellipse", "name": "Core Glow", "position": [240, 240], "size": [74, 74], "fill": "#ffffff"}));
    d.glow("Core Glow", 22.0);
    d.shape(json!({"type": "ellipse", "name": "Core", "position": [240, 240], "size": [30, 30], "fill": "#ffffff"}));
    d.op("create_slot", json!({"layer": "Core", "shape": "1", "property": "color", "slot_id": "core"}));
    d.op("create_slot", json!({"layer": "Halo", "shape": "1", "property": "color", "slot_id": "halo"}));

    // A ring of 24 orbiting dots whose opacity fades around the circle.
    d.shape(json!({"type": "ellipse", "name": "Orbit Dots", "position": [240, 240], "size": [9, 9], "fill": "#7fb4ff"}));
    d.op("update_shape_item", json!({"layer": "Orbit Dots", "shape": "0", "props": {"position": [0, -176]}}));
    d.op("add_repeater", json!({"layer": "Orbit Dots", "copies": 24, "rotation": 15, "start_opacity": 100, "end_opacity": 15}));
    d.op("create_slot", json!({"layer": "Orbit Dots", "shape": "1", "property": "color", "slot_id": "dots"}));

    // Palette swatches.
    let swatches = [("Swatch Ocean", 196.0, "#35e0ff", 60.0), ("Swatch Sunset", 240.0, "#ff7a59", 30.0), ("Swatch Mint", 284.0, "#4af0b4", 20.0)];
    for (name, x, color, period) in swatches {
        d.shape(json!({"type": "ellipse", "name": name, "position": [x, 432], "size": [24, 24], "fill": color, "stroke": "#ffffff", "stroke_width": 2}));
        let mut t = 0.0;
        while t <= 60.0 {
            let up = ((t / (period / 2.0)) as i64) % 2 == 1;
            d.key(name, None, "scale", t, scale(if up { 114.0 } else { 100.0 }), "ease_in_out");
            t += period / 2.0;
        }
        d.key(name, None, "scale", 105.0, scale(100.0), "ease_in_out");
    }
    d.text("Hint", "TAP THE FLOWER  ·  PICK A PALETTE", [240.0, 50.0], 13.0, "#8e93b8", 3.0);

    // Idle breathing, then a burst when clicked.
    d.track("Outer Petals", "rotation", "linear", &[pt(0.0, json!(0)), pt(60.0, json!(30))]);
    d.track("Outer Petals", "rotation", "ease_out", &[pt(60.0, json!(30)), pt(105.0, json!(150))]);
    d.track("Mid Petals", "rotation", "linear", &[pt(0.0, json!(0)), pt(60.0, json!(-45))]);
    d.track("Mid Petals", "rotation", "ease_out", &[pt(60.0, json!(-45)), pt(105.0, json!(-225))]);
    d.track("Inner Petals", "rotation", "linear", &[pt(0.0, json!(0)), pt(60.0, json!(60))]);
    d.track("Inner Petals", "rotation", "ease_out", &[pt(60.0, json!(60)), pt(105.0, json!(180))]);
    d.track("Orbit Dots", "rotation", "linear", &[pt(0.0, json!(0)), pt(60.0, json!(-30))]);
    d.track("Orbit Dots", "rotation", "ease_out", &[pt(60.0, json!(-30)), pt(105.0, json!(-150))]);

    let pulses = [("Outer Petals", 0.0, 126.0), ("Mid Petals", 2.0, 140.0), ("Inner Petals", 4.0, 165.0), ("Orbit Dots", 6.0, 112.0)];
    for (name, lag, peak) in pulses {
        d.track(name, "scale", "ease_in_out", &[pt(0.0, scale(100.0)), pt(30.0, scale(104.0)), pt(60.0, scale(100.0))]);
        d.key(name, None, "scale", 60.0, scale(100.0), "ease_in");
        d.key(name, None, "scale", 67.0 + lag, scale(76.0), "ease_out_back");
        d.key(name, None, "scale", 77.0 + lag, scale(peak), "ease_in_out");
        d.key(name, None, "scale", 92.0 + lag.min(8.0), scale(94.0), "ease_in_out");
        d.key(name, None, "scale", 105.0, scale(100.0), "ease_in_out");
    }
    d.track("Core", "scale", "ease_in_out", &[pt(0.0, scale(100.0)), pt(30.0, scale(118.0)), pt(60.0, scale(100.0)), pt(68.0, scale(60.0)), pt(78.0, scale(150.0)), pt(105.0, scale(100.0))]);
    d.track("Core Glow", "scale", "ease_in_out", &[pt(0.0, scale(100.0)), pt(30.0, scale(125.0)), pt(60.0, scale(100.0)), pt(78.0, scale(210.0)), pt(105.0, scale(100.0))]);
    d.track("Core Glow", "opacity", "ease_in_out", &[pt(0.0, json!(70)), pt(30.0, json!(100)), pt(60.0, json!(70)), pt(78.0, json!(100)), pt(105.0, json!(70))]);
    d.track("Halo", "opacity", "ease_in_out", &[pt(0.0, json!(55)), pt(30.0, json!(80)), pt(60.0, json!(55)), pt(78.0, json!(100)), pt(105.0, json!(55))]);
    d.track("Halo", "scale", "ease_in_out", &[pt(0.0, scale(100.0)), pt(30.0, scale(106.0)), pt(60.0, scale(100.0)), pt(78.0, scale(150.0)), pt(105.0, scale(100.0))]);

    // Palettes. Ocean is the default; the others override the same slots.
    let themes = [
        ("ocean", ["#2f7bff", "#35e0ff", "#b9f3ff", "#ffffff", "#7fb4ff", "#1b50ff"]),
        ("sunset", ["#ff5e62", "#ff9966", "#ffd27a", "#fff6e0", "#ffb199", "#ff3d6a"]),
        ("mint", ["#1fd1a5", "#7bf0b0", "#d6ffd9", "#ffffff", "#6fe3c8", "#12a37f"]),
    ];
    for (id, cols) in themes {
        d.op("add_theme", json!({"id": id, "name": id}));
        for (slot, c) in ["outer", "mid", "inner", "core", "dots", "halo"].iter().zip(cols) {
            d.op("update_theme_rule", json!({"theme": id, "slot_id": slot, "value": c}));
        }
    }

    let m = d.machine("Bloom");
    d.op("add_input", json!({"machine": m, "name": "bloom", "type": "event"}));
    d.state(&m, "Idle", "idle", true);
    d.state(&m, "Bloom", "burst", false);
    d.go(&m, "Idle", "Bloom", json!([{"input": "bloom"}]));
    d.go(&m, "Bloom", "Idle", Value::Null);
    for layer in ["Core", "Inner Petals", "Mid Petals", "Outer Petals"] {
        d.click(&m, layer, json!([{"type": "fire", "input": "bloom"}]));
    }
    for (layer, theme) in [("Swatch Ocean", "ocean"), ("Swatch Sunset", "sunset"), ("Swatch Mint", "mint")] {
        d.click(&m, layer, json!([{"type": "set_theme", "value": theme}, {"type": "fire", "input": "bloom"}]));
    }
    d
}


// -------------------------------------------------------------------------
// 2. Day & Night: tap the sun (or moon) and the whole landscape changes
//    time of day: sky, stars, cottage lights, shooting star.
// -------------------------------------------------------------------------
fn day_night() -> Doc {
    let mut d = Doc::new("Day & Night", 480.0, 360.0, 30.0, 200.0);
    d.segment("day", 0.0, 60.0);
    d.segment("dusk", 60.0, 100.0);
    d.segment("night", 100.0, 160.0);
    d.segment("dawn", 160.0, 200.0);
    let ease = "ease_in_out";

    // Three skies cross-fade: day, dusk, night.
    let sky = |d: &mut Doc, name: &str, a: &str, b: &str| {
        d.shape(json!({"type": "rect", "name": name, "position": [240, 180], "size": [480, 360], "fill": a}));
        d.op("set_gradient", json!({"layer": name, "kind": "fill", "type": "linear", "start": [0, -180], "end": [0, 180], "stops": [{"offset": 0, "color": a}, {"offset": 1, "color": b}]}));
    };
    sky(&mut d, "Sky Day", "#3b8dff", "#bfe6ff");
    sky(&mut d, "Sky Dusk", "#3a2a7a", "#ff8a5c");
    sky(&mut d, "Sky Night", "#050a2e", "#2b2468");
    d.track("Sky Dusk", "opacity", ease, &[pt(0.0, json!(0)), pt(60.0, json!(0)), pt(78.0, json!(100)), pt(100.0, json!(0)), pt(160.0, json!(0)), pt(182.0, json!(100)), pt(200.0, json!(0))]);
    d.track("Sky Night", "opacity", ease, &[pt(0.0, json!(0)), pt(75.0, json!(0)), pt(100.0, json!(100)), pt(160.0, json!(100)), pt(185.0, json!(0)), pt(200.0, json!(0))]);

    // Stars twinkle at different rates, and only exist at night.
    let star_sets: [(&str, [[f64; 3]; 5], f64); 3] = [
        ("Stars A", [[40.0, 40.0, 3.0], [130.0, 90.0, 2.5], [215.0, 30.0, 3.5], [300.0, 70.0, 2.5], [440.0, 100.0, 3.0]], 30.0),
        ("Stars B", [[70.0, 120.0, 2.5], [170.0, 55.0, 3.0], [260.0, 110.0, 2.0], [400.0, 40.0, 3.5], [455.0, 150.0, 2.5]], 20.0),
        ("Stars C", [[20.0, 95.0, 2.0], [100.0, 25.0, 3.0], [330.0, 125.0, 2.5], [370.0, 85.0, 2.0], [285.0, 28.0, 3.0]], 60.0),
    ];
    for (name, pts, period) in star_sets {
        d.shape(json!({"type": "star", "name": name, "position": [pts[0][0], pts[0][1]], "radius": pts[0][2] * 2.2, "inner_radius": pts[0][2] * 0.6, "points": 4, "fill": "#fffbe6"}));
        for p in &pts[1..] {
            d.op("add_shape", json!({"layer": name, "type": "star", "position": [p[0] - pts[0][0], p[1] - pts[0][1]], "radius": p[2] * 2.2, "inner_radius": p[2] * 0.6, "points": 4, "fill": "#fffbe6"}));
        }
        let mut pts_k = vec![pt(0.0, json!(0)), pt(70.0, json!(0)), pt(100.0, json!(100))];
        let mut t = 100.0;
        let mut hi = true;
        while t < 160.0 {
            t += period / 2.0;
            hi = !hi;
            pts_k.push(pt(t.min(160.0), json!(if hi { 100 } else { 30 })));
        }
        pts_k.extend([pt(185.0, json!(0)), pt(200.0, json!(0))]);
        d.track(name, "opacity", ease, &pts_k);
    }

    // Shooting star streaks across once per night loop.
    d.shape(json!({"type": "path", "name": "Shooting Star", "position": [0, 0], "closed": false, "points": [[330, 40], [250, 82]], "fill": null, "stroke": "#ffffff", "stroke_width": 2.5}));
    d.op("set_stroke", json!({"layer": "Shooting Star", "line_cap": "round"}));
    let tm = d.op("add_trim_path", json!({"layer": "Shooting Star", "start": 0, "end": 0}))["shape"].as_str().unwrap().to_string();
    d.shape_track("Shooting Star", &tm, "end", "ease_out", &[pt(0.0, json!(0)), pt(118.0, json!(0)), pt(126.0, json!(100)), pt(200.0, json!(100))]);
    d.shape_track("Shooting Star", &tm, "start", "ease_in", &[pt(0.0, json!(0)), pt(121.0, json!(0)), pt(132.0, json!(100)), pt(200.0, json!(100))]);
    d.track("Shooting Star", "opacity", ease, &[pt(0.0, json!(0)), pt(100.0, json!(0)), pt(117.0, json!(0)), pt(118.0, json!(100)), pt(132.0, json!(100)), pt(133.0, json!(0)), pt(200.0, json!(0))]);

    // Sun rig: glow, rays and disc ride a parent null.
    d.op("create_null", json!({"name": "Sun Rig", "position": [120, 120]}));
    d.shape(json!({"type": "ellipse", "name": "Sun Glow", "position": [0, 0], "size": [150, 150], "fill": "#ffb700"}));
    d.glow("Sun Glow", 40.0);
    d.op("set_transform", json!({"layer": "Sun Glow", "opacity": 70}));
    d.shape(json!({"type": "rect", "name": "Sun Rays", "position": [0, 0], "size": [8, 24], "roundness": 4, "fill": "#ffd23f"}));
    d.op("update_shape_item", json!({"layer": "Sun Rays", "shape": "0", "props": {"position": [0, -62]}}));
    d.op("add_repeater", json!({"layer": "Sun Rays", "copies": 12, "rotation": 30}));
    d.shape(json!({"type": "ellipse", "name": "Sun", "position": [0, 0], "size": [78, 78], "fill": "#ffd23f"}));
    d.op("set_gradient", json!({"layer": "Sun", "kind": "fill", "type": "radial", "stops": [{"offset": 0, "color": "#fff3a0"}, {"offset": 1, "color": "#ffb300"}]}));
    for l in ["Sun Glow", "Sun Rays", "Sun"] {
        d.op("set_parent", json!({"layer": l, "parent": "Sun Rig"}));
    }
    d.track("Sun Rays", "rotation", "linear", &[pt(0.0, json!(0)), pt(60.0, json!(30))]);
    d.track("Sun Rays", "rotation", ease, &[pt(60.0, json!(30)), pt(100.0, json!(80)), pt(160.0, json!(80)), pt(200.0, json!(30 + 360))]);
    d.track("Sun Rig", "position", ease, &[pt(0.0, json!([120, 120])), pt(30.0, json!([120, 111])), pt(60.0, json!([120, 120])), pt(100.0, json!([190, 395])), pt(160.0, json!([190, 395])), pt(200.0, json!([120, 120]))]);
    d.track("Sun Rig", "scale", ease, &[pt(0.0, scale(100.0)), pt(60.0, scale(100.0)), pt(100.0, scale(112.0)), pt(160.0, scale(112.0)), pt(200.0, scale(100.0))]);

    // Moon rig.
    d.op("create_null", json!({"name": "Moon Rig", "position": [370, 330]}));
    d.shape(json!({"type": "ellipse", "name": "Moon Glow", "position": [0, 0], "size": [120, 120], "fill": "#9fb4ff"}));
    d.glow("Moon Glow", 34.0);
    d.op("set_transform", json!({"layer": "Moon Glow", "opacity": 55}));
    d.shape(json!({"type": "ellipse", "name": "Moon", "position": [0, 0], "size": [66, 66], "fill": "#f4f1e3"}));
    for (x, y, r) in [(-12.0, -10.0, 16.0), (14.0, 8.0, 12.0), (-6.0, 16.0, 8.0)] {
        d.op("add_shape", json!({"layer": "Moon", "type": "ellipse", "position": [x, y], "size": [r, r], "fill": "#d8d4c0"}));
    }
    for l in ["Moon Glow", "Moon"] {
        d.op("set_parent", json!({"layer": l, "parent": "Moon Rig"}));
    }
    d.track("Moon Rig", "position", ease, &[pt(0.0, json!([370, 330])), pt(60.0, json!([370, 330])), pt(100.0, json!([365, 92])), pt(130.0, json!([365, 84])), pt(160.0, json!([365, 92])), pt(200.0, json!([370, 330]))]);

    // Clouds sway; they dim at night.
    let clouds = [("Cloud A", 100.0, 70.0, 1.0), ("Cloud B", 300.0, 50.0, 0.75), ("Cloud C", 410.0, 120.0, 0.55)];
    for (name, x, y, sc) in clouds {
        d.shape(json!({"type": "ellipse", "name": name, "position": [x, y], "size": [70, 40], "fill": "#ffffff"}));
        for (dx, dy, w, h) in [(-26.0, 8.0, 46.0, 28.0), (28.0, 8.0, 50.0, 30.0), (4.0, -14.0, 44.0, 34.0)] {
            d.op("add_shape", json!({"layer": name, "type": "ellipse", "position": [dx, dy], "size": [w, h], "fill": "#ffffff"}));
        }
        d.op("set_transform", json!({"layer": name, "scale": scale(sc * 100.0)}));
        let (a, b) = (x - 14.0 * sc, x + 14.0 * sc);
        d.track(name, "position", ease, &[pt(0.0, json!([a, y])), pt(30.0, json!([b, y])), pt(60.0, json!([a, y])), pt(100.0, json!([a, y])), pt(130.0, json!([b, y])), pt(160.0, json!([a, y])), pt(200.0, json!([a, y]))]);
        d.track(name, "opacity", ease, &[pt(0.0, json!(95)), pt(60.0, json!(95)), pt(100.0, json!(22)), pt(160.0, json!(22)), pt(200.0, json!(95))]);
    }

    // Landscape: layered hills whose colours cross-fade between day and night.
    let land = |d: &mut Doc, name: &str, pts: Vec<[f64; 2]>, day: &str, night: &str| {
        d.shape(json!({"type": "path", "name": name, "position": [0, 0], "closed": true, "points": pts, "fill": day}));
        d.recolor(name, "1", &[(0.0, day), (60.0, day), (100.0, night), (160.0, night), (200.0, day)]);
    };
    land(&mut d, "Far Mountains", vec![[0., 250.], [60., 190.], [110., 225.], [190., 150.], [260., 230.], [330., 175.], [400., 230.], [440., 205.], [480., 235.], [480., 360.], [0., 360.]], "#7ea6dc", "#1b2058");
    land(&mut d, "Near Hills", vec![[0., 285.], [90., 250.], [190., 280.], [300., 240.], [400., 275.], [480., 255.], [480., 360.], [0., 360.]], "#4aa36a", "#10264a");
    land(&mut d, "Foreground", vec![[0., 320.], [140., 296.], [280., 318.], [400., 300.], [480., 312.], [480., 360.], [0., 360.]], "#2f8a52", "#0b1a37");

    // Pine trees sway.
    for (i, (x, y, s)) in [(70.0, 300.0, 1.0), (105.0, 308.0, 0.75), (405.0, 300.0, 1.1), (440.0, 310.0, 0.8)].iter().enumerate() {
        let name = format!("Pine {}", i + 1);
        d.shape(json!({"type": "polygon", "name": name, "position": [*x, *y - 24.0 * *s], "radius": 26, "points": 3, "roundness": 18, "fill": "#1f7a45"}));
        d.op("add_shape", json!({"layer": name, "type": "polygon", "position": [0, 20], "radius": 22, "points": 3, "fill": "#1f7a45"}));
        d.op("set_anchor_point", json!({"layer": name, "anchor": "bottom"}));
        d.op("set_transform", json!({"layer": name, "scale": scale(s * 100.0)}));
        for shape in ["2", "0/1"] {
            d.recolor(&name, shape, &[(0.0, "#1f7a45"), (60.0, "#1f7a45"), (100.0, "#0c2c3d"), (160.0, "#0c2c3d"), (200.0, "#1f7a45")]);
        }
        let ph = i as f64 * 7.0;
        let sway = |v: f64| json!(v);
        d.track(&name, "rotation", ease, &[pt(0.0, sway(0.0)), pt(15.0 + ph.min(12.0), sway(2.4)), pt(45.0 - ph.min(12.0), sway(-2.4)), pt(60.0, sway(0.0)), pt(100.0, sway(0.0)), pt(115.0 + ph.min(12.0), sway(2.4)), pt(145.0 - ph.min(12.0), sway(-2.4)), pt(160.0, sway(0.0)), pt(200.0, sway(0.0))]);
    }

    // Cottage whose windows light up at dusk.
    d.shape(json!({"type": "rect", "name": "Cottage", "position": [250, 296], "size": [58, 38], "fill": "#e9d5b0"}));
    d.op("add_shape", json!({"layer": "Cottage", "type": "polygon", "position": [0, -33], "radius": 40, "points": 3, "fill": "#b4472f"}));
    d.recolor("Cottage", "2", &[(0.0, "#e9d5b0"), (60.0, "#e9d5b0"), (100.0, "#6a5f7a"), (160.0, "#6a5f7a"), (200.0, "#e9d5b0")]);
    d.recolor("Cottage", "0/1", &[(0.0, "#b4472f"), (60.0, "#b4472f"), (100.0, "#4a2a58"), (160.0, "#4a2a58"), (200.0, "#b4472f")]);
    d.shape(json!({"type": "rect", "name": "Window Glow", "position": [250, 300], "size": [30, 18], "roundness": 3, "fill": "#ffd166"}));
    d.glow("Window Glow", 8.0);
    d.shape(json!({"type": "rect", "name": "Window", "position": [250, 300], "size": [16, 14], "roundness": 2, "fill": "#ffd166"}));
    d.track("Window", "opacity", ease, &[pt(0.0, json!(15)), pt(60.0, json!(15)), pt(90.0, json!(100)), pt(100.0, json!(100)), pt(160.0, json!(100)), pt(185.0, json!(15)), pt(200.0, json!(15))]);
    d.track("Window Glow", "opacity", ease, &[pt(0.0, json!(0)), pt(60.0, json!(0)), pt(90.0, json!(90)), pt(115.0, json!(70)), pt(130.0, json!(95)), pt(145.0, json!(70)), pt(160.0, json!(90)), pt(185.0, json!(0)), pt(200.0, json!(0))]);
    d.text("Hint", "TAP THE SUN OR MOON", [240.0, 345.0], 11.0, "#ffffff", 3.0);
    d.op("set_transform", json!({"layer": "Hint", "opacity": 60}));

    let m = d.machine("Time of day");
    d.op("add_input", json!({"machine": m, "name": "night", "type": "boolean", "value": false}));
    d.state(&m, "Day", "day", true);
    d.state(&m, "Dusk", "dusk", false);
    d.state(&m, "Night", "night", true);
    d.state(&m, "Dawn", "dawn", false);
    let is = |v: bool| json!([{"input": "night", "condition": "Equal", "value": v}]);
    d.go(&m, "Day", "Dusk", is(true));
    d.go(&m, "Dusk", "Night", Value::Null);
    d.go(&m, "Night", "Dawn", is(false));
    d.go(&m, "Dawn", "Day", Value::Null);
    for l in ["Sun", "Moon"] {
        d.click(&m, l, json!([{"type": "toggle", "input": "night"}]));
    }
    d
}

// -------------------------------------------------------------------------
// 3. Buddy: a character with a real personality. Idle blinking, perks up on
//    hover, looks around, and hops with hearts when petted.
// -------------------------------------------------------------------------
fn buddy() -> Doc {
    let mut d = Doc::new("Buddy", 400.0, 400.0, 30.0, 170.0);
    d.segment("idle", 0.0, 60.0);
    d.segment("perk", 60.0, 72.0);
    d.segment("curious", 72.0, 102.0);
    d.segment("relax", 102.0, 114.0);
    d.segment("happy", 120.0, 170.0);
    let ease = "ease_in_out";
    let lin = |f: f64| f;
    let _ = lin;

    d.solid("Background", "#fff2dc");
    d.shape(json!({"type": "ellipse", "name": "Sun Disc", "position": [200, 215], "size": [340, 340], "fill": "#ffe0b0"}));

    // Ground shadow squashes when Buddy hops.
    d.shape(json!({"type": "ellipse", "name": "Shadow", "position": [200, 318], "size": [150, 22], "fill": "#d9a56d"}));
    d.op("set_transform", json!({"layer": "Shadow", "opacity": 70}));
    d.track("Shadow", "scale", ease, &[pt(0.0, scale(100.0)), pt(60.0, scale(100.0)), pt(66.0, scale(94.0)), pt(72.0, scale(96.0)), pt(102.0, scale(96.0)), pt(114.0, scale(100.0)), pt(120.0, scale(100.0)), pt(126.0, scale(112.0)), pt(133.0, scale(66.0)), pt(140.0, scale(112.0)), pt(146.0, scale(116.0)), pt(152.0, scale(84.0)), pt(158.0, scale(104.0)), pt(165.0, scale(100.0)), pt(170.0, scale(100.0))]);

    // Rig at Buddy's feet; everything below is positioned relative to it.
    d.op("create_null", json!({"name": "Buddy Rig", "position": [200, 310]}));
    let rel = |d: &mut Doc, name: &str| d.op("set_parent", json!({"layer": name, "parent": "Buddy Rig"}));

    // Ears behind the body.
    for (name, x, rot) in [("Ear L", -52.0, -14.0), ("Ear R", 52.0, 14.0)] {
        d.shape(json!({"type": "polygon", "name": name, "position": [x, -142], "radius": 34, "points": 3, "roundness": 30, "fill": "#ff9a3c"}));
        d.op("add_shape", json!({"layer": name, "type": "polygon", "position": [0, 4], "radius": 18, "points": 3, "fill": "#ffc9a0"}));
        d.op("set_anchor_point", json!({"layer": name, "anchor": "bottom"}));
        d.op("set_transform", json!({"layer": name, "rotation": rot}));
        rel(&mut d, name);
    }
    d.shape(json!({"type": "ellipse", "name": "Body", "position": [0, -78], "size": [176, 156], "fill": "#ff9a3c"}));
    d.op("set_gradient", json!({"layer": "Body", "kind": "fill", "type": "radial", "start": [-20, -30], "end": [90, 60], "stops": [{"offset": 0, "color": "#ffbb5c"}, {"offset": 1, "color": "#f2751f"}]}));
    rel(&mut d, "Body");
    d.shape(json!({"type": "ellipse", "name": "Belly", "position": [0, -52], "size": [104, 84], "fill": "#ffe2bd"}));
    rel(&mut d, "Belly");

    for (name, x) in [("Eye L", -34.0), ("Eye R", 34.0)] {
        d.shape(json!({"type": "ellipse", "name": name, "position": [x, -92], "size": [34, 38], "fill": "#ffffff"}));
        rel(&mut d, name);
    }
    for (name, x) in [("Pupil L", -34.0), ("Pupil R", 34.0)] {
        d.shape(json!({"type": "ellipse", "name": name, "position": [x, -90], "size": [17, 20], "fill": "#2a1b3d"}));
        d.op("add_shape", json!({"layer": name, "type": "ellipse", "position": [4, -5], "size": [6, 6], "fill": "#ffffff"}));
        rel(&mut d, name);
    }
    for (name, x) in [("Cheek L", -62.0), ("Cheek R", 62.0)] {
        d.shape(json!({"type": "ellipse", "name": name, "position": [x, -62], "size": [26, 16], "fill": "#ff6b81"}));
        d.op("set_transform", json!({"layer": name, "opacity": 35}));
        rel(&mut d, name);
    }
    d.shape(json!({"type": "ellipse", "name": "Mouth", "position": [0, -64], "size": [18, 6], "fill": "#7a2e2e"}));
    rel(&mut d, "Mouth");
    d.shape(json!({"type": "ellipse", "name": "Nose", "position": [0, -75], "size": [12, 8], "fill": "#2a1b3d"}));
    rel(&mut d, "Nose");

    // Poses. idle(0..60) -> perk(60..72) -> curious(72..102, loops) -> relax(102..114); happy(120..170).
    let s = |x: f64, y: f64| json!([x, y]);
    d.track("Buddy Rig", "scale", ease, &[
        pt(0.0, s(100.0, 100.0)), pt(30.0, s(102.0, 98.0)), pt(60.0, s(100.0, 100.0)),
        pt(66.0, s(96.0, 108.0)), pt(72.0, s(100.0, 104.0)), pt(87.0, s(101.0, 102.0)), pt(102.0, s(100.0, 104.0)),
        pt(108.0, s(100.0, 99.0)), pt(114.0, s(100.0, 100.0)), pt(120.0, s(100.0, 100.0)),
        pt(126.0, s(114.0, 84.0)), pt(133.0, s(90.0, 114.0)), pt(140.0, s(116.0, 84.0)), pt(146.0, s(92.0, 110.0)), pt(152.0, s(108.0, 92.0)), pt(158.0, s(98.0, 103.0)), pt(166.0, s(100.0, 100.0)), pt(170.0, s(100.0, 100.0)),
    ]);
    d.track("Buddy Rig", "position", ease, &[
        pt(0.0, json!([200, 310])), pt(120.0, json!([200, 310])), pt(126.0, json!([200, 310])), pt(133.0, json!([200, 232])), pt(140.0, json!([200, 310])),
        pt(146.0, json!([200, 266])), pt(152.0, json!([200, 310])), pt(170.0, json!([200, 310])),
    ]);
    d.track("Buddy Rig", "rotation", ease, &[
        pt(0.0, json!(0)), pt(72.0, json!(0)), pt(80.0, json!(-3)), pt(94.0, json!(3)), pt(102.0, json!(0)),
        pt(120.0, json!(0)), pt(133.0, json!(-6)), pt(146.0, json!(6)), pt(158.0, json!(0)), pt(170.0, json!(0)),
    ]);
    // Ears: tiny twitch while idle, perk up, wiggle while curious, flap on a hop.
    for (name, sign) in [("Ear L", -1.0), ("Ear R", 1.0)] {
        let base = 14.0 * sign;
        d.track(name, "rotation", ease, &[
            pt(0.0, json!(base)), pt(20.0, json!(base)), pt(24.0, json!(base + 7.0 * sign)), pt(28.0, json!(base)), pt(60.0, json!(base)),
            pt(66.0, json!(base - 10.0 * sign)), pt(72.0, json!(base - 6.0 * sign)), pt(80.0, json!(base - 12.0 * sign)), pt(88.0, json!(base - 3.0 * sign)), pt(95.0, json!(base - 12.0 * sign)), pt(102.0, json!(base - 6.0 * sign)),
            pt(114.0, json!(base)), pt(120.0, json!(base)), pt(126.0, json!(base + 8.0 * sign)), pt(133.0, json!(base - 22.0 * sign)), pt(140.0, json!(base + 8.0 * sign)), pt(146.0, json!(base - 18.0 * sign)), pt(158.0, json!(base)), pt(170.0, json!(base)),
        ]);
    }
    // Eyes: blink, widen, and squeeze happily shut.
    for name in ["Eye L", "Eye R"] {
        d.track(name, "scale", ease, &[
            pt(0.0, s(100.0, 100.0)), pt(36.0, s(100.0, 100.0)), pt(39.0, s(100.0, 8.0)), pt(43.0, s(100.0, 100.0)), pt(60.0, s(100.0, 100.0)),
            pt(66.0, s(124.0, 128.0)), pt(72.0, s(118.0, 120.0)), pt(90.0, s(118.0, 120.0)), pt(93.0, s(118.0, 10.0)), pt(97.0, s(118.0, 120.0)), pt(102.0, s(118.0, 120.0)),
            pt(114.0, s(100.0, 100.0)), pt(120.0, s(100.0, 100.0)), pt(126.0, s(110.0, 40.0)), pt(158.0, s(110.0, 40.0)), pt(166.0, s(100.0, 100.0)), pt(170.0, s(100.0, 100.0)),
        ]);
    }
    for (name, x) in [("Pupil L", -34.0), ("Pupil R", 34.0)] {
        d.track(name, "position", ease, &[
            pt(0.0, json!([x, -90])), pt(20.0, json!([x + 3.0, -90])), pt(50.0, json!([x - 3.0, -91])), pt(60.0, json!([x, -90])),
            pt(66.0, json!([x, -96])), pt(72.0, json!([x, -96])), pt(80.0, json!([x - 8.0, -97])), pt(90.0, json!([x + 8.0, -95])), pt(98.0, json!([x + 2.0, -97])), pt(102.0, json!([x, -96])),
            pt(114.0, json!([x, -90])), pt(170.0, json!([x, -90])),
        ]);
        d.track(name, "scale", ease, &[
            pt(0.0, s(100.0, 100.0)), pt(36.0, s(100.0, 100.0)), pt(39.0, s(100.0, 8.0)), pt(43.0, s(100.0, 100.0)), pt(60.0, s(100.0, 100.0)),
            pt(66.0, s(124.0, 128.0)), pt(72.0, s(118.0, 120.0)), pt(90.0, s(118.0, 120.0)), pt(93.0, s(118.0, 10.0)), pt(97.0, s(118.0, 120.0)), pt(102.0, s(118.0, 120.0)),
            pt(114.0, s(100.0, 100.0)), pt(120.0, s(100.0, 100.0)), pt(126.0, s(100.0, 6.0)), pt(158.0, s(100.0, 6.0)), pt(166.0, s(100.0, 100.0)), pt(170.0, s(100.0, 100.0)),
        ]);
    }
    for name in ["Cheek L", "Cheek R"] {
        d.track(name, "opacity", ease, &[pt(0.0, json!(35)), pt(60.0, json!(35)), pt(72.0, json!(75)), pt(102.0, json!(75)), pt(114.0, json!(35)), pt(120.0, json!(35)), pt(128.0, json!(100)), pt(158.0, json!(100)), pt(168.0, json!(35)), pt(170.0, json!(35))]);
    }
    d.shape_track("Mouth", "0", "size", ease, &[
        pt(0.0, json!([18, 6])), pt(60.0, json!([18, 6])), pt(68.0, json!([12, 12])), pt(72.0, json!([12, 12])), pt(102.0, json!([12, 12])), pt(114.0, json!([18, 6])),
        pt(120.0, json!([18, 6])), pt(128.0, json!([34, 26])), pt(158.0, json!([34, 26])), pt(166.0, json!([18, 6])), pt(170.0, json!([18, 6])),
    ]);

    // Hearts and a spark burst on pet.
    for i in 0..6 {
        let name = format!("Heart {}", i + 1);
        let x = 200.0 + (i as f64 - 2.5) * 22.0;
        d.shape(json!({"type": "rect", "name": name, "position": [x, 150], "size": [14, 14], "fill": "#ff4d6d"}));
        d.op("add_shape", json!({"layer": name, "type": "ellipse", "position": [0, -7], "size": [14, 14], "fill": "#ff4d6d"}));
        d.op("add_shape", json!({"layer": name, "type": "ellipse", "position": [-7, 0], "size": [14, 14], "fill": "#ff4d6d"}));
        d.op("set_transform", json!({"layer": name, "rotation": 45}));
        let t0 = 130.0 + i as f64 * 4.0;
        let drift = (i as f64 - 2.5) * 16.0;
        d.track(&name, "opacity", ease, &[pt(0.0, json!(0)), pt(t0 - 1.0, json!(0)), pt(t0 + 4.0, json!(100)), pt(t0 + 26.0, json!(0)), pt(170.0, json!(0))]);
        d.track(&name, "position", "ease_out", &[pt(0.0, json!([x, 160])), pt(t0, json!([x, 160])), pt(t0 + 26.0, json!([x + drift, 70 - (i % 3) as i64 * 18]))]);
        d.track(&name, "scale", ease, &[pt(0.0, scale(0.0)), pt(t0 - 1.0, scale(0.0)), pt(t0 + 6.0, scale(120.0 + (i % 2) as f64 * 40.0)), pt(t0 + 26.0, scale(70.0)), pt(170.0, scale(70.0))]);
    }
    d.shape(json!({"type": "star", "name": "Sparks", "position": [200, 232], "radius": 7, "inner_radius": 2.5, "points": 4, "fill": "#ffffff"}));
    d.op("update_shape_item", json!({"layer": "Sparks", "shape": "0", "props": {"position": [0, -118]}}));
    d.op("add_repeater", json!({"layer": "Sparks", "copies": 10, "rotation": 36}));
    d.op("set_fill", json!({"layer": "Sparks", "color": "#ffb703"}));
    d.track("Sparks", "scale", "ease_out", &[pt(0.0, scale(0.0)), pt(130.0, scale(0.0)), pt(150.0, scale(150.0)), pt(170.0, scale(150.0))]);
    d.track("Sparks", "opacity", ease, &[pt(0.0, json!(0)), pt(131.0, json!(0)), pt(133.0, json!(100)), pt(152.0, json!(0)), pt(170.0, json!(0))]);
    d.track("Sparks", "rotation", "ease_out", &[pt(0.0, json!(0)), pt(130.0, json!(0)), pt(152.0, json!(40))]);
    d.text("Hint", "HOVER · TAP TO PET", [200.0, 366.0], 12.0, "#b5835a", 3.0);

    let m = d.machine("Buddy");
    d.op("add_input", json!({"machine": m, "name": "hover", "type": "boolean", "value": false}));
    d.op("add_input", json!({"machine": m, "name": "pet", "type": "event"}));
    d.state(&m, "Idle", "idle", true);
    d.state(&m, "Perk", "perk", false);
    d.state(&m, "Curious", "curious", true);
    d.state(&m, "Relax", "relax", false);
    d.state(&m, "Happy", "happy", false);
    d.op("add_state", json!({"machine": m, "name": "Any", "type": "global"}));
    let h = |v: bool| json!([{"input": "hover", "condition": "Equal", "value": v}]);
    d.go(&m, "Idle", "Perk", h(true));
    d.go(&m, "Perk", "Curious", Value::Null);
    d.go(&m, "Curious", "Relax", h(false));
    d.go(&m, "Relax", "Idle", Value::Null);
    d.go(&m, "Happy", "Idle", Value::Null);
    d.go(&m, "Any", "Happy", json!([{"input": "pet"}]));
    for l in ["Body", "Belly", "Eye L", "Eye R", "Pupil L", "Pupil R", "Nose", "Mouth", "Ear L", "Ear R"] {
        d.op("add_interaction", json!({"machine": m, "type": "PointerEnter", "layer": l, "actions": [{"type": "set_boolean", "input": "hover", "value": true}]}));
        d.op("add_interaction", json!({"machine": m, "type": "PointerExit", "layer": l, "actions": [{"type": "set_boolean", "input": "hover", "value": false}]}));
        d.click(&m, l, json!([{"type": "fire", "input": "pet"}]));
    }
    d
}

// -------------------------------------------------------------------------
// 4. Groove: a music player. The record spins up, the tonearm drops, the
//    equalizer dances, notes drift up; tap again and it all winds down.
// -------------------------------------------------------------------------
fn groove() -> Doc {
    let mut d = Doc::new("Groove", 480.0, 400.0, 30.0, 115.0);
    d.segment("paused", 0.0, 1.0);
    d.segment("starting", 1.0, 25.0);
    d.segment("playing", 25.0, 85.0);
    d.segment("stopping", 85.0, 115.0);
    let ease = "ease_in_out";

    d.solid("Background", "#120a2a");
    d.shape(json!({"type": "ellipse", "name": "Backglow", "position": [170, 190], "size": [300, 300], "fill": "#ff4d8d"}));
    d.glow("Backglow", 90.0);
    d.track("Backglow", "opacity", ease, &[pt(0.0, json!(14)), pt(1.0, json!(14)), pt(25.0, json!(48)), pt(32.0, json!(62)), pt(40.0, json!(48)), pt(47.0, json!(62)), pt(55.0, json!(48)), pt(62.0, json!(62)), pt(70.0, json!(48)), pt(77.0, json!(62)), pt(85.0, json!(48)), pt(115.0, json!(14))]);

    // Vinyl: grooves, label, off-centre glint so you can see it spin.
    d.shape(json!({"type": "ellipse", "name": "Record", "position": [170, 190], "size": [230, 230], "fill": "#1b1233"}));
    d.op("set_gradient", json!({"layer": "Record", "kind": "fill", "type": "radial", "stops": [{"offset": 0, "color": "#2a1f4d"}, {"offset": 1, "color": "#0c0720"}]}));
    for r in [200.0, 172.0, 144.0, 116.0] {
        d.op("add_shape", json!({"layer": "Record", "type": "ellipse", "size": [r, r], "fill": null, "stroke": "#3b2f66", "stroke_width": 1.5}));
    }
    d.op("add_shape", json!({"layer": "Record", "type": "ellipse", "position": [0, 0], "size": [78, 78], "fill": "#ff4d8d"}));
    d.op("add_shape", json!({"layer": "Record", "type": "ellipse", "position": [20, -20], "size": [14, 14], "fill": "#ffd1e3"}));
    d.op("add_shape", json!({"layer": "Record", "type": "ellipse", "position": [0, 0], "size": [10, 10], "fill": "#120a2a"}));
    d.track("Record", "rotation", "ease_in", &[pt(0.0, json!(0)), pt(1.0, json!(0)), pt(25.0, json!(90))]);
    d.track("Record", "rotation", "linear", &[pt(25.0, json!(90)), pt(85.0, json!(450))]);
    d.track("Record", "rotation", "ease_out", &[pt(85.0, json!(450)), pt(115.0, json!(720))]);
    d.shape(json!({"type": "star", "name": "Sheen", "position": [170, 190], "radius": 114, "inner_radius": 3, "points": 2, "fill": "#ffffff"}));
    d.op("set_transform", json!({"layer": "Sheen", "opacity": 7, "rotation": 35}));

    // Tonearm swings onto the record.
    d.op("create_null", json!({"name": "Arm Pivot", "position": [332, 24]}));
    d.shape(json!({"type": "rect", "name": "Arm", "position": [0, 66], "size": [9, 140], "roundness": 4, "fill": "#d6cfe8"}));
    d.op("add_shape", json!({"layer": "Arm", "type": "rect", "position": [0, 68], "size": [22, 30], "roundness": 6, "fill": "#ff4d8d"}));
    d.op("add_shape", json!({"layer": "Arm", "type": "ellipse", "position": [0, -66], "size": [30, 30], "fill": "#3b2f66", "stroke": "#d6cfe8", "stroke_width": 3}));
    d.op("set_parent", json!({"layer": "Arm", "parent": "Arm Pivot"}));
    d.track("Arm Pivot", "rotation", "ease_out_back", &[pt(0.0, json!(-8)), pt(1.0, json!(-8)), pt(25.0, json!(25))]);
    d.track("Arm Pivot", "rotation", ease, &[pt(25.0, json!(25)), pt(40.0, json!(25.8)), pt(55.0, json!(25)), pt(70.0, json!(25.8)), pt(85.0, json!(25))]);
    d.track("Arm Pivot", "rotation", ease, &[pt(85.0, json!(25)), pt(100.0, json!(-8)), pt(115.0, json!(-8))]);

    // Equalizer bars: each loops its own bounce, dropping to a flat line when paused.
    let patterns: [[f64; 7]; 7] = [
        [0.55, 0.9, 0.4, 0.8, 0.5, 0.95, 0.55],
        [0.8, 0.45, 0.95, 0.5, 0.85, 0.4, 0.8],
        [0.4, 0.75, 0.6, 1.0, 0.45, 0.7, 0.4],
        [0.95, 0.5, 0.8, 0.4, 0.9, 0.55, 0.95],
        [0.5, 0.85, 0.45, 0.9, 0.6, 0.8, 0.5],
        [0.75, 0.4, 0.9, 0.55, 0.8, 0.45, 0.75],
        [0.45, 0.8, 0.55, 0.95, 0.4, 0.85, 0.45],
    ];
    let cols = ["#ff4d8d", "#ff6f91", "#ff9671", "#ffc75f", "#8be9a8", "#3ee0ff", "#6c8bff"];
    for (i, pat) in patterns.iter().enumerate() {
        let name = format!("Bar {}", i + 1);
        let x = 304.0 + i as f64 * 23.0;
        d.shape(json!({"type": "rect", "name": name, "position": [x, 210], "size": [15, 120], "roundness": 7.5, "fill": cols[i]}));
        d.op("set_anchor_point", json!({"layer": name, "anchor": "bottom"}));
        d.op("set_transform", json!({"layer": name, "scale": sy(100.0, 6.0)}));
        d.op("set_gradient", json!({"layer": name, "kind": "fill", "type": "linear", "start": [0, -120], "end": [0, 0], "stops": [{"offset": 0, "color": cols[i]}, {"offset": 1, "color": "#7a3cff"}]}));
        let mut k = vec![pt(0.0, sy(100.0, 6.0)), pt(1.0, sy(100.0, 6.0))];
        k.push(pt(13.0, sy(100.0, pat[0] * 55.0 + 10.0)));
        k.push(pt(25.0, sy(100.0, pat[0] * 100.0)));
        for (j, v) in pat.iter().enumerate().skip(1) {
            k.push(pt(25.0 + j as f64 * 10.0, sy(100.0, v * 100.0)));
        }
        k.push(pt(85.0, sy(100.0, pat[0] * 100.0)));
        k.push(pt(115.0, sy(100.0, 6.0)));
        d.track(&name, "scale", "ease_in_out", &k);
    }

    // Play / pause button with pulse ring.
    d.shape(json!({"type": "ellipse", "name": "Pulse Ring", "position": [370, 330], "size": [64, 64], "fill": null, "stroke": "#ff4d8d", "stroke_width": 3}));
    d.track("Pulse Ring", "scale", "ease_out", &[pt(0.0, scale(100.0)), pt(25.0, scale(100.0)), pt(55.0, scale(185.0)), pt(55.0, scale(100.0)), pt(85.0, scale(185.0)), pt(115.0, scale(185.0))]);
    d.track("Pulse Ring", "opacity", ease, &[pt(0.0, json!(0)), pt(25.0, json!(70)), pt(55.0, json!(0)), pt(56.0, json!(70)), pt(85.0, json!(0)), pt(115.0, json!(0))]);
    d.shape(json!({"type": "ellipse", "name": "Play Button", "position": [370, 330], "size": [64, 64], "fill": "#ff4d8d"}));
    d.op("set_gradient", json!({"layer": "Play Button", "kind": "fill", "type": "linear", "start": [-30, -30], "end": [30, 30], "stops": [{"offset": 0, "color": "#ff7aa8"}, {"offset": 1, "color": "#ff2d6f"}]}));
    d.op("add_effect", json!({"layer": "Play Button", "type": "drop_shadow", "params": {"color": "#ff2d6f", "opacity": 55, "distance": 6, "softness": 18, "direction": 180}}));
    d.shape(json!({"type": "polygon", "name": "Play Icon", "position": [372, 330], "radius": 15, "points": 3, "roundness": 12, "fill": "#ffffff"}));
    d.op("set_transform", json!({"layer": "Play Icon", "rotation": 90}));
    d.track("Play Icon", "scale", ease, &[pt(0.0, scale(100.0)), pt(1.0, scale(100.0)), pt(10.0, scale(0.0)), pt(92.0, scale(0.0)), pt(106.0, scale(100.0)), pt(115.0, scale(100.0))]);
    d.shape(json!({"type": "rect", "name": "Pause Icon", "position": [362, 330], "size": [8, 24], "roundness": 3, "fill": "#ffffff"}));
    d.op("add_shape", json!({"layer": "Pause Icon", "type": "rect", "position": [16, 0], "size": [8, 24], "roundness": 3, "fill": "#ffffff"}));
    d.track("Pause Icon", "scale", ease, &[pt(0.0, scale(0.0)), pt(8.0, scale(0.0)), pt(20.0, scale(100.0)), pt(85.0, scale(100.0)), pt(97.0, scale(0.0)), pt(115.0, scale(0.0))]);

    // Notes drift up while music plays.
    for (i, (x, t0)) in [(104.0, 27.0), (232.0, 38.0), (182.0, 50.0)].iter().enumerate() {
        let name = format!("Note {}", i + 1);
        d.shape(json!({"type": "ellipse", "name": name, "position": [*x, 120], "size": [13, 10], "fill": "#ffd1e3"}));
        d.op("add_shape", json!({"layer": name, "type": "rect", "position": [5.5, -12], "size": [2.5, 22], "fill": "#ffd1e3"}));
        d.op("add_shape", json!({"layer": name, "type": "rect", "position": [10, -21], "size": [8, 3], "fill": "#ffd1e3"}));
        d.track(&name, "opacity", ease, &[pt(0.0, json!(0)), pt(t0 - 1.0, json!(0)), pt(t0 + 6.0, json!(100)), pt(t0 + 22.0, json!(0)), pt(115.0, json!(0))]);
        d.track(&name, "position", "ease_out", &[pt(0.0, json!([*x, 110])), pt(*t0, json!([*x, 110])), pt(t0 + 22.0, json!([x + 20.0 * (1.0 - i as f64), 34]))]);
        d.track(&name, "rotation", ease, &[pt(0.0, json!(0)), pt(*t0, json!(-12)), pt(t0 + 11.0, json!(12)), pt(t0 + 22.0, json!(-12))]);
    }

    // Title swaps between PAUSED and NOW PLAYING.
    d.text("Title Paused", "PAUSED", [170.0, 346.0], 22.0, "#ffffff", 4.0);
    d.text("Title Playing", "NOW PLAYING", [170.0, 346.0], 22.0, "#ffffff", 4.0);
    d.track("Title Paused", "opacity", ease, &[pt(0.0, json!(100)), pt(1.0, json!(100)), pt(10.0, json!(0)), pt(94.0, json!(0)), pt(112.0, json!(100)), pt(115.0, json!(100))]);
    d.track("Title Playing", "opacity", ease, &[pt(0.0, json!(0)), pt(1.0, json!(0)), pt(14.0, json!(100)), pt(85.0, json!(100)), pt(97.0, json!(0)), pt(115.0, json!(0))]);
    d.text("Subtitle", "lumetrope · groove", [170.0, 372.0], 13.0, "#9d8fd0", 2.0);
    d.text("Hint", "TAP THE BUTTON", [370.0, 384.0], 11.0, "#6e62a0", 3.0);

    let m = d.machine("Player");
    d.op("add_input", json!({"machine": m, "name": "playing", "type": "boolean", "value": false}));
    d.state(&m, "Paused", "paused", false);
    d.state(&m, "Starting", "starting", false);
    d.state(&m, "Playing", "playing", true);
    d.state(&m, "Stopping", "stopping", false);
    let p = |v: bool| json!([{"input": "playing", "condition": "Equal", "value": v}]);
    d.go(&m, "Paused", "Starting", p(true));
    d.go(&m, "Starting", "Playing", Value::Null);
    d.go(&m, "Playing", "Stopping", p(false));
    d.go(&m, "Stopping", "Paused", Value::Null);
    // One interaction on the button disc only: the icons sit above it, so a click on them still hits it,
    // and listing them too would toggle once per layer under the pointer.
    d.click(&m, "Play Button", json!([{"type": "toggle", "input": "playing"}]));
    d
}

fn write_all(dir: &std::path::Path) {
    for (name, doc) in [("demo-bloom.lottie", bloom()), ("demo-day-night.lottie", day_night()), ("demo-buddy.lottie", buddy()), ("demo-groove.lottie", groove())] {
        std::fs::write(dir.join(name), doc.0.export_dotlottie().unwrap()).unwrap();
    }
}

#[test]
#[ignore]
fn write_demo_samples() {
    write_all(&samples_dir());
}

/// Contact sheets of every demo for eyeballing: `PREVIEW_DIR=/tmp/x cargo test ... preview_demo_samples`.
#[test]
#[ignore]
fn preview_demo_samples() {
    let out = PathBuf::from(std::env::var("PREVIEW_DIR").expect("PREVIEW_DIR"));
    let which = std::env::var("ONLY").ok();
    let all: Vec<(&str, Doc, Vec<f64>)> = vec![
        ("bloom", bloom(), vec![0.0, 20.0, 60.0, 70.0, 78.0, 90.0, 104.0]),
        ("day_night", day_night(), vec![0.0, 30.0, 70.0, 85.0, 100.0, 126.0, 170.0, 190.0]),
        ("buddy", buddy(), vec![0.0, 40.0, 66.0, 80.0, 94.0, 126.0, 133.0, 146.0, 158.0, 170.0]),
        ("groove", groove(), vec![0.0, 12.0, 25.0, 40.0, 62.0, 85.0, 100.0, 114.0]),
    ];
    for (name, mut d, frames) in all {
        if which.as_deref().map(|w| w != name).unwrap_or(false) {
            continue;
        }
        let info = d.0.info();
        let (aw, ah) = (info["w"].as_f64().unwrap(), info["h"].as_f64().unwrap());
        let tw = 300u32;
        let th = (tw as f64 * ah / aw) as u32;
        let cols = 4u32;
        let rows = (frames.len() as u32 + cols - 1) / cols;
        let mut sheet = tiny_skia::Pixmap::new(tw * cols, th * rows).unwrap();
        sheet.fill(tiny_skia::Color::from_rgba8(40, 40, 40, 255));
        for (i, f) in frames.iter().enumerate() {
            let rgba = d.0.render_rgba(*f, tw, th);
            let mut pm = tiny_skia::Pixmap::new(tw, th).unwrap();
            for (dst, src) in pm.data_mut().chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                let a = src[3] as u32;
                dst[0] = (src[0] as u32 * a / 255) as u8;
                dst[1] = (src[1] as u32 * a / 255) as u8;
                dst[2] = (src[2] as u32 * a / 255) as u8;
                dst[3] = src[3];
            }
            let (cx, cy) = ((i as u32 % cols) * tw, (i as u32 / cols) * th);
            sheet.draw_pixmap(cx as i32, cy as i32, pm.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        }
        sheet.save_png(out.join(format!("{name}.png"))).unwrap();
    }
}
