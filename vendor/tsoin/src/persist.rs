//! On-disk persistence for the `tsoin` CLI — the `.tsoin/` repository format.
//!
//! The library's [`Recorder`] is in-memory; this module makes it survive across
//! separate process invocations (mandatory for a "git of states" you drive from
//! the shell). It owns a tiny, dependency-free JSON document plus a directory of
//! content-addressed residue blobs, and can rebuild a fully-populated `Recorder`
//! from them in a fresh process.
//!
//! ## Layout (`docs/CLI-v0.md` is the canonical spec)
//!
//! ```text
//! .tsoin/
//!   HEAD          current branch name
//!   meta.json     DAG nodes + branches + per-state metadata + xerboxions + blob lens
//!   blobs/<hash>  one file per distinct delta: model::encode(raw_xor_delta),
//!                 named by BLAKE3(raw_xor_delta) (== the node's residue_hash)
//! ```
//!
//! Residue blobs are stored **model-coded** (the crate's [`crate::model`] codec)
//! so the on-disk footprint is the real, collapsed surprise rather than the
//! full-length raw delta. On load each blob is decoded back to the exact raw
//! delta and replayed into a `Recorder`, so [`Recorder::replay`] stays bit-exact.

use crate::coords::Coords;
use crate::hash::Hash;
use crate::model;
use crate::timeline::{Recorder, Xerboxion};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// The repository directory name placed in the working directory.
pub const REPO_DIR: &str = ".tsoin";

/// What kind of thing a state captured — used so `replay` knows whether to emit
/// a single file's bytes or restore a directory tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::File => "file",
            Kind::Dir => "dir",
        }
    }
    fn parse(s: &str) -> Option<Kind> {
        match s {
            "file" => Some(Kind::File),
            "dir" => Some(Kind::Dir),
            _ => None,
        }
    }
}

/// CLI-level metadata for one recorded state (the engine's Merkle id stays the
/// source of truth for identity; this is the human/bookkeeping layer).
#[derive(Debug, Clone)]
pub struct StateMeta {
    /// The engine node id (`TimelineNode::node_hash`).
    pub id: Hash,
    pub parent: Option<Hash>,
    pub residue: Hash,
    pub len: u64,
    pub coords_t: u64,
    pub kind: Kind,
    /// The recorded path's display name (file name or dir name).
    pub name: String,
    pub msg: String,
    /// Unix seconds at record time.
    pub time: u64,
    pub raw_size: u64,
}

/// A named branch: a head pointer into the shared DAG (or `None` if empty).
#[derive(Debug, Clone)]
pub struct BranchRef {
    pub name: String,
    pub head: Option<Hash>,
}

/// The whole persisted repository, loaded into memory and ready to use. Mutating
/// methods update the in-memory `Recorder` and metadata; [`Repo::save`] writes it
/// all back atomically enough for a single-user CLI.
pub struct Repo {
    root: PathBuf,
    /// The reconstructed engine recorder (DAG + blob store + xerboxion index).
    pub rec: Recorder,
    /// Current branch name (mirrors `HEAD`).
    pub head: String,
    pub branches: Vec<BranchRef>,
    /// State metadata keyed by node id, plus insertion order for stable listing.
    nodes: HashMap<Hash, StateMeta>,
    order: Vec<Hash>,
    /// raw length of each residue blob (needed to model::decode it back).
    blob_lens: HashMap<Hash, usize>,
}

impl Repo {
    /// Path to the `.tsoin` dir for a working directory.
    #[must_use]
    pub fn dir_for(cwd: &Path) -> PathBuf {
        cwd.join(REPO_DIR)
    }

    /// `true` if `cwd` already contains an initialized repo.
    #[must_use]
    pub fn exists(cwd: &Path) -> bool {
        Self::dir_for(cwd).join("meta.json").is_file()
    }

