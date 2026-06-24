//! `science` ploxion — REPRODUCES the lab experiments deterministically.
//!
//! On `science.run` (payload `{"experiment":"sierpinski"|"fern"}`) it runs the
//! IFS **chaos game** (replay of a tiny generator) and measures the attractor's
//! **box-counting dimension**, then emits `science.result`. The PRNG is a
//! fixed-seed xorshift64*, so EVERY run yields the SAME numbers — that is the
//! whole point: anyone, anywhere, re-running the experiment gets an identical,
//! verifiable result. The generator (a handful of affine maps) reconstructs an
//! object of infinite detail: generator + replay = fractal = the tsoin thesis,
//! made reproducible on the bus.
//!
//! Pure consumer of `science.run`, emitter of `science.result`. No capabilities
//! (no net): a self-contained, sandboxed experiment.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};
use ploxion_sdk::bions::{decode_str};

export_manifest!(
    r#"{"id":"science","version":"1.0.0","capabilities":[],"provides":["science.result"],"requires":["science.run"],"children_types":[],"parent_types":[]}"#
);

/// Deterministic PRNG (xorshift64*), fixed seed → the experiment is reproducible.
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0,1).
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

/// Affine map `[a,b,c,d,e,f]`: `x' = a*x + b*y + e`, `y' = c*x + d*y + f`.
type Map = [f64; 6];

/// Run the chaos game: pick a map by probability, apply it, collect points.
fn run_ifs(maps: &[Map], probs: &[f64], n: usize) -> Vec<(f64, f64)> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut cum = Vec::with_capacity(probs.len());
    let mut s = 0.0;
    for &p in probs {
        s += p;
        cum.push(s);
    }
    let (mut x, mut y) = (0.0f64, 0.0f64);
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let r = rng.unit();
        let mut k = 0;
        while k < cum.len() - 1 && r > cum[k] {
            k += 1;
        }
        let m = maps[k];
        let nx = m[0] * x + m[1] * y + m[4];
        let ny = m[2] * x + m[3] * y + m[5];
        x = nx;
        y = ny;
        if i > 20 {
            pts.push((x, y));
        }
    }
    pts
}

/// Box-counting dimension: log–log slope of occupied-cell-count vs grid size.
/// Returns (D, R²).
fn box_dim(pts: &[(f64, f64)]) -> (f64, f64) {
    let (mut mnx, mut mxx, mut mny, mut mxy) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in pts {
        if x < mnx {
            mnx = x;
        }
        if x > mxx {
            mxx = x;
        }
        if y < mny {
            mny = y;
        }
        if y > mxy {
            mxy = y;
        }
    }
    let w = (mxx - mnx).max(mxy - mny).max(1e-9);
    let grids = [8usize, 16, 32, 64, 128, 256, 512];
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for &g in &grids {
        let cell = w / (g as f64);
        let mut occ = std::collections::BTreeSet::new();
        for &(x, y) in pts {
            let i = ((x - mnx) / cell) as i64;
            let j = ((y - mny) / cell) as i64;
            occ.insert((i, j));
        }
        xs.push(libm::log(g as f64));
        ys.push(libm::log(occ.len() as f64));
    }
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for k in 0..xs.len() {
        num += (xs[k] - mx) * (ys[k] - my);
        den += (xs[k] - mx) * (xs[k] - mx);
    }
    let slope = num / den;
    let (mut ssr, mut sst) = (0.0, 0.0);
    for k in 0..xs.len() {
        let yh = my + slope * (xs[k] - mx);
        ssr += (ys[k] - yh) * (ys[k] - yh);
        sst += (ys[k] - my) * (ys[k] - my);
    }
    let r2 = if sst > 0.0 { 1.0 - ssr / sst } else { 0.0 };
    (slope, r2)
}

/// The Sierpinski IFS: 3 maps, contraction s=1/2 → D = ln3/ln2 = 1.58496.
const SIERPINSKI: [Map; 3] = [
    [0.5, 0.0, 0.0, 0.5, 0.0, 0.0],
    [0.5, 0.0, 0.0, 0.5, 0.5, 0.0],
    [0.5, 0.0, 0.0, 0.5, 0.25, 0.5],
];
/// The Barnsley fern: 4 affine maps (non-uniform contraction).
const FERN: [Map; 4] = [
    [0.0, 0.0, 0.0, 0.16, 0.0, 0.0],
    [0.85, 0.04, -0.04, 0.85, 0.0, 1.6],
    [0.2, -0.26, 0.23, 0.22, 0.0, 1.6],
    [-0.15, 0.28, 0.26, 0.24, 0.0, 0.44],
];

// --- PLC lifecycle ----------------------------------------------------------

ploxion_lifecycle!(init: "science: init (reproduces lab experiments; science.run -> science.result; deterministic)", goodbye: "science: goodbye");

#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = decode_str(topic_ptr, topic_len);
    if topic != "science.run" {
        return;
    }
    let payload = decode_str(payload_ptr, payload_len);

    // experiment selector (default sierpinski; "fern" if requested)
    let (exp, maps, probs, theory): (&str, &[Map], &[f64], &str) = if payload.contains("fern") {
        ("fern", &FERN, &[0.01, 0.85, 0.07, 0.07], "n/a (contraction non-uniforme)")
    } else {
        ("sierpinski", &SIERPINSKI, &[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0], "1.5850 (ln3/ln2)")
    };

    let n = 250_000usize;
    let pts = run_ifs(maps, probs, n);
    let (d, r2) = box_dim(&pts);

    let result = format!(
        "{{\"experiment\":\"{}\",\"points\":{},\"n_maps\":{},\"generator_params\":{},\"D_measured\":{:.4},\"D_theory\":\"{}\",\"R2\":{:.4},\"reproducible\":true,\"method\":\"chaos-game + box-counting, seed fixe\"}}",
        exp,
        pts.len(),
        maps.len(),
        maps.len() * 6,
        d,
        theory,
        r2
    );
    log(&format!("science: ran {exp} -> {result}"));
    emit("science.result", result.as_bytes());
}

