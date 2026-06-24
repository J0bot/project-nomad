//! `tsoin-store` ploxion — une **adresse de bus content-addressed** pour la
//! machine à tsoins : `addr64 = addr64(générateur, résidu)` (le bion
//! [`ploxion_sdk::bions::addr64`], une passe FNV-1a sur la concaténation
//! canonique len-préfixée de `(gen, residu)`). Même couple `(gen, residu)` ⇒
//! même adresse, partout, à jamais — ce qui rend possibles **dédup** et
//! **requête par générateur** côté bus.
//!
//! ## Position vs le ploxion `tsoin`
//! Le ploxion `tsoin` enveloppe un moteur record/replay dont le store est DÉJÀ
//! content-addressed (BLAKE3 + delta Merkle) ; il expose un id séquentiel comme
//! handle lisible sur le bus. `tsoin-store` n'« ajoute » donc pas le
//! content-addressing manquant : il ajoute une **adresse de bus dérivée du couple
//! `(générateur, résidu)`** plus un **index par générateur**, sur des topics
//! distincts. Les deux coexistent sur le bus sans conflit de topic.
//!
//! ## Contrat de bus
//!
//! **requires** (requêtes entrantes) :
//! - `tsoin.put` — `{"gen":<str>, "residu_hex":<hex>}`. Calcule
//!   `addr = addr64(gen, residu)`, stocke `addr -> (gen, residu)` si NOUVEAU,
//!   sinon **déduplique** (n'écrit rien) et incrémente le refcount — APRÈS avoir
//!   vérifié que les octets détenus sont identiques (sinon collision FNV : voir
//!   plus bas). Répond `tsoin.stored
//!   {"addr":<hex64>, "dedup":<bool>, "collision":<bool>, "refcount":<n>, "gen":<str>}`.
//! - `tsoin.get` — `{"addr":<hex64>}` (16 hex, préfixe `0x` toléré). Répond
//!   `tsoin.got {"addr":<hex64>, "gen":<str>, "residu_hex":<hex>, "found":<bool>}`.
//!   Si absent : `found:false`, `gen:""`, `residu_hex:""`.
//! - `tsoin.query` — `{"gen":<str>}`. Liste les adresses détenues pour CE
//!   générateur (index inverse, scan déterministe trié). Répond `tsoin.queried
//!   {"gen":<str>, "count":<n>, "addrs":[<hex64>,…]}`.
//!
//! **provides** : `tsoin.stored`, `tsoin.got`, `tsoin.queried`.
//!
//! ## Adresse en hex 64-bit
//! L'adresse est un `u64` rendu en **16 chiffres hex minuscule** (`{:016x}`),
//! SANS préfixe en sortie ; en entrée `tsoin.get` tolère un préfixe `0x`. Les
//! octets du résidu voyagent eux aussi en hex, **normalisés en minuscule** en
//! sortie (l'égalité se fait sur octets, pas sur string).
//!
//! ## Collision (honnêteté de la dédup)
//! `addr64` est un handle 64-bit non-cryptographique. Sur une adresse déjà
//! connue, on COMPARE les octets `(gen, residu)` détenus à ceux entrants : si
//! identiques ⇒ vraie dédup (`dedup:true`) ; si différents ⇒ **collision FNV**
//! (`dedup:false, collision:true`), on REFUSE d'écraser/dédup (le contenu détenu
//! est préservé), on rembobine le refcount et on logge. Jamais de data-loss
//! silencieux.
//!
//! ## État
//! `thread_local` : une `HashMap<u64, Entry>` (gen + octets du résidu) plus la
//! [`DedupTable`] partagée (refcounts). Vit dans la propre mémoire WASM du
//! ploxion, isolée des autres. L'hôte drop tout le Store au goodbye.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::collections::HashMap;

use ploxion_sdk::bions::{addr64, decode_event, from_hex, json_esc, json_str, to_hex, DedupTable};
use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};