    /// Initialize a fresh repo at `cwd`, with a single empty branch `main`.
    /// Errors if one already exists unless `force` is set (which wipes it).
    pub fn init(cwd: &Path, force: bool) -> io::Result<Repo> {
        let dir = Self::dir_for(cwd);
        if dir.exists() {
            if force {
                std::fs::remove_dir_all(&dir)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} already initialized (use --force to reset)", dir.display()),
                ));
            }
        }
        std::fs::create_dir_all(dir.join("blobs"))?;
        let repo = Repo {
            root: dir,
            rec: Recorder::new(),
            head: "main".to_string(),
            branches: vec![BranchRef {
                name: "main".to_string(),
                head: None,
            }],
            nodes: HashMap::new(),
            order: Vec::new(),
            blob_lens: HashMap::new(),
        };
        repo.save()?;
        Ok(repo)
    }

    /// Walk up from `start` to find the nearest ancestor directory holding a
    /// `.tsoin/`, returning that working directory (git-like discovery).
    #[must_use]
    pub fn discover(start: &Path) -> Option<PathBuf> {
        let mut cur = Some(start);
        while let Some(d) = cur {
            if Self::exists(d) {
                return Some(d.to_path_buf());
            }
            cur = d.parent();
        }
        None
    }

    /// Load an existing repo from `cwd`, rebuilding the `Recorder` from disk.
    pub fn open(cwd: &Path) -> io::Result<Repo> {
        let dir = Self::dir_for(cwd);
        let meta_path = dir.join("meta.json");
        let text = std::fs::read_to_string(&meta_path)?;
        let doc = json::parse(&text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("meta.json: {e}")))?;

        let head = doc.get_str("head").unwrap_or_else(|| "main".to_string());

        // blob_lens first (needed to decode residues).
        let mut blob_lens = HashMap::new();
        for b in doc.get_array("blobs") {
            if let (Some(h), Some(l)) = (b.get_hash("hash"), b.get_u64("raw_len")) {
                blob_lens.insert(h, l as usize);
            }
        }

        // Parse nodes; sort by coords_t then by appearance so parents come first.
        let mut raw_nodes: Vec<StateMeta> = Vec::new();
        for n in doc.get_array("nodes") {
            let id = n.get_hash("id").ok_or_else(bad("node id"))?;
            let parent = n.get_hash("parent");
            let residue = n.get_hash("residue").ok_or_else(bad("node residue"))?;
            let len = n.get_u64("len").unwrap_or(0);
            let coords_t = n.get_u64("coords_t").unwrap_or(0);
            let kind = n
                .get_str("kind")
                .and_then(|s| Kind::parse(&s))
                .unwrap_or(Kind::File);
            let name = n.get_str("name").unwrap_or_default();
            let msg = n.get_str("msg").unwrap_or_default();
            let time = n.get_u64("time").unwrap_or(0);
            let raw_size = n.get_u64("raw_size").unwrap_or(len);
            raw_nodes.push(StateMeta {
                id,
                parent,
                residue,
                len,
                coords_t,
                kind,
                name,
                msg,
                time,
                raw_size,
            });
        }

        // Rebuild the Recorder by re-recording each frame in causal order. We
        // can't `put` straight into the private store, so we reconstruct each
        // frame's bytes from disk blobs and feed Recorder::record, which yields
        // the SAME node ids (deterministic from parent+residue+len+coords) and
        // repopulates the store, DAG, and xerboxion index identically.
        let mut rec = Recorder::new();
        // Topologically order: parents (or None) before children. coords_t is
        // monotonically assigned at record time, so sorting by it suffices, with
        // a stable tiebreak.
        raw_nodes.sort_by(|a, b| a.coords_t.cmp(&b.coords_t).then(a.id.cmp(&b.id)));

        // Map disk node id -> rebuilt node id (they must be equal, but we verify).
        let mut nodes: HashMap<Hash, StateMeta> = HashMap::new();
        let mut order: Vec<Hash> = Vec::new();
        for meta in &raw_nodes {
            // Reconstruct this frame's raw bytes: residue blob (model-decoded)
            // XOR the parent's reconstructed frame, resized.
            let raw_delta = read_blob(&dir, &meta.residue, &blob_lens)?;
            let parent_frame = match meta.parent {
                Some(p) => rec.replay(&p),
                None => Vec::new(),
            };
            let frame = apply_delta(&raw_delta, &parent_frame, meta.len as usize);
            let coords = Coords {
                t: meta.coords_t,
                ..Coords::default()
            };
            let (got, _x) = rec.record(meta.parent, &frame, coords);
            if got != meta.id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "node id mismatch on load: disk {} rebuilt {}",
                        meta.id.short(),
                        got.short()
                    ),
                ));
            }
            nodes.insert(meta.id, meta.clone());
            order.push(meta.id);
        }

        // Branches.
        let mut branches = Vec::new();
        for b in doc.get_array("branches") {
            let name = b.get_str("name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            branches.push(BranchRef {
                name,
                head: b.get_hash("head"),
            });
        }
        if branches.is_empty() {
            branches.push(BranchRef {
                name: "main".to_string(),
                head: None,
            });
        }

        Ok(Repo {
            root: dir,
            rec,
            head,
            branches,
            nodes,
            order,
            blob_lens,
        })
    }

    /// The current branch's record, by name match against `head`.
    fn current_branch_idx(&self) -> Option<usize> {
        self.branches.iter().position(|b| b.name == self.head)
    }

    /// Head node of the current branch, if any.
    #[must_use]
    pub fn current_head(&self) -> Option<Hash> {
        self.current_branch_idx().and_then(|i| self.branches[i].head)
    }

    /// All branch names.
    #[must_use]
    pub fn branch_names(&self) -> Vec<String> {
        self.branches.iter().map(|b| b.name.clone()).collect()
    }

    /// Look up a branch head by name.
    #[must_use]
    pub fn branch_head(&self, name: &str) -> Option<Option<Hash>> {
        self.branches
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.head)
    }

    /// Metadata for a node.
    #[must_use]
    pub fn meta(&self, id: &Hash) -> Option<&StateMeta> {
        self.nodes.get(id)
    }

    /// The states on the current branch, newest-first (head back to root).
    #[must_use]
    pub fn current_chain(&self) -> Vec<StateMeta> {
        let mut out = Vec::new();
        let mut cur = self.current_head();
        while let Some(h) = cur {
            if let Some(m) = self.nodes.get(&h) {
                out.push(m.clone());
                cur = m.parent;
            } else {
                break;
            }
        }
        out
    }

    /// Resolve a (possibly short) hex id to a full node id. Errors on no match
    /// or an ambiguous prefix.
    pub fn resolve(&self, short: &str) -> Result<Hash, String> {
        let needle = short.trim().to_lowercase();
        if needle.is_empty() {
            return Err("empty state id".to_string());
        }
        let matches: Vec<Hash> = self
            .order
            .iter()
            .filter(|h| h.to_hex().starts_with(&needle))
            .copied()
            .collect();
        match matches.len() {
            0 => Err(format!("no state matches id '{short}'")),
            1 => Ok(matches[0]),
            n => Err(format!("ambiguous id '{short}' matches {n} states")),
        }
    }

    /// The next `coords_t` to assign — strictly greater than any existing, so a
    /// re-recorded identical frame after different history still gets a distinct
    /// node yet the same content hash (the xerboxion key).
    fn next_t(&self) -> u64 {
        self.nodes.values().map(|m| m.coords_t).max().map_or(0, |m| m + 1)
    }

    /// Record `frame` as a new state on the current branch. Stores only the new
    /// residue blob (model-coded) to disk. Returns the new state's id, the bytes
    /// actually written to disk for this delta (0 if deduplicated), and the
    /// detected xerboxion if the frame exactly repeats an earlier instant.
    pub fn record(
        &mut self,
        frame: &[u8],
        kind: Kind,
        name: &str,
        msg: &str,
    ) -> io::Result<(Hash, usize, Option<Xerboxion>)> {
        let parent = self.current_head();
        let t = self.next_t();
        let coords = Coords {
            t,
            ..Coords::default()
        };
        let (id, xerb) = self.rec.record(parent, frame, coords);
        let node = self
            .rec
            .node(&id)
            .expect("just-recorded node must be present");

        // Persist the residue blob (model-coded) if new on disk.
        let raw_delta = self
            .rec
            .store()
            .get(&node.residue_hash)
            .expect("residue present after record")
            .to_vec();
        let written = self.write_blob(&node.residue_hash, &raw_delta)?;
        self.blob_lens.insert(node.residue_hash, raw_delta.len());

        let now = now_secs();
        let meta = StateMeta {
            id,
            parent,
            residue: node.residue_hash,
            len: node.len,
            coords_t: t,
            kind,
            name: name.to_string(),
            msg: msg.to_string(),
            time: now,
            raw_size: frame.len() as u64,
        };
        if !self.nodes.contains_key(&id) {
            self.order.push(id);
        }
        self.nodes.insert(id, meta);

        // Advance the current branch head.
        if let Some(i) = self.current_branch_idx() {
            self.branches[i].head = Some(id);
        }

        self.save()?;
        Ok((id, written, xerb))
    }

    /// Create a new branch `name` at `at` (or the current head). Free: copies no
    /// blobs. Errors if the name already exists.
    pub fn fork(&mut self, name: &str, at: Option<Hash>) -> io::Result<()> {
        if self.branches.iter().any(|b| b.name == name) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("branch '{name}' already exists"),
            ));
        }
        let head = at.or_else(|| self.current_head());
        self.branches.push(BranchRef {
            name: name.to_string(),
            head,
        });
        self.save()
    }

    /// Switch the current branch.
    pub fn checkout(&mut self, name: &str) -> io::Result<()> {
        if !self.branches.iter().any(|b| b.name == name) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no such branch '{name}'"),
            ));
        }
        self.head = name.to_string();
        self.save()
    }

    /// Actual on-disk store footprint: the sum of the sizes of every distinct
    /// residue blob file (model-coded), i.e. what the repo really costs.
    pub fn store_footprint_disk(&self) -> io::Result<u64> {
        let mut total = 0u64;
        let blobs = self.root.join("blobs");
        if blobs.is_dir() {
            for entry in std::fs::read_dir(&blobs)? {
                let entry = entry?;
                total += entry.metadata()?.len();
            }
        }
        Ok(total)
    }

    /// Naive cost: the sum of every recorded state's raw size (storing each
    /// instant in full).
    #[must_use]
    pub fn naive_cost(&self) -> u64 {
        self.nodes.values().map(|m| m.raw_size).sum()
    }

    /// Number of distinct states (DAG nodes).
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of branches.
    #[must_use]
    pub fn branch_count(&self) -> usize {
        self.branches.len()
    }

    /// All detected xerboxions (from the rebuilt recorder).
    #[must_use]
    pub fn xerboxions(&self) -> &[Xerboxion] {
        self.rec.xerboxions()
    }

    // -- blob IO --------------------------------------------------------------

    /// Write a residue blob model-coded if absent; return bytes written (0 if it
    /// already existed on disk — deduplicated).
    fn write_blob(&self, hash: &Hash, raw_delta: &[u8]) -> io::Result<usize> {
        let path = self.root.join("blobs").join(hash.to_hex());
        if path.exists() {
            return Ok(0);
        }
        let coded = model::encode(raw_delta);
        let n = coded.len();
        std::fs::write(&path, &coded)?;
        Ok(n)
    }

    // -- saving ---------------------------------------------------------------

    /// Serialize the whole repo to `.tsoin/meta.json` and `HEAD`.
    pub fn save(&self) -> io::Result<()> {
        std::fs::create_dir_all(self.root.join("blobs"))?;
        std::fs::write(self.root.join("HEAD"), format!("{}\n", self.head))?;

        let mut o = json::Out::new();
        o.begin_obj();
        o.field_u64("version", 1);
        o.field_str("head", &self.head);

        o.key("branches");
        o.begin_arr();
        for b in &self.branches {
            o.begin_obj();
            o.field_str("name", &b.name);
            match b.head {
                Some(h) => o.field_str("head", &h.to_hex()),
                None => o.field_null("head"),
            }
            o.end_obj();
        }
        o.end_arr();

        o.key("nodes");
        o.begin_arr();
        // Stable order for reproducible files.
        let mut metas: Vec<&StateMeta> = self.nodes.values().collect();
        metas.sort_by(|a, b| a.coords_t.cmp(&b.coords_t).then(a.id.cmp(&b.id)));
        for m in metas {
            o.begin_obj();
            o.field_str("id", &m.id.to_hex());
            match m.parent {
                Some(p) => o.field_str("parent", &p.to_hex()),
                None => o.field_null("parent"),
            }
            o.field_str("residue", &m.residue.to_hex());
            o.field_u64("len", m.len);
            o.field_u64("coords_t", m.coords_t);
            o.field_str("kind", m.kind.as_str());
            o.field_str("name", &m.name);
            o.field_str("msg", &m.msg);
            o.field_u64("time", m.time);
            o.field_u64("raw_size", m.raw_size);
            o.end_obj();
        }
        o.end_arr();

        o.key("blobs");
        o.begin_arr();
        let mut blobs: Vec<(&Hash, &usize)> = self.blob_lens.iter().collect();
        blobs.sort_by(|a, b| a.0.cmp(b.0));
        for (h, l) in blobs {
            o.begin_obj();
            o.field_str("hash", &h.to_hex());
            o.field_u64("raw_len", *l as u64);
            o.end_obj();
        }
        o.end_arr();

        o.key("xerboxions");
        o.begin_arr();
        for x in self.rec.xerboxions() {
            o.begin_obj();
            o.field_str("content", &x.content_hash.to_hex());
            o.field_str("first", &x.first.to_hex());
            o.field_str("repeat", &x.repeat.to_hex());
            o.end_obj();
        }
        o.end_arr();

        o.end_obj();

        let tmp = self.root.join("meta.json.tmp");
        std::fs::write(&tmp, o.finish())?;
        std::fs::rename(&tmp, self.root.join("meta.json"))?;
        Ok(())
    }
}

