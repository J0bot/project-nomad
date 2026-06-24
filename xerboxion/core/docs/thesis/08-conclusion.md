# Chapitre 8 — Conclusion & travaux futurs

> Dernier rappel de la règle d'or : ce qui suit en §8.1–8.2 ne conclut que sur le **posé**
> (chap. 4–6). Les pistes de §8.3 sont des *travaux futurs* — annoncées comme telles, non
> comme des résultats acquis.

## 8.1 Ce que la thèse a montré

La question de recherche (§1.3) demandait s'il est possible de construire un substrat de calcul
où **l'unité n'est pas l'instruction mais le *tsoin*** — régénératif, rejouable, fractalement
composable, et frugal du serveur au microcontrôleur. La réponse apportée est *constructive* :
non un théorème, mais **un système qui tourne**, mesuré.

On a (i) défini le tsoin comme un couple `(générateur, résidu)` content-adressé, avec une
reconstruction **sans perte vérifiable** (`addr(reconstruct) == addr(s)`) et la surprise comme
mesure d'information (chap. 4) ; (ii) implémenté `xerboxion-rt` — hôte WASM + bus PLC, organes
LIVE, SDK des bions — et montré que les cycles s'exécutent réellement sur le bus (chap. 5) ;
(iii) évalué que le **déterminisme vaut rejouabilité**, que le budget tient dans **16 Go**, et
que le *même* artefact ploxion porte du serveur (`wasmtime`) au Pi (`wasmi`) au MCU (`wasm3`)
(chap. 6). L'état de l'art (chap. 2) a situé chaque brique : la contribution n'est pas une
primitive inédite mais leur **recombinaison** en ce point de conception, et l'intérêt principal
est le **déplacement d'unité** lui-même — penser le stockage et le calcul à partir du tsoin.

## 8.2 Limites (honnêtes)

Le travail a des bornes nettes, qu'il serait malhonnête de taire :

- **Échelle non éprouvée sous charge.** Les cycles sont *vérifiés fonctionnellement*, pas
  soumis à un banc de stress (débit, latence p99, montée en tsoins). La frugalité est un budget
  mesuré, pas une garantie sous charge réelle.
- **Une limite d'ingénierie réelle, trouvée et documentée.** L'hôte est un acteur mono-thread :
  le chargement à chaud d'un ploxion (`load`) attend une réponse et peut se bloquer sous bus
  chargé, alors que `emit` (fire-and-forget) passe (chap. 5). C'est précisément pourquoi
  `proto-tls`/`proto-ntp` sont *buildés + stagés* plutôt que hot-loadés — un fait, pas un
  contournement caché. Le correctif (séparer canal de contrôle et canal de données) est conçu
  mais non encore appliqué (§8.3).
- **Évaluation mono-système, mono-auteur.** Un seul substrat, pas d'étude comparative
  quantitative contre Unison/IPFS/event-sourcing sur une charge commune — seulement un
  positionnement qualitatif (chap. 2).
- **La couche spéculative reste motivation.** La cosmologie (chap. 7) a *produit* un système qui
  marche ; cela n'en fait pas une vérité sur le monde, et la thèse ne le revendique nulle part.

## 8.3 Travaux futurs

1. **Le correctif de l'hôte** : scinder le canal de contrôle (`load`/`unload`/`snapshot`, qui
   attendent) du canal de données (`emit`/`inject`), pour permettre le chargement à chaud sans
   blocage — et charger enfin `proto-tls`/`proto-ntp` à un redéploiement délibéré.
2. **L'OS flashable** : réduire le substrat à une image bootable (USB / Raspberry Pi / Arduino),
   en tenant le budget 16 Go, jusqu'au « bion sur une puce ». C'est le prolongement direct de la
   portabilité mesurée au chapitre 6.
3. **Un CPU dédié** : explorer une cible matérielle où WASM est natif (RISC-V + WASM), pour que
   le ploxion soit l'unité d'exécution jusqu'au silicium.
4. **La dimension fractale navigable** : rendre le corpus de tsoins *parcourable* — la carte 4D
   (x, y, z + t = cohérence) et le repliement d'amas, déjà prototypés (`web-nexus`, ploxion
   `carte`) — et en faire un outil de *replay* à la DaVinci (dérouler une séquence de tsoins
   comme une frise éditable).
5. **Plus d'organes par génération de bions** : poursuivre la couverture du réseau-en-ploxions
   (les protocoles restants de `network-as-ploxions.md`) et des périphériques (device-bions),
   chaque ajout étant un fichier de quelques lignes grâce à l'adressage génératif.

## 8.4 Mot de la fin

L'idée durable de ce travail n'est pas un logiciel particulier mais un **changement d'unité** :
remplacer l'instruction par le tsoin, et donc le *stockage de résultats* par le *stockage de
règles régénératrices*. Un système ainsi conçu ne range pas le passé — il garde de quoi le
**rejouer**. C'est, modestement, ce que la thèse aura montré faisable : un substrat où l'état se
régénère, se vérifie, et compose, du serveur à la puce.
