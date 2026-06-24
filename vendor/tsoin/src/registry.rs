//! Generator registry — resolves a `generator_hash` back to a runnable generator.
//!
//! An [`crate::Address`] stores only the *hash* of the generator. To `decode`,
//! we must reconstruct the generator from that hash. The registry maps
//! `generator_hash -> Box<dyn Generator>` by registering each known generator
//! under `Hash::of(id)`. In a fully content-addressed world the generator's WASM
//! bytes would themselves live in the store and be fetched by hash (synthesis
//! §4); v0 keeps a local registry of the built-in Rust generators.

use crate::generator::{default_generators, Generator};
use crate::hash::Hash;
use std::collections::HashMap;

/// Resolves generator hashes to generator instances.
#[derive(Default)]
pub struct Registry {
    gens: HashMap<Hash, Box<dyn Generator>>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Registry {
            gens: HashMap::new(),
        }
    }

    /// A registry pre-loaded with the v0 default generators.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut r = Registry::new();
        for g in default_generators() {
            r.register(g);
        }
        r
    }

    /// Register a generator under its own `generator_hash`.
    pub fn register(&mut self, g: Box<dyn Generator>) {
        self.gens.insert(g.generator_hash(), g);
    }

    /// Resolve a generator by its hash.
    #[must_use]
    pub fn get(&self, h: &Hash) -> Option<&dyn Generator> {
        self.gens.get(h).map(Box::as_ref)
    }

    /// Number of registered generators.
    #[must_use]
    pub fn len(&self) -> usize {
        self.gens.len()
    }

    /// Whether the registry has no generators.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.gens.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::Constant;

    #[test]
    fn resolves_registered_generator() {
        let r = Registry::with_defaults();
        let g = Constant(0x00);
        let resolved = r.get(&g.generator_hash()).expect("present");
        assert_eq!(resolved.id(), "constant:0x00");
    }

    #[test]
    fn unknown_hash_is_none() {
        let r = Registry::new();
        assert!(r.get(&Hash::of(b"nope")).is_none());
        assert!(r.is_empty());
    }
}
