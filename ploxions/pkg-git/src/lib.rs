//! `pkg-git` — le paquet **git**, en ploxion (package-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::package::PackageDef;

const DEF: PackageDef = PackageDef { name: "git", version: "2.43.0", arch: "amd64", brief: "controle de version" };

ploxion_sdk::package_ploxion!(DEF);
