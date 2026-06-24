//! `generator` ploxion — **la moitie REEL de l'adressage genuine** : le sens
//! `residu -> reel`. Partout dans le xion les ploxions font `reel -> residu` —
//! la `carte` replie un amas en cubions, `kion` collapse, `spectre` decompose,
//! `diff` reduit deux etats a leur XOR-delta, `xerbion` predit son entree et
//! **JETTE** sa prediction (il n'emet que l'erreur). Il manquait l'INVERSE :
//! **reconstruire le reel** a partir du residu (+ la base = l'etat precedent).
//! C'est le **replay** — la moitie qui rend la compression non seulement
//! reversible mais **verifiable**.
//!
//! ## La boucle de codage predictif (et ses limites HONNETES)
//! Avec `diff` (record = le residu minimal) + `tsoin-store` (store = l'adresse
//! content-addressed) + `generator` (replay = residu -> reel), l'anneau de la
//! machine a tsoins se ferme cote DONNEES : on grave UN residu minimal, on
//! l'adresse, et on peut RECONSTRUIRE le reel a la demande sans avoir stocke le
//! reel entier.
//!
//! ⚠️ **Sur l'adressage, soyons exacts.** `tsoin-store`/`diff` adressent
//! `addr64(gen, residu_minimal)` : le hash porte sur le RESIDU COMPRESSE, sous
//! un nom de generateur `gen` libre. `generator`, lui, re-adresse
//! `addr64("kind:base_hex", reel)` : nom DIFFERENT et contenu DIFFERENT (les
//! octets du REEL reconstruit, pas le residu). Ces deux adresses NE COINCIDENT
//! PAS — ce sont deux schemas d'adressage distincts (residu vs reel). Le champ
//! `verified` de ce ploxion **n'atteste donc PAS** l'egalite avec l'adresse
//! gravee au record par `tsoin-store` : il atteste **l'auto-coherence du REEL
//! reconstruit** sous le schema d'adressage du REEL (`addr64("kind:base_hex", reel)`).
//! Pour obtenir `verified=true`, l'appelant fournit `addr` calcule sous CE meme
//! schema (le REEL, nomme `kind:base_hex`). C'est le **P3 (machine)** au sens
//! ou le generateur s'execute et se re-adresse de facon deterministe ; ce n'est
//! PAS une preuve d'accord avec l'adresse de `tsoin-store`.
//!
//! ⚠️ **Limite de reversibilite xor-delta.** Le XOR-delta de la machine
//! (`xor_delta`/`residu_minimal`, format partage avec `diff`/`tsoin-store`)
//! n'encode PAS la longueur du reel cible. La reconstruction xor-delta est donc
//! lossless **uniquement quand `base.len() <= reel.len()`** (le reel ne retrecit
//! pas sous la base). Si la base est plus longue que le reel, le replay rend un
//! reel de longueur `base.len()` (queue a zero), pas le reel court — utiliser
//! `raw`/`lcg` pour un reel plus court que la base. (Verrou par les tests du SDK,
//! `generator_tests::xor_delta_shrink_is_not_reversible`.)
//!
//! ## Contrat de bus
//!
//! **requires** : `gen.expand` —
//! `{"kind":<str>, "residu_hex":<hex>, "base_hex":<hex>?, "addr":<hex64>?}`.
//! - `kind` ∈ {`"xor-delta"`, `"lcg"`, `"raw"`}, le dispatch ([`gen_apply`]).
//! - `residu_hex` = le residu en hex (prefixe `0x` tolere, casse libre).
//!   * `xor-delta` ⇒ un **residu minimal** (RLE des zeros d'un XOR-delta),
//!     exactement la sortie `residu_hex` de `diff` / le contenu d'un `tsoin-store`.
//!   * `lcg` ⇒ **8 octets** = la graine `u64` little-endian.
//!   * `raw` ⇒ le reel lui-meme.
//! - `base_hex` = l'etat PRECEDENT (le `a` du codage predictif). OPTIONNEL :
//!   absent ⇒ base vide. Pour `xor-delta` la base est le reel d'avant (et doit
//!   etre au plus aussi longue que le reel cible, cf. limite ci-dessus) ; pour
//!   `lcg` elle ne sert QUE de gabarit de longueur (base vide ⇒ reel vide) ;
//!   pour `raw` elle est ignoree.
//! - `addr` = l'adresse ATTENDUE sous le schema du REEL (`{:016x}`, prefixe `0x`
//!   tolere). OPTIONNELLE : si fournie, on compare la re-`addr64` du reel
//!   reconstruit ⇒ `verified`. (Ce n'est PAS l'adresse `tsoin-store`, qui porte
//!   sur le residu.)
//!
//! **provides** : `gen.reel` —
//! `{"kind":<str>, "bytes_hex":<hex>, "addr":<hex64>, "verified":<bool>}`.
//! - `bytes_hex` = le **reel reconstruit** en hex minuscule.
//! - `addr` = `addr64(kind ‖ ":" ‖ base_hex, reel)` re-calculee sur le reel
//!   reconstruit (le meme bion `addr64` que `tsoin-store`, mais applique au REEL
//!   et au nom `kind:base_hex` — un schema d'adressage du REEL, distinct de
//!   `addr64(gen, residu)`). Deux reconstructions de meme `kind` mais de base
//!   differente sont des generateurs differents — d'ou `kind:base` comme nom.
//! - `verified` = `true` ssi un `addr` attendu (schema du REEL) etait fourni ET
//!   egal a la re-`addr64`. `false` si aucun `addr` fourni (rien a verifier) ou
//!   s'il differe. **N'atteste que l'auto-coherence du reel reconstruit**, pas
//!   l'accord avec l'adresse de record `tsoin-store`.
//!
//! ## Etat
//! AUCUN. Comme `diff`, `generator` est une **fonction pure du bus** :
//! `(kind, residu[, base][, addr]) -> reel`. Pas de `thread_local`, pas de
//! capability. Deterministe, rejouable, identique partout.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::bions::{addr64, decode_event, from_hex, gen_apply, json_str, to_hex};
use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};

