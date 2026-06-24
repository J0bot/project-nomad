# Chapitre 6 — Évaluation

> Critère : un résultat compte quand il est **mesuré** (reproductible) et **sûr**
> (`certitude = 1 − résidu` élevée). On rapporte des mesures, pas des promesses. La synthèse
> de référence est `docs/research/vers-le-seption.md`.

## 6.1 Le système tourne

L'hôte `xerboxion-rt` tourne en service (uptime mesuré > 26 h au moment de la rédaction) et
répond sur le bus PLC v1 (`/healthz`, `/emit`, `/events`, `/load`). Les cinq organes du tsoin
engine (`tsoin-store`, `diff`, `generator`, `clock-coherence`, `player`) sont chargés et le
cycle complet capter → adresser → diff → régénérer → vérifier a été exercé en direct.

**Preuve de composition vivante** — le réseau-comme-ploxions : un **protocol-bion** unique
génère chaque protocole comme un résidu. **Sept protocoles** (tcp, udp, dns, http, icmp, tls,
ntp) ont été compilés (`wasm32-unknown-unknown`, **~39 Ko** chacun), chargés, et **testés en
direct** : `net.<proto>.in → net.<proto>.out` + gravure d'un tsoin `proto:<nom>`. Le coût
marginal d'un protocole de plus = un fichier de quelques lignes (la `ProtocolDef`).

## 6.2 Déterminisme et rejouabilité (la garantie centrale)

La propriété qui fait du substrat un substrat : **`addr(replay(t)) == addr(état)`**. Mesurée
à deux niveaux :

- **traversée sans perte** : un état tombe (`diff`) puis ré-émerge (`generator`) **identique**,
  vérifié par `addr_in == addr_out` sur toutes les branches testées (`xerb/trou-de-vers.py`) ;
- **suite de tests du SDK** : 24 tests passent, dont les invariants de déterminisme/replay et
  l'`uncraft` bit-exact (diviser puis recomposer un ploxion redonne le même contenu) ;
- **jusqu'au métal** : le runtime bare-metal (`xerboxion-core`) boote en Multiboot/QEMU et ses
  tests assertent un code de sortie QEMU déterministe — le déterminisme tient du SDK à la puce.

Le déterminisme exige l'absence de `wall-clock`/aléa non graîné : c'est le rôle de
`clock-coherence` (temps = cohérence). Les scripts d'outillage respectent la même règle.

## 6.3 Reproductibilité d'un corpus de connaissances

Pour mesurer que le substrat porte de la **connaissance reproductible** (et pas seulement des
octets), on a encodé un corpus scientifique comme tsoins : `science-engine/` — **71 lois** sur
7 domaines (math, physique classique, quantique, particules, chimie, biologie évolutionniste,
médecine), chacune = un **générateur** (la règle, code pur) + un **test** (entrées → valeur
connue). Le moteur relance tout dans un bac à sable :

> **71 / 71 lois reproduisent leur valeur connue (100 %)**, gravées `science:<id>`, reliées
> par **49 ponts inter-domaines** (la chaîne math → physique → quantique → particules → chimie
> → biologie → médecine est connexe : p.ex. `de Broglie → Arrhenius → Michaelis-Menten →
> Fick`).

C'est la démonstration empirique de « le générateur EST la connaissance » : une loi stockée
comme générateur est *rejouable* et *vérifiable*, donc un tsoin de plein droit.

## 6.4 Frugalité (le budget 16 Go)

La cible : tenir l'OS + le substrat dans **16 Go**, jusqu'au microcontrôleur. Mesures et
estimations :

