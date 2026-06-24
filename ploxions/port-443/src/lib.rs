//! `port-443` — le port **443** (https), en ploxion (port-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::port::PortDef;

const DEF: PortDef = PortDef { number: 443, name: "https", protos: "tcp", brief: "web chiffre (TLS)" };

ploxion_sdk::port_ploxion!(DEF);
