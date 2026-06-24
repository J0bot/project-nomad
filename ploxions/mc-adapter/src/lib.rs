//! `mc-adapter` ploxion — le pont **Minecraft → tsoins**.
//!
//! La lane web a posé une route `/xion/emit` (navigateur → bus du xion) et émet
//! des événements **`minecraft.command`** (ex `/setblock ~ ~ ~ grass`). Ce ploxion
//! les **écoute**, **grave chaque commande en tsoin** (record/replay bit-exact :
//! générateur = le nom adressé, résidu = la commande complète en hex), et **ré-émet
//! `mc.event`** normalisé pour les ploxions `carte`/`cosmos`. C'est la boucle
//! « jouer = capturer le tsoin » fermée **sans serveur Paper** : le réel (la partie)
//! mène, le xion suit et logge.
//!
//! ## Contrat du bus (le format attendu de `minecraft.command`)
//! Payload JSON : `{"player":"<nom>","dim":"<overworld|nether|end|...>","cmd":"<commande>","t":<ms>}`
//! (seul `cmd` est requis ; `player`/`dim` par défaut `?`/`overworld`).
//! - **requires** : `minecraft.command`
//! - **provides** : `mc.event` (`{kind:"command",player,dim,cmd,seq}`) + `tsoin.record`
//!   (`{name:"mc:<dim>:cmd:<seq>", bytes:<payload-hex>}` → routé vers le ploxion `tsoin`).
//!
//! Listener pur (pas de capability) : il consomme le bus, ne fetch rien. Bâti
//! depuis les bions partagés (json_str/json_esc/to_hex/tsoin_record_hex) + la macro
//! `ploxion!` (le cycle de vie est un bion).

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::bions::{json_esc, json_str, to_hex, tsoin_record_hex};
use ploxion_sdk::{emit, export_manifest, log, ploxion};
use std::cell::RefCell;

export_manifest!(
    r#"{"id":"mc-adapter","version":"1.0.0","capabilities":[],"provides":["mc.event","tsoin.record"],"requires":["minecraft.command"],"children_types":[],"parent_types":[]}"#
);

thread_local! {
    /// Séquence locale : adresse chaque commande de façon unique dans son tsoin.
    static SEQ: RefCell<u64> = const { RefCell::new(0) };
}

fn run(payload: &str) {
    let cmd = json_str(payload, "cmd").unwrap_or_default();
    if cmd.is_empty() {
        return; // rien à graver
    }
    let player = json_str(payload, "player").unwrap_or_else(|| "?".to_string());
    let dim = json_str(payload, "dim").unwrap_or_else(|| "overworld".to_string());
    let seq = SEQ.with(|s| {
        let mut s = s.borrow_mut();
        *s += 1;
        *s
    });

    // 1) grave le tsoin : générateur = nom adressé, résidu = le payload complet (hex).
    //    Rejouer ce tsoin = ré-émettre la commande exacte.
    let name = format!("mc:{dim}:cmd:{seq}");
    tsoin_record_hex(&name, &to_hex(payload.as_bytes()));

    // 2) ré-émet normalisé pour carte/cosmos (le monde 3D / le Nexus).
    emit(
        "mc.event",
        format!(
            "{{\"kind\":\"command\",\"player\":\"{}\",\"dim\":\"{}\",\"cmd\":\"{}\",\"seq\":{}}}",
            json_esc(&player),
            json_esc(&dim),
            json_esc(&cmd),
            seq
        )
        .as_bytes(),
    );
    log(&format!("mc-adapter: {player}@{dim} -> {cmd} (tsoin {name})"));
}

ploxion!(
    init: "mc-adapter: init (pont Minecraft -> tsoins ; ecoute minecraft.command, grave + re-emet mc.event)",
    goodbye: "mc-adapter: goodbye",
    on "minecraft.command" => run
);
