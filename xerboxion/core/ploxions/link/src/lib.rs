//! `link` ploxion — le linkifier universel, qui tourne partout.
//!
//! Sur `link.process` (payload `{"text":"..."}`) il scanne le texte et **active les liens** :
//! - les URLs (`http(s)://...`) → `<a href target=_blank>` ;
//! - chaque **kion** (tsoin, ploxion, kion, bion, cubion, koin, wormion, xion, xerboxion, xerax…)
//!   → un lien vers sa page terme `/wiki/term/<slug>` (le slug = la forme singulière).
//! Émet `link.result` (`{"html":"...","liens":N}`). Déterministe ; le site/coord l'appelle pour
//! rendre n'importe quel texte. Le tout HTML-échappé pour ne pas casser le rendu.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};
use ploxion_sdk::bions::{esc, is_url, json_esc, json_str, decode_str};

export_manifest!(
    r#"{"id":"link","version":"1.0.0","capabilities":[],"provides":["link.result"],"requires":["link.process"],"children_types":[],"parent_types":[]}"#
);

/// Le vocabulaire xerboxion (les kions). Forme singulière = le slug du terme.
const KIONS: &[&str] = &[
    "tsoin", "ploxion", "kion", "bion", "cubion", "koin", "wormion", "xion", "xerboxion",
    "spherion", "plobion", "boxion", "portion", "xerion", "xerxion", "xerax", "xerbot",
    "certitude", "generateur", "residu", "adressage", "koin",
];

/// Si `core` est un kion (singulier ou pluriel), renvoie son slug singulier.
fn kion_slug(core: &str) -> Option<&'static str> {
    let lc: String = core.to_lowercase();
    let singular = lc.strip_suffix('s').unwrap_or(lc.as_str());
    for &k in KIONS {
        if k == lc.as_str() || k == singular {
            return Some(k);
        }
    }
    None
}

/// Linkifie un token : sépare préfixe/suffixe de ponctuation, traite le cœur.
fn linkify_token(tok: &str, liens: &mut u32) -> String {
    let start = tok.find(|c: char| c.is_alphanumeric()).unwrap_or(tok.len());
    let end = tok
        .rfind(|c: char| c.is_alphanumeric())
        .map(|i| i + tok[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1))
        .unwrap_or(tok.len());
    if start >= end {
        return esc(tok);
    }
    let prefix = &tok[..start];
    let core = &tok[start..end];
    let suffix = &tok[end..];
    let inner = if is_url(core) {
        *liens += 1;
        format!(
            "<a href=\"{}\" target=\"_blank\" rel=\"noopener\">{}</a>",
            esc(core),
            esc(core)
        )
    } else if let Some(slug) = kion_slug(core) {
        *liens += 1;
        format!("<a href=\"/wiki/term/{}\" class=\"kion\">{}</a>", slug, esc(core))
    } else {
        esc(core)
    };
    format!("{}{}{}", esc(prefix), inner, esc(suffix))
}

fn process(text: &str, liens: &mut u32) -> String {
    text.split('\n')
        .map(|line| {
            line.split(' ')
                .map(|tok| linkify_token(tok, liens))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

ploxion_lifecycle!(init: "link: init (linkifier universel ; URLs + kions -> /wiki/term/<slug>)", goodbye: "link: goodbye");

#[no_mangle]
pub extern "C" fn plc_on_event(topic_ptr: i32, topic_len: i32, payload_ptr: i32, payload_len: i32) {
    let topic = decode_str(topic_ptr, topic_len);
    if topic != "link.process" {
        return;
    }
    let payload = decode_str(payload_ptr, payload_len);
    let text = json_str(&payload, "text").unwrap_or_default();
    let mut liens = 0u32;
    let html = process(&text, &mut liens);
    log(&format!("link: {liens} lien(s) actives"));
    emit(
        "link.result",
        format!("{{\"html\":\"{}\",\"liens\":{}}}", json_esc(&html), liens).as_bytes(),
    );
}

