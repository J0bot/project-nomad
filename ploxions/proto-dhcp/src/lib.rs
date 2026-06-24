//! `proto-dhcp` — le protocole **DHCP**, en ploxion (vague 1 du reseau-en-ploxions).
//!
//! Tout le cycle vit dans le PROTOCOL-BION partage ([`ploxion_sdk::protocol`]) ; ici il ne
//! RESTE que le **residu** : la `ProtocolDef`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::protocol::ProtocolDef;

const DEF: ProtocolDef = ProtocolDef {
    name: "dhcp",
    number: 67,
    transport: "udp",
    layer: 7,
    brief: "bail d'adresse (DHCP)",
};

ploxion_sdk::protocol_ploxion!(DEF);
