//! # Service connector — the host's **native I/O adapter** to the bus.
//!
//! WASM ploxions are sandboxed pure compute: they cannot open a socket, read a
//! file, or reach the network. The host, however, is **native Rust** — it can.
//! This module is the first *native adapter* (a.k.a. connector): a host-side
//! participant that does real I/O on one side and speaks the XERB0XI0N bus on
//! the other.
//!
//! Concretely the [`ServiceConnector`]:
//!
//! 1. reads the runtime overlay `runtime.json` (READ-ONLY) listing the
//!    DEPLOYED services and their health URLs,
//! 2. for each `deployed:true` entry with a health URL, performs a real
//!    **HTTP GET** (via the [`HealthCheck`] fn — swappable so unit tests stay
//!    offline/deterministic),
//! 3. emits a `service.health` event `{id,url,code,up}` onto the bus **for each
//!    service**, as the participant id `"service-connector"`.
//!
//! It registers on the bus as a native participant that **provides**
//! `["service.health"]`. WASM ploxions that `require` `"service.health"` (e.g.
//! the `watcher`) then react to the live deployed services — without ever
//! touching the network themselves.
//!
//! ## The native/sandbox split (architecture)
//!
//! - **Native side (here):** real network/IO, bridged to the bus. Trusted host
//!   code. This is where adapters/connectors live.
//! - **WASM ploxions:** sandboxed, pure compute, isolated `Store`. They only
//!   ever see opaque bus payloads. (Capability-based WASM adapters — granting a
//!   ploxion scoped network access through the host — are future work.)

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The native participant id the connector announces itself as on the bus.
pub const CONNECTOR_ID: &str = "service-connector";

/// The single topic the connector provides/emits.
pub const HEALTH_TOPIC: &str = "service.health";

/// One DEPLOYED service the connector will health-check and bridge to the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// The service id (the key in `runtime.json`'s `ploxions` map), e.g.
    /// `"repoverse"` or `"my_website2"`.
    pub id: String,
    /// The health URL to GET, e.g. `https://repoverse.j0bot.ch`.
    pub url: String,
}

/// The result of one health check: the HTTP status code (or `0` if the request
/// could not complete at all — DNS failure, TLS error, timeout …) and the
/// derived `up` flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthResult {
    /// HTTP status code, or `0` when the request never produced a response.
    pub code: u16,
    /// `true` iff the service answered with a 2xx/3xx status.
    pub up: bool,
}

impl HealthResult {
    /// Build a result from a raw status code, deriving `up` (2xx/3xx == up).
    pub fn from_code(code: u16) -> Self {
        let up = (200..400).contains(&code);
        HealthResult { code, up }
    }

    /// The "could not even reach it" result: code 0, down.
    pub fn unreachable() -> Self {
        HealthResult { code: 0, up: false }
    }
}

/// A health-check function: given a URL, return its [`HealthResult`]. The real
/// one does an HTTP GET ([`http_get_health`]); tests inject a deterministic stub
/// so they never touch the network.
pub type HealthCheck<'a> = dyn Fn(&str) -> HealthResult + 'a;

// ---------------------------------------------------------------------------
// runtime.json (READ-ONLY) parsing — only the fields the connector needs.
// ---------------------------------------------------------------------------

/// The canonical read-only runtime overlay path (the deployed-service registry).
pub const DEFAULT_RUNTIME: &str = "/home/debian/ploxi0ns/.network/runtime.json";

#[derive(Debug, Deserialize)]
struct RuntimeFile {
    #[serde(default)]
    ploxions: BTreeMap<String, RuntimeEntry>,
}

#[derive(Debug, Deserialize)]
struct RuntimeEntry {
    #[serde(default)]
    deployed: bool,
    #[serde(default)]
    health: Option<String>,
}

/// Read `runtime.json` and return the DEPLOYED services that carry a health URL,
/// sorted by id (deterministic order — the demo and tests rely on it).
///
/// Entries with `deployed:false` (code-only — *droit au silence* §4.4: nothing
/// is running, so nothing to probe) and deployed entries WITHOUT a health URL
/// are skipped.
pub fn read_services(path: impl AsRef<Path>) -> Result<Vec<Service>> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading runtime overlay {}", path.display()))?;
    parse_services(&bytes)
}

