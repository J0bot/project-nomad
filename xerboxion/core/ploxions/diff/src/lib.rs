//! `diff` ploxion — le **cœur du résidu** de la machine à tsoins, sorti du moteur
//! tsoin et posé sur le bus xion.
//!
//! La triade de la machine à tsoins est `gen -> diff -> store` :
//! - un **générateur** produit un état (les octets du réel) ;
//! - **diff** réduit deux états successifs `(a, b)` à leur **résidu minimal** —
//!   ce qui DIFFÈRE, pas l'état entier ;
//! - **tsoin-store** grave ce résidu, content-addressed et dédupliqué.
//!
//! Ce ploxion ferme l'anneau du milieu : il donne à TOUT ploxion le moyen de
//! produire son résidu minimal pour nourrir `tsoin-store`. Et il le **câble**
//! réellement : si `diff.compare` porte un champ `gen`, `diff` émet en plus un
//! `tsoin.put {"gen","residu_hex"}` consommé par `tsoin-store` — l'anneau du
//! milieu se ferme alors de bout en bout (`gen -> diff -> store`).
//!
//! ## Le résidu, concrètement
//! Entre `a` et `b` on calcule le **XOR-delta** (bion [`ploxion_sdk::bions::xor_delta`]) :
//! réversible (`b == a ^ delta`), tout-zéro là où rien n'a changé. Puis :
//! - **résidu MINIMAL** = run-length des zéros du delta
//!   (bion [`ploxion_sdk::bions::residu_minimal`]) — deux états identiques ⇒
//!   résidu minuscule, `minimal:true`. NB : « minimal » vaut pour les deltas
//!   SPARSE (longues plages de zéros, le cas attendu de la machine à tsoins) ;
//!   un delta dense peut gonfler (chaque transition zéro/non-zéro coûte 5 octets
//!   d'en-tête). C'est du RLE des zéros, pas une borne de compressibilité ;
//! - **surprise** = popcount du delta (bion [`ploxion_sdk::bions::popcount_bytes`])
//!   = le nombre exact de bits qui diffèrent = la **distance de Hamming en bits**
//!   ⇒ `surprise_bits` (PAS l'entropie de Shannon — c'est un poids de Hamming) ;
//! - **distance** = le nombre d'octets non nuls du delta (les positions qui ont
//!   bougé) ⇒ une distance de Hamming au niveau octet.
//!
//! ## Contrat de bus
//!
//! **requires** : `diff.compare` —
//! `{"a_hex":<hex>, "b_hex":<hex>, "gen":<str>?}`. Les deux états en hex
//! (longueurs quelconques, le delta gère le débordement). Préfixe `0x` toléré
//! sur chaque champ hex ; hex insensible à la casse. `gen` est OPTIONNEL : s'il
//! est présent, `diff` émet aussi `tsoin.put` (voir provides) pour graver le
//! résidu dans `tsoin-store` sous ce générateur.
//!
//! **provides** :
//! - `diff.residu` —
//!   `{"residu_hex":<hex>, "distance":<n>, "minimal":<bool>, "surprise_bits":<n>}`.
//!   - `residu_hex` = le résidu MINIMAL, **COMPRESSÉ** (delta run-length encodé)
//!     en hex minuscule. ⚠️ C'est le delta COMPRESSÉ, PAS le delta brut : pour
//!     retrouver `surprise_bits`/`distance` côté consommateur il faut d'abord
//!     `residu_expand(from_hex(residu_hex))` (le bion
//!     [`ploxion_sdk::bions::residu_expand`]) pour ré-obtenir le delta brut, puis
//!     `popcount_bytes` (surprise) / compter les octets non nuls (distance) ;
//!   - `distance`   = nombre d'octets non nuls dans le delta BRUT (octets changés) ;
//!   - `minimal`    = `true` ssi le delta est entièrement nul (`a == b` au sens
//!     XOR : aucune surprise, le résidu est à son plancher) ;
//!   - `surprise_bits` = popcount du delta BRUT (bits qui diffèrent = Hamming).
//! - `tsoin.put` (CONDITIONNEL : seulement si `diff.compare` portait un `gen`) —
//!   `{"gen":<str>, "residu_hex":<hex>}`, exactement le contrat d'entrée de
//!   `tsoin-store`, qui répondra `tsoin.stored`. C'est le câblage qui ferme
//!   `gen -> diff -> store`.
//!
//! ## État
//! AUCUN. `diff` est une **fonction pure du bus** : `(a, b[, gen]) -> résidu`.
//! Pas de `thread_local`, pas de capability. Déterministe, rejouable, identique
//! partout — exactement ce qu'un bion de la machine à tsoins doit être.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::bions::{
    decode_event, from_hex, json_esc, json_str, popcount_bytes, residu_minimal, to_hex, xor_delta,
};
use ploxion_sdk::{emit, export_manifest, log, ploxion_lifecycle};

