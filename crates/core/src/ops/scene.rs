//! Scene-level tools.

use serde_json::{json, Value};

use super::catalog::{obj, ToolDef};
use super::{err, opt_f64, opt_str, Cx, OpResult};

pub fn defs() -> Vec<ToolDef> {
    vec![ToolDef {
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
