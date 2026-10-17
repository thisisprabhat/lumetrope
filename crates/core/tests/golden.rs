//! Golden-image regression tests. Run with `UPDATE_GOLDEN=1` to (re)write the PNGs.
use lumetrope_core::model;
use lumetrope_core::render::{render, RenderCtx};
use std::path::PathBuf;

const SAMPLES: &[&str] = &["shapes", "gradients", "masks_mattes", "precomp", "text", "trim_repeater"];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

#[test]
fn golden_renders_match() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let mut failures = vec![];
    for name in SAMPLES {
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(dir().join(format!("{name}.json"))).unwrap()).unwrap();
        let a = model::parse(&json).unwrap();
        let ctx = RenderCtx::from_animation(&a);
        for frame in [0.0, ((a.op - a.ip) / 2.0).floor()] {
            let pm = render(&a, &ctx, frame, a.w as u32, a.h as u32);
            let path = dir().join(format!("{name}_{frame}.png"));
            if update {
                pm.save_png(&path).unwrap();
                continue;
            }
            let Ok(expected) = tiny_skia::Pixmap::load_png(&path) else {
                failures.push(format!("{name}@{frame}: missing golden {}", path.display()));
                continue;
            };
            let total = (pm.width() * pm.height()) as usize;
            let bad = pm
                .data()
                .chunks_exact(4)
                .zip(expected.data().chunks_exact(4))
                .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| (*x as i32 - *y as i32).abs() > 8))
                .count();
            if expected.width() != pm.width() || bad * 100 > total {
                failures.push(format!("{name}@{frame}: {bad}/{total} pixels differ"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
