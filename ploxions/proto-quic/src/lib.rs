//! `proto-quic` — le protocole **QUIC**, en ploxion (vague 1 du reseau-en-ploxions).
//!
//! Tout le cycle vit dans le PROTOCOL-BION partage ([`ploxion_sdk::protocol`]) ; ici il ne
//! RESTE que le **residu** : la `ProtocolDef`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::protocol::ProtocolDef;

const DEF: ProtocolDef = ProtocolDef {
    name: "quic",
    number: 443,
    transport: "udp",
    layer: 4,
    brief: "transport moderne chiffre (QUIC)",
};

ploxion_sdk::protocol_ploxion!(DEF);
