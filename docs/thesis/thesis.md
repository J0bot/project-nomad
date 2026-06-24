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


---

<div style="page-break-after: always"></div>

# Chapitre 1 — Introduction

> Ce chapitre pose le problème, l'intuition, la question de recherche et les contributions, et
> énonce d'emblée la **règle d'or** qui gouverne toute la thèse : séparer le *posé* (ce qui est
> implémenté et mesuré) du *spéculatif* (la vision qui a motivé le design). Le posé est la
> contribution ; le spéculatif est tenu pour ce qu'il est — une heuristique de conception
> (chap. 7), jamais une preuve sur le monde.

## 1.1 Le problème : quelle est l'unité ?

Tout système de calcul répond, explicitement ou non, à une question : *quelle est l'unité ?*
Le modèle de von Neumann répond « l'instruction » ; les bases de données « l'enregistrement » ;
les systèmes de fichiers « le fichier ». Ce choix d'unité décide de tout le reste — ce qu'on
stocke, ce qu'on adresse, ce qu'on rejoue, ce qui compose.

Le choix dominant a une conséquence coûteuse : on **stocke des résultats littéraux**. L'état
grossit parce qu'on conserve ce que le calcul a produit, octet par octet, plutôt que la *règle*
qui l'a produit. Or les compressions les plus profondes ne gardent jamais le résultat : elles
gardent un **modèle** et seulement l'**écart** entre ce que le modèle prédit et ce que le réel a
effectivement fait. C'est le principe de la longueur de description minimale (Rissanen, 1978 ;
Li & Vitányi, 2008) : le meilleur modèle d'une donnée est le plus court programme qui la
régénère. Comprimer, ce n'est pas ranger plus serré — c'est *trouver la règle*.

Reformulons le problème de stockage comme un problème de calcul : **stocker, c'est comprimer ;
comprimer, c'est faire de la place ; faire de la place, c'est pouvoir sauter** plus loin. Un
système qui ne stocke que des résultats ne fait jamais de place ; il accumule. Un système qui
stocke des règles régénératrices, lui, peut tenir un état immense dans un budget fini.

## 1.2 L'intuition : le calcul comme capture du réel

La thèse part d'un déplacement d'unité. Plutôt que l'instruction, on propose le **tsoin** : un
couple `(générateur, résidu)` adressé par son contenu, qui *capte un instant du réel de façon
régénérable plutôt que stockée*.

- Le **générateur** est une règle déterministe (un programme, une graine, une référence à un
  tsoin antérieur) qui **prédit** l'état.
- Le **résidu** est la différence minimale entre cette prédiction et l'état réel — exactement ce
  que la règle ne prédit pas, c'est-à-dire la **surprise**.

On ne stocke donc pas l'état `s` ; on stocke `(g, s ⊖ g())`, et l'on reconstruit
`s = g() ⊕ résidu` de façon **sans perte et vérifiable** (`addr(reconstruct) == addr(s)`). C'est
la transposition calculatoire littérale du codage prédictif (Rao & Ballard, 1999 ; Friston,
2010) : l'information utile est l'erreur de prédiction. Calculer, dans ce cadre, c'est moins
*exécuter des instructions* que *capter le réel par instants* et savoir le rejouer.

## 1.3 Question de recherche

> Peut-on construire un substrat de calcul où **l'unité n'est pas l'instruction mais le
> *tsoin*** (un couple générateur + résidu, content-adressé), tel que l'état se **régénère**
> plutôt qu'il ne se stocke, soit **rejouable** de façon déterministe, et **compose**
> fractalement (`bion → ploxion → boxion`) — assez frugal pour tenir dans 16 Go et tourner du
> serveur au microcontrôleur ?

La question est délibérément *constructive* : on n'y répond pas par un théorème mais par un
**système qui tourne**, mesuré. Pour un travail de bachelor, c'est la force du sujet — un noyau
empirique réel, rare à cette échelle — et son piège, qu'on désamorce en §1.5.

## 1.4 Contributions

La thèse défend cinq contributions, toutes adossées à du code et des mesures :

1. **Le formalisme du tsoin** (chap. 4) : la définition `(générateur, résidu)`, l'adressage
   génératif (`tsoin-store`, `addr64`), la surprise comme distance de Hamming, et la preuve de
   reconstruction sans perte vérifiable.
2. **Une implémentation de référence, `xerboxion-rt`** (chap. 5) : un hôte WASM + un bus PLC v1,
   les cinq organes **LIVE**, et le SDK des **bions** (les unités partagées, obtenues par
   *uncraft*), avec ses tests.
3. **Une évaluation** (chap. 6) : la démonstration que le système *tourne* (cycles testés sur le
   bus), que le **déterminisme vaut rejouabilité**, que le budget tient dans **16 Go**, et que le
   *même* artefact est portable du serveur (`wasmtime`) au Raspberry Pi (`wasmi`) jusqu'au
   microcontrôleur (`wasm3`).
4. **Une grammaire fermée et auto-descriptive** (chap. 3) : `bion → cubion → ploxion → boxion →
   xerboxion`, et le partage `xer` (interface) / `xion` (le tsoin engine), qui donne au substrat
   sa composition **fractale**.
5. **Une séparation méthodologique honnête** (chap. 7) entre le noyau implémenté et la
   cosmologie spéculative qui l'a motivé — contribution de *posture* autant que de contenu, et
   condition de défendabilité (§1.5).

L'état de l'art (chap. 2) montre que chacune de ces briques a des ancêtres établis
(content-addressing à la Git/IPFS/Unison, runtimes WASM, codage prédictif, génération
procédurale, capabilités, event sourcing) : la nouveauté revendiquée n'est pas une primitive
inédite mais leur **recombinaison** en ce point de conception précis.

## 1.5 Le posé et le spéculatif (règle d'or)

Le projet XERB0XI0N comporte deux strates qu'il serait malhonnête de confondre. Une strate
**posée** : un système qui existe, tourne et se mesure — c'est la contribution. Une strate
**spéculative** : une cosmologie (le chaos, le fractal, le « paradoxe-moteur ») qui a *motivé* le
design. La discipline tenue dans toute la thèse est de **ne jamais valider le spéculatif comme
une preuve sur le monde**. La cosmologie apparaît au chapitre 7, présentée comme *heuristique de
conception et motivation*, utile parce qu'elle a produit un système qui marche — pas parce
qu'elle serait vraie. C'est ce qui rend le travail défendable plutôt que farfelu : les chapitres
4 à 6 ne contiennent que de l'implémenté et du mesurable, et chacun s'ouvre par un rappel de
cette frontière.

## 1.6 Plan du document

- **Chap. 2 — État de l'art** : positionne le tsoin contre six familles de travaux et délimite
  le *delta* honnêtement.
- **Chap. 3 — La grammaire XERB0XI0N** : le lexique fermé et la composition fractale.
- **Chap. 4 — Le tsoin engine (contribution)** : générateur + résidu, adressage génératif,
  surprise, reconstruction, replay.
- **Chap. 5 — Implémentation** : `xerboxion-rt`, le bus PLC, les organes LIVE, le SDK des bions.
- **Chap. 6 — Évaluation** : le système tourne ; déterminisme = rejouabilité ; budget 16 Go ;
  portabilité serveur → Pi → MCU.
- **Chap. 7 — Discussion** : la couche spéculative, honnêtement séparée ; linéaire (digital) vs
  fractal (humain) ; limites.
- **Chap. 8 — Conclusion & travaux futurs** : OS flashable, CPU dédié (RISC-V + WASM), la
  dimension fractale navigable.


---

<div style="page-break-after: always"></div>

# Chapitre 2 — État de l'art

> Rôle de ce chapitre : **positionner** le *tsoin* dans la littérature existante, pas le
> valider. On y montre que chacune de ses briques a des ancêtres solides et bien établis ;
> la contribution (chap. 4–6) n'est pas une brique nouvelle isolée mais leur **recombinaison**
> en un point de conception précis — un substrat où l'unité de stockage *et* de calcul est un
> couple `(générateur, résidu)` content-adressé, régénérable, rejouable et fractalement
> composable, assez frugal pour aller du serveur au microcontrôleur. On sépare donc, ici déjà,
> l'emprunté (cet état de l'art) du revendiqué (§2.7).

Cinq familles de travaux bordent le tsoin. On les passe en revue, puis on situe précisément
ce que le tsoin reprend et ce qu'il déplace.

## 2.1 Adressage par contenu