// PLC manifest : on PRODUIT les réponses store, on CONSOMME les requêtes store.
// Pas de clé `capabilities` (aligné sur le ploxion `tsoin` voisin ; aucune
// capability réseau).
export_manifest!(
    r#"{"id":"tsoin-store","version":"1.0.0","provides":["tsoin.stored","tsoin.got","tsoin.queried"],"requires":["tsoin.put","tsoin.get","tsoin.query"],"children_types":[],"parent_types":[]}"#
);

/// Le résidu détenu à une adresse : le générateur + les octets bruts du réel.
/// (Le refcount vit dans la [`DedupTable`], pas ici — séparation bion/résidu.)
struct Entry {
    gen: String,
    residu: Vec<u8>,
}

/// L'état complet du ploxion : le store content-addressed + la table de dédup.
/// `store[addr]` = les octets ; `dedup.refcount(addr)` = combien de fois gravé.
struct Store {
    store: HashMap<u64, Entry>,
    dedup: DedupTable,
}

impl Store {
    fn new() -> Self {
        Self {
            store: HashMap::new(),
            dedup: DedupTable::new(),
        }
    }
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::new());
}

// --- cycle de vie PLC (multi-topic : on garde notre propre plc_on_event) -----
ploxion_lifecycle!(
    init: "tsoin-store: init (adressage génératif: addr64(gen, residu) sur concat len-préfixée, dédup vérifiée par refcount)",
    goodbye: "tsoin-store: goodbye",
);

// --- handlers de bus ---------------------------------------------------------

/// Issue d'un `put` : ce qu'on répond dans `tsoin.stored`.
struct PutOutcome {
    dedup: bool,
    collision: bool,
    refcount: u64,
}

/// `tsoin.put {"gen","residu_hex"}` → grave (ou déduplique) un tsoin et répond
/// `tsoin.stored`. L'adresse ne dépend QUE de `(gen, residu)`.
fn handle_put(payload: &str) {
    let gen = match json_str(payload, "gen") {
        Some(g) => g,
        None => {
            log("tsoin-store: put dropped — payload has no \"gen\"");
            return;
        }
    };
    let hex = match json_str(payload, "residu_hex") {
        Some(h) => h,
        None => {
            log("tsoin-store: put dropped — payload has no \"residu_hex\"");
            return;
        }
    };
    let residu = match from_hex(&hex) {
        Some(b) => b,
        None => {
            log("tsoin-store: put dropped — \"residu_hex\" is not valid hex");
            return;
        }
    };

    // ADRESSE GÉNÉRATIVE = le bion addr64.
    let addr = addr64(&gen, &residu);

    let outcome = STORE.with(|s| {
        let mut st = s.borrow_mut();
        // `touch` insère (inserted=true) OU incrémente (inserted=false).
        let (refcount, inserted) = st.dedup.touch(addr);
        if inserted {
            // NOUVEAU contenu : on stocke les octets une seule fois.
            st.store.insert(
                addr,
                Entry {
                    gen: gen.clone(),
                    residu: residu.clone(),
                },
            );
            PutOutcome {
                dedup: false,
                collision: false,
                refcount,
            }
        } else {
            // Adresse déjà connue : VÉRIFIER les octets avant de conclure dédup.
            let same = st
                .store
                .get(&addr)
                .map(|e| e.gen == gen && e.residu == residu)
                .unwrap_or(false);
            if same {
                // Vraie dédup : même contenu, on garde le refcount incrémenté.
                PutOutcome {
                    dedup: true,
                    collision: false,
                    refcount,
                }
            } else {
                // COLLISION FNV : contenu DIFFÉRENT à la même adresse. On refuse
                // d'écraser ou de compter ce put comme une dédup — on rembobine.
                st.dedup.untouch(addr);
                PutOutcome {
                    dedup: false,
                    collision: true,
                    refcount: st.dedup.refcount(addr),
                }
            }
        }
    });

    if outcome.collision {
        log(&format!(
            "tsoin-store: put gen='{}' ({} octets) -> addr {:016x} COLLISION FNV (contenu détenu préservé, put refusé)",
            gen,
            residu.len(),
            addr
        ));
    } else {
        log(&format!(
            "tsoin-store: put gen='{}' ({} octets) -> addr {:016x} ({}, refcount {})",
            gen,
            residu.len(),
            addr,
            if outcome.dedup { "DÉDUP" } else { "stocké" },
            outcome.refcount
        ));
    }

    emit(
        "tsoin.stored",
        format!(
            "{{\"addr\":\"{:016x}\",\"dedup\":{},\"collision\":{},\"refcount\":{},\"gen\":\"{}\"}}",
            addr,
            outcome.dedup,
            outcome.collision,
            outcome.refcount,
            json_esc(&gen)
        )
        .as_bytes(),
    );
}

