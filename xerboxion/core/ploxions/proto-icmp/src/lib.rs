//! `proto-icmp` — le protocole **ICMP**, en ploxion (vague 1 reseau-en-ploxions).
#![allow(clippy::missing_safety_doc)]
use ploxion_sdk::protocol::ProtocolDef;
const DEF: ProtocolDef = ProtocolDef { name: "icmp", number: 1, transport: "-", layer: 3, brief: "écho et erreurs (ping)" };
ploxion_sdk::protocol_ploxion!(DEF);