// PLC manifest : on PRODUIT le résidu (et, conditionnellement, un tsoin.put qui
// nourrit tsoin-store), on CONSOMME la requête de comparaison. Pas de clé
// `capabilities` (calcul pur, aucune capability réseau ; aligné sur les ploxions
// voisins tsoin / tsoin-store).
export_manifest!(
    r#"{"id":"diff","version":"1.0.0","provides":["diff.residu","tsoin.put"],"requires":["diff.compare"],"children_types":[],"parent_types":[]}"#
);

// --- cycle de vie PLC (mono-requête, mais on garde notre plc_on_event pour le
//     filtrage de topic explicite, comme tsoin-store) ---------------------------
ploxion_lifecycle!(
    init: "diff: init (cœur du résidu — XOR-delta + run-length des zéros + popcount de surprise ; gen -> diff -> store)",
    goodbye: "diff: goodbye",
);

// --- handler de bus ----------------------------------------------------------

/// Décode un champ hex `key` du payload (préfixe `0x` toléré) en octets bruts.
/// `None` (et un log) si absent ou si la string n'est pas du hex valide.
fn field_bytes(payload: &str, key: &str) -> Option<Vec<u8>> {
    let hex = match json_str(payload, key) {
        Some(h) => h,
        None => {
            log(&format!("diff: compare dropped — payload has no \"{key}\""));
            return None;
        }
    };
    let hex = hex.strip_prefix("0x").unwrap_or(&hex);
    match from_hex(hex) {
        Some(b) => Some(b),
        None => {
            log(&format!("diff: compare dropped — \"{key}\" is not valid hex"));
            None
        }
    }
}

/// `diff.compare {"a_hex","b_hex","gen"?}` → calcule le résidu et répond
/// `diff.residu` ; si `gen` est fourni, émet AUSSI `tsoin.put` pour graver le
/// résidu dans `tsoin-store`. Fonction pure : aucune mutation d'état, mêmes
/// entrées ⇒ mêmes sorties.
fn handle_compare(payload: &str) {
    let a = match field_bytes(payload, "a_hex") {
        Some(v) => v,
        None => return,
    };
    let b = match field_bytes(payload, "b_hex") {
        Some(v) => v,
        None => return,
    };

    // 1) Le delta brut, réversible : b == a ^ delta (sur le préfixe commun).
    let delta = xor_delta(&a, &b);

    // 2) La surprise = bits qui diffèrent (popcount du delta) = Hamming en bits.
    let surprise_bits = popcount_bytes(&delta);

    // 3) La distance = octets non nuls du delta (positions qui ont bougé).
    let distance = delta.iter().filter(|&&x| x != 0).count() as u64;

    // 4) minimal = aucune surprise ⇒ le delta est entièrement nul ⇒ a == b.
    let minimal = surprise_bits == 0;

    // 5) Le résidu MINIMAL = run-length des zéros du delta. Deux états
    //    identiques ⇒ delta tout-zéro ⇒ résidu minuscule.
    let residu = residu_minimal(&delta);
    let residu_hex = to_hex(&residu);

    log(&format!(
        "diff: compare a={} b={} octets -> delta {} octets, distance {}, surprise {} bits, minimal {}, résidu {} octets",
        a.len(),
        b.len(),
        delta.len(),
        distance,
        surprise_bits,
        minimal,
        residu.len()
    ));

    emit(
        "diff.residu",
        format!(
            "{{\"residu_hex\":\"{}\",\"distance\":{},\"minimal\":{},\"surprise_bits\":{}}}",
            residu_hex, distance, minimal, surprise_bits
        )
        .as_bytes(),
    );

    // 6) Câblage de la triade : si la requête porte un `gen`, on grave le résidu
    //    dans tsoin-store. C'est ce qui FERME `gen -> diff -> store` (sinon diff
    //    se contente de répondre diff.residu, fonction pure du bus).
    if let Some(gen) = json_str(payload, "gen") {
        log(&format!(
            "diff: gen='{}' présent -> tsoin.put ({} octets de résidu) — gen -> diff -> store",
            gen,
            residu.len()
        ));
        emit(
            "tsoin.put",
            format!(
                "{{\"gen\":\"{}\",\"residu_hex\":\"{}\"}}",
                json_esc(&gen),
                residu_hex
            )
            .as_bytes(),
        );
    }
}

#[no_mangle]
pub extern "C" fn plc_on_event(
    topic_ptr: i32,
    topic_len: i32,
    payload_ptr: i32,
    payload_len: i32,
) {
    let (topic, payload) = decode_event(topic_ptr, topic_len, payload_ptr, payload_len);
    if topic == "diff.compare" {
        handle_compare(&payload);
    }
}
