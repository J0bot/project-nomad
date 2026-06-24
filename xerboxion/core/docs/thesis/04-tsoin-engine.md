# Chapitre 4 — Le *tsoin engine* (contribution)

> Posé vs spéculatif : ce chapitre ne décrit **que l'implémenté et le mesurable**. La
> motivation cosmologique (chaos, oction, le paradoxe-moteur) est tenue à l'écart ici et
> traitée au chapitre 7, présentée comme heuristique de design, jamais comme preuve.

## 4.1 Définition : le tsoin

On appelle **tsoin** un couple `(générateur, résidu)` content-adressé qui capte un état ou
un instant de façon **régénérable** plutôt que stockée littéralement.

- Le **générateur** est une règle déterministe (un programme, une graine, une référence à
  un tsoin antérieur) qui *prédit* l'état.
- Le **résidu** est la différence minimale entre la prédiction du générateur et l'état réel
  — exactement ce que la règle ne prédit pas.

Un tsoin est donc une **compression alimentée par le réel** : on garde la règle (peu de
bits) et seulement l'écart (le réel non prédit). C'est l'analogue calculatoire du codage
prédictif : la valeur d'information d'un tsoin est sa *surprise* (§4.3).

Formellement, pour un état `s` et un générateur `g` produisant la prédiction `g()` :

```
residu(s, g) = s ⊖ g()              (différence minimale, §4.3)
tsoin        = (g, residu)
s            = g() ⊕ residu          (reconstruction exacte, §4.4)
```

La reconstruction est **sans perte et vérifiable** : `addr(reconstruct(tsoin)) == addr(s)`.

## 4.2 L'adressage génératif (`tsoin-store`)

Chaque tsoin est nommé par l'adresse de son contenu, pas par un chemin. L'implémentation
de référence utilise **`addr64` = FNV-1a sur 64 bits** :

```
addr64(octets):
    h = 0xcbf29ce484222325
    pour chaque octet b :
        h = (h XOR b) * 0x100000001b3   mod 2^64
    retourne h            # 16 caractères hexadécimaux
```

Propriétés exploitées (ce sont des propriétés **réelles** du content-addressing, pas des
postulats) :

1. **Déduplication exacte.** Deux tsoins identiques ont la même adresse : on ne les stocke
   qu'une fois. C'est la base de la couche collective (§4.7).
2. **Auto-vérification / inviolabilité.** On ne peut pas modifier un tsoin sans changer son
   adresse. *Toute action change l'adresse* — démontré sur le dépôt lui-même : un seul
   octet (`"test"` → `"test "`) donne une adresse totalement différente. C'est le « propre
   système de chiffrement » au sens de Git/IPFS/Merkle : la confiance ne vient pas d'une
   clé protégée mais de l'adresse qui *est* le contenu.
