# Run autonome — pendant Vallorbe (2026-06-23)

> José : « Enchaîne tout sans moi, je veux voir tout ce que t'as fait quand je reviens. »
> Tout est LIVE sur **xer.j0bot.ch** (ouvre la grille des apps ▦ du bureau) et servi par le cœur.
> Tout est committé+poussé sur **operational-core**.

## 🧱 La fondation : « tout dans le cœur »

- **Store de documents générique** dans le cœur (Rust, `serve.rs`) — `GET/POST/PUT/DELETE
  /store/:ns/:collection[/:id]`. Persisté (`state_dir/store/`), lectures publiques, écritures gardées
  par `X-Xer-Token` (**token fablab = `fablab-78280d7c1b5cbaed`**), émet `store.<ns>.<coll>.changed`
  sur le bus à chaque mutation (les UI se synchronisent). Buildé + déployé + vérifié. Réutilisable.

## 🔧 MakeLab — connecté au xerboxion (« amélioré au max »)

Ouvre **🔧 MakeLab** (`/px/makelab`) = le hub fablab (stats inventaire live + grille d'outils) :

| Ploxion | `/px/…` | Quoi |
|---|---|---|
| 📦 **Inventaire** | `inventaire` | articles/stock/emplacements, recherche, stock bas, ajout/édition |
| 📥 **Mouvements** | `mouvements` | entrées/sorties + ledger (porte unique d'écriture du stock) |
| 📷 **Scan** | `scan` | caméra QR/code-barres → article → ajustement rapide |
| 📌 **Boxion** | `boxion` | brochage ports+pins (Raspberry Pi 40 / ESP32), fonctions I2C/SPI/UART/PWM |
| 🔪 **Slicer** | `slicer` | trancher un STL (Kiri:Moto intégré) |
| 🧊 **3D** | (core `/3d`) | visualiseur 3D |
| 🔨 **Forge** | (app système) | idée → objet |
| ✏️ OpenSCAD / 🧱 Object | — | marqués « bientôt » (existent côté labo, à porter) |

**Token fablab** : bouton 🔑 dans inventaire/mouvements/scan. **Quand tu m'envoies le SQL**, je
l'importe dans `/store/inventaire/*` → ton vrai inventaire apparaît partout (les UI sont génériques).

## 📱 Android — le xer installable

Le xer est une **PWA** : sur ton tel → « ajouter à l'écran d'accueil » → app **xer plein écran**
(icône chill-tekk). Prêt pour tes tests mobile. Deeper (core on-device aarch64 / launcher natif) =
documenté dans la mémoire `prepare-terrain-vallorbe`.

## 🛰️ OSIRIS → divisé en ploxions+bions

Ouvre **🛰️ Veille** (`/px/veille`) = machine à veille : carte monde + **couches OSINT togglables où
chaque couche est un bion** (source publique normalisée). v1 : **séismes USGS en live** (sans clé,
288 séismes/24h au build). Archi `LAYERS` extensible (flights/fires/weather/CVE = ajouter un objet).

## 🏠 Home Assistant — terrain posé

Ouvre **🏠 Maison** (`/px/maison`) : connecte ton HA (URL + jeton longue-durée) → tes entités
(lumières/prises/capteurs/climat), allume/éteint, états live. (HA doit autoriser l'origine :
`http: cors_allowed_origins`.) Se mariera avec boxion + les device-bions.

## 🧩 Aussi (capacité)

Le bureau accepte qu'**un ploxion en ouvre un autre** (`postMessage {xer:'open'/'openCore'/'openApp'}`)
— c'est ce qui fait marcher le hub makelab.

## ⏭️ Reste (pour ton retour / ton input)

- **Importer ton SQL** makelab (dès que tu l'envoies).
- **Universal suite** : à cadrer (périmètre).
- **OSIRIS** : ajouter des couches (flights/fires/weather…) — trivial avec l'archi `LAYERS`.
- **Android on-device** : tester le core en aarch64 (Termux) si tu veux le local-first.
- **OpenSCAD/Object** ploxions natifs (porter du labo).
- **Fix non urgent** : borner `health_sweep` (cf. docs/network-as-ploxions.md) pour que le hang
  transitoire de `/events` ne revienne jamais.
