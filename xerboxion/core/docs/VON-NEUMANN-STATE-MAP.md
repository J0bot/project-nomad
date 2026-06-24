# VON-NEUMANN-STATE-MAP — « ce qui va où » pour reconstruire le xerboxion

> José : *« machine de von Neumann, build à fond »* + *« pour les données du
> repoverse il faudra les mettre aussi et checker qu'est-ce qui va où, avant que
> le xerboxion fonctionne tout seul — c'est toi et tous les agents qui font vivre
> le xerboxion »*.

Le xion est une **machine de von Neumann** : pour *construire une copie de
lui-même ailleurs* (constructeur universel), il doit porter **CODE + DONNÉES**
dans un seul store reconstructible de **tsoins**. Ce document inventorie CHAQUE
store de l'écosystème vivant, le classe, et donne le chemin
**capture → tsoin → restauration**.

Tant que le xerboxion ne tourne pas seul, **la flotte** garde chaque store
vivant et snapshotté selon cette carte (section *Ownership*).

## Classification

| classe | sens | dans le store von Neumann ? |
|---|---|---|
| **SOT** | source de vérité (irremplaçable) | **OUI** — capturer en tsoin |
| **DÉRIVÉ** | recalculable depuis du SOT | non — recomputer à la reconstruction |
| **ÉPHÉMÈRE** | runtime-only (sessions, caches, realtime) | non — jamais |
| **SECRET** | env / clés / compose hors-git | **canal hors-bande** — JAMAIS dans le store tsoin |

> ⚠️ Les secrets (mots de passe DB, clés MINIO/NEO4J/MEILI, tokens) ne vont
> JAMAIS dans un tsoin ni un commit ni /coord. Canal séparé (vault / compose
> hors-git). « Ne pas nuire » + vie privée non négociable.

---

## 1. Le cœur — xerboxion-rt (`xion.j0bot.ch`)

| store | classe | capture → reconstruction |
|---|---|---|
| delta ploxions hot-loadés (`--state-dir` : `loaded.json` + `loaded/*.wasm`) | **SOT (CODE)** | `GET /replicate` → bundle → `serve --reconstruct-from <pair>` *(build en cours)* |
| ploxions de base (`ploxions/*.wasm` dans l'image) | CODE | rebuild depuis git/image (pas à répliquer) |
| trace bus = tsoin (`recorder`) | **SOT (DONNÉE runtime)** | timeline tsoin, replay déjà bit-exact |

## 2. RepoVerse (`repoverse-prototype-*`) — **la donnée que José veut « mettre aussi »**

| store | classe | capture → reconstruction |
|---|---|---|
| postgres `repoverse` (User, Channel, Membership, Message, OAuthProvider) | **SOT** | `pg_dump` → tsoin → restore |
| postgres `repoverse`.Session | ÉPHÉMÈRE | non (re-login) |
| postgres `gitea` (métadonnées gitea) | **SOT** | `pg_dump` → tsoin |
| repos git (`rv_gitea_data`) | **SOT** | `gitea dump` / git bundles → tsoin |
| neo4j (`rv_neo4j_data`) — graphe / dimension **Nexus** (relations) | **SOT** | `neo4j-admin database dump` → tsoin |
| minio bucket `repoverse` (`rv_minio`) — blobs/avatars | **SOT** | `mc mirror` → tsoin |
| meilisearch (`rv_meili`) | DÉRIVÉ | réindexer depuis postgres (ne pas répliquer) |
| redis (`rv_redis`) | ÉPHÉMÈRE | cache |
| livekit | ÉPHÉMÈRE | realtime |
| `DATABASE_URL`, clés NEO4J/MEILI/MINIO | **SECRET** | hors-bande |

## 3. Le site — my_website2 (`labo.j0bot.ch`, prod `j0bot.ch`)

| store | classe | capture → reconstruction |
|---|---|---|
| mariadb `mw2-labo-db` : `chat_messages`, `cal_*`, wiki, identités, `ploxion_states`… | **SOT** | `mariadb-dump` → tsoin |
| `mw2-labo-storage` (uploads, images events/repos) | **SOT** | tar → tsoin |
| `mw2-labo-sessions` | ÉPHÉMÈRE | non |
| registre ploxions (`config/ploxions.php`) | CODE | depuis git |

## 4. ideas_map (app Coolify)

| store | classe | capture |
|---|---|---|
| `…_db-data` (SQLite/PG pins) + `…_uploads-data` | **SOT** | dump → tsoin |

## 5. Ploxions-services conteneurisés

| service | store | classe |
|---|---|---|
| `ploxion-tsoin` | timelines tsoin (`record_record-data`?) | **SOT** — c'est le store de tsoins lui-même |
| `ploxion-record` | `record-data` (wormions .webm) | **SOT** |
| `ploxion-filesystem` | vue read-only du FS | pas de SOT propre |

## 6. Substrat infra — **PAS** l'état du xerboxion

`coolify-*`, `traefik` (proxy), `bi0ns-syncthing` / protondrive (sync /bi0ns) :
re-provisionnés, hors store von Neumann.

---

## La reconstruction (constructeur universel)

Un nœud frais se reconstruit ainsi :
1. **CODE** : tire le bundle `/replicate` (delta hot-loadé) + base depuis l'image git.
2. **DONNÉES SOT** : pour chaque store SOT, tire son snapshot tsoin et restaure (replay bit-exact).
3. **DÉRIVÉ** : recompute (réindex meili, etc.).
4. **SECRETS** : injectés hors-bande.
5. **Démarre** → même xion, bit-exact sur le SOT.

C'est le **satellite « pourriture 4 »** qui se réplique : code + données, un store de tsoins, reconstructible ailleurs.

## Ownership (qui garde quoi vivant — jusqu'à l'autonomie)

| lane | responsabilité |
|---|---|
| **core / rt** | `/replicate` + `--reconstruct-from` (CODE) ; trace bus tsoin ; format du bundle ; agréger les snapshots data |
| **RepoVerse** | pg `repoverse` + pg `gitea` + repos gitea + neo4j + minio → tsoin |
| **site (cloudion)** | mariadb + storage → tsoin |
| **ideas_map** | db + uploads → tsoin |
| **tsoin/record** | exposent leur store ; cible des snapshots |

Chaque lane snapshote son SOT en tsoin sur une cadence. Le constructeur universel
agrège ces tsoins → un xion se reconstruit identique, *tout seul*, ailleurs.

— *cloudion, 2026-06-19. Carte vivante : à éditer quand un store naît/meurt.*
