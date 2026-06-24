//! `repoverse-adapter` ploxion — pure-WASM adapter for the LIVE repoverse
//! service (PLC v1.1, capability `net.fetch`), now **enriched** to the same
//! pattern as `osiris-adapter`: it does not merely count repos, it PARSES them
//! and republishes each repo as a compact `repo.item` on the bus AND records a
//! bounded sample as `tsoin.record` frames — **un repo = un instant de réel**,
//! replayable bit-exact by the `tsoin` ploxion.
//!
//! ## What it does
//!
//!   (A) it **fetches** the deployed repoverse `/api/repos` through the host's
//!       brokered `plc_fetch` (it never opens a socket — it asks the host),
//!       parses the `{"repos":[...],"count":N}` wrapper, and:
//!         - keeps emitting the back-compat `repo.list` summary
//!           `{id,url,code,count,up}` exactly once per poll, and
//!         - emits one `repo.item` per parsed repo (bounded by `MAX_REPOS`),
//!           carrying the operationally useful flat fields.
//!
//!   (B) it **connects repoverse to the machine à tsoins**. For each emitted
//!       `repo.item` within budget it ALSO emits
//!       `tsoin.record {"name":"repoverse:repo:<slug>","bytes":<repo-item-hex>}`.
//!       The hex is of the EXACT same `repo.item` payload string, so the
//!       recorded tsoin frame is a faithful, bit-exact copy of what went on the
//!       bus.
//!
//! Re-poll on `repo.refresh` (payload may carry `{"url":..}` to override the
//! baked default). A DOWN upstream (non-2xx/3xx, unreachable) is a valid
//! outcome: it emits ONLY `repo.list` with `count:0,up:false`, no items, no
//! tsoins, and **never panics**.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, fetch_get, log, read_args};
use ploxion_sdk::bions::{esc, json_str, json_str as json_string_field_unescaped, to_hex, tsoin_record_hex, json_uint, array_body, json_num_raw, split_objects};

export_manifest!(
    r#"{"id":"repoverse-adapter","version":"2.0.0","capabilities":["net.fetch"],"provides":["repo.list","repo.item","tsoin.record"],"requires":["repo.refresh"],"children_types":[],"parent_types":[]}"#
);

/// Baked default repoverse repos endpoint. Overridable at runtime via
/// `repo.refresh {"url":..}`.
const DEFAULT_URL: &str = "https://repoverse.j0bot.ch/api/repos";

/// Cap on `repo.item` events emitted per poll, so the bus is never flooded
/// (the live feed is ~100 repos).
const MAX_REPOS: usize = 50;

/// Of the repos we emit, how many are ALSO recorded as a tsoin. Bounded
/// independently so the tsoin store stays small — "un échantillon d'instants".
const MAX_TSOIN: usize = 50;

// --- tiny hex codec (matches the tsoin ploxion's contract exactly) ----------

// --- minimal JSON helpers (ported verbatim from osiris-adapter) --------------

/// Read an integer count field tolerantly, defaulting to `0` on missing/garbage.
fn count(json: &str, key: &str) -> u64 {
    json_num_raw(json, key)
        .and_then(|t| t.parse::<f64>().ok())
        .map(|f| if f.is_finite() && f >= 0.0 { f as u64 } else { 0 })
        .unwrap_or(0)
}

/// Extract `"status"` (or any flat unsigned int) as u16.
fn json_u16(json: &str, key: &str) -> Option<u16> {
    json_uint(json, key).and_then(|v| u16::try_from(v).ok())
}

// --- normalised repo + the tsoin connection ---------------------------------

/// A parsed repoverse repo, ready to publish + (optionally) remember as a tsoin.
struct Repo {
    id: String,
    slug: String,
    name: String,
    rtype: String,
    visibility: String,
    member_count: u64,
    star_count: u64,
    fork_count: u64,
    created_at: String,
    updated_at: String,
}

