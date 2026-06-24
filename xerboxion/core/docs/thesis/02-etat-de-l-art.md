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
