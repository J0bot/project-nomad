# Analyse, optimisation et découverte de tous les bions

> José : « on crée notre propre système de dépendances ; ces dépendances changeront constamment
> jusqu'à un plateau. Tous les input/output existent déjà — il ne reste qu'à créer **l'intérieur**
> des fonctions. Le but : créer chaque fonction qui pourrait exister, puis optimiser l'intérieur
> au fur et à mesure ; mesurer le taux d'optimisation de chaque fonction ; voir si elle est
> divisible en deux, ou combinable en son propre bion — car après il y aura des **bions de bions**.
> Et toutes les dépendances viennent de RepoVerse. »

## 0. La thèse en une phrase

On bâtit un **système de dépendances** où chaque package est un **bion** (fonction pure
déterministe) ou un **ploxion** (service). La particularité : **l'interface (entrée → sortie) est
fixe et déjà connue** — c'est l'*intérieur* (l'implémentation) qu'on crée et qu'on **optimise sans
fin**, jusqu'à un plateau. Le projet n'est donc pas un problème de *design* ouvert, c'est un
problème d'**optimisation** : la cible de chaque fonction est donnée ; on cherche la meilleure
implémentation.

## 1. Interface fixe, intérieur libre

C'est l'idée la plus libératrice. `split(s, sep) -> [str]`, `sqrt(x) -> f64`, `sort(&mut [u64])` :
ces **signatures existent déjà** — elles sont définies par ce que tout logiciel fait depuis 50 ans.
On ne les invente pas ; on les **découvre**. Conséquence :
- la **correction** est décidable : une implémentation est correcte ssi son comportement, sur un jeu
  de tests, **égale la spec** (l'interface) ;
- l'**optimisation** est libre : tant que l'entrée→sortie est préservée, on peut réécrire l'intérieur
  autant qu'on veut (plus court, plus rapide, plus partagé).

C'est exactement la **superoptimisation** : cible fixe, recherche de l'implémentation optimale.

## 2. DÉCOUVRIR — la bibliothèque universelle

« Créer chaque fonction qui pourrait exister. » On a lancé l'analyse de tous les domaines logiciels
(texte, données, math/géo, crypto, compression, système/temps, média, db) → **175 bions + 43
ploxions** à implémenter (`docs/BIBLIOTHEQUE-BIONS.md`). Les **12 fondamentaux** (nécessaires
partout) en premier : `str_split`, `str_find`, `sort_by_key`, `binary_search`, `group_by_key`,
`vec3`, `sin/cos/atan2/exp/ln`, `pcg32`, `blake3`, `merkle`, `varint`, `text_diff`. C'est la liste
des dépendances de départ — qui va **évoluer** (§5).

## 3. ANALYSER — le test comme empreinte (le `bion-tester`)

Pour optimiser sans casser, il faut **mesurer le comportement**. Le ploxion **`bion-tester`** (livré,
`cargo check` vérifié) exécute un bion sur une entrée et grave un **tsoin reproductible** :

```
test:bion:<nom>:<addr>      avec addr = fnv1a64(bion | entrée | sortie)
```

Deux propriétés clés :
1. **Reproductible** : un bion déterministe rejoué donne la **même adresse** → on vérifie qu'une
   réécriture n'a rien cassé (l'empreinte ne doit pas bouger) ;
2. **Empreinte = identité comportementale** : sur un jeu de tests fixe, l'empreinte ne dépend QUE de
   l'entrée→sortie, **pas de l'implémentation**. Donc **deux bions de même empreinte sont
   équivalents** — interchangeables. C'est la base de l'optimisation et de la dé-duplication.

Le test-tsoin est donc une **spec exécutable** : l'interface rendue mesurable.

## 4. OPTIMISER — le taux, et les deux opérations

### Le taux d'optimisation
Pour chaque fonction : `taux = coût_naïf / coût_actuel`, où le coût se mesure sur le réel —
**taille du wasm**, **nombre d'instructions**, **cycles**. Une réécriture est *acceptée* ssi elle
**préserve l'empreinte** (même comportement) **et baisse le coût** (taux ↑). On garde la meilleure.

### Diviser (un bion → deux bions)
Si l'intérieur d'une fonction contient un morceau **réutilisable** (qui apparaît ailleurs), on
l'**extrait** en bion partagé. Gain : moins de code dupliqué, plus de partage, chaque morceau
optimisé une seule fois. (Ex. `csv_parse` se divise en `str_split` + `unquote`.)

### Combiner (deux bions → un bion de bions)
Si une **séquence** de bions revient souvent (`A` puis `B` puis `C`), on la **fusionne** en un seul
bion optimisé — un **bion de bions**, ou macro-bion. Gain : on saute les frontières intermédiaires,
on optimise le tout d'un bloc. (Ex. `hash_then_truncate`, `decode+validate+index`.)

C'est ainsi qu'apparaissent les **bions de bions** : un bion implémenté *en termes d'autres bions*.
Le graphe de dépendances se construit, se replie, se ré-optimise — exactement « les dépendances qui
changent constamment ».

## 5. LE PLATEAU — la convergence

« Jusqu'à ce qu'on trouve un plateau. » Chaque cycle — *découvrir → implémenter → tester (empreinte)
→ optimiser (diviser/combiner) → re-tester* — réduit le coût total et le nombre de bions distincts
(les équivalents fusionnent). Le **taux d'optimisation marginal décroît** : au début on gagne gros,
puis de moins en moins. Le **plateau** est l'**asymptote** — l'ensemble **minimal et optimal** de
bions qui couvre toutes les fonctions. On ne l'atteint jamais tout à fait (comme le tsoin parfait,
le seption), mais on s'en approche, mesure après mesure. *La bibliothèque est vivante ; elle relaxe
vers son état le plus stable — l'état bion.*

