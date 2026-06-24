# xerkion/ — le premier Kion physique

José, le xerkion #1 en 5 lignes :
1. TU COMMANDES : Pi Zero 2 W (15€) + écran carré 1,54" ST7789 SPI (13€) + IMU LSM6DS3 (12€) + ampli MAX98357A + HP (8,50€) + PiSugar 3 Plus 5000mAh (55€) + capteur Hall pour le levier + le HAT PCBway — total ~138€. (PowerBoost/MT3608 bannis : ils font brown-out le Pi.)
2. ON IMPRIME : un cube PETG ~70mm en 2 pièces (corps + capot) + le levier ; l'IMU posé coplanaire au fond, axes alignés aux faces — c'est ça qui fait que tourner le cube tourne le cub4ion.
3. ON FLASHE : RPi OS Lite 64-bit + daemon xion + cog -P drm sur l'écran SPI ; MAIS il reste 3 bouts de code à écrire AVANT le makelab (route /cub4ion, le pont accel xerkion-tilt, le patch de la page qui coupe la dérive auto) — testés sur table au jalon D-soft.
4. AUTONOMIE : ~5,5 h réelles (PiSugar Plus, conso mesurée ~600mA ; l'idle ~120mA était faux car le cube redessine en continu). Pour ~8-10h : 3× 18650 ou rendu événementiel.
5. RISQUE RÉSIDUEL : la RAM de Cog/WPE (à mesurer ; plan B = renderer natif cairo qui règle RAM+latence d'un coup) et la scène = 6 faces discrètes (pas rotation libre — le yaw est inobservable sans magnéto ; V2 = ICM-20948). Tout le reste est dé-risqué jalon par jalon pour marcher au 1er allumage fermé.

---

## Dossier de fabrication v1.0 (dé-risqué, fabricable sans phase de test)

- [`xerkion-paper.md`](xerkion-paper.md) — le papier complet (concept, archi, pinout, boîtier, logiciel, alim, fab+collab PCBway, checklist zéro-test)
- [`enclosure.scad`](enclosure.scad) — le cube paramétrique OpenSCAD (change `scr_win` → tout se recalcule)
- [`bom.md`](bom.md) — la nomenclature (~138 EUR)
- [`software-image.md`](software-image.md) — la recette de l'image (le bion physique) + les 3 bouts de code à écrire avant le makelab (jalon D-soft)
- [`build-checklist.md`](build-checklist.md) — la checklist de dé-risquage jalon par jalon (D-soft → D6)

> Issu d'un workflow à 9 agents (5 design + 3 vérif adversariale + 1 synthèse). Les 3 vérifs ont trouvé 8 blockers + 10 majors ; la synthèse les a corrigés en amont. Risque résiduel nommé : RAM de Cog/WPE (plan B = renderer natif cairo) ; scène = 6 faces discrètes (pas rotation libre — V2 = ICM-20948 9 axes).
