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
    /// Composition shown on the canvas: "root" or a precomp asset id.
    view: String,
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
            view: "root".into(),
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
        self.view = "root".into();
        self.project = p;
        self.ctx.images.clear();
        self.register_project_fonts();
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

    /// Merge the following tool calls into one undo step (e.g. a canvas drag).
    pub fn begin_batch(&mut self) {
        self.hist.begin_batch("batch");
    }

    pub fn end_batch(&mut self) {
        self.hist.end_batch();
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
            let mut src = project::active_anim(&self.project).clone();
            let theme = self.project.pointer("/settings/preview_theme").and_then(Value::as_str).and_then(|t| self.project.get("themes").and_then(|th| th.get(t))).cloned();
            crate::dotlottie::apply_theme(&mut src, theme.as_ref(), &project::active_id(&self.project));
            let a = model::parse(&src).unwrap_or_default();
            self.cache = Some((self.revision, a));
        }
        &self.cache.as_ref().unwrap().1
    }

    /// Show a nested scene (precomp) on the canvas instead of the root; unknown ids select root.
    pub fn set_view_scene(&mut self, id: &str) {
        let ok = id != "root" && self.animation().asset(id).map(|a| a.layers.is_some()).unwrap_or(false);
        self.view = if ok { id.to_string() } else { "root".into() };
    }

    pub fn view_scene(&self) -> &str {
        &self.view
    }

    /// The animation as seen through the current view scene.
    pub fn view_animation(&mut self) -> Animation {
        let view = self.view.clone();
        let base = self.animation().clone();
        if view == "root" {
            return base;
        }
        let Some(asset) = base.asset(&view).cloned() else { return base };
        let holder = base.layers.iter().find(|l| l.ref_id.as_deref() == Some(view.as_str()));
        let w = if asset.w > 0.0 { asset.w } else { holder.map(|l| l.w).filter(|w| *w > 0.0).unwrap_or(base.w) };
        let h = if asset.h > 0.0 { asset.h } else { holder.map(|l| l.h).filter(|h| *h > 0.0).unwrap_or(base.h) };
        Animation { layers: asset.layers.unwrap_or_default(), w, h, ..base }
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
        let view = self.view.clone();
        let a = &self.view_animation();
        json!({
            "view": view,
            "w": a.w, "h": a.h, "fr": a.fr, "ip": a.ip, "op": a.op, "name": a.nm,
            "layers": a.layers.len(), "animations": ids, "active": active,
            "markers": a.markers.iter().map(|m| json!({"name": m.name, "time": m.time, "duration": m.duration})).collect::<Vec<_>>(),
            "revision": self.revision,
        })
    }

    /// Render the active animation to straight-alpha RGBA (as `ImageData` expects).
    pub fn render_rgba(&mut self, frame: f64, w: u32, h: u32) -> Vec<u8> {
        self.sync_images();
        let anim = self.view_animation();
        let pm = render::render(&anim, &self.ctx, frame, w, h);
        let mut out = pm.take();
        unpremultiply(&mut out);
        out
    }

    /// Render through a camera into a viewport-sized straight-alpha RGBA buffer:
    /// screen = comp × scale + (tx, ty). Keeps zoomed views crisp and cheap.
    pub fn render_view_rgba(&mut self, frame: f64, w: u32, h: u32, scale: f64, tx: f64, ty: f64) -> Vec<u8> {
        self.sync_images();
        let anim = self.view_animation();
        let pm = render::render_view(&anim, &self.ctx, frame, w, h, scale, tx, ty);
        let mut out = pm.take();
        unpremultiply(&mut out);
        out
    }

    /// GPU display list (JSON) through a camera: screen = comp × scale + (tx, ty).
    pub fn display_list(&mut self, frame: f64, scale: f64, tx: f64, ty: f64) -> String {
        let anim = self.view_animation();
        let base = crate::geom::Affine { a: scale, b: 0.0, c: 0.0, d: scale, e: tx, f: ty };
        crate::render::list::record(&anim, &self.ctx.fonts, frame, &base).to_string()
    }

    /// `{asset id: data URI}` for every embedded image of the viewed animation.
    pub fn image_assets(&mut self) -> Value {
        let anim = self.view_animation();
        let mut out = serde_json::Map::new();
        for a in &anim.assets {
            if a.layers.is_none() && a.p.starts_with("data:") {
                out.insert(a.id.clone(), json!(a.p));
            }
        }
        Value::Object(out)
    }

    /// The active animation as clean Lottie JSON (editor metadata stripped).
    pub fn export_lottie(&self) -> String {
        let mut a = project::active_anim(&self.project).clone();
        project::strip_editor_meta(&mut a);
        a.to_string()
    }

    /// Topmost root layer id under a composition-space point.
    pub fn hit_test(&mut self, x: f64, y: f64, frame: f64) -> Option<String> {
        let anim = self.view_animation();
        crate::hit::hit_test_in(&anim, &self.ctx.fonts, &self.view, frame, x, y)
    }

    /// Geometry JSON for a layer id, or null if not found.
    pub fn layer_geometry(&mut self, id: &str, frame: f64) -> Value {
        let anim = self.animation().clone();
        match crate::hit::layer_geometry(&anim, &self.ctx.fonts, id, frame) {
            Some(g) => json!({
                "id": g.id, "local": g.local, "bounds": g.bounds, "corners": g.corners,
                "matrix": g.matrix, "anchor": g.anchor
            }),
            None => Value::Null,
        }
    }

    pub fn fonts(&self) -> &crate::text::FontRegistry {
        &self.ctx.fonts
    }

    /// Register a font for rendering and embed it in the project (exported to `f/`).
    pub fn add_font(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        if !self.ctx.fonts.add(name, bytes) {
            return false;
        }
        if let Some(f) = self.project.get_mut("fonts").and_then(Value::as_object_mut) {
            f.insert(name.to_string(), json!({"file": format!("{name}.ttf"), "data": data}));
        }
        self.touch();
        true
    }

    fn register_project_fonts(&mut self) {
        use base64::Engine as _;
        let fonts = self.project.get("fonts").and_then(Value::as_object).cloned().unwrap_or_default();
        for (name, f) in fonts {
            if let Some(b) = f.get("data").and_then(Value::as_str).and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()) {
                self.ctx.fonts.add(&name, b);
            }
        }
    }

    /// The whole project as a dotLottie v2 archive.
    pub fn export_dotlottie(&self) -> Result<Vec<u8>, String> {
        crate::dotlottie::write(&self.project)
    }

    pub fn add_image(&mut self, id: &str, bytes: &[u8]) -> bool {
        self.ctx.add_image(id, bytes)
    }
}

fn unpremultiply(px: &mut [u8]) {
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in p.iter_mut().take(3) {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}
