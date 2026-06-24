//! # `federation` — peer-link two xion daemons' buses (the inter-node protocol).
//!
//! This is the **xion as the protocol BETWEEN machines**: two daemons (e.g.
//! José's PC and the VPS) open a peer link so a ploxion on node A reacts to an
//! event emitted on node B and vice versa. The local bus is unchanged; this
//! layer only **forwards locally-originated emits out to peers** and **injects
//! events received from peers onto the local bus** marked with the peer's
//! origin node id.
//!
//! ## Node identity & origin tagging
//! Every daemon has a stable **node id** (`--node-id`, default = hostname). When
//! an event is injected from a peer, the host records it with
//! `from = "<FED_ORIGIN_PREFIX><peer-node>"` (e.g. `fed:VPS`). That prefix is
//! the **single source of truth for "this hop came from another node"** and is
//! the whole loop-prevention mechanism (see below).
//!
//! ## Federation message (the wire frame)
//! A peer link is a WebSocket. The *dedicated* `/peer` endpoint speaks ONLY this
//! frame, JSON, in both directions:
//! ```json
//! {"fed":{"origin_node":"A","seq":42,"topic":"ping","payload":"…"}}
//! ```
//! Distinct from the `/ws` client frame (`{"emit":{…}}`) so a federated inject
//! is never confused with an ordinary client emit. `origin_node` is the node the
//! event *originated on* (NOT necessarily the sender — though with 2 peers they
//! coincide); `seq` is that node's trace index of the emit (for dedup/debug).
//!
//! ## Forwarding rule (what crosses the link)
//! On the host thread, after a bus-touching command, every newly-appended
//! `Trace::Emit` hop is examined:
//! - **Local origin** (`from` does NOT start with `fed:`): forward it to all
//!   connected peers as a `{"fed":{…}}` frame.
//! - **Remote origin** (`from` starts with `fed:`): **NEVER forward it onward.**
//!
//! ## Loop safety (provable)
//! An event emitted on A and forwarded to B is injected on B with `from=fed:A`.
//! Because B only forwards *local-origin* hops, the `fed:A` emit is **not
//! re-forwarded** back to A. Symmetrically A never re-forwards a `fed:B` hop.
//! Therefore a single emit crosses the link **exactly once per direction** and
//! cannot echo: the `fed:` prefix is a one-bit "is-remote" flag and the rule is
//! "forward iff not remote". Two peers emitting cannot storm, because each
//! remote-origin injection terminates at the bus (it triggers local ploxions but
//! is never reflected). *Application-level* cascades (B's ploxion emits a NEW
//! topic in reaction, which is local-origin on B and so legitimately forwards to
//! A) are real new events, not echoes — they carry a different topic/origin and
//! each still crosses at most once per direction.
//!
//! ## Authentication
//! The link MUST be able to be authenticated (the real PC<->VPS link will be).
//! Two mechanisms, both supported and combinable:
//! - **Basic-auth in the peer URL** — `ws://user:pass@host:port/peer`. The
//!   client sends an `Authorization: Basic …` header on the upgrade; this is
//!   what a Traefik basic-auth in front of the daemon expects.
//! - **A shared peer token** — `--peer-token <tok>`; the client sends it as
//!   `X-Xion-Peer-Token` and the server (`--peer-token` set) rejects `/peer`
//!   upgrades that do not carry it.
//!
//! ## Additivity
//! With NO `--peer` and NO `--node-id`, nothing here runs differently: no peer
//! client tasks are spawned, no frames are sent, and the forwarding check is a
//! cheap prefix test over emit hops that simply never matches a peer to send to.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Prefix stamped on the `from` of any event injected from a peer. The presence
/// of this prefix is the *entire* "is-remote" signal used for loop prevention.
pub const FED_ORIGIN_PREFIX: &str = "fed:";

/// The header carrying the optional shared peer token on a `/peer` upgrade.
pub const PEER_TOKEN_HEADER: &str = "x-xion-peer-token";

/// Reconnect backoff bounds for a peer client whose link is down (a peer may not
/// be up yet, or may restart). Starts small, doubles, caps — never gives up.
pub const RECONNECT_MIN: Duration = Duration::from_millis(500);
pub const RECONNECT_MAX: Duration = Duration::from_secs(15);

