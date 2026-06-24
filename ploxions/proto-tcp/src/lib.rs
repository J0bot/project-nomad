//! `proto-tcp` — le protocole **TCP**, en ploxion (vague 1 du reseau-en-ploxions).
//!
//! Tout le cycle (manifest/init/health/goodbye/on_event, parse du PDU, re-emission
//! `net.tcp.out`, gravure du tsoin) vit dans le PROTOCOL-BION partage
//! ([`ploxion_sdk::protocol`]). Ici il ne RESTE que le **residu** : la `ProtocolDef`.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::protocol::ProtocolDef;

const DEF: ProtocolDef = ProtocolDef {
    name: "tcp",
    number: 6,
    transport: "-",
    layer: 4,
    brief: "flux fiable ordonné",
};

ploxion_sdk::protocol_ploxion!(DEF);
