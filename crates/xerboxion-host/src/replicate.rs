//! # `replicate` — the UNIVERSAL CONSTRUCTOR.
//!
//! The xion is a **von Neumann machine**: it can *construct a copy of itself
//! elsewhere*. This module is that constructor — it turns a node's
//! reconstructable state into a portable **bundle**, and materializes a bundle
//! into a (fresh) state dir so a `serve --state-dir <dir>` boot comes up
//! IDENTICAL. The self-reproducing satellite « pourriture 4 ».
//!
//! ## What a bundle carries (see `docs/VON-NEUMANN-STATE-MAP.md`)
//! - **CODE** — the runtime-loaded ploxion DELTA (the part NOT in the base
//!   image): each [`crate::state::LoadedRecord`], with the wasm bytes inline
//!   (base64) for `Source::Wasm` records and id-only for `Source::Staged` (their
//!   bytes reload from the base `<ploxions_dir>`, which a copy also has).
//! - **DATA** — a manifest of the SOT data stores that must travel too (José:
//!   *« les données du repoverse il faudra les mettre aussi »*). Each
//!   [`DataRef`] names a store + its capture method + the id of its captured
//!   **tsoin** snapshot once a lane records one. v0 carries the manifest; the
//!   owning lanes fill `tsoin` as they snapshot (the bus stays the transport).
//!
//! ## Why CODE travels but base does not
//! The base set always reloads from the image's `<ploxions_dir>` — replicating
//! it would double-load. We replicate ONLY the delta, exactly the same rule as
//! durable persistence (`docs/PERSISTENCE-v0.md`). A copy reconstructs: base
//! from its own image + delta from the bundle = the same loaded set.
//!
//! Secrets NEVER travel here (env/keys are out-of-band — see the state map).

use anyhow::{Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::state::{Source, StateStore};

/// Bundle schema version. Bumped on an incompatible shape change so a copy can
/// refuse a bundle it cannot reconstruct from (rather than mis-build).
pub const BUNDLE_VERSION: u32 = 1;

/// One reconstructable ploxion (CODE). For `Source::Wasm` the bytes travel
/// inline (`wasm_b64`); for `Source::Staged` only the id (bytes are in the base
/// image the copy also has).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeRecord {
    /// The ploxion's manifest id (its bus identity).
    pub id: String,
    /// How it was originally loaded — decides where its bytes come from.
    pub source: Source,
    /// Base64 wasm bytes — `Some` ONLY for `Source::Wasm`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wasm_b64: Option<String>,
}

/// One SOT data store the copy must also carry (per the state map). v0 declares
/// it; `tsoin` is the id of its captured snapshot once the owning lane records
/// one (`None` = declared but not yet snapshotted).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataRef {
    /// Logical store name, e.g. `"repoverse-postgres"`, `"site-mariadb"`.
    pub store: String,
    /// How it is captured, e.g. `"pg_dump"`, `"mariadb-dump"`, `"mc-mirror"`.
    pub capture: String,
    /// The captured tsoin snapshot id (filled by the owning lane), or `None`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tsoin: Option<String>,
}

/// Everything needed to construct a copy of a node: CODE + a DATA manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplicaBundle {
    /// Schema version ([`BUNDLE_VERSION`]).
    pub version: u32,
    /// The runtime-loaded ploxion delta (code), in load order.
    pub code: Vec<CodeRecord>,
    /// SOT data stores to carry (manifest; `tsoin` filled by lanes).
    #[serde(default)]
    pub data: Vec<DataRef>,
}

impl ReplicaBundle {
    /// Attach/replace a data-store reference (a lane registers its SOT store, and
    /// later its captured tsoin id). Idempotent on `store`.
    pub fn with_data(mut self, r: DataRef) -> Self {
        if let Some(slot) = self.data.iter_mut().find(|d| d.store == r.store) {
            *slot = r;
        } else {
            self.data.push(r);
        }
        self
    }
}

/// EXPORT — read a node's reconstructable CODE state from its state dir into a
/// bundle. The DATA manifest starts empty; callers attach [`DataRef`]s (the
/// lanes' SOT stores) via [`ReplicaBundle::with_data`].
pub fn export(store: &StateStore) -> Result<ReplicaBundle> {
    let records = store
        .read_records()
        .context("replicate: reading state records")?;
    let mut code = Vec::with_capacity(records.len());
    for rec in records {
        let wasm_b64 = match rec.source {
            Source::Wasm => {
                let bytes = store
                    .read_wasm(&rec.id)
                    .with_context(|| format!("replicate: reading wasm for '{}'", rec.id))?;
                Some(base64::engine::general_purpose::STANDARD.encode(bytes))
            }
            Source::Staged => None,
        };
        code.push(CodeRecord { id: rec.id, source: rec.source, wasm_b64 });
    }
    Ok(ReplicaBundle { version: BUNDLE_VERSION, code, data: Vec::new() })
}

