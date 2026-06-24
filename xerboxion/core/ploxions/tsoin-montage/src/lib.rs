//! `tsoin-montage` ploxion — le MONTAGE de tsoins (cahier rs-flay, IMG_0643 « Combinaison de
//! deux tsoins : `]` + `[` = … ou … ou … »). Combine deux tsoins (ou une LISTE = une timeline)
//! en un NOUVEAU tsoin, content-adresse (addr64) et grave. Le « plusieurs resultats possibles »
//! du plan = le choix de l'OPERATEUR : la meme paire donne un composite different selon l'op.
//!
//!  - `seq`   : a ++ b            — le CUT/splice du montage (coller dans le temps). Defaut.
//!  - `xor`   : xor_delta(a,b)    — la SURPRISE/delta (ce qui differe entre les deux).
//!  - `weave` : a0 b0 a1 b1 …     — entrelacer (alterner les frames).
//!  - `fold`  : residu_minimal(xor) — l'ESSENCE (le residu compresse de la combinaison).
//!  - `mask`  : a la ou b != 0    — surimpression (a masque par b).
//!
//! Tout est deterministe : memes entrees + meme op => meme addr (tsoin REPRODUCTIBLE). Un montage
//! EST un tsoin (« tsoin dans un tsoin », « chaque tsoin contient tous les tsoins »).
//! Events : `tsoin.montage {a,b,op}` ou `{items:[{t:hex},…],op}` ; `tsoin.montage.all {a,b}`
//! (applique TOUS les ops => grave 5 composites = les « plusieurs resultats » d'une seule paire).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{decode_str, json_str, tsoin_record, to_hex, from_hex,
                         xor_delta, popcount_bytes, residu_minimal, addr64,
                         array_body, split_objects};

export_manifest!(
    r#"{"id":"tsoin-montage","version":"1.0.0","capabilities":[],"provides":["tsoin.montaged","tsoin.record"],"requires":["tsoin.montage","tsoin.montage.all"],"children_types":[],"parent_types":[]}"#
);

const OPS: &[&str] = &["seq", "xor", "weave", "fold", "mask"];

fn js(p: &str, k: &str) -> String { json_str(p, k).unwrap_or_default() }
fn hx(p: &str, k: &str) -> Vec<u8> { from_hex(&js(p, k)).unwrap_or_default() }

/// Pad les deux cotes a la meme longueur (zero-fill) pour weave/mask.
fn pad_max(a: &[u8], b: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let n = a.len().max(b.len());
    let mut x = a.to_vec(); x.resize(n, 0);
    let mut y = b.to_vec(); y.resize(n, 0);
    (x, y)
}

/// Combine deux tsoins selon l'operateur. Le cœur du montage.
fn combine(op: &str, a: &[u8], b: &[u8]) -> Vec<u8> {
    match op {
        "xor"  => xor_delta(a, b),
        "fold" => residu_minimal(&xor_delta(a, b)),
        "weave" => {
            let (x, y) = pad_max(a, b);
            let mut out = Vec::with_capacity(x.len() * 2);
            for i in 0..x.len() { out.push(x[i]); out.push(y[i]); }
            out
        }
        "mask" => {
            let (x, y) = pad_max(a, b);
            x.iter().zip(&y).map(|(&p, &q)| if q != 0 { p } else { 0 }).collect()
        }
        // "seq" et tout op inconnu => splice (le cut du montage).
        _ => { let mut v = a.to_vec(); v.extend_from_slice(b); v }
    }
}

/// Content-adresse le composite, grave le tsoin reproductible, emet `tsoin.montaged`.
fn grave_montage(op: &str, n: usize, out: &[u8]) -> u64 {
    let addr = addr64(&format!("montage:{op}:{n}"), out);
    let bits = popcount_bytes(out);
    tsoin_record(&format!("montage:{op}:{:016x}", addr), out);
    let body = format!(
        "{{\"op\":\"{}\",\"n\":{},\"len\":{},\"bits\":{},\"addr\":\"{:016x}\",\"out\":\"{}\"}}",
        op, n, out.len(), bits, addr,
        // composite tronque a 64 octets dans l'event (le tsoin complet est grave).
        to_hex(&out[..out.len().min(64)])
    );
    emit("tsoin.montaged", body.as_bytes());
    log(&format!("montage[{op}] {n} tsoins -> {} octets, {bits} bits, addr {:016x}", out.len(), addr));
    addr
}

/// Parse `items:[{"t":"<hex>"},…]` en une liste d'octets (la timeline).
fn parse_items(payload: &str) -> Vec<Vec<u8>> {
    match array_body(payload, "items") {
        Some(body) => split_objects(body).iter()
            .map(|o| from_hex(&json_str(o, "t").unwrap_or_default()).unwrap_or_default())
            .filter(|v| !v.is_empty())
            .collect(),
        None => Vec::new(),
    }
}

/// Monte une timeline : replie l'op de gauche a droite (acc = op(acc, item)).
fn montage_list(op: &str, items: &[Vec<u8>]) {
    if items.is_empty() { log("montage: liste vide"); return; }
    let mut acc = items[0].clone();
    for it in &items[1..] { acc = combine(op, &acc, it); }
    grave_montage(op, items.len(), &acc);
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("tsoin-montage: init (combiner des tsoins -> un nouveau tsoin ; ops seq/xor/weave/fold/mask)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
    let topic = decode_str(tp, tl);
    let payload = decode_str(pp, pl);
    match topic.as_str() {
        "tsoin.montage" => {
            let items = parse_items(&payload);
            if !items.is_empty() {
                let op = js(&payload, "op");
                montage_list(if op.is_empty() { "seq" } else { &op }, &items);
            } else {
                let (a, b) = (hx(&payload, "a"), hx(&payload, "b"));
                if a.is_empty() && b.is_empty() { log("montage: ni items ni a/b"); return; }
                let op = js(&payload, "op");
                let out = combine(if op.is_empty() { "seq" } else { &op }, &a, &b);
                grave_montage(if op.is_empty() { "seq" } else { &op }, 2, &out);
            }
        }
        "tsoin.montage.all" => {
            // La meme paire, TOUS les ops => les « plusieurs resultats possibles » du plan.
            let (a, b) = (hx(&payload, "a"), hx(&payload, "b"));
            if a.is_empty() && b.is_empty() { log("montage.all: a/b vides"); return; }
            for op in OPS { grave_montage(op, 2, &combine(op, &a, &b)); }
            log(&format!("montage.all: {} composites d'une seule paire (op={:?})", OPS.len(), OPS));
        }
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("tsoin-montage: goodbye");
}