3. **Indépendance de l'ordre** (pour un ensemble de tsoins) : en triant les adresses avant
   de les ré-adresser, l'adresse de l'ensemble ne dépend pas de l'ordre d'insertion (§4.7,
   l'Epsylaeu).

> Note d'honnêteté : `addr64` n'est **pas** cryptographiquement résistante aux collisions
> (FNV n'est pas conçu pour ça). Pour le présent travail — un substrat de calcul
> rejouable, pas un système anti-adversaire — la résistance pré-image n'est pas requise ;
> un passage à BLAKE3/SHA-256 est trivial et discuté au chapitre 7 (limites).

## 4.3 Le résidu et la surprise (`diff`)

L'organe `diff` calcule le résidu minimal entre la prédiction et l'état, et en dérive une
mesure scalaire de **surprise**. Sur des états de taille fixe, le résidu se mesure par
**distance de Hamming** ; la surprise normalisée est

```
surprise(s, g) = popcount(s XOR g()) / |s|     ∈ [0, 1]
certitude      = 1 − surprise
```

`certitude = 1 − résidu` est le fil conducteur de tout le système : un tsoin parfait
(générateur qui prédit tout) a une surprise nulle ; un état imprévisible a une surprise
proche de 1. C'est aussi la sémantique du `?` du langage (chapitre 5) : un trou non résolu
part à surprise = 1, le Chaoxion le ramène vers 0.

## 4.4 Le générateur et le *replay* (`generator`)

`generator` est l'inverse de `diff` : à partir de `(g, residu)` il **régénère** l'état.
La garantie centrale, testée, est le **déterminisme** : rejouer le même tsoin produit
bit-à-bit le même état, sur n'importe quelle machine. C'est ce qui transforme un simple
enregistrement en *voyage* réversible : pour « revenir » à un instant, il suffit d'avoir
capté assez de résidu pour le régénérer fidèlement. Un tsoin plus gros (plus de résidu)
rend le retour plus complet — d'où la stratégie des **ultra-tsoins** (chapitre 6 : la
capture horaire content-adressée).

La rejouabilité déterministe est vérifiée par la propriété `addr(replay(tsoin)) ==
addr(état_original)` dans la suite de tests du SDK (chapitre 5).

## 4.5 La cohérence temporelle (`clock-coherence`)

Le temps du système n'est **pas** l'horloge murale mais la **cohérence** : un `GatedTick`
n'avance que lorsque les tsoins concernés sont mutuellement cohérents. Conséquence
pratique pour la rejouabilité : aucune dépendance à `wall-clock`, `Date.now()` ou à un
générateur d'aléa non graîné — ces sources briseraient le déterminisme. « Synchroniser au
maximum » (la veille de l'utilisateur) se formalise ici comme **amener tous les tsoins à la
cohérence** ; c'est la condition d'un replay multi-flux propre.

## 4.6 Le *player*

`player` déroule une **séquence** de tsoins (un flux) en appliquant `generator` pas à pas,
sous le contrôle de `clock-coherence`. C'est l'organe qui « joue » un enregistrement
multimodal : la même mécanique sert à rejouer une trace de calcul, une session, ou (cible
applicative) un instant capté.

## 4.7 Les bions partagés : *uncraft*, communs, et l'Epsylaeu

Un tsoin se **décompose** en sous-tsoins partagés — les **bions**. L'*uncraft* divise un
ploxion en bions jusqu'à atteindre un bion déjà présent ailleurs (un bloc partagé). Rendu
calculable (`xerb/commun.py`), cela donne deux niveaux de communauté entre tsoins :

- **littéral** : des *shingles* de k mots content-adressés ; un bion présent dans ≥2 tsoins
  est un tsoin commun, stocké une fois ;
- **conceptuel** : recouvrement d'ensembles de termes porteurs (Jaccard) — « se
  ressemblent » même formulés autrement.

Mesure sur le corpus du projet (72 tsoins, frontmatter exclu) : **dédup littéral ≈ 2 %**,
**une seule famille conceptuelle** — c'est-à-dire un corpus à **faible redondance / haute
information**, où les rares ressemblances retrouvées sont celles attendues (deux versions
d'une même recette ; un lien `[[ ]]` posé à la main entre le ❤️ et la dissociation,
retrouvé automatiquement). Les bions les plus communs sont les idées-socles (« comprimer =
faire de la place = sauter » ; « tout passe par le xion »).

Au sommet, l'**Epsylaeu** (`xerb/epsylaeu.py`) est l'adresse de l'ensemble trié des
adresses de tous les tsoins : la seule adresse qui dépend de **tous** les tsoins. Un seul
tsoin change → l'Epsylaeu change. C'est le « tsoin des tsoins », et la cible de
déduplication d'un ultra-tsoin **collectif** (Nexus, chapitre 8).

## 4.8 Composition fractale : le *protocol-bion* (démonstration)

La thèse pose que le substrat **compose fractalement** (`bion → ploxion → boxion`). On en
donne une démonstration *implémentée et vivante* avec le réseau-comme-ploxions. Un unique
**protocol-bion** (`ploxions/sdk/src/protocol.rs` : `struct ProtocolDef` + macro
`protocol_ploxion!`) sert de générateur ; chaque protocole réseau n'est plus qu'un
**résidu** (son nom, son numéro, sa couche, son transport) injecté dans ce générateur :

```rust
const DEF: ProtocolDef = ProtocolDef { name: "tcp", number: 6, transport: "-", layer: 4, brief: "..." };
ploxion_sdk::protocol_ploxion!(DEF);
```

À partir de ce seul bion, **cinq ploxions de protocole** (tcp, udp, dns, http, icmp) ont
été générés, compilés en `wasm32-unknown-unknown` (~39 Ko chacun), chargés sur l'hôte et
testés **en direct** sur le bus (`net.<proto>.in → net.<proto>.out`, avec gravure d'un
tsoin `proto:<nom>:<numéro>`). Le coût marginal d'un protocole supplémentaire est un fichier
de quelques lignes : *le générateur est partagé, seul le résidu varie*. C'est la grammaire
du chapitre 3 vérifiée empiriquement, et le chemin vers « tout est un ploxion ».

## 4.9 Synthèse du chapitre

Le *tsoin engine* réalise une unité de calcul qui n'est pas l'instruction mais le **tsoin** :
content-adressé (§4.2), mesuré par sa surprise (§4.3), régénéré de façon déterministe et
donc **rejouable** (§4.4–4.6), décomposable en **bions partagés** déduplicables (§4.7), et
**composable fractalement** comme le montre le protocol-bion (§4.8). Le chapitre 5 détaille
l'implémentation (l'hôte WASM, le bus PLC, le SDK, le langage `xerb`) ; le chapitre 6 en
donne l'évaluation (le système tourne, le budget 16 Go, la portabilité serveur→MCU).
