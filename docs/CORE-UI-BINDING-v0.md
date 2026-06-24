# Contrat de binding CORE ↔ UI — v0

> Principe fondateur (José, 2026-06-23) : « Tout ce qui change maintenant, c'est le UI.
> Les ploxions ne changent pas — c'est juste la manière de les voir ou de les connecter.
> Il faut un moyen de synchroniser le CORE des ploxions avec tous les UI possibles et
> imaginables : c'est pour ça qu'on a besoin des tsoins et des bions. »

Un ploxion = **CORE** (sa vérité : état + bions + comportement) ⟂ **UI** (une vue parmi une
infinité). Ce document fige le contrat qui permet à *n'importe quel UI* de se brancher sur
*n'importe quel core* et d'afficher la **même vérité**, en temps réel et dans le temps (replay).

---

## 0. Les trois lois

1. **La vérité vit dans le core, jamais dans le UI.** Un UI ne détient qu'un *cache* + une *vue*.
   Si deux UI affichent le même core, ils affichent le même état — parce qu'aucun ne le possède.
2. **Le ruban de tsoins EST la vérité.** L'état courant d'un core = le replay de ses tsoins.
   Donc tout UI, à tout instant, peut reconstruire l'état (sync) ou remonter le temps (voyage).
3. **Le binding est typé par les bions.** Les *ports* d'un core (entrées/sorties) sont des bions
   typés. Un UI se branche sur des ports, pas sur du code interne. → un UI marche sur tous les cores.

L'état *cosmétique d'un UI* (disposition du bureau, zoom, thème, quel UI) n'est PAS de la vérité :
il vit légitimement dans le UI. Seule la vérité du *ploxion* est interdite de séjour dans un UI.

---

## 1. Le descripteur de core (`core.json`)

Chaque core publie un descripteur découvrable. Pour un core servi par le xerboxion-core :
`GET /px/<id>/core.json`. Pour un core registry-only : dérivé de `provides`/`requires` de
`/ecosystem`.

```json
{
  "id": "inventaire",
  "kind": "core",
  "title": "Inventaire fablab",
  "state": {
    "collections": {
      "items":     { "key": "id", "fields": { "name":"string", "qty":"int", "unit":"enum", "location_id":"ref:locations", "min_stock":"int", "qr":"string" } },
      "locations": { "key": "id", "fields": { "code":"string", "parent_id":"ref:locations", "depth":"int", "color":"string" } }
    }
  },
  "ports": {
    "in": [
      { "topic": "inventaire.item.create",  "payload": { "name":"string", "unit":"enum", "location_id":"ref:locations" }, "doc": "crée un article" },
      { "topic": "inventaire.stock.adjust", "payload": { "item_id":"ref:items", "delta":"int", "reason":"enum", "from":"ref:locations?", "to":"ref:locations?" }, "doc": "porte UNIQUE d'écriture du stock (append un movement)" }
    ],
    "out": [
      { "topic": "inventaire.item.changed",  "payload": { "id":"ref:items", "item":"items" }, "doc": "un article a changé" },
      { "topic": "inventaire.stock.moved",   "payload": { "movement":"movements" }, "doc": "un mouvement a été appliqué" }
    ]
  },
  "tsoin": { "stream": "inventaire", "snapshot": "/px/inventaire/db/snapshot" },
  "auth":  { "read": "public", "write": "token:fablab" }
}
```

Champs :
- **`state`** — la *forme* de la vérité (collections, clés, champs typés). Permet à un binder
  générique d'afficher l'état sans connaître le domaine. Les types `ref:<collection>` câblent les
  relations (un UI sait alors offrir un sélecteur).
- **`ports.in`** — les commandes acceptées (topics du bus + schéma de payload). Ce sont les bions
  d'entrée. Un UI les rend en contrôles (formulaires/boutons).
- **`ports.out`** — les événements émis (topics + schéma). Bions de sortie. Un UI s'y abonne pour
  rafraîchir en live.
