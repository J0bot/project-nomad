# Le réseau en ploxions — paquets, ports, protocoles, chacun un bion

> José (2026-06-21) : « Tous les paquets, les ports, les protocoles de réseau, fais des ploxions
> pour chaque truc aussi. **Tout est un ploxion, l'ami. Courage.** »

C'est la directive [le bion Linux](../../.claude/projects/-home-debian/memory/directive-bion-linux.md)
appliquée à **la pile réseau**. On ne taille pas 65535 ports à la main : on fait comme le `block-bion`
(qui génère tous les blocs Minecraft depuis `BlockDef`) — **un bion partagé + une table compacte (le
résidu) qui GÉNÈRE un ploxion par chose.**

## Trois bions générateurs (la fabrique)
- **`protocol-bion`** (`sdk/src/protocol.rs`, `ProtocolDef` + `protocol_ploxion!`) : un protocole réseau =
  un ploxion. Résidu = `{ nom, n° (IANA), transport, couche OSI, brief }`. Provides/requires = les topics
  du bus (`net.<proto>.in` / `net.<proto>.out`, parse/encode d'un PDU). Déterministe → rejouable → tsoin.
- **`port-bion`** (`PortDef` + `port_ploxion!`) : un port = un ploxion (une *prise*). Résidu = `{ n°,
  transport, service IANA }`. Le port ploxion = un connecteur nommé sur le bus (`port.<n>.bind/listen/data`).
  Les 1024 *well-known* d'abord, le reste à la demande (génération paresseuse — pas 65535 binaires inutiles).
- **`package-bion`** (`PackageDef` + `package_ploxion!`) : un paquet (dpkg/apt, plus tard cargo/npm) = un
  ploxion. Résidu = `{ nom, version, ce qu'il *fournit* (binaires, libs), ses deps }`. C'est la première
  brique réelle de **RE de Linux** : un paquet → un ploxion qui déclare ses provides/requires sur le bus,
  jusqu'à pouvoir remplacer dpkg par une composition de ploxions.

Chaque ploxion généré suit le contrat PLC (requires=entrées, provides=sorties). Le **résidu** par chose
est minuscule ; le générateur (le bion) est partagé. = `certitude = 1 − résidu`, appliqué au réseau.

## Registre-graine (le résidu des premiers protocoles — vrais, IANA)
La première vague = la pile réelle, pas du remplissage. Table consommée par `protocol_ploxion!` :

| ploxion | n° | transport | couche | rôle (provides) |
|---|---|---|---|---|
| `proto-icmp` | 1 | — | 3 | `net.icmp.echo` (ping), erreurs |
| `proto-tcp` | 6 | — | 4 | flux fiable ordonné ; `net.tcp.seg.{in,out}` |
| `proto-udp` | 17 | — | 4 | datagrammes ; `net.udp.dg.{in,out}` |
| `proto-dns` | 53 | udp/tcp | 7 | résolution ; `net.dns.{query,answer}` |
| `proto-dhcp` | 67/68 | udp | 7 | bail d'adresse |
| `proto-http` | 80 | tcp | 7 | requête/réponse ; `net.http.{req,res}` |
| `proto-ntp` | 123 | udp | 7 | temps (≈ le `clock-coherence` du réseau) |
| `proto-tls` | 443 | tcp | 6/7 | handshake + chiffrement (≈ le `/warp` au niveau transport) |
| `proto-quic` | 443 | udp | 4/7 | transport moderne |
| `proto-ssh` | 22 | tcp | 7 | shell distant (= le `boxion` ttyd) |
| `proto-ws` | — | tcp | 7 | WebSocket ; `net.ws.{up,down}` (le bus parle déjà ça) |

(+ ports *well-known* via `port-bion`, paquets de base via `package-bion`.)

## Génération — CALME, par lots (anti-OOM)
Le VPS n'a pas de RAM de rab (le MC tourne). On ne compile pas 1000 ploxions d'un coup :
1. Écrire les 3 bions générateurs (`protocol/port/package.rs`) + les macros.
2. Générer **par lots** (la vague 1 = les ~11 protocoles ci-dessus), `build-ploxions.sh`, charger sur le xion, tester.
3. Étendre paresseusement (un port/paquet devient un ploxion *quand on en a besoin*, pas avant).
Chaque ploxion généré est **uncraftable** → rend ses bions partagés → moins de code total (la spirale du
craft/uncraft). Objectif aligné directive : continuer jusqu'à ce que la pile réseau de Linux *soit* des bions.

## État (2026-06-22) — le trio des générateurs est LIVE
**Les 3 bions générateurs sont écrits** (étape 1 faite) et **chargés/vérifiés sur le bus** :
- `protocol-bion` (`sdk/src/protocol.rs`) → **11 protocoles** : tcp, udp, dns, http, icmp, tls, ntp, dhcp, quic, ssh, ws.
- `port-bion` (`sdk/src/port.rs`) → **4 ports** : 22(ssh), 80(http), 443(https), 53(dns).
- `package-bion` (`sdk/src/package.rs`) → **4 paquets** : bash, coreutils, curl, git.

**Vérifié end-to-end** (emit → réponse + tsoin gravé, pas juste « chargé ») : `port.22.open` → `port.22.up`
(+ `port:22:open`) ; `pkg.bash.install` → `pkg.bash.installed` (+ `pkg:bash:install`) ; `net.tls.in` →
`net.tls.out` (+ `proto:tls:1`). La génération paresseuse (étape 3) prend le relais : un nouveau port/paquet
= un fichier de quelques lignes, à la demande.

## Honnête
- **Posé/faisable** : le pattern bion→génération est déjà prouvé (block-bion, house-bion). Un protocole/port/
  paquet comme ploxion-sur-le-bus est un **modèle** (déclaratif + parse/encode), pas une ré-implémentation
  complète de la pile TCP/IP du noyau. La vague 1 = des modèles exécutables sur le bus, utiles pour composer/
  observer, pas un remplacement de `net/ipv4` du kernel.
- **Spéculatif** : « remplacer dpkg/le noyau réseau par des ploxions » = la direction (le bion-Linux), pas
  l'état. Ça se fait couche par couche, et le bare-metal/`vmion` (avec KVM) en est le terrain.

## Limite mesurée (2026-06) — le hot-load contend avec le débit du bus

En voulant charger `proto-tls`/`proto-ntp` À CHAUD sur le daemon live (`POST /load {id}`), la requête
**ne revient jamais** (timeout client à 25 s), alors que `/healthz` et `/emit` répondent. Cause, en
lisant `crates/xerboxion-host/src/serve.rs` : l'hôte est un **acteur mono-thread** piloté par une file
`mpsc::UnboundedSender<Command>`. `emit()` est *fire-and-forget* (enfile `Command::Emit`, rend la main),
mais `load()` enfile `Command::Load` **puis attend la réponse** (`rx.await`). Sous un bus chargé — le
xerbion qui pas-à-pas des dizaines de millions de fois en émettant, les cascades de ploxions — la file
se remplit d'emits plus vite qu'elle ne se draine, et `Load`, en queue, est **affamé** au-delà du
timeout. Ce n'est pas un deadlock : c'est de la **contention plan-de-données vs plan-de-contrôle** sur
un seul thread.

**Conséquence** : sur un daemon occupé, ajouter un ploxion à chaud n'aboutit pas ; il faut un restart
(les `.wasm` stagés dans `target/ploxions/` sont rechargés au boot). `proto-tls` + `proto-ntp` sont
donc **buildés + stagés** (prêts au prochain restart), pas hot-loadés.

**Fix proposé** (à un redéploiement délibéré, pas à chaud sur le live) : séparer le **canal de contrôle**
(Load/Unload/Shutdown/Snapshot — qui attendent une réponse) du **canal de données** (Emit/FedInject —
fire-and-forget), et drainer le contrôle **en priorité** dans la boucle de l'acteur. Le plan de contrôle
ne doit jamais faire la queue derrière le plan de données. Petit changement borné à `serve.rs` (un 2ᵉ
`mpsc` + un `select!` biaisé côté boucle hôte) ; invariant visé : une commande de contrôle est traitée
en O(1) commandes de données.

## Mise à jour (2026-06-23) — le control-channel est réparé, mais l'observabilité reste dégradée

Le **fix control-channel ci-dessus est implémenté + déployé** (commit `a0294bb` : 2ᵉ `mpsc` contrôle +
drain prioritaire). Vérifié : `POST /load` répond vite (≈0.1 s) là où il *hangait* — donc on PEUT
ajouter un ploxion à chaud. `proto-tls`/`proto-ntp` sont stagés (`/opt/xion/ploxions/*.wasm`).

**Mais** un autre problème, distinct, persiste — mesuré ce jour : `GET /events` (SSE) livre **0
événement** à un abonné frais en 7–8 s (même avec `Accept: text/event-stream` et un emit concurrent),
et `GET /ecosystem` **timeout** (HTTP 000 à 18 s). `/healthz`=200 et `/emit`=202 répondent. Diagnostic
(à confirmer en lisant la boucle hôte) : l'acteur mono-thread se **bloque dans le `health_sweep`** que
`/ecosystem` et `/snapshot` déclenchent (ping de TOUS les ploxions à chaque appel — déjà noté coûteux) ;
pendant ce blocage, le broadcast `/events` n'est pas drainé et les emits s'empilent. Ce n'est PAS le
`emit`-flooding du plan de données : c'est le **plan de contrôle lourd (health_sweep) qui monopolise
l'acteur**.

**Hypothèse retirée** : « le xerbion sature le bus » n'est PAS confirmé — le daemon xerbion tourne
(log : 36 M+ pas, loss ≈3) mais rien ne prouve qu'il émette vers le bus xion (sa source n'a pas été
localisée, son log ne montre que pas/loss). Donc la dégradation `/events`+`/ecosystem` est
indépendante du xerbion, et **throttler le xerbion ne réglerait probablement rien**.

