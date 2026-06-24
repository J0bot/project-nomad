//! `proto-udp` — le protocole **UDP**, en ploxion (vague 1 du reseau-en-ploxions).
#![allow(clippy::missing_safety_doc)]
use ploxion_sdk::protocol::ProtocolDef;
const DEF: ProtocolDef = ProtocolDef { name: "udp", number: 17, transport: "-", layer: 4, brief: "datagrammes" };
ploxion_sdk::protocol_ploxion!(DEF);
