//! `tester` ploxion — tester des ploxions, et graver le tsoin de leur comportement.
//!
//! Sur `test.run` (payload `{"target":"science","in":"science.run","payload":"{...}","out":"science.result"}`)
//! il **drive** la cible (émet `in`/`payload`), **attend** la réponse sur `out`, puis **grave un tsoin**
//! `test:ploxion:<target>` qui capture l'input ET l'output réels — le *comportement* du ploxion, rejouable
//! et envoyable. Émet aussi `test.result {target, ok, out}`. C'est l'analogue « test unitaire » du bus :
//! une cible + une entrée connue + la sortie observée = un tsoin de test. Combiné au `/replicate` (qui
//! porte le wasm), on obtient le tsoin COMPLET d'un ploxion (code + comportement).
//!
//! Pas de capability. La garde anti-boucle : ne grave pas de tsoin pour un `out` qu'il n'attendait pas.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{esc, json_str, decode_str, tsoin_record};

// requires inclut les topics de SORTIE des ploxions testables (pour recevoir leurs reponses).
export_manifest!(
    r#"{"id":"tester","version":"1.0.0","capabilities":[],"provides":["test.result","tsoin.record"],"requires":["test.run","science.result","synth.result","xp.state","index.summary","ultra.detected","tsoin.replayed"],"children_types":[],"parent_types":[]}"#
);

#[derive(Default)]
struct State {
    target: String,
    out_topic: String,
    in_topic: String,
    waiting: bool,
    tests: u32,
}
thread_local! {
    static S: RefCell<State> = RefCell::new(State::default());
}

#[no_mangle]
pub extern "C" fn plc_init() {
    log("tester: init (drive une cible, capture la reponse, grave test:ploxion:<id>)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = decode_str(topic_ptr, topic_len);
    let payload = decode_str(payload_ptr, payload_len);

    if topic == "test.run" {
        // {target, in, payload, out}
        let target = json_str(&payload, "target").unwrap_or_default();
        let in_topic = json_str(&payload, "in").unwrap_or_default();
        let out_topic = json_str(&payload, "out").unwrap_or_default();
        let inner = json_str(&payload, "payload").unwrap_or_else(|| "{}".to_string());
        if target.is_empty() || in_topic.is_empty() || out_topic.is_empty() {
            log("tester: test.run invalide (target/in/out requis)");
            return;
        }
        S.with(|s| {
            let mut s = s.borrow_mut();
            s.target = target.clone();
            s.in_topic = in_topic.clone();
            s.out_topic = out_topic.clone();
            s.waiting = true;
        });
        log(&format!("tester: drive {target} via {in_topic} (attend {out_topic})"));
        // l'inner est un JSON-string deja echappe par l'appelant ; on le passe tel quel sur le bus
        emit(&in_topic, inner.as_bytes());
        return;
    }

    // une reponse possible d'une cible
    let (waiting, target, in_topic, expected) = S.with(|s| {
        let s = s.borrow();
        (s.waiting, s.target.clone(), s.in_topic.clone(), s.out_topic.clone())
    });
    if !waiting || topic != expected {
        return;
    }
    // capture : le ploxion a repondu -> grave le tsoin de son comportement
    let tests = S.with(|s| {
        let mut s = s.borrow_mut();
        s.waiting = false;
        s.tests += 1;
        s.tests
    });
    let tsoin_body = format!(
        "{{\"target\":\"{}\",\"in\":\"{}\",\"out\":\"{}\",\"output\":\"{}\",\"ok\":true}}",
        target,
        in_topic,
        topic,
        esc(&payload)
    );
    tsoin_record(&format!("test:ploxion:{target}"), tsoin_body.as_bytes());
    log(&format!("tester: {target} OK -> tsoin test:ploxion:{target} grave (test #{tests})"));
    emit(
        "test.result",
        format!("{{\"target\":\"{target}\",\"ok\":true,\"out\":\"{topic}\",\"tests\":{tests}}}")
            .as_bytes(),
    );
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let t = S.with(|s| s.borrow().tests);
    log(&format!("tester: goodbye ({t} tests)"));
}
