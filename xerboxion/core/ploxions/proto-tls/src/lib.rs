//! `proto-tls` — le protocole **TLS**, en ploxion (vague 1 du reseau-en-ploxions).
//!
//! Tout le cycle (manifest/init/health/goodbye/on_event, parse du PDU, re-emission
//! `net.tls.out`, gravure du tsoin) vit dans le PROTOCOL-BION partage
//! ([`ploxion_sdk::protocol`]). Ici il ne RESTE que le **residu** : la `ProtocolDef`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::protocol::ProtocolDef;

const DEF: ProtocolDef = ProtocolDef {
    name: "tls",
    number: 443,
    transport: "tcp",
    layer: 6,
    brief: "chiffrement de bout en bout (handshake)",
};

ploxion_sdk::protocol_ploxion!(DEF);