// PLC manifest : on PRODUIT le reel reconstruit, on CONSOMME la requete de
// reconstruction. Pas de cle `capabilities` (calcul pur, aucune capability
// reseau ; aligne sur les ploxions voisins diff / tsoin-store).
export_manifest!(
    r#"{"id":"generator","version":"1.0.0","provides":["gen.reel"],"requires":["gen.expand"],"children_types":[],"parent_types":[]}"#
);

// --- cycle de vie PLC (mono-requete, mais plc_on_event propre pour le filtrage
//     explicite de topic, comme diff / tsoin-store) ----------------------------
ploxion_lifecycle!(
    init: "generator: init (la moitie REEL de l'adressage genuine — residu -> reel ; replay xor-delta/lcg/raw ; re-addr64 = auto-coherence du REEL reconstruit ; xor-delta reversible ssi base.len() <= reel.len())",
    goodbye: "generator: goodbye",
);

// --- handler de bus ----------------------------------------------------------

/// Decode un champ hex `key` du payload (prefixe `0x` tolere, casse libre) en
/// octets bruts. `None` (et un log) si absent ou si la string n'est pas du hex
/// valide. Meme bion de lecture que `diff::field_bytes`.
fn field_bytes(payload: &str, key: &str) -> Option<Vec<u8>> {
    let hex = match json_str(payload, key) {
        Some(h) => h,
        None => {
            log(&format!(
                "generator: expand dropped — payload has no \"{key}\""
            ));
            return None;
        }
    };
    let hex = hex.strip_prefix("0x").unwrap_or(&hex);
    match from_hex(hex) {
        Some(b) => Some(b),
        None => {
            log(&format!(
                "generator: expand dropped — \"{key}\" is not valid hex"
            ));
            None
        }
    }
}

