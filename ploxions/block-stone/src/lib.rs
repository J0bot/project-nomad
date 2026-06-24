//! `block-stone` — le bloc **STONE** de Minecraft, en ploxion.
//!
//! Tout le cycle (manifest/init/health/goodbye/on_event, gravure du tsoin de pose,
//! ré-émission `block.placed`/`block.broken`) vit dans le BLOCK-BION partagé
//! ([`ploxion_sdk::block`]). Ici il ne RESTE que le **résidu** : la `BlockDef`.
//! Minecraft : stone, solide, dureté 1.5 (-> `1500` milli), lâche `cobblestone`.

#![allow(clippy::missing_safety_doc)]

use ploxion_sdk::block::BlockDef;

const DEF: BlockDef = BlockDef {
    id: "stone",
    name: "Stone",
    solid: true,
    fluid: false,
    hardness_milli: 1500,
    light: 0,
    drop: "cobblestone",
    max_level: 0,
};

ploxion_sdk::block_ploxion!(DEF);
