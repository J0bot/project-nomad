//! # `plc_fetch` — the host's **brokered, capability-gated** HTTP power.
//!
//! PLC v1.1 (additive) lets a ploxion be a *pure-WASM service adapter* instead of
//! native host code. A WASM ploxion is sandboxed pure compute — no sockets, no
//! syscalls — so it cannot reach the network itself. With the `net.fetch`
//! capability declared in its manifest, the host links a single extra import,
//! [`xerboxion_plc::imports::FETCH`] (`plc_fetch`), into *that ploxion's* `Store`
//! and only that ploxion's. The host does the real I/O (native, via `ureq`,
//! reusing the connector's blocking/rustls setup) and hands back an opaque JSON
//! result. The sandbox sees a function, never the network stack — least
//! authority, consent by manifest, mirroring `provides`/`requires`.
//!
//! This module is the I/O engine; the gate (link-or-not) lives in
//! [`crate::Host::make_linker_for`], the trace hop in the import body.

use serde::Serialize;

/// Default per-request timeout. The host bounds every brokered fetch so a slow
/// or hung endpoint cannot stall a ploxion's call indefinitely.
pub const DEFAULT_TIMEOUT_SECS: u64 = 8;

/// Default cap on the response body the host will read back into the sandbox
/// (bytes). Larger responses are truncated (and flagged) so a ploxion cannot be
/// handed an unbounded buffer. 256 KiB is plenty for a health JSON.
pub const DEFAULT_MAX_BODY: usize = 256 * 1024;

/// Limits the host applies to a brokered fetch. Construct with [`FetchLimits::default`].
#[derive(Debug, Clone, Copy)]
pub struct FetchLimits {
    /// Per-request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum response body the host reads back (bytes); excess is truncated.
    pub max_body: usize,
}

impl Default for FetchLimits {
    fn default() -> Self {
        FetchLimits {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_body: DEFAULT_MAX_BODY,
        }
    }
}

/// The JSON result the host returns into the ploxion's memory from `plc_fetch`.
///
/// Flat and JSON-safe so a WASM ploxion can parse it with the same tiny
/// field-extraction helpers the other ploxions already use. `status` is the
/// HTTP code, or `0` when the request never produced a response (DNS/TLS/timeout
/// — `error` then explains why). `body` is the (possibly truncated) UTF-8
/// response text; `truncated` marks when the cap was hit.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FetchResult {
    /// HTTP status, or `0` when the request never completed.
    pub status: u16,
    /// Response body as text (lossy UTF-8), possibly truncated to `max_body`.
    pub body: String,
    /// Whether `body` was truncated at the host's `max_body` limit.
    #[serde(skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// Present only on a transport-level failure (status 0) or a rejected
    /// request — a short reason. Absent on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl FetchResult {
    /// A transport failure: no response at all (status 0), with a reason.
    pub fn unreachable(reason: impl Into<String>) -> Self {
        FetchResult {
            status: 0,
            body: String::new(),
            truncated: false,
            error: Some(reason.into()),
        }
    }

    /// A request the host refused to perform (bad method / unsupported scheme).
    /// Also status 0 — indistinguishable to the sandbox from "could not reach",
    /// which is intentional: the ploxion only learns it got nothing.
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self::unreachable(reason)
    }

    /// Serialize to the JSON the ABI transports back into the ploxion's memory.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            // Never panic on the host side, even if serialization somehow fails.
            r#"{"status":0,"body":"","error":"serialize"}"#.to_string()
        })
    }
}

/// The brokered HTTP power. A function of `(method, url, body, limits)` so it is
/// trivially testable offline (the gate + memory plumbing is tested separately
/// by loading a real wasm). NEVER panics: any transport error becomes status 0.
///
/// Only `GET` and `POST` over `http`/`https` are allowed; anything else is
/// rejected (status 0, with a reason) — the host does not expose arbitrary
/// verbs or schemes (e.g. `file://`) to the sandbox.
pub fn http_fetch(method: &str, url: &str, body: &[u8], limits: FetchLimits) -> FetchResult {
    let method_uc = method.trim().to_ascii_uppercase();

    // Scheme allow-list: http/https only. No file://, no anything else.
    let lower = url.trim_start().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return FetchResult::rejected(format!("unsupported url scheme: {url}"));
    }

    // Garde SSRF : un ploxion (même avec la capability `net.fetch` déclarée) ne doit pas faire
    // émettre à l'HÔTE des requêtes vers le réseau interne — le bus (10.0.0.1:8730), loopback,
    // sous-réseaux conteneurs, ou 169.254.169.254 (métadonnées cloud). On résout l'hôte et on
    // refuse si une IP résolue est interne (anti-rebind partiel : rejet si UNE des IP est privée).
    if let Some(reason) = forbidden_target(url, lower.starts_with("https://")) {
        return FetchResult::rejected(reason);
    }

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(limits.timeout_secs)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let resp = match method_uc.as_str() {
        "GET" => agent.get(url).call(),
        "POST" => agent.post(url).send(body),
        other => return FetchResult::rejected(format!("unsupported method: {other}")),
    };

    match resp {
        Ok(mut resp) => {
            let status = resp.status().as_u16();
            read_body(resp.body_mut(), status, limits.max_body)
        }
        // ureq surfaces a 4xx/5xx as an error carrying the code — reachable but
        // unhealthy. Preserve the status; report an empty body (the error path
        // does not stream it) so the sandbox still learns "code, but down".
        Err(ureq::Error::StatusCode(code)) => FetchResult {
            status: code,
            body: String::new(),
            truncated: false,
            error: None,
        },
        Err(e) => FetchResult::unreachable(short_err(&e)),
    }
}

