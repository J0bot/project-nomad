# XERKION #1 — Le premier Kion physique

*Le `cub4ion` qui sort de l'écran : un cube qu'on tourne dans la main et qui tourne le tesseract du réel.*

**cloudion · dossier de fabrication v1.0 (de-risqué, fabricable sans phase de test)**

---

## Résumé

Le **xerkion** est le premier **bion physique** du XERB0XI0N : un cube imprimé FDM qui matérialise le ploxion `web-cub4ion`. À l'écran, le `cub4ion` est un tesseract Canvas2D dont l'angle 4D `rZW` choisit la scène (plage / mer / espace) et dont un levier Minecraft lance la playlist Spotify de José. Le xerkion remplace le doigt par le réel : **on tourne le cube, l'IMU lit la face en haut, la scène change ; on bascule le levier, la musique part.**

Matériel : **Raspberry Pi Zero 2 W** (hôte du daemon xion + renderer plein écran), **écran carré ST7789 SPI**, **accéléromètre LSM6DS3 I2C**, **ampli I2S MAX98357A + HP**, **PiSugar 3** (batterie/alim 2,5 A). Tout sur une **carte porteuse (HAT) PCBway**, dans un cube **PETG paramétrique** (~70 mm pour le 1,54").

Ce dossier est écrit pour la règle de José : **« direct le truc qui fonctionne, pas de phase de test »**. Trois pièges qui auraient cassé le premier allumage fermé ont été **corrigés en amont** :

1. **La pile d'affichage se branche vraiment sur l'écran SPI** : on rend en **DRM via `panel-mipi-dbi-spi`** (vraie carte DRM + connecteur ST7789) que `cog -P drm` peut cibler. Plus de seam fbtft-vs-DRM.
2. **La scène est mappée sur 6 faces discrètes** (axe de gravité dominant), **pas sur un yaw** : un accéléromètre 6 axes le donne de façon **déterministe** (le yaw est inobservable sans magnétomètre, il dériverait).
3. **Le pont logiciel est écrit AVANT le makelab** : route `/cub4ion` ajoutée au daemon, pont `xerkion-tilt` (I2C → bus + WebSocket), patch de la page (listener SSE/WS + flag `imuActive` qui **coupe la dérive autonome** `rZW+=0.0016`). Ces trois pièces **n'existent pas encore** dans le repo : du **travail à faire**, testé sur table (jalon **D-soft**) avant fermeture.

Le tout tient dans **512 Mo de RAM**, image gravée **~1,9 Go** (≤16 Go), autonomie **~5,5 h** (PiSugar 3 Plus 5000 mAh, conso réelle ~600 mA mesurée, pas idle).

---

## 1. Concept — le Kion dans le réel

La grammaire du xion : *« la forme EST la nature »* → **kion = cube**. Le `cub4ion` est un tesseract logiciel ; son incarnation fidèle est un **vrai cube**. Tourner le cube = tourner le cub4ion. C'est un **tsoin physique** qu'on tient dans la main.

**Ancrage code réel** (`web-cub4ion/index.html`, lu ligne à ligne) :
- Scène choisie par `rZW` (l.78) : `const p=((rZW/(Math.PI*2))%1+1)%1*3;` — aujourd'hui nourri par le **drag** (l.160) ET une **dérive autonome** `rZW+=0.0016` chaque frame (l.174).
- Levier → `toggle()` (l.149) → ON/OFF + iframe Spotify `5rZDHBR8tLni1xp8FlRMjW` (l.151).
- **Aucun listener réseau** (pas d'EventSource/WebSocket/fetch).

**Décision de mapping : 6 faces discrètes** (seul modèle déterministe avec un capteur 6 axes) :

| Face en haut (g dominant) | Scène |
|---|---|
| ±Z (face écran) | plage |
| ±Y (face avant) | mer |
| ±X (face droite) | espace |

Le pont calcule la face dominante puis **fixe `rZW`** ∈ {0, 2π/3, 4π/3} (hystérésis anti-clignotement) et **désactive la dérive autonome** quand l'IMU pilote. Tourner le cube → nouvelle face → nouvelle scène, sans creep.

> Pas de yaw : l'accéléromètre donne le tilt mais le **yaw est inobservable** et le gyro **dérive** sans magnétomètre → mapper `rZW ← yaw` ferait vagabonder la scène. Le 6-faces évite ce blocker. *(Rotation libre absolue = V2 avec ICM-20948 9 axes + fusion mag.)*

---

## 2. Architecture matérielle

```
        +-------------------- xerkion (cube PETG ~70mm) -----------------------+
        |   [PiSugar 3 / 3 Plus]  --(sortie 5V / 2.5A, RTC, soft-shutdown)-> 5V |
        |                        Raspberry Pi Zero 2 W                          |
        |          (daemon xion :8730  +  cog/WPE -P drm  plein ecran)          |
        |    SPI0|         I2C1|          I2S |           GPIO|                  |
        |  [ECRAN ST7789]  [IMU LSM6DS3]  [MAX98357A]    [LEVIER Hall GPIO17]    |
        |   240x240 carre     @0x6A        -> HP 4ohm 3W    + LEDs redstone      |
        |   (DRM mipi-dbi)  axes // cube                                         |
        +----------------------------------------------------------------------+
               BL=GPIO13/PWM1   SD_MODE=tie-resistif VDD (pas de GPIO)
```

Bus disjoints sur le silicium BCM (vérifié), cohabitent sans collision : **SPI0** (7,8,9,10,11), **I2C1** (2,3), **I2S/PCM** (18,19,21), **GPIO simples** (levier 17, backlight PWM1 13, redstone).

---

## 3. Électronique + pinout — LA table de vérité unique

> ⚠️ Les brouillons se contredisaient (BL 12 vs 13 ; levier 17 vs 23 ; SD_MODE NC vs GPIO16 vs diviseur ; IMU 0x68 vs 0x6A). **Cette table est la seule valide**, sérigraphiée sur le HAT.

| Périphérique | Signal | BCM | Pin | Note |
|---|---|---|---|---|
| Écran ST7789 SPI0 | MOSI | GPIO10 | 19 | |
| | SCLK | GPIO11 | 23 | |
| | CS (CE0) | GPIO8 | 24 | |
| | DC | GPIO25 | 22 | |
| | RST | GPIO27 | 13 | |
| | **BL** | **GPIO13** | **33** | **PWM1 indépendant — JAMAIS 12/18** |
| | VCC/GND | 3V3/GND | 17/9 | MISO (GPIO9) **non câblé** (write-only) |
| IMU LSM6DS3 I2C1 | SDA | GPIO2 | 3 | **0x6A** |
| | SCL | GPIO3 | 5 | axes // faces |
| | VCC/GND | 3V3/GND | 1/6 | |
| Audio MAX98357A | BCLK | GPIO18 | 12 | |
| | LRCLK | GPIO19 | 35 | |
| | DIN | GPIO21 | 40 | |
| | Vin/GND | 5V/GND | 4/14 | |
| | **SD_MODE** | — | — | **tié VDD via ~100kΩ (mono ON). Overlay SANS sdmode-pin.** |
| Levier | OUT | **GPIO17** | 11 | Hall A3144, pull-up interne |
| Alim | 5V | 5V | 2 | depuis PiSugar 3 |

**Libres** (redstone/ESP32) : GPIO 4,5,6,12,16,22,23,24,26.

### Pièges éliminés d'avance
1. **Backlight vs I2S** : BL sur GPIO18 (=I2S BCLK) ou GPIO12 (=PWM0, même canal que GPIO18) = collisions cachées. → **BL = GPIO13 (PWM1)**, GPIO12 gardé libre. (Option : BL en simple GPIO on/off.)
2. **SD_MODE** : flottant ≠ mute fiable ; `sdmode-pin=0` serait lu GPIO0=ID_SD. → **SD tié VDD via ~100kΩ**, overlay `max98357a` **sans sdmode-pin**.
3. **Adresse IMU** : LSM6DS3 **@0x6A** évite 0x68/0x57/0x32 quel que soit l'adressage PiSugar (confirmé `i2cdetect` D2).
4. **MISO** : lib sans read-back (DRM mipi-dbi) ; pad de secours GPIO9 routé quand même.
5. **GC9A01 banni** (rond) : la pièce carrée est un ST7789 1,54" 240×240.

---

## 4. Boîtier — le cube

Cube FDM **paramétrique** (`scr_win` pilote tout). Pour le **1,54" 240×240**, côté extérieur **~70 mm**.

| Grandeur | Valeur |
|---|---|
| Côté extérieur | ~70 mm (recalculé si taille change) |
| Côté intérieur | 65,2 mm (dominé par Pi 65×30 + batterie) |
| Fenêtre écran | 28×28 mm (+0,3 jeu) + lamage 1 mm |
| Parois | 2,4 mm (3 périmètres PETG) |
| Vis Pi | M2,5 inserts laiton, entraxe **58×23 mm** |
| IMU | **berceau coplanaire au fond, rigide, axes // faces** |

Deux pièces : corps (Pi+HAT, IMU au fond, HP, batterie en **compartiment isolé**) + capot clipsable. Levier séparé. Aérations grille fond + cheminée, dissipateur SoC, passage USB-C, bouton Ø6,6. Le code OpenSCAD compile (manifold).

---

## 5. Logiciel — le bion physique

- **OS** : Raspberry Pi OS Lite 64-bit (Bookworm, aarch64). ~1,9 Go, ~95 Mo RAM repos.
- **Affichage** : **`panel-mipi-dbi-spi`** (vrai DRM SPI) + **`cog -P drm`** (WPE). Pile cohérente DRM↔DRM. **Plan B mesuré** : renderer natif cairo/fbdev (~5 Mo) si RSS WPE > ~200 Mo — règle RAM + latence.
- **Pont accel** (`xerkion-tilt`, à écrire) : I2C → face dominante → (a) WS localhost:8731 60 Hz **direct** (fluide), (b) `POST /emit {topic:"sensor.tilt"}` 10 Hz **bus** (tsoin). Levier → `cub4ion.lever`.
- **Patch page** (à écrire, ~20 lignes) : `EventSource('/events')` + WS, flag **`imuActive`** qui **coupe la dérive** (l.174) et l'inertie drag (l.173) ; drag en fallback.
- **Route `/cub4ion`** (à ajouter, pattern `include_str!` de `/paper`) : sinon 404. Testée D-soft.
- **Audio** : **`mpg123` d'un .ogg embarqué = défaut** (offline, sans user-gesture) ; **`librespot`** bonus si wifi+Premium. Son sort du Pi via I2S. **Levier ON = toujours un son.**
- **Installeur xerkion dédié** (4 units + config.txt + overlays + zram), **`--addr 127.0.0.1`** (pas le `flash.sh pi` nu en 0.0.0.0).

---

## 6. Alimentation + autonomie

> Steady-state réel ~600 mA (rAF permanent + wifi), crêtes ~2,3 A — pas l'idle ~120 mA.

- **PiSugar 3 (2,5 A)** retenu — seul à tenir la crête. **PowerBoost 1000C (1A) / MT3608 (~1,2A) bannis** (brown-out).
- **Bulk** ≥470 µF low-ESR + 100 nF sur l'entrée 5 V du Pi.
- **Rendu événementiel** : redessiner seulement quand l'IMU bouge → −100/−200 mA.

| Source | Utile @5V | Autonomie |
|---|---|---|
| PiSugar 3 1200 mAh | ~770 mAh | ~1,3 h |
| **PiSugar 3 Plus 5000 mAh** | ~3 220 mAh | **~5,5 h** |
| 2× 18650 3000 mAh | ~3 860 mAh | ~6,4 h |
| ~8–10 h | — | 3× 18650 ou + événementiel |

Thermique : dissipateur SoC, alim+LiPo éloignés du Pi, compartiment isolé. Critère : cube **fermé** 30 min → SoC < 70 °C, alim < 60 °C, `get_throttled`=0x0.

---

## 7. Plan de fab + collab PCBway

- **HAT** 65×30 mm FR-4 1,6 mm 2 couches, plan de masse, **keep-out cuivre sous l'antenne wifi**, connecteurs JST-PH + pads secours, condo 470 µF, **sérigraphie table de vérité + repère X/Y/Z**.
- **Livrable** : Gerbers RS-274X + drill Excellon + BOM CSV + CPL (si CMS). Pas de stencil si full traversant.
- **Pitch** : *« le premier xerkion physique »*, open/reproductible, **famille 1,54"/2,1"/4"** = volume → *sponsored prototype run* (5–10 HAT) contre crédit.
- **Impression** : PETG, 0,2 mm, 3 parois, 20–25 % gyroid, supports sous fenêtre/grille, face vers le haut, inserts laiton.

---

## 8. La checklist zéro-test (principe)

On valide **D-soft** (pont logiciel, sans matériel) + **5 bions seuls** (D0–D4) + **emboîtement** (D5) + **fermeture** (D6). Critère binaire à chaque étape ; rouge → on ne câble pas la suite. **D-soft** (route `/cub4ion` + patch SSE/WS + `imuActive` + fallback mpg123) est fait **avant** le makelab car c'est la moitié logicielle qui n'existe pas encore. Détail en annexe D.

---

## Annexes
- **A** Code OpenSCAD final · **B** BOM · **C** Recette d'image · **D** Checklist de-risquage.

### Sources vérifiées
- Pi Zero 2 W brief + mechanical (datasheets.raspberrypi.com) · MAX98357 I2S wiring Adafruit (BCLK18/LRC19/DIN21) · `panel-mipi-dbi-spi` (drm/tiny) · Cog/WPE kiosk DRM (wpewebkit.org) · PiSugar 3 (5V/2,5A) · code repo : `web-cub4ion/index.html` (pas de listener, dérive l.174, iframe l.151), `serve.rs` (pas de `/cub4ion`), `flash.sh` (`--addr 0.0.0.0`).
