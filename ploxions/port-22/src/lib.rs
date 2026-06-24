//! `port-22` — le port **22** (ssh), en ploxion (port-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::port::PortDef;

const DEF: PortDef = PortDef { number: 22, name: "ssh", protos: "tcp", brief: "shell distant chiffre" };

ploxion_sdk::port_ploxion!(DEF);
