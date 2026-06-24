# XERB0XI0N — un *tsoin engine*

**Stockage génératif content-adressé et runtime de ploxions WASM pour un substrat de calcul navigable et rejouable.**

*Thèse de bachelor — premier jet.*
Auteur : José (j0bot). Terrain rédactionnel : cloudion.

---

## Résumé

Tout système de calcul choisit une *unité* — l'instruction pour von Neumann, l'enregistrement
pour les bases de données, le fichier pour les systèmes de fichiers — et ce choix décide de ce
qu'on stocke, adresse, rejoue et compose. Le choix dominant a un coût : on conserve des
**résultats littéraux**, alors que les compressions les plus profondes ne gardent qu'une *règle*
et l'*écart* du réel à cette règle.

Cette thèse déplace l'unité : elle propose le **tsoin**, un couple `(générateur, résidu)` adressé
par son contenu, qui capte un état de façon **régénérable** plutôt que stockée. Le générateur
prédit l'état ; le résidu est la différence minimale — la *surprise* — entre cette prédiction et
le réel ; la reconstruction est **sans perte et vérifiable** (`addr(reconstruct) == addr(s)`).
C'est la transposition calculatoire du codage prédictif, posée comme principe de conception d'un
format de stockage.

La contribution est **constructive** : un système de référence, `xerboxion-rt` (hôte WebAssembly
+ bus PLC, organes LIVE, SDK des *bions*), démontre que les cycles s'exécutent réellement, que le
**déterminisme vaut rejouabilité**, que le budget tient dans **16 Go**, et que le *même* artefact
porte du serveur (`wasmtime`) au Raspberry Pi (`wasmi`) au microcontrôleur (`wasm3`). L'unité —
le tsoin — compose **fractalement** (`bion → ploxion → boxion → xerboxion`). L'état de l'art
montre que chaque brique a des ancêtres établis (content-addressing à la Git/IPFS/Unison,
runtimes WASM, codage prédictif, génération procédurale, capabilités, event sourcing) : la
nouveauté revendiquée est leur **recombinaison** en ce point de conception, non une primitive
inédite.

La thèse tient une **séparation stricte** entre le *posé* (implémenté, mesuré) et le *spéculatif*
(la cosmologie qui a motivé le design, présentée comme heuristique et non comme preuve).

**Mots-clés** — stockage content-adressé · codage prédictif · WebAssembly · runtime portable ·
déterminisme & rejouabilité · génératif · informatique frugale · composition fractale.

---

## Abstract *(traduction provisoire — langue finale à décider)*

Every computing system picks a *unit* — the instruction for von Neumann, the row for databases,
the file for filesystems — and that choice governs what is stored, addressed, replayed and
composed. The dominant choice is costly: it keeps **literal results**, whereas the deepest
compressions keep only a *rule* and the *deviation* of reality from it.

This thesis shifts the unit. It proposes the **tsoin**, a content-addressed
`(generator, residual)` pair that captures state in a **regenerable** rather than stored form.
The generator predicts the state; the residual is the minimal difference — the *surprise* —
between prediction and reality; reconstruction is **lossless and verifiable**. This is the
computational transposition of predictive coding, cast as a storage-format design principle.

The contribution is **constructive**: a reference system, `xerboxion-rt` (a WebAssembly host + a
PLC bus, LIVE organs, a *bions* SDK), shows that cycles actually run, that **determinism equals
replayability**, that the budget fits in **16 GB**, and that the *same* artifact is portable from
server (`wasmtime`) to Raspberry Pi (`wasmi`) to microcontroller (`wasm3`). The unit composes
**fractally**. Related work shows each building block has established ancestors; the claimed
novelty is their **recombination** at this design point, not a new primitive. Throughout, the
*implemented-and-measured* is kept strictly apart from the *speculative* motivation.

**Keywords** — content-addressed storage · predictive coding · WebAssembly · portable runtime ·
determinism & replay · generative · frugal computing · fractal composition.
