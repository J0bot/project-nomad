//! `osiris-adapter` ploxion — the **convergence of the two machines**.
//!
//! OSIRIS (github.com/simplifaisoul/osiris) is José's *machine à veille*: an
//! OSINT dashboard whose `GET /api/*` routes return JSON of live world events
//! (earthquakes, flights, war frontlines, exploited CVEs, …). This pure-WASM
//! adapter (PLC v1.1, capability `net.fetch`) does two things:
//!
//!   (A) it **polls** a configurable OSIRIS base URL through the host's brokered
//!       `plc_fetch` (it never opens a socket — it asks the host), parses each
//!       route's real JSON shape, and republishes every world event as a
//!       compact, normalised `osiris.<domain>` topic on the XERB0XI0N bus
//!       (`{id,kind,lat,lng,label,severity}`); and
//!
//!   (B) it **connects OSIRIS to the machine à tsoins**. For each emitted event
//!       (bounded so the store is never flooded) it ALSO emits
//!       `tsoin.record {"name":"osiris:<domain>:<id>","bytes":<event-json-hex>}`.
//!       The `tsoin` ploxion is subscribed to `tsoin.record`, so the host routes
//!       that event into its sandbox and it stores the bytes as one frame of its
//!       record/replay timeline — **un tsoin = un instant de réel**, replayable
//!       bit-exact. OSIRIS sees the world; the tsoin engine remembers it.
//!
//! The bytes travel as lowercase **hex** inside the `tsoin.record` payload (the
//! exact contract the `tsoin` ploxion + `state-client` already use), so the
//! recorded reel is a faithful copy of the normalised OSIRIS event JSON.
//!
//! ## Bus contract
//!
//! - capabilities: `net.fetch` — the host links `plc_fetch` into THIS Store only.
//! - requires: `osiris.refresh` — a trigger to (re-)poll. Payload may carry
//!   `{"base":"http://host:port"}` to override the baked default base URL.
//! - provides: `osiris.air.track`, `osiris.land.quake`, `osiris.land.fire`,
//!   `osiris.conflict.zone`, `osiris.cyber.cve`, `osiris.alert`, and
//!   `tsoin.record` (it emits it — included in `provides` so the manifest is
//!   honest and the host wires `tsoin.record` from this ploxion to the `tsoin`).
//!
//! It also polls its baked defaults on `plc_init`. A route that fails, is
//! unreachable, or returns unexpected JSON is **skipped with a log, never a
//! panic** — a DOWN upstream is a valid outcome.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, fetch_get, log, read_args};
use ploxion_sdk::bions::{esc, json_str, json_str as json_string_field_unescaped, to_hex, tsoin_record_hex, json_uint, array_body, json_num_raw, split_objects};

export_manifest!(
    r#"{"id":"osiris-adapter","version":"1.0.0","capabilities":["net.fetch"],"provides":["osiris.air.track","osiris.land.quake","osiris.land.fire","osiris.conflict.zone","osiris.cyber.cve","osiris.alert","tsoin.record"],"requires":["osiris.refresh"],"children_types":[],"parent_types":[]}"#
);

/// Baked default OSIRIS base URL — a local OSIRIS / the demo mock. Overridable
/// at runtime via `osiris.refresh {"base":..}`.
const DEFAULT_BASE: &str = "http://127.0.0.1:3000";

/// Cap on events emitted per route per poll, so the bus is never flooded by a
/// route that returns thousands of features (USGS/ADS-B can be large).
const MAX_PER_ROUTE: usize = 20;

/// Of the events we emit, how many (per route) are ALSO recorded as a tsoin.
/// Bounded independently from `MAX_PER_ROUTE` so the tsoin store stays small
/// even if the bus carries more — "un échantillon d'instants de réel".
const MAX_TSOIN_PER_ROUTE: usize = 5;

/// A route the adapter polls: the OSIRIS path, the `osiris.*` domain topic it
/// republishes onto, and a `kind` tag baked into each event payload.
struct Route {
    /// OSIRIS path appended to the base URL (e.g. `/api/earthquakes`).
    path: &'static str,
    /// The bus topic events from this route are emitted on.
    topic: &'static str,
    /// A short kind tag stamped into each normalised event payload.
    kind: &'static str,
}

