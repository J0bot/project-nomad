# Xerboxion × Project N.O.M.A.D.

> Intégration du **xerboxion-core** dans Project N.O.M.A.D. — pour rendre le système
> auto-hébergé encore plus **offline-first**, navigable et rejouable. Travail mené dans le
> fork `J0bot/project-nomad` ; **pull request vers `Crosstalk-Solutions/project-nomad`
> quand le projet sera fini**. Suivi de l'upstream via `git remote upstream`.

## Le pari
N.O.M.A.D. orchestre des services offline (savoir, cartes, médias, IA locale…) via Docker +
un Command Center (AdonisJS/React). Le **xerboxion** ajoute par-dessus une grammaire
fractale d'unités composables et rejouables :

```
bion  →  cubion  →  PLOBION  →  ploxion  →  boxion  →  xerboxion
```

Chaque **capacité NOMAD = un PLOBION** (assemblé de **bions** partagés), montable comme
**ploxion** dans n'importe quel UI. Le descripteur de service NOMAD
(`Service{service_name, container_image, container_command, container_config, friendly_name,
description, powered_by, icon, installed}`) mappe presque 1:1 sur un descripteur de plobion.

## Règles d'intégration (décidées par José)
1. **Tout le xerboxion-core entre dans le fork** (`./xerboxion/`) — host WASM + bus PLC +
   ploxions + bions + le daemon. Le fork est le terrain d'intégration ; le PR upstream
   embarquera le core.
2. **Si un ploxion existe déjà, on l'ENRICHIT** (jamais de doublon) avec l'angle offline de
   l'outil NOMAD correspondant.
3. **Fil rouge : rendre le système excellent OFFLINE.**
4. Un **plugion `nomad`** garde le lien upstream pour tracker les changements et préparer le PR.

## Carte de recouvrement (NOMAD ↔ ploxion core existant)
| Capacité NOMAD | Outil | Ploxion core | Action |
|---|---|---|---|
| Savoir offline | Kiwix (ZIM) | dictionnaire / techtree / science / nexus / citations | **NOUVEAU plobion** (flagship) |
| Cartes offline | ProtoMaps | map / lausanne / carte | ENRICHIR (tuiles offline) |
| Inventaire | Homebox | inventaire / makelab | ENRICHIR |
| Notes | FlatNotes | post-it / braindump | ENRICHIR |
| Fichiers | FileBrowser | ranger / binder | ENRICHIR |
| Outils data | CyberChef / IT-Tools | convertisseur / regex / morse / couleurs / text | ENRICHIR (hub data) |
| PDF | Stirling | pdf | ENRICHIR |
| Coffre mots de passe | Vaultwarden | password / xi0n | ENRICHIR (offline) |
| Éducation | Kolibri | techtree / kardashev | ENRICHIR |
| Média | Jellyfin | (player wasm) | NOUVEAU |
| IA locale + RAG | Ollama + Qdrant | — | NOUVEAU (≠ xerbion, à cadrer) |
| Ebooks / Mesh / Whiteboard | Calibre / Meshtastic / Excalidraw | — | NOUVEAU |

## Disposition du fork
```
project-nomad/
├── admin/            # Command Center NOMAD (AdonisJS + React) — inchangé, on s'y branche
├── install/          # compose + scripts NOMAD — inchangé
├── collections/      # contenu offline (zim/maps/wikipedia) — réutilisé
└── xerboxion/        # ⬅ LE CORE (importé) : host WASM + bus + ploxions web-*/ + bions SDK
    └── INTEGRATION.md # journal d'intégration plobion par plobion
```

## Roadmap (plobion par plobion, offline-first)
- [ ] Importer le xerboxion-core sous `./xerboxion/`
- [ ] Plugion `nomad` : pont upstream + lecture du catalogue Supply Depot → plobions
- [ ] Plobion **savoir-offline** (Kiwix/ZIM) — flagship
- [ ] Enrichir `map` → tuiles ProtoMaps offline
- [ ] Enrichir `inventaire` → Homebox
- [ ] … (le reste de la carte)
- [ ] PR vers upstream `Crosstalk-Solutions/project-nomad`
