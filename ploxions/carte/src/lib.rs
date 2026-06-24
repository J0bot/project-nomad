//! `carte` ploxion — le cartographe 4D du xion.
//!
//! Réponse à José : « le but d'une map c'est de mettre une donnée dans une
//! coordonnée en 4 dimensions, 3D et 1T, pck la dimension du temps est cohérence »
//! et « quand $a devient trop grand il faut que $a devienne des bions / cubions,
//! pck il faut comprimer tout ça, sinon mon browser va crash ».
//!
//! Sur `carte.map` (payload `{"grid":G,"cap":C,"points":[{"id":"..","cat":".."}, ..]}`) :
//!   1. **place** chaque point en 4D — `(x,y,z)` = hash déterministe FNV-1a (le
//!      bion a une adresse stable), `t` = **cohérence** = densité thématique
//!      (combien de points partagent sa `cat`), normalisée en milli ;
//!   2. **comprime** — découpe l'espace en grille `G^3` ; toute cellule de plus
//!      de `C` points est **repliée en un cubion** : `{kind:"cubion", gen, count,
//!      centroid, t}` où `gen` = FNV des ids membres (le *générateur*/adresse) et
//!      `count`+`centroid` = le *résidu* (la surprise). Exactement un [[tsoin]] :
//!      générateur + résidu. Les cellules clairsemées restent des **bions**.
//!
//! Émet `carte.mapped` (`{before,after,ratio_milli,cubions,bions,nodes:[..]}`).
//! Déterministe ; même entrée -> même carte. Le rendu 3D lui-même = lane RepoVerse.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log, ploxion};
use ploxion_sdk::bions::{json_esc as esc_json, fnv1a64, json_int, json_str};
use std::collections::BTreeMap;

export_manifest!(
    r#"{"id":"carte","version":"1.0.0","capabilities":[],"provides":["carte.mapped"],"requires":["carte.map"],"children_types":[],"parent_types":[]}"#
);

// --- petit parseur JSON (même esprit que les autres ploxions) ----------------

/// Découpe le tableau `"points":[ {..}, {..} ]` en sous-chaînes d'objets `{..}`.
fn split_objects(p: &str) -> Vec<String> {
    let mut objs = Vec::new();
    let Some(arr) = p.find("\"points\"").and_then(|i| p[i..].find('[').map(|j| i + j + 1)) else {
        return objs;
    };
    let bytes = p.as_bytes();
    let mut i = arr;
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut in_str = false;
    let mut esc = false;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if in_str {
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
            }
        } else if ch == '"' {
            in_str = true;
        } else if ch == '{' {
            if depth == 0 {
                start = i;
            }
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                objs.push(p[start..=i].to_string());
            }
        } else if ch == ']' && depth == 0 {
            break;
        }
        i += 1;
    }
    objs
}

// --- l'adressage : hash -> coordonnées 3D ------------------------------------

/// 21 bits -> coordonnée signée en milli, dans [-1000, 1000].
fn coord(bits: u64) -> i64 {
    let v = (bits & 0x1f_ffff) as i64; // 0 .. 2_097_151
    (v * 2000 / 2_097_151) - 1000
}

/// Indice de cellule sur un axe : [-1000,1000] -> [0, g).
fn cell_of(c: i64, g: i64) -> i64 {
    let idx = (c + 1000) * g / 2001;
    idx.clamp(0, g - 1)
}

struct P {
    id: String,
    cat: String,
    x: i64,
    y: i64,
    z: i64,
    t: i64, // coherence en milli (rempli après)
    cell: u64,
}

