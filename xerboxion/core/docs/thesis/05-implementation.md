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
