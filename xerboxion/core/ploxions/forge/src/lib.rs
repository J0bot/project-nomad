//! `forge` ploxion — la FORGE **idée → objet**. José : « le but, c'est d'arriver de l'idée à
//! l'objet le plus vite possible avec la machine à tsoins. Tu donnes une idée, tu ajoutes des
//! blocs de bions, et ça te rend un objet complet **avec les instructions pour le construire** —
//! mais tous les blocs existent déjà. » Donc la forge n'INVENTE rien : elle **assemble** des bions
//! existants (cubion / code / modèle 3D / port) et **génère le mode d'emploi déterministe**.
//!
//! Crucial : **sans LLM**. Seul José a accès au LLM ; tout le monde doit pouvoir s'en sortir seul.
//! La forge est un pur algorithme : mêmes (idée, blocs) ⇒ même objet (addr64) ⇒ mêmes instructions.
//!
//! `forge.assemble {idea, blocks:[{kind,id,x?,y?,z?,from?,to?}, …]}` ->
//!   1. content-adresse l'assemblage (idée + blocs canoniques) = `object_addr`, grave la recette ;
//!   2. génère les **instructions** numérotées (une étape par bloc, selon son `kind`) ;
//!   3. matérialise : pour chaque `cubion`, émet `block.place` (déclenche les bions blocs) ;
//!   4. émet `forge.assembled {idea, addr, n, instructions}`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{decode_str, json_str, json_int, json_esc, tsoin_record, addr64,
                         array_body, split_objects};

export_manifest!(
    r#"{"id":"forge","version":"1.0.0","capabilities":[],"provides":["forge.assembled","block.place","tsoin.record"],"requires":["forge.assemble"],"children_types":["block"],"parent_types":[]}"#
);

/// Un bloc de l'assemblage : un bion existant + son rôle dans l'objet (le résidu d'une étape).
struct Block {
    kind: String,
    id: String,
    x: i64, y: i64, z: i64,
    from: String, to: String,
}

fn parse_blocks(payload: &str) -> Vec<Block> {
    let body = match array_body(payload, "blocks") { Some(b) => b, None => return Vec::new() };
    split_objects(body).iter().filter_map(|o| {
        let id = json_str(o, "id").unwrap_or_default();
        let from = json_str(o, "from").unwrap_or_default();
        let to = json_str(o, "to").unwrap_or_default();
        // Un bloc est valide s'il a un id, OU si c'est un câblage (port) défini par from/to.
        if id.is_empty() && (from.is_empty() || to.is_empty()) { return None; }
        Some(Block {
            kind: json_str(o, "kind").unwrap_or_else(|| "bloc".to_string()),
            id,
            x: json_int(o, "x", 0), y: json_int(o, "y", 0), z: json_int(o, "z", 0),
            from, to,
        })
    }).collect()
}

/// Sérialisation canonique (déterministe) : `idea\n` + une ligne par bloc dans l'ordre donné.
/// L'ordre EST l'ordre de construction (la forge respecte la séquence de l'idée).
fn canonical(idea: &str, blocks: &[Block]) -> Vec<u8> {
    let mut s = format!("{idea}\n");
    for b in blocks {
        s.push_str(&format!("{}:{}:{},{},{}:{}>{}\n", b.kind, b.id, b.x, b.y, b.z, b.from, b.to));
    }
    s.into_bytes()
}

/// Le mode d'emploi DÉTERMINISTE : une étape numérotée par bloc, selon son `kind`. Aucune
/// créativité, aucune ambiguïté — un humain (ou un autre algo) suit la liste et obtient l'objet.
fn instructions(blocks: &[Block]) -> String {
    let mut out = String::new();
    for (i, b) in blocks.iter().enumerate() {
        let step = match b.kind.as_str() {
            "cubion" => format!("Poser le cubion '{}' en ({},{},{})", b.id, b.x, b.y, b.z),
            "code"   => format!("Inclure le bion-code '{}'", b.id),
            "model"  => format!("Rendre le modèle 3D '{}'", b.id),
            "port"   => format!("Câbler le port '{}' -> '{}'", b.from, b.to),
            other    => format!("Assembler le bloc {} '{}'", other, b.id),
        };
        out.push_str(&format!("{}. {}\n", i + 1, step));
    }
    out
}

fn assemble(payload: &str) {
    let idea = json_str(payload, "idea").unwrap_or_else(|| "idee".to_string());
    let dim = json_str(payload, "dim").unwrap_or_else(|| "overworld".to_string());
    let blocks = parse_blocks(payload);
    if blocks.is_empty() { log("forge: aucun bloc (rien a assembler)"); return; }

    let canon = canonical(&idea, &blocks);
    let addr = addr64(&format!("forge:{idea}"), &canon);

    // 1) l'objet (la recette) EST un tsoin : grave la sérialisation canonique (rejouable).
    tsoin_record(&format!("forge:{:016x}", addr), &canon);

    // 2) le mode d'emploi.
    let steps = instructions(&blocks);

    // 3) matérialise les cubions -> déclenche les bions blocs (chaine block.place).
    for b in &blocks {
        if b.kind == "cubion" {
            emit("block.place", format!(
                "{{\"block\":\"{}\",\"dim\":\"{}\",\"x\":{},\"y\":{},\"z\":{}}}",
                json_esc(&b.id), json_esc(&dim), b.x, b.y, b.z
            ).as_bytes());
        }
    }

    // 4) annonce l'objet construit + ses instructions (déterministes, sans LLM).
    emit("forge.assembled", format!(
        "{{\"idea\":\"{}\",\"addr\":\"{:016x}\",\"n\":{},\"instructions\":\"{}\"}}",
        json_esc(&idea), addr, blocks.len(), json_esc(steps.trim_end())
    ).as_bytes());
    log(&format!("forge '{idea}': {} blocs -> objet {:016x} (+ {} etapes)", blocks.len(), addr, blocks.len()));
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("forge: init (idee + blocs de bions -> objet complet + instructions deterministes, sans LLM)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
    let topic = decode_str(tp, tl);
    let payload = decode_str(pp, pl);
    if topic == "forge.assemble" {
        assemble(&payload);
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("forge: goodbye");
}