/// Magic header of the deterministic directory serialization.
const DIR_MAGIC: &[u8] = b"TSOINDIR\0";

/// Deterministically serialize a directory tree into a single byte stream so a
/// recorded state replays the exact tree. Format (see `docs/CLI-v0.md`):
///
/// ```text
/// "TSOINDIR\0"
/// for each regular file, sorted by relative path (byte order):
///   <rel-path-bytes> '\n' <len-decimal> '\n' <raw file bytes>
/// ```
///
/// Only regular files are captured. Paths are stored relative to `root` with
/// `/` separators so the stream is platform-stable.
pub fn serialize_dir(root: &Path) -> io::Result<Vec<u8>> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

    let mut out = Vec::new();
    out.extend_from_slice(DIR_MAGIC);
    for (rel, path) in files {
        let bytes = std::fs::read(&path)?;
        out.extend_from_slice(rel.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(bytes.len().to_string().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

/// `true` if `bytes` is a directory serialization (starts with the magic).
#[must_use]
pub fn is_dir_stream(bytes: &[u8]) -> bool {
    bytes.starts_with(DIR_MAGIC)
}

/// Restore a directory serialization produced by [`serialize_dir`] under `dest`,
/// recreating every file with its exact bytes. Bit-exact inverse of
/// [`serialize_dir`].
pub fn restore_dir(stream: &[u8], dest: &Path) -> io::Result<()> {
    if !is_dir_stream(stream) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a tsoin directory stream",
        ));
    }
    std::fs::create_dir_all(dest)?;
    let mut i = DIR_MAGIC.len();
    while i < stream.len() {
        // rel-path up to '\n'
        let nl = find_byte(stream, i, b'\n').ok_or_else(corrupt)?;
        let rel = std::str::from_utf8(&stream[i..nl]).map_err(|_| corrupt())?;
        i = nl + 1;
        // length up to '\n'
        let nl2 = find_byte(stream, i, b'\n').ok_or_else(corrupt)?;
        let len: usize = std::str::from_utf8(&stream[i..nl2])
            .map_err(|_| corrupt())?
            .parse()
            .map_err(|_| corrupt())?;
        i = nl2 + 1;
        if i + len > stream.len() {
            return Err(corrupt());
        }
        let bytes = &stream[i..i + len];
        i += len;

        let path = dest.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes)?;
    }
    Ok(())
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            // Skip a nested .tsoin repo to avoid recording the store into itself.
            if path.file_name().and_then(|n| n.to_str()) == Some(REPO_DIR) {
                continue;
            }
            collect_files(root, &path, out)?;
        } else if ft.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, path));
        }
        // symlinks and other types are skipped
    }
    Ok(())
}

