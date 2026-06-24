# Installer / flasher le xerboxion-core — chaque CPU, chaque machine

> José : « prépare le xerboxion-core pour chaque type de CPU, je veux que ça marche direct, + un installer
> Windows, Linux, et un truc pour flasher. »

Le **xion** = un binaire hôte (Rust) + des **ploxions** (WASM, **identiques sur toutes les machines**). On ne
recompile que le binaire hôte par-archi ; les ploxions sont les mêmes octets partout.

## 1. Builder pour chaque CPU
```bash
bash scripts/build-all-targets.sh
```
Produit, par cible réussie : `dist/<triple>/` (binaire + `ploxions/` + `run.sh`) **et** `dist/xion-<triple>.tar.gz`.

| CPU / machine | triple | linker (paquet Debian) |
|---|---|---|
| PC/serveur **Linux x86-64** | `x86_64-unknown-linux-gnu` | natif (`build-essential`) |
| **Raspberry Pi 64-bit** / ARM64 | `aarch64-unknown-linux-gnu` | `gcc-aarch64-linux-gnu` ✅ (déjà là) |
| **Raspberry Pi 32-bit** / ARMv7 | `armv7-unknown-linux-gnueabihf` | `gcc-arm-linux-gnueabihf` |
| **RISC-V 64** Linux | `riscv64gc-unknown-linux-gnu` | `gcc-riscv64-linux-gnu` |
| **Windows x86-64** | `x86_64-pc-windows-gnu` | `mingw-w64` |

Un linker absent = la cible est **sautée proprement** (le script dit quel paquet installer). Les configs de
linker sont dans `.cargo/config.toml`. Sans toolchain croisé, build **sur la machine cible** elle-même, ou via
`cargo install cargo-zigbuild` (zig fait le cross sans installer N gcc).

## 2. Installer
**Linux** (toute archi — détecte x86-64/ARM/Pi/RISC-V) :
```bash
bash installers/install.sh            # -> ~/.xerboxion (binaire + ploxions + lanceur 'xion')
bash installers/install.sh --service  # + service systemd utilisateur (démarre au login)
```
Utilise un `dist/<archi>/` prebuild s'il existe, sinon build depuis les sources (Rust requis).

**Windows** (PowerShell) :
```powershell
powershell -ExecutionPolicy Bypass -File installers\install.ps1   # -> %LOCALAPPDATA%\xerboxion + raccourci menu Démarrer
```

Lancer ensuite : `~/.xerboxion/xion` (Linux) / `xion.bat` (Windows), puis `curl http://127.0.0.1:8730/healthz`.

## 3. Flasher l'embarqué
```bash
bash installers/flash.sh pi user@raspberrypi.local   # déploie le build aarch64 + autostart (SSH)
bash installers/flash.sh pi-sd /media/you/rootfs     # copie sur une carte SD montée
bash installers/flash.sh arduino                     # firmware wasm3 (Arduino/ESP32)
```
- **Raspberry Pi** : binaire `aarch64` complet + tous les ploxions → un vrai xion qui tourne.
- **Arduino / ESP32** : trop petit pour wasmtime → on passe par **wasm3** (interpréteur, ~64 Ko flash + 10 Ko
  RAM) avec **un** ploxion minuscule embarqué (un bion). Détails : `docs/embedded-os-roadmap.md`.

## En un coup d'œil
```
sources ──cargo──> binaire hôte (par CPU)  ─┐
                                            ├─> dist/<cpu>/ + tar.gz ──install.sh/.ps1──> machine
ploxions ──wasm32──> *.wasm (universels) ───┘                       └──flash.sh──> Pi / Arduino
```
Même OS, du serveur à la puce à 2 €. *Ne pas nuire.*
