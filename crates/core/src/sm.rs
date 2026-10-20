//! dotLottie 2.0 state machine runtime.
//!
//! Semantics (dotLottie spec): global-state transitions are checked before the
//! current state's, in declaration order; all guards of a transition must pass;
//! event inputs are consumed after each evaluation run; tweened transitions
//! block evaluation until they finish; exit actions run before a state change,
//! entry actions after it (after the tween for tweened transitions).
//! Builder's call: guardless transitions fire once the current playback state
//! has completed, so "play then continue" chains work without infinite loops.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::anim::cubic_ease;
use crate::model::Animation;

/// Hard cap on transitions per evaluation run (prevents A↔B ping-pong hangs).
const MAX_STEPS: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum InputValue {
    Bool(bool),
    Num(f64),
    Str(String),
    Event,
}

impl InputValue {
    pub fn to_json(&self) -> Value {
        match self {
            InputValue::Bool(b) => Value::Bool(*b),
            InputValue::Num(n) => serde_json::json!(n),
            InputValue::Str(s) => Value::String(s.clone()),
            InputValue::Event => Value::Null,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Down,
    Up,
    Move,
    Enter,
    Exit,
    Click,
}

impl PointerKind {
    pub fn parse(s: &str) -> Option<PointerKind> {
        Some(match s.to_ascii_lowercase().as_str() {
            "down" | "pointerdown" => PointerKind::Down,
            "up" | "pointerup" => PointerKind::Up,
            "move" | "pointermove" => PointerKind::Move,
            "enter" | "pointerenter" => PointerKind::Enter,
            "exit" | "pointerexit" | "leave" => PointerKind::Exit,
            "click" => PointerKind::Click,
            _ => return None,
        })
    }
    fn interaction(self) -> &'static str {
        match self {
            PointerKind::Down => "PointerDown",
            PointerKind::Up => "PointerUp",
            PointerKind::Move => "PointerMove",
            PointerKind::Enter => "PointerEnter",
            PointerKind::Exit => "PointerExit",
            PointerKind::Click => "Click",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SmEvent {
    StateEntered(String),
    StateExited(String),
    Transition { from: String, to: String },
    OpenUrl { url: String, target: String },
    CustomEvent(String),
    SetTheme(String),
    InputChanged { name: String, value: Value },
    Complete,
    LoopComplete,
}

impl SmEvent {
    pub fn to_json(&self) -> Value {
        use serde_json::json;
        match self {
            SmEvent::StateEntered(s) => json!({"type": "StateEntered", "state": s}),
            SmEvent::StateExited(s) => json!({"type": "StateExited", "state": s}),
            SmEvent::Transition { from, to } => json!({"type": "Transition", "from": from, "to": to}),
            SmEvent::OpenUrl { url, target } => json!({"type": "OpenUrl", "url": url, "target": target}),
            SmEvent::CustomEvent(s) => json!({"type": "CustomEvent", "value": s}),
            SmEvent::SetTheme(s) => json!({"type": "SetTheme", "theme": s}),
            SmEvent::InputChanged { name, value } => json!({"type": "InputChanged", "name": name, "value": value}),
            SmEvent::Complete => json!({"type": "Complete"}),
            SmEvent::LoopComplete => json!({"type": "LoopComplete"}),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Forward,
    Reverse,
    Bounce,
    ReverseBounce,
}

#[derive(Debug, Clone)]
struct Playback {
    frame: f64,
    start: f64,
    end: f64,
    playing: bool,
    looping: bool,
    loop_count: Option<u32>,
    loops_done: u32,
    mode: Mode,
    speed: f64,
    dir: f64,
    completed: bool,
}

#[derive(Debug, Clone)]
struct Tween {
    to_state: usize,
    from_frame: f64,
    to_frame: f64,
    elapsed: f64,
    duration: f64,
    easing: [f64; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub state: String,
    pub frame: f64,
    pub playing: bool,
    pub tweening: bool,
    pub inputs: Vec<(String, InputValue)>,
    pub theme: Option<String>,
    pub stopped: bool,
}

pub struct Machine {
    states: Vec<Value>,
    interactions: Vec<Value>,
    input_defs: Vec<(String, InputValue)>,
    values: HashMap<String, InputValue>,
    initial: usize,
    current: usize,
    global: Option<usize>,
    pending_event: Option<String>,
    tween: Option<Tween>,
    pb: Playback,
    fr: f64,
    ip: f64,
    op: f64,
    markers: Vec<(String, f64, f64)>,
    events: Vec<SmEvent>,
    hovered: HashSet<String>,
    theme: Option<String>,
    stopped: bool,
}

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

impl Machine {
    pub fn new(def: &Value, anim: &Animation) -> Result<Machine, String> {
        if !def.is_object() {
            return Err("state machine must be a JSON object".into());
        }
        let states: Vec<Value> = def.get("states").and_then(Value::as_array).cloned().unwrap_or_default();
        let initial_name = s(def, "initial").unwrap_or("");
        let initial = states
            .iter()
            .position(|st| s(st, "name") == Some(initial_name) && s(st, "type") != Some("GlobalState"))
            .ok_or_else(|| format!("initial state '{initial_name}' not found"))?;
        let global = states.iter().position(|st| s(st, "type") == Some("GlobalState"));
        let mut input_defs = vec![];
        for i in def.get("inputs").and_then(Value::as_array).cloned().unwrap_or_default() {
            let name = s(&i, "name").unwrap_or("").to_string();
            let v = match s(&i, "type").unwrap_or("") {
                "Boolean" => InputValue::Bool(i.get("value").and_then(Value::as_bool).unwrap_or(false)),
                "Numeric" => InputValue::Num(i.get("value").and_then(Value::as_f64).unwrap_or(0.0)),
                "String" => InputValue::Str(s(&i, "value").unwrap_or("").to_string()),
                _ => InputValue::Event,
            };
            input_defs.push((name, v));
        }
        let values = input_defs.iter().cloned().collect();
        Ok(Machine {
            states,
            interactions: def.get("interactions").and_then(Value::as_array).cloned().unwrap_or_default(),
            input_defs,
            values,
            initial,
            current: initial,
            global,
            pending_event: None,
            tween: None,
            pb: Playback { frame: anim.ip, start: anim.ip, end: anim.op, playing: false, looping: false, loop_count: None, loops_done: 0, mode: Mode::Forward, speed: 1.0, dir: 1.0, completed: false },
            fr: anim.fr,
            ip: anim.ip,
            op: anim.op,
            markers: anim.markers.iter().map(|m| (m.name.clone(), m.time, m.time + m.duration)).collect(),
            events: vec![],
            hovered: HashSet::new(),
            theme: None,
            stopped: false,
        })
    }

    /// Reset inputs and enter the initial state.
    pub fn start(&mut self) -> Vec<SmEvent> {
        self.values = self.input_defs.iter().cloned().collect();
        self.tween = None;
        self.stopped = false;
        self.hovered.clear();
        self.current = self.initial;
        self.enter(self.initial);
        self.run();
        std::mem::take(&mut self.events)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: s(&self.states[self.current], "name").unwrap_or("").to_string(),
            frame: self.pb.frame,
            playing: self.pb.playing,
            tweening: self.tween.is_some(),
            inputs: self.input_defs.iter().map(|(n, _)| (n.clone(), self.values.get(n).cloned().unwrap_or(InputValue::Event))).collect(),
            theme: self.theme.clone(),
            stopped: self.stopped,
        }
    }

    pub fn input(&self, name: &str) -> Option<InputValue> {
        self.values.get(name).cloned()
    }

    pub fn state_names(&self) -> Vec<String> {
        self.states.iter().filter_map(|st| s(st, "name").map(str::to_string)).collect()
    }

    /// Set an input from outside (e.g. the host app) and evaluate.
    pub fn set_input(&mut self, name: &str, v: InputValue) -> Vec<SmEvent> {
        if self.tween.is_none() {
            self.assign(name, v);
            self.run();
        }
        std::mem::take(&mut self.events)
    }

    /// Fire an event input and evaluate.
    pub fn fire(&mut self, name: &str) -> Vec<SmEvent> {
        if self.tween.is_none() {
            self.pending_event = Some(name.to_string());
            self.run();
        }
        std::mem::take(&mut self.events)
    }

    /// A pointer event over the given layer names (topmost first).
    pub fn pointer(&mut self, kind: PointerKind, layers: &[String]) -> Vec<SmEvent> {
        let mut fire: Vec<usize> = vec![];
        let now: HashSet<String> = layers.iter().cloned().collect();
        for (i, it) in self.interactions.iter().enumerate() {
            let ty = s(it, "type").unwrap_or("");
            let layer = s(it, "layerName");
            let hit = match (ty, layer) {
                // Layer enter/exit is derived from hover changes on move/enter/exit.
                ("PointerEnter", Some(l)) => matches!(kind, PointerKind::Move | PointerKind::Enter) && now.contains(l) && !self.hovered.contains(l),
                ("PointerExit", Some(l)) => matches!(kind, PointerKind::Move | PointerKind::Exit) && !now.contains(l) && self.hovered.contains(l),
                (t, Some(l)) => t == kind.interaction() && now.contains(l),
                (t, None) => t == kind.interaction(),
            };
            if hit {
                fire.push(i);
            }
        }
        self.hovered = if kind == PointerKind::Exit { HashSet::new() } else { now };
        for i in fire {
            let acts = self.interactions[i].get("actions").cloned().unwrap_or(Value::Null);
            self.actions(&acts);
        }
        self.run();
        std::mem::take(&mut self.events)
    }

    /// Advance time by `dt` seconds.
    pub fn tick(&mut self, dt: f64) -> Vec<SmEvent> {
        if let Some(mut tw) = self.tween.take() {
            tw.elapsed += dt;
            let p = (tw.elapsed / tw.duration.max(1e-6)).min(1.0);
            let e = cubic_ease(tw.easing[0], tw.easing[1], tw.easing[2], tw.easing[3], p);
            self.pb.frame = tw.from_frame + (tw.to_frame - tw.from_frame) * e;
            if p >= 1.0 {
                self.current = tw.to_state;
                self.enter(tw.to_state);
                self.pending_event = None;
                self.run();
            } else {
                self.tween = Some(tw);
            }
            return std::mem::take(&mut self.events);
        }
        if self.pb.playing && !self.stopped {
            self.advance(dt * self.fr * self.pb.speed.abs().max(0.0));
        }
        std::mem::take(&mut self.events)
    }

    // ------------------------------------------------------------ internals

    fn segment(&self, st: &Value) -> (f64, f64) {
        if let Some(name) = s(st, "segment") {
            if let Some(m) = self.markers.iter().find(|m| m.0 == name) {
                return (m.1, m.2.max(m.1));
            }
        }
        (self.ip, (self.op - 1.0).max(self.ip))
    }

    fn enter(&mut self, idx: usize) {
        let st = self.states[idx].clone();
        let name = s(&st, "name").unwrap_or("").to_string();
        if s(&st, "type") == Some("PlaybackState") {
            let (start, end) = self.segment(&st);
            let mode = match s(&st, "mode").unwrap_or("Forward") {
                "Reverse" => Mode::Reverse,
                "Bounce" => Mode::Bounce,
                "ReverseBounce" => Mode::ReverseBounce,
                _ => Mode::Forward,
            };
            let looping = st.get("loop").and_then(Value::as_bool).unwrap_or(false);
            let reversed = matches!(mode, Mode::Reverse | Mode::ReverseBounce);
            self.pb = Playback {
                frame: if reversed { end } else { start },
                start,
                end,
                playing: st.get("autoplay").and_then(Value::as_bool).unwrap_or(false),
                looping,
                loop_count: if looping { st.get("loopCount").and_then(Value::as_u64).filter(|c| *c >= 1).map(|c| c as u32) } else { None },
                loops_done: 0,
                mode,
                speed: st.get("speed").and_then(Value::as_f64).unwrap_or(1.0),
                dir: if reversed { -1.0 } else { 1.0 },
                completed: false,
            };
            if st.get("final").and_then(Value::as_bool).unwrap_or(false) {
                self.stopped = true;
            }
        }
        self.events.push(SmEvent::StateEntered(name));
        if let Some(a) = st.get("entryActions").cloned() {
            self.actions(&a);
        }
    }

    fn exit(&mut self, idx: usize) {
        let st = self.states[idx].clone();
        if let Some(a) = st.get("exitActions").cloned() {
            self.actions(&a);
        }
        self.events.push(SmEvent::StateExited(s(&st, "name").unwrap_or("").to_string()));
    }

    fn advance(&mut self, df: f64) {
        let pb = &mut self.pb;
        let span = (pb.end - pb.start).max(0.0);
        let mut remaining = df;
        let mut guard = 0;
        while remaining > 1e-9 && pb.playing && guard < 1000 {
            guard += 1;
            let target = if pb.dir > 0.0 { pb.end } else { pb.start };
            let dist = (target - pb.frame).abs();
            if span <= 0.0 || remaining < dist {
                pb.frame += remaining * pb.dir;
                if span <= 0.0 {
                    pb.frame = pb.start;
                }
                break;
            }
            pb.frame = target;
            remaining -= dist;
            let bounce = matches!(pb.mode, Mode::Bounce | Mode::ReverseBounce);
            // A bounce cycle is complete when we return to where we started.
            let at_cycle_end = !bounce || (pb.mode == Mode::Bounce && pb.dir < 0.0) || (pb.mode == Mode::ReverseBounce && pb.dir > 0.0);
            if bounce && !at_cycle_end {
                pb.dir = -pb.dir;
                continue;
            }
            if pb.looping {
                pb.loops_done += 1;
                self.events.push(SmEvent::LoopComplete);
                if pb.loop_count.map(|c| pb.loops_done >= c).unwrap_or(false) {
                    pb.playing = false;
                    pb.completed = true;
                    self.events.push(SmEvent::Complete);
                    break;
                }
                if bounce {
                    pb.dir = -pb.dir;
                } else {
                    pb.frame = if pb.dir > 0.0 { pb.start } else { pb.end };
                }
            } else {
                pb.playing = false;
                pb.completed = true;
                self.events.push(SmEvent::Complete);
                break;
            }
        }
        // Fire OnLoopComplete / OnComplete interactions for the current state.
        let state = s(&self.states[self.current], "name").unwrap_or("").to_string();
        let new_events: Vec<SmEvent> = self.events.iter().filter(|e| matches!(e, SmEvent::Complete | SmEvent::LoopComplete)).cloned().collect();
        for e in new_events {
            let ty = if e == SmEvent::Complete { "OnComplete" } else { "OnLoopComplete" };
            let acts: Vec<Value> = self
                .interactions
                .iter()
                .filter(|it| s(it, "type") == Some(ty) && s(it, "stateName") == Some(state.as_str()))
                .filter_map(|it| it.get("actions").cloned())
                .collect();
            for a in acts {
                self.actions(&a);
            }
        }
        self.run();
    }

    fn num_ref(&self, v: Option<&Value>, default: f64) -> f64 {
        match v {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
            Some(Value::String(r)) if r.starts_with('$') => match self.values.get(&r[1..]) {
                Some(InputValue::Num(n)) => *n,
                _ => default,
            },
            Some(Value::String(r)) => r.parse().unwrap_or(default),
            _ => default,
        }
    }

    fn resolve(&self, v: &Value) -> Value {
        match v {
            Value::String(r) if r.starts_with('$') => self.values.get(&r[1..]).map(InputValue::to_json).unwrap_or(Value::Null),
            other => other.clone(),
        }
    }

    fn assign(&mut self, name: &str, v: InputValue) {
        let Some(cur) = self.values.get(name) else { return };
        let ok = matches!((cur, &v), (InputValue::Bool(_), InputValue::Bool(_)) | (InputValue::Num(_), InputValue::Num(_)) | (InputValue::Str(_), InputValue::Str(_)));
        if ok && *cur != v {
            self.events.push(SmEvent::InputChanged { name: name.to_string(), value: v.to_json() });
            self.values.insert(name.to_string(), v);
        }
    }

    fn actions(&mut self, list: &Value) {
        let Some(list) = list.as_array() else { return };
        for a in list.clone() {
            let input = s(&a, "inputName").unwrap_or("").to_string();
            match s(&a, "type").unwrap_or("") {
                "Increment" | "Decrement" => {
                    let step = self.num_ref(a.get("value"), 1.0);
                    if let Some(InputValue::Num(n)) = self.values.get(&input).cloned() {
                        let sign = if s(&a, "type") == Some("Increment") { 1.0 } else { -1.0 };
                        self.assign(&input, InputValue::Num(n + sign * step));
                    }
                }
                "Toggle" => {
                    if let Some(InputValue::Bool(b)) = self.values.get(&input).cloned() {
                        self.assign(&input, InputValue::Bool(!b));
                    }
                }
                "SetBoolean" => {
                    let v = self.resolve(a.get("value").unwrap_or(&Value::Bool(false)));
                    self.assign(&input, InputValue::Bool(v.as_bool().unwrap_or(false)));
                }
                "SetNumeric" => {
                    let v = self.num_ref(a.get("value"), 0.0);
                    self.assign(&input, InputValue::Num(v));
                }
                "SetString" => {
                    let v = self.resolve(a.get("value").unwrap_or(&Value::Null));
                    self.assign(&input, InputValue::Str(v.as_str().unwrap_or("").to_string()));
                }
                "Fire" => self.pending_event = Some(input),
                "Reset" => {
                    if let Some((_, init)) = self.input_defs.iter().find(|(n, _)| *n == input).cloned() {
                        self.assign(&input, init);
                    }
                }
                "SetFrame" => {
                    let f = self.num_ref(a.get("value"), self.pb.frame);
                    self.pb.frame = f;
                }
                "SetProgress" => {
                    let p = self.num_ref(a.get("value"), 0.0).clamp(0.0, 1.0);
                    self.pb.frame = self.pb.start + (self.pb.end - self.pb.start) * p;
                }
                "OpenUrl" => {
                    let url = self.resolve(a.get("url").unwrap_or(&Value::Null));
                    self.events.push(SmEvent::OpenUrl { url: url.as_str().unwrap_or("").to_string(), target: s(&a, "target").unwrap_or("_blank").to_string() });
                }
                "SetTheme" => {
                    let t = self.resolve(a.get("value").unwrap_or(&Value::Null));
                    let t = t.as_str().unwrap_or("").to_string();
                    self.theme = Some(t.clone());
                    self.events.push(SmEvent::SetTheme(t));
                }
                "FireCustomEvent" => {
                    let v = a.get("value").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
                    self.events.push(SmEvent::CustomEvent(v));
                }
                _ => {}
            }
        }
    }

    fn guard_ok(&self, g: &Value) -> bool {
        let name = s(g, "inputName").unwrap_or("");
        let ty = s(g, "type").unwrap_or("");
        if ty == "Event" {
            return self.pending_event.as_deref() == Some(name);
        }
        let cond = s(g, "conditionType").unwrap_or("Equal");
        let cmp = self.resolve(g.get("compareTo").unwrap_or(&Value::Null));
        match self.values.get(name) {
            Some(InputValue::Num(v)) => {
                let Some(c) = cmp.as_f64().or_else(|| cmp.as_str().and_then(|x| x.parse().ok())) else { return false };
                match cond {
                    "Equal" => (v - c).abs() < 1e-9,
                    "NotEqual" => (v - c).abs() >= 1e-9,
                    "GreaterThan" => *v > c,
                    "GreaterThanOrEqual" => *v >= c,
                    "LessThan" => *v < c,
                    "LessThanOrEqual" => *v <= c,
                    _ => false,
                }
            }
            Some(InputValue::Bool(v)) => {
                let c = cmp.as_bool().or_else(|| cmp.as_str().map(|x| x == "true")).unwrap_or(false);
                if cond == "NotEqual" {
                    *v != c
                } else {
                    *v == c
                }
            }
            Some(InputValue::Str(v)) => {
                let c = cmp.as_str().unwrap_or("");
                if cond == "NotEqual" {
                    v != c
                } else {
                    v == c
                }
            }
            _ => false,
        }
    }

    /// First passing transition of state `idx`: (target index, transition JSON).
    fn pick(&self, idx: usize, allow_guardless: bool) -> Option<(usize, Value)> {
        let ts = self.states[idx].get("transitions").and_then(Value::as_array)?;
        let mut fallback = None;
        for t in ts {
            let to = s(t, "toState").and_then(|n| self.states.iter().position(|st| s(st, "name") == Some(n) && s(st, "type") != Some("GlobalState")));
            let Some(to) = to else { continue };
            match t.get("guards").and_then(Value::as_array).filter(|g| !g.is_empty()) {
                Some(guards) => {
                    if guards.iter().all(|g| self.guard_ok(g)) {
                        return Some((to, t.clone()));
                    }
                }
                None if allow_guardless && fallback.is_none() => fallback = Some((to, t.clone())),
                None => {}
            }
        }
        fallback
    }

    fn run(&mut self) {
        for _ in 0..MAX_STEPS {
            if self.tween.is_some() || self.stopped {
                break;
            }
            let from = self.current;
            let picked = self.global.and_then(|g| self.pick(g, false)).or_else(|| self.pick(from, self.pb.completed));
            let Some((to, t)) = picked else { break };
            if to == from && t.get("guards").is_none() {
                break;
            }
            self.pending_event = None;
            self.exit(from);
            let names = (s(&self.states[from], "name").unwrap_or("").to_string(), s(&self.states[to], "name").unwrap_or("").to_string());
            self.events.push(SmEvent::Transition { from: names.0, to: names.1 });
            if s(&t, "type") == Some("Tweened") {
                let target = self.states[to].clone();
                let (start, end) = self.segment(&target);
                let reversed = matches!(s(&target, "mode"), Some("Reverse") | Some("ReverseBounce"));
                let e = t.get("easing").and_then(Value::as_array).map(|a| [0, 1, 2, 3].map(|i| a.get(i).and_then(Value::as_f64).unwrap_or(0.0))).unwrap_or([0.0, 0.0, 1.0, 1.0]);
                self.tween = Some(Tween {
                    to_state: to,
                    from_frame: self.pb.frame,
                    to_frame: if reversed { end } else { start },
                    elapsed: 0.0,
                    duration: t.get("duration").and_then(Value::as_f64).unwrap_or(0.5).max(0.0),
                    easing: e,
                });
                self.pb.playing = false;
                break;
            }
            self.current = to;
            self.enter(to);
        }
        self.pending_event = None;
    }
}