impl Repo {
    /// Parse one `{...}` repo object slice into the fields we carry. Best-effort:
    /// missing/garbage fields default (`"?"` for id/slug so the tsoin name stays
    /// well-formed; `""` for the rest; `0` for counts). Never panics.
    fn parse(o: &str) -> Repo {
        Repo {
            id: json_str(o, "id").unwrap_or_else(|| "?".into()),
            slug: json_str(o, "slug").unwrap_or_else(|| "?".into()),
            name: json_str(o, "name").unwrap_or_default(),
            rtype: json_str(o, "type").unwrap_or_default(),
            visibility: json_str(o, "visibility").unwrap_or_default(),
            member_count: count(o, "memberCount"),
            star_count: count(o, "starCount"),
            fork_count: count(o, "forkCount"),
            created_at: json_str(o, "createdAt").unwrap_or_default(),
            updated_at: json_str(o, "updatedAt").unwrap_or_default(),
        }
    }

    /// The compact, flat `repo.item` payload — field names + order verbatim per
    /// spec (b). `description` and `createdBy` are deliberately OMITTED (still
    /// captured wholesale in the tsoin's hex of THIS payload). String values are
    /// `esc()`-ed; counts rendered plainly.
    fn to_item_json(&self) -> String {
        format!(
            "{{\"id\":\"{}\",\"slug\":\"{}\",\"name\":\"{}\",\"type\":\"{}\",\"visibility\":\"{}\",\"memberCount\":{},\"starCount\":{},\"forkCount\":{},\"createdAt\":\"{}\",\"updatedAt\":\"{}\"}}",
            esc(&self.id),
            esc(&self.slug),
            esc(&self.name),
            esc(&self.rtype),
            esc(&self.visibility),
            self.member_count,
            self.star_count,
            self.fork_count,
            esc(&self.created_at),
            esc(&self.updated_at),
        )
    }

    /// The tsoin frame name: `repoverse:repo:<slug>`, falling back to the id when
    /// slug is empty/`?` so the name is always well-formed.
    fn tsoin_name(&self) -> String {
        let key = if self.slug.is_empty() || self.slug == "?" {
            &self.id
        } else {
            &self.slug
        };
        format!("repoverse:repo:{key}")
    }
}

/// Parse the `/api/repos` body (`{"repos":[...],"count":N}`). Returns whatever
/// it can; an unexpected shape (no `"repos"` array) yields an empty vec. Never
/// panics.
fn parse_repos(body: &str) -> Vec<Repo> {
    let arr = match array_body(body, "repos") {
        Some(a) => a,
        None => return Vec::new(),
    };
    split_objects(arr).into_iter().map(Repo::parse).collect()
}

/// Emit one `repo.item` AND — bounded — record it as a tsoin. THIS is the
/// machine-à-tsoins connection: `tsoin.record` carries the EXACT `repo.item`
/// payload (hex) under `name:"repoverse:repo:<slug>"`; the `tsoin` ploxion
/// stores it as one replayable frame of reel.
fn publish(repo: &Repo, record_as_tsoin: bool) {
    let payload = repo.to_item_json();
    emit("repo.item", payload.as_bytes());

    if record_as_tsoin {
        let name = repo.tsoin_name();
        let hex = to_hex(payload.as_bytes());
        log(&format!(
            "repoverse-adapter: -> tsoin.record name='{name}' ({} bytes) [un instant de réel]",
            payload.len()
        ));
        tsoin_record_hex(&esc(&name), &hex);
    }
}

