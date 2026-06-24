//! `proto-ssh` — le protocole **SSH**, en ploxion (vague 1 du reseau-en-ploxions).
//!
//! Tout le cycle vit dans le PROTOCOL-BION partage ([`ploxion_sdk::protocol`]) ; ici il ne
//! RESTE que le **residu** : la `ProtocolDef`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::protocol::ProtocolDef;

const DEF: ProtocolDef = ProtocolDef {
    name: "ssh",
    number: 22,
    transport: "tcp",
    layer: 7,
    brief: "shell distant chiffre",
};

ploxion_sdk::protocol_ploxion!(DEF);
