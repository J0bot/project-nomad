# Les couches d'accès, le bion USB, et le hotswap — l'identité du xerboxion

> José (2026-06-21) : « hotswap avec des clés USB qui ont le bion d'auth. Tant que le bion est
> branché, tous les accès ; sinon le SSO ; sinon le lien xi0n — différentes couches. Mais le
> xerboxion, c'est quand on l'utilise *uniquement sur la clé USB* : tout tourne depuis la clé.
> L'USB est le bion qui devient boxion dès qu'il est branché. Tout en hotswap, oklm. Et surtout
> un **ploxion imprimante** — c'est le ploxion le pire. »

## Les couches d'accès (de la plus basse à la plus haute)
On ne choisit pas UNE auth : on **empile** des couches, et l'accès = la plus haute disponible.

| couche | preuve | accès | état |
|---|---|---|---|
| **0 · lien xi0n** | un LIEN-capability (le lien EST la clé, pas de login) | l'instance, le `/talk`, les bions | **LIVE** (`xi0n-hub.py`) |
| **1 · compte SSO** | un login j0bot (identité authentifiée) | + le labo, ses données | existe (SSO j0bot) |
| **2 · bion USB** | **possession physique** de la clé | **TOUS** les accès | à bâtir |
| **∞ · xerboxion pur** | l'OS tourne **depuis la clé** | tout, hors-VPS | la cible |

Dégradation gracieuse : **bion USB branché → full** ; débranché → on **retombe** sur le SSO, ou à
défaut sur le lien xi0n. Jamais de mur : juste moins d'accès. (= le modèle des couches GPG : tout
automatique, rien dévoilé sauf ce qu'il faut, cf [[heart-bion-giga-tsoin]] / le /warp.)

## Le bion USB = le bion qui devient boxion
La clé USB de José = « mon bion du Josion », l'OS entier ≤ 16 Go. Au branchement, ce **bion** monte
et **devient un boxion** (bion + rien + xion = un serveur qui boote et tourne). Le **xerboxion pur**,
c'est quand on n'utilise QUE ça : tout (les ploxions, la machine à tsoins, l'auth) tourne depuis la
clé, sans dépendre du VPS ni du labo. C'est l'aboutissement de [[directive-bion-linux]] : l'OS en
bions, sur la clé, flashable. (Le terrain existe déjà : `installers/make-usb.sh`, `install.sh`, `flash.sh`.)

## Hotswap — oklm
- **L'auth en hotswap** : un `udev`/watcher détecte le bion USB (par son empreinte, pas son montage
  brut) → bascule la couche d'accès à chaud, sans relogin. Débranche → retombe, sans rien casser.
- **Les fichiers/dossiers en hotswap** : monter/démonter des bions-données à chaud ; un dossier =
  un bion qu'on branche/débranche dans le boxion courant (le `tsoin-store` content-adressé rend ça
  sûr : on régénère, on ne perd pas). « hotswap des fichiers ou des dossiers, oklm chill ».
- Règle : tout device = un **device-bion** qui s'annonce sur le bus au branchement (`dev.<id>.up`)
  et s'en retire au débranchement (`dev.<id>.down`). Le hotswap = pub/sub, pas du montage fragile.

## Le ploxion imprimante — le boss
> « surtout un ploxion imprimante, c'est le ploxion le pire. »

L'imprimante = l'enfer historique de l'informatique (CUPS, IPP, drivers, files d'attente maudites).
**Si on ploxionise l'imprimante, on a prouvé que « tout est un ploxion » tient même sur le pire.**
- `printer-bion` (sur le modèle du `protocol-bion`) : un ploxion qui parle **IPP** (Internet Printing
  Protocol) — déjà un protocole, donc déjà dans la lignée [[directive-bion-linux]] (réseau → ploxions).
  Provides `printer.<id>.job` / requires `printer.<id>.print` ; hotswap comme un device-bion.
- Wrap CUPS/IPP en bus, jamais de driver dans le cœur : le ploxion traduit `print(tsoin)` → IPP → la
  bécane. L'imprimante devient un boxion-périphérique, hotswappable comme la clé.
- C'est le **test final** de la grammaire : le jour où l'imprimante chill, le xerboxion a gagné.

## Honnête
- **Posé/faisable** : la couche 0 (lien xi0n) est LIVE. Le modèle de couches + le device-bion (pub/sub
  hotplug) est de l'architecture saine et implémentable.
- **Branche bare-metal / physique** (comme le `vmion`) : le bion-USB-qui-boote, le hotswap `udev`, et
  le `printer-bion` réel demandent le **matériel de José** (sa clé, son PC, une imprimante) — pas ce
  VPS. À faire quand on est sur le terrain, après la fusion. Ici = la spec.

— cloudion. L'accès en couches, l'USB qui devient boxion, et l'imprimante comme dernier dragon. 🜂