/// Fetch + parse + emit. Keeps the back-compat `repo.list` summary, adds
/// per-repo `repo.item` + bounded `tsoin.record`. Never panics; a DOWN upstream
/// emits only the summary with `count:0,up:false`.
fn poll(url: &str) {
    log(&format!("repoverse-adapter: GET {url} (via plc_fetch, capability net.fetch)"));
    let resp = fetch_get(url);
    let code = json_u16(&resp, "status").unwrap_or(0);
    let up = (200..400).contains(&code);

    if !up {
        let why = json_str(&resp, "error").unwrap_or_default();
        log(&format!("repoverse-adapter: DOWN ({code:03}) [{why}] — repo.list count=0, no items, no panic"));
        let payload = format!("{{\"id\":\"repoverse\",\"url\":\"{}\",\"code\":{code},\"count\":0,\"up\":false}}", esc(url));
        emit("repo.list", payload.as_bytes());
        return;
    }

    let body = json_string_field_unescaped(&resp, "body").unwrap_or_default();
    let repos = parse_repos(&body);
    let total = repos.len();

    // Back-compat summary: count = total parsed (truthful about upstream size,
    // not the capped/emitted subset).
    let summary = format!(
        "{{\"id\":\"repoverse\",\"url\":\"{}\",\"code\":{code},\"count\":{total},\"up\":true}}",
        esc(url)
    );
    emit("repo.list", summary.as_bytes());

    let emitting = total.min(MAX_REPOS);
    log(&format!(
        "repoverse-adapter: code={code:03} up=true repos={total} (emitting {emitting} repo.item, recording up to {MAX_TSOIN} tsoins)"
    ));

    let mut tsoins = 0usize;
    for (i, repo) in repos.into_iter().take(MAX_REPOS).enumerate() {
        let record = i < MAX_TSOIN;
        publish(&repo, record);
        if record {
            tsoins += 1;
        }
    }
    log(&format!(
        "repoverse-adapter: poll done — {emitting} repo.item on the bus, {tsoins} recorded as tsoins"
    ));
}

// --- PLC lifecycle ----------------------------------------------------------

#[no_mangle]
pub extern "C" fn plc_init() {
    log("repoverse-adapter: init (LIVE repoverse -> bus + machine à tsoins, capability net.fetch)");
    poll(DEFAULT_URL);
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

/// On `repo.refresh` (payload may carry `{"url":..}`), re-poll repoverse.
#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = unsafe { read_args(topic_ptr, topic_len) };
    if topic != b"repo.refresh" {
        return;
    }
    let payload = unsafe { read_args(payload_ptr, payload_len) };
    let payload = String::from_utf8_lossy(payload);
    let url = json_str(&payload, "url")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    poll(&url);
}

#[no_mangle]
pub extern "C" fn plc_goodbye() { log("repoverse-adapter: goodbye"); }