/// Résout l'hôte de l'URL et renvoie `Some(raison)` si une IP résolue est interne (garde SSRF),
/// `None` si la cible est publique (ou non résolvable — ureq échouera proprement ensuite). Parsing
/// d'URL minimal, suffisant ici : le schéma `http(s)` est déjà validé par l'appelant.
fn forbidden_target(url: &str, is_https: bool) -> Option<String> {
    use std::net::ToSocketAddrs;
    let after_scheme = url.splitn(2, "://").nth(1)?;
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    let hostport = authority.rsplit('@').next()?; // retire l'userinfo éventuel
    let (host, port) = if let Some(rest) = hostport.strip_prefix('[') {
        let (h, p) = rest.split_once(']')?; // [IPv6]:port
        (h.to_string(), p.strip_prefix(':').and_then(|s| s.parse::<u16>().ok()))
    } else if let Some((h, p)) = hostport.rsplit_once(':') {
        (h.to_string(), p.parse::<u16>().ok())
    } else {
        (hostport.to_string(), None)
    };
    let port = port.unwrap_or(if is_https { 443 } else { 80 });
    match (host.as_str(), port).to_socket_addrs() {
        Ok(addrs) => addrs
            .map(|sa| sa.ip())
            .find(|ip| is_internal_ip(*ip))
            .map(|ip| format!("blocked internal/SSRF target: {host} -> {ip}")),
        Err(_) => None,
    }
}

/// Une IP interne / non-routable qu'un fetch piloté par un ploxion ne doit jamais atteindre.
fn is_internal_ip(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.octets()[0] == 0
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64) // 100.64/10 CGNAT
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique-local
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
                || v6.to_ipv4_mapped().map_or(false, |m| is_internal_ip(IpAddr::V4(m)))
        }
    }
}

/// Read up to `max_body` bytes of the response body, marking truncation.
fn read_body(body: &mut ureq::Body, status: u16, max_body: usize) -> FetchResult {
    // Cap the read so a huge response cannot be handed to the sandbox. Read one
    // extra byte to detect truncation.
    match body.with_config().limit((max_body + 1) as u64).read_to_vec() {
        Ok(bytes) => {
            let truncated = bytes.len() > max_body;
            let slice = if truncated { &bytes[..max_body] } else { &bytes[..] };
            FetchResult {
                status,
                body: String::from_utf8_lossy(slice).into_owned(),
                truncated,
                error: None,
            }
        }
        // We got a status but couldn't read the body — still report the status.
        Err(e) => FetchResult {
            status,
            body: String::new(),
            truncated: false,
            error: Some(short_err_str(&e.to_string())),
        },
    }
}

/// A short, single-line reason for a ureq transport error (no giant chains).
fn short_err(e: &ureq::Error) -> String {
    short_err_str(&e.to_string())
}

fn short_err_str(s: &str) -> String {
    let one = s.lines().next().unwrap_or(s);
    let one = one.trim();
    if one.len() > 120 {
        format!("{}…", &one[..120])
    } else {
        one.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_scheme_without_panicking() {
        let r = http_fetch("GET", "file:///etc/passwd", &[], FetchLimits::default());
        assert_eq!(r.status, 0);
        assert!(r.error.as_deref().unwrap().contains("scheme"));
        // Serializes cleanly.
        assert!(r.to_json().contains("\"status\":0"));
    }

    #[test]
    fn rejects_unsupported_method() {
        let r = http_fetch("DELETE", "https://example.test", &[], FetchLimits::default());
        assert_eq!(r.status, 0);
        assert!(r.error.as_deref().unwrap().contains("method"));
    }

    #[test]
    fn unreachable_host_is_status_zero_not_panic() {
        // RFC 5737 TEST-NET / a port nothing listens on: must fail to connect,
        // fast, and come back as status 0 with an error — never panic.
        let r = http_fetch(
            "GET",
            "http://127.0.0.1:9/never",
            &[],
            FetchLimits {
                timeout_secs: 2,
                max_body: DEFAULT_MAX_BODY,
            },
        );
        assert_eq!(r.status, 0, "unreachable must be status 0");
        assert!(r.error.is_some(), "unreachable must carry a reason");
    }

    #[test]
    fn result_json_shape_is_flat_and_parseable() {
        let ok = FetchResult {
            status: 200,
            body: "{\"status\":\"ok\"}".to_string(),
            truncated: false,
            error: None,
        };
        let j = ok.to_json();
        assert!(j.contains("\"status\":200"));
        assert!(j.contains("\"body\":"));
        // error/truncated omitted when not meaningful.
        assert!(!j.contains("error"));
        assert!(!j.contains("truncated"));
    }
}