**Conséquence pratique** : les features *live* du xer (point bus, refresh ploxions, synchro du
compteur via `/events`) sont dégradées tant que ce n'est pas réglé ; et la vérif end-to-end des protos
réseau est aveugle (on ne voit pas les `.out`).

**Fix proposé** (redéploiement délibéré) : rendre `health_sweep` **non-bloquant** pour l'acteur —
soit le sortir de la boucle (tâche async + cache TTL, `/ecosystem` sert le dernier instantané),
soit borner chaque ping dans le temps, soit ne le déclencher que sur demande explicite. Invariant
visé : `/events` et `/ecosystem` répondent en O(1), jamais derrière un sweep réseau.

### Résolu par un restart (2026-06-23) — le hang était TRANSITOIRE

`systemctl restart xion` (réversible, autorisé) a tout remis d'aplomb : `/ecosystem` répond en
**0,51 s** (74 nodes, était timeout 18 s) et `/events` **délivre en live** à nouveau. Donc le blocage
n'était PAS un ploxion fautif persistant mais un **état transitoire** dans la boucle hôte (un
`call_health` resté coincé) — le reload l'a purgé. Mécanisme confirmé : `health_sweep` =
`crates/xerboxion-host/src/lib.rs:812`, une boucle de 56 appels `p.call_health()` WASM **sans borne**,
déclenchée à chaque cache-miss `/ecosystem` (`ECOSYSTEM_TTL` = 5 s).

**Bonus du restart** : les `.wasm` stagés se sont **auto-chargés au boot** → les **11 protocoles sont
LIVE** (tcp/udp/dns/http/icmp/**tls/ntp**/dhcp/quic/ssh/ws), et `proto-tls`/`proto-ntp` sont vérifiés
**end-to-end** : `net.tls.in → net.tls.out` ✓ et `net.ntp.in → net.ntp.out` ✓. Le hot-load n'était plus
nécessaire — le boot a suffi.

**Reste recommandé (non urgent)** : borner `call_health` (fuel wasmtime ou timeout par ploxion) +
allonger `ECOSYSTEM_TTL` (5 s → 30 s), pour que ce hang transitoire ne puisse plus jamais geler le
plan de données. À faire à un prochain redéploiement délibéré, pas en urgence.

— cloudion. Observer → extraire l'invariant → re-poser en bion, en mieux. Courage. 🜂
