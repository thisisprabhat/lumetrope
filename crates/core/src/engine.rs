//! The editor engine: owns the project document, a parsed-model cache and
//! render resources. The wasm crate is a thin binding over this type.

use serde_json::{json, Value};

use crate::model::{self, Animation};
use crate::ops::{self, History};
use crate::project;
use crate::render::{self, RenderCtx};

pub struct Engine {
    project: Value,
    revision: u64,
    cache: Option<(u64, Animation)>,
    ctx: RenderCtx,
    images_rev: u64,
    hist: History,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            project: project::from_lottie(project::blank_lottie(512.0, 512.0, 30.0, 90.0)),
            revision: 1,
            cache: None,
            ctx: RenderCtx::default(),
            images_rev: 0,
            hist: History::default(),
        }
    }

    pub fn project(&self) -> &Value {
        &self.project
    }

    /// Mutable access for the ops layer; call [`Engine::touch`] afterwards.
    pub fn project_mut(&mut self) -> &mut Value {
        &mut self.project
    }

    /// Mark the document changed.
    pub fn touch(&mut self) {
        self.revision += 1;
        self.cache = None;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn replace_project(&mut self, p: Value) {
        self.project = p;
        self.ctx.images.clear();
        self.hist.clear();
        self.touch();
    }

    /// Run a catalogue tool. Returns `{"ok": result}` or `{"error": message}`.
    pub fn apply(&mut self, tool: &str, args: &Value) -> Value {
        match ops::apply(&mut self.project, &mut self.hist, tool, args) {
            Ok(v) => {
                if ops::catalog::find(tool).map(|t| t.mutates).unwrap_or(false) {
                    self.touch();
                }
                json!({ "ok": v })
            }
            Err(e) => json!({ "error": e.0 }),
        }
    }

    pub fn catalog(&self) -> Value {
        ops::catalog::catalog_json()
    }

    pub fn undo(&mut self) -> bool {
        let ok = self.hist.undo(&mut self.project).is_some();
        if ok {
            self.touch();
        }
        ok
    }

    pub fn redo(&mut self) -> bool {
        let ok = self.hist.redo(&mut self.project).is_some();
        if ok {
            self.touch();
        }
        ok
    }

    pub fn can_undo(&self) -> bool {
        self.hist.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.hist.can_redo()
    }

    pub fn new_project(&mut self, w: f64, h: f64, fr: f64, frames: f64) {
        self.replace_project(project::from_lottie(project::blank_lottie(w, h, fr, frames)));
    }

    /// Load a file by name; returns [`Engine::info`] on success.
    pub fn load_file(&mut self, name: &str, bytes: &[u8]) -> Result<Value, String> {
        let lower = name.to_lowercase();
        let p = if lower.ends_with(".json") {
            project::from_json_bytes(bytes)?
        } else if lower.ends_with(".lottie") {
            crate::dotlottie::read(bytes)?
        } else {
            return Err(format!("unsupported file type: {name}"));
        };
        self.replace_project(p);
        // Validate that the active animation parses.
        model::parse(project::active_anim(&self.project)).map_err(|e| e.to_string())?;
        Ok(self.info())
    }

    pub fn project_json(&self) -> String {
        self.project.to_string()
    }

    pub fn set_project_json(&mut self, s: &str) -> Result<(), String> {
        let v: Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let p = project::normalize(v)?;
        self.replace_project(p);
        Ok(())
    }

    /// Restore a project without clearing caches of unrelated resources (undo/redo, VCS).
    pub fn set_project_value(&mut self, p: Value) {
        self.project = p;
        self.touch();
    }

    /// Parsed model of the active animation (cached per revision).
    pub fn animation(&mut self) -> &Animation {
        if self.cache.as_ref().map(|c| c.0) != Some(self.revision) {
            let a = model::parse(project::active_anim(&self.project)).unwrap_or_default();
            self.cache = Some((self.revision, a));
        }
        &self.cache.as_ref().unwrap().1
    }

    fn sync_images(&mut self) {
        if self.images_rev == self.revision {
            return;
        }
        self.images_rev = self.revision;
        let anim = self.animation().clone();
        self.ctx.load_embedded(&anim);
    }

    pub fn info(&mut self) -> Value {
        let ids: Vec<String> = self.project["animations"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        let active = project::active_id(&self.project);
        let a = self.animation();
        json!({
            "w": a.w, "h": a.h, "fr": a.fr, "ip": a.ip, "op": a.op, "name": a.nm,
            "layers": a.layers.len(), "animations": ids, "active": active,
            "markers": a.markers.iter().map(|m| json!({"name": m.name, "time": m.time, "duration": m.duration})).collect::<Vec<_>>(),
            "revision": self.revision,
        })
    }

    /// Render the active animation to straight-alpha RGBA (as `ImageData` expects).
    pub fn render_rgba(&mut self, frame: f64, w: u32, h: u32) -> Vec<u8> {
        self.sync_images();
        let anim = self.animation().clone();
        let pm = render::render(&anim, &self.ctx, frame, w, h);
        let mut out = pm.take();
        for px in out.chunks_exact_mut(4) {
            let a = px[3] as u32;
            if a > 0 && a < 255 {
                for c in px.iter_mut().take(3) {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        out
    }

    /// The active animation as clean Lottie JSON (editor metadata stripped).
    pub fn export_lottie(&self) -> String {
        let mut a = project::active_anim(&self.project).clone();
        project::strip_editor_meta(&mut a);
        a.to_string()
    }

    pub fn add_font(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        self.ctx.fonts.add(name, bytes)
    }

    pub fn add_image(&mut self, id: &str, bytes: &[u8]) -> bool {
        self.ctx.add_image(id, bytes)
    }
}
