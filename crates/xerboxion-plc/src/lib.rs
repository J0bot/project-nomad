//! # PLC v1 — the PL0XI0N Link Contract
//!
//! This crate is the **shared, freezable contract** ("the software CFC") that
//! every ploxion and the host agree on. It defines:
//!
//! 1. The [`Manifest`] a ploxion declares about itself (id, version,
//!    provides/requires topics, parent/child types).
//! 2. The exact **WASM ABI** — the names of the functions a ploxion module must
//!    export, and the names of the host functions it may import.
//!
//! The contract is deliberately minimal: JSON for the manifest and payloads, a
//! tiny linear-memory `alloc` so the host can pass bytes in, and five lifecycle
//! exports. Once frozen, these names and shapes do not change within v1.
//!
//! ## Lifecycle (PLC paper §3.8)
//! A ploxion MUST run standalone, MUST support clean shutdown (no residual
//! daemon — *droit au silence* §4.4), MUST expose health + a capabilities
//! manifest, MUST NEVER send data without consent, MUST NEVER listen when off.
//! On this host, "off" is enforced structurally: the host drops the ploxion's
//! entire wasmtime `Store` on `plc_goodbye`, so no linear memory, no callbacks,
//! nothing of the ploxion survives.

use serde::{Deserialize, Serialize};

/// The PLC ABI **major** version. Frozen contract surface — does not change
/// within v1: every v1 ploxion plugs into every v1 host.
pub const PLC_ABI_VERSION: u32 = 1;

/// The PLC ABI **minor** version. Bumped for purely **additive, backward
/// compatible** extensions to v1 (a v1.0 ploxion still loads on a v1.N host, and
/// a v1.N ploxion that uses no new feature still loads on a v1.0 host).
///
/// - `0` — the frozen v1.0 surface (manifest + 5 exports + `plc_emit`/`plc_log`).
/// - `1` — **consented capabilities**: the optional manifest field
///   [`Manifest::capabilities`] and the *gated* host import
///   [`imports::FETCH`] (`plc_fetch`), linked into a ploxion's `Store` **only
///   if** it declares the matching capability. Absent capabilities ⇒ `[]` ⇒
///   exactly the v1.0 behaviour.
pub const PLC_ABI_MINOR: u32 = 1;

// ---------------------------------------------------------------------------
// Capabilities (PLC v1.1) — least-authority host access, opt-in by manifest.
// ---------------------------------------------------------------------------

/// The capability tokens a ploxion may declare in [`Manifest::capabilities`].
///
/// A capability is *consent to a scoped host power*, mirroring how `provides`
/// is consent to emit on a topic. The host grants a power (links the matching
/// host import) **only if** the manifest declares it — least authority. A
/// ploxion that declares no capability is exactly a v1.0 ploxion: pure
/// sandboxed compute, only `plc_emit`/`plc_log`, no syscalls.
pub mod capabilities {
    /// `"net.fetch"` — permission to perform host-brokered HTTP requests via the
    /// gated import [`super::imports::FETCH`] (`plc_fetch`). Only a ploxion that
    /// declares this token gets `plc_fetch` linked into its `Store`; one that
    /// does not declare it has **no such import** and a wasm that references it
    /// is rejected cleanly at instantiation.
    pub const NET_FETCH: &str = "net.fetch";

    /// Every capability token the host understands. A manifest may only declare
    /// tokens from this set; unknown tokens are rejected at load (fail closed).
    pub const KNOWN: &[&str] = &[NET_FETCH];
}

// ---------------------------------------------------------------------------
// WASM ABI — exact symbol names. These are the FROZEN contract.
// ---------------------------------------------------------------------------

/// Names of the functions a ploxion **module must export**.
///
/// All `_ptr`/`_len` pairs are `(i32, i32)` indices into the module's own
/// WASM linear memory (UTF-8 / raw bytes). The host writes into memory the
/// ploxion gave it via [`exports::ALLOC`].
pub mod exports {
    /// `fn alloc(len: i32) -> i32`
    ///
    /// Reserve `len` bytes in the module's linear memory and return a pointer.
    /// The host calls this to hand strings/bytes (topics, payloads) into the
    /// ploxion. Memory ABI: the host owns nothing inside the module; it only
    /// borrows the region long enough to copy bytes in before a call.
    pub const ALLOC: &str = "alloc";

    /// `fn plc_manifest() -> i64`
    ///
    /// Return the ploxion's [`Manifest`] as JSON. The 64-bit return packs the
    /// `(ptr, len)` of that JSON in the module's memory:
    /// `ptr = (ret >> 32) as u32`, `len = (ret & 0xffff_ffff) as u32`.
    /// See [`pack_ptr_len`] / [`unpack_ptr_len`].
    pub const MANIFEST: &str = "plc_manifest";

