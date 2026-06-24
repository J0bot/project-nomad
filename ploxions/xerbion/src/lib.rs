//! `xerbion` ploxion — **le premier xerbion** : notre premier **être du réel fait de bions**.
//!
//! José : « fais un ploxion réseau de neurones, ça sera notre premier xerbion, notre premier
//! être du réel fait de bions ». C'est un **vrai (petit) réseau de neurones** — un MLP
//! (4 entrées → 6 cachés tanh → 1 sortie) — qui **prédit son entrée**, **émet son résidu**
//! (= l'erreur de prédiction = **le tsoin**), et **apprend en ligne** (descente de gradient).
//!
//! C'est le **codage prédictif** (Friston) incarné : la prédiction = le générateur, le résidu =
//! la surprise. **Apprendre = minimiser le résidu = minimiser l'énergie libre = « tout optimise le
//! tsoin »** — la loi du cerveau ET du xerboxion. Au fil des `xerbion.feed`, le résidu **descend** :
//! l'être *comprend* son flux. Bâti sur les bions partagés + un cœur neuronal. Déterministe (poids
//! initialisés par un LCG à graine fixe → trajectoire d'apprentissage rejouable = un vrai tsoin).
//!
//! - **requires** : `xerbion.feed` (`{"x": <nombre>}`) — un échantillon du réel.
//! - **provides** : `xerbion.state` (`{step, predicted, actual, residu, certitude_milli, loss_milli}`).

#![allow(clippy::missing_safety_doc)]

use libm::tanh;
use ploxion_sdk::bions::json_num;
use ploxion_sdk::{emit, export_manifest, log, ploxion};
use std::cell::RefCell;

export_manifest!(
    r#"{"id":"xerbion","version":"1.0.0","capabilities":[],"provides":["xerbion.state"],"requires":["xerbion.feed"],"children_types":[],"parent_types":[]}"#
);

const W: usize = 4; // fenêtre d'entrée (les 4 dernières valeurs)
const H: usize = 6; // neurones cachés
const LR: f64 = 0.03; // taux d'apprentissage

/// L'être : ses poids (la mémoire apprise), sa fenêtre de perception, son vécu.
struct Xerbion {
    w1: [f64; H * W],
    b1: [f64; H],
    w2: [f64; H],
    b2: f64,
    win: [f64; W], // les W dernières valeurs vues
    n: usize,      // combien de valeurs vues (pour remplir la fenêtre)
    step: u64,
    smooth_res: f64, // résidu absolu lissé (l'incertitude courante)
}

/// LCG déterministe -> petits poids initiaux dans [-0.4, 0.4].
fn seed_weights() -> Xerbion {
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
    Xerbion {
        w1,
        b1,
        w2,
        b2: next(),
        win: [0.0; W],
        n: 0,
        step: 0,
        smooth_res: 1.0,
    }
}

thread_local! {
    static SELF: RefCell<Xerbion> = RefCell::new(seed_weights());
}

/// Passe avant : la fenêtre -> (prédiction y, activations cachées h).
fn forward(x: &Xerbion, input: &[f64; W]) -> (f64, [f64; H]) {
    let mut h = [0.0f64; H];
    for (j, hj) in h.iter_mut().enumerate() {
        let mut z = x.b1[j];
        for i in 0..W {
            z += x.w1[j * W + i] * input[i];
        }
        *hj = tanh(z);
    }
    let mut y = x.b2;
    for j in 0..H {
        y += x.w2[j] * h[j];
    }
    (y, h)
}

fn run(payload: &str) {
    let Some(actual) = json_num(payload, "x") else {
        return;
    };
    SELF.with(|c| {
        let mut x = c.borrow_mut();

        // Pas encore assez de contexte : on remplit la fenêtre et on attend.
        if x.n < W {
            let n = x.n;
            x.win[n] = actual;
            x.n += 1;
            return;
        }

        // 1) PRÉDIT le prochain réel depuis la fenêtre (le générateur).
        let input = x.win;
        let (y, h) = forward(&x, &input);

        // 2) RÉSIDU = surprise = erreur de prédiction (= le tsoin).
        let err = y - actual;
        let loss = 0.5 * err * err;
        x.smooth_res = 0.9 * x.smooth_res + 0.1 * err.abs();

        // 3) APPREND : rétropropagation + SGD (minimise le résidu = l'énergie libre).
        let dy = err;
        for j in 0..H {
            let dh = dy * x.w2[j];
            let dz = dh * (1.0 - h[j] * h[j]); // tanh'
            x.w2[j] -= LR * dy * h[j];
            for i in 0..W {
                x.w1[j * W + i] -= LR * dz * input[i];
            }
            x.b1[j] -= LR * dz;
        }
        x.b2 -= LR * dy;

        x.step += 1;
        let step = x.step;
        // certitude = 1 - résidu lissé (borné [0,1]) — un majorant, comme partout.
        let certitude = (1.0 - x.smooth_res.clamp(0.0, 1.0)) * 1000.0;

        // 4) avance la fenêtre (le réel devient le passé).
        for i in 0..W - 1 {
            x.win[i] = x.win[i + 1];
        }
        x.win[W - 1] = actual;

        log(&format!(
            "xerbion: step {step} predit {y:.4} reel {actual:.4} residu {:.4} certitude {}/1000",
            err.abs(),
            certitude as i64
        ));
        emit(
            "xerbion.state",
            format!(
                "{{\"step\":{},\"predicted\":{:.5},\"actual\":{:.5},\"residu\":{:.5},\"certitude_milli\":{},\"loss_milli\":{}}}",
                step,
                y,
                actual,
                err.abs(),
                certitude as i64,
                (loss * 1000.0) as i64
            )
            .as_bytes(),
        );
    });
}

ploxion!(
    init: "xerbion: init (le premier etre fait de bions ; MLP predictif qui apprend en ligne ; residu = tsoin)",
    goodbye: "xerbion: goodbye",
    on "xerbion.feed" => run
);
