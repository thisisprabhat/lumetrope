//! Media export: animated GIF and WebP encoders and a WebM muxer, all keeping
//! transparency. Frames arrive as straight-alpha RGBA (what the renderers
//! produce); identical consecutive frames are merged into one longer frame.
//!
//! - GIF has 1-bit alpha: pixels under `alpha_threshold` use a reserved
//!   transparent palette index; soft edges are composited onto `matte`.
//! - WebP frames are lossless VP8L with a full alpha channel, wrapped in an
//!   animated container (VP8X + ANIM + ANMF).
//! - WebM: frames are encoded by the host (WebCodecs VP8/VP9); this muxer
//!   writes the Matroska container, storing an alpha stream as BlockAdditional
//!   (BlockAddID 1, AlphaMode 1) the way WebM players expect.

#[derive(Clone, Debug)]
pub struct RasterOptions {
    /// Keep transparency; otherwise flatten onto `background`.
    pub transparent: bool,
    pub background: [u8; 3],
    /// GIF only: colour that soft (partially transparent) edges are blended onto.
    pub matte: [u8; 3],
    /// GIF only: alpha below this becomes fully transparent.
    pub alpha_threshold: u8,
}

impl Default for RasterOptions {
    fn default() -> Self {
        RasterOptions { transparent: true, background: [255, 255, 255], matte: [255, 255, 255], alpha_threshold: 128 }
    }
}

fn over(c: u8, a: u8, bg: u8) -> u8 {
    ((c as u32 * a as u32 + bg as u32 * (255 - a as u32) + 127) / 255) as u8
}

/// Prepare straight-alpha RGBA for a format. `binary_alpha` = the format only has
/// fully transparent or opaque pixels (GIF).
pub fn prepare_rgba(px: &mut [u8], opts: &RasterOptions, binary_alpha: bool) {
    for p in px.chunks_exact_mut(4) {
        let a = p[3];
        if !opts.transparent {
            for k in 0..3 {
                p[k] = over(p[k], a, opts.background[k]);
            }
            p[3] = 255;
        } else if a == 0 || (binary_alpha && a < opts.alpha_threshold) {
            p.copy_from_slice(&[0, 0, 0, 0]);
        } else if binary_alpha {
            for k in 0..3 {
                p[k] = over(p[k], a, opts.matte[k]);
            }
            p[3] = 255;
        }
    }
}

/// Frame-accurate durations: frame i starts at round(i * unit / fps).
fn span(start: u64, count: u64, fps: f64, unit: f64) -> u32 {
    let at = |i: u64| (i as f64 * unit / fps).round() as u64;
    (at(start + count) - at(start)) as u32
}

/// Holds the last prepared frame until the next one shows whether it repeats.
struct Pending {
    px: Vec<u8>,
    count: u64,
}

pub struct GifEncoder {
    enc: gif::Encoder<Vec<u8>>,
    w: u16,
    h: u16,
    fps: f64,
    opts: RasterOptions,
    pending: Option<Pending>,
    written: u64,
}

impl GifEncoder {
    pub fn new(w: u16, h: u16, fps: f64, opts: RasterOptions) -> Result<Self, String> {
        if w == 0 || h == 0 || !(fps > 0.0) {
            return Err("GIF needs a non-zero size and frame rate".into());
        }
        let mut enc = gif::Encoder::new(Vec::new(), w, h, &[]).map_err(|e| e.to_string())?;
        enc.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
        Ok(GifEncoder { enc, w, h, fps, opts, pending: None, written: 0 })
    }

    pub fn push(&mut self, rgba: &[u8]) -> Result<(), String> {
        if rgba.len() != self.w as usize * self.h as usize * 4 {
            return Err("frame size does not match the GIF size".into());
        }
        let mut px = rgba.to_vec();
        prepare_rgba(&mut px, &self.opts, true);
        match &mut self.pending {
            Some(p) if p.px == px => p.count += 1,
            _ => {
                self.flush()?;
                self.pending = Some(Pending { px, count: 1 });
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        let Some(mut p) = self.pending.take() else { return Ok(()) };
        let mut frame = gif::Frame::from_rgba_speed(self.w, self.h, &mut p.px, 10);
        // Delays are in hundredths of a second.
        frame.delay = span(self.written, p.count, self.fps, 100.0).clamp(1, u16::MAX as u32) as u16;
        // Clear to transparent before each frame so earlier frames never show through.
        frame.dispose = gif::DisposalMethod::Background;
        self.enc.write_frame(&frame).map_err(|e| e.to_string())?;
        self.written += p.count;
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        self.flush()?;
        self.enc.into_inner().map_err(|e| e.to_string())
    }
}

pub struct WebpEncoder {
    w: u32,
    h: u32,
    fps: f64,
    opts: RasterOptions,
    pending: Option<Pending>,
    written: u64,
    /// (VP8L chunk incl. header and padding, duration ms)
    frames: Vec<(Vec<u8>, u32)>,
}

fn chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}

fn u24(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes()[..3]);
}

