# Lancer le xerboxion-core (le xion) sur ton PC

Le **xion** = l'hôte WASM + bus PLC qui charge **les 24 ploxions** et les fait tourner
ensemble, en local. Par défaut il écoute **127.0.0.1:8730** — rien d'exposé.

## Le plus rapide — bundle prebuilt (Linux x86-64, rien à installer)
Si tu as `xerboxion-core-pc.tar.gz` :
```bash
tar xzf xerboxion-core-pc.tar.gz && cd xerboxion-core
./run-pc.sh
```
Le script prend le binaire + les 24 `.wasm` déjà compilés (dossier `prebuilt/`) et lance
le xion direct. Ouvre un 2e terminal pour parler au bus (voir « Interagir »).

> Pas Linux x86-64 (Windows/Mac/ARM) ? Le binaire prebuilt ne tournera pas → passe par
> « Depuis les sources ». Le `run-pc.sh` rebuild tout seul si le binaire prebuilt manque.

## Depuis les sources (toute plateforme avec Rust)
Prérequis : **Rust** (https://rustup.rs). Le reste est automatique.
```bash
./run-pc.sh          # build host + ploxions (ajoute la cible wasm32) puis serve
```
Ou à la main :
```bash
rustup target add wasm32-unknown-unknown
cargo build --release -p xerboxion-host
bash scripts/build-ploxions.sh           # -> target/ploxions/*.wasm (24)
./target/release/xerboxion-rt serve target/ploxions --addr 127.0.0.1 --port 8730
```

## Interagir (2e terminal)
```bash
# santé
curl http://127.0.0.1:8730/healthz

# flux d'événements en direct (SSE) — laisse tourner pour tout voir passer
curl -N http://127.0.0.1:8730/events

# nourrir le premier xerbion (réseau de neurones) : envoie des x, lis xerbion.state
curl -X POST http://127.0.0.1:8730/emit -H 'Content-Type: application/json' \
  -d '{"topic":"xerbion.feed","payload":"{\"x\":0.5}"}'
```

Topics utiles : `xerbion.feed`→`xerbion.state`, `xerbionbit.feed`→`xerbionbit.state`
(ternaire/BitNet), `carte.map`→`carte.mapped` (place 4D), `synthe.*`/`spectre.*` (son),
`minecraft.command`→`mc.event` (mc-adapter), `tsoin.record` (graver un tsoin).
Endpoints HTTP : `/healthz` `/emit` `/events` (SSE) `/load` `/unload` `/snapshot`
`/replicate` `/paper`. (Détails dans le `README.md` racine.)

## Les 24 ploxions chargés
ping, pong, tracer, tsoin, state-client, watcher, health-adapter, ideas-map-adapter,
repoverse-adapter, gitea-adapter, osiris-adapter, index, science, synthe, xp,
ultra-detector, tester, link, **carte** (place 4D, t=cohérence), **kion** (réel→kion),
**spectre** (décompose le son en bions de fréquence), **mc-adapter** (Minecraft→tsoins),
**xerbion** (1er être neuronal, prédictif), **xerbion-bit** (xerbion ternaire 1-bit).

## Arrêt / état
`Ctrl-C` → le xion fait `plc_goodbye` proprement (droit au silence). L'état durable
est dans `./xion-state`. Relance `./run-pc.sh` : il repart de cet état.
