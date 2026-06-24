//! `bion-tester` ploxion — unit-teste chaque bion du SDK et grave un tsoin REPRODUCTIBLE.
//!
//! Sur `bion.test {bion, ...args}` il execute le bion nomme sur les args et grave
//! `test:bion:<nom>:<addr>` ou addr = fnv1a64(bion|in|out) : l'EMPREINTE comportementale. Un
//! bion deterministe -> re-run = meme addr = reproductible/verifiable. Deux bions de meme
//! empreinte sur les memes entrees = EQUIVALENTS (la base pour melanger + recreer le minimal
//! optimal, cf. docs/research/analyse-optimisation-decouverte-bions.md). Sur `bion.test.all`
//! il deroule une suite de cas et grave chaque empreinte. Pas de capability.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log};
use ploxion_sdk::bions::{decode_str, json_str, json_int, esc, tsoin_record, to_hex, from_hex,
                         fnv1a64, addr64, xor_delta, popcount_bytes};
use ploxion_sdk::math;

export_manifest!(
    r#"{"id":"bion-tester","version":"1.0.0","capabilities":[],"provides":["bion.tested","tsoin.record"],"requires":["bion.test","bion.test.all"],"children_types":[],"parent_types":[]}"#
);

fn ji(p: &str, k: &str) -> i32 { json_int(p, k, 0) as i32 }
fn js(p: &str, k: &str) -> String { json_str(p, k).unwrap_or_default() }
fn hx(p: &str, k: &str) -> Vec<u8> { from_hex(&js(p, k)).unwrap_or_default() }

/// Execute le bion nomme sur les args du payload. `None` si bion inconnu.
fn run_bion(bion: &str, p: &str) -> Option<String> {
    Some(match bion {
        "add"            => math::add(ji(p, "a"), ji(p, "b")).to_string(),
        "sub"            => math::sub(ji(p, "a"), ji(p, "b")).to_string(),
        "mul"            => math::mul(ji(p, "a"), ji(p, "b")).to_string(),
        "div"            => math::div(ji(p, "a"), ji(p, "b")).to_string(),
        "square"         => math::square(ji(p, "a")).to_string(),
        "abs"            => math::abs(ji(p, "a")).to_string(),
        "min"            => math::min(ji(p, "a"), ji(p, "b")).to_string(),
        "max"            => math::max(ji(p, "a"), ji(p, "b")).to_string(),
        "and"            => math::and(ji(p, "a"), ji(p, "b")).to_string(),
        "or"             => math::or(ji(p, "a"), ji(p, "b")).to_string(),
        "xor"            => math::xor(ji(p, "a"), ji(p, "b")).to_string(),
        "popcount"       => math::popcount(ji(p, "a")).to_string(),
        "fnv1a64"        => format!("{:016x}", fnv1a64(&js(p, "s"))),
        "addr64"         => format!("{:016x}", addr64(&js(p, "gen"), &hx(p, "residu"))),
        "to_hex"         => to_hex(&hx(p, "bytes")),
        "xor_delta"      => to_hex(&xor_delta(&hx(p, "a"), &hx(p, "b"))),
        "popcount_bytes" => popcount_bytes(&hx(p, "bytes")).to_string(),
        _ => return None,
    })
}

/// Teste un bion : execute, content-adresse (in|out) = empreinte, grave le tsoin reproductible.
fn test_one(bion: &str, payload: &str) {
    match run_bion(bion, payload) {
        Some(out) => {
            let addr = fnv1a64(&format!("{bion}|{payload}|{out}"));
            let body = format!(
                "{{\"bion\":\"{}\",\"in\":\"{}\",\"out\":\"{}\",\"addr\":\"{:016x}\",\"ok\":true}}",
                bion, esc(payload), esc(&out), addr
            );
            tsoin_record(&format!("test:bion:{}:{:016x}", bion, addr), body.as_bytes());
            emit("bion.tested", body.as_bytes());
            log(&format!("bion-tester: {bion}({payload}) = {out}  [empreinte {:016x}]", addr));
        }
        None => log(&format!("bion-tester: bion inconnu '{bion}'")),
    }
}

/// La suite de reference (chaque cas = un tsoin reproductible).
const SUITE: &[(&str, &str)] = &[
    ("add", "{\"a\":2,\"b\":3}"),
    ("mul", "{\"a\":6,\"b\":7}"),
    ("square", "{\"a\":7}"),
    ("popcount", "{\"a\":255}"),
    ("xor", "{\"a\":12,\"b\":10}"),
    ("fnv1a64", "{\"s\":\"hello\"}"),
    ("addr64", "{\"gen\":\"carre\",\"residu\":\"00\"}"),
    ("xor_delta", "{\"a\":\"aabb\",\"b\":\"aacc\"}"),
    ("popcount_bytes", "{\"bytes\":\"ff0f\"}"),
];

#[no_mangle]
pub extern "C" fn plc_init() {
    log("bion-tester: init (unit-test des bions ; chaque test = un tsoin reproductible = une empreinte)");
}

#[no_mangle]
pub extern "C" fn plc_health() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
    let topic = decode_str(tp, tl);
    let payload = decode_str(pp, pl);
    if topic == "bion.test" {
        let bion = js(&payload, "bion");
        test_one(&bion, &payload);
    } else if topic == "bion.test.all" {
        for &(b, args) in SUITE {
            test_one(b, args);
        }
        log(&format!("bion-tester: suite complete ({} bions testes)", SUITE.len()));
    }
}

#[no_mangle]
pub extern "C" fn plc_goodbye() {
    log("bion-tester: goodbye");
}
