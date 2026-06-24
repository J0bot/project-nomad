//! `player` ploxion — **le DEROULAGE des tsoins** : rejouer une SEQUENCE de
//! tsoins DANS LE TEMPS, pas un instant isole.
//!
//! Jose (#596) : c'est « la partie la plus dure, non faite cote core ». La
//! machine a tsoins savait deja GRAVER (`diff` -> residu minimal), ADRESSER
//! (`tsoin-store` : `addr64(gen, residu)`), et REJOUER **UN** tsoin (`generator` :
//! `residu -> reel`). Mais un tsoin tout seul est un **instantane mort**. Le
//! `player` deroule une **SEQUENCE ORDONNEE** de tsoins, chacun a son temps
//! logique `t`, au fil de l'horloge : « rejouer une journee », pas « rejouer un
//! instant ». Il FERME la boucle de la machine a tsoins :
//!
//! ```text
//!   record (diff: reel -> residu)
//!     -> store (tsoin-store: residu -> adresse)
//!       -> REPLAY (player: deroule la sequence d'adresses dans le temps
//!                  -> tsoin.get -> tsoin-store ressort le residu
//!                  -> generator: residu -> reel)
//! ```
//!
//! ## Contrat de bus
//!
//! **requires** (entrants) :
//! - `player.load` — `{"tsoins":[{"t":<u64>,"addr":<hex64>}, ...]}`. Charge une
//!   SEQUENCE a rejouer : chaque tsoin a un temps logique `t` (quand le rejouer)
//!   et une `addr` content-addressed (quoi rejouer — l'`addr64` que `tsoin-store`
//!   ressort). La sequence est TRIEE par `(t, addr)` a la charge (le bion
//!   [`SeqCursor::new`]) ⇒ le deroulage est deterministe quel que soit l'ordre
//!   d'arrivee. `addr` est 16 hex (prefixe `0x` tolere) ; un item sans `t`/`addr`
//!   lisible est ignore (et logge). Recharger REMPLACE la sequence et remet le
//!   curseur a 0. Repond `player.loaded {"count":<n>,"mode":<str>}`.
//! - `player.play` — pilote le mode de lecture (etat thread_local, PAS de re-emit
//!   immediat ; c'est `clock.now` qui cadence). Formes acceptees :
//!   * `{"speed":<u64>}` ⇒ PLAY a cette vitesse (combien de pas de `t` on
//!     consomme par `clock.now` ; `0` ⇒ traite comme `1`). `{}` ⇒ PLAY vitesse 1.
//!   * `{"pause":true}` (ou le payload brut `"pause"`) ⇒ PAUSE (le curseur fige ;
//!     `clock.now` ne deroule plus rien jusqu'au prochain PLAY).
//!   * `{"loop":true}` (ou le payload brut `"loop"`) ⇒ active le LOOP (en fin de
//!     sequence, le curseur revient au debut) ; `{"loop":false}` le coupe.
//!
//!   Dans tous les cas, repond `player.state` (l'etat courant).
//! - `player.seek` — `{"t":<u64>}` : **SCRUB**. Repositionne le curseur juste
//!   APRES tous les tsoins de temps `<= t` (rejouer « depuis t » sans re-emettre
//!   le passe). Repond `player.state`.
//! - `clock.now` — `{"t":<u64>, ...}` : **L'HORLOGE GATED** de `clock-coherence`.
//!   A CHAQUE battement, si PLAY est actif, on deroule TOUS les tsoins dus a ce
//!   `t` (le bion [`schedule`]) : pour chacun on emet
//!   `player.tick {t,beat,addr}` PUIS on RE-EMET `tsoin.get {addr}` (pour que
//!   `tsoin-store` ressorte le tsoin et que `generator` le rejoue). `t` est le
//!   temps logique de l'item ; `beat` est le `clock.now` REEL de ce battement
//!   (ils different sous `speed>1`, ou plusieurs `t` sortent dans un battement).
//!   En fin de sequence : `loop` ⇒ rewind ; sinon le player s'arrete (et passe en
//!   mode "ended"). Si `clock.now` RECULE (p.ex. `clock.reset` -> t=0), le player
//!   RESYNC son curseur (seek sur le nouveau `t` cible) au lieu de figer.
//!
//! **provides** : `player.tick`, `tsoin.get`, `player.loaded`, `player.state`,
//! `player.ended`.
//!
//! ## Mecanique du curseur (deterministe)
//! Un [`SeqCursor`] sur la sequence triee par `(t, addr)`. Le `t` consomme est
//! `clock_t * speed` (l'horloge donne le battement, `speed` dilate l'echelle) :
//! on derive jusqu'a ce `t` cible via [`schedule`]. `pause` fige le curseur ;
//! `loop` reboucle ; `seek` repositionne. **AUCUN wall-clock, AUCUN alea** : tout
//! vient de la sequence triee + de la suite de `t` de `clock.now`. D'ou la
//! propriete-cle : **meme sequence + meme suite de `clock.now` ⇒ memes
//! `player.tick`** (le player est REJOUABLE, comme tout le reste de la machine).
//!
//! ## Etat
//! `thread_local` : la sequence (curseur), le mode (paused/playing/ended), la
//! vitesse, le flag loop. Vit dans la propre memoire WASM du ploxion (isolee).
//! L'hote drop tout le Store au goodbye. `player.load` recree l'etat.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;