/// `gen.expand {"kind","residu_hex","base_hex"?,"addr"?}` → reconstruit le reel
/// et repond `gen.reel`. Fonction pure : aucune mutation d'etat, memes entrees
/// ⇒ memes sorties.
fn handle_expand(payload: &str) {
    // kind : le dispatch du generateur (obligatoire).
    let kind = match json_str(payload, "kind") {
        Some(k) => k,
        None => {
            log("generator: expand dropped — payload has no \"kind\"");
            return;
        }
    };

    // residu : le contenu compresse (obligatoire).
    let residu = match field_bytes(payload, "residu_hex") {
        Some(r) => r,
        None => return,
    };

    // base : l'etat precedent (OPTIONNEL ⇒ vide). On garde aussi sa forme hex
    // canonique (minuscule, sans prefixe) car elle entre dans le NOM du
    // generateur pour l'adresse (kind:base_hex).
    let base = match json_str(payload, "base_hex") {
        Some(h) => {
            let h = h.strip_prefix("0x").unwrap_or(&h).to_string();
            match from_hex(&h) {
                Some(b) => b,
                None => {
                    log("generator: expand dropped — \"base_hex\" is not valid hex");
                    return;
                }
            }
        }
        None => Vec::new(),
    };
    // base_hex re-derive depuis les octets ⇒ canonique (minuscule), meme si
    // l'appelant l'avait omis (base vide ⇒ "").
    let base_hex = to_hex(&base);

    // RECONSTRUCTION : le bion gen_apply (le dispatch xor-delta / lcg / raw).
    let reel = match gen_apply(&kind, &residu, &base) {
        Some(r) => r,
        None => {
            log(&format!(
                "generator: expand dropped — gen_apply a echoue (kind='{}', residu {} octets, base {} octets) : kind inconnu ou residu mal forme",
                kind,
                residu.len(),
                base.len()
            ));
            return;
        }
    };
    let bytes_hex = to_hex(&reel);

    // RE-ADRESSE le reel reconstruit sous le SCHEMA DU REEL. Le generateur =
    // `kind:base_hex` (canonique, len-separe par addr64). ⚠️ C'est addr64 sur le
    // REEL (pas sur le residu) : un schema d'adressage DISTINCT de tsoin-store
    // (qui fait addr64(gen, residu)). Cette adresse atteste l'auto-coherence du
    // reel reconstruit, PAS l'accord avec l'adresse de record.
    let gen_name = format!("{}:{}", kind, base_hex);
    let addr = addr64(&gen_name, &reel);

    // VERIFICATION : si un `addr` attendu (schema du REEL) est fourni, on le
    // compare a la re-addr64. verified=true ⇒ le reel reconstruit est coherent
    // avec l'adresse-de-reel annoncee par l'appelant.
    let (verified, expected) = match json_str(payload, "addr") {
        Some(a) => {
            let a = a.strip_prefix("0x").unwrap_or(&a);
            match u64::from_str_radix(a, 16) {
                Ok(exp) => (exp == addr, Some(exp)),
                Err(_) => {
                    log("generator: \"addr\" attendu illisible (pas un hex u64) — non verifie");
                    (false, None)
                }
            }
        }
        None => (false, None),
    };

    match expected {
        Some(_) if verified => log(&format!(
            "generator: expand kind='{}' residu {} octets, base {} octets -> reel {} octets, addr {:016x} VERIFIE (== addr-reel attendue ; auto-coherence du reel reconstruit)",
            kind, residu.len(), base.len(), reel.len(), addr
        )),
        Some(exp) => log(&format!(
            "generator: expand kind='{}' -> reel {} octets, addr {:016x} != attendu {:016x} (le reel reconstruit ne correspond PAS a l'addr-reel annoncee)",
            kind, reel.len(), addr, exp
        )),
        None => log(&format!(
            "generator: expand kind='{}' residu {} octets, base {} octets -> reel {} octets, addr {:016x} (aucune addr attendue ⇒ non verifie)",
            kind, residu.len(), base.len(), reel.len(), addr
        )),
    }

    emit(
        "gen.reel",
        format!(
            "{{\"kind\":\"{}\",\"bytes_hex\":\"{}\",\"addr\":\"{:016x}\",\"verified\":{}}}",
            kind, bytes_hex, addr, verified
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
    if topic == "gen.expand" {
        handle_expand(&payload);
    }
}