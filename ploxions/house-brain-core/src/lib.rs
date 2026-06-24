//! `house-brain-core` — la feature **brain/core** de la maison autonome, en ploxion.
//!
//! Le HUB central VITAL : il SENT la demande d'énergie (`house.energy.demand.state`)
//! et l'alerte sécurité (`house.sec.alert.state`), et émet une `house.directive` —
//! l'arbitrage `sécurité > confort > éco`. Tout le cycle (manifest/init/health/
//! goodbye/on_event, maj du Mode, DÉCIDE, gravure du tsoin, ré-émission de l'acte)
//! vit dans le MAISON-BION partagé ([`ploxion_sdk::house`]). Ici il ne RESTE que le
//! **résidu** : la [`HouseDef`]. La règle [`Rule::Aggregate`] porte l'arbitrage —
//! en `Urgence` la sécurité SATURE la directive (priorité absolue), en `Absent`/
//! `Nuit` la demande est atténuée (éco) ; voir [`ploxion_sdk::house::Rule::decide`].

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::house::{HouseDef, Rule};

const DEF: HouseDef = HouseDef {
    domaine: "brain",
    feature: "core",
    sense: &["house.energy.demand.state", "house.sec.alert.state"],
    act: &["house.directive"],
    rule: Rule::Aggregate,
    vital: true,
};

ploxion_sdk::house_ploxion!(DEF);
