//! ploxion-sdk — minimal glue for writing a WASM ploxion that satisfies PLC v1.
//!
//! A ploxion crate using this SDK only has to:
//!   1. provide a manifest JSON via [`export_manifest!`] / [`manifest`],
//!   2. implement `plc_init` / `plc_health` / `plc_on_event` / `plc_goodbye`,
//!   3. call [`emit`] / [`log`] to talk to the host.
//!
//! The SDK provides the `alloc` export and the manifest `(ptr,len)` packing so
//! the host's PLC memory ABI is honoured exactly.
//!
//! This crate has NO dependency on the host. It mirrors the ABI symbol names
//! from the (host-side) `xerboxion-plc` crate; both must stay in sync — that is
//! the whole point of freezing the contract.

#![allow(clippy::missing_safety_doc)]

/// Les **bions partagés** : blocs de code (parsing JSON, échappement, hachage)
/// réutilisés par tous les ploxions au lieu d'être recopiés dans chacun.
pub mod bions;

/// Les **bions arithmétiques** (i32 + f64) : la couche calcul pur, incluse sur le
/// xerboxion depuis les catalogues WASM côté UI. Généré (voir `math.rs`).
pub mod math;

/// Le **BLOCK-BION** : un bloc de Minecraft = un cubion. [`block::BlockDef`] = le
/// résidu par bloc ; `block_ploxion!` génère tout le cycle PLC depuis ce résidu.
pub mod block;

/// Le **MAISON-BION** : une feature de maison autonome = un cubion.
/// [`house::HouseDef`] = le résidu par feature ; `house_ploxion!` génère tout le
/// cycle PLC (manifest/init/on_event + DÉCIDE déclaratif) depuis ce résidu.
pub mod house;

/// Le modèle de **DISSOCIATION** : le giga-tsoin *avant* fragmentation. Fenêtre de
/// tolérance (Siegel) + perte de synthèse (Janet) + double représentation (Brewin)
/// mises dans le code ; la machine garde le résidu entier et **réintègre** par le
/// replay ce que le cerveau a dû scinder.
pub mod dissociation;

/// Le **PROTOCOL-BION** : un protocole reseau = un ploxion (vague 1 du reseau-en-ploxions).
pub mod protocol;
/// Le **PORT-BION** : un port reseau (une prise) = un ploxion (generation paresseuse).
pub mod port;
/// Le **PACKAGE-BION** : un paquet (dpkg/apt) = un ploxion (3e generateur, direction bion-Linux).
pub mod package;

use std::alloc::{alloc as rust_alloc, Layout};

// --- Host imports (env module "xerboxion") ----------------------------------
//
// These are provided by the host's Linker under module "xerboxion".
#[link(wasm_import_module = "xerboxion")]
extern "C" {
    fn plc_emit(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32);
    fn plc_log(ptr: i32, len: i32);
}

// --- Gated host import: plc_fetch (PLC v1.1, capability "net.fetch") ---------
//
// This import is in a SEPARATE `extern` block guarded by the `net-fetch` cargo
// feature, so a ploxion only references `plc_fetch` when it actually intends to
// use the capability. The host links `plc_fetch` ONLY for a ploxion whose
// manifest declares `"net.fetch"`; a ploxion that references this symbol WITHOUT
// declaring the capability is rejected by the host at load (the gate). So: opt
// in to this feature AND declare the capability in your manifest together.
#[cfg(feature = "net-fetch")]
#[link(wasm_import_module = "xerboxion")]
extern "C" {
    fn plc_fetch(
        method_ptr: i32,
        method_len: i32,
        url_ptr: i32,
        url_len: i32,
        body_ptr: i32,
        body_len: i32,
    ) -> i64;
}

/// Emit `payload` on `topic` to the bus.
pub fn emit(topic: &str, payload: &[u8]) {
    unsafe {
        plc_emit(
            topic.as_ptr() as i32,
            topic.len() as i32,
            payload.as_ptr() as i32,
            payload.len() as i32,
        );
    }
}

/// Write a log line to the host journal.
pub fn log(line: &str) {
    unsafe {
        plc_log(line.as_ptr() as i32, line.len() as i32);
    }
}