fn find_byte(b: &[u8], from: usize, needle: u8) -> Option<usize> {
    b[from..].iter().position(|&x| x == needle).map(|p| p + from)
}

fn corrupt() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "corrupt tsoin directory stream")
}

/// XOR a raw delta against the (resized) parent frame to recover the frame.
fn apply_delta(raw_delta: &[u8], parent_frame: &[u8], len: usize) -> Vec<u8> {
    let mut pred = vec![0u8; len];
    let take = parent_frame.len().min(len);
    pred[..take].copy_from_slice(&parent_frame[..take]);
    raw_delta
        .iter()
        .zip(pred.iter())
        .map(|(a, b)| a ^ b)
        .collect()
}

/// Read a residue blob from disk and model-decode it back to the raw delta.
fn read_blob(dir: &Path, hash: &Hash, lens: &HashMap<Hash, usize>) -> io::Result<Vec<u8>> {
    let path = dir.join("blobs").join(hash.to_hex());
    let coded = std::fs::read(&path)?;
    let raw_len = *lens.get(hash).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("no raw_len for blob {}", hash.short()),
        )
    })?;
    Ok(model::decode(&coded, raw_len))
}

fn bad(what: &'static str) -> impl Fn() -> io::Error {
    move || io::Error::new(io::ErrorKind::InvalidData, format!("meta.json: missing {what}"))
}

