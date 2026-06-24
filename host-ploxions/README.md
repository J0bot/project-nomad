# host-ploxions — services migrés HORS de Docker (vers l'hôte / le xerboxion-core)

José (2026-06-22) : migrer tous les conteneurs hors-docker → ploxions → bions dans le core.
Étape 1 = sortir de docker : le service tourne sur l'HÔTE (systemd, lié 127.0.0.1), son code vit
ICI dans le repo core, et Traefik route via une route file-based (`/data/coolify/proxy/dynamic/<x>.yaml`
→ `host.docker.internal:PORT`) au lieu des labels du conteneur. Gain sécu : plus sur le réseau docker
partagé (pas d'accès latéral conteneur). Étape 2/3 (→ ploxion sur le bus → bion) = ensuite.

Pattern par conteneur : extraire le `.py` → `host-ploxions/<nom>/` ; unit systemd `ploxion-<nom>.service`
(127.0.0.1:PORT) ; route Traefik file-based + SSO ; `docker compose -f /bi0ns/ploxi0ns/<nom>/compose.*.yml down`.

Données runtime (recordings…) : hors du repo, dans `/home/debian/xerboxion-data/<nom>/` (jamais en git).

| ploxion | port | état | notes |
|---|---|---|---|
| filesystem | 3010 | ✅ migré (host systemd, conteneur retiré) | accès latéral retiré (finding #5) |
| tsoin | 3011 | ✅ migré | route tsoin.j0bot.ch→host, SSO |
| record | 3012 | ✅ migré | 12 recordings préservés → /home/debian/xerboxion-data/record |
| openscad | 8770 | ⏳ à venir | needs openscad+xvfb sur l'hôte (binaire lourd) |
| vscode | 8080 | ⏳ à venir | code-server (binaire IDE), --auth none → ajouter gate |

Restent infra-lourde (pas core-able en 12h, honnête) : Coolify (mgmt), les stores (postgres/mariadb/
neo4j/minio/redis/meili), gitea, repoverse-stack, le labo Laravel+mariadb. Migration profonde = recherche.