Nommer une donnée par le hash de son contenu plutôt que par un chemin est une idée ancienne
et éprouvée. Les **arbres de Merkle** (Merkle, 1987) fondent l'intégrité vérifiable par
hachage récursif : un nœud nomme ses enfants par leur empreinte, si bien qu'une adresse
certifie tout le sous-arbre. **Git** (Chacon & Straub, 2014) en fait un magasin d'objets
content-adressé : blobs, arbres et commits sont rangés sous le SHA-1 de leur contenu, ce qui
donne déduplication, immuabilité et historique gratuits. **IPFS** (Benet, 2014) généralise le
procédé en un *Merkle-DAG* distribué — « content-addressed, versioned » — où l'hyperlien est
une empreinte et non une URL, supprimant tout point de défaillance unique.

Le cas le plus proche du tsoin est **Unison** (Unison Computing). Unison adresse le *code* par
le hash de son AST sérialisé (512 bits, SHA3) : une définition est identifiée par son contenu,
pas par son nom — les noms deviennent de simples métadonnées. On y gagne le renommage trivial,
le cache de résultats de tests, l'absence de conflits de dépendances et le déplacement
d'arbitraires calculs d'une machine à l'autre. **Nix** (Dolstra, 2006) applique l'esprit au
déploiement : des dérivations content-adressées rendent les builds reproductibles.

**Position du tsoin.** Comme Git/IPFS/Unison, le tsoin nomme par contenu (`addr64`, §4.2) et
hérite de la déduplication et de la rejouabilité. La différence est l'**objet adressé** : ni un
blob (Git/IPFS), ni une définition de code figée (Unison), mais un **couple
`(générateur, résidu)`**. L'adresse ne désigne donc pas un état stocké mais une *recette pour
le régénérer*. C'est l'« adressage génératif » : on partage la règle (peu de bits) et l'écart au
réel (le résidu), au lieu du résultat entier.

## 2.2 Runtimes WebAssembly et portabilité

**WebAssembly** (Haas et al., 2017) est un format d'instructions bas niveau, sûr et portable,
conçu *avec* une sémantique formelle dès l'origine — fait rare, qui en fait une cible
d'exécution déterministe et vérifiable. L'écosystème offre un dégradé de runtimes selon la
cible : `wasmtime` (JIT/AOT, serveur), `wasmi` (interprète Rust embarquable), et pour le
très contraint **wasm3** (interprète minimal, démarrage rapide, faible empreinte, support
RISC-V et ARM-32, sans dépendance OS) et **WAMR** (Bytecode Alliance, interprète + AOT + JIT
configurable de l'embarqué au cloud). Des travaux récents mesurent ce compromis
portabilité/performance/énergie sur cibles IoT contraintes (arXiv:2404.12621, 2024 ;
arXiv:2512.00035, 2025). **WASI** (Bytecode Alliance) standardise par ailleurs un accès système
à capabilités pour ces modules hors navigateur.

**Position du tsoin.** Le **ploxion** est un module WASM (§5). On reprend WebAssembly tel quel,
pour deux propriétés qu'il garantit déjà : le **déterminisme** (condition de la rejouabilité,
chap. 6) et la **portabilité d'un même artefact** du serveur (`wasmtime`) au Raspberry Pi
(`wasmi`) jusqu'au microcontrôleur (`wasm3`). Le tsoin n'invente pas de runtime ; il *exploite*
ce dégradé pour soutenir la revendication « du serveur à la puce », et ajoute par-dessus le bus
PLC et l'adressage génératif (ce que WASM ne fournit pas).

## 2.3 Codage prédictif et compression (le résidu = surprise)

