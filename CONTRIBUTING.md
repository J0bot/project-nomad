# Faire un ploxion / un bion — le guide

> Tout dans le xerboxion est un **bion** (l'atome) ou un **ploxion** (un service, une
> molécule de bions). Ce guide montre comment en fabriquer un, de zéro, et le faire tourner
> sur le xion (l'hôte WASM + bus). C'est tout ce dont tu as besoin pour contribuer.

## 1. C'est quoi, concrètement

- **bion** = la plus petite unité réutilisable (une fonction, une règle, une donnée
  content-adressée). Les bions partagés vivent dans le SDK (`ploxions/sdk`).
- **ploxion** = un **crate Rust** compilé en `wasm32-unknown-unknown`, qui s'abonne à des
  *topics* d'entrée et émet sur des *topics* de sortie via le **bus PLC**. Il ne connaît pas
  l'hôte : un ploxion est un module sandboxé.
- **tsoin** = un instant content-adressé (`addr64` = FNV-1a 64 bits). Graver un tsoin =
  `POST /emit {topic:"tsoin.record", payload:{name, bytes}}`.

## 2. Anatomie d'un ploxion (2 fichiers)

Un ploxion = un dossier `ploxions/<nom>/` avec :

```
ploxions/mon-ploxion/
├── Cargo.toml
└── src/lib.rs
```

**`Cargo.toml`** (copie un voisin, ex. `ploxions/proto-tcp/Cargo.toml`) :
```toml
[package]
name = "ploxion-mon-ploxion"
description = "WASM ploxion: <ce qu'il fait>."
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib"]      # <- obligatoire : compile en .wasm

[dependencies]
ploxion-sdk = { path = "../sdk" }
```

**`src/lib.rs`** — le cycle de vie + la logique, via les macros du SDK.

## 3. Les macros du SDK (il y en a 6)

Dans `ploxions/sdk/src/lib.rs` :

| Macro | À quoi ça sert |
|---|---|
| `ploxion!` | déclarer un ploxion générique (manifest + boot/tick/halt) |
| `ploxion_lifecycle!` | le cycle de vie OS d'une unité (boote → tourne → s'éteint) |
| `block_ploxion!` | un **bloc** (cubion) du jeu — pierre, eau, etc. |
| `protocol_ploxion!` | un **protocole réseau** depuis une `ProtocolDef` |
| `house_ploxion!` | une feature « maison » |
| `export_manifest!` | exposer le manifest (`requires` / `provides`) au host |

## 4. L'exemple le plus simple — un protocole, en 12 lignes

Le **protocol-bion** est le meilleur modèle : un protocole = juste un **résidu** (sa
`ProtocolDef`) branché dans le générateur partagé. Tout `ploxions/proto-*/src/lib.rs` :

```rust
//! `proto-mqtt` — le protocole MQTT, en ploxion.
#![allow(clippy::missing_safety_doc)]
use ploxion_sdk::protocol::ProtocolDef;

const DEF: ProtocolDef = ProtocolDef {
    name: "mqtt",
    number: 1883,
    transport: "tcp",
    layer: 7,
    brief: "publish/subscribe léger",
};

ploxion_sdk::protocol_ploxion!(DEF);
```

C'est ça, « ajouter un protocole = quelques lignes » : le cycle complet (parse, ré-émission
`net.mqtt.out`, gravure du tsoin) vit dans le générateur partagé ; toi tu n'écris que ce qui
change (le résidu). **C'est le principe : on stocke la règle, pas l'instance.**

## 5. L'enregistrer + le builder

1. **Ajoute-le au workspace** : dans `ploxions/Cargo.toml`, ajoute `"mon-ploxion"` à
   `members`.
2. **Ajoute une ligne de stage** dans `scripts/build-ploxions.sh` :
   ```sh
   stage ploxion_mon_ploxion   mon-ploxion
   ```
   (le 1er arg = le nom du crate avec `-`→`_` ; le 2e = l'id du ploxion, `-` gardé.)
3. **Build** :
   ```sh
   source "$HOME/.cargo/env"
   bash scripts/build-ploxions.sh          # build + stage tous les .wasm
   # ou juste le tien :
   cargo build --release --target wasm32-unknown-unknown \
       --manifest-path ploxions/Cargo.toml -p ploxion-mon-ploxion
   ```

## 6. Le charger + le tester sur le xion

Le xion tourne en local (`./run-pc.sh` → `http://127.0.0.1:8730`).

```sh
# charger le ploxion (renvoie son manifest : requires/provides)
curl -X POST http://127.0.0.1:8730/load -H 'Content-Type: application/json' \
     -d '{"id":"mon-ploxion"}'

# regarder le bus en direct
curl -N http://127.0.0.1:8730/events

# lui parler : émets sur un topic qu'il `requires`, observe le topic qu'il `provides`
curl -X POST http://127.0.0.1:8730/emit -H 'Content-Type: application/json' \
     -d '{"topic":"net.mqtt.in","payload":"{}"}'
```

Succès = tu vois `net.mqtt.out` (et le tsoin `proto:mqtt:...`) passer dans `/events`.

## 7. Le contrat du bus (l'essentiel)

- un ploxion **`requires`** des topics (ses entrées) et **`provides`** des topics (ses
  sorties) — c'est son I/O, exposé par son manifest ;
- tout passe par le bus : `POST /emit {topic, payload}` (payload = chaîne JSON), `GET /events`
  (SSE) ;
- pour **graver un tsoin** depuis ton ploxion : émets `tsoin.record {name, bytes}`. Il sera
  content-adressé et dédupliqué (deux instants identiques = le même bion).

## 8. Faire un *bion* (pas un ploxion entier)

Un bion partagé = une fonction `pub fn` dans `ploxions/sdk/src/bions.rs` (il y en a déjà des
dizaines : décodage, hash, math, etc.). Ajoute la tienne là, et tous les ploxions peuvent
l'`use`. Garde-la **pure et déterministe** (pas d'I/O, pas d'horloge murale) — c'est ce qui
rend un tsoin rejouable à l'identique.

## 9. La règle d'or

**Auto-contenu, déterministe, content-adressé.** Pas de chemin absolu, pas de dépendance hors
du repo, pas d'aléa non-graîné. Un bon ploxion/bion *boote, tourne, s'éteint sans trace*, et
se rejoue bit-à-bit n'importe où. C'est l'« état bion » : le plus stable, le plus partageable.

---
*Tu as fait ton premier ploxion ? Ouvre une PR sur `J0bot/xerboxion-core`. Le but du projet :
découvrir **tous** les bions et **tous** les ploxions. Chaque résidu compte.*
