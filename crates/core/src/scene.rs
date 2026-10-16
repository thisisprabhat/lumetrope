//! Layer time, visibility and transform resolution (including parent chains).

use crate::anim::{eval1, eval_or};
use crate::geom::Affine;
use crate::model::{Layer, Transform};

/// Time used for a layer's own property keyframes.
pub fn layer_time(l: &Layer, t: f64) -> f64 {
    t - l.st
}

pub fn is_visible(l: &Layer, t: f64) -> bool {
    !l.hd && t >= l.ip && t < l.op
}

/// Matrix of a Lottie transform block at time `t`.
pub fn transform_matrix(tr: &Transform, t: f64) -> Affine {
    let a = eval_or(tr.a.as_ref(), t, &[0.0, 0.0]);
    let p = if tr.px.is_some() || tr.py.is_some() {
        vec![eval1(tr.px.as_ref(), t, 0.0), eval1(tr.py.as_ref(), t, 0.0)]
    } else {
        eval_or(tr.p.as_ref(), t, &[0.0, 0.0])
    };
    let s = eval_or(tr.s.as_ref(), t, &[100.0, 100.0]);
    let g = |v: &[f64], i: usize, d: f64| v.get(i).copied().unwrap_or(d);
    Affine::from_lottie(
        [g(&a, 0, 0.0), g(&a, 1, 0.0)],
        [g(&p, 0, 0.0), g(&p, 1, 0.0)],
        [g(&s, 0, 100.0), g(&s, 1, g(&s, 0, 100.0))],
        eval1(tr.r.as_ref(), t, 0.0),
        eval1(tr.sk.as_ref(), t, 0.0),
        eval1(tr.sa.as_ref(), t, 0.0),
    )
}

/// Opacity of a transform block in 0..=1.
pub fn opacity(tr: &Transform, t: f64) -> f64 {
    (eval1(tr.o.as_ref(), t, 100.0) / 100.0).clamp(0.0, 1.0)
}

/// Full matrix of `layers[idx]` including its parent chain (cycle-safe).
pub fn layer_matrix(layers: &[Layer], idx: usize, t: f64) -> Affine {
    let mut m = transform_matrix(&layers[idx].ks, layer_time(&layers[idx], t));
    let mut cur = &layers[idx];
    for _ in 0..32 {
        let Some(pid) = cur.parent else { break };
        let Some(parent) = layers.iter().find(|l| l.ind == Some(pid)) else { break };
        m = transform_matrix(&parent.ks, layer_time(parent, t)).mul(&m);
        cur = parent;
    }
    m
}
