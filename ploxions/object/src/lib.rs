//! `object` ploxion — la pipeline de CREATION D'OBJET. « Refaire la redstone mais avec des
//! bions » (José) : un objet de redstone = des **cubions (blocs) CONNECTÉS**. Ce ploxion
//! assemble une liste de cubions en un OBJET, qui est lui-même un **tsoin** content-adressé
//! (addr64) et gravé — donc montable ([[tsoin-montage]]), rejouable, partageable.
//!
//! Il ferme aussi le **chaînon manquant** documenté dans `sdk/src/block.rs` : les bions blocs
//! (`block-stone`/`block-water`) `require` `block.place` mais AUCUN ploxion ne l'émettait — ils
//! étaient des consommateurs pendants. En posant un objet, on **émet `block.place` par cubion**,
//! ce qui DÉCLENCHE les bions blocs en live (ils gravent leur tsoin de pose + émettent
//! `block.placed`/`block.flow`). L'objet est la cause, le cubion est l'effet, le tsoin est la trace.
//!
//! Déterministe & reproductible : les cubions sont triés (ordre-indépendant) puis sérialisés
//! canoniquement ; deux fois le même objet (même contenu, ordre quelconque) => même addr.
//!
//! Events :
//!  - `object.build {name, dim?, blocks:[{block,x,y,z}, …]}` -> grave `object:<name>:<addr>`
//!    (résidu = la sérialisation canonique), émet `block.place` par cubion, puis `object.built`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{decode_str, json_str, json_int, json_esc, tsoin_record, addr64,
                         array_body, split_objects};

export_manifest!(
    r#"{"id":"object","version":"1.0.0","capabilities":[],"provides":["object.built","block.place","tsoin.record"],"requires":["object.build"],"children_types":["block"],"parent_types":[]}"#
);

/// Un cubion posé : le bloc + sa coordonnée. C'est le résidu d'une case de l'objet.
struct Cubion { block: String, x: i64, y: i64, z: i64 }

/// Parse `blocks:[{block,x,y,z}, …]` en cubions (bions json_* réels).
fn parse_cubions(payload: &str) -> Vec<Cubion> {
    let body = match array_body(payload, "blocks") { Some(b) => b, None => return Vec::new() };
    split_objects(body).iter().filter_map(|o| {
        let block = json_str(o, "block").unwrap_or_default();
        if block.is_empty() { return None; }
        Some(Cubion {
            block,
            x: json_int(o, "x", 0),
            y: json_int(o, "y", 0),
            z: json_int(o, "z", 0),
        })
    }).collect()
}

/// Sérialisation CANONIQUE (triée => ordre-indépendante) : une ligne `block:x,y,z` par cubion.
/// C'est le résidu content-adressé de l'objet.
fn canonical(cubions: &mut [Cubion]) -> Vec<u8> {
    cubions.sort_by(|a, b| (a.x, a.y, a.z, &a.block).cmp(&(b.x, b.y, b.z, &b.block)));
    let mut s = String::new();
    for c in cubions.iter() {
        s.push_str(&format!("{}:{},{},{}\n", c.block, c.x, c.y, c.z));
    }
    s.into_bytes()
}

fn build_object(payload: &str) {
    let name = json_str(payload, "name").unwrap_or_else(|| "objet".to_string());
    let dim = json_str(payload, "dim").unwrap_or_else(|| "overworld".to_string());
    let mut cubions = parse_cubions(payload);
    if cubions.is_empty() { log("object: aucun cubion (blocks vide)"); return; }

    let canon = canonical(&mut cubions);
    let addr = addr64(&format!("object:{name}"), &canon);

    // 1) l'objet EST un tsoin : grave la sérialisation canonique (rejouable).
    tsoin_record(&format!("object:{}:{:016x}", json_esc(&name), addr), &canon);

    // 2) pose chaque cubion -> DÉCLENCHE les bions blocs (chaînon block.place).
    for c in &cubions {
        emit("block.place", format!(
            "{{\"block\":\"{}\",\"dim\":\"{}\",\"x\":{},\"y\":{},\"z\":{}}}",
            json_esc(&c.block), json_esc(&dim), c.x, c.y, c.z
        ).as_bytes());
    }

    // 3) annonce l'objet construit.
    emit("object.built", format!(
        "{{\"name\":\"{}\",\"count\":{},\"bytes\":{},\"addr\":\"{:016x}\"}}",
        json_esc(&name), cubions.len(), canon.len(), addr
    ).as_bytes());
    log(&format!("object '{name}': {} cubions posés, addr {:016x}", cubions.len(), addr));
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("object: init (assembler des cubions en objet = un tsoin ; pose => déclenche les bions blocs)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
    let topic = decode_str(tp, tl);
    let payload = decode_str(pp, pl);
    if topic == "object.build" {
        build_object(&payload);
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("object: goodbye");
}