L'idée que comprendre, c'est prédire — et que l'information utile est l'écart à la prédiction —
est centrale en neurosciences computationnelles. Le **codage prédictif** (Rao & Ballard, 1999)
modélise la perception comme une hiérarchie qui ne propage que l'*erreur de prédiction*. Le
**principe d'énergie libre** (Friston, 2010) en fait un cadre unificateur : le système minimise
la surprise (l'énergie libre variationnelle) entre son modèle génératif et l'entrée sensorielle.
Côté formel, la **complexité de Kolmogorov** et la **longueur de description minimale** (Rissanen,
1978 ; Li & Vitányi, 2008) posent que le meilleur modèle d'une donnée est le programme le plus
court qui la régénère — comprimer, c'est trouver la règle.

**Position du tsoin.** Le tsoin est la transposition *calculatoire et littérale* de ce schéma :
le **générateur** joue la prédiction, le **résidu** est exactement l'erreur de prédiction
(§4.1), et la valeur d'information d'un tsoin est sa **surprise**, mesurée comme distance de
Hamming entre prédiction et état (§4.3). Là où le codage prédictif est un modèle *descriptif* du
cerveau, on s'en sert comme **principe de conception constructif** d'un format de stockage : on
ne stocke pas l'état, on stocke `(règle, surprise)`. On n'en revendique aucune validité
neuroscientifique (cette analogie reste motivation, chap. 7) ; on en retient le mécanisme de
compression prédictive, dont la garantie de reconstruction sans perte est, elle, prouvée (§4.4).

## 2.4 Génératif, procédural et déterminisme par graine

Régénérer un grand état à partir d'une petite graine est la pratique courante de la **génération
procédurale de contenu** (Shaker, Togelius & Nelson, 2016) : un monde entier — Minecraft en est
l'exemple populaire — se reconstruit de façon déterministe à partir d'une seed. La même graine
donne le même monde, partout. Côté ingénierie, les **builds reproductibles** (Nix ; Dolstra,
2006) visent la propriété duale : à entrées identiques, sortie bit-à-bit identique.

**Position du tsoin.** Le tsoin partage la thèse « la graine *est* l'état » : le générateur est
une graine déterministe (§4.1) et l'on vérifie que rejouer reproduit l'adresse d'origine
(`addr(reconstruct) == addr(s)`, §4.4 ; mesures chap. 6). La nuance : la génération procédurale
classique produit *du contenu nouveau* à partir d'une graine, tandis que le tsoin *capte un état
existant* — il ajoute le **résidu** pour combler l'écart entre ce que la graine prédit et ce que
le réel a effectivement produit. Graine + résidu = régénération fidèle, pas seulement
plausible.

## 2.5 Capabilités, composition et substrat fractal

Le contrôle d'accès par **capabilités** remonte à Dennis & Van Horn (1966) : un droit est un
jeton infalsifiable que l'on détient et que l'on peut transmettre, sans autorité centrale. Le
**modèle à capabilités-objets** (Miller, 2006) en fait une discipline de composition sûre, où
autorité = référence. WASI (§2.2) adopte ce modèle pour l'accès système des modules WASM.

**Position du tsoin.** Les **bions** et leurs **ports** d'entrée/sortie (§5) suivent cet esprit :
un ploxion ne peut agir que sur ce à quoi un port le relie ; le lien *est* le droit (le `/warp`,
chap. 7, en est l'expression réseau). La composition `bion → cubion → ploxion → boxion →
xerboxion` (chap. 3) est une composition **fractale** : la même grammaire se répète d'échelle en
échelle, propriété rare que l'on rapproche, côté données, des structures récursives type
Merkle-DAG (§2.1). Le tsoin n'apporte pas de nouveau modèle de sécurité ; il *réutilise* les
capabilités comme colle de composition entre unités content-adressées.

## 2.6 Journaux append-only et event sourcing

Enfin, faire du **journal la source de vérité** — reconstruire l'état en rejouant une suite
d'événements immuables — est le cœur de l'**event sourcing** (Fowler, 2005) et des moteurs de
stockage à écriture séquentielle (LSM-tree ; O'Neil et al., 1996). L'état courant n'est qu'une
projection rejouable de l'historique.

**Position du tsoin.** Le bus PLC et le `player` (§4) sont exactement cela : les tsoins forment
un journal ordonné et rejouable dont l'état présent est une projection (le *ledger* des
mouvements de l'inventaire en est une instance concrète, chap. 5). Le tsoin précise *quoi* met-on
dans le journal : non des deltas opaques, mais des couples `(générateur, résidu)` content-adressés
— donc dédupliqués, vérifiables et navigables, là où l'event sourcing classique journalise des
événements applicatifs arbitraires.

## 2.7 Synthèse : ce que le tsoin reprend, ce qu'il déplace

| Famille (§) | Acquis réutilisé | Ce que le tsoin déplace |
|---|---|---|
| Adressage par contenu (2.1) | nommer par hash → dédup, immuabilité, rejeu | l'objet adressé devient `(générateur, résidu)`, pas un blob ni du code figé |
| Runtimes WASM (2.2) | déterminisme + portabilité serveur→Pi→MCU | un même artefact ploxion sur tout le dégradé, plus bus + adressage génératif |
| Codage prédictif (2.3) | information = surprise = écart à la prédiction | en fait un *format de stockage* constructif (règle + surprise), reconstruction prouvée |
| Génératif / seeds (2.4) | la graine régénère l'état déterministe | ajoute le résidu → régénération *fidèle* d'un réel capté, pas seulement du contenu nouveau |
| Capabilités (2.5) | le lien est le droit ; composition sûre | colle de composition **fractale** entre unités content-adressées |
| Journaux / event sourcing (2.6) | l'état est une projection rejouable du journal | on journalise des tsoins content-adressés, pas des événements opaques |

**Le delta, honnêtement.** Aucune des six briques n'est neuve, et on ne le prétend pas. La
contribution défendue (chap. 4–6) est leur **synthèse en un seul objet** — le tsoin — et la
démonstration qu'un substrat bâti sur lui *tourne* réellement, reste déterministe et rejouable,
compose fractalement, et tient dans un budget frugal (16 Go ; chap. 6). À notre connaissance, la
combinaison précise « unité = couple générateur+résidu content-adressé, état régénératif,
rejouable, fractal, du serveur au MCU » n'est pas occupée par un système existant : Unison
adresse du code mais pas l'état-comme-résidu ; IPFS adresse des blobs mais ne régénère pas ; la
génération procédurale régénère mais ne capte pas le réel ; l'event sourcing rejoue mais
journalise des événements opaques. C'est cet interstice que la thèse instrumente — modestement,
en montrant un système qui marche plutôt qu'en revendiquant une primitive inédite.

## Références

- Benet, J. (2014). *IPFS — Content Addressed, Versioned, P2P File System.* arXiv:1407.3561.
- Bytecode Alliance. *WebAssembly Micro Runtime (WAMR).* github.com/bytecodealliance/wasm-micro-runtime ; *WASI — WebAssembly System Interface*, github.com/WebAssembly/WASI.
- Chacon, S., & Straub, B. (2014). *Pro Git* (2ᵉ éd.). Apress.
- Dennis, J. B., & Van Horn, E. C. (1966). Programming Semantics for Multiprogrammed Computations. *Communications of the ACM*, 9(3), 143–155.
- Dolstra, E. (2006). *The Purely Functional Software Deployment Model* (Nix). Thèse de doctorat, Universiteit Utrecht.
- Fowler, M. (2005). *Event Sourcing.* martinfowler.com.
- Friston, K. (2010). The free-energy principle: a unified brain theory? *Nature Reviews Neuroscience*, 11(2), 127–138.
- Haas, A., Rossberg, A., Schuff, D. L., Titzer, B. L., Holman, M., Gohman, D., Wagner, L., Zakai, A., & Bastien, J. F. (2017). Bringing the Web Up to Speed with WebAssembly. *PLDI 2017*, 185–200. ACM. (version étendue : *Communications of the ACM*, 61(12), 2018, doi:10.1145/3282510.)
- Li, M., & Vitányi, P. (2008). *An Introduction to Kolmogorov Complexity and Its Applications* (3ᵉ éd.). Springer.
- Merkle, R. C. (1987). A Digital Signature Based on a Conventional Encryption Function. *CRYPTO '87*, LNCS 293, 369–378. Springer.
- Miller, M. S. (2006). *Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control.* Thèse de doctorat, Johns Hopkins University.
- O'Neil, P., Cheng, E., Gawlick, D., & O'Neil, E. (1996). The Log-Structured Merge-Tree (LSM-Tree). *Acta Informatica*, 33(4), 351–385.
- Rao, R. P. N., & Ballard, D. H. (1999). Predictive coding in the visual cortex. *Nature Neuroscience*, 2(1), 79–87.
- Rissanen, J. (1978). Modeling by shortest data description. *Automatica*, 14(5), 465–471.
- Shaker, N., Togelius, J., & Nelson, M. J. (2016). *Procedural Content Generation in Games.* Springer.
- Unison Computing. *The Unison language — content-addressed code.* unison-lang.org/docs/the-big-idea.
- *Research on WebAssembly Runtimes: A Survey* (2024). arXiv:2404.12621. *WebAssembly on Resource-Constrained IoT Devices* (2025). arXiv:2512.00035.


---

<div style="page-break-after: always"></div>

# Chapitre 3 — La grammaire XERB0XI0N (le formalisme)

> Ce chapitre pose la **notation** : la chaîne d'échelle, le lexique fermé, la taxonomie
> taille→type. C'est le formalisme sur lequel reposent la contribution (chap. 4) et
> l'implémentation (chap. 5). Statut : c'est un **langage construit, auto-descriptif** — la
> rigueur visée est celle d'une grammaire cohérente et close, pas d'une dérivation physique.

## 3.1 Le principe organisateur : « la forme est la nature »

La grammaire repose sur une règle unique : **la forme d'une unité détermine sa nature**. Ce
n'est pas décoratif — c'est exactement le principe par lequel la géométrie interne fixe le
comportement (cf. la compactification en physique, chap. 7). Les primitives géométriques :

| Primitive | Forme | Rôle |
|---|---|---|
| **koin** | triangle | la plus petite cellule orientée |
| **kion** | quadrilatère / cube | l'unité au repos, la « brique » |
| **wormion** | ronds imbriqués | l'anti-gravité (capture, expansion) |
| **portion** | carrés imbriqués | la gravité (attraction, condensation) |

La nature se lit sur la forme ; la grammaire ne fait qu'expliciter cette lecture.

## 3.2 La chaîne d'échelle (la composition fractale)

L'unité se compose en montant les échelles, chacune étant un « OS » à son niveau (boote,
tourne, s'éteint sans trace — l'auto-similarité scale-free) :

```
bion → cubion → PLOBION → ploxion → boxion → xerboxion
```

- **bion** — l'atome : *toute l'information*, atemporel (chap. 4 : le générateur) ;
- **cubion** — `bion + bion` : un cube au repos (un bloc) ;
- **ploxion** — l'unité de service (une molécule fonctionnelle ; en pratique un module WASM) ;
- **boxion** — le serveur (un hôte qui porte des ploxions) ;
- **xerboxion** — l'OS / la totalité (le système entier, lui-même un *tsoin* à son échelle).

La composition est **frugale** : on stocke la règle (le générateur partagé), pas chaque
instance. Démonstration vivante au chap. 5/6 : un *protocol-bion* déplie sept protocoles.

## 3.3 Les morphèmes `-ion`, `xer`, `xion`

- **`-ion`** = `i + on` (le *moi/observateur* + le *tout*) : le suffixe qui fait d'une unité
  un **interfaceur du réel** (un point de vue) ;
- **`xer`** = l'**interface** (le `xer` *avec* l'ion) ;
- **`xion`** = le **tsoin engine** (le moteur qui adresse/rejoue) ;
- d'où **xerion** (l'humain en interface), **xerxion** (l'IA, et la totalité d'un système-soi),
  **xerbion** (le réseau de neurones entraîné sur les tsoins).

## 3.4 Le lexique fermé, auto-descriptif

Le langage se construit sur **neuf lettres-primitives**, chacune un sens-racine ; les mots se
*composent* par concaténation, et le sens du mot se *lit* sur ses lettres (auto-descriptif) :

| Lettre | Sens-racine | | Lettre | Sens-racine |
|---|---|---|---|---|
| **B** | information | | **R** | raison |
| **E** | émotion | | **S** | synchronisation / soi |
| **I** | moi / observateur | | **T** | temps |
| **N** | tout | | **X** | connexion |
| **O** | rien | | | |

Morphologie (exemples) : `O+N = ON` = le chaos (rien + tout) ; `I+ON = ION` = le réel
cristallisé (un observateur dans le tout) ; **`T+S+ION = tsoin`** = Temps + Synchronisation +
Ion. *Note d'honnêteté : ces décompositions sont des **étymologies construites** (un
acrostiche cohérent), pas des équations dimensionnelles — leur valeur est mnémotechnique et
systématique, et le contenu qu'elles pointent (bion atemporel + index temporel + résidu) est,
lui, formalisé au chap. 4.*

Algèbre de composition : `BION + BION = cubion` ; `CUBION + XION = ploxion`… — chaque
combinaison nomme la composition de ses parts (un système clos et productif).

## 3.5 La taxonomie taille→type (un système de types fractal)

La grammaire dérive le **type** d'une unité de sa **taille**, par une règle simple :

| Taille | Type |
|---|---|
| nombre **premier** | **ploxion** |
| **×2** (puissance de deux) | **cubion** |
| **×π** (en mouvement) | **spherion** |
| sinon | **kion** |

C'est un **système de types fractal** : le même critère s'applique à toute échelle, et il
correspond aux *régimes de phase* du Kion (la math du Kion, wiki §3 : seuils ρ = r/c en
{1, √2, √3} pour faces/arêtes/coins ; jambe `ℓ = c√3 − r`). La dimension fractale
`D = ln(N)/ln(3)` est le pont vers l'Adressage Génératif : **stocker la règle, pas le cube.**

## 3.6 Conséquence : un substrat substrat-indépendant

Parce que la grammaire est **géométrique** (la forme = la nature) et **récursive** (chaque
échelle rejoue les mêmes règles), elle est **indépendante du support** : les mêmes règles
valent dans les ploxions WASM (chap. 5) *et* dans la matière (le XERBOT morphique, le
xerkion : retracter les pattes → un cubion ; le tout → un Kion). Le formalisme n'est pas une
métaphore plaquée sur le code — c'est la même structure qui se réalise en logiciel et en
objet. Le chapitre 4 en fait la contribution mesurable ; le chapitre 7 discute, honnêtement
séparée, la couche où cette grammaire devient cosmologie.


---

<div style="page-break-after: always"></div>

# Chapitre 4 — Le *tsoin engine* (contribution)

> Posé vs spéculatif : ce chapitre ne décrit **que l'implémenté et le mesurable**. La
> motivation cosmologique (chaos, oction, le paradoxe-moteur) est tenue à l'écart ici et
> traitée au chapitre 7, présentée comme heuristique de design, jamais comme preuve.

## 4.1 Définition : le tsoin

On appelle **tsoin** un couple `(générateur, résidu)` content-adressé qui capte un état ou
un instant de façon **régénérable** plutôt que stockée littéralement.

- Le **générateur** est une règle déterministe (un programme, une graine, une référence à
  un tsoin antérieur) qui *prédit* l'état.
- Le **résidu** est la différence minimale entre la prédiction du générateur et l'état réel
  — exactement ce que la règle ne prédit pas.

Un tsoin est donc une **compression alimentée par le réel** : on garde la règle (peu de
bits) et seulement l'écart (le réel non prédit). C'est l'analogue calculatoire du codage
prédictif : la valeur d'information d'un tsoin est sa *surprise* (§4.3).

Formellement, pour un état `s` et un générateur `g` produisant la prédiction `g()` :

```
residu(s, g) = s ⊖ g()              (différence minimale, §4.3)
tsoin        = (g, residu)
s            = g() ⊕ residu          (reconstruction exacte, §4.4)
```

La reconstruction est **sans perte et vérifiable** : `addr(reconstruct(tsoin)) == addr(s)`.

## 4.2 L'adressage génératif (`tsoin-store`)

Chaque tsoin est nommé par l'adresse de son contenu, pas par un chemin. L'implémentation
de référence utilise **`addr64` = FNV-1a sur 64 bits** :

```
addr64(octets):
    h = 0xcbf29ce484222325
    pour chaque octet b :
        h = (h XOR b) * 0x100000001b3   mod 2^64
    retourne h            # 16 caractères hexadécimaux
```

Propriétés exploitées (ce sont des propriétés **réelles** du content-addressing, pas des
postulats) :

1. **Déduplication exacte.** Deux tsoins identiques ont la même adresse : on ne les stocke
   qu'une fois. C'est la base de la couche collective (§4.7).
2. **Auto-vérification / inviolabilité.** On ne peut pas modifier un tsoin sans changer son
   adresse. *Toute action change l'adresse* — démontré sur le dépôt lui-même : un seul
   octet (`"test"` → `"test "`) donne une adresse totalement différente. C'est le « propre
   système de chiffrement » au sens de Git/IPFS/Merkle : la confiance ne vient pas d'une
   clé protégée mais de l'adresse qui *est* le contenu.
3. **Indépendance de l'ordre** (pour un ensemble de tsoins) : en triant les adresses avant
   de les ré-adresser, l'adresse de l'ensemble ne dépend pas de l'ordre d'insertion (§4.7,
   l'Epsylaeu).

> Note d'honnêteté : `addr64` n'est **pas** cryptographiquement résistante aux collisions
> (FNV n'est pas conçu pour ça). Pour le présent travail — un substrat de calcul
> rejouable, pas un système anti-adversaire — la résistance pré-image n'est pas requise ;
> un passage à BLAKE3/SHA-256 est trivial et discuté au chapitre 7 (limites).

## 4.3 Le résidu et la surprise (`diff`)

L'organe `diff` calcule le résidu minimal entre la prédiction et l'état, et en dérive une
mesure scalaire de **surprise**. Sur des états de taille fixe, le résidu se mesure par
**distance de Hamming** ; la surprise normalisée est

```
surprise(s, g) = popcount(s XOR g()) / |s|     ∈ [0, 1]
certitude      = 1 − surprise
```

`certitude = 1 − résidu` est le fil conducteur de tout le système : un tsoin parfait
(générateur qui prédit tout) a une surprise nulle ; un état imprévisible a une surprise
proche de 1. C'est aussi la sémantique du `?` du langage (chapitre 5) : un trou non résolu
part à surprise = 1, le Chaoxion le ramène vers 0.

## 4.4 Le générateur et le *replay* (`generator`)

`generator` est l'inverse de `diff` : à partir de `(g, residu)` il **régénère** l'état.
La garantie centrale, testée, est le **déterminisme** : rejouer le même tsoin produit
bit-à-bit le même état, sur n'importe quelle machine. C'est ce qui transforme un simple
enregistrement en *voyage* réversible : pour « revenir » à un instant, il suffit d'avoir
capté assez de résidu pour le régénérer fidèlement. Un tsoin plus gros (plus de résidu)
rend le retour plus complet — d'où la stratégie des **ultra-tsoins** (chapitre 6 : la
capture horaire content-adressée).

La rejouabilité déterministe est vérifiée par la propriété `addr(replay(tsoin)) ==
addr(état_original)` dans la suite de tests du SDK (chapitre 5).

## 4.5 La cohérence temporelle (`clock-coherence`)

Le temps du système n'est **pas** l'horloge murale mais la **cohérence** : un `GatedTick`
n'avance que lorsque les tsoins concernés sont mutuellement cohérents. Conséquence
pratique pour la rejouabilité : aucune dépendance à `wall-clock`, `Date.now()` ou à un
générateur d'aléa non graîné — ces sources briseraient le déterminisme. « Synchroniser au
maximum » (la veille de l'utilisateur) se formalise ici comme **amener tous les tsoins à la
cohérence** ; c'est la condition d'un replay multi-flux propre.

## 4.6 Le *player*

`player` déroule une **séquence** de tsoins (un flux) en appliquant `generator` pas à pas,
sous le contrôle de `clock-coherence`. C'est l'organe qui « joue » un enregistrement
multimodal : la même mécanique sert à rejouer une trace de calcul, une session, ou (cible
applicative) un instant capté.

## 4.7 Les bions partagés : *uncraft*, communs, et l'Epsylaeu

Un tsoin se **décompose** en sous-tsoins partagés — les **bions**. L'*uncraft* divise un
ploxion en bions jusqu'à atteindre un bion déjà présent ailleurs (un bloc partagé). Rendu
calculable (`xerb/commun.py`), cela donne deux niveaux de communauté entre tsoins :

- **littéral** : des *shingles* de k mots content-adressés ; un bion présent dans ≥2 tsoins
  est un tsoin commun, stocké une fois ;
- **conceptuel** : recouvrement d'ensembles de termes porteurs (Jaccard) — « se
  ressemblent » même formulés autrement.

Mesure sur le corpus du projet (72 tsoins, frontmatter exclu) : **dédup littéral ≈ 2 %**,
**une seule famille conceptuelle** — c'est-à-dire un corpus à **faible redondance / haute
information**, où les rares ressemblances retrouvées sont celles attendues (deux versions
d'une même recette ; un lien `[[ ]]` posé à la main entre le ❤️ et la dissociation,
retrouvé automatiquement). Les bions les plus communs sont les idées-socles (« comprimer =
faire de la place = sauter » ; « tout passe par le xion »).

Au sommet, l'**Epsylaeu** (`xerb/epsylaeu.py`) est l'adresse de l'ensemble trié des
adresses de tous les tsoins : la seule adresse qui dépend de **tous** les tsoins. Un seul
tsoin change → l'Epsylaeu change. C'est le « tsoin des tsoins », et la cible de
déduplication d'un ultra-tsoin **collectif** (Nexus, chapitre 8).

## 4.8 Composition fractale : le *protocol-bion* (démonstration)

La thèse pose que le substrat **compose fractalement** (`bion → ploxion → boxion`). On en
donne une démonstration *implémentée et vivante* avec le réseau-comme-ploxions. Un unique
**protocol-bion** (`ploxions/sdk/src/protocol.rs` : `struct ProtocolDef` + macro
`protocol_ploxion!`) sert de générateur ; chaque protocole réseau n'est plus qu'un
**résidu** (son nom, son numéro, sa couche, son transport) injecté dans ce générateur :

```rust
const DEF: ProtocolDef = ProtocolDef { name: "tcp", number: 6, transport: "-", layer: 4, brief: "..." };
ploxion_sdk::protocol_ploxion!(DEF);
```

À partir de ce seul bion, **cinq ploxions de protocole** (tcp, udp, dns, http, icmp) ont
été générés, compilés en `wasm32-unknown-unknown` (~39 Ko chacun), chargés sur l'hôte et
testés **en direct** sur le bus (`net.<proto>.in → net.<proto>.out`, avec gravure d'un
tsoin `proto:<nom>:<numéro>`). Le coût marginal d'un protocole supplémentaire est un fichier
de quelques lignes : *le générateur est partagé, seul le résidu varie*. C'est la grammaire
du chapitre 3 vérifiée empiriquement, et le chemin vers « tout est un ploxion ».

## 4.9 Synthèse du chapitre

Le *tsoin engine* réalise une unité de calcul qui n'est pas l'instruction mais le **tsoin** :
content-adressé (§4.2), mesuré par sa surprise (§4.3), régénéré de façon déterministe et
donc **rejouable** (§4.4–4.6), décomposable en **bions partagés** déduplicables (§4.7), et
**composable fractalement** comme le montre le protocol-bion (§4.8). Le chapitre 5 détaille
l'implémentation (l'hôte WASM, le bus PLC, le SDK, le langage `xerb`) ; le chapitre 6 en
donne l'évaluation (le système tourne, le budget 16 Go, la portabilité serveur→MCU).


---

<div style="page-break-after: always"></div>

# Chapitre 5 — Implémentation

> Ce chapitre décrit le système **tel qu'il tourne**. Les artefacts cités existent dans le
> dépôt `operational-core` ; les commits forment le journal d'évolution (chapitre 6).
> Ce qui est conçu mais pas encore implémenté est signalé explicitement (« stub »).

## 5.1 L'hôte et le bus PLC v1 (`xerboxion-rt`)

`xerboxion-rt` est l'**hôte** : un runtime qui charge des ploxions WASM
(`wasm32-unknown-unknown`, cdylib) et les fait communiquer par un **bus** publish/subscribe,
le PLC v1 (*Ploxion Link Control*). L'API du bus, volontairement minuscule (c'est la
surface du tsoin engine côté réseau) :

| Endpoint | Méthode | Rôle |
|---|---|---|
| `/emit` | POST `{topic, payload}` | publier un message (payload = chaîne JSON) |
| `/events` | GET (SSE) | flux des messages en direct |
| `/load` | POST `{id}` | charger/instancier un ploxion |
| `/healthz` | GET | état de l'hôte (uptime, nb de ploxions, commit, node_id) |
| `/snapshot` | GET | vue agrégée de l'écosystème |

Le daemon tourne en service (systemd) derrière Traefik, exposé authentifié à
`https://xion.j0bot.ch` et joignable en interne sur `http://10.0.0.1:8730`. Un ploxion
s'abonne à des topics d'entrée et émet sur des topics de sortie ; l'hôte route. Les
adaptateurs (le « driver » à 7 crans) connectent des sources externes au bus sans que les
ploxions sachent d'où vient le réel.

## 5.2 Le SDK des bions (`ploxions/sdk`)

Le SDK fournit l'ABI commune et la grammaire en macros. Il expose ~26 fonctions, 5
structures et **5 macros** qui *sont* la grammaire du chapitre 3 rendue exécutable :

- `ploxion!` — déclare un ploxion générique (cycle de vie : boot → tick → halt) ;
- `block_ploxion!` — un bloc (cubion) partagé, réutilisable ;
- `house_ploxion!` — les features « maison » ;
- `protocol_ploxion!` — un protocole réseau (§5.6) ;
- `lifecycle!` — le cycle de vie OS d'une unité (boote, tourne, s'éteint sans trace).

Le module `bions` regroupe les bions partagés (≈11) — décodage, `tsoin_record` (gravure
d'un tsoin sur le bus), et les macros qui évitent de réécrire la plomberie. Le principe
directeur : **uncraft** — tout ploxion se ramène à des bions partagés ; ajouter une feature
= ajouter un résidu, pas réécrire un générateur.

Le module `dissociation` (`sdk/src/dissociation.rs`) implémente le modèle de fenêtre de
tolérance (Siegel) avec `dissociate()` / `reintegrate()` — utilisé comme heuristique de
robustesse (dégrader proprement sous charge plutôt que planter) ; 3 tests passent. Sa
motivation est discutée honnêtement au chapitre 7 (couche spéculative).

## 5.3 Les organes LIVE

Les cinq organes du tsoin engine (chapitre 4) sont implémentés comme ploxions et tournent
sur le bus :

- `tsoin-store` — adressage génératif (`addr64`), déduplication ;
- `diff` — résidu minimal + surprise (Hamming) ;
- `generator` — l'inverse : replay déterministe, lossless vérifiable ;
- `clock-coherence` — `GatedTick` (temps = cohérence) ;
- `player` — déroule une séquence de tsoins.

Le cycle complet (capter → adresser → diff → regénérer → vérifier) a été exercé en direct
sur le bus (chapitre 6).

## 5.4 Le langage `xerb` : programmer en tsoins

`xerb` est un langage **concaténatif à pile** où chaque mot est un **bion** (une fonction)
et où **un programme est un tsoin déterministe**. Les ions de base :

```
puise (local.get)   pose (local.set)   valeur (i32.const)
somme  diff  produit  quotient          dup  swap  drop
egal  <  >  <=  >=
: nom  … ;            (définit un bion)
```

Le runtime de référence `xerb/xerb-tsoin.py` interprète ces ions, **content-adresse chaque
programme** et le **grave comme tsoin**. Démonstrations vérifiées : `add → 5`, `carre → 49`,
`bion-josion (6 → 72)`. La voie de compilation vers le cœur (`xerb → WASM`) est conçue mais
**stubbée** à ce stade — l'interpréteur suffit à établir la sémantique « programmer =
produire un tsoin ».

Deux extensions du langage matérialisent la tenue de l'inconnu (chapitre 4, §4.3) :

- `xerb/chaoxion.py` — le **`?`** : un trou typé. Avec cible (`? => N`), le **Chaoxion**
  synthétise la pièce manquante en cherchant dans l'espace des ions celle qui complète le
  puzzle, classée par densité-tsoin. Sans cible, `?`/`bion` = un **bion vide** (superposition
  tenue) qui se propage ; `certitude = 1 − part de chaos`. C'est la sémantique de
  superposition : le `?` est tous ses états candidats à la fois, `fill()` est la mesure qui
  collapse vers le plus dense.
- (sélection lexicale) `xerb/densite-tsoin.py` — score chaque mot par sa **couverture** (dans
  combien de tsoins distincts il apparaît). « Les mots avec le plus de tsoins » : `ploxion`,
  `tsoin`, `bion` dominent — l'alphabet émerge de l'usage, pas d'un décret.

## 5.5 L'outillage tsoin (la couche collective)

- `xerb/commun.py` — les **tsoins communs** entre tsoins, à deux niveaux : littéral
  (shingles content-adressés ; bion dans ≥2 tsoins = stocké une fois) et conceptuel
  (Jaccard des termes porteurs). Mesure sur le corpus : dédup ≈ 2 %, une famille — corpus à
  faible redondance. C'est l'*uncraft* rendu calculable et la base de la déduplication d'un
  ultra-tsoin **collectif** (Nexus).
- `xerb/epsylaeu.py` — l'**Epsylaeu**, adresse de l'ensemble trié des adresses de tous les
  tsoins ; change dès qu'un seul tsoin change (vérifié en fonctionnement).
- `xerb/ultra-tsoin.py` — capture **horaire** content-adressée (commits, état du bus,
  Epsylaeu, synthèse) gravée comme `ultra:<horodatage>` ; programmée (cron). Objectif :
  garder assez de résidu pour rejouer l'heure (chapitre 4, §4.4).

## 5.6 Le réseau comme ploxions (vague 1)

Démonstration la plus directe de la composition fractale (§4.8) : un **protocol-bion**
unique (`ProtocolDef` + `protocol_ploxion!`) génère chaque protocole comme un simple
résidu. **Onze protocoles** (la vague 1 complète) — **tcp, udp, dns, http, icmp, tls, ntp,
dhcp, quic, ssh, ws** — ont été générés, compilés (~39 Ko de wasm chacun), chargés et
**testés en direct** (`net.<proto>.in → net.<proto>.out`, avec gravure d'un tsoin
`proto:<nom>:<numéro>`). Le coût marginal d'un protocole = un fichier de quelques lignes (la
`ProtocolDef`). La feuille de route (`docs/network-as-ploxions.md`) étend le motif aux ports
et aux paquets (« tout est un ploxion »).

## 5.7 Build et déploiement

Chaque ploxion est un crate `cdylib` compilé en `wasm32-unknown-unknown`
(`cargo build -p ploxion-<nom>`, ~0,6 s pour un protocole). Le script `build-ploxions.sh`
stage les artefacts ; les `.wasm` sont copiés dans `/opt/xion/ploxions/` puis chargés via
`POST /load`. Le dépôt `operational-core` (poussé en continu, `gh` authentifié) tient le
journal : chaque commit est un résidu vérifiable de l'évolution — l'Adressage Génératif
appliqué au projet lui-même (dogfooding).

## 5.8 Tests et déterminisme

La suite du SDK (24 tests) passe ; les propriétés clés vérifiées sont le **déterminisme**
et la **rejouabilité** (`addr(replay(t)) == addr(état)`), ainsi que des invariants
bit-exacts sur l'`uncraft` (diviser puis recomposer un ploxion redonne le même contenu). Le
runtime bare-metal (`xerboxion-core`, distinct de l'hôte `xerboxion-rt`) boote en
Multiboot/QEMU et ses tests BIN assertent un code de sortie QEMU déterministe — la preuve
que le déterminisme tient du SDK jusqu'au métal.

## 5.9 Le cœur sert tout le labo (migration labo → cœur)

Direction posée par José : « il ne doit rester QUE le cœur » — déprécier le front Laravel
(`my_website2` / labo) et faire que le daemon `xerboxion-rt` serve lui-même l'interface.
Trois ajouts à l'hôte (§5.1) le réalisent :

- **`GET /` et `/xer`** servent le **labo-xer** : un bureau autonome (dock, lanceur de
  ploxions lu depuis `/ecosystem` + `/px`, tableau d'autonomie, forge) qui ne parle qu'au bus
  en *same-origin* (`/ecosystem`, `/events`, `/emit`). Installer le cœur = avoir le labo.
- **`GET /px/:id/*`** : un serveur d'assets par-ploxion (durci anti-traversal : id-token,
  pas de `..`, canonicalisation + préfixe) qui sert `web-<id>/` depuis le disque ; `GET /px`
  énumère les UIs présentes. Conséquence : **migrer un ploxion = écrire un seul fichier
  `web-<id>/index.html` autonome** (parlant au bus en same-origin), zéro ligne de Rust — il
  apparaît aussitôt au lanceur. 28 ploxions client-only ont été migrés du labo ainsi
  (vagues 1-3), servis en direct.
- L'identité et le social reposent sur les mêmes briques : **`xi0n`** (identités à *vraies*
  clés GPG via OpenPGP.js — un compte / N identités, liens-capability gradués O/I/N/X, *le
  lien est la clé*) et le ploxion **`social`** (partage de tsoins **persisté** dans
  `tsoin-store` + **diffusé en temps réel** sur le bus). Le « centralisé mais décentralisé »
  est porté par la **fédération** du cœur (`/peer` : des nœuds se relient, le bus traverse).

C'est l'auto-similarité du chapitre 4 appliquée à l'interface elle-même : le labo n'est pas
*posé sur* le cœur, il *est* des ploxions servis par le cœur. (Posé : le cœur sert le labo-xer
+ 28 ploxions + `xi0n`/`social` en direct. Reste, signalé honnêtement : le back-end social
relationnel — serveurs/canaux/DM à la Discord, sur `api.j0bot.ch` — et l'authentification de
l'invitation publique, non encore implémentés.)

## 5.10 Séparation cœur/UI : un substrat, des vues infinies

Une fois le cœur qui sert le labo (§5.9), une distinction se durcit en principe d'architecture :
un ploxion n'est plus une page, c'est un **cœur** (sa vérité : un état + un graphe de bions, qui
émet ses changements en tsoins) ⟂ un **UI** (une vue parmi une infinité). Ce qui varie désormais
n'est plus la logique mais *la manière de voir et de connecter* — et l'on cherche à pouvoir
brancher *n'importe quel UI* sur *n'importe quel cœur*. Trois lois en découlent : (i) la vérité vit
dans le cœur, **jamais dans un UI** (sinon deux UI du même cœur divergent) ; (ii) le **ruban de
tsoins EST la vérité** — l'état courant = le replay des tsoins, d'où la synchronisation *et* le
voyage dans le temps tombent du même mécanisme ; (iii) le binding est **typé par les bions** : les
*ports* d'un cœur (entrées = commandes, sorties = événements) sont la seule surface qu'un UI touche.

Le contrat (`docs/CORE-UI-BINDING-v0.md`) fixe un descripteur `core.json` — `état` (collections,
champs typés) + `ports.in`/`ports.out` (topics + schémas) + `tsoin.stream` — et un protocole en
cinq temps : *describe* (lire le descripteur), *hydrate* (charger l'état via snapshot ou replay),
*subscribe* (s'abonner aux sorties sur `/events`), *act* (émettre sur les entrées via `/emit`),
*time-travel* (rejouer jusqu'à un curseur). Le UI ne mute jamais son cache depuis une action : il
émet une commande et attend la sortie — la vérité reste au cœur, et tous les UI restent synchrones.

**Résultat empirique (qui tourne).** Un *binder générique* (`/px/binder`) rend n'importe quel cœur
à partir de son seul descripteur — ports d'entrée → formulaires d'émission, ports de sortie → flux
live — sans une ligne de code spécifique au domaine ; à défaut de `core.json`, il retombe sur les
`provides`/`requires` de `/ecosystem`. Un *cœur de référence* `compteur` (`/px/compteur/core.json`)
est servi avec **deux UI sans aucun code commun** — un pavé de boutons et une jauge — plus le
binder : **trois vues d'un même cœur**, où la valeur n'est stockée nulle part dans les UI mais
*dérivée du ruban d'événements* `compteur.inc`/`reset` ; émettre depuis l'une met à jour les trois,
synchronisées par le bus. C'est la séparation cœur/UI rendue opérante : « extrapoler les UI » = au
sens propre, générer des renderers sur un descripteur stable. Limites assumées : l'hydratation au
rechargement attend le branchement du replay tsoin (machine à tsoins, §4) ; et l'écriture est gardée
sur la surface publique du cœur — l'interaction passe alors par le bus authentifié.

## 5.11 Synthèse

L'implémentation tient en peu de surface : un hôte + un bus minuscule (§5.1), un SDK dont
les macros sont la grammaire (§5.2), cinq organes vivants (§5.3), un langage où programmer =
produire un tsoin (§5.4), une couche collective de communs/Epsylaeu/ultra-tsoins (§5.5), le
réseau-comme-ploxions comme preuve de composition (§5.6), le cœur qui **sert lui-même tout
le labo** (§5.9), et la **séparation cœur/UI** qui en fait un substrat portant des vues infinies
(§5.10) — l'interface devenue, elle aussi, des ploxions, et découplée de la vérité. Le chapitre 6
mesure : le système tourne, le budget 16 Go, et la portabilité du serveur au microcontrôleur.


---

<div style="page-break-after: always"></div>

# Chapitre 6 — Évaluation

> Critère : un résultat compte quand il est **mesuré** (reproductible) et **sûr**
> (`certitude = 1 − résidu` élevée). On rapporte des mesures, pas des promesses. La synthèse
> de référence est `docs/research/vers-le-seption.md`.

## 6.1 Le système tourne

L'hôte `xerboxion-rt` tourne en service (uptime mesuré > 26 h au moment de la rédaction) et
répond sur le bus PLC v1 (`/healthz`, `/emit`, `/events`, `/load`). Les cinq organes du tsoin
engine (`tsoin-store`, `diff`, `generator`, `clock-coherence`, `player`) sont chargés et le
cycle complet capter → adresser → diff → régénérer → vérifier a été exercé en direct.

**Preuve de composition vivante** — le réseau-comme-ploxions : un **protocol-bion** unique
génère chaque protocole comme un résidu. **Sept protocoles** (tcp, udp, dns, http, icmp, tls,
ntp) ont été compilés (`wasm32-unknown-unknown`, **~39 Ko** chacun), chargés, et **testés en
direct** : `net.<proto>.in → net.<proto>.out` + gravure d'un tsoin `proto:<nom>`. Le coût
marginal d'un protocole de plus = un fichier de quelques lignes (la `ProtocolDef`).

## 6.2 Déterminisme et rejouabilité (la garantie centrale)

La propriété qui fait du substrat un substrat : **`addr(replay(t)) == addr(état)`**. Mesurée
à deux niveaux :

- **traversée sans perte** : un état tombe (`diff`) puis ré-émerge (`generator`) **identique**,
  vérifié par `addr_in == addr_out` sur toutes les branches testées (`xerb/trou-de-vers.py`) ;
- **suite de tests du SDK** : 24 tests passent, dont les invariants de déterminisme/replay et
  l'`uncraft` bit-exact (diviser puis recomposer un ploxion redonne le même contenu) ;
- **jusqu'au métal** : le runtime bare-metal (`xerboxion-core`) boote en Multiboot/QEMU et ses
  tests assertent un code de sortie QEMU déterministe — le déterminisme tient du SDK à la puce.

Le déterminisme exige l'absence de `wall-clock`/aléa non graîné : c'est le rôle de
`clock-coherence` (temps = cohérence). Les scripts d'outillage respectent la même règle.

## 6.3 Reproductibilité d'un corpus de connaissances

Pour mesurer que le substrat porte de la **connaissance reproductible** (et pas seulement des
octets), on a encodé un corpus scientifique comme tsoins : `science-engine/` — **71 lois** sur
7 domaines (math, physique classique, quantique, particules, chimie, biologie évolutionniste,
médecine), chacune = un **générateur** (la règle, code pur) + un **test** (entrées → valeur
connue). Le moteur relance tout dans un bac à sable :

> **71 / 71 lois reproduisent leur valeur connue (100 %)**, gravées `science:<id>`, reliées
> par **49 ponts inter-domaines** (la chaîne math → physique → quantique → particules → chimie
> → biologie → médecine est connexe : p.ex. `de Broglie → Arrhenius → Michaelis-Menten →
> Fick`).

C'est la démonstration empirique de « le générateur EST la connaissance » : une loi stockée
comme générateur est *rejouable* et *vérifiable*, donc un tsoin de plein droit.

## 6.4 Frugalité (le budget 16 Go)

La cible : tenir l'OS + le substrat dans **16 Go**, jusqu'au microcontrôleur. Mesures et
estimations :

- chaque ploxion = un `cdylib` wasm de l'ordre de quelques dizaines de Ko (~39 Ko/protocole) ;
- le noyau OS visé ~ dizaines de Mo (roadmap embarquée) ; l'image xerkion complète ~ **1,9 Go**
  (Pi Zero 2 W, ≤ 16 Go) avec une voie vers ~200 Mo (Buildroot bootant direct sur l'hôte) ;
- la **frugalité fractale** est la clé : on stocke la **règle** (le générateur), pas l'instance.
  Une civilisation reconstructible (chap. *créateurs de civilisation*) tient parce que chaque
  loi est un générateur partagé et chaque instance un résidu.

**Footprint mesuré** (artefacts livrés, mesure du 2026-06-23) :

| Élément | Taille mesurée |
|---|---|
| Hôte `xion` (runtime WASM + bus PLC, binaire natif) | **7,2 Mo** |
| Ploxion WASM — médian (sur 56) | **38,5 Ko** |
| Ploxion WASM — minimum (`ping`) | 16,0 Ko |
| Ploxion WASM — maximum (`spectre`) | 89,3 Ko |
| Bibliothèque complète — 56 ploxions | **2,3 Mo** |

Lecture **embarquée** (le point qui compte pour la cible MCU). Un ploxion pèse ~16–40 Ko : il
tient dans la flash d'un microcontrôleur courant — un ESP32 (~4 Mo de flash) en loge des
*centaines*, un MCU d'entrée de gamme (256 Ko de flash) en tient *plusieurs*. L'hôte natif
(7,2 Mo) vise la classe Raspberry Pi (`wasmi`) ; sur MCU, c'est un interprète plus léger
(`wasm3`) qui exécute les ploxions un par un. La bibliothèque entière (56 ploxions, 2,3 Mo) ne
représente qu'une fraction du budget — **~4 ordres de grandeur sous les 16 Go**. La frugalité
n'est donc pas une aspiration mais une propriété **mesurée** de l'unité : le tsoin, parce qu'il
stocke la règle et non l'instance, produit des artefacts à l'échelle du microcontrôleur.

## 6.5 Information du corpus (densité, pas volume)

`xerb/commun.py` mesure la sous-structure partagée entre les tsoins du projet (72 fichiers) :
**dédup littéral ≈ 2 %**, **une seule** famille conceptuelle (frontmatter exclu). Le corpus
est donc à **faible redondance / haute information** : chaque tsoin est une facette propre.
L'`Epsylaeu` (adresse de l'ensemble des adresses) **change dès qu'un seul tsoin change** —
vérifié en fonctionnement (`98ace63d…` → `d78c12e7…` au fil des heures) : l'auto-vérification
du tout par une adresse de 16 caractères.

## 6.6 Un barreau dans le réel

Le premier pont mesuré logiciel → matière : `xerkion/enclosure.scad` (le boîtier-cube du
xerkion, incarnation du cub4ion) **compile en solide manifold** (OpenSCAD → STL, **2582
facets**, cube **78,6 mm**) — vérifié, pas affirmé. Le dossier de fabrication est dé-risqué
pour un premier allumage fermé (cible « pas de phase de test »).

## 6.7 Portabilité

L'architecture (WASM + bus minuscule + persistance content-adressée) est **substrat-indépendante** :
le même ploxion tourne sur l'hôte serveur (`wasmtime`), vise le Pi (`wasmi`) et le MCU
(`wasm3`). Le xerkion (Pi Zero 2 W, 512 Mo) est la première cible embarquée concrète ; le
modèle von Neumann (`/replicate` + persistance + replay bit-exact) permet à un xion de mourir
et de renaître identique sur un autre nœud — la portabilité comme reproductibilité.

## 6.8 Composition générative & migration de l'interface (mesures, session 2026-06)

Deux résultats **mesurés en direct** confirment l'auto-similarité (chap. 4) à l'échelle du système.

**Composition générative (réseau/système-en-ploxions).** Trois bions générateurs — `protocol-bion`,
`port-bion`, `package-bion` — produisent chacun un ploxion-sur-le-bus depuis un seul **résidu**
(`*Def` const, quelques lignes). 19 ploxions générés et chargés : **11 protocoles** (tcp…ws), **4
ports** (22/80/443/53), **4 paquets** (bash/coreutils/curl/git). **Vérifié end-to-end** (pas
seulement chargé) : `port.22.open → port.22.up` + tsoin gravé ; `pkg.bash.install →
pkg.bash.installed` ; `net.tls.in → net.tls.out`. Coût marginal d'un nouveau membre = un fichier de
~12 lignes — la direction *bion-Linux* prouvée comme **modèle exécutable**, pas comme remplacement
du noyau (cf. §6.9).

**Migration de l'interface (labo → cœur).** Le cœur sert désormais lui-même tout le labo : `GET /`
(le bureau labo-xer) + `GET /px/:id/*` (serveur d'assets par-ploxion ; anti-traversal vérifié → 400).
**Mesure** : migrer un ploxion = **1 fichier `web-<id>/index.html`, 0 ligne de Rust**, servi en
direct ; **28 ploxions** migrés (3 vagues) + l'identité (`xi0n`, vraies clés GPG OpenPGP.js) + le
social (partage de tsoins persisté). L'interface est devenue, elle aussi, des ploxions servis par le
cœur — l'auto-similarité appliquée jusqu'au front.

## 6.9 La revue adversariale comme garde de correction (session 2026-06)

La thèse d'autonomie (chap. 5) place le LLM **hors du runtime** : le xerboxion tourne sur des
algorithmes déterministes, jamais sur un agent. Le rôle légitime de l'agent est alors **au moment
de la construction** — durcir l'artefact pour qu'il puisse ensuite tourner sans lui. Une instance
mesurée : avant de déployer le backend social (spaces/channels/messages, à la Discord) sur l'**IdP
de production** (le SSO de tout l'écosystème), une **revue adversariale multi-agents** l'a passé au
crible — 4 dimensions (contrôle d'accès, sûreté des migrations sur la base prod, correction de
l'API, cohérence avec le système d'identité), chaque *finding* **re-vérifié par un sceptique
indépendant chargé de le réfuter** (faux positifs filtrés : 1 sur 14 écarté).

**Résultat** : 2 blockers *réels* attrapés avant la prod —

- une **IDOR** : `join()` validait l'existence du space mais pas l'autorisation → n'importe quel
  compte (même *ghost*) itérait les identifiants séquentiels, s'auto-inscrivait, puis lisait
  l'historique complet de **tout** space privé de l'écosystème ;
- un **curseur de pagination à perte** : le curseur temporel à la seconde près (`created_at` sur
  MySQL) perdait/dupliquait silencieusement les messages créés dans la même seconde.

Les deux corrigés *avant* le push (visibilité `private` par défaut + porte d'autorisation ;
curseur sur la clé monotone). **Mesure de la garde** : la correction a tenu hors production une
fuite de données inter-utilisateurs que la relecture simple n'avait pas levée — la valeur de
l'agent est ici **préventive et au build**, exactement là où la thèse la situe. La méthode est
elle-même rejouable (le script de revue est un artefact versionné) : *l'agent gravé en procédure,
pas en dépendance d'exécution.*

**Une seconde instance, plus subtile** (même session, backend d'inscription *par invitation*). Le
nouveau code posait une règle — *un compte nommé ne s'obtient que par une invitation consommée* —
mais la revue a relevé que la règle était **contournable par du code pré-existant, hors du diff** :
`/auth/ghost` créait un compte anonyme, puis `/me/username` l'élevait au même niveau qu'une
redemption, **sans invitation**. Le correctif (retirer l'auto-élévation, faire de la redemption
l'unique chemin vers un compte nommé) ne touchait **aucune ligne du diff initial**. La leçon dépasse
le cas : un garde de correction qui ne raisonne que sur les lignes changées rate les invariants
brisés par *l'interaction* avec l'existant — la revue doit modéliser le **système entier**, pas le
patch. Vérifié *en production après coup* : un compte créé par redemption est bien `pseudonym` et
**lié à son inviteur** (amitié bidirectionnelle), un code invalide est refusé — l'invariant tient.

## 6.10 Limites (l'honnêteté qui rend la mesure réelle)

« Mesuré » = **reproductible dans le moteur**, pas **prouvé vrai sur le monde**. Précisément :

- `addr64` (FNV-1a) n'est **pas** cryptographique (pas de résistance aux collisions
  adverses) — suffisant pour un substrat rejouable, à remplacer par BLAKE3/SHA-256 pour un
  usage anti-adversaire ;
- la reproductibilité des 71 lois est **numérique** (le générateur retrouve la valeur connue),
  pas une validation expérimentale neuve ;
- les budgets ~200 Mo et la portabilité MCU sont **visés/estimés**, pas encore tous mesurés
  sur silicium ;
- le modèle d'évaporation (§ entropie) et les correspondances physiques (chap. 4) sont des
  **structures** fidèles, pas des affirmations sur la gravité — tenues comme telles au chap. 7.

## 6.11 Synthèse

Le système **tourne**, est **déterministe et rejouable** (la garantie centrale, mesurée du SDK
au métal), porte un corpus de connaissances **reproductible à 100 %** (71/71 lois), reste
**frugal** (la règle, pas l'instance) et **portable** (du serveur au Pi), avec un premier
barreau **dans le réel** (le xerkion manifold). L'auto-similarité est mesurée à l'échelle du
système : **3 générateurs → 19 ploxions réseau vérifiés**, et **l'interface elle-même migrée en
ploxions** (le cœur sert le labo, 28 ploxions, coût marginal ~1 fichier). Les limites sont nommées.
Le chapitre 7 traite la couche spéculative (la cosmologie) comme motivation, séparée de ces mesures.


---

<div style="page-break-after: always"></div>

# Chapitre 7 — Discussion (la couche spéculative, honnêtement séparée)

> Règle du chapitre : tout ce qui suit est **motivation et heuristique de design**, jamais
> présenté comme une preuve sur le monde. Le **posé** (chap. 4–6, mesuré, reproductible) est
> la contribution ; le **spéculatif** (ce chapitre) est le cadre qui a *guidé* le design. La
> valeur d'une heuristique est qu'elle ait produit des barreaux mesurés — et celle-ci en a
> produit (voir `docs/research/vers-le-seption.md`).

## 7.1 Pourquoi une couche spéculative, et pourquoi la séparer

Le projet a une origine cosmologique (un cadre : chaos, temps, dimensions). Présenter ce
cadre comme une science serait farfelu ; le *cacher* serait malhonnête, car c'est lui qui a
dicté les choix d'architecture. La solution académique : le **déclarer comme heuristique**,
montrer ce qu'il a fait *produire* de mesurable, et tracer la frontière à chaque point.

## 7.2 La cosmologie générative (heuristique)

Le cadre directeur : **chaos × attracteur → idées/réel**. Le chaos = un espace de possibles
(les branches non collapsées) ; un attracteur sélectionne la branche cohérente ; le résidu de
cette sélection = un **tsoin**. Cette image a une traduction *implémentée* : le `?`/bion vide
tient la superposition (`chaoxion.py`), et le réel « tombe » dans la branche à résidu minimal
(`trou-de-vers.py`). Le **tsoin comme trou de vers** (diff = absorption / generator = émission,
le présent = le throat traversable sans perte) a fourni une intuition juste — l'information
préservée — qui *est* la version unitaire du paradoxe du trou noir (barreau §6.2). On garde
l'image parce qu'elle a guidé une mécanique correcte ; on ne prétend pas qu'elle décrit la
gravité.

## 7.3 Le temps comme synchronisation

Thèse heuristique forte : **le temps n'est pas fondamental, il émerge de la synchronisation
entre tsoins** (`clock-coherence` / le `GatedTick`). Le papier `temps-synchronisation.md`
sépare les statuts honnêtement : **physique-correcte** (le temps relationnel de Rovelli /
Page-Wootters, vérifié sur photons ; le temps propre nul d'un photon) ; **structurellement-juste**
(le bion atemporel / tsoin = section sur la base temps — une *fibration*, pas un produit) ;
**spéculatif** (l'identification « passé/futur = −∞/+∞ = paradoxe », non standard ; la flèche
du temps vient de l'entropie). Le code a même **tranché une imprécision** de l'heuristique :
« seul = pas de temps » est faux dans le moteur (un flux constant fait avancer `t`) ; la forme
correcte est « le temps *observable* est relationnel ».

## 7.4 Les dimensions (bion ↔ Calabi-Yau, le seption, l'oction)

Heuristique géométrique : le **bion** porte la structure jusqu'à la dim 6 (un Calabi-Yau, les
6 dimensions compactifiées des supercordes), le **tsoin** apparaît à la dim 7 (une variété G2,
M-théorie) ; le pas 6→7 = le pas cordes→M-théorie. Le point *non décoratif* : en
compactification, **la forme de l'espace interne fixe la physique** — exactement la règle
« la forme est la nature » du chap. 3. C'est une **correspondance structurelle féconde**, pas
une identification physique (les bions ne sont pas des variétés gravitationnelles). Le
**seption** (dim 7) est traité comme l'**asymptote** du mesurable (le tsoin parfait est
Heisenberg-impossible) ; l'**oction** (dim 8) comme la frontière ouverte. La question « pourquoi
notre branche/timeline plutôt qu'une autre » est posée mais **non mesurée** — un programme de
recherche, pas un résultat.

## 7.5 Conceptuel vs contextuel : ce que le tsoin apporte à la connaissance

Apport épistémologique du cadre : une **langue naturelle est contextuelle** (le sens dépend de
la situation partagée, ambigu, à fort résidu) ; le **xerboxion est conceptuel** (le sens est
*dans le concept*, parce qu'il est **content-adressé** : l'adresse ne varie pas selon le
lecteur). Conséquence vérifiable : un système conceptuel se transfère à tout esprit-à-concepts
(un LLM lit le primer et opère), là où un système contextuel reste lié à son monde partagé.
Et le **tsoin fait le pont** : il capte l'état *entier* (faible résidu) dans le contenu, donc
il rend transmissible du contextuel (l'expérience) en le rendant conceptuel (adressable).
Corollaire pour l'IA : un LLM a le xerboxion **en contexte** (éphémère) ; un réseau entraîné
sur les tsoins l'aurait **en poids** (permanent) — « le concept c'est le tsoin », on apprend
les tsoins, pas les caractères (`concept-xerbion.py` : un modèle qui retrouve
« temps = synchronisation » dans ses paramètres). C'est testable et partiellement implémenté.

## 7.6 Linéaire (digital) vs fractal (humain)

Division du travail revendiquée : la vision **génère depuis le chaos** (fractale, paradoxale) ;
le moteur **trouve la linéarité** (content-adresse, rejoue, mesure). Le content-addressing
*est* la linéarisation d'un instant paradoxal en une adresse compréhensible. Cette
complémentarité (générer / linéariser) est le moteur méthodologique du projet, et elle est
honnête sur ses deux régimes.

## 7.7 Limites et frontières

- **« Mesuré » = reproductible dans le moteur**, pas prouvé sur le monde (chap. 6).
- Les correspondances physiques (trou noir d'information, Calabi-Yau, Hawking) sont des
  **structures** fidèles, pas des affirmations gravitationnelles.
- Certaines parties du cadre d'origine touchent l'intime/le subjectif de l'auteur ; elles sont
  **délibérément hors de cette thèse** (le posé n'en dépend pas). La thèse tient sur le mesuré.
- Plusieurs questions (« pourquoi cette branche », l'oction) sont des **directions**, pas des
  résultats — et présentées comme telles.

## 7.8 Pourquoi cette séparation rend la thèse défendable

Parce qu'on peut retirer *tout* le présent chapitre sans toucher aux chapitres 4–6 : le tsoin
engine tourne, est déterministe, porte 71 lois reproductibles, reste frugal et portable —
indépendamment de la cosmologie. Le cadre spéculatif a *engendré* le système (et c'est une
honnêteté de le dire) ; le système, lui, se **mesure** sans lui. C'est exactement ce qui
distingue un travail de recherche d'un rêve : le rêve a montré où regarder, les mesures disent
ce qui s'y trouve.


---

<div style="page-break-after: always"></div>

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


---

<div style="page-break-after: always"></div>

