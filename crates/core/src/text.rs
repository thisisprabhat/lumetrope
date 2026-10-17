//! Text layout: font registry, glyph outlines (ttf-parser) and Lottie text
//! document layout (justify, tracking, line height, box wrapping).

use std::sync::Arc;

use crate::geom::{Contour, Path};
use crate::model::TextDoc;

static DEFAULT_FONT: &[u8] = include_bytes!("../assets/Poppins-Regular.ttf");

/// Fonts available to the renderer. Always contains the bundled default.
#[derive(Clone, Default)]
pub struct FontRegistry {
    fonts: Vec<(String, Arc<Vec<u8>>)>,
}

impl FontRegistry {
    /// Register font bytes under `name` (matched case-insensitively against a
    /// text document's font name). Returns false if the bytes are not a font.
    pub fn add(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        if ttf_parser::Face::parse(&bytes, 0).is_err() {
            return false;
        }
        let key = name.to_lowercase();
        self.fonts.retain(|(n, _)| *n != key);
        self.fonts.push((key, Arc::new(bytes)));
        true
    }

    pub fn names(&self) -> Vec<String> {
        self.fonts.iter().map(|(n, _)| n.clone()).collect()
    }

    /// Bytes for `name`, falling back to the bundled font.
    pub fn bytes(&self, name: &str) -> &[u8] {
        let key = name.to_lowercase();
        self.fonts
            .iter()
            .find(|(n, _)| *n == key || (!key.is_empty() && (n.starts_with(&key) || key.starts_with(n.as_str()))))
            .map(|(_, b)| b.as_slice())
            .unwrap_or(DEFAULT_FONT)
    }
}

struct Outline {
    contours: Vec<Contour>,
    cur: Contour,
    scale: f64,
    ox: f64,
    oy: f64,
}

impl Outline {
    fn pt(&self, x: f32, y: f32) -> [f64; 2] {
        [self.ox + x as f64 * self.scale, self.oy - y as f64 * self.scale]
    }
    fn flush(&mut self) {
        if !self.cur.v.is_empty() {
            self.contours.push(std::mem::take(&mut self.cur));
        }
    }
    fn last(&self) -> [f64; 2] {
        *self.cur.v.last().unwrap_or(&[self.ox, self.oy])
    }
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        let p = self.pt(x, y);
        self.cur.v.push(p);
        self.cur.i.push([0.0, 0.0]);
        self.cur.o.push([0.0, 0.0]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.cur.v.push(p);
        self.cur.i.push([0.0, 0.0]);
        self.cur.o.push([0.0, 0.0]);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = self.last();
        let q = self.pt(x1, y1);
        let p = self.pt(x, y);
        let c1 = [p0[0] + 2.0 / 3.0 * (q[0] - p0[0]), p0[1] + 2.0 / 3.0 * (q[1] - p0[1])];
        let c2 = [p[0] + 2.0 / 3.0 * (q[0] - p[0]), p[1] + 2.0 / 3.0 * (q[1] - p[1])];
        self.cubic(p0, c1, c2, p);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = self.last();
        let (c1, c2, p) = (self.pt(x1, y1), self.pt(x2, y2), self.pt(x, y));
        self.cubic(p0, c1, c2, p);
    }
    fn close(&mut self) {
        let n = self.cur.v.len();
        if n > 1 {
            let (a, b) = (self.cur.v[0], self.cur.v[n - 1]);
            if (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6 {
                self.cur.i[0] = self.cur.i[n - 1];
                self.cur.v.pop();
                self.cur.i.pop();
                self.cur.o.pop();
            }
        }
        self.cur.closed = true;
        self.flush();
    }
}

impl Outline {
    fn cubic(&mut self, p0: [f64; 2], c1: [f64; 2], c2: [f64; 2], p: [f64; 2]) {
        if let Some(o) = self.cur.o.last_mut() {
            *o = [c1[0] - p0[0], c1[1] - p0[1]];
        }
        self.cur.v.push(p);
        self.cur.i.push([c2[0] - p[0], c2[1] - p[1]]);
        self.cur.o.push([0.0, 0.0]);
    }
}

fn apply_caps(s: &str, caps: i64) -> String {
    if caps != 0 {
        s.to_uppercase()
    } else {
        s.to_string()
    }
}

/// Lay out a text document; returns one path per glyph in layer space
/// (origin at the first baseline for point text, box top-left for box text).
pub fn layout(doc: &TextDoc, fonts: &FontRegistry) -> Vec<Path> {
    let bytes = fonts.bytes(&doc.font);
    let face = match ttf_parser::Face::parse(bytes, 0) {
        Ok(f) => f,
        Err(_) => match ttf_parser::Face::parse(DEFAULT_FONT, 0) {
            Ok(f) => f,
            Err(_) => return vec![],
        },
    };
    let upem = face.units_per_em() as f64;
    let scale = doc.size / upem;
    let tracking = doc.tracking / 1000.0 * doc.size;
    let line_h = if doc.line_height > 0.0 { doc.line_height } else { doc.size * 1.2 };
    let text = apply_caps(&doc.text, doc.caps);
    let advance = |c: char| -> f64 {
        face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g)).map(|a| a as f64 * scale).unwrap_or(doc.size * 0.5) + tracking
    };

    // Split into lines (explicit breaks, then optional box wrapping).
    let mut lines: Vec<String> = vec![];
    for raw in text.split(['\r', '\n', '\u{3}']) {
        match doc.box_size {
            Some(sz) if sz[0] > 0.0 => {
                let mut cur = String::new();
                let mut w = 0.0;
                for word in raw.split_inclusive(' ') {
                    let ww: f64 = word.chars().map(advance).sum();
                    if w + ww > sz[0] && !cur.is_empty() {
                        lines.push(std::mem::take(&mut cur));
                        w = 0.0;
                    }
                    cur.push_str(word);
                    w += ww;
                }
                lines.push(cur);
            }
            _ => lines.push(raw.to_string()),
        }
    }

    let (box_x, box_w, first_baseline) = match (doc.box_size, doc.box_pos) {
        (Some(sz), pos) => {
            let p = pos.unwrap_or([0.0, 0.0]);
            (p[0], sz[0], p[1] + face.ascender() as f64 * scale)
        }
        (None, _) => (0.0, 0.0, 0.0),
    };

    let mut out = vec![];
    for (n, line) in lines.iter().enumerate() {
        let width: f64 = line.trim_end().chars().map(advance).sum::<f64>() - if line.is_empty() { 0.0 } else { tracking };
        let start = match (doc.justify, doc.box_size.is_some()) {
            (1, false) => -width,
            (2, false) => -width / 2.0,
            (1, true) => box_x + box_w - width,
            (2, true) => box_x + (box_w - width) / 2.0,
            (_, true) => box_x,
            _ => 0.0,
        };
        let y = first_baseline + n as f64 * line_h - doc.baseline;
        let mut x = start;
        for c in line.chars() {
            if let Some(g) = face.glyph_index(c) {
                let mut ob = Outline { contours: vec![], cur: Contour::default(), scale, ox: x, oy: y };
                if face.outline_glyph(g, &mut ob).is_some() {
                    ob.flush();
                    if !ob.contours.is_empty() {
                        out.push(Path { contours: ob.contours });
                    }
                }
            }
            x += advance(c);
        }
    }
    out
}