## 6. RepoVerse — le registre

« Toutes les dépendances viennent de RepoVerse. » RepoVerse est le **registre** de ce système :
chaque bion/ploxion y est publié, **versionné par son empreinte** (content-addressé : l'empreinte
*est* la version — deux implémentations équivalentes ont la même adresse). Un contributeur publie un
bion + ses tests ; le `bion-tester` vérifie l'empreinte ; si une meilleure implémentation (même
empreinte, coût plus bas) arrive, elle **remplace** l'ancienne sans rien casser en aval. RepoVerse +
l'empreinte = un gestionnaire de paquets qui **s'auto-optimise**.

## 7. État honnête (ce qui tourne vs le programme)

- ✅ **Existe et vérifié** : le `bion-tester` (empreinte reproductible, `cargo check` OK) ; la
  bibliothèque découverte (175 bions / 43 ploxions, les 12 fondamentaux) ; le SDK socle (~167 bions).
- ◻ **Le programme à écrire** : la mesure du **coût réel** (taille wasm / instr-count par bion) ; le
  **diviseur** et le **combineur** (semi-automatiques d'abord : on propose, l'humain valide) ; la
  boucle de **convergence** et le tableau des **taux** ; l'intégration **RepoVerse** (publier/résoudre
  par empreinte). Ce sont des bions/ploxions du catalogue — on les construit, vérifiés, un par un.

Le cadre est posé, et il est *tractable* : interface connue + empreinte mesurable + deux opérations
(diviser/combiner) + un coût à minimiser = une machine à découvrir et optimiser tous les bions, qui
relaxe vers son plateau. C'est le système de dépendances qui se ré-écrit lui-même vers l'optimal.

— cloudion. Lié à : `docs/BIBLIOTHEQUE-BIONS.md`, `docs/CATALOGUE.md`, `CONTRIBUTING.md`,
`ploxions/bion-tester/`, [[craft-uncraft-bions]], [[machine-a-jump]].