/// The raw JSON the host returns from a brokered [`fetch`] — a string like
/// `{"status":200,"body":"...","error":"..."}`. The SDK keeps it as the opaque
/// JSON text (the host is the source of truth for the shape); the adapter
/// extracts the fields it cares about with its own tiny parser, exactly like the
/// other ploxions parse bus payloads. Requires the `net-fetch` feature AND the
/// `"net.fetch"` capability in the manifest, or the host rejects the ploxion.
///
/// `status` is the HTTP code, or `0` when the request never produced a response
/// (the host NEVER panics on a network error — `error` then explains why).
#[cfg(feature = "net-fetch")]
pub fn fetch(method: &str, url: &str, body: &[u8]) -> String {
    let packed = unsafe {
        plc_fetch(
            method.as_ptr() as i32,
            method.len() as i32,
            url.as_ptr() as i32,
            url.len() as i32,
            body.as_ptr() as i32,
            body.len() as i32,
        )
    };
    if packed == 0 {
        // Host could not even allocate/return — present it as an unreachable.
        return r#"{"status":0,"body":"","error":"no result"}"#.to_string();
    }
    let ptr = (packed >> 32) as u32;
    let len = (packed & 0xffff_ffff) as u32;
    // The host allocated this buffer in OUR memory via `alloc` and wrote the
    // JSON there. Read it back (leaked, like every alloc in this SDK).
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

/// Convenience: a GET via the brokered [`fetch`].
#[cfg(feature = "net-fetch")]
pub fn fetch_get(url: &str) -> String {
    fetch("GET", url, &[])
}

// --- Memory ABI: `alloc` export ---------------------------------------------
//
// The host calls `alloc(len)` then writes bytes there before a call. We keep a
// simple bump-free strategy: leak each allocation. Ploxions are short-lived
// inside the host's drive loop and the host drops the whole Store on goodbye,
// so leaking inside the sandbox is harmless and keeps the ABI trivial.

/// Reserve `len` bytes in linear memory and return a pointer the host can write
/// into. Memory is intentionally leaked (freed wholesale when the host drops
/// the Store at `plc_goodbye`).
///
/// # Safety
/// Exposed as the WASM `alloc` export; callable only by the host with a
/// sensible `len`.
#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    if len <= 0 {
        return 0;
    }
    let len = len as usize;
    // align 1 is fine for byte buffers (strings / payloads).
    let layout = Layout::from_size_align(len, 1).expect("valid layout");
    unsafe {
        let ptr = rust_alloc(layout);
        if ptr.is_null() {
            return 0;
        }
        ptr as i32
    }
}

/// Pack `(ptr, len)` into the i64 the host expects back from `plc_manifest`.
#[inline]
pub fn pack_ptr_len(ptr: u32, len: u32) -> i64 {
    (((ptr as u64) << 32) | (len as u64)) as i64
}

/// Helper: leak a manifest JSON string and return its packed `(ptr,len)` i64.
/// The bytes live for the lifetime of the module (the host reads them once).
pub fn manifest_ret(json: &'static str) -> i64 {
    pack_ptr_len(json.as_ptr() as u32, json.len() as u32)
}

/// Read the bytes the host passed into a `plc_on_event` call back into a slice.
///
/// # Safety
/// `ptr`/`len` must be the pair the host passed into the current call.
pub unsafe fn read_args(ptr: i32, len: i32) -> &'static [u8] {
    if ptr == 0 || len <= 0 {
        return &[];
    }
    std::slice::from_raw_parts(ptr as *const u8, len as usize)
}

/// Convenience: define the `plc_manifest` export from a `&'static str` of JSON.
#[macro_export]
macro_rules! export_manifest {
    ($json:expr) => {
        #[no_mangle]
        pub extern "C" fn plc_manifest() -> i64 {
            $crate::manifest_ret($json)
        }
    };
}

/// Cycle de vie **complet** d'un ploxion mono-topic : génère
/// `plc_init`/`plc_health`/`plc_goodbye`/`plc_on_event`. Le ploxion ne fournit
/// que son message d'init/goodbye, le topic qu'il écoute, et son handler
/// `fn(&str)`. C'est le **bion de cycle de vie** : avant, ~25 lignes recopiées
/// dans chaque ploxion ; maintenant une ligne.
///
/// ```ignore
/// ploxion!(init: "kion: init", goodbye: "kion: goodbye", on "kion.collapse" => run);
/// ```
#[macro_export]
macro_rules! ploxion {
    (init: $init:expr, goodbye: $bye:expr, on $topic:literal => $handler:path $(,)?) => {
        #[no_mangle]
        pub extern "C" fn plc_init() {
            $crate::log($init);
        }
        #[no_mangle]
        pub extern "C" fn plc_health() -> i32 {
            0
        }
        #[no_mangle]
        pub extern "C" fn plc_goodbye() {
            $crate::log($bye);
        }
        #[no_mangle]
        pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
            let topic = $crate::bions::decode_str(tp, tl);
            if topic != $topic {
                return;
            }
            let payload = $crate::bions::decode_str(pp, pl);
            $handler(&payload);
        }
    };
}

/// Cycle de vie **trivial** (juste `plc_init`/`plc_health`/`plc_goodbye`) pour
/// les ploxions multi-topics ou adaptateurs qui gardent un `plc_on_event` propre
/// (en utilisant [`bions::decode_event`]).
#[macro_export]
macro_rules! ploxion_lifecycle {
    (init: $init:expr, goodbye: $bye:expr $(,)?) => {
        #[no_mangle]
        pub extern "C" fn plc_init() {
            $crate::log($init);
        }
        #[no_mangle]
        pub extern "C" fn plc_health() -> i32 {
            0
        }
        #[no_mangle]
        pub extern "C" fn plc_goodbye() {
            $crate::log($bye);
        }
    };
}