fn run(payload: &str) {
    let grid = json_int(payload, "grid", 6).clamp(1, 64);
    let cap = json_int(payload, "cap", 1).max(1);
    let objs = split_objects(payload);
    let n = objs.len();
    if n == 0 {
        emit(
            "carte.mapped",
            br#"{"before":0,"after":0,"ratio_milli":0,"cubions":0,"bions":0,"nodes":[]}"#,
        );
        log("carte: 0 point");
        return;
    }

    // 1) place + compte par categorie (la coherence)
    let mut cat_count: BTreeMap<String, i64> = BTreeMap::new();
    let mut pts: Vec<P> = Vec::with_capacity(n);
    for o in &objs {
        let id = json_str(o, "id").unwrap_or_default();
        let cat = json_str(o, "cat").unwrap_or_default();
        let h = fnv1a64(&id);
        let x = coord(h);
        let y = coord(h >> 21);
        let z = coord(h >> 42);
        let cx = cell_of(x, grid);
        let cy = cell_of(y, grid);
        let cz = cell_of(z, grid);
        let cell = (cx + cy * grid + cz * grid * grid) as u64;
        if !cat.is_empty() {
            *cat_count.entry(cat.clone()).or_insert(0) += 1;
        }
        pts.push(P { id, cat, x, y, z, t: 0, cell });
    }
    // coherence = densite thematique normalisee (t = temps = coherence, dixit Jose)
    let max_cat = cat_count.values().copied().max().unwrap_or(1).max(1);
    for p in pts.iter_mut() {
        let c = if p.cat.is_empty() {
            0
        } else {
            *cat_count.get(&p.cat).unwrap_or(&0)
        };
        p.t = c * 1000 / max_cat;
    }

    // 2) groupe par cellule
    let mut cells: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (i, p) in pts.iter().enumerate() {
        cells.entry(p.cell).or_default().push(i);
    }

    // 3) replie : cellule dense -> cubion (generateur+residu) ; sinon bions
    let mut nodes = String::from("[");
    let mut first = true;
    let mut n_cubions = 0i64;
    let mut n_bions = 0i64;
    for (cell, idxs) in &cells {
        if !first {
            nodes.push(',');
        }
        first = false;
        if (idxs.len() as i64) <= cap {
            // bions : on garde chaque point tel quel
            for (k, &i) in idxs.iter().enumerate() {
                if k > 0 {
                    nodes.push(',');
                }
                let p = &pts[i];
                nodes.push_str(&format!(
                    "{{\"kind\":\"bion\",\"id\":\"{}\",\"cat\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"t\":{}}}",
                    esc_json(&p.id),
                    esc_json(&p.cat),
                    p.x,
                    p.y,
                    p.z,
                    p.t
                ));
                n_bions += 1;
            }
        } else {
            // cubion : on replie l'amas. generateur = FNV des ids tries (adresse),
            // residu = count + centroid + cat dominante (la surprise).
            let mut ids: Vec<&str> = idxs.iter().map(|&i| pts[i].id.as_str()).collect();
            ids.sort_unstable();
            let gen = fnv1a64(&ids.join("\u{1}"));
            let count = idxs.len() as i64;
            let (mut sx, mut sy, mut sz, mut st) = (0i64, 0i64, 0i64, 0i64);
            let mut dom: BTreeMap<&str, i64> = BTreeMap::new();
            for &i in idxs {
                let p = &pts[i];
                sx += p.x;
                sy += p.y;
                sz += p.z;
                st += p.t;
                if !p.cat.is_empty() {
                    *dom.entry(p.cat.as_str()).or_insert(0) += 1;
                }
            }
            let dom_cat = dom.iter().max_by_key(|(_, v)| **v).map(|(k, _)| *k).unwrap_or("");
            nodes.push_str(&format!(
                "{{\"kind\":\"cubion\",\"gen\":\"{:016x}\",\"cell\":{},\"count\":{},\"cat\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"t\":{}}}",
                gen,
                cell,
                count,
                esc_json(dom_cat),
                sx / count,
                sy / count,
                sz / count,
                st / count
            ));
            n_cubions += 1;
        }
    }
    nodes.push(']');

    let after = n_bions + n_cubions;
    let ratio_milli = after * 1000 / n as i64;
    log(&format!(
        "carte: {n} points -> {after} noeuds ({n_cubions} cubions, {n_bions} bions), ratio={}/1000",
        ratio_milli
    ));
    emit(
        "carte.mapped",
        format!(
            "{{\"before\":{},\"after\":{},\"ratio_milli\":{},\"cubions\":{},\"bions\":{},\"grid\":{},\"cap\":{},\"nodes\":{}}}",
            n, after, ratio_milli, n_cubions, n_bions, grid, cap, nodes
        )
        .as_bytes(),
    );
}

ploxion!(
    init: "carte: init (cartographe 4D ; place x,y,z,t=coherence + replie en cubions)",
    goodbye: "carte: goodbye",
    on "carte.map" => run
);