/// One federation frame on the `/peer` link, both directions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerFrame {
    pub fed: FedMessage,
}

/// The payload of a [`PeerFrame`]: a single forwarded bus emit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FedMessage {
    /// The node the event ORIGINATED on (its `--node-id`).
    pub origin_node: String,
    /// The origin node's trace index of this emit (dedup/debug aid).
    #[serde(default)]
    pub seq: usize,
    /// The bus topic.
    pub topic: String,
    /// The opaque payload (UTF-8; the host never inspects it).
    #[serde(default)]
    pub payload: String,
}

/// Is this trace-`from` a REMOTE origin (an event a peer sent us)? Locally
/// originated emits return `false` and are the only ones forwarded onward.
#[inline]
pub fn is_remote_origin(from: &str) -> bool {
    from.starts_with(FED_ORIGIN_PREFIX)
}

/// The `from` tag the host records for an event injected from peer `node`.
#[inline]
pub fn remote_from(node: &str) -> String {
    format!("{FED_ORIGIN_PREFIX}{node}")
}

/// A peer the daemon dials, parsed from a `--peer <url>` argument. Carries the
/// ws url (auth, if any, is embedded in the url userinfo) plus an optional
/// shared token to present.
#[derive(Debug, Clone)]
pub struct PeerSpec {
    /// The full ws/wss url, possibly with `user:pass@` userinfo.
    pub url: String,
    /// An optional shared token presented as `X-Xion-Peer-Token`.
    pub token: Option<String>,
}

impl PeerSpec {
    /// A short, human label for traces/logs: the host:port of the url (auth
    /// stripped) so a secret in the userinfo never lands in a log line.
    pub fn label(&self) -> String {
        let s = self
            .url
            .strip_prefix("ws://")
            .or_else(|| self.url.strip_prefix("wss://"))
            .unwrap_or(&self.url);
        // Drop any `user:pass@` userinfo, then drop any trailing path.
        let after_auth = s.split('@').next_back().unwrap_or(s);
        after_auth.split('/').next().unwrap_or(after_auth).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_origin_round_trips_and_is_detected() {
        let f = remote_from("VPS");
        assert_eq!(f, "fed:VPS");
        assert!(is_remote_origin(&f));
        // A locally-originated emit (a ploxion id, "api", a connector) is NOT remote.
        assert!(!is_remote_origin("ping"));
        assert!(!is_remote_origin("api"));
        assert!(!is_remote_origin("service-connector"));
    }

    #[test]
    fn loop_rule_is_one_bit() {
        // The forwarding predicate is exactly "forward iff not remote".
        let forwardable = |from: &str| !is_remote_origin(from);
        assert!(forwardable("api")); // local emit -> crosses the link
        assert!(!forwardable("fed:A")); // remote inject -> NEVER re-forwarded
    }

    #[test]
    fn peer_frame_serde_roundtrip() {
        let frame = PeerFrame {
            fed: FedMessage {
                origin_node: "A".into(),
                seq: 7,
                topic: "ping".into(),
                payload: "hello".into(),
            },
        };
        let s = serde_json::to_string(&frame).unwrap();
        assert!(s.contains("\"fed\""));
        assert!(s.contains("\"origin_node\":\"A\""));
        let back: PeerFrame = serde_json::from_str(&s).unwrap();
        assert_eq!(back.fed.topic, "ping");
        assert_eq!(back.fed.origin_node, "A");
        // It must NOT parse as a client {emit:{…}} frame and vice-versa.
        assert!(serde_json::from_str::<PeerFrame>(r#"{"emit":{"topic":"x"}}"#).is_err());
    }

    #[test]
    fn label_strips_auth_and_path() {
        let p = PeerSpec { url: "ws://user:secret@10.0.0.1:8731/peer".into(), token: None };
        assert_eq!(p.label(), "10.0.0.1:8731");
        let p2 = PeerSpec { url: "ws://127.0.0.1:8730/peer".into(), token: None };
        assert_eq!(p2.label(), "127.0.0.1:8730");
    }
}