    /// `fn plc_init()`
    ///
    /// Called exactly once after instantiation and after the manifest is read.
    /// The ploxion sets up internal state here. It MAY emit on the bus from
    /// init (with consent implied by its declared `provides`).
    pub const INIT: &str = "plc_init";

    /// `fn plc_health() -> i32`
    ///
    /// Return `0` for OK, any non-zero for degraded/unhealthy. The host sweeps
    /// this periodically. See [`HEALTH_OK`].
    pub const HEALTH: &str = "plc_health";

    /// `fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32)`
    ///
    /// Delivery of a bus event the ploxion subscribed to. The host has already
    /// copied `topic` and `payload` into the module's memory (via `alloc`).
    pub const ON_EVENT: &str = "plc_on_event";

    /// `fn plc_goodbye()`
    ///
    /// Clean shutdown. The ploxion releases anything it must release. The host
    /// then drops the whole `Store` — *droit au silence*: nothing of the
    /// ploxion keeps running or listening.
    pub const GOODBYE: &str = "plc_goodbye";

    /// Every export the host requires a valid ploxion module to provide.
    /// (`plc_init`, `plc_health`, `plc_goodbye`, `plc_on_event` are looked up
    /// but `on_event` is only required if the ploxion declares `requires`.)
    pub const REQUIRED: &[&str] = &[ALLOC, MANIFEST, INIT, HEALTH, GOODBYE];
}

/// Names of the functions the **host imports into** the ploxion (module env).
pub mod imports {
    /// The WASM import module name the host registers its functions under.
    pub const MODULE: &str = "xerboxion";

    /// `fn plc_emit(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32)`
    ///
    /// The ploxion sends an event to the bus. The host ROUTES it to every other
    /// ploxion subscribed to `topic` and TRACES it. The host does NOT filter
    /// the payload.
    pub const EMIT: &str = "plc_emit";

    /// `fn plc_log(ptr: i32, len: i32)`
    ///
    /// The ploxion writes a UTF-8 line to the host log/journal.
    pub const LOG: &str = "plc_log";

    /// `fn plc_fetch(method_ptr,i32, method_len,i32, url_ptr,i32, url_len,i32, body_ptr,i32, body_len,i32) -> i64`
    ///
    /// **Gated** host-brokered HTTP request (PLC v1.1). Linked into a ploxion's
    /// `Store` **only if** its manifest declares the
    /// [`super::capabilities::NET_FETCH`] capability — least authority. A
    /// ploxion without that capability has NO such import and a wasm that
    /// references it is rejected at instantiation.
    ///
    /// The host does the real I/O (native, via `ureq`) and returns a packed
    /// `(ptr, len)` (see [`super::pack_ptr_len`]) into the *ploxion's own*
    /// linear memory holding a JSON result
    /// `{"status":u16,"body":string,"error"?:string}`. `status` is the HTTP
    /// code, or `0` when the request never produced a response (DNS/TLS/timeout
    /// — the host NEVER panics on a network error). The host validates and
    /// limits the request (timeout, max body) before performing it.
    pub const FETCH: &str = "plc_fetch";
}

/// Return value of [`exports::HEALTH`] meaning "OK".
pub const HEALTH_OK: i32 = 0;

// ---------------------------------------------------------------------------
// (ptr, len) packing for the 64-bit manifest return.
// ---------------------------------------------------------------------------

/// Pack a `(ptr, len)` pair into the `i64` returned by [`exports::MANIFEST`].
#[inline]
pub fn pack_ptr_len(ptr: u32, len: u32) -> i64 {
    (((ptr as u64) << 32) | (len as u64)) as i64
}

/// Inverse of [`pack_ptr_len`]: unpack an `i64` into `(ptr, len)`.
#[inline]
pub fn unpack_ptr_len(packed: i64) -> (u32, u32) {
    let p = packed as u64;
    ((p >> 32) as u32, (p & 0xffff_ffff) as u32)
}

// ---------------------------------------------------------------------------
// Manifest — the capabilities a ploxion declares.
// ---------------------------------------------------------------------------

/// A topic name on the bus (e.g. `"ping"`). Just a string; kept as an alias to
/// make intent obvious in signatures.
pub type Topic = String;

