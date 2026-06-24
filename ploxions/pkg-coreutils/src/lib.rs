//! `pkg-coreutils` — le paquet **coreutils**, en ploxion (package-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::package::PackageDef;

const DEF: PackageDef = PackageDef { name: "coreutils", version: "9.1", arch: "amd64", brief: "les utilitaires de base GNU" };

ploxion_sdk::package_ploxion!(DEF);
