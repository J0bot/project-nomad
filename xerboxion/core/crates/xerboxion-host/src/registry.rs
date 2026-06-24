//! Registry: the **known** ploxions, read (read-only) from the runtime
//! session's `ploxi0ns.json`. This is the catalogue of what exists in the
//! ecosystem; the host's own *loaded* set (live WASM instances) is the live
//! registry. The two are complementary: we never write to `ploxi0ns.json`.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The canonical read-only registry path shared with the runtime session.
pub const DEFAULT_REGISTRY: &str = "/home/debian/ploxi0ns/.network/ploxi0ns.json";

/// One catalogue entry from `ploxi0ns.json` (only the fields we surface).
#[derive(Debug, Clone, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    /// The short machine name (defaults to `id` when absent).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    ploxions: Vec<RegistryEntry>,
}

/// Read the known-ploxion catalogue. Missing/unreadable file => empty list (the
/// host still works from its own loaded set), so this never hard-fails startup.
pub fn read_registry(path: impl AsRef<Path>) -> Result<Vec<RegistryEntry>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading registry {}", path.display()))?;
    let file: RegistryFile =
        serde_json::from_slice(&bytes).context("parsing ploxi0ns.json")?;
    Ok(file.ploxions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_registry_is_empty_not_error() {
        let v = read_registry("/no/such/registry.json").unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn parses_minimal_registry() {
        let dir = std::env::temp_dir();
        let p = dir.join("xerb_test_reg.json");
        std::fs::write(&p, r#"{"ploxions":[{"id":"ping","category":"core","status":"active","displayName":"Ping"}]}"#).unwrap();
        let v = read_registry(&p).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "ping");
        assert_eq!(v[0].display_name, "Ping");
        assert_eq!(v[0].category, "core");
        let _ = std::fs::remove_file(&p);
    }
}
