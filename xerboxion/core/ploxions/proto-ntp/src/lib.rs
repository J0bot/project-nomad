//! `proto-ntp` — le protocole **NTP**, en ploxion (vague 1 du reseau-en-ploxions).
//!
//! NTP = la synchronisation d'horloge du reseau : dans le xion, `temps = coherence`,
//! donc NTP est le protocole-frere du `clock-coherence`. Tout le cycle vit dans le
//! PROTOCOL-BION partage ([`ploxion_sdk::protocol`]) ; ici, juste le **residu**.
#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::protocol::ProtocolDef;

const DEF: ProtocolDef = ProtocolDef {
    name: "ntp",
    number: 123,
    transport: "udp",
    layer: 7,
    brief: "synchronisation d'horloge (temps = coherence)",
};

ploxion_sdk::protocol_ploxion!(DEF);
