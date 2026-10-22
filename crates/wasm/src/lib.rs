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

    /// GPU display list JSON for the Canvas2D executor.
    pub fn display_list(&mut self, frame: f64, scale: f64, tx: f64, ty: f64) -> String {
        self.inner.display_list(frame, scale, tx, ty)
    }

    /// `{asset id: data URI}` for embedded images.
    pub fn image_assets(&mut self) -> String {
        self.inner.image_assets().to_string()
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

    /// Start previewing a state machine; returns the snapshot JSON.
    pub fn sm_start(&mut self, id: &str) -> Result<String, JsValue> {
        self.inner.sm_start(id).map(|v| v.to_string()).map_err(err)
    }

    pub fn sm_stop(&mut self) {
        self.inner.sm_stop();
    }

    /// Advance the running machine by `dt` seconds; returns snapshot + events JSON.
    pub fn sm_tick(&mut self, dt: f64) -> String {
        self.inner.sm_tick(dt).to_string()
    }

    /// Pointer event ("down" | "up" | "move" | "enter" | "exit" | "click") at a composition point.
    pub fn sm_pointer(&mut self, kind: &str, x: f64, y: f64) -> String {
        self.inner.sm_pointer(kind, x, y).to_string()
    }

    /// Set an input (JSON value) or fire an event (null).
    pub fn sm_set_input(&mut self, name: &str, value_json: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(value_json).unwrap_or(serde_json::Value::Null);
        self.inner.sm_set_input(name, &v).to_string()
    }

    pub fn sm_snapshot(&mut self) -> String {
        self.inner.sm_snapshot().to_string()
    }

    /// Commit the current project; returns the version id.
    pub fn vcs_commit(&mut self, message: &str, author: &str, ts: f64) -> Result<String, JsValue> {
        self.inner.vcs_commit(message, author, ts).map_err(err)
    }

    pub fn vcs_log(&self) -> String {
        self.inner.vcs_log().to_string()
    }

    pub fn vcs_restore(&mut self, id: &str, ts: f64) -> Result<(), JsValue> {
        self.inner.vcs_restore(id, ts).map_err(err)
    }

    pub fn vcs_branches(&self) -> String {
        self.inner.vcs_branches().to_string()
    }

    pub fn vcs_create_branch(&mut self, name: &str) -> Result<(), JsValue> {
        self.inner.vcs_create_branch(name).map_err(err)
    }

    pub fn vcs_switch(&mut self, name: &str, ts: f64) -> Result<(), JsValue> {
        self.inner.vcs_switch(name, ts).map_err(err)
    }

    pub fn vcs_diff(&self, a: &str, b: &str) -> Result<String, JsValue> {
        self.inner.vcs_diff(a, b).map(|v| v.to_string()).map_err(err)
    }

    pub fn vcs_status(&self) -> String {
        self.inner.vcs_status().to_string()
    }

    pub fn vcs_export(&self) -> Vec<u8> {
        self.inner.vcs_export()
    }

    pub fn vcs_import(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        self.inner.vcs_import(bytes).map_err(err)
    }

    pub fn vcs_reset(&mut self) {
        self.inner.vcs_reset();
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

// ---- media export (GIF / animated WebP / WebM container) ----

use lumetrope_core::media;

fn raster_opts(transparent: bool, background: &[u8], matte: &[u8], alpha_threshold: u8) -> media::RasterOptions {
    let rgb = |c: &[u8], d: [u8; 3]| if c.len() >= 3 { [c[0], c[1], c[2]] } else { d };
    media::RasterOptions { transparent, background: rgb(background, [255, 255, 255]), matte: rgb(matte, [255, 255, 255]), alpha_threshold }
}

/// Streams straight-alpha RGBA frames into an animated GIF.
#[wasm_bindgen]
pub struct GifExporter {
    inner: media::GifEncoder,
}

#[wasm_bindgen]
impl GifExporter {
    #[wasm_bindgen(constructor)]
    pub fn new(w: u16, h: u16, fps: f64, transparent: bool, background: &[u8], matte: &[u8], alpha_threshold: u8) -> Result<GifExporter, JsValue> {
        media::GifEncoder::new(w, h, fps, raster_opts(transparent, background, matte, alpha_threshold)).map(|inner| GifExporter { inner }).map_err(err)
    }

    pub fn push(&mut self, rgba: &[u8]) -> Result<(), JsValue> {
        self.inner.push(rgba).map_err(err)
    }

    pub fn finish(self) -> Result<Vec<u8>, JsValue> {
        self.inner.finish().map_err(err)
    }
}

/// Streams straight-alpha RGBA frames into a lossless animated WebP.
#[wasm_bindgen]
pub struct WebpExporter {
    inner: media::WebpEncoder,
}

#[wasm_bindgen]
impl WebpExporter {
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32, fps: f64, transparent: bool, background: &[u8]) -> WebpExporter {
        WebpExporter { inner: media::WebpEncoder::new(w, h, fps, raster_opts(transparent, background, &[], 128)) }
    }

    pub fn push(&mut self, rgba: &[u8]) -> Result<(), JsValue> {
        self.inner.push(rgba).map_err(err)
    }

    pub fn finish(self) -> Result<Vec<u8>, JsValue> {
        self.inner.finish().map_err(err)
    }
}

/// Writes WebCodecs-encoded VP8/VP9 frames (plus an optional alpha stream) into WebM.
#[wasm_bindgen]
pub struct WebmWriter {
    inner: media::WebmMuxer,
}

#[wasm_bindgen]
impl WebmWriter {
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32, vp9: bool, alpha: bool) -> WebmWriter {
        WebmWriter { inner: media::WebmMuxer::new(w, h, if vp9 { media::Codec::Vp9 } else { media::Codec::Vp8 }, alpha) }
    }

    /// `alpha` is empty when the file has no alpha stream.
    pub fn push(&mut self, data: &[u8], alpha: &[u8], key: bool, ts_ms: f64) {
        self.inner.push(data, if alpha.is_empty() { None } else { Some(alpha) }, key, ts_ms);
    }

    pub fn finish(self, duration_ms: f64) -> Vec<u8> {
        self.inner.finish(duration_ms)
    }
}
