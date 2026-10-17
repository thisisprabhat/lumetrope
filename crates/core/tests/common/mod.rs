#![allow(dead_code)]
use lumetrope_core::engine::Engine;
use serde_json::Value;

pub fn eng() -> Engine {
    let mut e = Engine::new();
    e.new_project(400.0, 300.0, 30.0, 60.0);
    e
}

pub fn ok(e: &mut Engine, tool: &str, args: Value) -> Value {
    let r = e.apply(tool, &args);
    if let Some(err) = r.get("error") {
        panic!("{tool} failed: {err}");
    }
    r["ok"].clone()
}

pub fn fail(e: &mut Engine, tool: &str, args: Value) -> String {
    let r = e.apply(tool, &args);
    r.get("error").and_then(Value::as_str).unwrap_or_else(|| panic!("{tool} unexpectedly succeeded: {r}")).to_string()
}

pub fn anim(e: &Engine) -> Value {
    lumetrope_core::project::active_anim(e.project()).clone()
}

pub fn layer(e: &Engine, idx: usize) -> Value {
    anim(e)["layers"][idx].clone()
}
