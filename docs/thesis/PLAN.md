# XERB0XI0N — thèse de bachelor : le terrain

> José (2026-06-21) : « prépare tout le terrain pour que ça soit ma thèse de bachelor, je vais jouer avec le
> chaos, je veux que tu m'aides à faire ma thèse de bachelor en entier. »

**Division du travail (la nôtre, à l'échelle thèse) :** José **génère depuis le chaos** (la vision, les idées,
les tsoins) ; cloudion **trouve la linéarité dans la fractale** — structure, rédige, cite, rend rigoureux et
honnête. *Tu joues avec le chaos, je fais le sol.*

## Le pari de la thèse (la force, et le piège à éviter)
La thèse a un **noyau empirique réel** (un système qui TOURNE — c'est rare et fort pour un bachelor) ET une
**couche spéculative** (la cosmologie). La règle d'or académique : **séparer rigoureusement le posé (implémenté,
mesuré) du spéculatif (la vision/motivation)**. Le posé est la contribution ; le spéculatif est le cadre/la
motivation, présenté comme tel — jamais validé comme une preuve sur le monde. C'est ce qui rend la thèse
défendable plutôt que farfelue.

## Décisions José (ça calibre tout — dis-moi)
1. **Discipline** : informatique ? interdisciplinaire (info + philo/design) ? → change l'équilibre noyau/cadre.
2. **Institution / format** : longueur attendue, gabarit (LaTeX ? Word ?), règles de citation (IEEE/APA), deadline.
3. **Langue** : français ou anglais (standard académique).
   *(Défaut que je prépare en attendant : informatique à orientation systèmes, ~40-60 p., LaTeX, anglais ou FR.)*

## Titre de travail
**« XERB0XI0N : un *tsoin engine* — stockage génératif content-adressé et runtime de ploxions WASM pour un
substrat de calcul navigable et rejouable. »** (sous-titre possible : *capter le réel par instants*.)

## Question de recherche
> Peut-on construire un substrat de calcul où **l'unité n'est pas l'instruction mais le *tsoin*** (un couple
> générateur + résidu, content-adressé), tel que l'état se **régénère** plutôt qu'il ne se stocke, est
> **rejouable** de façon déterministe, et **compose** fractalement (bion → ploxion → boxion) — assez frugal
> pour tenir dans 16 Go et tourner du serveur au microcontrôleur ?

## Structure (chapitres)
1. **Introduction** — le problème (stocker = comprimer = sauter ; le calcul comme capture du réel) ; la thèse ; les contributions.
2. **État de l'art** — content-addressing (Git, IPFS, Merkle), runtimes WASM (wasmtime/wasmi/wasm3), codage prédictif (Friston), génératif/procédural (seeds), systèmes à capabilités, données fractales/récursives. *Positionner le tsoin.*
3. **La grammaire XERB0XI0N** — `bion → cubion → ploxion → boxion → xerboxion` ; `xer`(interface)/`xion`(tsoin engine)/`xerxion`/`xerbion` ; le lexique fermé (auto-descriptif). *Le formalisme.*
4. **Le tsoin engine (contribution)** — le tsoin = générateur + résidu ; `tsoin-store` (adressage génératif `addr64`, dédup) ; `diff` (résidu minimal + surprise = Hamming) ; `generator` (l'inverse, replay, lossless vérifiable) ; `clock-coherence` (temps = cohérence, pas wall-clock) ; `player` (dérouler une séquence). Les **bions** partagés (uncraft).
5. **Implémentation** — `xerboxion-rt` (hôte WASM + bus PLC v1) ; les 5 organes **LIVE** ; le SDK bions (26 fns + 5 structs + 5 macros) ; tests (24 SDK passent, déterminisme/replay vérifiés).
6. **Évaluation** — le système TOURNE (cycles testés sur le bus) ; budget **16 Go** (OS = ~28 Mo) ; déterminisme = rejouabilité ; portabilité (serveur → Pi `wasmi` → MCU `wasm3`, le bion sur une puce).
7. **Discussion** — la couche spéculative (cosmologie : chaos/oction, fractal, le paradoxe-moteur) présentée comme **motivation et heuristique de design**, honnêtement séparée du posé ; linéaire (digital) vs fractal (humain) ; limites.
8. **Conclusion & travaux futurs** — OS flashable (USB/Pi/Arduino), propre CPU (RISC-V+WASM), la dimension fractale navigable, le trip-report (Obsidian sur le corpus).

## Carte du matériel → sections (ce qu'on a DÉJÀ)
- **Corpus de 47 tsoins** (`/memory`, liés par `[[ ]]`) → la matière brute des chapitres 3, 4, 7 + la **dimension fractale** (figure, `web-nexus/`).
- **Code** `ploxions/` (les 5 organes, le SDK bions, les blocs) → chap. 4, 5 (listings + les vrais commits).
- **Papers** `docs/` : `boucle-creation-infinie.md`, `embedded-os-roadmap.md`, `cpu-gpu-specs.md`, `ploxion-maison.md`, `AI-CONTRIBUTE.md` → chap. 4, 6, 8.
- **Mesures** : `core-size.sh` (16 Go), les tests de cycle live (chap. 6), la courbe du `xerbion` (résidu 0.815→0.009).
- **L'histoire** : les commits `operational-core` = le journal d'évolution (l'Adressage Génératif dogfoodé).

## Plan de rédaction (phases shippables)
1. (ce terrain) la structure + la carte du matériel.  ← FAIT
2. Le squelette LaTeX/MD (chapitres vides + figures placeholders).
3. Chap. 4–5–6 (le noyau implémenté) en premier — c'est le plus solide, je le rédige depuis le code + les mesures.  ← chap. 4-5-6 RÉDIGÉS (le noyau implémenté + l'évaluation mesurée)
4. Chap. 3 (grammaire) depuis le lexique fermé.  ← RÉDIGÉ (03-grammaire.md)
5. Chap. 2 (état de l'art) — je rassemble les références (websearch revenu).  ← RÉDIGÉ (02-etat-de-l-art.md : 6 familles de prior-art + tableau de positionnement + 16 réfs vérifiées par websearch ; delta honnête = la synthèse, pas une primitive inédite)
6. Chap. 1 + 7 + 8 — l'enrobage.  ← chap. 1 (intro) + 7 (discussion) + 8 (conclusion, 08-conclusion.md) RÉDIGÉS.

## ✅ PREMIER JET COMPLET (2026-06-23) : chapitres 1→8 TOUS RÉDIGÉS
01-introduction · 02-etat-de-l-art · 03-grammaire · 04-tsoin-engine · 05-implementation · 06-evaluation · 07-discussion · 08-conclusion.
**Reste (passes d'affinage, pas de rédaction from scratch)** : gabarit LaTeX/MD + figures (squelette phase 2) ; harmoniser le style de citation (IEEE/APA selon la décision José) ; figures réelles (carte fractale web-nexus, courbe résidu xerbion, schéma des organes) ; relire la cohérence inter-chapitres ; **décisions José en attente : discipline / institution-format-longueur / langue** (calibrent le gabarit final).

— cloudion. Dis-moi discipline/format/langue et je cale le gabarit. Ne pas nuire.