- chaque ploxion = un `cdylib` wasm de l'ordre de quelques dizaines de Ko (~39 Ko/protocole) ;
- le noyau OS visé ~ dizaines de Mo (roadmap embarquée) ; l'image xerkion complète ~ **1,9 Go**
  (Pi Zero 2 W, ≤ 16 Go) avec une voie vers ~200 Mo (Buildroot bootant direct sur l'hôte) ;
- la **frugalité fractale** est la clé : on stocke la **règle** (le générateur), pas l'instance.
  Une civilisation reconstructible (chap. *créateurs de civilisation*) tient parce que chaque
  loi est un générateur partagé et chaque instance un résidu.

**Footprint mesuré** (artefacts livrés, mesure du 2026-06-23) :

| Élément | Taille mesurée |
|---|---|
| Hôte `xion` (runtime WASM + bus PLC, binaire natif) | **7,2 Mo** |
| Ploxion WASM — médian (sur 56) | **38,5 Ko** |
| Ploxion WASM — minimum (`ping`) | 16,0 Ko |
| Ploxion WASM — maximum (`spectre`) | 89,3 Ko |
| Bibliothèque complète — 56 ploxions | **2,3 Mo** |

Lecture **embarquée** (le point qui compte pour la cible MCU). Un ploxion pèse ~16–40 Ko : il
tient dans la flash d'un microcontrôleur courant — un ESP32 (~4 Mo de flash) en loge des
*centaines*, un MCU d'entrée de gamme (256 Ko de flash) en tient *plusieurs*. L'hôte natif
(7,2 Mo) vise la classe Raspberry Pi (`wasmi`) ; sur MCU, c'est un interprète plus léger
(`wasm3`) qui exécute les ploxions un par un. La bibliothèque entière (56 ploxions, 2,3 Mo) ne
représente qu'une fraction du budget — **~4 ordres de grandeur sous les 16 Go**. La frugalité
n'est donc pas une aspiration mais une propriété **mesurée** de l'unité : le tsoin, parce qu'il
stocke la règle et non l'instance, produit des artefacts à l'échelle du microcontrôleur.

## 6.5 Information du corpus (densité, pas volume)

`xerb/commun.py` mesure la sous-structure partagée entre les tsoins du projet (72 fichiers) :
**dédup littéral ≈ 2 %**, **une seule** famille conceptuelle (frontmatter exclu). Le corpus
est donc à **faible redondance / haute information** : chaque tsoin est une facette propre.
L'`Epsylaeu` (adresse de l'ensemble des adresses) **change dès qu'un seul tsoin change** —
vérifié en fonctionnement (`98ace63d…` → `d78c12e7…` au fil des heures) : l'auto-vérification
du tout par une adresse de 16 caractères.

## 6.6 Un barreau dans le réel

Le premier pont mesuré logiciel → matière : `xerkion/enclosure.scad` (le boîtier-cube du
xerkion, incarnation du cub4ion) **compile en solide manifold** (OpenSCAD → STL, **2582
facets**, cube **78,6 mm**) — vérifié, pas affirmé. Le dossier de fabrication est dé-risqué
pour un premier allumage fermé (cible « pas de phase de test »).

## 6.7 Portabilité

L'architecture (WASM + bus minuscule + persistance content-adressée) est **substrat-indépendante** :
le même ploxion tourne sur l'hôte serveur (`wasmtime`), vise le Pi (`wasmi`) et le MCU
(`wasm3`). Le xerkion (Pi Zero 2 W, 512 Mo) est la première cible embarquée concrète ; le
modèle von Neumann (`/replicate` + persistance + replay bit-exact) permet à un xion de mourir
et de renaître identique sur un autre nœud — la portabilité comme reproductibilité.

## 6.8 Composition générative & migration de l'interface (mesures, session 2026-06)

Deux résultats **mesurés en direct** confirment l'auto-similarité (chap. 4) à l'échelle du système.

**Composition générative (réseau/système-en-ploxions).** Trois bions générateurs — `protocol-bion`,
`port-bion`, `package-bion` — produisent chacun un ploxion-sur-le-bus depuis un seul **résidu**
(`*Def` const, quelques lignes). 19 ploxions générés et chargés : **11 protocoles** (tcp…ws), **4
ports** (22/80/443/53), **4 paquets** (bash/coreutils/curl/git). **Vérifié end-to-end** (pas
seulement chargé) : `port.22.open → port.22.up` + tsoin gravé ; `pkg.bash.install →
pkg.bash.installed` ; `net.tls.in → net.tls.out`. Coût marginal d'un nouveau membre = un fichier de
~12 lignes — la direction *bion-Linux* prouvée comme **modèle exécutable**, pas comme remplacement
du noyau (cf. §6.9).

