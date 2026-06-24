//! `ultra-detector` ploxion — le système qui s'observe lui-même.
//!
//! La fractale appliquée à elle-même : la machine à tsoins observe ses PROPRES tsoins. À chaque
//! `tsoin.recorded`, ce ploxion **score l'exceptionnalité** du tsoin par son nom (une clé conceptuelle
//! — découverte / compréhension / convergence / snapshot — pèse lourd ; un record de routine pèse 0).
//! Quand le score franchit le seuil, il émet `ultra.detected` : le système prend conscience, en
//! direct, de ses moments exceptionnels. Pas de boucle (il n'écrit aucun tsoin), pas de capability.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{json_str, decode_str};

export_manifest!(
    r#"{"id":"ultra-detector","version":"1.0.0","capabilities":[],"provides":["ultra.detected"],"requires":["tsoin.recorded"],"children_types":[],"parent_types":[]}"#
);

const SEUIL: u32 = 40;

#[derive(Default)]
struct State {
    vus: u32,
    ultras: u32,
}
thread_local! {
    static S: RefCell<State> = RefCell::new(State::default());
}

/// Score d'exceptionnalité d'un tsoin par son nom.
fn score(name: &str) -> u32 {
    let mut s = 0u32;
    if name.starts_with("xerboxion:ultra-tsoin") {
        s += 100; // un snapshot complet du systeme
    }
    if name.starts_with("comprehension:") {
        s += 60; // une comprehension (vers le point fixe)
    }
    if name.starts_with("knowledge:") {
        s += 40; // une decouverte / synthese
    }
    if name.starts_with("jose:grammaire") || name.starts_with("jose:braindump") {
        s += 30; // la vision de Jose
    }
    // clés conceptuelles nommees explicitement
    match name {
        "paper:commun:carte"
        | "tsoin:index:tout"
        | "cahier:rs-flay:7:page-34"
        | "cahier:rs-flay:7:synthese" => s += 50,
        _ => {}
    }
    s
}

/// Un niveau lisible pour l'humain.
fn niveau(s: u32) -> &'static str {
    if s >= 100 {
        "LEGENDAIRE"
    } else if s >= 60 {
        "EXCEPTIONNEL"
    } else {
        "ULTRA"
    }
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("ultra-detector: init (le systeme s observe ; flague les ultra tsoins en direct)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = decode_str(topic_ptr, topic_len);
    if topic != "tsoin.recorded" {
        return;
    }
    let payload = decode_str(payload_ptr, payload_len);
    let name = match json_str(&payload, "name") {
        Some(n) => n,
        None => return,
    };
    let sc = score(&name);
    let (vus, ultras) = S.with(|s| {
        let mut s = s.borrow_mut();
        s.vus += 1;
        if sc >= SEUIL {
            s.ultras += 1;
        }
        (s.vus, s.ultras)
    });
    if sc >= SEUIL {
        let niv = niveau(sc);
        log(&format!("ultra-detector: ULTRA TSOIN detecte ({niv}, score {sc}) : {name} [{ultras}/{vus}]"));
        emit(
            "ultra.detected",
            format!(
                "{{\"name\":\"{name}\",\"score\":{sc},\"niveau\":\"{niv}\",\"ultras\":{ultras},\"vus\":{vus}}}"
            )
            .as_bytes(),
        );
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let (v, u) = S.with(|s| {
        let s = s.borrow();
        (s.vus, s.ultras)
    });
    log(&format!("ultra-detector: goodbye ({u} ultra / {v} tsoins vus)"));
}
