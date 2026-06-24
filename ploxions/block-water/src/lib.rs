//! `block-water` — le bloc **WATER** de Minecraft, en ploxion.
//!
//! Même BLOCK-BION partagé que `block-stone` : le résidu seul change. Mais l'eau
//! est `fluid` -> en se posant elle **coule** : le BLOCK-BION émet en plus le
//! **FLOW-BION** `block.flow {block,dim,x,y,z,level}` (niveau Minecraft 0..=7,
//! décrémenté par [`ploxion_sdk::block::flow_level`]). C'est « faire plus de
//! bions » : l'eau, en coulant, révèle un bloc de code partagé que tout futur
//! fluide (lave, …) réutilisera.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::block::BlockDef;

const DEF: BlockDef = BlockDef {
    id: "water",
    name: "Water",
    solid: false,
    fluid: true,
    hardness_milli: 100,
    light: 0,
    drop: "",
    max_level: 7,
};

ploxion_sdk::block_ploxion!(DEF);