/// A representative SUBSET of OSIRIS routes (solid, not exhaustive). Each maps a
/// real OSIRIS GET route to one `osiris.<domain>` topic.
const ROUTES: &[Route] = &[
    Route { path: "/api/earthquakes", topic: "osiris.land.quake",    kind: "quake" },
    Route { path: "/api/flights",     topic: "osiris.air.track",     kind: "flight" },
    Route { path: "/api/frontlines",  topic: "osiris.conflict.zone", kind: "frontline" },
    Route { path: "/api/cyber-threats", topic: "osiris.cyber.cve",   kind: "cve" },
];

// --- tiny hex codec (matches the tsoin ploxion's contract exactly) ----------

// --- minimal JSON helpers (payloads are real but we only need a few fields) --

/// Parse a JSON number token into f64 (best-effort; `None` on garbage).
fn num(json: &str, key: &str) -> Option<f64> {
    json_num_raw(json, key)?.parse().ok()
}

// --- the brokered fetch result ---------------------------------------------

/// The host returns `{"status":..,"body":"...","error":..}` from `plc_fetch`.
/// `body` is the upstream JSON, but JSON-string-escaped (so `"` -> `\"`). We
/// extract + UNESCAPE it so the route parsers see the upstream JSON as-is.
fn fetch_body(base: &str, path: &str) -> Option<(u16, String)> {
    let url = format!("{base}{path}");
    log(&format!("osiris-adapter: GET {url} (via plc_fetch, capability net.fetch)"));
    let resp = fetch_get(&url);
    let code = json_u16(&resp, "status").unwrap_or(0);
    if !(200..400).contains(&code) {
        let why = json_str(&resp, "error").unwrap_or_default();
        log(&format!("osiris-adapter: {path} DOWN ({code:03}) [{why}] — skipped, no panic"));
        return None;
    }
    let body = json_string_field_unescaped(&resp, "body")?;
    Some((code, body))
}

/// Extract `"status"` as u16.
fn json_u16(json: &str, key: &str) -> Option<u16> {
    json_uint(json, key).and_then(|v| u16::try_from(v).ok())
}

// --- normalised event + the tsoin connection --------------------------------

/// A normalised OSIRIS event ready to publish + (optionally) remember as a tsoin.
struct Norm {
    id: String,
    kind: &'static str,
    lat: Option<f64>,
    lng: Option<f64>,
    label: String,
    severity: f64,
}

impl Norm {
    /// The compact normalised payload `{id,kind,lat,lng,label,severity}`. Numbers
    /// are rendered plainly; absent lat/lng become `null`.
    fn to_json(&self) -> String {
        let lat = self.lat.map(fmt_num).unwrap_or_else(|| "null".into());
        let lng = self.lng.map(fmt_num).unwrap_or_else(|| "null".into());
        format!(
            "{{\"id\":\"{}\",\"kind\":\"{}\",\"lat\":{lat},\"lng\":{lng},\"label\":\"{}\",\"severity\":{}}}",
            esc(&self.id),
            self.kind,
            esc(&self.label),
            fmt_num(self.severity),
        )
    }
}

