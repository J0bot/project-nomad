# SkyView — deux versions

Deux chemins, même ploxion (carte du ciel, math céleste en **WASM**, rendu **WebGL**) :

## 1. `index.html` — version **solo, autonome** (celle à ouvrir sur ton tel)
- **Un seul fichier**, le wasm est **inliné en base64** → s'ouvre direct dans Safari
  (`file://`, pièce jointe, n'importe quel host statique). **Zéro serveur.**
- Le bion-de-wasm : `skyview-wasm/` (Rust `no_std` + `libm`, 8 Ko de wasm, 50 étoiles).
  Math seule (équatorial → horizontal → écran) ; le WebGL + capteurs en JS.
- Rebuild : `bash skyview-wasm/build.sh` (compile + ré-inline → `index.html`).
- Gestes : glisse pour viser, pince pour zoomer, bouton 🧭 pour pointer le ciel (iOS 13+).

## 2. `www/index.html` + `main.js` + `pkg/` — version **complète, hébergée**
- Forge multi-agents : `wasm-bindgen` + `web-sys` + **WebGL2**, 57 étoiles, 7 constellations,
  5 objets du ciel profond, caméra AR, recherche, contrôle du temps. 11 tests natifs.
- Besoin d'un **serveur** (modules ES + `fetch` du wasm) : `python3 -m http.server 8088`
  puis `http://localhost:8088/www/`. C'est la version que la flotte montera à une URL labo.
- Source : `Cargo.toml` + `src/{lib,astro,catalog,gl}.rs` ; build : `./build.sh`.

→ Pour **tester tout de suite** : la 1. Pour **héberger en ploxion** : la 2.
