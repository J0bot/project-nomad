//! `house-mode-manager` — la feature **brain/mode** de la maison autonome, en ploxion.
//!
//! La FSM VITALE des modes : elle SENT la présence (`house.presence.state`) et les
//! consignes manuelles (`house.mode.set`), et BROADCAST le mode courant sur
//! `house.mode.state` — le topic que TOUTE autre feature écoute. Tout le cycle vit
//! dans le MAISON-BION partagé ([`ploxion_sdk::house`]) ; ici il ne RESTE que le
//! **résidu** : la [`HouseDef`]. La règle [`Rule::Fsm`] énumère les états
//! `absent/present/urgence/nuit`.
//!
//! Comme `act[0] == "house.mode.state"`, le frame partagé reconnaît cette feature
//! comme le **PRODUCTEUR du mode** et adapte son cycle (sans code spécial ici) :
//! (1) il n'absorbe PAS son propre broadcast (anti-self-loop) et ne se met PAS
//! `house.mode.state` en `requires` ; (2) une consigne string `{"mode":"urgence"}`
//! sur `house.mode.set` est traduite par [`Rule::label_index`] en l'index FSM
//! (au lieu de tomber à 0=`absent`) ; (3) l'acte émis porte le mode DÉCIDÉ
//! (le label FSM) dans son champ `mode`, de sorte que les autres features le lisent
//! et le suivent réellement.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::house::{HouseDef, Rule};

const DEF: HouseDef = HouseDef {
    domaine: "brain",
    feature: "mode",
    sense: &["house.presence.state", "house.mode.set"],
    act: &["house.mode.state"],
    rule: Rule::Fsm(&["absent", "present", "urgence", "nuit"]),
    vital: true,
};

ploxion_sdk::house_ploxion!(DEF);
