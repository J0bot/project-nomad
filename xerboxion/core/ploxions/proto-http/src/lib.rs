//! `proto-http` — le protocole **HTTP**, en ploxion (vague 1 reseau-en-ploxions).
#![allow(clippy::missing_safety_doc)]
use ploxion_sdk::protocol::ProtocolDef;
const DEF: ProtocolDef = ProtocolDef { name: "http", number: 80, transport: "tcp", layer: 7, brief: "requête/réponse web" };
ploxion_sdk::protocol_ploxion!(DEF);