use ploxion_sdk::bions::{
    array_body, decode_event, json_int, json_str, schedule, split_objects, SeqCursor, SeqItem,
};
use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};

// PLC manifest : on PRODUIT le tick cadence + re-emet tsoin.get (pour rejouer) ;
// on CONSOMME le pilotage (load/play/seek) ET l'horloge (clock.now). Pas de
// capability (calcul pur sur le bus, aucun reseau ; aligne sur clock-coherence /
// generator / tsoin-store).
export_manifest!(
    r#"{"id":"player","version":"1.0.0","provides":["player.tick","tsoin.get","player.loaded","player.state","player.ended"],"requires":["player.load","player.play","player.seek","clock.now"],"children_types":[],"parent_types":[]}"#
);

// --- le MODE de lecture ------------------------------------------------------

/// L'etat de lecture du player. `Paused` au demarrage (rien ne se deroule tant
/// qu'on n'a pas `player.play`). `Playing` deroule sur chaque `clock.now`.
/// `Ended` = la sequence est epuisee sans `loop` (un `play`/`seek`/`load` en
/// ressort).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Paused,
    Playing,
    Ended,
}

impl Mode {
    /// La string du mode pour les payloads/log (stable, deterministe).
    fn as_str(self) -> &'static str {
        match self {
            Mode::Paused => "paused",
            Mode::Playing => "playing",
            Mode::Ended => "ended",
        }
    }
}

/// L'etat complet du player : le curseur de sequence + le mode + la vitesse + le
/// flag loop. `speed` = combien de pas de `t` on consomme par `clock.now` (>= 1).
/// `last_clock_t` = le dernier `clock.now` vu (`None` avant le 1er battement),
/// pour DETECTER un retour-arriere de l'horloge (`clock.reset` -> t=0) et resync.
struct Player {
    cursor: SeqCursor,
    mode: Mode,
    speed: u64,
    looping: bool,
    last_clock_t: Option<u64>,
}

impl Player {
    fn new() -> Self {
        Self {
            cursor: SeqCursor::empty(),
            mode: Mode::Paused,
            speed: 1,
            looping: false,
            last_clock_t: None,
        }
    }
}

thread_local! {
    static PLAYER: RefCell<Player> = RefCell::new(Player::new());
}

// --- cycle de vie PLC (multi-topic : on garde notre propre plc_on_event) -----
ploxion_lifecycle!(
    init: "player: init (le DEROULAGE des tsoins — rejouer une SEQUENCE dans le temps, pas un instant ; curseur SeqCursor sur sequence triee (t,addr) ; clock.now gated cadence -> player.tick + tsoin.get ; deterministe/rejouable ; ferme record->store->REPLAY)",
    goodbye: "player: goodbye",
);

// --- helpers d'adresse -------------------------------------------------------

/// Parse une `addr` hex 64-bit (16 hex, prefixe `0x` tolere) en `u64`. `None`
/// (sans log ici — l'appelant logge avec le contexte) si illisible.
fn parse_addr(s: &str) -> Option<u64> {
    u64::from_str_radix(s.strip_prefix("0x").unwrap_or(s), 16).ok()
}

