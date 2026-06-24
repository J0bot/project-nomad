//! `bion-accelerator` ploxion — le xerboxion vu comme un **ACCÉLÉRATEUR DE PARTICULES DE BIONS**
//! (José). On fait entrer des bions en collision : ils **FUSIONNENT** (deux bions → un bion
//! composé, avec une *énergie de liaison*) ou ils **FISSIONNENT** (un bion → deux bions à sa
//! *ligne de faille*, avec une *énergie libérée*). Chaque produit est un **NOUVEAU bion**
//! content-adressé (addr64) et gravé : « on trouve plus de bions ». C'est le moteur de DÉCOUVERTE
//! du système de dépendances (cf. docs/research/analyse-optimisation-decouverte-bions.md :
//! fission = *diviser*, fusion = *combiner* → **bions de bions**).
//!
//! Mécanique (déterministe, sur les octets-résidu du bion) :
//!  - **fission(B)** : on coupe à l'indice `k` qui maximise l'énergie de la couture
//!    `popcount(B[k-1] ^ B[k])` (la liaison la plus tendue = la ligne de faille). Produits =
//!    `B[..k]` et `B[k..]` ; énergie libérée = ces bits de liaison rompus.
//!  - **fusion(A,B)** : le bion composé = `A ++ B` (le noyau lié) ; énergie de liaison = le
//!    nombre de bits où A et B **s'accordent** sur leur recouvrement (plus d'accord = noyau plus
//!    stable). Beaucoup d'accord ⇒ liaison forte ; aucun ⇒ composé instable (prêt à re-fissionner).
//!  - **collide(A,B)** : un évènement complet d'accélérateur — fusion(A,B)=P puis fission(P) —
//!    qui découvre jusqu'à 3 bions d'un coup (le composé + ses deux filles).
//!
//! Events : `bion.fusion {a,b}`, `bion.fission {b}`, `bion.collide {a,b}` (hex). Émet
//! `bion.discovered {addr,len,from}` par produit + `bion.fused`/`bion.fissioned`/`bion.collided`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{decode_str, json_str, from_hex, addr64, tsoin_record};

export_manifest!(
    r#"{"id":"bion-accelerator","version":"1.0.0","capabilities":[],"provides":["bion.discovered","bion.fused","bion.fissioned","bion.collided","tsoin.record"],"requires":["bion.fusion","bion.fission","bion.collide"],"children_types":[],"parent_types":[]}"#
);

fn hx(p: &str, k: &str) -> Vec<u8> { from_hex(&json_str(p, k).unwrap_or_default()).unwrap_or_default() }

/// Découvre un bion : content-adresse, grave `bion:<addr>`, émet `bion.discovered`. Renvoie addr.
fn discover(bytes: &[u8], from: &str) -> u64 {
    let addr = addr64("bion", bytes);
    tsoin_record(&format!("bion:{:016x}", addr), bytes);
    emit("bion.discovered", format!(
        "{{\"addr\":\"{:016x}\",\"len\":{},\"from\":\"{}\"}}", addr, bytes.len(), from
    ).as_bytes());
    addr
}

/// FISSION : trouve la ligne de faille (couture la plus tendue) ; renvoie (k, energie_bits).
fn fault_line(b: &[u8]) -> Option<(usize, u32)> {
    if b.len() < 2 { return None; }
    let mut best_k = 1usize;
    let mut best_e = 0u32;
    for k in 1..b.len() {
        let e = (b[k - 1] ^ b[k]).count_ones();
        if e > best_e { best_e = e; best_k = k; }
    }
    Some((best_k, best_e))
}

/// FUSION : énergie de liaison = bits d'ACCORD entre A et B sur leur recouvrement.
fn binding_energy(a: &[u8], b: &[u8]) -> u32 {
    let n = a.len().min(b.len());
    let mut agree = 0u32;
    for i in 0..n { agree += 8 - (a[i] ^ b[i]).count_ones(); }
    agree
}

/// fission(B) -> grave les deux filles, émet bion.fissioned. Renvoie (addr_left, addr_right).
fn do_fission(b: &[u8]) -> Option<(u64, u64)> {
    let (k, e) = fault_line(b)?;
    let parent = addr64("bion", b);
    let la = discover(&b[..k], "fission");
    let ra = discover(&b[k..], "fission");
    emit("bion.fissioned", format!(
        "{{\"parent\":\"{:016x}\",\"k\":{},\"energy\":{},\"left\":\"{:016x}\",\"right\":\"{:016x}\"}}",
        parent, k, e, la, ra
    ).as_bytes());
    log(&format!("fission {:016x} @k={k} E={e} -> {:016x} + {:016x}", parent, la, ra));
    Some((la, ra))
}

/// fusion(A,B) -> grave le composé, émet bion.fused. Renvoie (addr_produit, energie_liaison).
fn do_fusion(a: &[u8], b: &[u8]) -> (u64, u32) {
    let mut p = a.to_vec();
    p.extend_from_slice(b);
    let binding = binding_energy(a, b);
    let pa = discover(&p, "fusion");
    emit("bion.fused", format!(
        "{{\"a_len\":{},\"b_len\":{},\"binding\":{},\"product\":\"{:016x}\",\"len\":{}}}",
        a.len(), b.len(), binding, pa, p.len()
    ).as_bytes());
    log(&format!("fusion {}+{} octets -> {:016x} liaison={binding}b", a.len(), b.len(), pa));
    (pa, binding)
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("bion-accelerator: init (collisionneur de bions ; fusion + fission => on decouvre plus de bions)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
    let topic = decode_str(tp, tl);
    let payload = decode_str(pp, pl);
    match topic.as_str() {
        "bion.fusion" => {
            let (a, b) = (hx(&payload, "a"), hx(&payload, "b"));
            if a.is_empty() && b.is_empty() { log("fusion: a/b vides"); return; }
            do_fusion(&a, &b);
        }
        "bion.fission" => {
            let b = hx(&payload, "b");
            if b.len() < 2 { log("fission: bion trop petit (<2 octets, indivisible)"); return; }
            do_fission(&b);
        }
        "bion.collide" => {
            // Évènement d'accélérateur complet : fusion puis fission du composé.
            let (a, b) = (hx(&payload, "a"), hx(&payload, "b"));
            if a.is_empty() && b.is_empty() { log("collide: a/b vides"); return; }
            let mut p = a.to_vec();
            p.extend_from_slice(&b);
            let (pa, binding) = do_fusion(&a, &b);
            let (la, ra, e) = match fault_line(&p) {
                Some((k, e)) => { let l = discover(&p[..k], "collide"); let r = discover(&p[k..], "collide"); (l, r, e) }
                None => (pa, pa, 0),
            };
            emit("bion.collided", format!(
                "{{\"product\":\"{:016x}\",\"binding\":{},\"energy\":{},\"left\":\"{:016x}\",\"right\":\"{:016x}\"}}",
                pa, binding, e, la, ra
            ).as_bytes());
            log(&format!("collide -> compose {:016x} (liaison {binding}) puis fission E={e} -> {:016x}+{:016x}", pa, la, ra));
        }
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("bion-accelerator: goodbye");
}
