# Checklist d'assemblage + de-risquage — XERKION #1

> Règle d'or : un jalon non **VERT** bloque le suivant. Pas de big-bang : on empile des vérités. Logiciel manquant validé en **D-soft AVANT le makelab**.

## Préparation (la veille)
- [ ] Cube + capot + levier **imprimés** PETG (~6–8 h).
- [ ] HAT PCBway **soudé** ; ohmmètre : continuité GND, **pas de court 3V3↔GND ni 5V↔GND**.
- [ ] microSD 16 Go A2 **flashée** (RPi OS Lite 64 + installeur xerkion) et **bootée headless**.

---

## D-soft — Pont logiciel (sur table, AUCUN matériel xerkion)
- [ ] Route `/cub4ion` → **`curl 127.0.0.1:8730/cub4ion` = 200**.
- [ ] Patch page : un `POST /emit {topic:"sensor.tilt",payload:{rZW:2.09}}` manuel → la scène **change** (test PC).
- [ ] Flag `imuActive` : après une trame tilt, le cube **ne dérive plus** (scène fixe cube immobile).
- [ ] `POST /emit {topic:"cub4ion.lever"}` → `toggle()` (ON/OFF + musique).
- [ ] `xerkion-tilt` : en simulation, poste sur le bus + le WS.
- [ ] Fallback **mpg123** : `cub4ion.power on` joue le `.ogg` **sans réseau**.
- **✅** : sur PC, rotation simulée → scène change + levier → son, **sans dérive**. Rouge → pas de makelab.

## D0 — SD / OS / daemon (Pi seul, HDMI bureau)
- [ ] Boot < 40 s ; `systemctl status xion` = **active** ; `/healthz` = 200 ; `/cub4ion` = 200 ; page affichée.
- **✅** : daemon vivant + page servie.

## D1 — Écran SPI seul (Pi + écran, breadboard)
- [ ] Overlay `mipi-dbi-spi` chargé ; **`/dev/dri/card0` a un connecteur actif** (`modetest`).
- [ ] `cog -P drm` (ou mire) → **plein écran 240×240**, 0 pixel mort, ≥ 20 fps, backlight PWM **GPIO13** réglable.
- **✅** : image réelle via DRM (pas un /dev/fb1 orphelin). *Valide le seam d'affichage.*

## D2 — Accéléromètre I2C seul (Pi + IMU)
- [ ] `i2cdetect -y 1` → **0x6A** ; pas de 0x68.
- [ ] Au repos sur une face : **g ≈ ±1,0 sur un seul axe**, les autres ≈ 0.
- [ ] Enregistrer les **6 signatures de face** → scène + `accel-cal.json` (offsets à plat).
- **✅** : 6 faces = 6 états déterministes (alignement confirmé).

## D3 — Audio I2S seul (Pi + MAX98357A + HP)
- [ ] Overlay `max98357a` SANS sdmode-pin ; SD **tié VDD via 100kΩ** vérifié ohmmètre.
- [ ] `speaker-test -c1 -twav` → son **net**, **pas de souffle**, pas de pop ; mpg123 du fallback sort.
- **✅** : audio I2S propre + fallback offline.

## D4 — Alim / PiSugar seul (SANS le Pi)
- [ ] Sortie 5 V sous **charge factice 1,5 A** → **≥ 4,9 V**, jamais < 4,75 V ; crête 2,3 A/100 ms sans sag (bulk 470 µF).
- [ ] PiSugar < 60 °C.
- **✅** : tient la crête réelle. *PowerBoost/MT3608 ne passent pas — interdits.*

## D5 — Intégration sur table (les 5 + D-soft, cube OUVERT)
- [ ] Boot < 40 s ; `vcgencmd get_throttled` = **0x0**.
- [ ] **Tourner physiquement** → scène change **< 200 ms**, **sans dérive** (`imuActive`).
- [ ] Levier (Hall) → `cub4ion.lever` → `toggle()` → **un son part**.
- [ ] **30 min continu** sans reboot ni throttling.
- **✅** : démo marche, ouvert, en continu.

## D6 — Fermeture (cube assemblé)
- [ ] Montage : (a) IMU **coplanaire ±0,5°** au fond (gabarit), (b) écran + joint, (c) Pi+HAT colonnes M2,5, (d) HP derrière grille, (e) levier, (f) batterie en **compartiment isolé** + dissipateur SoC.
- [ ] **Cube fermé 30 min** charge réelle → SoC < 70 °C, alim < 60 °C, `get_throttled`=0x0.
- [ ] 6 faces → 6 états ; levier → son.
- **✅** : **démo complète au 1er boot fermé.** Cible « pas de phase de test » atteinte.