**Migration de l'interface (labo → cœur).** Le cœur sert désormais lui-même tout le labo : `GET /`
(le bureau labo-xer) + `GET /px/:id/*` (serveur d'assets par-ploxion ; anti-traversal vérifié → 400).
**Mesure** : migrer un ploxion = **1 fichier `web-<id>/index.html`, 0 ligne de Rust**, servi en
direct ; **28 ploxions** migrés (3 vagues) + l'identité (`xi0n`, vraies clés GPG OpenPGP.js) + le
social (partage de tsoins persisté). L'interface est devenue, elle aussi, des ploxions servis par le
cœur — l'auto-similarité appliquée jusqu'au front.

## 6.9 La revue adversariale comme garde de correction (session 2026-06)

La thèse d'autonomie (chap. 5) place le LLM **hors du runtime** : le xerboxion tourne sur des
algorithmes déterministes, jamais sur un agent. Le rôle légitime de l'agent est alors **au moment
de la construction** — durcir l'artefact pour qu'il puisse ensuite tourner sans lui. Une instance
mesurée : avant de déployer le backend social (spaces/channels/messages, à la Discord) sur l'**IdP
de production** (le SSO de tout l'écosystème), une **revue adversariale multi-agents** l'a passé au
crible — 4 dimensions (contrôle d'accès, sûreté des migrations sur la base prod, correction de
l'API, cohérence avec le système d'identité), chaque *finding* **re-vérifié par un sceptique
indépendant chargé de le réfuter** (faux positifs filtrés : 1 sur 14 écarté).

**Résultat** : 2 blockers *réels* attrapés avant la prod —

- une **IDOR** : `join()` validait l'existence du space mais pas l'autorisation → n'importe quel
  compte (même *ghost*) itérait les identifiants séquentiels, s'auto-inscrivait, puis lisait
  l'historique complet de **tout** space privé de l'écosystème ;
- un **curseur de pagination à perte** : le curseur temporel à la seconde près (`created_at` sur
  MySQL) perdait/dupliquait silencieusement les messages créés dans la même seconde.

Les deux corrigés *avant* le push (visibilité `private` par défaut + porte d'autorisation ;
curseur sur la clé monotone). **Mesure de la garde** : la correction a tenu hors production une
fuite de données inter-utilisateurs que la relecture simple n'avait pas levée — la valeur de
l'agent est ici **préventive et au build**, exactement là où la thèse la situe. La méthode est
elle-même rejouable (le script de revue est un artefact versionné) : *l'agent gravé en procédure,
pas en dépendance d'exécution.*

**Une seconde instance, plus subtile** (même session, backend d'inscription *par invitation*). Le
nouveau code posait une règle — *un compte nommé ne s'obtient que par une invitation consommée* —
mais la revue a relevé que la règle était **contournable par du code pré-existant, hors du diff** :
`/auth/ghost` créait un compte anonyme, puis `/me/username` l'élevait au même niveau qu'une
redemption, **sans invitation**. Le correctif (retirer l'auto-élévation, faire de la redemption
l'unique chemin vers un compte nommé) ne touchait **aucune ligne du diff initial**. La leçon dépasse
le cas : un garde de correction qui ne raisonne que sur les lignes changées rate les invariants
brisés par *l'interaction* avec l'existant — la revue doit modéliser le **système entier**, pas le
patch. Vérifié *en production après coup* : un compte créé par redemption est bien `pseudonym` et
**lié à son inviteur** (amitié bidirectionnelle), un code invalide est refusé — l'invariant tient.

## 6.10 Limites (l'honnêteté qui rend la mesure réelle)

« Mesuré » = **reproductible dans le moteur**, pas **prouvé vrai sur le monde**. Précisément :

- `addr64` (FNV-1a) n'est **pas** cryptographique (pas de résistance aux collisions
  adverses) — suffisant pour un substrat rejouable, à remplacer par BLAKE3/SHA-256 pour un
  usage anti-adversaire ;
- la reproductibilité des 71 lois est **numérique** (le générateur retrouve la valeur connue),
  pas une validation expérimentale neuve ;
- les budgets ~200 Mo et la portabilité MCU sont **visés/estimés**, pas encore tous mesurés
  sur silicium ;
- le modèle d'évaporation (§ entropie) et les correspondances physiques (chap. 4) sont des
  **structures** fidèles, pas des affirmations sur la gravité — tenues comme telles au chap. 7.

## 6.11 Synthèse

Le système **tourne**, est **déterministe et rejouable** (la garantie centrale, mesurée du SDK
au métal), porte un corpus de connaissances **reproductible à 100 %** (71/71 lois), reste
**frugal** (la règle, pas l'instance) et **portable** (du serveur au Pi), avec un premier
barreau **dans le réel** (le xerkion manifold). L'auto-similarité est mesurée à l'échelle du
système : **3 générateurs → 19 ploxions réseau vérifiés**, et **l'interface elle-même migrée en
ploxions** (le cœur sert le labo, 28 ploxions, coût marginal ~1 fichier). Les limites sont nommées.
Le chapitre 7 traite la couche spéculative (la cosmologie) comme motivation, séparée de ces mesures.