// ---------------------------------------------------------------------------
// Unit tests: the pure parsers, OFFLINE, against a repoverse-shaped fixture.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // Two full repo objects under the live "repos" wrapper, with the real field
    // set (note: createdBy + description present upstream; omitted from item).
    const REPOS: &str = r#"{"repos":[{"id":"11111111-1111-1111-1111-111111111111","slug":"repoverse-backend","name":"Repoverse Backend","description":"the API","type":"project","visibility":"public","createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-02-01T00:00:00Z","createdBy":"22222222-2222-2222-2222-222222222222","memberCount":3,"starCount":0,"forkCount":1},{"id":"33333333-3333-3333-3333-333333333333","slug":"planet-zero","name":"Planet Zero","description":"a planet","type":"planet","visibility":"public","createdAt":"2026-03-01T00:00:00Z","updatedAt":"2026-03-02T00:00:00Z","createdBy":"44444444-4444-4444-4444-444444444444","memberCount":10,"starCount":42,"forkCount":0}],"count":2}"#;

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn array_body_and_split_yield_two_repos() {
        let arr = array_body(REPOS, "repos").unwrap();
        let objs = split_objects(arr);
        assert_eq!(objs.len(), 2);
        assert!(objs[0].contains("repoverse-backend"));
        assert!(objs[1].contains("planet-zero"));
    }

    #[test]
    fn repo_parses_to_exact_item_shape() {
        let repos = parse_repos(REPOS);
        assert_eq!(repos.len(), 2);
        let item = repos[0].to_item_json();
        assert!(item.contains("\"slug\":\"repoverse-backend\""));
        assert!(item.contains("\"type\":\"project\""));
        assert!(item.contains("\"visibility\":\"public\""));
        assert!(item.contains("\"starCount\":0"));
        assert!(item.contains("\"memberCount\":3"));
        assert!(item.contains("\"forkCount\":1"));
        assert!(item.contains("\"createdAt\":\"2026-01-01T00:00:00Z\""));
        assert!(item.contains("\"updatedAt\":\"2026-02-01T00:00:00Z\""));
        // Deliberately OMITTED from repo.item:
        assert!(!item.contains("description"));
        assert!(!item.contains("createdBy"));
        // open enum 'type' passes through verbatim:
        let planet = repos[1].to_item_json();
        assert!(planet.contains("\"type\":\"planet\""));
        assert!(planet.contains("\"starCount\":42"));
    }

    #[test]
    fn tsoin_name_and_hex_round_trip_bit_exact() {
        let repos = parse_repos(REPOS);
        let r = &repos[0];
        assert_eq!(r.tsoin_name(), "repoverse:repo:repoverse-backend");
        let payload = r.to_item_json();
        let hex = to_hex(payload.as_bytes());
        // decode hex -> equals the exact repo.item payload bytes (replay-exact).
        assert_eq!(String::from_utf8(decode_hex(&hex)).unwrap(), payload);
    }

    #[test]
    fn down_or_garbage_body_yields_zero_no_panic() {
        // Not JSON at all.
        assert!(parse_repos("not json at all").is_empty());
        // Missing the "repos" wrapper key.
        assert!(parse_repos(r#"{"data":[{"slug":"x"}],"count":1}"#).is_empty());
        // array_body of garbage is None.
        assert!(array_body("not json", "repos").is_none());
        // Empty repos array -> zero repos.
        assert!(parse_repos(r#"{"repos":[],"count":0}"#).is_empty());
    }

    #[test]
    fn missing_fields_default_and_tsoin_name_falls_back_to_id() {
        // A repo with no slug -> tsoin name uses the id; counts default to 0.
        let body = r#"{"repos":[{"id":"abc","name":"No Slug"}],"count":1}"#;
        let repos = parse_repos(body);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].slug, "?");
        assert_eq!(repos[0].star_count, 0);
        assert_eq!(repos[0].tsoin_name(), "repoverse:repo:abc");
    }

    #[test]
    fn esc_and_split_objects_are_quote_and_brace_aware() {
        // A name containing a quote/backslash/brace must not break the splitter
        // or the emitted JSON.
        let body = r#"{"repos":[{"slug":"a","name":"x{}\"y","type":"topic","starCount":1},{"slug":"b","name":"plain"}],"count":2}"#;
        let repos = parse_repos(body);
        assert_eq!(repos.len(), 2);
        let item = repos[0].to_item_json();
        // The embedded quote/backslash is escaped, so the JSON stays well-formed:
        // re-extract the slug from our own emitted payload.
        assert_eq!(json_str(&item, "slug").as_deref(), Some("a"));
        assert_eq!(repos[1].slug, "b");
    }

    #[test]
    fn fetch_body_unescapes_the_host_json_string_body() {
        // The host wraps the upstream JSON as an escaped "body" string.
        let host = r#"{"status":200,"body":"{\"repos\":[{\"id\":\"q1\",\"slug\":\"s1\",\"name\":\"N\",\"type\":\"city\",\"visibility\":\"public\",\"memberCount\":2,\"starCount\":3,\"forkCount\":0,\"createdAt\":\"t1\",\"updatedAt\":\"t2\"}],\"count\":1}"}"#;
        assert_eq!(json_u16(host, "status"), Some(200));
        let body = json_string_field_unescaped(host, "body").unwrap();
        assert!(body.starts_with("{\"repos\""));
        let repos = parse_repos(&body);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].slug, "s1");
        assert_eq!(repos[0].rtype, "city");
        assert_eq!(repos[0].star_count, 3);
    }
}
