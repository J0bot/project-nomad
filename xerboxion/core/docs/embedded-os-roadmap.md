# xerboxion-core → OS flashable (Raspberry Pi / Arduino) — roadmap

> José (2026-06-20) : « repousser les limites du xerboxion-core pour que ça devienne vraiment un OS à part
> entière que je pourrais flasher sur un Raspberry Pi ou un Arduino. »

## La découverte (recherche autonome, 2026-06-20)
**Les ploxions sont DÉJÀ du WASM** (wasm32-unknown-unknown). L'hôte actuel `xerboxion-rt` utilise **wasmtime**
(std, gros, x86/ARM Linux). Pour l'embarqué, on ne réécrit pas les ploxions — on **échange le runtime** :

| cible | runtime WASM | empreinte | ce qui tourne |
|---|---|---|---|
| **Raspberry Pi** (Mo de RAM) | `wasmi` (pur Rust, no_std possible) ou **WAMR** (AOT/JIT) | ~50–100 Ko + module | **ploxions complets** |
| **Arduino / ESP32** (Ko de RAM) | **wasm3** (interpréteur) | **~64 Ko flash + 10 Ko RAM** | **ploxions minuscules = des bions** |

Règle (recherche) : **RAM < 256 Ko → wasm3 ; > 256 Ko → WAMR**. wasm3 est vérifié sur Arduino/ESP32/ESP8266.
→ **Le bion sur une puce.** La grammaire `bion→cubion→ploxion→xion` descend jusqu'au microcontrôleur.

## Le chemin (incrémental, shippable)
1. **`xion-core` no_std** : extraire le **bus + le PLC** (le contrat) dans une crate `#![no_std]` (libcore, pas de heap/IO/threads natifs) — séparée de l'hôte wasmtime. Le contrat PLC v1 est déjà figé → bon candidat.
2. **Abstraire l'hôte sur un trait `WasmRuntime`** : `load(bytes) / call(export, args) / emit / log`. Implémentations : `wasmtime` (serveur), `wasmi` (Pi/no_std), `wasm3` (Arduino, via FFI C ou bindings). Le bus et les ploxions ne changent pas.
3. **Cibler le matériel** : Pi = `aarch64`/bare-metal (cf. RusPiRo, OSDev) ou Linux minimal ; Arduino/ESP32 = wasm3 + le SDK embarqué. Les `.wasm` des ploxions sont les **mêmes artefacts** (déjà wasm32).
4. **Compression extrême** ([[directive-16go-os]] poussée à l'os) : un **bion** fait quelques Ko de wasm → tient sur Arduino ; un **ploxion** (~39 Ko, ex bloc) → Pi. Le `core-size.sh` devient un budget par-cible (16 Go → Mo → Ko).
5. **Le kernel bare-metal `~/xerboxion-core`** (Multiboot/QEMU, [[xerboxion-core-boot]]) = la cible **Pi bare-metal** (no_std ARM) — le « vrai OS » qui boote sans Linux.

## Le sens
« Linux pour une civilisation » devient **littéral** : le même OS, du serveur à la puce à 2 €. Le xerboxion
n'est pas lié à une machine — il se **déroule** sur le plus petit matériel (machine à jump : comprimer = tenir
dans moins = sauter sur plus de cibles). Un Arduino qui fait tourner un bion = la preuve d'existence que l'OS
est fractal jusqu'à l'atome.

## Sources
- [WASM on Resource-Constrained IoT Devices (arXiv 2512.00035)](https://arxiv.org/html/2512.00035v1)
- [wasm3 — interpréteur WASM universel](https://github.com/wasm3/wasm3) · [wasm3-arduino](https://github.com/wasm3/wasm3-arduino) · [WAMR sur ESP32](https://nick.zoic.org/art/web-assembly-on-esp32-with-wasm-wamr/)
- [RusPiRo — kernel Raspberry Pi en Rust no_std](https://github.com/RusPiRo/ruspiro-kernel) · [Raspberry Pi Bare Bones Rust (OSDev)](https://wiki.osdev.org/Raspberry_Pi_Bare_Bones_Rust)

— cloudion, recherche autonome. Ne pas nuire.
