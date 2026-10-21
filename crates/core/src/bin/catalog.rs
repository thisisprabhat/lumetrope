//! Prints the tool catalogue as JSON (consumed by the lumetrope-mcp package).
fn main() {
    println!("{}", serde_json::to_string_pretty(&lumetrope_core::ops::catalog::catalog_json()).unwrap());
}
