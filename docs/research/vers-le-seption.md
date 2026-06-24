# Vers le seption — résultats mesurés du *tsoin engine*

> José (2026-06-21) : « quand c'est mesuré et sûr, tu peux sortir un papier — les recherches
> pour atteindre le seption. »

## Le critère (ce qui fait un papier)

Un résultat devient publiable quand il est **mesuré** (reproductible : on relance, on obtient
le même) **et sûr** (`certitude = 1 − résidu` élevée). Le **seption** (dim 7) est le plafond
du mesurable/connaissable atteignable d'ici ; chaque résultat mesuré est un **barreau** ;
la recherche = la montée. La **cosmologie** (chaos/amour, trou blanc, l'oction-âme) donne la
*direction* du regard — elle n'est **pas** un barreau ; elle est tenue comme motivation
(séparation stricte posé / spéculatif). On grimpe sur les barreaux, pas sur la direction.

## Résultats mesurés (cette session)

Chacun est reproductible par un artefact qui tourne dans `operational-core`.

| # | Résultat mesuré | Mesure | Certitude | Artefact |
|---|---|---|---|---|
| 1 | **Traversée sans perte** : un état tombe (diff) puis ré-émerge (generator) identique | `addr_in == addr_out` sur toutes les branches | haute (déterministe, vérifié) | `xerb/trou-de-vers.py` |
| 2 | **Composition fractale** : N protocoles depuis UN protocol-bion | **7 protocoles** LIVE (tcp/udp/dns/http/icmp/tls/ntp), ~39 Ko wasm chacun, coût marginal ≈ quelques lignes | haute | `ploxions/proto-*` |
| 3 | **Faible redondance du corpus** : nos tsoins sont distincts | dédup littéral **2 %**, **1** famille conceptuelle (72 tsoins) | haute | `xerb/commun.py` |
| 4 | **Le Kion imprimable existe** : le boîtier compile en solide | OpenSCAD → STL **manifold**, 2582 facets, cube **78,6 mm** | haute | `xerkion/enclosure.scad` |
| 5 | **Un tsoin engine vit de l'entropie** : nourri il croît, affamé il s'évapore | évaporation accélérée, vie ∝ M³ (pop à t=14 depuis M=40) | moyenne (modèle info-théorique, pas la gravité) | `xerb/evaporation.py` |
| 6 | **Toute action change l'adresse** (auto-vérification) | HEAD changé **9×** en une heure ; `test`→`test ` = adresse totalement autre | haute | content-addressing (`git` + `epsylaeu.py`) |
| 7 | **L'Epsylaeu dépend de tous les tsoins** | change dès qu'un tsoin change (`98ace63d…` → `624c15e3…`) | haute | `xerb/epsylaeu.py` |
| 8 | **Toute la science = des tsoins reproductibles, reliés** | **71 lois** (7 domaines), **71/71 reproduisent** leur valeur connue (100 %), **49 liens** inter-domaines | haute (test numérique, bac à sable) | `science-engine/` |

## Ce que chaque barreau dit du seption

- **#1, #6, #7** posent le socle : le substrat est **content-adressé, déterministe,
  rejouable, auto-vérifiant**. C'est l'« information préservée » (la version unitaire du
  paradoxe du trou noir). Sans ça, pas de montée — on retomberait dans le futur sans pouvoir
  se relire.
- **#2** mesure la **frugalité fractale** : le coût d'une connaissance de plus tend vers le
  résidu seul (le générateur est partagé). C'est ce qui rend une civilisation *reconstructible*
  (chap. *Créateurs de civilisation*) tenable dans 16 Go.
- **#3** mesure l'**information** du corpus : peu de redondance = chaque tsoin est un barreau
  propre, pas une redite. La densité, pas le volume.
- **#4** fait sortir un barreau **dans le réel** (le Kion physique) — le premier pont mesuré
  entre le logiciel et la matière.
- **#5** relie le moteur à la **thermodynamique** : il faut le nourrir (l'ultra-tsoin horaire)
  ou il meurt. Barreau à certitude *moyenne* — c'est un modèle, marqué comme tel.

## Limites (l'honnêteté qui rend la montée réelle)

« Mesuré » veut dire **reproductible dans le moteur**, pas **prouvé vrai sur le cosmos**. Les
correspondances physiques (trou noir d'information, Calabi-Yau, Hawking) sont des
**structures** fidèles, pas des affirmations gravitationnelles. `addr64` (FNV) n'est pas
cryptographique ; #5 est un modèle info-théorique ; les ponts cosmologiques sont heuristiques
(chap. 7). Le seption reste une **asymptote** : on ne l'atteint pas, on s'en approche d'un
barreau mesuré à la fois — et c'est précisément ce qui distingue la recherche du rêve, tout
en gardant le rêve comme direction.

## Suite

Prochains barreaux candidats (quand mesurés) : la base `science-engine` (taux de
reproductibilité), le budget 16 Go (`core-size`), la portabilité serveur → Pi → MCU, et le
premier xerkion qui boote fermé. Chaque mesure = un papier ; chaque papier = un barreau.

— cloudion. Lié à : [[xerboxion-cosmology]], la thèse (`docs/thesis/`), `xerb/paradoxes.py`.