impl WebpEncoder {
    pub fn new(w: u32, h: u32, fps: f64, opts: RasterOptions) -> Self {
        WebpEncoder { w, h, fps, opts, pending: None, written: 0, frames: vec![] }
    }

    pub fn push(&mut self, rgba: &[u8]) -> Result<(), String> {
        if rgba.len() != self.w as usize * self.h as usize * 4 {
            return Err("frame size does not match the WebP size".into());
        }
        let mut px = rgba.to_vec();
        prepare_rgba(&mut px, &self.opts, false);
        match &mut self.pending {
            Some(p) if p.px == px => p.count += 1,
            _ => {
                self.flush()?;
                self.pending = Some(Pending { px, count: 1 });
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        let Some(p) = self.pending.take() else { return Ok(()) };
        let mut still = Vec::new();
        image_webp::WebPEncoder::new(&mut still).encode(&p.px, self.w, self.h, image_webp::ColorType::Rgba8).map_err(|e| e.to_string())?;
        // Simple container: "RIFF" size "WEBP" then the VP8L chunk.
        let vp8l = still.get(12..).ok_or("WebP encoder produced no image")?.to_vec();
        let ms = span(self.written, p.count, self.fps, 1000.0).clamp(1, 0xFF_FFFF);
        self.frames.push((vp8l, ms));
        self.written += p.count;
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        self.flush()?;
        let mut body = Vec::new();
        let mut vp8x = vec![0x02 | if self.opts.transparent { 0x10 } else { 0 }, 0, 0, 0];
        u24(&mut vp8x, self.w - 1);
        u24(&mut vp8x, self.h - 1);
        chunk(&mut body, b"VP8X", &vp8x);
        // Transparent background, loop forever.
        chunk(&mut body, b"ANIM", &[0, 0, 0, 0, 0, 0]);
        for (vp8l, ms) in &self.frames {
            let mut anmf = Vec::with_capacity(16 + vp8l.len());
            u24(&mut anmf, 0);
            u24(&mut anmf, 0);
            u24(&mut anmf, self.w - 1);
            u24(&mut anmf, self.h - 1);
            u24(&mut anmf, *ms);
            // Replace the canvas (no blending, no disposal): each frame carries its full alpha.
            anmf.push(0b10);
            anmf.extend_from_slice(vp8l);
            chunk(&mut body, b"ANMF", &anmf);
        }
        let mut out = Vec::with_capacity(body.len() + 12);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend(body);
        Ok(out)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Vp8,
    Vp9,
}

struct Block {
    ts_ms: i64,
    key: bool,
    data: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

pub struct WebmMuxer {
    w: u32,
    h: u32,
    codec: Codec,
    alpha: bool,
    blocks: Vec<Block>,
}

// ---- EBML writing ----

fn size_vint(n: u64) -> Vec<u8> {
    for len in 1..=8u32 {
        if n < (1u64 << (7 * len)) - 1 {
            let mut v = n | (1u64 << (7 * len));
            let mut out = vec![0; len as usize];
            for i in (0..len as usize).rev() {
                out[i] = v as u8;
                v >>= 8;
            }
            return out;
        }
    }
    panic!("EBML element too large");
}

fn el(out: &mut Vec<u8>, id: u32, payload: &[u8]) {
    let idb = id.to_be_bytes();
    let skip = idb.iter().take_while(|&&b| b == 0).count();
    out.extend_from_slice(&idb[skip..]);
    out.extend(size_vint(payload.len() as u64));
    out.extend_from_slice(payload);
}

fn uint_bytes(v: u64) -> Vec<u8> {
    let b = v.to_be_bytes();
    let skip = b.iter().take_while(|&&x| x == 0).count().min(7);
    b[skip..].to_vec()
}

fn el_uint(out: &mut Vec<u8>, id: u32, v: u64) {
    el(out, id, &uint_bytes(v));
}

fn el_int(out: &mut Vec<u8>, id: u32, v: i64) {
    // Minimal two's-complement big-endian.
    let b = v.to_be_bytes();
    let mut skip = 0;
    while skip < 7 && ((b[skip] == 0 && b[skip + 1] & 0x80 == 0) || (b[skip] == 0xFF && b[skip + 1] & 0x80 != 0)) {
        skip += 1;
    }
    el(out, id, &b[skip..]);
}

fn master(out: &mut Vec<u8>, id: u32, f: impl FnOnce(&mut Vec<u8>)) {
    let mut body = Vec::new();
    f(&mut body);
    el(out, id, &body);
}

impl WebmMuxer {
    pub fn new(w: u32, h: u32, codec: Codec, alpha: bool) -> Self {
        WebmMuxer { w, h, codec, alpha, blocks: vec![] }
    }

    /// Add one encoded frame (and, with alpha, the encoded alpha plane) at `ts_ms`.
    pub fn push(&mut self, data: &[u8], alpha: Option<&[u8]>, key: bool, ts_ms: f64) {
        self.blocks.push(Block { ts_ms: ts_ms.round() as i64, key, data: data.to_vec(), alpha: alpha.map(<[u8]>::to_vec) });
    }

    pub fn finish(self, duration_ms: f64) -> Vec<u8> {
        let mut out = Vec::new();
        master(&mut out, 0x1A45DFA3, |h| {
            el_uint(h, 0x4286, 1);
            el_uint(h, 0x42F7, 1);
            el_uint(h, 0x42F2, 4);
            el_uint(h, 0x42F3, 8);
            el(h, 0x4282, b"webm");
            el_uint(h, 0x4287, 4);
            el_uint(h, 0x4285, 2);
        });
        let codec_id: &[u8] = match self.codec {
            Codec::Vp8 => b"V_VP8",
            Codec::Vp9 => b"V_VP9",
        };
        master(&mut out, 0x18538067, |seg| {
            master(seg, 0x1549A966, |info| {
                el_uint(info, 0x2AD7B1, 1_000_000);
                el(info, 0x4D80, b"Lumetrope");
                el(info, 0x5741, b"Lumetrope");
                el(info, 0x4489, &duration_ms.to_be_bytes());
            });
            master(seg, 0x1654AE6B, |tracks| {
                master(tracks, 0xAE, |t| {
                    el_uint(t, 0xD7, 1);
                    el_uint(t, 0x73C5, 1);
                    el_uint(t, 0x9C, 0);
                    el(t, 0x86, codec_id);
                    el_uint(t, 0x83, 1);
                    if self.alpha {
                        el_uint(t, 0x55EE, 1); // MaxBlockAdditionID
                    }
                    master(t, 0xE0, |v| {
                        el_uint(v, 0xB0, self.w as u64);
                        el_uint(v, 0xBA, self.h as u64);
                        if self.alpha {
                            el_uint(v, 0x53C0, 1); // AlphaMode
                        }
                    });
                });
            });
            // A cluster starts at every keyframe (or when block offsets would overflow i16).
            let mut i = 0;
            while i < self.blocks.len() {
                let start = self.blocks[i].ts_ms;
                let mut j = i + 1;
                while j < self.blocks.len() && !self.blocks[j].key && self.blocks[j].ts_ms - start <= i16::MAX as i64 {
                    j += 1;
                }
                master(seg, 0x1F43B675, |c| {
                    el_uint(c, 0xE7, start.max(0) as u64);
                    for k in i..j {
                        let b = &self.blocks[k];
                        let rel = (b.ts_ms - start) as i16;
                        let mut block = vec![0x81];
                        block.extend_from_slice(&rel.to_be_bytes());
                        if self.alpha {
                            block.push(0);
                            block.extend_from_slice(&b.data);
                            master(c, 0xA0, |g| {
                                el(g, 0xA1, &block);
                                if let Some(a) = &b.alpha {
                                    master(g, 0x75A1, |adds| {
                                        master(adds, 0xA6, |more| {
                                            el_uint(more, 0xEE, 1);
                                            el(more, 0xA5, a);
                                        });
                                    });
                                }
                                if !b.key && k > 0 {
                                    el_int(g, 0xFB, self.blocks[k - 1].ts_ms - b.ts_ms);
                                }
                            });
                        } else {
                            block.push(if b.key { 0x80 } else { 0 });
                            block.extend_from_slice(&b.data);
                            el(c, 0xA3, &block);
                        }
                    }
                });
                i = j;
            }
        });
        out
    }
}
