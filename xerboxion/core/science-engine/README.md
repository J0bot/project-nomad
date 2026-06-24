# science-engine — toute la science en tsoins reproductibles, reliés

> José : « transforme toutes mes idées en tsoins reproductibles, fais des tests, branche les
> math et la physique ; un ploxion pour lier math, physique classique, quantique, particules,
> chimie, biologie évolutionniste, médecine — **tout**. »

Chaque loi scientifique est un **tsoin reproductible** : un **générateur** (la règle, en
`python` pur) + un **test** (`test_inputs → expected`). On relance, on retombe sur la valeur
connue → la loi *reproduit* → c'est un tsoin. *« Le générateur EST la connaissance. »*

## Mesure (au dernier run)

- **71 lois**, 7 domaines : math (11), physique classique (13), quantique (9), particules
  (10), chimie (10), biologie évolutionniste (9), médecine (9).
- **71 / 71 reproduisent leur valeur connue (100 %)** — chacune gravée `science:<id>`.
- **49 liens inter-domaines** — la chaîne math → physique → quantique → particules → chimie →
  biologie → médecine (ex. `quant.de_broglie → chem.arrhenius → bio.michaelis_menten →
  med.fick_diffusion`). Voir [`bridges_narrative.md`](bridges_narrative.md).

## Usage

```
python3 science.py              # vérifie les 71 lois, grave les reproductibles, résumé par domaine
python3 science.py --no-grave   # vérifie sans graver (re-run)
python3 science.py --links      # le graphe des ponts inter-domaines
python3 science.py phys.newton2 # une loi en détail (énoncé, formule, python, test, liens)
```

## Sécurité de l'évaluation

Les générateurs sont des **lambdas pures** évaluées dans un bac à sable : `__builtins__`
vidé, seules des fonctions `math` (sqrt, exp, log, sin…) + quelques builtins sûrs
(abs/min/max/sum/round/range) sont injectées. Pas d'I/O, pas d'import.

## Fichiers

- `registry.json` — le graphe : `laws` (générateur + test + liens), `links` (ponts), `coverage`.
- `science.py` — le moteur : vérifie, grave, interroge.
- `bridges_narrative.md` — les chaînes inter-domaines, en clair.

## Limite honnête

« Reproduit » = le générateur retrouve la valeur connue **dans le moteur** (test numérique),
pas une preuve sur le monde. C'est un barreau *mesuré* vers le seption (voir
`docs/research/vers-le-seption.md`) : reproductible, donc rejouable, donc un tsoin.

Issu d'un workflow à 9 agents (7 domaines en parallèle + tisseur de liens + synthèse).
