//! # `state` — DURABLE persistence for the runtime-loaded ploxion DELTA.
//!
//! The `serve` daemon loads a **base set** of ploxions from `<ploxions_dir>` at
//! startup (those always reload from disk). On top of that, `POST /load`
//! hot-loads MORE ploxions into the running [`crate::Host`] without a restart.
//! That runtime-loaded set is otherwise **ephemeral** — a deploy/crash/restart
//! loses it.
//!
//! This module makes that delta DURABLE. When the daemon is started with a state
//! dir, every successful `POST /load` is PERSISTED here and every `POST /unload`
//! REMOVES the persisted record; on the NEXT startup the daemon restores each
//! persisted ploxion (after loading the base set), so a hot-loaded ploxion
//! SURVIVES a restart.
//!
//! ## What is persisted (the delta ONLY — never the base set)
//! The base set is NOT persisted: it always reloads from `<ploxions_dir>`, so
//! persisting it would DOUBLE-load on restart. We persist only ploxions loaded
//! via `/load` AFTER startup. Each persisted ploxion is one [`LoadedRecord`]:
//! - `id`     — the ploxion's manifest id (its bus identity).
//! - `source` — [`Source::Staged`] (loaded by registered id from
//!   `<ploxions_dir>/<id>.wasm`) or [`Source::Wasm`] (a base64-uploaded module).
//!
//! For a [`Source::Wasm`] record the WASM BYTES are persisted too, at
//! `<state-dir>/loaded/<id>.wasm`, because the bytes are not on disk anywhere
//! else (an uploaded module). For a [`Source::Staged`] record we persist only
//! the id — the bytes reload from `<ploxions_dir>/<id>.wasm` (the same path the
//! original `/load {id}` read), so we never duplicate the staged wasm.
//!
//! ## Layout
//! ```text
//! <state-dir>/
//!   loaded.json          # the manifest: { "version": 1, "loaded": [ {id, source}, ... ] }
//!   loaded/
//!     <id>.wasm          # ONLY for source = "wasm" records
//! ```
//!
//! ## Atomicity + crash-safety
//! Every write is ATOMIC: bytes go to a `<file>.tmp` in the same directory, then
//! a `rename` swaps it into place (rename is atomic on a POSIX filesystem). A
//! crash mid-write leaves the previous good file intact, never a half-written
//! one. The manifest is the source of truth: a stray `loaded/<id>.wasm` not
//! referenced by the manifest is simply ignored on restore (and overwritten on
//! the next persist of that id).
//!
//! ## Graceful degradation (never crash the daemon)
//! Every operation here returns a [`Result`], but the daemon treats persistence
//! as BEST-EFFORT: a missing state dir means "nothing to restore" (normal);
//! corrupt/partial state (bad JSON, a truncated/absent wasm) is skipped with a
//! warning, never a panic. The base set still boots regardless. With NO state
//! dir configured the feature is entirely OFF — the daemon behaves exactly as
//! before.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The manifest schema version. Bumped only if the on-disk layout changes; an
/// unknown version is treated as corrupt (ignored) so an older daemon never
/// mis-reads a newer state dir.
const MANIFEST_VERSION: u32 = 1;

/// The manifest filename inside the state dir.
const MANIFEST_FILE: &str = "loaded.json";

/// The subdirectory holding persisted wasm bytes (one `<id>.wasm` per
/// `source = "wasm"` record).
const WASM_SUBDIR: &str = "loaded";

/// How a persisted runtime-loaded ploxion was originally loaded — which decides
/// where its wasm bytes come from on restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Loaded by registered id from `<ploxions_dir>/<id>.wasm`. Only the id is
    /// persisted; the bytes reload from the staged dir on restore (no
    /// duplication of the staged wasm).
    Staged,
    /// Loaded from base64-uploaded bytes. The bytes are persisted at
    /// `<state-dir>/loaded/<id>.wasm` because they live nowhere else.
    Wasm,
}

/// One persisted runtime-loaded ploxion: its id + how it was loaded. The wasm
/// bytes (for [`Source::Wasm`]) live in a sibling file, not inline, so the
/// manifest stays small + human-readable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedRecord {
    /// The ploxion's manifest id (its bus identity, and the wasm filename for a
    /// `wasm`-source record).
    pub id: String,
    /// Where its bytes come from on restore.
    pub source: Source,
}

