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
