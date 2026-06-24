//! `spectre` ploxion — **décomposer la musique** avec le xerboxion (José).
//!
//! L'inverse de [`synthe`] : `synthe` compose (freq → son), `spectre` **décompose**
//! (son → les fréquences qui le composent). C'est l'**uncraft appliqué au son** :
//! tout signal se décompose en **ondes sinusoïdales** — les **bions universels de la
//! musique** (Fourier). La DFT trouve quels sinus (fréquences) composent le son ; les
//! pics du spectre = les **fréquences-bions**, leur amplitude = la recette.
//!
//! Sur `spectre.analyze` (payload `{"wave":"saw","freq":440,"dur_ms":50}` OU
//! `{"freqs":[440,554,659]}` pour un accord) :
//!   1. **génère** le signal (même oscillateur que `synthe` ; un `saw` = somme des
//!      harmoniques 1/n, un accord = somme de sinus) ;
//!   2. **décompose** par DFT sur une fenêtre `N=1024` (résolution `SR/N ≈ 43 Hz`) ;
//!   3. **émet** `spectre.result {fundamental, peaks:[{freq,amp_milli}], resolution_hz}`
//!      = les fréquences-bions retrouvées. Pour un `saw 440` on retrouve 440, 880,
//!      1320… avec des amplitudes ~1, 1/2, 1/3 (la série de Fourier, vérifiable).
//!
//! Déterministe (échantillonnage + DFT analytiques). Compose ⇄ décompose = craft ⇄
//! uncraft, sur le son.

#![allow(clippy::missing_safety_doc)]

use libm::{cos, fabs, sin, sqrt};
use ploxion_sdk::bions::{json_num, json_str};
use ploxion_sdk::{emit, export_manifest, log, ploxion};

export_manifest!(
    r#"{"id":"spectre","version":"1.0.0","capabilities":[],"provides":["spectre.result"],"requires":["spectre.analyze"],"children_types":[],"parent_types":[]}"#
);

const SR: f64 = 44100.0;
const N: usize = 1024; // fenêtre DFT (résolution SR/N ≈ 43 Hz)
const TAU: f64 = 6.283_185_307_179_586;

/// L'oscillateur (mêmes formes que `synthe`). `ph` ∈ [0,1).
fn osc(wave: &str, ph: f64) -> f64 {
    match wave {
        "saw" => 2.0 * ph - 1.0,
        "square" => {
            if ph < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        "triangle" => 4.0 * fabs(ph - 0.5) - 1.0,
        _ => sin(TAU * ph), // sine
    }
}

/// Parse `"freqs":[a,b,c]` en Vec<f64> (None si absent).
fn parse_freqs(p: &str) -> Option<Vec<f64>> {
    let i = p.find("\"freqs\"")?;
    let lb = p[i..].find('[')? + i + 1;
    let rb = p[lb..].find(']')? + lb;
    let mut out = Vec::new();
    for tok in p[lb..rb].split(',') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(v) = t.parse::<f64>() {
            out.push(v);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn run(payload: &str) {
    // 1) génère le signal (le "morceau" à décomposer)
    let (mut samples, label) = if let Some(freqs) = parse_freqs(payload) {
        // accord : somme de sinus normalisée
        let mut s = vec![0.0f64; N];
        for (i, sv) in s.iter_mut().enumerate() {
            let t = i as f64 / SR;
            let mut acc = 0.0;
            for &f in &freqs {
                acc += sin(TAU * f * t);
            }
            *sv = acc / freqs.len() as f64;
        }
        let lab = format!(
            "accord [{}]",
            freqs.iter().map(|f| format!("{f:.0}")).collect::<Vec<_>>().join(",")
        );
        (s, lab)
    } else {
        let wave = json_str(payload, "wave").unwrap_or_else(|| "sine".to_string());
        let freq = json_num(payload, "freq").unwrap_or(440.0).clamp(20.0, 20_000.0);
        let mut s = vec![0.0f64; N];
        for (i, sv) in s.iter_mut().enumerate() {
            let ph = (freq * i as f64 / SR).fract();
            *sv = osc(&wave, ph);
        }
        (s, format!("{wave} {freq:.0}Hz"))
    };

    // léger fenêtrage de Hann pour réduire les fuites spectrales (déterministe)
    for (i, sv) in samples.iter_mut().enumerate() {
        let w = 0.5 - 0.5 * cos(TAU * i as f64 / (N as f64 - 1.0));
        *sv *= w;
    }

    // 2) DFT -> magnitudes (k = 0..N/2)
    let half = N / 2;
    let mut mag = vec![0.0f64; half];
    for (k, mk) in mag.iter_mut().enumerate() {
        let mut re = 0.0;
        let mut im = 0.0;
        let w = TAU * k as f64 / N as f64;
        for (i, &s) in samples.iter().enumerate() {
            let a = w * i as f64;
            re += s * cos(a);
            im -= s * sin(a);
        }
        *mk = sqrt(re * re + im * im);
    }

    // 3) trouve les pics (maxima locaux au-dessus de 8% du max)
    let maxm = mag.iter().cloned().fold(0.0f64, f64::max).max(1e-9);
    let thresh = maxm * 0.08;
    let res_hz = SR / N as f64;
    let mut peaks: Vec<(f64, f64)> = Vec::new(); // (freq, amp normalisée)
    for k in 1..half - 1 {
        if mag[k] >= thresh && mag[k] >= mag[k - 1] && mag[k] >= mag[k + 1] {
            peaks.push((k as f64 * res_hz, mag[k] / maxm));
        }
    }
    // garde les 10 plus forts, triés par fréquence
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    peaks.truncate(10);
    peaks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let fundamental = peaks.first().map(|p| p.0).unwrap_or(0.0);
    let mut arr = String::from("[");
    for (i, (f, a)) in peaks.iter().enumerate() {
        if i > 0 {
            arr.push(',');
        }
        arr.push_str(&format!(
            "{{\"freq\":{:.1},\"amp_milli\":{}}}",
            f,
            (a * 1000.0).round() as i64
        ));
    }
    arr.push(']');

    log(&format!(
        "spectre: {label} -> {} pics, fondamentale {:.1} Hz (res {:.1} Hz)",
        peaks.len(),
        fundamental,
        res_hz
    ));
    emit(
        "spectre.result",
        format!(
            "{{\"input\":\"{}\",\"fundamental\":{:.1},\"resolution_hz\":{:.1},\"n_peaks\":{},\"peaks\":{}}}",
            label,
            fundamental,
            res_hz,
            peaks.len(),
            arr
        )
        .as_bytes(),
    );
}

ploxion!(
    init: "spectre: init (decompose le son en ses frequences-bions ; DFT ; inverse de synthe)",
    goodbye: "spectre: goodbye",
    on "spectre.analyze" => run
);
