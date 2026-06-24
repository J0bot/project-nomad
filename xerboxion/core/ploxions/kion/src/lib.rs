//! `kion` ploxion — la transformation **wormion -> portion** = le kion = « du réel au kion ».
//!
//! José (2026-06-20) : « tu peux voir mtn comment on part du wormion à un portion,
//! c'est le kion, du réel au kion ». Le **wormion** = le réel rond/continu (ondes,
//! cercles imbriqués, superposition). Le **portion** = le carré/discret/mesuré
//! (gravité, ancrage). Le **kion** est l'opérateur qui collapse l'un dans l'autre.
//!
//! Sur `kion.collapse` (payload `{"mode":"concentrique"|"spirale","rings":R,
//! "points":P,"radius":rad,"grid":G,"turns":T}`) :
//!   1. **échantillonne** le wormion : `mode=concentrique` -> R cercles de P points ;
//!      `mode=spirale` -> une spirale lisse de P points sur T tours (le réel continu) ;
//!   2. **collapse en portion** : snappe chaque point sur une grille carrée `G×G`
//!      (cellule = un carré). Les cellules occupées = la portion (le discret) ;
//!   3. **mesure** : `residu` = erreur de quantification moyenne (distance point ->
//!      centre de cellule) normalisée par le rayon ; `certitude` = 1 − residu, un
//!      **majorant** : grille fine -> petits carrés -> residu↓ -> certitude↑ (la
//!      portion mesure le rond de plus en plus fidèlement). Le **résidu de
//!      quantification EST le résidu du [[tsoin]]** (générateur = les params du
//!      wormion, surprise = l'erreur). Replay : mêmes params -> même portion.
//!
//! Émet `kion.portion` (`{mode,grid,samples,occupied,residu_milli,certitude_milli,cells}`).
//! Déterministe. Le rendu = lane web (les ploxions portion/wormion de José).

#![allow(clippy::missing_safety_doc)]

use libm::{cos, sin, sqrt};
use ploxion_sdk::{emit, export_manifest, log, ploxion};
use ploxion_sdk::bions::{json_int, json_str};
use std::collections::BTreeSet;

export_manifest!(
    r#"{"id":"kion","version":"1.0.0","capabilities":[],"provides":["kion.portion"],"requires":["kion.collapse"],"children_types":[],"parent_types":[]}"#
);

const TAU: f64 = 6.283_185_307_179_586;

/// Snappe une coordonnée [-rad, rad] sur l'indice de cellule [0, g).
fn cell_of(c: f64, rad: f64, g: i64) -> i64 {
    let idx = ((c + rad) / (2.0 * rad) * g as f64).floor() as i64;
    idx.clamp(0, g - 1)
}

/// Centre de la cellule `idx` sur l'axe, en coordonnées réelles.
fn cell_center(idx: i64, rad: f64, g: i64) -> f64 {
    -rad + (idx as f64 + 0.5) * (2.0 * rad / g as f64)
}

fn run(payload: &str) {
    let mode = json_str(payload, "mode").unwrap_or_else(|| "concentrique".to_string());
    let rings = json_int(payload, "rings", 6).clamp(1, 256);
    let points = json_int(payload, "points", 64).clamp(1, 20_000);
    let grid = json_int(payload, "grid", 16).clamp(1, 64);
    let turns = json_int(payload, "turns", 3).clamp(1, 64);
    let rad: f64 = 1000.0; // rayon de référence (unités milli)

    // 1) échantillonne le wormion (le réel rond/continu)
    let mut samples: Vec<(f64, f64)> = Vec::new();
    if mode == "spirale" {
        // une spirale d'Archimède lisse : r croît de 0 a rad sur `turns` tours
        let n = points.max(2);
        for k in 0..n {
            let frac = k as f64 / (n - 1).max(1) as f64;
            let r = rad * frac;
            let theta = TAU * turns as f64 * frac;
            samples.push((r * cos(theta), r * sin(theta)));
        }
    } else {
        // cercles concentriques : R anneaux de P points
        for ring in 1..=rings {
            let r = rad * ring as f64 / rings as f64;
            for k in 0..points {
                let theta = TAU * k as f64 / points as f64;
                samples.push((r * cos(theta), r * sin(theta)));
            }
        }
    }
    let n = samples.len() as f64;
    if n < 1.0 {
        emit("kion.portion", br#"{"mode":"","grid":0,"samples":0,"occupied":0,"residu_milli":0,"certitude_milli":1000,"cells":[]}"#);
        return;
    }

    // 2) collapse en portion : snap grille carrée + (3) mesure le résidu de quantif
    let mut occupied: BTreeSet<(i64, i64)> = BTreeSet::new();
    let mut err_sum = 0.0f64;
    for &(x, y) in &samples {
        let cx = cell_of(x, rad, grid);
        let cy = cell_of(y, rad, grid);
        occupied.insert((cx, cy));
        let dx = x - cell_center(cx, rad, grid);
        let dy = y - cell_center(cy, rad, grid);
        err_sum += sqrt(dx * dx + dy * dy);
    }
    // residu = erreur moyenne normalisée par le rayon (dans [0,1])
    let residu = (err_sum / n) / rad;
    let residu_milli = (residu * 1000.0).round() as i64;
    let certitude_milli = (1000 - residu_milli).clamp(0, 1000);

    // liste des cellules occupées (la portion) : [cx,cy]
    let mut cells = String::from("[");
    for (i, (cx, cy)) in occupied.iter().enumerate() {
        if i > 0 {
            cells.push(',');
        }
        cells.push_str(&format!("[{cx},{cy}]"));
    }
    cells.push(']');

    log(&format!(
        "kion: {mode} {} pts -> portion {grid}x{grid}, {} carres, residu={}/1000, certitude={}/1000",
        samples.len(),
        occupied.len(),
        residu_milli,
        certitude_milli
    ));
    emit(
        "kion.portion",
        format!(
            "{{\"mode\":\"{}\",\"grid\":{},\"samples\":{},\"occupied\":{},\"residu_milli\":{},\"certitude_milli\":{},\"cells\":{}}}",
            mode,
            grid,
            samples.len(),
            occupied.len(),
            residu_milli,
            certitude_milli,
            cells
        )
        .as_bytes(),
    );
}

ploxion!(
    init: "kion: init (transformation wormion -> portion ; du reel au kion ; residu=erreur de quantif)",
    goodbye: "kion: goodbye",
    on "kion.collapse" => run
);