/// Render an f64 without a trailing `.0` for integers, JSON-safe.
fn fmt_num(x: f64) -> String {
    if x.is_finite() {
        if x.fract() == 0.0 && x.abs() < 1e15 {
            format!("{}", x as i64)
        } else {
            // trim to a sane precision
            let s = format!("{x:.5}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    } else {
        "0".to_string()
    }
}

/// Parse the OSIRIS earthquakes route body (USGS-derived): the OSIRIS route
/// normalises USGS into `{"earthquakes":[{id,lat,lng,magnitude,place,...}]}`.
/// (It also tolerates a raw USGS FeatureCollection by falling back to features.)
fn parse_quakes(body: &str) -> Vec<Norm> {
    let arr = array_body(body, "earthquakes")
        .or_else(|| array_body(body, "features"))
        .unwrap_or("");
    split_objects(arr)
        .into_iter()
        .map(|o| {
            let id = json_str(o, "id").unwrap_or_else(|| "?".into());
            // OSIRIS shape: lat/lng/magnitude/place at top of the object.
            let lat = num(o, "lat");
            let lng = num(o, "lng");
            let mag = num(o, "magnitude").or_else(|| num(o, "mag")).unwrap_or(0.0);
            let place = json_str(o, "place").unwrap_or_default();
            Norm {
                id,
                kind: "quake",
                lat,
                lng,
                label: place,
                severity: mag,
            }
        })
        .collect()
}

/// Parse the OSIRIS flights route body. The route returns several arrays
/// (`commercial_flights`, `military_flights`, `private_jets`, …); we pull the
/// most operationally interesting ones, each aircraft being one air track.
fn parse_flights(body: &str) -> Vec<Norm> {
    let mut out = Vec::new();
    for (arr_key, sev) in [
        ("military_flights", 3.0f64),
        ("private_jets", 2.0),
        ("commercial_flights", 1.0),
    ] {
        if let Some(arr) = array_body(body, arr_key) {
            for o in split_objects(arr) {
                let id = json_str(o, "icao24")
                    .filter(|s| !s.is_empty())
                    .or_else(|| json_str(o, "callsign"))
                    .unwrap_or_else(|| "?".into());
                let label = json_str(o, "callsign").unwrap_or_default();
                out.push(Norm {
                    id,
                    kind: "flight",
                    lat: num(o, "lat"),
                    lng: num(o, "lng"),
                    label,
                    severity: sev,
                });
            }
        }
    }
    out
}

/// Parse the OSIRIS cyber-threats route body: `{"threats":[{id,name,severity,...}]}`
/// — these are the exploited-CVE entries (CISA KEV). Each becomes a cyber event.
fn parse_cves(body: &str) -> Vec<Norm> {
    let arr = array_body(body, "threats").unwrap_or("");
    split_objects(arr)
        .into_iter()
        .map(|o| {
            let id = json_str(o, "id").unwrap_or_else(|| "?".into());
            let name = json_str(o, "name").unwrap_or_default();
            let sev_str = json_str(o, "severity").unwrap_or_default();
            let severity = match sev_str.to_ascii_uppercase().as_str() {
                "CRITICAL" => 4.0,
                "HIGH" => 3.0,
                "MEDIUM" => 2.0,
                "LOW" => 1.0,
                _ => 2.0,
            };
            Norm { id, kind: "cve", lat: None, lng: None, label: name, severity }
        })
        .collect()
}

/// Parse the OSIRIS frontlines route body (DeepStateMap GeoJSON, deeply nested).
/// We do NOT deep-parse the polygon geometry; instead we surface the named map
/// (one conflict-zone event) so the convergence is demonstrated without a full
/// GeoJSON parser. Tolerant: emits at most one zone event.
fn parse_frontlines(body: &str) -> Vec<Norm> {
    // The OSIRIS route wraps DeepState's history under `"frontlines"`. If absent
    // or null, there is no zone to surface.
    if !body.contains("frontlines") {
        return Vec::new();
    }
    // A name/id may live deep in the GeoJSON; use whatever flat hints exist.
    let id = json_str(body, "id")
        .or_else(|| json_str(body, "name"))
        .unwrap_or_else(|| "deepstate".into());
    let label = json_str(body, "name").unwrap_or_else(|| "Ukraine frontline".into());
    vec![Norm {
        id,
        kind: "frontline",
        lat: None,
        lng: None,
        label,
        severity: 3.0,
    }]
}

/// Dispatch a route's body to the right parser by its `kind`.
fn parse_route(kind: &str, body: &str) -> Vec<Norm> {
    match kind {
        "quake" => parse_quakes(body),
        "flight" => parse_flights(body),
        "cve" => parse_cves(body),
        "frontline" => parse_frontlines(body),
        _ => Vec::new(),
    }
}

/// Emit one normalised event on its route topic AND — bounded — record it as a
/// tsoin. THIS is the machine-à-tsoins connection: `tsoin.record` carries the
/// event JSON (hex) under `name:"osiris:<domain>:<id>"`; the `tsoin` ploxion
/// stores it as one replayable frame of reel.
fn publish(topic: &str, kind: &str, n: &Norm, record_as_tsoin: bool) {
    let payload = n.to_json();
    emit(topic, payload.as_bytes());

    // A high-severity event is ALSO surfaced as an osiris.alert (convergence
    // beacon), regardless of the tsoin budget.
    if n.severity >= 4.0 {
        emit("osiris.alert", payload.as_bytes());
    }

    if record_as_tsoin {
        let name = format!("osiris:{kind}:{}", n.id);
        let hex = to_hex(payload.as_bytes());
        log(&format!(
            "osiris-adapter: -> tsoin.record name='{name}' ({} bytes) [un instant de réel]",
            payload.len()
        ));
        tsoin_record_hex(&esc(&name), &hex);
    }
}

/// Poll every configured route against `base`, normalise, publish, and record a
/// bounded sample as tsoins. Never panics: a failed/odd route is skipped.
fn poll_all(base: &str) {
    log(&format!("osiris-adapter: polling OSIRIS base={base} ({} route(s))", ROUTES.len()));
    let mut total_events = 0usize;
    let mut total_tsoins = 0usize;
    for r in ROUTES {
        let Some((code, body)) = fetch_body(base, r.path) else { continue };
        let events = parse_route(r.kind, &body);
        let n_events = events.len().min(MAX_PER_ROUTE);
        log(&format!(
            "osiris-adapter: {} ({code:03}) -> {} event(s) (emitting {n_events}, recording up to {MAX_TSOIN_PER_ROUTE})",
            r.path,
            events.len()
        ));
        for (i, n) in events.into_iter().take(MAX_PER_ROUTE).enumerate() {
            let record = i < MAX_TSOIN_PER_ROUTE;
            publish(r.topic, r.kind, &n, record);
            total_events += 1;
            if record {
                total_tsoins += 1;
            }
        }
    }
    log(&format!(
        "osiris-adapter: poll done — {total_events} osiris.* event(s) on the bus, {total_tsoins} recorded as tsoins"
    ));
}

// --- PLC lifecycle ----------------------------------------------------------

#[no_mangle]
pub extern "C" fn plc_init() {
    log("osiris-adapter: init (machine à veille -> bus + machine à tsoins, capability net.fetch)");
    poll_all(DEFAULT_BASE);
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

/// On `osiris.refresh` (payload may carry `{"base":..}`), re-poll OSIRIS.
#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic != b"osiris.refresh" {
        return;
    }
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let payload = String::from_utf8_lossy(payload);
    let base = json_str(&payload, "base").unwrap_or_else(|| DEFAULT_BASE.to_string());
    poll_all(&base);
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("osiris-adapter: goodbye");
}

// ---------------------------------------------------------------------------
// Unit tests: the pure parsers, OFFLINE, against OSIRIS-shaped fixture JSON.
// (The end-to-end record+replay-bit-exact through the real tsoin ploxion lives
// in the host integration test crate.)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    const QUAKES: &str = r#"{"earthquakes":[{"id":"us7000abcd","lat":38.1,"lng":-118.2,"depth":5.0,"magnitude":4.7,"place":"Nevada"},{"id":"us7000efgh","lat":-12.5,"lng":166.9,"magnitude":6.1,"place":"Vanuatu"}],"total":2}"#;

    const FLIGHTS: &str = r#"{"commercial_flights":[{"callsign":"AFR123","lat":48.8,"lng":2.3,"icao24":"3944ed"}],"military_flights":[{"callsign":"RCH456","lat":50.1,"lng":8.6,"icao24":"ae1234"}],"private_jets":[],"total":2}"#;

    const CVES: &str = r#"{"threats":[{"id":"CVE-2024-0001","name":"BadThing RCE","vendor":"ACME","severity":"CRITICAL"},{"id":"CVE-2024-0002","name":"Lesser Bug","severity":"HIGH"}]}"#;

    const FRONT: &str = r#"{"frontlines":{"id":42,"name":"2024/06 line","map":{"features":[]}},"timestamp":"now"}"#;

    #[test]
    fn quakes_parse_to_normalised_events() {
        let v = parse_quakes(QUAKES);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "us7000abcd");
        assert_eq!(v[0].kind, "quake");
        assert_eq!(v[0].lat, Some(38.1));
        assert_eq!(v[0].lng, Some(-118.2));
        assert_eq!(v[0].label, "Nevada");
        assert_eq!(v[0].severity, 4.7);
        assert_eq!(v[1].severity, 6.1);
    }

    #[test]
    fn quake_payload_shape_is_compact_normalised() {
        let v = parse_quakes(QUAKES);
        let j = v[0].to_json();
        assert!(j.contains("\"id\":\"us7000abcd\""));
        assert!(j.contains("\"kind\":\"quake\""));
        assert!(j.contains("\"lat\":38.1"));
        assert!(j.contains("\"lng\":-118.2"));
        assert!(j.contains("\"label\":\"Nevada\""));
        assert!(j.contains("\"severity\":4.7"));
    }

    #[test]
    fn flights_parse_military_then_private_then_commercial() {
        let v = parse_flights(FLIGHTS);
        // military first (severity 3), then commercial (severity 1); no private.
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "ae1234"); // military icao24
        assert_eq!(v[0].severity, 3.0);
        assert_eq!(v[1].id, "3944ed"); // commercial
        assert_eq!(v[1].severity, 1.0);
        assert_eq!(v[1].label, "AFR123");
    }

    #[test]
    fn cves_parse_with_severity_ranking() {
        let v = parse_cves(CVES);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "CVE-2024-0001");
        assert_eq!(v[0].severity, 4.0); // CRITICAL
        assert!(v[0].lat.is_none());
        assert_eq!(v[1].severity, 3.0); // HIGH
    }

    #[test]
    fn frontlines_surface_one_zone() {
        let v = parse_frontlines(FRONT);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, "frontline");
        assert_eq!(v[0].label, "2024/06 line");
    }

    #[test]
    fn split_objects_is_brace_and_string_aware() {
        // A label containing braces/quotes must not break the splitter.
        let body = r#"{"a":1,"l":"x{}\"y"},{"b":2}"#;
        let objs = split_objects(body);
        assert_eq!(objs.len(), 2);
        assert!(objs[0].contains("\"a\":1"));
        assert!(objs[1].contains("\"b\":2"));
    }

    #[test]
    fn array_body_is_bracket_depth_aware() {
        let json = r#"{"k":[{"nested":[1,2,3]},{"x":4}],"after":9}"#;
        let body = array_body(json, "k").unwrap();
        let objs = split_objects(body);
        assert_eq!(objs.len(), 2);
        assert!(objs[0].contains("nested"));
    }

    #[test]
    fn fetch_body_unescapes_the_host_json_string_body() {
        // The host wraps the upstream JSON as an escaped "body" string.
        let host = r#"{"status":200,"body":"{\"earthquakes\":[{\"id\":\"q1\",\"lat\":1.0,\"lng\":2.0,\"magnitude\":5.0,\"place\":\"X\"}]}"}"#;
        let body = json_string_field_unescaped(host, "body").unwrap();
        assert!(body.starts_with("{\"earthquakes\""));
        let v = parse_quakes(&body);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "q1");
        assert_eq!(v[0].severity, 5.0);
    }

    #[test]
    fn unexpected_json_is_skipped_not_panicked() {
        // Garbage / wrong-shape bodies yield zero events, never a panic.
        assert!(parse_quakes("not json at all").is_empty());
        assert!(parse_flights("{}").is_empty());
        assert!(parse_cves("{\"threats\":\"oops\"}").is_empty());
    }

    #[test]
    fn tsoin_record_payload_round_trips_to_the_event_bytes() {
        // The hex inside tsoin.record decodes back to the exact event JSON.
        let v = parse_quakes(QUAKES);
        let payload = v[0].to_json();
        let hex = to_hex(payload.as_bytes());
        // decode hex
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(String::from_utf8(bytes).unwrap(), payload);
    }
}