/// The capabilities manifest a ploxion declares about itself.
///
/// `provides` = topics this ploxion MAY emit. `requires` = topics it wants to
/// receive (the host auto-subscribes it to these). `children_types` /
/// `parent_types` describe composition in the ploxion tree (which ploxion
/// kinds may sit below / above this one) — carried for the registry, not yet
/// load-bearing in the bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    /// Stable unique id, e.g. `"ping"`.
    pub id: String,
    /// Semantic version of the ploxion.
    pub version: String,
    /// Topics this ploxion may emit on the bus.
    #[serde(default)]
    pub provides: Vec<Topic>,
    /// Topics this ploxion wants delivered to it (auto-subscribed).
    #[serde(default)]
    pub requires: Vec<Topic>,
    /// Ploxion kinds allowed as children in the tree.
    #[serde(default)]
    pub children_types: Vec<String>,
    /// Ploxion kinds allowed as parents in the tree.
    #[serde(default)]
    pub parent_types: Vec<String>,
    /// **Capabilities (PLC v1.1, additive).** Scoped host powers this ploxion
    /// consents to — the host links the matching host import ONLY if the token
    /// is present (least authority). Absent ⇒ `[]` ⇒ a v1.0 pure-sandbox
    /// ploxion. Tokens come from [`capabilities`] (e.g.
    /// [`capabilities::NET_FETCH`]); unknown tokens are rejected at load.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl Manifest {
    /// Convenience constructor for a leaf ploxion that only provides/requires.
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        provides: Vec<Topic>,
        requires: Vec<Topic>,
    ) -> Self {
        Manifest {
            id: id.into(),
            version: version.into(),
            provides,
            requires,
            children_types: Vec::new(),
            parent_types: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    /// Serialize to the canonical JSON the ABI transports.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Manifest serializes")
    }

    /// Parse a manifest from JSON bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Whether this ploxion declared the capability token `cap` (consent to a
    /// scoped host power). The host uses this to gate which host imports it
    /// links — least authority.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }

    /// The first capability token this manifest declares that the host does not
    /// recognise (not in [`capabilities::KNOWN`]), if any. The host rejects a
    /// ploxion that declares an unknown capability — fail closed, never grant a
    /// power that does not exist.
    pub fn unknown_capability(&self) -> Option<&str> {
        self.capabilities
            .iter()
            .map(String::as_str)
            .find(|c| !capabilities::KNOWN.contains(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptr_len_roundtrip() {
        for (p, l) in [(0u32, 0u32), (1, 2), (0xdead_beef, 0x1234), (u32::MAX, u32::MAX)] {
            let packed = pack_ptr_len(p, l);
            assert_eq!(unpack_ptr_len(packed), (p, l));
        }
    }

    #[test]
    fn manifest_json_roundtrip() {
        let m = Manifest::new("ping", "1.0.0", vec!["ping".into()], vec![]);
        let json = m.to_json();
        let back = Manifest::from_json_bytes(json.as_bytes()).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn manifest_defaults_optional_fields() {
        // A minimal manifest (only id+version) must still parse.
        let json = r#"{"id":"x","version":"0.1.0"}"#;
        let m = Manifest::from_json_bytes(json.as_bytes()).unwrap();
        assert!(m.provides.is_empty());
        assert!(m.requires.is_empty());
        assert!(m.children_types.is_empty());
        // v1.1 additive: capabilities defaults to [] for a v1.0-shaped manifest.
        assert!(m.capabilities.is_empty());
        assert!(!m.has_capability(capabilities::NET_FETCH));
        assert!(m.unknown_capability().is_none());
    }

    #[test]
    fn manifest_parses_capabilities_when_present() {
        let json =
            r#"{"id":"adapter","version":"1.0.0","capabilities":["net.fetch"],"provides":["service.health"]}"#;
        let m = Manifest::from_json_bytes(json.as_bytes()).unwrap();
        assert!(m.has_capability(capabilities::NET_FETCH));
        assert!(m.unknown_capability().is_none());
        // Round-trips through JSON unchanged.
        let back = Manifest::from_json_bytes(m.to_json().as_bytes()).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn manifest_flags_unknown_capability() {
        let json = r#"{"id":"x","version":"0.1.0","capabilities":["net.fetch","fs.write"]}"#;
        let m = Manifest::from_json_bytes(json.as_bytes()).unwrap();
        assert_eq!(m.unknown_capability(), Some("fs.write"));
    }

    #[test]
    fn abi_minor_is_v1_1() {
        assert_eq!(PLC_ABI_VERSION, 1);
        assert_eq!(PLC_ABI_MINOR, 1);
        assert!(capabilities::KNOWN.contains(&capabilities::NET_FETCH));
    }

    #[test]
    fn required_exports_listed() {
        assert!(exports::REQUIRED.contains(&exports::MANIFEST));
        assert!(exports::REQUIRED.contains(&exports::INIT));
        assert!(exports::REQUIRED.contains(&exports::ALLOC));
    }
}
