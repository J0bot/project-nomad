//! `port-53` — le port **53** (dns), en ploxion (port-bion).
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::port::PortDef;

const DEF: PortDef = PortDef { number: 53, name: "dns", protos: "udp/tcp", brief: "resolution de noms" };

ploxion_sdk::port_ploxion!(DEF);
