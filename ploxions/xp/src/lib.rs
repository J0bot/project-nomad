//! `xp` ploxion — le moteur d'EXPÉRIENCE du labo.
//!
//! Le labo, c'est l'endroit où l'on crée tout le temps des tsoins. Ce ploxion en fait un jeu :
//! - **on gagne de l'XP en CRÉANT des tsoins** : il s'abonne à `tsoin.recorded` → +XP par tsoin ;
//! - **on gagne de l'XP en ÉTANT PRÉSENT** : `session.start`/`session.end` (envoyés par le site avec
//!   un timestamp) → le temps d'utilisation devient de l'XP, et **chaque session est gravée en tsoin**.
//! Il émet `xp.gain` (à chaque gain, pour un toast) et `xp.state` (le total : xp, niveau, tsoins,
//! secondes, sessions) que le dashboard Minecraft affiche.
//!
//! L'horloge vient du site (les events portent `t` en secondes) : un ploxion WASM est déterministe,
//! il n'a pas d'horloge — il fait l'arithmétique sur les timestamps reçus. Garde anti-boucle : il NE
//! compte PAS ses propres tsoins de session (`jose:session:*`) ni `xp:*`.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{json_num, json_str, decode_str, tsoin_record};

export_manifest!(
    r#"{"id":"xp","version":"1.0.0","capabilities":[],"provides":["xp.gain","xp.state","tsoin.record"],"requires":["tsoin.recorded","session.start","session.end"],"children_types":[],"parent_types":[]}"#
);

/// XP par tsoin créé.
const XP_PAR_TSOIN: u64 = 10;
/// 1 XP par tranche de présence (secondes) : ici 1 XP / 10 s.
const SECONDES_PAR_XP: u64 = 10;

#[derive(Default)]
struct State {
    xp: u64,
    tsoins: u64,
    secondes: u64,
    sessions: u64,
    session_start: f64,
    en_session: bool,
}

thread_local! {
    static S: RefCell<State> = RefCell::new(State::default());
}

/// Niveau à la Minecraft : croît comme la racine de l'XP.
fn niveau(xp: u64) -> u64 {
    let mut lvl = 1u64;
    while (lvl * lvl) * 100 <= xp {
        lvl += 1;
    }
    lvl
}

fn state_json(s: &State) -> String {
    format!(
        "{{\"xp\":{},\"niveau\":{},\"tsoins\":{},\"secondes\":{},\"sessions\":{},\"en_session\":{}}}",
        s.xp,
        niveau(s.xp),
        s.tsoins,
        s.secondes,
        s.sessions,
        s.en_session
    )
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("xp: init (moteur d'experience ; +XP par tsoin cree + par presence ; grave les sessions)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = decode_str(topic_ptr, topic_len);
    let payload = decode_str(payload_ptr, payload_len);

    match topic.as_str() {
        "tsoin.recorded" => {
            // Garde anti-boucle : ne pas compter mes propres tsoins de session / xp.
            let name = json_str(&payload, "name").unwrap_or_default();
            if name.starts_with("jose:session") || name.starts_with("xp:") {
                return;
            }
            let st = S.with(|s| {
                let mut s = s.borrow_mut();
                s.xp += XP_PAR_TSOIN;
                s.tsoins += 1;
                state_json(&s)
            });
            emit(
                "xp.gain",
                format!("{{\"raison\":\"tsoin\",\"montant\":{XP_PAR_TSOIN},\"nom\":\"{name}\"}}")
                    .as_bytes(),
            );
            emit("xp.state", st.as_bytes());
        }
        "session.start" => {
            let t = json_num(&payload, "t").unwrap_or(0.0);
            let st = S.with(|s| {
                let mut s = s.borrow_mut();
                s.session_start = t;
                s.en_session = true;
                s.sessions += 1;
                state_json(&s)
            });
            log("xp: session.start (José arrive)");
            emit("xp.state", st.as_bytes());
        }
        "session.end" => {
            let t = json_num(&payload, "t").unwrap_or(0.0);
            let (gain, start, st) = S.with(|s| {
                let mut s = s.borrow_mut();
                let dur = if s.en_session && t > s.session_start {
                    (t - s.session_start) as u64
                } else {
                    0
                };
                let start = s.session_start as u64;
                s.secondes += dur;
                let gain = dur / SECONDES_PAR_XP;
                s.xp += gain;
                s.en_session = false;
                (gain, start, state_json(&s))
            });
            // Grave la session comme un tsoin (tout est un tsoin).
            let body = format!("{{\"start\":{start},\"end\":{},\"dur_s\":{}}}", t as u64, (t as u64).saturating_sub(start));
            tsoin_record(&format!("jose:session:{start}"), body.as_bytes());
            emit(
                "xp.gain",
                format!("{{\"raison\":\"presence\",\"montant\":{gain}}}").as_bytes(),
            );
            log("xp: session.end (José part) -> session gravee en tsoin");
            emit("xp.state", st.as_bytes());
        }
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let xp = S.with(|s| s.borrow().xp);
    log(&format!("xp: goodbye (xp total={xp})"));
}
