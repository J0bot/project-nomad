//! `tsoin` ploxion — the **versioning / state layer** of the xerboxion-core.
//!
//! This ploxion wraps the REAL `tsoin` engine (the record/replay timeline:
//! [`Recorder`] + [`Timeline`], XOR-delta residue over a content-addressed
//! BLAKE3 store) and exposes it on the XERB0XI0N bus. It is genuinely stateful:
//! every recorded state is stored as a delta from the previous one, and every
//! replay reconstructs the original bytes **bit-exact** by walking the Merkle
//! timeline — all inside this ploxion's OWN wasm linear memory / `Store`.
//!
//! ## Bus contract
//!
//! - requires `tsoin.record` — payload `{"name": <str>, "bytes": <hex>}`.
//!   Records a new state (one frame appended to the timeline). Replies
//!   `tsoin.recorded` `{"id": <n>, "name": <str>, "deltaStored": <bytes>,
//!   "cumulative": <total-distinct-delta-bytes>}`.
//! - requires `tsoin.replay` — payload `{"id": <n>}`. Reconstructs that state's
//!   bytes bit-exact and replies `tsoin.replayed` `{"id": <n>, "bytes": <hex>}`.
//!
//! `id` is a small sequential integer the ploxion assigns to each recorded
//! state; internally it maps `id -> timeline node Hash`, so the client never has
//! to know about BLAKE3 hashes — it just records and replays by index.
//!
//! Bytes travel as **lowercase hex** inside the JSON payload (the host treats
//! payloads as opaque bytes; hex keeps them JSON-safe end to end).

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;

use ploxion_sdk::{emit, export_manifest, log, read_args};
use ploxion_sdk::bions::{json_esc as json_escape, json_str, to_hex, json_uint as json_u64};
use tsoin::{Coords, Recorder, Timeline};

// PLC manifest: provides the two reply topics, requires the two request topics.
export_manifest!(
    r#"{"id":"tsoin","version":"1.0.0","provides":["tsoin.recorded","tsoin.replayed","tsoin.listed"],"requires":["tsoin.record","tsoin.replay","tsoin.list"],"children_types":[],"parent_types":[]}"#
);

/// The ploxion's whole state: the real engine recorder + a linear timeline head,
/// plus the `id -> node hash` index so states can be replayed by integer id.
/// Lives in this ploxion's own wasm memory; isolated from every other ploxion.
struct State {
    recorder: Recorder,
    timeline: Timeline,
    /// `heads[i]` is the timeline node hash of the state with id `i`.
    heads: Vec<tsoin::Hash>,
    /// `names[i]` is the name passed at record time for the state with id `i`.
    /// Kept in record order so `tsoin.list` can echo (name, id) pairs.
    names: Vec<String>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Build a fresh, empty engine `State`. Single source of truth for State
/// construction, shared by `plc_init` and the lazy get-or-init path so they can
/// never drift.
fn new_state() -> State {
    State {
        recorder: Recorder::new(),
        timeline: Timeline::new(),
        heads: Vec::new(),
        names: Vec::new(),
    }
}

/// Run `f` against the engine `State`, **lazily building it on first use** if
/// the cell currently holds `None`.
///
/// This is what makes the ploxion boot-robust: a `tsoin.record` event can be
/// routed to us synchronously DURING another ploxion's `plc_init` — i.e. before
/// our OWN `plc_init` has run. Without lazy-init that record would be a silent
/// no-op (`State` is `None`) and the boot-time tsoins would be lost until a
/// manual re-poll. With it, the very first record graves a frame and stands up
/// the real engine; a later `plc_init` then becomes a no-op (see there).
fn with_state_mut<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| {
        let mut guard = s.borrow_mut();
        let st = guard.get_or_insert_with(new_state);
        f(st)
    })
}

// --- tiny hex codec (no extra deps; ploxions stay lean) ---------------------

fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.as_bytes();
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut i = 0;
    while i < s.len() {
        let hi = (s[i] as char).to_digit(16)?;
        let lo = (s[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

// --- minimal JSON field extraction (payloads are flat + ploxion-authored) ---

// --- PLC lifecycle ----------------------------------------------------------

#[no_mangle]
pub extern "C" fn plc_init() {
    // IDEMPOTENT: only build State if it is still `None`. If boot-time records
    // already lazy-built it (see `with_state_mut`), DO NOT reset it — replacing
    // it here would wipe every tsoin recorded before init.
    let already = STATE.with(|s| {
        let mut guard = s.borrow_mut();
        let already = guard.is_some();
        guard.get_or_insert_with(new_state);
        already
    });
    if already {
        log("tsoin: init (state already lazy-built by boot-time records; preserved)");
    } else {
        log("tsoin: init (real engine: Recorder + Timeline, XOR-delta + BLAKE3 store)");
    }
}

/// Health: ok only once init built the engine state.
#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    STATE.with(|s| if s.borrow().is_some() { 0 } else { 1 })
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let n = STATE.with(|s| s.borrow().as_ref().map(|st| st.heads.len()).unwrap_or(0));
    log(&format!("tsoin: goodbye (held {n} recorded state(s))"));
    // Drop the engine state explicitly; the host also drops the whole Store.
    STATE.with(|s| *s.borrow_mut() = None);
}

// --- bus handlers -----------------------------------------------------------

fn handle_record(payload: &str) {
    let name = json_str(payload, "name").unwrap_or_else(|| "?".to_string());
    let hex = match json_str(payload, "bytes") {
        Some(h) => h,
        None => {
            log("tsoin: record dropped — payload has no \"bytes\"");
            return;
        }
    };
    let bytes = match from_hex(&hex) {
        Some(b) => b,
        None => {
            log("tsoin: record dropped — \"bytes\" is not valid hex");
            return;
        }
    };

    // Lazy get-or-init: a record arriving BEFORE plc_init still stands up the
    // real engine and graves its frame (boot-robust), instead of silently
    // dropping. `with_state_mut` always yields a live `State`, so a record
    // always graves and always emits `tsoin.recorded`.
    let json = with_state_mut(|st| {
        let before = st.recorder.store_footprint_raw();
        // Record one new frame on the real engine timeline (delta vs previous).
        st.timeline.record(&mut st.recorder, &bytes, Coords::default());
        let head = st.timeline.head().expect("just recorded a frame");
        let id = st.heads.len() as u64;
        st.heads.push(head);
        st.names.push(name.clone());
        let after = st.recorder.store_footprint_raw();
        let delta_stored = after.saturating_sub(before);

        log(&format!(
            "tsoin: recorded id={id} name='{name}' ({} bytes) -> delta {delta_stored}B stored, cumulative {after}B, node {}",
            bytes.len(),
            head.short()
        ));
        format!(
            "{{\"id\":{id},\"name\":\"{name}\",\"deltaStored\":{delta_stored},\"cumulative\":{after}}}"
        )
    });

    emit("tsoin.recorded", json.as_bytes());
}

fn handle_replay(payload: &str) {
    let id = match json_u64(payload, "id") {
        Some(i) => i,
        None => {
            log("tsoin: replay dropped — payload has no integer \"id\"");
            return;
        }
    };

    let reply = STATE.with(|s| {
        let guard = s.borrow();
        let st = guard.as_ref()?;
        let head = match st.heads.get(id as usize) {
            Some(h) => *h,
            None => {
                log(&format!("tsoin: replay dropped — no state with id={id}"));
                return None;
            }
        };
        // BIT-EXACT reconstruction by walking the Merkle timeline.
        let bytes = st.recorder.replay(&head);
        let hex = to_hex(&bytes);
        log(&format!(
            "tsoin: replayed id={id} -> {} bytes (bit-exact from node {})",
            bytes.len(),
            head.short()
        ));
        Some(format!("{{\"id\":{id},\"bytes\":\"{hex}\"}}"))
    });

    if let Some(json) = reply {
        emit("tsoin.replayed", json.as_bytes());
    }
}

/// List every state recorded this session, as `(name, id)` pairs in record
/// order. Payload is ignored. Lets `/replicate` reference the data-tsoins this
/// ploxion holds when assembling a `ReplicaBundle` (code + data).
fn handle_list() {
    // Tolerate None gracefully: a list before any record (State still `None`)
    // returns an explicit empty bundle rather than silence, so consumers like
    // `/replicate` can distinguish "empty" from "not wired". Read-only, so this
    // does not lazy-init State.
    let json = STATE.with(|s| match s.borrow().as_ref() {
        Some(st) => {
            let count = st.names.len();
            let entries: Vec<String> = st
                .names
                .iter()
                .enumerate()
                .map(|(id, name)| format!("{{\"name\":\"{}\",\"id\":{id}}}", json_escape(name)))
                .collect();
            log(&format!("tsoin: listed {count} recorded state(s)"));
            format!("{{\"count\":{count},\"entries\":[{}]}}", entries.join(","))
        }
        None => {
            log("tsoin: listed 0 recorded state(s) (state not yet built)");
            "{\"count\":0,\"entries\":[]}".to_string()
        }
    });

    emit("tsoin.listed", json.as_bytes());
}

#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let payload = String::from_utf8_lossy(payload);
    match topic {
        b"tsoin.record" => handle_record(&payload),
        b"tsoin.replay" => handle_replay(&payload),
        b"tsoin.list" => handle_list(),
        _ => {}
    }
}
