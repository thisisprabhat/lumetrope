//! wasm-bindgen facade over `lumetrope_core::engine::Engine`.
//! Structured values cross the boundary as JSON strings.

use lumetrope_core::engine;
use wasm_bindgen::prelude::*;

fn err(e: String) -> JsValue {
    JsValue::from_str(&e)
}

#[wasm_bindgen]
pub struct Engine {
    inner: engine::Engine,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Engine {
        Engine { inner: engine::Engine::new() }
    }

    /// Load a `.json` (Lottie or project) or `.lottie` file. Returns info JSON.
    pub fn load_file(&mut self, name: &str, bytes: &[u8]) -> Result<String, JsValue> {
        self.inner.load_file(name, bytes).map(|v| v.to_string()).map_err(err)
    }

    pub fn new_project(&mut self, w: f64, h: f64, fr: f64, frames: f64) {
        self.inner.new_project(w, h, fr, frames);
    }

    pub fn info(&mut self) -> String {
        self.inner.info().to_string()
    }

    /// Straight-alpha RGBA pixels of the active animation at `frame`.
    pub fn render(&mut self, frame: f64, w: u32, h: u32) -> Vec<u8> {
        self.inner.render_rgba(frame, w, h)
    }

    /// Viewport render through a camera: screen = comp × scale + (tx, ty).
    pub fn render_view(&mut self, frame: f64, w: u32, h: u32, scale: f64, tx: f64, ty: f64) -> Vec<u8> {
        self.inner.render_view_rgba(frame, w, h, scale, tx, ty)
    }

    pub fn project_json(&self) -> String {
        self.inner.project_json()
    }

    pub fn set_project_json(&mut self, s: &str) -> Result<(), JsValue> {
        self.inner.set_project_json(s).map_err(err)
    }

    pub fn export_lottie(&self) -> String {
        self.inner.export_lottie()
    }

    /// The project as a dotLottie v2 archive (animations, themes, state machines, images, fonts).
    pub fn export_dotlottie(&self) -> Result<Vec<u8>, JsValue> {
        self.inner.export_dotlottie().map_err(err)
    }

    pub fn add_font(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        self.inner.add_font(name, bytes)
    }

    /// Run a catalogue tool with JSON args; returns `{"ok":…}` or `{"error":…}` JSON.
    pub fn apply(&mut self, tool: &str, args_json: &str) -> String {
        let args: serde_json::Value = match serde_json::from_str(args_json) {
            Ok(v) => v,
            Err(e) => return serde_json::json!({"error": format!("invalid JSON args: {e}")}).to_string(),
        };
        self.inner.apply(tool, &args).to_string()
    }

    pub fn catalog(&self) -> String {
        self.inner.catalog().to_string()
    }

    pub fn begin_batch(&mut self) {
        self.inner.begin_batch();
    }

    pub fn end_batch(&mut self) {
        self.inner.end_batch();
    }

    pub fn undo(&mut self) -> bool {
        self.inner.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.inner.redo()
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    /// Topmost root layer id at a composition-space point ("" if none).
    pub fn hit_test(&mut self, x: f64, y: f64, frame: f64) -> String {
        self.inner.hit_test(x, y, frame).unwrap_or_default()
    }

    /// Geometry JSON (bounds, corners, matrix, anchor) for a layer id, or "null".
    pub fn layer_geometry(&mut self, id: &str, frame: f64) -> String {
        self.inner.layer_geometry(id, frame).to_string()
    }

    /// Show a nested scene on the canvas ("root" for the main scene).
    pub fn set_view_scene(&mut self, id: &str) {
        self.inner.set_view_scene(id);
    }

    /// Names of fonts registered with add_font.
    pub fn font_names(&self) -> String {
        serde_json::to_string(&self.inner.fonts().names()).unwrap_or_default()
    }

    pub fn revision(&self) -> f64 {
        self.inner.revision() as f64
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
