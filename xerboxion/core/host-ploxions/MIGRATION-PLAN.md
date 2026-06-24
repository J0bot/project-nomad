# Plan de migration des conteneurs → xerboxion-core (honnête)

José (2026-06-22) : *« dans 12h tout migré sur le xerboxion-core, divise en ploxions et fais des
bions »*. Voici l'état réel des **25 conteneurs** et ce qui est faisable en 12h vs ce qui est une
migration **profonde** (recherche multi-jours). Je ne survends pas : « tout en 12h » bute sur les
services **stateful** (un Postgres/Git/IDE ne devient pas un bion en 12h).

## ✅ Migrés (3) — hors-docker → services host dans le core
`filesystem` (3010), `tsoin` (3011), `record` (3012). Pattern : code dans `host-ploxions/<nom>/`,
systemd `ploxion-<nom>.service` lié 127.0.0.1, route Traefik file-based + SSO, conteneur retiré.
**Gain sécu** : plus sur le réseau docker partagé.

## 🟡 Migrables ensuite (3) — débloqués par l'AUTH-BUS
- `ploxion-openscad` (8770) + `ploxion-vscode` (8080, code-server) : **consommés par le conteneur
  labo** → le service host doit binder la passerelle (`10.0.0.1`) pour être atteignable depuis le
  conteneur → ré-exposition latérale **tant que l'auth-bus n'enforce pas**. Donc : faire après
  l'auth-bus (middleware déjà codé, `e51d5b5`). vscode en `--auth none` → ajouter aussi un gate.
- `bi0ns-syncthing` : un binaire syncthing → service host systemd (migrable, pas couplé).

## 🔴 Infra-lourde (19) — PAS core-able en 12h (migration profonde = recherche)
- **Stores stateful** : `mw2-labo-db` (mariadb), `repoverse-…-postgres/neo4j/redis/minio/meilisearch/
  livekit`, `coolify-db`, `coolify-redis`. → Réimplémenter une base de données/graph/objet **en bions**
  = un projet en soi (durabilité, requêtes, index). Pas 12h.
- **Git servers** : `gitea-…`, `repoverse-…-gitea`. → Idem (un serveur Git = stateful + protocole).
- **Apps** : `mw2-labo-app` (Laravel — les **UIs** migrables en `web-*` dans le core, le **backend**
  Laravel non), `repoverse-…-backend/frontend` (Node), les apps Coolify (`app-l4t…`, `jope5qs…`,
  `wwso80…`, `db-l4t…`).
- **Plan de gestion Coolify** (6) : `coolify`, `coolify-proxy` (Traefik — **le routeur que J'UTILISE**
  pour router les migrations), `coolify-realtime`, `coolify-sentinel`, `coolify-db`, `coolify-redis`.
  → Le retirer = retirer le plan de déploiement + le routage. **En dernier**, et c'est un remplacement
  complet (le core devient son propre routeur/déployeur).

## Phases réalistes
1. **Auth-bus** (codée `e51d5b5`) → activer : poser `XION_BUS_TOKEN`, maj des ~10 clients, restart core
   (étape coordonnée délibérée). **Débloque** openscad/vscode (bind-gateway devient sûr).
2. **Migrer** openscad, vscode, syncthing (services host, comme les 3 premiers).
3. **UIs du labo** → `web-*` dans le core (migration déjà amorcée : le core sert `/px/:id/*`).
4. **Profond (multi-jours, honnête)** : les stores/git → soit des **bions stateful** (recherche : un
   bion KV/graph durable), soit garder ces services mais les sortir du réseau partagé + auth. Le core
   devient routeur (remplace Traefik) = le dernier gros morceau avant de retirer Coolify.

## Métrique honnête
**Migrable proprement** (services sans état, ploxions) : ~6/27. **Infra-lourde** (stateful + plan de
gestion) : ~19/27. La vraie cible « tout en bions » est un **arc de recherche**, pas un sprint 12h —
mais chaque service migré est réel, réversible, et un gain sécu immédiat.
