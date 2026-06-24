# warp-relay — relais aveugle E2E pour le /warp du coeur

Relais **content-blind** pour le `/warp` du web-heart : il deplace des octets
**opaques** (ciphertext AES-GCM + `roomId` opaque) entre deux navigateurs. Il ne
dechiffre rien, ne connait aucune cle privee, ne loggue **aucun** contenu, vit
**100% en RAM** avec TTL. Le coeur reste l'autorite (record/replay, crypto,
export `.heart`) et fonctionne **offline-first** : si ce relais tombe, rien ne
casse.

- Single-file Python 3.11 stdlib `asyncio`, zero dependance.
- Bind par defaut `127.0.0.1:8732` (Phase locale, rien d'expose).
- Exposition `warp.j0bot.ch` via Traefik **a valider par Jose** (Phase exposee).

## API (toutes reponses CORS `*`, `no-store`, `no-referrer`)

| methode | chemin | role |
|---|---|---|
| `GET`  | `/warp/healthz` | `{ok,rooms,boxes,rss_kb}` (jamais de contenu) |
| `POST` | `/warp/{roomId}/send` | publier `{ct}` chiffre (<=64 KiB) -> `{seq}` |
| `GET`  | `/warp/{roomId}/events?after=N` | SSE temps reel (backlog + live, heartbeat 25s) |
| `GET`  | `/warp/{roomId}/poll?after=N&wait=25` | long-poll de repli |
| `POST` | `/warp/box/{boxId}` | boite d'amorcage pubkeys `{ct}` (<=8 KiB) |
| `GET`  | `/warp/box/{boxId}?after=N` | recuperer la boite |

`roomId`/`boxId` : `[A-Za-z0-9_-]{16,64}`. TTL room 10 min, box 5 min ; rings
bornes (200/20) ; rate-limit token-bucket par IP ; `MAX_ROOMS/BOXES=2000` ;
`MemoryMax=64M`. WARP_LOG=quiet par defaut (rien sauf erreurs ; jamais `ct`).

## Lancer (local, reversible) — voir le champ `deploy` de la livraison.

Teardown total : `sudo systemctl disable --now warp-relay.service` puis
`sudo rm -f /etc/systemd/system/warp-relay.service /data/coolify/proxy/dynamic/warp.yaml`
et `sudo rm -rf /opt/warp`. Aucun residu (rien sur disque, rien en base).
