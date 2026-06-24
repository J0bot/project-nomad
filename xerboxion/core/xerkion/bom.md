# BOM final — XERKION #1 (config retenue)

> Table de vérité gravée sur le HAT : BL=GPIO13/PWM1 · IMU LSM6DS3 @0x6A · SD_MODE tié VDD (pas de GPIO) · Levier GPIO17.
> Alim **PiSugar 3** retenue (2,5 A, seule à tenir la crête ~2,3 A). PowerBoost 1000C / MT3608 **bannis** (brown-out).

| Qté | Réf / Produit | Interface | Prix EUR | Fournisseur |
|---|---|---|---|---|
| 1 | **Raspberry Pi Zero 2 W** (SC0918) — quad A53, 512 Mo, wifi | hôte (microSD, USB-OTG) | 15 | Pimoroni / Adafruit (PID 5291) |
| 1 | Header 2×20 mâle (coudé ou stacking) à souder | — | 1 | Pimoroni |
| 1 | **Écran 1,54" carré 240×240 ST7789** (Pimoroni PIM578) | SPI (DRM panel-mipi-dbi-spi) | 13 | Pimoroni |
| 1 | **IMU LSM6DS3TR-C 6-DoF** (STEMMA QT) | I2C **0x6A** | 12 | Adafruit PID 4503 / Pimoroni |
| 1 | **Ampli MAX98357A I2S 3W** | I2S (SD tié VDD) | 6,50 | Adafruit PID 3006 |
| 1 | **HP 40 mm 4Ω 3W** (ou 28 mm) | 2 fils → bornier ampli | 2 | Adafruit PID 3968 |
| 1 | **Alim PiSugar 3 Plus 5000 mAh** (sortie 5V/2,5A, RTC, soft-shutdown) | I2C 0x57 + 5V | 55 | pisugar.com |
| 1 | Capteur Hall A3144 (+ aimant) — levier | GPIO17 | 2 | générique |
| 1 | **Carte porteuse HAT** 65×30 mm FR4 1,6 mm 2 couches | regroupe SPI+I2C+I2S+5V | 10 (5 pcs) | **PCBway** |
| — | Condo 470 µF low-ESR + 100 nF + R 100 kΩ (SD) + JST-PH/borniers + câbles | — | 9 | Pimoroni / PCBway |
| — | Inserts laiton M2,5 + visserie + dissipateur SoC adhésif | — | 5 | générique |
| — | microSD 16 Go A2 | stockage image (~1,9 Go) | 6 | générique |
| — | Filament PETG (cube + capot + levier, ~60 g) | — | 2 | makelab |
| **—** | **TOTAL (config retenue, hors envoi PCBway/port)** | — | **~138,50** | — |

**Variante éco** (PiSugar 3 1200 mAh à 37 € — autonomie ~1,3 h) : **~120,50 EUR**.

**Option V2 (si rotation libre)** : remplacer le LSM6DS3 par **ICM-20948 9-axes** (Adafruit PID 4554, @0x69, ~15 €) — magnétomètre/cap absolu sans dérive. **Non requis** pour le premier xerkion (mapping 6 faces).