/// The on-disk manifest: a versioned list of runtime-loaded ploxions.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    /// Schema version (see [`MANIFEST_VERSION`]).
    version: u32,
    /// The runtime-loaded delta, in load order.
    loaded: Vec<LoadedRecord>,
}

/// A handle to the daemon's state dir. Cheap to clone (just a path). All methods
/// are best-effort + atomic; the daemon holds ONE of these (on the host thread)
/// when `--state-dir` is set, and `None` when it is not (feature off).
#[derive(Debug, Clone)]
pub struct StateStore {
    dir: PathBuf,
}

impl StateStore {
    /// Open (creating if needed) the state dir at `dir`. Creates `dir` and its
    /// `loaded/` subdir so later atomic writes find them. Returns an error only
    /// if the directories cannot be created (e.g. a permission problem) — the
    /// caller logs that and runs WITHOUT persistence rather than crashing.
    pub fn open(dir: impl AsRef<Path>) -> Result<StateStore> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(dir.join(WASM_SUBDIR))
            .with_context(|| format!("creating state dir {}", dir.join(WASM_SUBDIR).display()))?;
        Ok(StateStore { dir })
    }

    /// The state dir path (for log lines).
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn manifest_path(&self) -> PathBuf {
        self.dir.join(MANIFEST_FILE)
    }

    fn wasm_path(&self, id: &str) -> PathBuf {
        self.dir.join(WASM_SUBDIR).join(format!("{id}.wasm"))
    }

    /// Read the manifest, returning the persisted records. A MISSING manifest is
    /// not an error — it means "nothing persisted yet" (empty list). A
    /// corrupt/unreadable/unknown-version manifest returns an error so the
    /// caller can WARN and continue (the daemon ignores it and still boots the
    /// base set).
    pub fn read_records(&self) -> Result<Vec<LoadedRecord>> {
        let path = self.manifest_path();
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            // Missing file => nothing persisted yet. Not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing state manifest {}", path.display()))?;
        if manifest.version != MANIFEST_VERSION {
            anyhow::bail!(
                "state manifest {} has unknown version {} (expected {MANIFEST_VERSION})",
                path.display(),
                manifest.version
            );
        }
        Ok(manifest.loaded)
    }

    /// Load the wasm bytes persisted for a [`Source::Wasm`] record. Errors if the
    /// file is missing/unreadable/empty — the caller skips that record with a
    /// warning (a truncated or absent blob must never crash restore).
    pub fn read_wasm(&self, id: &str) -> Result<Vec<u8>> {
        let path = self.wasm_path(id);
        let bytes =
            std::fs::read(&path).with_context(|| format!("reading wasm {}", path.display()))?;
        if bytes.is_empty() {
            anyhow::bail!("persisted wasm {} is empty (truncated?)", path.display());
        }
        Ok(bytes)
    }

    /// PERSIST one freshly hot-loaded ploxion (idempotent on `id`): record it in
    /// the manifest and, for a [`Source::Wasm`] record, write its bytes. If a
    /// record with the same id already exists it is REPLACED (a re-load after an
    /// unload). The whole thing is atomic per-file; on any I/O error the caller
    /// logs but the live load still succeeded (persistence is best-effort).
    ///
    /// `wasm` MUST be `Some` for [`Source::Wasm`] and is ignored for
    /// [`Source::Staged`] (staged bytes reload from `<ploxions_dir>`).
    pub fn persist(&self, id: &str, source: Source, wasm: Option<&[u8]>) -> Result<()> {
        // 1. Write the wasm blob FIRST (so the manifest never references a
        //    missing blob even if we crash between the two writes).
        if source == Source::Wasm {
            let bytes = wasm.ok_or_else(|| {
                anyhow::anyhow!("persist: source=wasm for '{id}' but no bytes given")
            })?;
            atomic_write(&self.wasm_path(id), bytes)
                .with_context(|| format!("persisting wasm bytes for '{id}'"))?;
        }
        // 2. Update the manifest (replace-or-append this id).
        let mut records = self.read_records().unwrap_or_default();
        records.retain(|r| r.id != id);
        records.push(LoadedRecord { id: id.to_string(), source });
        self.write_manifest(&records)?;
        Ok(())
    }

    /// REMOVE a ploxion from the persisted set (on `POST /unload`): drop its
    /// manifest record and delete any persisted wasm blob. Idempotent — removing
    /// an id that is not persisted is a no-op success (e.g. unloading a base-set
    /// ploxion, which was never persisted). Best-effort like [`Self::persist`].
    pub fn remove(&self, id: &str) -> Result<()> {
        let mut records = self.read_records().unwrap_or_default();
        let before = records.len();
        records.retain(|r| r.id != id);
        if records.len() != before {
            self.write_manifest(&records)?;
        }
        // Delete any wasm blob (ignore "not found").
        let wasm = self.wasm_path(id);
        if let Err(e) = std::fs::remove_file(&wasm) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e).with_context(|| format!("removing wasm {}", wasm.display()));
            }
        }
        Ok(())
    }

    /// Atomically write the manifest from the current record list.
    fn write_manifest(&self, records: &[LoadedRecord]) -> Result<()> {
        let manifest = Manifest {
            version: MANIFEST_VERSION,
            loaded: records.to_vec(),
        };
        let json = serde_json::to_vec_pretty(&manifest).context("serializing state manifest")?;
        atomic_write(&self.manifest_path(), &json)
            .with_context(|| format!("writing {}", self.manifest_path().display()))
    }
}

