//! `state-client` ploxion — drives the tsoin state layer over the bus.
//!
//! On `plc_init` it emits three `tsoin.record` requests carrying evolving
//! buffers (a slowly-mutating 16-byte session, the kind temporal-residue
//! storage is built for). It remembers, for each recorded state, the original
//! bytes it sent.
//!
//! As `tsoin.recorded` replies arrive it learns the integer `id` the tsoin
//! ploxion assigned (ids come back in order). Once it has all three, it emits a
//! `tsoin.replay` for an EARLY id (id 0 — the first state). When the
//! `tsoin.replayed` reply comes back, it compares the reconstructed bytes
//! against the original it recorded and logs **OK** (bit-exact) or **MISMATCH**.
//!
//! This is the end-to-end proof: bytes recorded by one ploxion, stored as a
//! delta inside a *separate* ploxion's sandbox, then reconstructed and returned
//! bit-exact — all across the bus + two WASM boundaries.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;

use ploxion_sdk::{emit, export_manifest, log, read_args};
use ploxion_sdk::bions::{json_str, tsoin_record, json_uint as json_u64};

// requires the two reply topics; provides the two request topics.
export_manifest!(
    r#"{"id":"state-client","version":"1.0.0","provides":["tsoin.record","tsoin.replay"],"requires":["tsoin.recorded","tsoin.replayed"],"children_types":[],"parent_types":[]}"#
);

/// The three evolving states this client records. A 16-byte buffer where each
/// successive frame flips a couple of bytes — exactly the slowly-evolving
/// session temporal-residue storage targets (mostly-zero deltas).
fn states() -> [Vec<u8>; 3] {
    let mut a = vec![0x42u8; 16];
    let mut b = a.clone();
    b[3] = 0xAA;
    b[4] = 0xBB;
    let mut c = b.clone();
    c[15] = 0xFF;
    a[0] = 0x01;
    [a, b, c]
}

/// The id we replay to prove old states survive bit-exact (the FIRST one).
const REPLAY_TARGET_ID: u64 = 0;

struct Client {
    /// Original bytes we recorded, in record order (index == assigned id).
    recorded: Vec<Vec<u8>>,
    /// Ids confirmed back via `tsoin.recorded`.
    confirmed: usize,
    /// Whether we've already fired the replay (fire exactly once).
    replay_sent: bool,
    /// Verdict, set when the replayed bytes come back.
    verdict: Option<bool>,
}

thread_local! {
    static CLIENT: RefCell<Client> = const {
        RefCell::new(Client {
            recorded: Vec::new(),
            confirmed: 0,
            replay_sent: false,
            verdict: None,
        })
    };
}

// --- tiny hex codec (mirrors the tsoin ploxion's) ---------------------------

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

// --- lifecycle --------------------------------------------------------------

#[no_mangle]
pub extern "C" fn plc_init() {
    log("state-client: init — recording 3 evolving states via the bus");
    let states = states();
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        for (i, st) in states.iter().enumerate() {
            c.recorded.push(st.clone());
            log(&format!(
                "state-client: emit tsoin.record name=frame{i} ({} bytes)",
                st.len()
            ));
            tsoin_record(&format!("frame{i}"), st);
        }
    });
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 {
    // ok; degraded (2) only if a verdict came back as MISMATCH.
    CLIENT.with(|c| match c.borrow().verdict {
        Some(false) => 2,
        _ => 0,
    })
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
        b"tsoin.recorded" => on_recorded(&payload),
        b"tsoin.replayed" => on_replayed(&payload),
        _ => {}
    }
}

/// A state was recorded; learn its id. Once all three are confirmed, replay an
/// early one.
fn on_recorded(payload: &str) {
    let id = json_u64(payload, "id");
    let total = CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        c.confirmed += 1;
        if let Some(id) = id {
            log(&format!(
                "state-client: tsoin.recorded id={id} (confirmed {}/{})",
                c.confirmed,
                c.recorded.len()
            ));
        }
        c.recorded.len()
    });

    let (ready, already_sent) =
        CLIENT.with(|c| {
            let c = c.borrow();
            (c.confirmed >= total && total > 0, c.replay_sent)
        });

    if ready && !already_sent {
        CLIENT.with(|c| c.borrow_mut().replay_sent = true);
        log(&format!(
            "state-client: all {total} states recorded -> replaying EARLY id={REPLAY_TARGET_ID}"
        ));
        let payload = format!("{{\"id\":{REPLAY_TARGET_ID}}}");
        emit("tsoin.replay", payload.as_bytes());
    }
}

/// The replayed bytes came back; compare against what we originally recorded.
fn on_replayed(payload: &str) {
    let id = match json_u64(payload, "id") {
        Some(i) => i,
        None => return,
    };
    let hex = match json_str(payload, "bytes") {
        Some(h) => h,
        None => return,
    };
    let got = match from_hex(&hex) {
        Some(b) => b,
        None => {
            log("state-client: replayed bytes not valid hex — MISMATCH");
            CLIENT.with(|c| c.borrow_mut().verdict = Some(false));
            return;
        }
    };

    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        let original = c.recorded.get(id as usize).cloned();
        match original {
            Some(orig) if orig == got => {
                log(&format!(
                    "state-client: replay id={id} -> OK (bit-exact, {} bytes match the original)",
                    got.len()
                ));
                c.verdict = Some(true);
            }
            Some(orig) => {
                log(&format!(
                    "state-client: replay id={id} -> MISMATCH (orig {} bytes != got {} bytes)",
                    orig.len(),
                    got.len()
                ));
                c.verdict = Some(false);
            }
            None => {
                log(&format!("state-client: replay id={id} -> MISMATCH (no original recorded)"));
                c.verdict = Some(false);
            }
        }
    });
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    let verdict = CLIENT.with(|c| c.borrow().verdict);
    match verdict {
        Some(true) => log("state-client: goodbye (verdict: OK — bit-exact replay confirmed)"),
        Some(false) => log("state-client: goodbye (verdict: MISMATCH)"),
        None => log("state-client: goodbye (no replay verdict)"),
    }
}
