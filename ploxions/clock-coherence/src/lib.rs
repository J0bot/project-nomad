//! `clock-coherence` ploxion — **l'HORLOGE DU XION**, ou le temps n'est PAS le
//! wall-clock mais la **COHERENCE**.
//!
//! Loi du cahier p.42 (Jose, signal #596) : « le temps n'existe pas, il faut le
//! recreer ; le temps = la coherence = la synchronisation ». Le mur n'a aucun
//! sens ici : le seul temps qui compte pour le xion est *a quel point
//! l'orchestre est synchrone*. La `carte` mesure deja cette coherence EN BATCH
//! (`t = count_du_theme * 1000 / max_count` : la part du theme le plus
//! represente dans un nuage de points, carte/src/lib.rs:132-141). Ce ploxion en
//! fait la version **LIVE** (flux) de la MEME mesure de concentration : une
//! fenetre glissante des topics qui passent sur le bus, et un temps logique qui
//! n'avance QUE quand une adresse domine (l'orchestre est d'accord). Il donne au
//! futur *player* — et a la carte en mode live — une horloge CONFORME a la loi.
//!
//! ## Contrat de bus
//!
//! **requires** : `clock.observe` — deux formes acceptees :
//! - `{"topic":<str>}` (la forme canonique), ou
//! - le payload entier comme STRING BRUTE du topic (tolerance, si ce n'est pas
//!   un objet JSON commencant par `{`).
//! Un objet JSON SANS champ `"topic"` (ex. `{"foo":1}`) est rejete et logge.
//! Un topic = un battement de l'orchestre vu passer sur le bus.
//!
//! Egalement : `clock.reset` — `{}` ou `{"threshold_milli":N}` : recree le
//! temps (fenetre videe, `t=0`, seuil optionnellement re-arme).
//!
//! **provides** : `clock.now` —
//! `{"t":<u64>, "coherence_milli":<0..=1000>, "sync":<bool>, "distinct":<n>, "window":<f>, "cap":<cap>}`.
//! - `t` = le temps LOGIQUE (compteur MONOTONE) — n'a avance ce coup-ci QUE si la
//!   coherence a franchi le seuil (gate). Sinon il est reste FIGE : le temps
//!   n'est pas passe. C'est le NOMBRE de moments d'accord, PAS le niveau courant.
//! - `coherence_milli` = la coherence LIVE de la fenetre (`0..=1000`) = la part
//!   du topic dominant, via [`CoherenceWindow::coherence_milli`].
//! - `sync` = `true` ssi `coherence_milli >= seuil` (l'orchestre est synchrone).
//! - `distinct`/`window`/`cap` = l'etat de la fenetre (transparence de la mesure).
//!
//! ## Mecanique
//! Une **FENETRE GLISSANTE bornee** ([`CoherenceWindow`], ring-buffer) des
//! derniers topics observes (les strings brutes, comme `carte` bucketise sur la
//! string `cat` litterale). A chaque `clock.observe` : on pousse le topic, on
//! recalcule la coherence (part du dominant), on appelle le **gate**
//! ([`GatedTick::gated_tick`]) — qui n'incremente `t` que si la coherence est
//! suffisante — puis on emet `clock.now`. Materialise « le temps n'avance que si
//! l'orchestre est d'accord ». La mesure est INDEPENDANTE du remplissage : pas de
//! warm-up wall-clock qui se reinvite.
//!
//! ## Etat
//! Un `thread_local` : la fenetre + le tick. C'est le SEUL ploxion-horloge a
//! garder un etat de synchronisation vivant (comme `xerbion` garde ses poids).
//! Capacite et seuil sont fixes (cohesion deterministe du xion) ; un `clock.reset`
//! remet la fenetre et le compteur a zero (recommencer le temps).

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::bions::{decode_event, json_int, json_str, CoherenceWindow, GatedTick};
use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};
use std::cell::RefCell;

// PLC manifest : on PRODUIT le temps (clock.now), on CONSOMME les observations
// du bus (clock.observe). Pas de capability (calcul pur, aucun reseau ; aligne
// sur carte / generator / diff).
export_manifest!(
    r#"{"id":"clock-coherence","version":"1.0.0","provides":["clock.now"],"requires":["clock.observe"],"children_types":[],"parent_types":[]}"#
);

// --- parametres de l'horloge (fixes, deterministes) --------------------------
//
// CAP   : la taille de la fenetre glissante (combien de battements recents on
//         garde pour mesurer la synchronisation).
// SEUIL : la coherence (milli) sous laquelle le temps NE PASSE PAS. 500 = il
//         faut qu'au moins la moitie de la fenetre batte sur la MEME adresse
//         (un dominant qui pese >= 50%) pour que le temps avance.
const CAP: usize = 32;
const THRESHOLD_MILLI: u32 = 500;