/// `tsoin.get {"addr":<hex64>}` → renvoie le tsoin à cette adresse via
/// `tsoin.got`. `found:false` (+ champs vides) si l'adresse est inconnue.
fn handle_get(payload: &str) {
    let addr_hex = match json_str(payload, "addr") {
        Some(a) => a,
        None => {
            log("tsoin-store: get dropped — payload has no \"addr\"");
            return;
        }
    };
    // Tolère un préfixe 0x optionnel ; la sortie, elle, est toujours sans préfixe.
    let addr = match u64::from_str_radix(addr_hex.strip_prefix("0x").unwrap_or(&addr_hex), 16) {
        Ok(a) => a,
        Err(_) => {
            log("tsoin-store: get dropped — \"addr\" is not a hex u64");
            return;
        }
    };

    let json = STORE.with(|s| {
        let st = s.borrow();
        match st.store.get(&addr) {
            Some(e) => {
                let hex = to_hex(&e.residu);
                log(&format!(
                    "tsoin-store: get addr {:016x} -> gen='{}' ({} octets)",
                    addr,
                    e.gen,
                    e.residu.len()
                ));
                format!(
                    "{{\"addr\":\"{:016x}\",\"gen\":\"{}\",\"residu_hex\":\"{}\",\"found\":true}}",
                    addr,
                    json_esc(&e.gen),
                    hex
                )
            }
            None => {
                log(&format!("tsoin-store: get addr {:016x} -> NOT FOUND", addr));
                format!(
                    "{{\"addr\":\"{:016x}\",\"gen\":\"\",\"residu_hex\":\"\",\"found\":false}}",
                    addr
                )
            }
        }
    });

    emit("tsoin.got", json.as_bytes());
}

/// `tsoin.query {"gen":<str>}` → liste les adresses détenues pour CE générateur,
/// via `tsoin.queried`. C'est un **index inverse** (scan linéaire trié), pas une
/// recalcul d'adresse : depuis le seul générateur on ne peut pas dériver l'addr
/// (il faut le résidu). Le tri rend la réponse déterministe.
fn handle_query(payload: &str) {
    let gen = match json_str(payload, "gen") {
        Some(g) => g,
        None => {
            log("tsoin-store: query dropped — payload has no \"gen\"");
            return;
        }
    };

    let (count, addrs_json) = STORE.with(|s| {
        let st = s.borrow();
        let mut addrs: Vec<String> = st
            .store
            .iter()
            .filter(|(_, e)| e.gen == gen)
            .map(|(addr, _)| format!("\"{:016x}\"", addr))
            .collect();
        // Tri stable : la requête est déterministe quel que soit l'ordre de hash.
        addrs.sort();
        (addrs.len(), addrs.join(","))
    });

    log(&format!(
        "tsoin-store: query gen='{}' -> {} adresse(s)",
        gen, count
    ));

    emit(
        "tsoin.queried",
        format!(
            "{{\"gen\":\"{}\",\"count\":{},\"addrs\":[{}]}}",
            json_esc(&gen),
            count,
            addrs_json
        )
        .as_bytes(),
    );
}

#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let (topic, payload) = decode_event(topic_ptr, topic_len, payload_ptr, payload_len);
    match topic.as_str() {
        "tsoin.put" => handle_put(&payload),
        "tsoin.get" => handle_get(&payload),
        "tsoin.query" => handle_query(&payload),
        _ => {}
    }
}
