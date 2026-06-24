//! `pkg-bash` — le paquet **bash**, en ploxion (package-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::package::PackageDef;

const DEF: PackageDef = PackageDef { name: "bash", version: "5.2.15", arch: "amd64", brief: "le shell GNU" };

ploxion_sdk::package_ploxion!(DEF);