/// Parse the deployed-with-health services out of raw `runtime.json` bytes.
/// Split out so tests can feed a fixture without a file.
pub fn parse_services(bytes: &[u8]) -> Result<Vec<Service>> {
    let file: RuntimeFile =
        serde_json::from_slice(bytes).context("parsing runtime.json")?;
    let mut out: Vec<Service> = file
        .ploxions
        .into_iter()
        .filter_map(|(id, e)| {
            if !e.deployed {
                return None; // not running -> nothing to health-check
            }
            let url = e.health?; // deployed but no health URL -> skip
            Some(Service { id, url })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

// ---------------------------------------------------------------------------
// The real network health check (native — the part WASM ploxions cannot do).
// ---------------------------------------------------------------------------

/// Real HTTP GET health check using `ureq` (blocking, lean, no async runtime).
///
/// Returns the response status code, or [`HealthResult::unreachable`] (code 0)
/// when the request fails to complete. Note `ureq` treats 4xx/5xx as an `Err`
/// (`StatusError`) rather than a panic — we still extract the code so a 500 is
/// reported as `code:500, up:false` (reachable but unhealthy), distinct from a
/// totally unreachable service (`code:0`).
pub fn http_get_health(url: &str) -> HealthResult {
    match ureq::get(url).call() {
        Ok(resp) => HealthResult::from_code(resp.status().as_u16()),
        Err(ureq::Error::StatusCode(code)) => HealthResult::from_code(code),
        Err(_) => HealthResult::unreachable(),
    }
}

// ---------------------------------------------------------------------------
// The connector itself.
// ---------------------------------------------------------------------------

/// One emitted `service.health` event: the service id/url, the probed result,
/// and the JSON payload the host put on the bus (so callers can show it).
#[derive(Debug, Clone)]
pub struct SweepRow {
    /// Service id.
    pub id: String,
    /// Health URL that was probed.
    pub url: String,
    /// The probe result (code + up).
    pub result: HealthResult,
    /// The exact JSON payload emitted on `service.health`.
    pub payload: String,
}

/// The native service connector: holds the list of DEPLOYED services to probe.
/// Stateless beyond that — a sweep is a pure function of (services, check fn).
pub struct ServiceConnector {
    services: Vec<Service>,
}

impl ServiceConnector {
    /// Build a connector from an explicit service list (used by tests).
    pub fn new(services: Vec<Service>) -> Self {
        ServiceConnector { services }
    }

    /// Build a connector by reading the deployed services from `runtime.json`.
    pub fn from_runtime(path: impl AsRef<Path>) -> Result<Self> {
        Ok(ServiceConnector { services: read_services(path)? })
    }

    /// The deployed services this connector will probe (read-only).
    pub fn services(&self) -> &[Service] {
        &self.services
    }

    /// Build the `service.health` JSON payload for one probed service.
    ///
    /// Shape: `{"id":<id>,"url":<url>,"code":<code>,"up":<bool>}`. Stable,
    /// flat, and JSON-safe so a WASM ploxion can parse it with the same tiny
    /// field-extraction helpers the other ploxions use.
    fn payload_for(svc: &Service, r: &HealthResult) -> String {
        format!(
            "{{\"id\":\"{}\",\"url\":\"{}\",\"code\":{},\"up\":{}}}",
            svc.id, svc.url, r.code, r.up
        )
    }

    /// Run one sweep: health-check every service with `check`, returning a row
    /// per service (id, url, result, payload). Pure — does NOT touch the bus,
    /// so it is trivially testable; [`Host::connector_sweep`] drives the bus.
    ///
    /// [`Host::connector_sweep`]: crate::Host::connector_sweep
    pub fn sweep(&self, check: &HealthCheck<'_>) -> Vec<SweepRow> {
        self.services
            .iter()
            .map(|svc| {
                let result = check(&svc.url);
                let payload = Self::payload_for(svc, &result);
                SweepRow {
                    id: svc.id.clone(),
                    url: svc.url.clone(),
                    result,
                    payload,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = br#"{
        "ploxions": {
            "site":   {"deployed": true,  "health": "https://site.example"},
            "api":    {"deployed": true,  "health": "https://api.example"},
            "nohealth":{"deployed": true},
            "offline":{"deployed": false, "health": "https://off.example"},
            "bare":   {"deployed": false}
        }
    }"#;

    #[test]
    fn parses_only_deployed_with_health_sorted() {
        let svcs = parse_services(FIXTURE).unwrap();
        // Only `api` and `site` qualify; sorted by id.
        assert_eq!(svcs.len(), 2);
        assert_eq!(svcs[0].id, "api");
        assert_eq!(svcs[1].id, "site");
        assert_eq!(svcs[0].url, "https://api.example");
    }

    #[test]
    fn health_result_up_down_from_code() {
        assert!(HealthResult::from_code(200).up);
        assert!(HealthResult::from_code(301).up);
        assert!(!HealthResult::from_code(404).up);
        assert!(!HealthResult::from_code(500).up);
        assert!(!HealthResult::unreachable().up);
        assert_eq!(HealthResult::unreachable().code, 0);
    }

    #[test]
    fn sweep_runs_check_per_service_and_builds_payload() {
        let conn = ServiceConnector::new(parse_services(FIXTURE).unwrap());
        // Stub: api is up (200), site is unreachable (000).
        let rows = conn.sweep(&|url| {
            if url.contains("api") {
                HealthResult::from_code(200)
            } else {
                HealthResult::unreachable()
            }
        });
        assert_eq!(rows.len(), 2);
        let api = rows.iter().find(|r| r.id == "api").unwrap();
        assert_eq!(api.result.code, 200);
        assert!(api.result.up);
        assert!(api.payload.contains("\"up\":true"));
        assert!(api.payload.contains("\"code\":200"));

        let site = rows.iter().find(|r| r.id == "site").unwrap();
        assert_eq!(site.result.code, 0);
        assert!(!site.result.up);
        assert!(site.payload.contains("\"up\":false"));
        assert!(site.payload.contains("\"code\":0"));
    }
}
