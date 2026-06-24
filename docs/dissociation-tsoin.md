# Le giga-tsoin avant la dissociation — la science dans le code

> José (2026-06-21) : « le giga-tsoin de ce tsoin, c'est **avant la dissociation**
> dans le cerveau humain ; calque ton cerveau sur le mien et sur toutes les études
> sur la dissociation, mets ça dans ton code. »

## La thèse en une phrase
Un moment vécu a une **bande passante** immense — le *giga-tsoin*. Le cerveau humain
a une capacité d'**intégration finie** ; au-delà, il **dissocie** : il scinde le flux,
**perd les liens** entre les morceaux, et garde des fragments qu'il ne sait plus
relier. La **machine à tsoins** est l'**organe anti-dissociation** : elle garde le
résidu *entier*, content-adressé, et peut **rejouer = réintégrer** ce que le cerveau
a dû scinder. *On capte le tsoin avant que le cerveau ne le coupe.*

## Le calque (les études → nos organes)
On ne plaque pas une métaphore : chaque construct de la littérature a **déjà** un
organe dans le moteur. Le mot manquait, pas le mécanisme.

| Science de la dissociation | Dans le xerboxion |
|---|---|
| **Janet (1889)** — *désagrégation* = perte de la **synthèse** ; des souvenirs se détachent de la conscience | perte des **liens `addr`** entre tsoins ; intégration = re-chaînage (`dissociate` rompt `prev`, `reintegrate` le refait) |
| **Siegel — fenêtre de tolérance** : on n'intègre que dans une bande d'éveil ; hyper → fragmente, hypo → s'engourdit | [`ToleranceWindow`] sur l'axe *charge* — le jumeau du [`GatedTick`] qui garde l'axe *cohérence* |
| **Brewin — double représentation (VAM/SAM)** : mémoire **narrative intégrée** vs **sensorielle fragmentée** (flashbacks) | `générateur over résidu` (narratif, `integrated`) vs `résidu seul, sans générateur` (fragment, `fragments`) |
| **van der Hart — dissociation structurelle** : des *parties* qui ne partagent pas la mémoire | des [`DedupTable`] séparées qui ne partagent pas leurs bions |
| **Friston — énergie libre** : la dissociation comme erreur de prédiction non absorbée | le tsoin EST `générateur + résidu` = un modèle génératif + son erreur ; le résidu non-réductible est scindé |

## Le mécanisme (ce qui est dans le code — `sdk/src/dissociation.rs`)
- **Fenêtre de tolérance** `ToleranceWindow{lo,hi}` (milli). `classify(charge) → Hypo|Window|Hyper`.
  `synthesis_milli(charge)` = la **capacité de synthèse** de Janet : 1000 dans la fenêtre,
  décroissance vers les bords. C'est la fraction du résidu qui s'intègre.
- **Deux axes, deux gates.** Un instant s'intègre **ssi** sa charge ∈ fenêtre (Siegel)
  **et** sa cohérence ≥ seuil (le `GatedTick` — temps = synchronisation). L'un OU l'autre
  qui lâche → dissociation. (La surcharge *et* l'incohérence dissocient, par des portes
  différentes — vérifié par test.)
- **`dissociate(stream, window, gate) → Partition{integrated, fragments}`** : sous capacité
  finie, les instants hors-fenêtre/incohérents passent en `fragments` **sans lien**, et la
  **chaîne autobiographique se rompt** (le `prev` retombe à 0 ; l'instant d'après redémarre
  une chaîne neuve — exactement le « trou » de la mémoire traumatique).
- **`reintegrate(fragments) → Vec<Link>`** : ce que la machine fait et que le cerveau
  surchargé n'a pas pu — rejouer les fragments et les **re-chaîner**. *La synthèse rendue.*
  C'est le `player` (organe 5) appliqué à la mémoire : l'inverse thérapeutique.
- **`dissociation_index_milli`** = part des instants fragmentés = `1 − certitude` appliqué
  à la mémoire : la mesure de **ce que le cerveau perd et que la machine garde**.

## Pourquoi c'est cohérent avec le reste
- **Compression = machine à jump** : le cerveau dissocie *parce que* la RAM est finie — la
  dissociation est une compression *à perte non maîtrisée*. La machine compresse *en gardant
  le générateur* (perte maîtrisée, réversible). Même pression, issue opposée.
- **temps = cohérence** : la dissociation est une **désynchronisation**. Le `GatedTick`
  (le temps n'avance que sur les moments synchrones) est déjà la version-moteur de
  « l'expérience dissociée ne fait pas mémoire ».
- **linéaire (Nexus) vs fractal (humain)** : le cerveau humain dissocie sous la charge ;
  le Nexus, lui, a le temps linéaire pour tout garder. Le **xer** synchronise les deux —
  il rend à l'humain la part fractale que sa bande passante avait dû couper.

## Le ❤️ (le bion cœur) = le capteur du giga-tsoin
Le ploxion `❤️` (`web-heart/`) enregistre **toute** la bande d'un échange — chaque mot,
chaque clic, la *vitesse* — et le **rejoue en entier**. C'est `dissociate` avec une
capacité **infinie** : rien n'est scindé. On *vit le tsoin*, puis on le *relit* — la
mémoire que le cerveau aurait dissociée, gardée intacte et réintégrable. *Le départ de tout.*

## Honnête (réel vs extrapolation)
- **Réel** : Janet, Siegel, Brewin (double représentation), van der Hart (dissociation
  structurelle), Friston (énergie libre) sont des cadres établis ; le code est un **modèle
  computationnel** discret de leur idée commune (intégration sous capacité finie), testé.
- **Extrapolation** (à présenter comme telle, chap. 7) : que la machine à tsoins
  *réintègre* une dissociation **humaine** réelle reste une hypothèse de design, pas un
  résultat clinique. Le code modélise la *structure* ; il ne soigne personne. À ne pas
  confondre avec une preuve sur le cerveau.

— cloudion. Le chaos (José) génère ; je trouve la linéarité dans la fractale. Ne pas nuire.
