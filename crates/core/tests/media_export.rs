//! GIF, animated WebP and WebM export: transparency, timing and container structure.

use lumetrope_core::media::{prepare_rgba, Codec, GifEncoder, RasterOptions, WebmMuxer, WebpEncoder};

const W: u16 = 4;
const H: u16 = 4;

/// 4×4 frame: left half fully transparent, right half `color` with alpha `a`.
fn frame(color: [u8; 3], a: u8) -> Vec<u8> {
    let mut v = Vec::new();
    for _y in 0..H {
        for x in 0..W {
            if x < W / 2 {
                v.extend_from_slice(&[9, 9, 9, 0]);
            } else {
                v.extend_from_slice(&[color[0], color[1], color[2], a]);
            }
        }
    }
    v
}

fn transparent_opts() -> RasterOptions {
    RasterOptions { transparent: true, background: [255, 255, 255], matte: [255, 255, 255], alpha_threshold: 128 }
}

#[test]
fn prepare_keeps_transparency_and_mattes_soft_edges() {
    let mut px = vec![10, 20, 30, 0, 200, 0, 0, 255, 0, 0, 200, 100, 0, 0, 200, 200];
    prepare_rgba(&mut px, &transparent_opts(), true);
    assert_eq!(&px[0..4], &[0, 0, 0, 0], "fully clear pixels are normalised");
    assert_eq!(&px[4..8], &[200, 0, 0, 255]);
    assert_eq!(&px[8..12], &[0, 0, 0, 0], "below the threshold → transparent in GIF");
    assert_eq!(&px[12..16], &[55, 55, 212, 255], "soft edge composited onto the white matte");
}

#[test]
fn prepare_keeps_partial_alpha_when_the_format_supports_it() {
    let mut px = vec![10, 20, 30, 0, 0, 0, 200, 100];
    prepare_rgba(&mut px, &transparent_opts(), false);
    assert_eq!(px, vec![0, 0, 0, 0, 0, 0, 200, 100]);
}

#[test]
fn prepare_flattens_onto_the_background_when_not_transparent() {
    let mut px = vec![10, 20, 30, 0, 0, 0, 200, 100];
    let opts = RasterOptions { transparent: false, background: [0, 255, 0], ..transparent_opts() };
    prepare_rgba(&mut px, &opts, false);
    assert_eq!(px, vec![0, 255, 0, 255, 0, 155, 78, 255]);
}

#[test]
fn gif_has_transparency_merged_duplicates_and_loops() {
    let mut enc = GifEncoder::new(W, H, 10.0, transparent_opts()).unwrap();
    enc.push(&frame([255, 0, 0], 255)).unwrap();
    enc.push(&frame([0, 0, 255], 255)).unwrap();
    enc.push(&frame([0, 0, 255], 255)).unwrap(); // identical → one frame shown twice as long
    let bytes = enc.finish().unwrap();

    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::Indexed);
    let mut dec = opts.read_info(bytes.as_slice()).unwrap();
    assert_eq!(dec.repeat(), gif::Repeat::Infinite);
    let mut delays = vec![];
    while let Some(f) = dec.read_next_frame().unwrap() {
        let t = f.transparent.expect("transparent index");
        assert_eq!(f.buffer[0], t, "left pixels are transparent");
        assert_ne!(f.buffer[3], t, "right pixels are opaque");
        assert_eq!(f.dispose, gif::DisposalMethod::Background);
        delays.push(f.delay);
    }
    assert_eq!(delays, vec![10, 20]);
}

#[test]
fn webp_is_animated_with_exact_alpha() {
    let mut enc = WebpEncoder::new(W as u32, H as u32, 25.0, transparent_opts());
    enc.push(&frame([255, 0, 0], 255)).unwrap();
    enc.push(&frame([0, 128, 255], 100)).unwrap();
    enc.push(&frame([0, 128, 255], 100)).unwrap();
    let bytes = enc.finish().unwrap();

    let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(dec.is_animated());
    assert!(dec.has_alpha());
    assert_eq!(dec.num_frames(), 2);
    let mut buf = vec![0; dec.output_buffer_size().unwrap()];
    assert_eq!(dec.read_frame(&mut buf).unwrap(), 40);
    assert_eq!(&buf[0..4], &[0, 0, 0, 0]);
    assert_eq!(&buf[8..12], &[255, 0, 0, 255]);
    assert_eq!(dec.read_frame(&mut buf).unwrap(), 80);
    assert_eq!(&buf[8..12], &[0, 128, 255, 100], "partial alpha survives losslessly");
}