/// Lit un **booleen JSON** `"key":true|false` (token nu, PAS une string) du
/// payload : `Some(true)`/`Some(false)` selon le token, `None` si la cle est
/// absente. Le bus du player porte des flags `{"pause":true}`/`{"loop":false}` —
/// `json_str`/`json_int` ne savent pas lire un bare `true`/`false`, ce petit
/// lecteur si. (Garde local : un flag booleen est specifique au pilotage du
/// player ; on ne pollue pas la BIONLIB tant qu'un 2e ploxion n'en a pas besoin.)
fn json_bool(payload: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\"");
    let start = payload.find(&needle)? + needle.len();
    let after = payload[start..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

// --- handlers de bus ---------------------------------------------------------

/// `player.load {"tsoins":[{"t","addr"},...]}` → charge une SEQUENCE a rejouer.
/// Les items sont TRIES par `(t, addr)` par [`SeqCursor::new`] (deroulage
/// deterministe quel que soit l'ordre d'arrivee). Recharger REMPLACE tout et
/// remet le curseur a 0 (mais conserve `speed`/`loop` deja regles). Repond
/// `player.loaded`.
fn handle_load(payload: &str) {
    // Le corps du tableau "tsoins" (bion array_body, conscient des chaines + de
    // la profondeur), puis ses objets de premier niveau (bion split_objects).
    let body = match array_body(payload, "tsoins") {
        Some(b) => b,
        None => {
            log("player: load dropped — payload has no \"tsoins\" array");
            return;
        }
    };

    let mut items: Vec<SeqItem> = Vec::new();
    let mut skipped = 0usize;
    for obj in split_objects(body) {
        // `t` : le temps logique (>= 0 ; un t absent/negatif ⇒ item ignore).
        let t_raw = json_int(obj, "t", -1);
        if t_raw < 0 {
            skipped += 1;
            continue;
        }
        // `addr` : l'adresse content-addressed du tsoin a ressortir.
        let addr = match json_str(obj, "addr").as_deref().and_then(parse_addr) {
            Some(a) => a,
            None => {
                skipped += 1;
                continue;
            }
        };
        items.push(SeqItem::new(t_raw as u64, addr));
    }

    let count = items.len();
    PLAYER.with(|p| {
        let mut pl = p.borrow_mut();
        pl.cursor = SeqCursor::new(items); // trie + curseur a 0
        // Une sequence vide ⇒ on reste en pause ; sinon on garde le mode courant,
        // sauf qu'on sort de "ended" (une nouvelle sequence est rejouable).
        if pl.mode == Mode::Ended {
            pl.mode = Mode::Paused;
        }
    });

    let mode = PLAYER.with(|p| p.borrow().mode.as_str());
    log(&format!(
        "player: load -> {} tsoin(s) en sequence ({} ignore(s)), mode={}",
        count, skipped, mode
    ));
    emit(
        "player.loaded",
        format!("{{\"count\":{},\"mode\":\"{}\"}}", count, mode).as_bytes(),
    );
}

/// `player.play {speed?|pause|loop}` → pilote le MODE. N'emet PAS de tick ici
/// (c'est `clock.now` qui cadence) : on ne fait que regler l'etat, puis on repond
/// `player.state`. Voir le contrat dans la doc du module pour les formes.
fn handle_play(payload: &str) {
    let trimmed = payload.trim();
    let raw_pause = trimmed == "pause" || trimmed == "\"pause\"";
    let raw_loop = trimmed == "loop" || trimmed == "\"loop\"";

    PLAYER.with(|p| {
        let mut pl = p.borrow_mut();

        // PAUSE (objet {"pause":true} ou payload brut "pause").
        if raw_pause || json_bool(payload, "pause") == Some(true) {
            pl.mode = Mode::Paused;
            log("player: play -> PAUSE (curseur fige)");
            return;
        }

        // LOOP on/off ({"loop":true|false} ou payload brut "loop" ⇒ on).
        if raw_loop {
            pl.looping = true;
        } else if let Some(v) = json_bool(payload, "loop") {
            pl.looping = v;
        }

        // SPEED (optionnel) : 0 ⇒ traite comme 1 (eviter un player qui n'avance
        // jamais). Absent ⇒ vitesse inchangee (par defaut 1 a la construction).
        let sp = json_int(payload, "speed", -1);
        if sp >= 0 {
            pl.speed = (sp as u64).max(1);
        }

        // Un play (non-pause) (re)met en PLAYING — et sort de "ended" pour
        // permettre de relancer apres un seek/load.
        pl.mode = Mode::Playing;
        log(&format!(
            "player: play -> PLAYING (speed={}, loop={})",
            pl.speed, pl.looping
        ));
    });

    emit_state();
}

/// `player.seek {"t":<u64>}` → SCRUB : repositionne le curseur juste APRES les
/// tsoins de temps `<= t` (le bion [`SeqCursor::seek`]). Ne deroule rien
/// (n'emet pas de tick) : c'est un repositionnement ; le prochain `clock.now`
/// reprend a partir de la. Sort de "ended" si on recule avant la fin.
fn handle_seek(payload: &str) {
    let t = json_int(payload, "t", -1);
    if t < 0 {
        log("player: seek dropped — payload has no valid \"t\"");
        return;
    }
    let t = t as u64;
    PLAYER.with(|p| {
        let mut pl = p.borrow_mut();
        pl.cursor.seek(t);
        // Si le scrub nous laisse avant la fin, on n'est plus "ended".
        if pl.mode == Mode::Ended && !pl.cursor.exhausted() {
            pl.mode = Mode::Paused;
        }
        log(&format!(
            "player: seek t={} -> curseur pos={}/{}",
            t,
            pl.cursor.pos(),
            pl.cursor.len()
        ));
    });
    emit_state();
}

/// `clock.now {"t":<u64>, ...}` → **LE BATTEMENT**. Si PLAY est actif, on deroule
/// tous les tsoins dus a `t_cible = clock_t * speed` (le bion [`schedule`]) :
/// pour chacun on emet `player.tick {t,addr}` PUIS on RE-EMET `tsoin.get {addr}`.
/// En fin de sequence : `loop` ⇒ rewind (et on continue a derouler ce meme
/// battement depuis le debut) ; sinon le player passe en "ended" et emet
/// `player.ended`. Deterministe : meme suite de `clock.now` ⇒ memes ticks.
fn handle_clock_now(payload: &str) {
    let clock_t = json_int(payload, "t", -1);
    if clock_t < 0 {
        // Un clock.now sans `t` lisible : on l'ignore silencieusement (le bus peut
        // porter d'autres formes ; on ne casse pas le deroulage en cours).
        return;
    }
    let clock_t = clock_t as u64;

    // On collecte les items a derouler SOUS le borrow, puis on emet HORS borrow
    // (eviter de re-rentrer dans PLAYER si un handler reagissait — robustesse).
    enum Outcome {
        Idle,                       // pas en PLAY (ou deja ended) : rien
        Ticks(Vec<SeqItem>, bool),  // items a derouler ; bool = a-t-on fini (ended) ?
    }

    let outcome = PLAYER.with(|p| {
        let mut pl = p.borrow_mut();

        // Le `t` cible = battement * vitesse (speed dilate l'echelle de temps).
        // saturating_mul : pas de wrap (un speed enorme sature, le deroulage
        // reste monotone et borne par la sequence).
        let target = clock_t.saturating_mul(pl.speed.max(1));

        // RESYNC sur horloge non-monotone : `clock.now` est cense MONTER, mais
        // `clock.reset` re-emet `clock.now {t:0}` et l'horloge gated peut figer.
        // Si le battement RECULE par rapport au dernier vu, on REPOSITIONNE le
        // curseur sur le nouveau `target` (seek, O(log n)) au lieu de figer
        // silencieusement (le curseur etait au-dela ⇒ due_count=0 ⇒ gel). Le
        // deroulage reste deterministe (seek est pur) et le scrub-back de
        // l'horloge se reflete vraiment dans le player. On (re)note le battement.
        let backward = pl.last_clock_t.is_some_and(|last| clock_t < last);
        pl.last_clock_t = Some(clock_t);
        if backward {
            pl.cursor.seek(target);
            // Un retour-arriere d'horloge sort le player de "ended" (il y a de
            // nouveau du a venir) ; il reste sinon dans son mode courant.
            if pl.mode == Mode::Ended && !pl.cursor.exhausted() {
                pl.mode = Mode::Paused;
            }
            log(&format!(
                "player: clock RECULE (t={}) -> RESYNC curseur seek(target={}) pos={}/{}",
                clock_t,
                target,
                pl.cursor.pos(),
                pl.cursor.len()
            ));
        }

        if pl.mode != Mode::Playing {
            return Outcome::Idle;
        }

        // Deroule tous les items dus a `target` (prefixe contigu, schedule avance
        // le curseur). C'est UN passage au plus par battement (deterministe,
        // borne).
        let due = schedule(&mut pl.cursor, target);

        // Fin de sequence : soit LOOP (rewind, effectif au PROCHAIN clock.now —
        // pas de re-deroulage du meme battement, sinon boucle infinie a target
        // grand), soit on s'arrete (ENDED). On ne passe en ENDED qu'UNE fois (le
        // curseur est epuise et le loop est coupe).
        let mut ended = false;
        if pl.cursor.exhausted() {
            if pl.looping {
                pl.cursor.reset();
                log("player: fin de sequence -> LOOP (rewind au debut)");
            } else {
                pl.mode = Mode::Ended;
                ended = true;
            }
        }
        Outcome::Ticks(due, ended)
    });

    match outcome {
        Outcome::Idle => {}
        Outcome::Ticks(due, ended) => {
            for it in &due {
                // 1) le TICK cadence : « a t, le tsoin addr est du ». On porte
                //    AUSSI `beat` = le `clock.now` REEL de ce battement : sous
                //    speed>1 plusieurs `it.t` distincts sortent dans un meme
                //    battement (target=clock_t*speed regroupe le prefixe), donc
                //    `t != beat` ; `beat` permet a un consommateur de reconstruire
                //    a quel battement reel chaque tsoin a ete deroule.
                emit(
                    "player.tick",
                    format!(
                        "{{\"t\":{},\"beat\":{},\"addr\":\"{:016x}\"}}",
                        it.t, clock_t, it.addr
                    )
                    .as_bytes(),
                );
                // 2) RE-EMET tsoin.get : tsoin-store ressort le residu (et un
                //    generator branche derriere le rejoue). C'est ICI que le
                //    deroulage FAIT rejouer le tsoin (ferme la boucle REPLAY).
                emit(
                    "tsoin.get",
                    format!("{{\"addr\":\"{:016x}\"}}", it.addr).as_bytes(),
                );
            }
            if !due.is_empty() {
                log(&format!(
                    "player: clock t={} -> {} tsoin(s) deroule(s) (player.tick + tsoin.get)",
                    clock_t,
                    due.len()
                ));
            }
            if ended {
                let count = PLAYER.with(|p| p.borrow().cursor.len());
                log("player: sequence epuisee (pas de loop) -> ENDED");
                emit(
                    "player.ended",
                    format!("{{\"t\":{},\"count\":{}}}", clock_t, count).as_bytes(),
                );
            }
        }
    }
}

/// Emet l'etat courant du player sur `player.state` (transparence du pilotage).
fn emit_state() {
    let (mode, speed, looping, pos, len, next_t) = PLAYER.with(|p| {
        let pl = p.borrow();
        (
            pl.mode.as_str(),
            pl.speed,
            pl.looping,
            pl.cursor.pos(),
            pl.cursor.len(),
            pl.cursor.next_t(),
        )
    });
    // next_t : -1 quand la sequence est epuisee (pas de prochain tsoin).
    let next = next_t.map(|t| t as i64).unwrap_or(-1);
    emit(
        "player.state",
        format!(
            "{{\"mode\":\"{}\",\"speed\":{},\"loop\":{},\"pos\":{},\"count\":{},\"next_t\":{}}}",
            mode, speed, looping, pos, len, next
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
        "player.load" => handle_load(&payload),
        "player.play" => handle_play(&payload),
        "player.seek" => handle_seek(&payload),
        "clock.now" => handle_clock_now(&payload),
        _ => {}
    }
}