/// Current unix time in seconds (0 if the clock is before the epoch).
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A minimal, dependency-free JSON reader/writer scoped to exactly the document
/// shape `meta.json` uses (objects, arrays, strings, integers, null). Not a
/// general JSON library — just enough to be robust for our own files.
mod json {
    use crate::hash::Hash;
    use std::collections::HashMap;

    /// A parsed JSON value (the subset we emit).
    #[derive(Debug, Clone)]
    pub enum Value {
        Null,
        Str(String),
        Num(i64),
        Arr(Vec<Value>),
        Obj(HashMap<String, Value>),
    }

    impl Value {
        /// Object field as a string.
        pub fn get_str(&self, key: &str) -> Option<String> {
            match self.field(key)? {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            }
        }
        /// Object field as a u64 (from a JSON integer).
        pub fn get_u64(&self, key: &str) -> Option<u64> {
            match self.field(key)? {
                Value::Num(n) if *n >= 0 => Some(*n as u64),
                _ => None,
            }
        }
        /// Object field as a 32-byte hash parsed from 64 hex chars; `null` -> None.
        pub fn get_hash(&self, key: &str) -> Option<Hash> {
            match self.field(key)? {
                Value::Str(s) => parse_hash(s),
                _ => None,
            }
        }
        /// Object field as an array (empty if absent / not an array).
        pub fn get_array(&self, key: &str) -> Vec<Value> {
            match self.field(key) {
                Some(Value::Arr(a)) => a.clone(),
                _ => Vec::new(),
            }
        }
        fn field(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Obj(m) => m.get(key),
                _ => None,
            }
        }
    }

    fn parse_hash(s: &str) -> Option<Hash> {
        if s.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Hash::from_bytes(bytes))
    }

    /// Parse a JSON document of our restricted shape.
    pub fn parse(text: &str) -> Result<Value, String> {
        let mut p = Parser {
            b: text.as_bytes(),
            i: 0,
        };
        p.ws();
        let v = p.value()?;
        p.ws();
        Ok(v)
    }

    struct Parser<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl Parser<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() {
                self.i += 1;
            }
        }
        fn peek(&self) -> Option<u8> {
            self.b.get(self.i).copied()
        }
        fn value(&mut self) -> Result<Value, String> {
            self.ws();
            match self.peek() {
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => Ok(Value::Str(self.string()?)),
                Some(b'n') => {
                    self.expect_lit("null")?;
                    Ok(Value::Null)
                }
                Some(b't') => {
                    self.expect_lit("true")?;
                    Ok(Value::Num(1))
                }
                Some(b'f') => {
                    self.expect_lit("false")?;
                    Ok(Value::Num(0))
                }
                Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
                other => Err(format!("unexpected byte {other:?} at {}", self.i)),
            }
        }
        fn expect_lit(&mut self, lit: &str) -> Result<(), String> {
            if self.b[self.i..].starts_with(lit.as_bytes()) {
                self.i += lit.len();
                Ok(())
            } else {
                Err(format!("expected '{lit}' at {}", self.i))
            }
        }
        fn object(&mut self) -> Result<Value, String> {
            self.i += 1; // {
            let mut m = HashMap::new();
            self.ws();
            if self.peek() == Some(b'}') {
                self.i += 1;
                return Ok(Value::Obj(m));
            }
            loop {
                self.ws();
                let k = self.string()?;
                self.ws();
                if self.peek() != Some(b':') {
                    return Err(format!("expected ':' at {}", self.i));
                }
                self.i += 1;
                let v = self.value()?;
                m.insert(k, v);
                self.ws();
                match self.peek() {
                    Some(b',') => {
                        self.i += 1;
                    }
                    Some(b'}') => {
                        self.i += 1;
                        break;
                    }
                    other => return Err(format!("expected ',' or '}}' got {other:?}")),
                }
            }
            Ok(Value::Obj(m))
        }
        fn array(&mut self) -> Result<Value, String> {
            self.i += 1; // [
            let mut a = Vec::new();
            self.ws();
            if self.peek() == Some(b']') {
                self.i += 1;
                return Ok(Value::Arr(a));
            }
            loop {
                let v = self.value()?;
                a.push(v);
                self.ws();
                match self.peek() {
                    Some(b',') => {
                        self.i += 1;
                    }
                    Some(b']') => {
                        self.i += 1;
                        break;
                    }
                    other => return Err(format!("expected ',' or ']' got {other:?}")),
                }
            }
            Ok(Value::Arr(a))
        }
        fn string(&mut self) -> Result<String, String> {
            if self.peek() != Some(b'"') {
                return Err(format!("expected string at {}", self.i));
            }
            self.i += 1;
            let mut s = String::new();
            while let Some(c) = self.peek() {
                self.i += 1;
                match c {
                    b'"' => return Ok(s),
                    b'\\' => {
                        let e = self.peek().ok_or("bad escape")?;
                        self.i += 1;
                        match e {
                            b'"' => s.push('"'),
                            b'\\' => s.push('\\'),
                            b'/' => s.push('/'),
                            b'n' => s.push('\n'),
                            b't' => s.push('\t'),
                            b'r' => s.push('\r'),
                            b'b' => s.push('\u{8}'),
                            b'f' => s.push('\u{c}'),
                            b'u' => {
                                let hex = std::str::from_utf8(&self.b[self.i..self.i + 4])
                                    .map_err(|_| "bad \\u")?;
                                let cp = u32::from_str_radix(hex, 16).map_err(|_| "bad \\u")?;
                                self.i += 4;
                                s.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                            }
                            _ => return Err("bad escape".to_string()),
                        }
                    }
                    // Multi-byte UTF-8: collect raw bytes until the next quote;
                    // simpler to push the byte and rely on String being UTF-8 by
                    // re-decoding. Here we handle ASCII fast-path and pass through
                    // continuation bytes verbatim.
                    _ => {
                        if c < 0x80 {
                            s.push(c as char);
                        } else {
                            // Gather a full UTF-8 sequence.
                            let start = self.i - 1;
                            let extra = match c {
                                0xC0..=0xDF => 1,
                                0xE0..=0xEF => 2,
                                0xF0..=0xF7 => 3,
                                _ => 0,
                            };
                            self.i += extra;
                            let seq = &self.b[start..self.i];
                            s.push_str(std::str::from_utf8(seq).map_err(|_| "bad utf8")?);
                        }
                    }
                }
            }
            Err("unterminated string".to_string())
        }
        fn number(&mut self) -> Result<Value, String> {
            let start = self.i;
            if self.peek() == Some(b'-') {
                self.i += 1;
            }
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.i += 1;
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "bad number")?;
            s.parse::<i64>().map(Value::Num).map_err(|e| e.to_string())
        }
    }

    /// A tiny JSON writer with field helpers, emitting compact, valid JSON.
    ///
    /// `need_comma` is a stack of "has the current container emitted an item
    /// yet?" flags. `suppress` is set by [`Out::key`] so the `begin_obj` /
    /// `begin_arr` that supplies the key's value does NOT prepend a comma (the
    /// key already counted as this container's item).
    pub struct Out {
        buf: String,
        need_comma: Vec<bool>,
        suppress: bool,
    }

    impl Out {
        pub fn new() -> Self {
            Out {
                buf: String::new(),
                need_comma: Vec::new(),
                suppress: false,
            }
        }
        /// Separator before the next item of the current container. After a
        /// bare key the comma is suppressed (the value is part of that field).
        fn comma(&mut self) {
            if self.suppress {
                self.suppress = false;
                return;
            }
            if let Some(last) = self.need_comma.last_mut() {
                if *last {
                    self.buf.push(',');
                }
                *last = true;
            }
        }
        pub fn begin_obj(&mut self) {
            self.comma();
            self.buf.push('{');
            self.need_comma.push(false);
        }
        pub fn end_obj(&mut self) {
            self.buf.push('}');
            self.need_comma.pop();
        }
        pub fn begin_arr(&mut self) {
            self.comma();
            self.buf.push('[');
            self.need_comma.push(false);
        }
        pub fn end_arr(&mut self) {
            self.buf.push(']');
            self.need_comma.pop();
        }
        /// Emit a bare `"key":` whose value is supplied by the next `begin_*`.
        pub fn key(&mut self, k: &str) {
            self.comma();
            write_json_str(&mut self.buf, k);
            self.buf.push(':');
            self.suppress = true; // the following begin_* must not add a comma
        }
        pub fn field_str(&mut self, k: &str, v: &str) {
            self.comma();
            write_json_str(&mut self.buf, k);
            self.buf.push(':');
            write_json_str(&mut self.buf, v);
        }
        pub fn field_u64(&mut self, k: &str, v: u64) {
            self.comma();
            write_json_str(&mut self.buf, k);
            self.buf.push(':');
            self.buf.push_str(&v.to_string());
        }
        pub fn field_null(&mut self, k: &str) {
            self.comma();
            write_json_str(&mut self.buf, k);
            self.buf.push_str(":null");
        }
        pub fn finish(self) -> String {
            self.buf
        }
    }

    fn write_json_str(buf: &mut String, s: &str) {
        buf.push('"');
        for c in s.chars() {
            match c {
                '"' => buf.push_str("\\\""),
                '\\' => buf.push_str("\\\\"),
                '\n' => buf.push_str("\\n"),
                '\r' => buf.push_str("\\r"),
                '\t' => buf.push_str("\\t"),
                c if (c as u32) < 0x20 => buf.push_str(&format!("\\u{:04x}", c as u32)),
                c => buf.push(c),
            }
        }
        buf.push('"');
    }
}
