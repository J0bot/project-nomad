//! `synthe` ploxion — le premier NODE-INSTRUMENT audio du xerboxion.
//!
//! Sur `synth.play` (payload `{"wave":"sine|saw|square|triangle","freq":440,"dur_ms":500}`)
//! il SYNTHÉTISE un PCM 16-bit mono déterministe (oscillateur + court fade anti-clic) et émet
//! `synth.result` avec les stats + un hash FNV-1a du PCM. Déterministe → REPRODUCTIBLE : les
//! mêmes paramètres rejouent le MÊME son, au bit près. C'est la preuve que **un son est un tsoin** :
//! ~quelques octets de paramètres (le générateur) reconstruisent un signal de dizaines de milliers
//! d'échantillons (l'attracteur audio) — l'analogue sonore de Sierpinski. Le rendu vers le haut-
//! parleur (Web Audio) est la lane web ; ce ploxion EST le générateur, content-adressable en tsoin.
//!
//! Pure consumer de `synth.play`, émetteur de `synth.result`. Aucune capability.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};
use ploxion_sdk::bions::{json_num, decode_str};

export_manifest!(
    r#"{"id":"synthe","version":"1.0.0","capabilities":[],"provides":["synth.result"],"requires":["synth.play"],"children_types":[],"parent_types":[]}"#
);

const SR: u32 = 44_100;
const TWO_PI: f64 = core::f64::consts::PI * 2.0;

/// Un échantillon de l'oscillateur, phase `ph` ∈ [0,1).
fn osc(wave: &str, ph: f64) -> f64 {
    if wave.contains("saw") {
        2.0 * ph - 1.0
    } else if wave.contains("square") {
        if ph < 0.5 { 1.0 } else { -1.0 }
    } else if wave.contains("triangle") {
        4.0 * (ph - (ph + 0.5).floor()).abs() - 1.0
    } else {
        libm::sin(TWO_PI * ph) // sine par défaut
    }
}

/// FNV-1a 64 bits sur les octets du PCM (empreinte reproductible).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

ploxion_lifecycle!(init: "synthe: init (node-instrument; synth.play -> synth.result; un son est un tsoin, deterministe)", goodbye: "synthe: goodbye");

#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = decode_str(topic_ptr, topic_len);
    if topic != "synth.play" {
        return;
    }
    let payload = decode_str(payload_ptr, payload_len);

    let wave = if payload.contains("saw") {
        "saw"
    } else if payload.contains("square") {
        "square"
    } else if payload.contains("triangle") {
        "triangle"
    } else {
        "sine"
    };
    let freq = json_num(&payload, "freq").unwrap_or(440.0).clamp(20.0, 20_000.0);
    let dur_ms = json_num(&payload, "dur_ms").unwrap_or(500.0).clamp(1.0, 10_000.0);

    let n = ((SR as f64) * dur_ms / 1000.0) as usize;
    let fade = (SR / 200).max(1) as usize; // ~5 ms anti-clic
    let mut pcm: Vec<u8> = Vec::with_capacity(n * 2);
    let mut peak: f64 = 0.0;
    let mut sumsq: f64 = 0.0;
    for i in 0..n {
        let ph = (freq * i as f64 / SR as f64).fract();
        let mut s = osc(wave, ph) * 0.3; // -10 dBFS pour ne pas saturer
        // fade in/out lineaire
        if i < fade {
            s *= i as f64 / fade as f64;
        }
        if i >= n.saturating_sub(fade) {
            s *= (n - i) as f64 / fade as f64;
        }
        if s.abs() > peak {
            peak = s.abs();
        }
        sumsq += s * s;
        let v = (s * 32767.0) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    let rms = if n > 0 { libm::sqrt(sumsq / n as f64) } else { 0.0 };
    let hash = fnv1a(&pcm);
    // le GENERATEUR (le tsoin du son) : quelques octets qui reproduisent tout le PCM
    let generateur = format!("{{\"wave\":\"{wave}\",\"freq\":{freq},\"dur_ms\":{dur_ms},\"sr\":{SR}}}");
    let result = format!(
        "{{\"wave\":\"{}\",\"freq\":{:.1},\"dur_ms\":{:.0},\"sr\":{},\"samples\":{},\"pcm_octets\":{},\"generateur_octets\":{},\"pcm_fnv1a\":\"{:016x}\",\"peak\":{:.3},\"rms\":{:.3},\"reproducible\":true,\"generateur\":{}}}",
        wave, freq, dur_ms, SR, n, pcm.len(), generateur.len(), hash, peak, rms, generateur
    );
    log(&format!("synthe: {wave} {freq:.0}Hz {dur_ms:.0}ms -> {n} samples, fnv {hash:016x}"));
    emit("synth.result", result.as_bytes());
}