/// Write `bytes` to `path` ATOMICALLY: write to `<path>.tmp` in the same
/// directory, flush, then `rename` it into place. A rename within one directory
/// is atomic on POSIX, so a reader sees either the old file or the complete new
/// one — never a half-written file. On any error the temp file is best-effort
/// cleaned up.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)
            .with_context(|| format!("creating temp {}", tmp.display()))?;
        f.write_all(bytes)
            .with_context(|| format!("writing temp {}", tmp.display()))?;
        f.sync_all()
            .with_context(|| format!("syncing temp {}", tmp.display()))?;
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp); // best-effort cleanup
            Err(e).with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "xion-state-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    #[test]
    fn missing_manifest_reads_as_empty() {
        let dir = tmp_dir("missing");
        let store = StateStore::open(&dir).unwrap();
        assert!(store.read_records().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn persist_then_read_roundtrips_staged_and_wasm() {
        let dir = tmp_dir("roundtrip");
        let store = StateStore::open(&dir).unwrap();
        store.persist("alpha", Source::Staged, None).unwrap();
        store
            .persist("beta", Source::Wasm, Some(b"\0asm-fake-bytes"))
            .unwrap();
        let recs = store.read_records().unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].id, "alpha");
        assert_eq!(recs[0].source, Source::Staged);
        assert_eq!(recs[1].id, "beta");
        assert_eq!(recs[1].source, Source::Wasm);
        assert_eq!(store.read_wasm("beta").unwrap(), b"\0asm-fake-bytes");
        // A NEW store on the same dir (simulating a fresh process) reads the same.
        let store2 = StateStore::open(&dir).unwrap();
        assert_eq!(store2.read_records().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn persist_replaces_same_id_no_duplicate() {
        let dir = tmp_dir("replace");
        let store = StateStore::open(&dir).unwrap();
        store.persist("x", Source::Staged, None).unwrap();
        store.persist("x", Source::Staged, None).unwrap();
        assert_eq!(store.read_records().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_drops_record_and_wasm() {
        let dir = tmp_dir("remove");
        let store = StateStore::open(&dir).unwrap();
        store.persist("gone", Source::Wasm, Some(b"bytes")).unwrap();
        assert!(store.wasm_path("gone").exists());
        store.remove("gone").unwrap();
        assert!(store.read_records().unwrap().is_empty());
        assert!(!store.wasm_path("gone").exists());
        // Idempotent: removing again is fine.
        store.remove("gone").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_manifest_is_an_error_not_a_panic() {
        let dir = tmp_dir("corrupt");
        let store = StateStore::open(&dir).unwrap();
        std::fs::write(store.manifest_path(), b"{ this is not json").unwrap();
        assert!(store.read_records().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_version_is_rejected() {
        let dir = tmp_dir("version");
        let store = StateStore::open(&dir).unwrap();
        std::fs::write(
            store.manifest_path(),
            br#"{"version":999,"loaded":[]}"#,
        )
        .unwrap();
        assert!(store.read_records().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_wasm_blob_is_rejected() {
        let dir = tmp_dir("emptywasm");
        let store = StateStore::open(&dir).unwrap();
        std::fs::write(store.wasm_path("e"), b"").unwrap();
        assert!(store.read_wasm("e").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