// --- l'etat vivant de l'horloge ---------------------------------------------
//
// La fenetre de coherence (le pendant LIVE de la mesure batch de carte) + le
// tick gate (le temps logique qui n'avance que sous coherence). thread_local
// car le ploxion est mono-thread dans le store de l'hote (meme pattern que
// xerbion::SELF).
thread_local! {
    static CLOCK: RefCell<(CoherenceWindow, GatedTick)> =
        RefCell::new((CoherenceWindow::new(CAP), GatedTick::new(THRESHOLD_MILLI)));
}

// --- cycle de vie PLC (ploxion_lifecycle! n'emet QUE plc_init/health/goodbye ;
//     on garde notre propre plc_on_event multi-topics, comme diff/generator) ---
ploxion_lifecycle!(
    init: "clock-coherence: init (horloge du xion ou le temps = la coherence ; fenetre glissante des topics + gated_tick : t n'avance que si l'orchestre est synchrone)",
    goodbye: "clock-coherence: goodbye",
);

/// Extrait le topic observe d'un payload `clock.observe`. On prend le champ
/// `"topic"` s'il existe ; sinon, si le payload entier est une string non vide
/// qui n'est PAS un objet JSON, on le prend BRUT (tolerance pour un emetteur qui
/// balance juste le nom du topic). `None` si rien d'exploitable (ex. un objet
/// JSON sans `"topic"`).
fn observed_topic(payload: &str) -> Option<String> {
    if let Some(t) = json_str(payload, "topic") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let trimmed = payload.trim();
    if !trimmed.is_empty() && !trimmed.starts_with('{') {
        return Some(trimmed.to_string());
    }
    None
}

/// `clock.observe {"topic":..}` → pousse le topic dans la fenetre, recalcule la
/// coherence LIVE (part du dominant), fait avancer le temps logique SI (et
/// seulement si) la coherence franchit le seuil (le gate), puis emet
/// `clock.now`. C'est ici que « le temps = la coherence » devient executable :
/// un battement n'avance le temps que s'il s'inscrit dans un orchestre synchrone.
fn handle_observe(payload: &str) {
    let topic = match observed_topic(payload) {
        Some(t) => t,
        None => {
            log("clock-coherence: observe dropped — ni \"topic\" ni string brute");
            return;
        }
    };

    let (t, coherence, sync, distinct, window, cap) = CLOCK.with(|c| {
        let (win, tick) = &mut *c.borrow_mut();
        // 1) observe : pousse le battement dans la fenetre glissante bornee.
        win.observe(&topic);
        // 2) recalcule la coherence LIVE = part du dominant (twin de carte).
        let coherence = win.coherence_milli();
        // 3) LE GATE : le temps logique n'avance QUE si l'orchestre est synchrone.
        let (t, sync) = tick.gated_tick(coherence);
        (t, coherence, sync, win.distinct(), win.len(), win.cap())
    });

    log(&format!(
        "clock-coherence: observe '{}' -> coherence={}/1000 (window {}/{}, {} distincts), {} -> t={}",
        topic,
        coherence,
        window,
        cap,
        distinct,
        if sync { "SYNC (le temps passe)" } else { "dis-coordonne (le temps reste fige)" },
        t
    ));

    // 4) emet le temps courant. Le pendant LIVE de carte.mapped : ici le `t` du
    //    xion est le COMPTE des moments synchrones, et coherence_milli le NIVEAU.
    emit(
        "clock.now",
        format!(
            "{{\"t\":{},\"coherence_milli\":{},\"sync\":{},\"distinct\":{},\"window\":{},\"cap\":{}}}",
            t, coherence, sync, distinct, window, cap
        )
        .as_bytes(),
    );
}

/// `clock.reset` → recommence le temps : fenetre videe, compteur logique a 0.
/// Le seuil (optionnel `{"threshold_milli":N}`) peut etre re-arme ; sinon on
/// garde le seuil par defaut. « Recreer le temps » au sens litteral.
fn handle_reset(payload: &str) {
    let new_threshold = {
        let v = json_int(payload, "threshold_milli", THRESHOLD_MILLI as i64);
        v.clamp(0, 1000) as u32
    };
    CLOCK.with(|c| {
        *c.borrow_mut() = (CoherenceWindow::new(CAP), GatedTick::new(new_threshold));
    });
    log(&format!(
        "clock-coherence: reset — fenetre videe, t=0, seuil={}/1000 (le temps recommence)",
        new_threshold
    ));
    emit(
        "clock.now",
        format!(
            "{{\"t\":0,\"coherence_milli\":0,\"sync\":false,\"distinct\":0,\"window\":0,\"cap\":{}}}",
            CAP
        )
        .as_bytes(),
    );
}

#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let (topic, payload) = decode_event(topic_ptr, topic_len, payload_ptr, payload_len);
    match topic.as_str() {
        "clock.observe" => handle_observe(&payload),
        "clock.reset" => handle_reset(&payload),
        _ => {}
    }
}