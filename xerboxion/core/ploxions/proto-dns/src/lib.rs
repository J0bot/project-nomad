//! `proto-dns` — le protocole **DNS**, en ploxion (vague 1 du reseau-en-ploxions).
#![allow(clippy::missing_safety_doc)]
use ploxion_sdk::protocol::ProtocolDef;
const DEF: ProtocolDef = ProtocolDef { name: "dns", number: 53, transport: "udp/tcp", layer: 7, brief: "résolution de noms" };
ploxion_sdk::protocol_ploxion!(DEF);