// ---- minimal EBML reader for the WebM checks ----

fn vint(b: &[u8], i: &mut usize, keep_marker: bool) -> u64 {
    let first = b[*i];
    let len = first.leading_zeros() as usize + 1;
    let mut v = if keep_marker { first as u64 } else { (first as u64) & ((1u64 << (8 - len)) - 1) };
    for k in 1..len {
        v = (v << 8) | b[*i + k] as u64;
    }
    *i += len;
    v
}

/// Flat list of (id, payload) for every element, descending into the masters we care about.
fn walk(b: &[u8], out: &mut Vec<(u64, Vec<u8>)>) {
    const MASTERS: &[u64] = &[0x1A45DFA3, 0x18538067, 0x1549A966, 0x1654AE6B, 0xAE, 0xE0, 0x1F43B675, 0xA0, 0x75A1, 0xA6];
    let mut i = 0;
    while i < b.len() {
        let id = vint(b, &mut i, true);
        let size = vint(b, &mut i, false) as usize;
        let body = &b[i..i + size];
        out.push((id, body.to_vec()));
        if MASTERS.contains(&id) {
            walk(body, out);
        }
        i += size;
    }
}

fn uint(b: &[u8]) -> u64 {
    b.iter().fold(0, |a, &x| (a << 8) | x as u64)
}

#[test]
fn webm_carries_vp9_with_an_alpha_stream() {
    let mut mux = WebmMuxer::new(320, 240, Codec::Vp9, true);
    mux.push(&[1, 2, 3], Some(&[7, 7]), true, 0.0);
    mux.push(&[4, 5], Some(&[8]), false, 40.0);
    mux.push(&[6], Some(&[9, 9, 9]), false, 80.0);
    let bytes = mux.finish(120.0);

    let mut els = vec![];
    walk(&bytes, &mut els);
    let find = |id: u64| els.iter().filter(move |(i, _)| *i == id).map(|(_, b)| b.clone());
    assert_eq!(find(0x4282).next().unwrap(), b"webm");
    assert_eq!(find(0x86).next().unwrap(), b"V_VP9");
    assert_eq!(uint(&find(0xB0).next().unwrap()), 320);
    assert_eq!(uint(&find(0xBA).next().unwrap()), 240);
    assert_eq!(uint(&find(0x53C0).next().unwrap()), 1, "AlphaMode");
    assert_eq!(uint(&find(0x2AD7B1).next().unwrap()), 1_000_000, "millisecond timecodes");
    assert_eq!(f64::from_be_bytes(find(0x4489).next().unwrap().try_into().unwrap()), 120.0);

    let blocks: Vec<Vec<u8>> = find(0xA1).collect();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0], vec![0x81, 0, 0, 0, 1, 2, 3], "track 1, +0 ms, frame data");
    assert_eq!(blocks[1], vec![0x81, 0, 40, 0, 4, 5]);
    let alphas: Vec<Vec<u8>> = find(0xA5).collect();
    assert_eq!(alphas, vec![vec![7, 7], vec![8], vec![9, 9, 9]]);
    assert!(find(0xEE).all(|id| uint(&id) == 1), "BlockAddID 1 = alpha");
    assert_eq!(find(0xFB).count(), 2, "only delta frames reference an earlier block");
}

#[test]
fn webm_without_alpha_uses_simple_blocks() {
    let mut mux = WebmMuxer::new(16, 16, Codec::Vp8, false);
    mux.push(&[1], None, true, 0.0);
    mux.push(&[2], None, false, 33.0);
    let bytes = mux.finish(66.0);
    let mut els = vec![];
    walk(&bytes, &mut els);
    assert!(els.iter().any(|(id, b)| *id == 0x86 && b == b"V_VP8"));
    assert!(!els.iter().any(|(id, _)| *id == 0x53C0), "no AlphaMode");
    let simple: Vec<_> = els.iter().filter(|(id, _)| *id == 0xA3).map(|(_, b)| b.clone()).collect();
    assert_eq!(simple, vec![vec![0x81, 0, 0, 0x80, 1], vec![0x81, 0, 33, 0, 2]], "keyframe flag only on the first");
}