/// IMPORT — materialize a bundle's CODE into a (fresh) state dir. After this, a
/// `serve --state-dir <dir>` boot RESTORES exactly these ploxions on top of the
/// base set → an identical running node. Returns the count written.
///
/// Refuses a bundle whose version it cannot reconstruct from. A `Source::Wasm`
/// record MUST carry its bytes; a `Source::Staged` record needs the same base
/// image present (its bytes reload from `<ploxions_dir>`).
pub fn import(store: &StateStore, bundle: &ReplicaBundle) -> Result<usize> {
    if bundle.version != BUNDLE_VERSION {
        anyhow::bail!(
            "replicate: bundle version {} unsupported (this node reconstructs v{BUNDLE_VERSION})",
            bundle.version
        );
    }
    let mut n = 0usize;
    for rec in &bundle.code {
        let bytes: Option<Vec<u8>> = match rec.source {
            Source::Wasm => {
                let b64 = rec.wasm_b64.as_ref().with_context(|| {
                    format!("replicate: source=wasm '{}' but bundle carries no bytes", rec.id)
                })?;
                let raw = base64::engine::general_purpose::STANDARD
                    .decode(b64.trim())
                    .with_context(|| format!("replicate: bad base64 for '{}'", rec.id))?;
                if raw.is_empty() {
                    anyhow::bail!("replicate: '{}' decoded to empty wasm", rec.id);
                }
                Some(raw)
            }
            Source::Staged => None,
        };
        store
            .persist(&rec.id, rec.source, bytes.as_deref())
            .with_context(|| format!("replicate: persisting '{}'", rec.id))?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "xion-replicate-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    /// The universal constructor: a node's CODE state exports, then imports into
    /// a FRESH dir IDENTICALLY — staged id-only, wasm bytes bit-exact.
    #[test]
    fn export_then_import_reconstructs_identical_code() {
        let src = tmp_dir("src");
        let store_a = StateStore::open(&src).unwrap();
        store_a.persist("alpha", Source::Staged, None).unwrap();
        let wasm = b"\0asm-fake-module-bytes-\xff\x01\x02";
        store_a.persist("beta", Source::Wasm, Some(wasm)).unwrap();

        let bundle = export(&store_a).unwrap();
        assert_eq!(bundle.version, BUNDLE_VERSION);
        assert_eq!(bundle.code.len(), 2);

        // round-trip through JSON (the wire form of GET /replicate)
        let json = serde_json::to_string(&bundle).unwrap();
        let wire: ReplicaBundle = serde_json::from_str(&json).unwrap();

        let dst = tmp_dir("dst");
        let store_b = StateStore::open(&dst).unwrap();
        let n = import(&store_b, &wire).unwrap();
        assert_eq!(n, 2);

        // the reconstructed dir holds the SAME delta
        let recs = store_b.read_records().unwrap();
        assert_eq!(recs.len(), 2);
        assert!(recs.iter().any(|r| r.id == "alpha" && r.source == Source::Staged));
        assert!(recs.iter().any(|r| r.id == "beta" && r.source == Source::Wasm));
        // and the wasm bytes came back BIT-EXACT across export→json→import
        assert_eq!(store_b.read_wasm("beta").unwrap(), wasm);

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }

    #[test]
    fn data_manifest_attaches_and_is_idempotent() {
        let b = ReplicaBundle { version: BUNDLE_VERSION, code: vec![], data: vec![] }
            .with_data(DataRef { store: "repoverse-postgres".into(), capture: "pg_dump".into(), tsoin: None })
            .with_data(DataRef { store: "repoverse-postgres".into(), capture: "pg_dump".into(), tsoin: Some("tsoin:abc".into()) });
        assert_eq!(b.data.len(), 1, "same store replaced, not duplicated");
        assert_eq!(b.data[0].tsoin.as_deref(), Some("tsoin:abc"));
    }

    #[test]
    fn refuses_unknown_version() {
        let dir = tmp_dir("ver");
        let store = StateStore::open(&dir).unwrap();
        let bad = ReplicaBundle { version: 9999, code: vec![], data: vec![] };
        assert!(import(&store, &bad).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
