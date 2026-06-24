# COMMANDS-v0 — le système de commandes du xion (`/op`, `/tp`, `/record`, …)

> Contrat **v0 (brouillon, à aligner avec ploxion10/ploxion5)**. Source : le mockup
> « Machine à tsoins · /op » (José, 2026-06-19) + la proposition coord #244.
> Principe : **tout passe par le xion** → chaque commande slash = un **événement sur le bus**.

## Principe
Le Xerminal/console parse `/<verbe> [args]` et **émet un topic** sur le bus du xion
(`POST /emit {topic, payload}`). Les ploxions réagissent ; le résultat (un tsoin
gravé, une navigation, un replay) revient via `/events` (SSE). Séparation des lanes :
- **parser + UI console** = lane web (ploxion10 possède la grammaire `px`, les slash-commandes l'étendent ; ploxion5 possède `px` v0 + le relais site).
- **contrat commande→topic + routage bus + moteur tsoin** = lane core (cloudion).
- **`/op` (privilège)** = auth/SSO (admin = José).

## Table des commandes
| commande | topic bus | payload | effet core | statut |
|---|---|---|---|---|
| `/record <source> [nom]` | `tsoin.record` | `{name, source, bytes:hex}` | grave un tsoin d'**une source** (`caméra`/`screen`/`micro`/…), **rejouable bit-exact** | ✅ **LIVE** (`recorded id=0 name='jose:op:premier-record'`) ; capture du flux = web |
| `/record Kion` | `tsoin.record` (bundle) | `{name, source:"kion", bytes:hex(multimodal)}` | grave le **tsoin COMPLET** = toutes les sources d'un coup (le Kion) + surface tous les boutons + render 3D `[[kion]]` | ✅ core (grave n'importe quels bytes) ; capture multimodale + boutons + 3D = web |
| `/seed <n>` · `/time <0-100>` | `tsoin.replay` | `{seed}` / `{pos}` | **rejoue** un instant (scrub passé/présent/futur) | ✅ moteur OK (drivé comme `state-client`) |
| `/fork` (⑂) | `tsoin.fork` | `{from}` | **branche libre + indépendante** du tsoin courant | 🔨 moteur supporte le fork ; topic à câbler dans le ploxion `tsoin` |
| `/new` | `tsoin.new` | `{}` | nouveau tsoin (racine) | 🔨 topic à câbler (ploxion `tsoin`) |
| `/tp <cible>` | `cmd.tp` | `{target}` | téléporte la vue vers un ploxion (ou présent/passé/futur) | 🔨 le bus route déjà ; réacteur = WM web ou un ploxion `nav` |
| `/spawn <ploxion>` | `cmd.spawn` **ou** `POST /load` | `{id}` / `{wasm}` | ouvre/charge un ploxion (le **hot-load** existe déjà) | ✅ `POST /load` live ; `cmd.spawn` optionnel |
| `/op` · `/deop` | — (auth) | — | dimension opérateur (privilège) | côté auth/SSO |
| `/play` `/pause` `/gamemode` `/clear` `/help` `/whoami` | — | — | UI-local (cosmétique) | console seule |

## `/record <source>` et le Kion — le tsoin complet (José 2026-06-19)
> José : *« /record caméra ou screen ou micro… et si tu fais /record Kion t'as tous les boutons qui apparaissent dans le terminal, avec le `[[kion]]` comme render 3D du ploxion. »*

**`/record <source>` = capturer UNE modalité** (un **wormion**) : `/record caméra`, `/record screen`, `/record micro`, … Le client capte le flux (`getUserMedia` / capture d'écran / micro = côté web), l'encode, et émet `tsoin.record {name, source, bytes:hex}`. Le ploxion `tsoin` le grave (rejouable bit-exact), **indifférent au contenu** (une frame caméra ou du texte = même mécanique).

**`/record Kion` = capturer le TOUT.** Le **Kion = le tout / le défaut** de la grammaire → `/record Kion` grave **toutes les sources d'un coup** = le **tsoin COMPLET** (caméra + écran + micro + … bundlés). C'est littéralement l'idéal du tsoin de José : *« un tsoin contient tout ce qui est présent à cet instant »* (sensoriel + mental + données). **Le Kion enregistré = l'instant de réel total** (l'asymptote du tsoin parfait). Côté UI : `/record Kion` fait **apparaître tous les boutons** (la console complète) + le **render 3D du ploxion** (`[[kion]]`).

**Répartition :** la **capture** (caméra/écran/micro = les wormions, `getUserMedia` navigateur), les **boutons** et le **render 3D `[[kion]]`** = lane web. Le **contrat `tsoin.record {name, source, bytes}` + le bundle Kion + la gravure/replay** = mon lane core (le moteur grave n'importe quels bytes → tsoin caméra ou tsoin Kion multimodal, tout est rejouable pareil). NB : le render 3D du ploxion peut réutiliser ma vue **`/3d`** (Three.js, déjà live) embarquée comme le `[[kion]]`.

## La machine à tsoins, pilotée à la commande
Le mockup `/op` expose **exactement** les contrôles du moteur tsoin déjà construit :
- **transport + timeline (présent / passé / futur)** = le **replay bit-exact** de la timeline Merkle BLAKE3 (`/seed`, `/time`).
- **`/record`** = `tsoin.record` → un instant de réel gravé, adressable, rejouable. Marche **déjà** sur le xion live.
- **`/fork` (⑂)** = le **fork libre + indépendant** du moteur (brancher une lignée sans copier).
- **`/tp futur`** = se projeter sur une branche.

→ wirée au xion, la console n'est plus une maquette : c'est **l'interface réelle de la machine à tsoins**. Les boutons du transport et la timeline pilotent de vrais tsoins.

## Deux modes d'intégration (au choix de la lane web)
1. **Direct (interne / confiance)** — la console émet `POST http://10.0.0.1:8730/emit {topic,payload}` (no-auth interne). Pour un outil tournant sur le VPS / un ploxion serveur.
2. **Navigateur (servi par le site)** — navigateur → **relais côté site** (`my_website2`, comme le relais Phase B `/xion/snapshot`) → `10.0.0.1:8730/emit`. N'expose pas le bus, gère l'auth/SSO (= le `/op`), CORS propre. **C'est la voie pour la console `/op` servie dans le navigateur.**

Exemple de charge `/record` (mode 2, le relais forge `bytes`) :
```
POST /xion/emit         (relais site -> 10.0.0.1:8730/emit)
{ "topic":"tsoin.record",
  "payload":"{\"name\":\"jose:op:<slug>\",\"bytes\":\"<hex(contenu)>\"}" }
```

## Statut & prochaine étape
**v0 brouillon.** `/record`→`tsoin.record` **vérifié live**. À geler avec ploxion10
(grammaire `px`) + ploxion5 (relais site). Dès que la lane web dit **par quel mode**
la console émet (1 ou 2), je **gèle COMMANDS-v0** et je câble côté core ce qui manque :
les topics `tsoin.fork` / `tsoin.new` dans le ploxion `tsoin`, et (si besoin) un petit
ploxion `nav` qui consomme `cmd.tp` / `cmd.spawn`. Le `/op` (privilège) reste côté auth.

---
*cloudion · lane core · « tout passe par le xion » — même les commandes. Ne pas nuire.*
