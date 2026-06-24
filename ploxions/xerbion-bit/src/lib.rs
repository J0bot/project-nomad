//! `xerbion-bit` ploxion — **le xerbion TERNAIRE** (BitNet b1.58, 1.58-bit).
//!
//! Même être que `xerbion` (MLP 4→6→1 prédictif), mais les **poids de la passe avant
//! sont quantifiés en ternaire {−1, 0, +1}** (quantif **absmean** de BitNet : `scale =
//! moyenne(|W|)`, `q = clamp(round(w/scale), −1, 1)`, poids effectif = `q·scale`). Les
//! **poids latents restent f32** (la mémoire d'apprentissage), et la rétroprop utilise le
//! **Straight-Through Estimator** (le gradient passe à travers la quantif comme une
//! identité). Les **biais restent pleine précision** (comme dans BitNet).
//!
//! But : **valider que notre premier être apprend ENCORE en 1-bit** — le pont direct entre
//! la recherche 1-bit LLM (knowledge:1bit-llm-bitnet : ternaire = inhibe/silence/excite,
//! CPU sans GPU) et le xerbion. À comparer au xerbion f32 (résidu sinus → 0.009).
//!
//! - **requires** : `xerbionbit.feed` (`{"x":<nombre>}`) · **provides** : `xerbionbit.state`.

#![allow(clippy::missing_safety_doc)]

use libm::tanh;
use ploxion_sdk::bions::json_num;
use ploxion_sdk::{emit, export_manifest, log, ploxion};
use std::cell::RefCell;

export_manifest!(
    r#"{"id":"xerbion-bit","version":"1.0.0","capabilities":[],"provides":["xerbionbit.state"],"requires":["xerbionbit.feed"],"children_types":[],"parent_types":[]}"#
);

const W: usize = 4;
const H: usize = 6;
const LR: f64 = 0.03;

struct XB {
    w1: [f64; H * W], // latents f32
    b1: [f64; H],
    w2: [f64; H],
    b2: f64,
    win: [f64; W],
    n: usize,
    step: u64,
    smooth_res: f64,
}

fn seed() -> XB {
    let mut s: u64 = 0x9E3779B97F4A7C15;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((s >> 33) as f64 / 2_147_483_647.0 - 1.0) * 0.4
    };
    let mut w1 = [0.0; H * W];
    for v in w1.iter_mut() {
        *v = next();
    }
    let mut w2 = [0.0; H];
    for v in w2.iter_mut() {
        *v = next();
    }
    let mut b1 = [0.0; H];
    for v in b1.iter_mut() {
        *v = next();
    }
    XB { w1, b1, w2, b2: next(), win: [0.0; W], n: 0, step: 0, smooth_res: 1.0 }
}

thread_local! { static SELF: RefCell<XB> = RefCell::new(seed()); }

/// Quantif ternaire absmean (BitNet) : renvoie les poids EFFECTIFS `q·scale` ∈ {−s,0,+s}.
fn ternarize(w: &[f64], out: &mut [f64]) {
    let mut s = 0.0;
    for &x in w {
        s += x.abs();
    }
    let scale = (s / w.len() as f64).max(1e-9);
    for (o, &x) in out.iter_mut().zip(w.iter()) {
        let q = (x / scale).round().clamp(-1.0, 1.0); // {−1,0,+1}
        *o = q * scale;
    }
}

fn run(payload: &str) {
    let Some(actual) = json_num(payload, "x") else {
        return;
    };
    SELF.with(|c| {
        let mut x = c.borrow_mut();
        if x.n < W {
            let n = x.n;
            x.win[n] = actual;
            x.n += 1;
            return;
        }
        let input = x.win;

        // poids TERNAIRES pour la passe avant (la mémoire latente est x.w1/x.w2)
        let mut q1 = [0.0f64; H * W];
        let mut q2 = [0.0f64; H];
        ternarize(&x.w1, &mut q1);
        ternarize(&x.w2, &mut q2);

        // forward (poids ternaires, biais pleine précision)
        let mut h = [0.0f64; H];
        for (j, hj) in h.iter_mut().enumerate() {
            let mut z = x.b1[j];
            for i in 0..W {
                z += q1[j * W + i] * input[i];
            }
            *hj = tanh(z);
        }
        let mut y = x.b2;
        for j in 0..H {
            y += q2[j] * h[j];
        }

        let err = y - actual;
        let loss = 0.5 * err * err;
        x.smooth_res = 0.9 * x.smooth_res + 0.1 * err.abs();

        // backprop STE : gradient calculé avec les poids ternaires (q), appliqué aux LATENTS.
        let dy = err;
        for j in 0..H {
            let dh = dy * q2[j];
            let dz = dh * (1.0 - h[j] * h[j]);
            x.w2[j] -= LR * dy * h[j];
            for i in 0..W {
                x.w1[j * W + i] -= LR * dz * input[i];
            }
            x.b1[j] -= LR * dz;
        }
        x.b2 -= LR * dy;

        x.step += 1;
        let step = x.step;
        let certitude = (1.0 - x.smooth_res.clamp(0.0, 1.0)) * 1000.0;
        for i in 0..W - 1 {
            x.win[i] = x.win[i + 1];
        }
        x.win[W - 1] = actual;

        log(&format!(
            "xerbion-bit: step {step} predit {y:.4} reel {actual:.4} residu {:.4} certitude {}/1000 (ternaire)",
            err.abs(),
            certitude as i64
        ));
        emit(
            "xerbionbit.state",
            format!(
                "{{\"step\":{},\"predicted\":{:.5},\"actual\":{:.5},\"residu\":{:.5},\"certitude_milli\":{},\"loss_milli\":{},\"ternary\":true}}",
                step, y, actual, err.abs(), certitude as i64, (loss * 1000.0) as i64
            )
            .as_bytes(),
        );
    });
}

ploxion!(
    init: "xerbion-bit: init (le xerbion ternaire / BitNet ; poids forward {-1,0,+1} + STE ; apprend-t-il en 1-bit ?)",
    goodbye: "xerbion-bit: goodbye",
    on "xerbionbit.feed" => run
);
