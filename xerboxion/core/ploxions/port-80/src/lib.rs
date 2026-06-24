//! `port-80` — le port **80** (http), en ploxion (port-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::port::PortDef;

const DEF: PortDef = PortDef { number: 80, name: "http", protos: "tcp", brief: "web en clair" };

ploxion_sdk::port_ploxion!(DEF);
