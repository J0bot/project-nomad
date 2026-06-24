//! `pkg-curl` — le paquet **curl**, en ploxion (package-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::package::PackageDef;

const DEF: PackageDef = PackageDef { name: "curl", version: "8.5.0", arch: "amd64", brief: "client HTTP en ligne de commande" };

ploxion_sdk::package_ploxion!(DEF);
