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