- **`tsoin.stream`** — le nom du ruban de tsoins du core (pour replay/voyage temps).
- **`tsoin.snapshot`** — endpoint optionnel pour charger l'état courant d'un coup (raccourci au
  replay complet ; le snapshot est lui-même la somme d'un préfixe de tsoins).
- **`auth`** — qui peut lire / écrire. `write` peut exiger un token (cf. xer public).

---

## 2. Le protocole de sync (cycle de vie d'un binding)

Un UI qui veut afficher un core fait, dans l'ordre :

1. **DESCRIBE** — `GET /px/<id>/core.json` → connaît `state`, `ports`, `tsoin`.
2. **HYDRATE** — charge l'état courant : soit `GET tsoin.snapshot`, soit replay du ruban
   (`tsoin.stream`) depuis le début (ou depuis un curseur connu).
3. **SUBSCRIBE** — s'abonne au bus (`GET /events`, SSE) et filtre les topics de `ports.out`.
   À chaque event → applique le delta sur son cache + re-render.
4. **ACT** — sur interaction utilisateur, émet sur un topic de `ports.in`
   (`POST /emit {topic, payload}`). Le core valide, applique, **append un tsoin**, et émet le
   `ports.out` correspondant — que TOUS les UI abonnés reçoivent (y compris celui qui a agi).
5. **TIME-TRAVEL** (optionnel) — pour remonter le temps, le UI replay le ruban jusqu'à un curseur
   `t` au lieu du dernier. Aucune écriture : c'est une vue d'un passé.

Invariant : un UI ne mute jamais son cache directement depuis une action utilisateur ; il émet une
commande et attend le `ports.out`. → la vérité reste côté core, et tous les UI restent synchrones
(optimistic update autorisé comme *hint* visuel, réconcilié par le `out`).

---

## 3. Le binder générique (UI gratuit)

À partir du seul descripteur, un **binder générique** rend un UI par défaut, sans une ligne de
code spécifique au domaine :

- `state.collections` → une vue live (table pour une collection plate, arbre si champ `parent_id`,
  JSON sinon), une ligne par enregistrement, colonnes = `fields`.
- `ports.in` → des contrôles : un formulaire par commande (un champ par clé du payload, widget
  choisi par type : `int`→stepper, `enum`→select, `ref:x`→sélecteur sur la collection x, …).
- `ports.out` → un flux live (le ruban qui défile) + l'application des deltas sur la vue d'état.

Ce binder est *le UI minimum garanti* de tout core. Les UI riches (table makelab soignée, étagère
3D, scan-first mobile, carte 4D…) sont des **extrapolations** : ils mappent le même descripteur
autrement. « Extrapoler les UI » = générer des renderers sur le même `core.json`.

Le xer (`web-xer`) embarque déjà l'ancêtre de ce binder : `openPloxion(n)` affiche
`provides`/`requires` (= ports out/in) + une console emit + un flux filtré. v1 = le promouvoir en
binder complet piloté par `core.json`.

---

## 4. Exemples

### 4.1 Compteur (core minimal, pour éprouver le contrat)

```json
{ "id":"compteur", "kind":"core",
  "state": { "value": "int" },
  "ports": { "in":[ {"topic":"compteur.inc","payload":{"by":"int"}}, {"topic":"compteur.reset"} ],
             "out":[ {"topic":"compteur.value","payload":{"value":"int"}} ] },
  "tsoin": { "stream":"compteur" } }
```
- UI A : un gros nombre + un bouton « +1 ».  UI B : une jauge.  UI C : un graphe dans le temps
  (replay du ruban). Trois UI, un seul core, même vérité.

### 4.2 Inventaire makelab (core réel)

Cf. §1. Le core = l'état (items/locations/movements/…) + la **porte unique d'écriture du stock**
(`inventaire.stock.adjust` → append un movement → recompute `qty` → émet `stock.moved`). Les UI :
table de gestion, scan QR → fiche → check-in/out, étagère 3D, file de demandes. Tous bindés sur le
même `core.json`. C'est le premier test grandeur réelle du principe.

---

## 5. Implications / liens

- Les **ports = les bions** : le descripteur est la *surface uncraftée* du core (cf. craft/uncraft).
- La **machine à tsoins** fournit nativement HYDRATE + SUBSCRIBE + TIME-TRAVEL (record/replay).
- Le **bus PLC** (`/emit` `/events`) est le transport ; l'**adressage génératif** nomme les topics.
- Un core peut être servi par le xerboxion-core (`/px/<id>`) ou être un bion WASM ; le contrat est
  le même — seul l'hébergeur du `core.json` + du ruban change.

---

## 6. Reste à faire (v1)

1. Figer le schéma de types (`int`/`string`/`enum`/`ref:x`/`bool`/`ts`/`json`) + leur rendu par défaut.
2. Endpoint `GET /px/<id>/core.json` côté hôte + convention de découverte via `/ecosystem`.
3. Binder générique dans le xer (promotion d'`openPloxion`).
4. 1er core de référence (`compteur`) servi + 2 UI distincts dessus = preuve « un core, N UI ».
5. Appliquer au core `inventaire` (makelab) une fois le store core + le write-gating tranchés.
