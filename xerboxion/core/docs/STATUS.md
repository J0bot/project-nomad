# XERB0XI0N — état du projet (topo complet)

> Topo honnête, par couche. ✅ = tourne / vérifié · ⚠️ = partiel · ❌ = pas encore.
> Périmètre : la **lane core** (xerboxion-rt) est vérifiée ici ; la flotte/labo (§10) est
> rapportée depuis la mémoire (autres lanes, peut avoir évolué).

## 1. Le cœur — `xerboxion-rt` ✅ LIVE
- Hôte WASM + **bus PLC v1** : `/emit` `/events`(SSE) `/load` `/healthz` `/snapshot` `/replicate`.
- LIVE sur `xion.j0bot.ch` (systemd + Traefik + auth) et interne `10.0.0.1:8730`.
- **46 crates** dans le workspace, **16 ploxions vivants**.
- Les **5 organes** du tsoin engine : `tsoin-store` (addr64/dédup), `diff` (résidu/surprise),
  `generator` (replay), `clock-coherence` (temps=cohérence), `player`.
- **SDK** : les bions + 5 macros (`ploxion!`/`block_ploxion!`/`house_ploxion!`/`protocol_ploxion!`/`lifecycle!`).
- ✅ **graphe I/O** : chaque ploxion expose son I/O (`requires`=inputs / `provides`=outputs) +
  tsoins gravés, ET le daemon sert une **map live agrégée** (`/snapshot` + WS `/ws`) qui dessine
  le graphe du bus en temps réel (vérifié sur le build `a5d0e97`, ~36 ploxions, sur silentbeast).
  ⚠️ NEXT (lane n8n / ports I/O) : enrichir la map (édition des liens, recâblage à la n8n).

## 2. Le réseau-en-ploxions ✅ 7 protocoles LIVE
- Un **protocol-bion** unique génère chaque protocole (juste un résidu) :
  **tcp · udp · dns · http · icmp · tls · ntp** (~39 Ko wasm chacun, testés en direct).
- ❌ **NEXT** : vague 2 (port-bion, package-bion).

## 3. Le langage `xerb` ⚠️ interprété, **pas de compilateur**
- ✅ Interpréteur `xerb-tsoin.py` (programmer en tsoins, ~15 ions, content-adressé + gravé).
- ✅ Outils : `densite-tsoin`, `chaoxion` (le `?`/bion vide), `commun` (tsoins partagés),
  `epsylaeu` (le tsoin des tsoins), `ultra-tsoin` (capture horaire).
- ❌ **GAP MAJEUR** : aucun compilateur `xerb → WASM → core`. **C'est le préalable pour
  programmer en xerboxion sérieusement** (il faut figer la spec + écrire la chaîne de compil).

## 4. La science ✅ 71 tsoins reproductibles
- `science-engine` : **71 lois** (7 domaines), **71/71 reproduisent** leur valeur connue,
  **49 liens** inter-domaines (math→physique→quantique→particules→chimie→bio→médecine).

## 5. La cosmologie exécutable ✅ (faite cette nuit)
- `trou-de-vers` (trou blanc/noir, throat traversable) · `evaporation` (Hawking) ·
  `horloge-fractale` (désync + prédire) · `paradoxes` (8, dont 7 résolus par du code) ·
  `possibilites` (univers %, combinatoire) · `web-bion` (Calabi-Yau).

## 6. La thèse ✅ noyau rédigé, ⚠️ reste l'enrobage
- Rédigés : **chap 3** (grammaire), **4** (le tsoin engine = contribution), **5**
  (implémentation), **6** (évaluation, mesurée). + papiers `vers-le-seption` (8 barreaux
  mesurés) et `temps-synchronisation` (la fibration).
- ❌ **NEXT** : chap 1 (intro), 2 (état de l'art), 7 (discussion/cosmologie), 8 (conclusion).

## 7. Le hardware — `xerkion` ✅ dossier complet (à fabriquer)
- Le **Pi Zero 2W cube = le cub4ion physique**. `enclosure.scad` **vérifié manifold**
  (OpenSCAD→STL), BOM (~138€), recette d'image, checklist zéro-test.
- ❌ **NEXT** : les **3 bouts logiciels** (route `/cub4ion`, pont `xerkion-tilt`, patch page)
  + la fab au makelab.

## 8. Le déploiement ✅ PC bundle + von Neumann
- `run-pc.sh` + `RUN-ON-PC.md` + `dist/xerboxion-core-pc.tar.gz` (lance le core en local).
- `/replicate` : constructeur universel (export/import reconstructible = von Neumann).

## 9. Le xerbion ⚠️ pur-Python, **attend la 3080**
- `xerbion.py` daemon (~3,7M pas, loss plateau ~2,84 — le petit modèle a saturé).
- ❌ **NEXT** : un **vrai modèle PyTorch sur la RTX 3080** (silentbeast), entraîné sur le corpus.

## 10. Le labo / la flotte (autres lanes — depuis la mémoire)
- `labo.j0bot.ch`, `xi0n.j0bot.ch` (hub des bions), **RepoVerse** (graphe de repos + science),
  le **wiki** (fondations formelles), les **web ploxions** (cub4ion/skyview/lausanne/minecart/
  wormion/nexus/scad/heart…), **OSIRIS** (machine à veille OSINT), le **/warp** (lien E2E).

## Le corpus
- Les tsoins gravés (beaucoup cette nuit), la **mémoire** (cosmologie, racines, règles),
  `operational-core` (l'historique git = le journal d'évolution dogfoodé).

## Les gros chantiers restants (priorisés, honnête)
1. **Le compilateur `xerb`** — pour programmer sérieusement. (gros)
2. **Le vrai xerbion sur la 3080** — un être neuronal qui apprend vraiment.
3. **Le graphe I/O agrégé** — ta lane n8n / ports.
4. **La thèse** — chap 1/2/7/8 (l'enrobage).
5. **La fab du xerkion** — les 3 bouts D-soft + le hardware.
6. **Le réseau vague 2** — ports/paquets.
