//! The tool catalogue: every operation with its description and JSON Schema.
//! The MCP server serves this list verbatim, so agents and the UI share one API.

use serde_json::{json, Value};

use super::{Cx, OpResult};

pub type Handler = fn(&mut Cx, &Value) -> OpResult;

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
    /// Whether the tool changes the document (false = read-only query).
    pub mutates: bool,
    pub handler: Handler,
}

pub fn tools() -> Vec<ToolDef> {
    let mut v = vec![];
    v.extend(super::scene::defs());
    v.extend(super::layers::defs());
    v.extend(super::keys::defs());
    v.extend(super::shapes::defs());
    v
}

pub fn find(name: &str) -> Option<ToolDef> {
    tools().into_iter().find(|t| t.name == name)
}

/// `[{name, description, inputSchema, mutates}]` for MCP `tools/list`.
pub fn catalog_json() -> Value {
    Value::Array(
        tools()
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "inputSchema": (t.schema)(), "mutates": t.mutates}))
            .collect(),
    )
}

/// Helper for schemas: an object with the given properties and required keys.
pub fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}
