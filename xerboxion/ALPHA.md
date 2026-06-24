# Xerboxion ALPHA — la première version du xerboxion (via N.O.M.A.D.)

> **Cap José (2026-06-24)** : « avance à fond sur nomad, comme ça dans deux semaines je peux
> le sortir en version alpha, la première version du xerboxion. »
> **Cible : ~2026-07-08.** Forme : la stack NOMAD + le xerboxion-core, offline-first, en un
> `docker compose up`. Le xer (l'OS public) tourne à côté du Command Center NOMAD.

## Ce que l'alpha doit faire (definition of done)
1. `docker compose up` lance NOMAD **+** le xerboxion → le **xer** est accessible (`:8730/xer`).
2. Le **launcher du xer** liste les ploxions UI (les 66 `web-*`) — utilisables **offline**.
3. Les **services NOMAD** (Kiwix, IA, CyberChef, Homebox…) apparaissent dans le xer comme
   **plobions** (tuiles) ; cliquer ouvre le service. Catalogue lu depuis NOMAD.
4. **Offline-first** : tout marche sans réseau (le xer + les ploxions statiques + les services NOMAD locaux).
5. Installable proprement (extension du flux d'install NOMAD).

## Jalons (≈ 2 semaines)
**Semaine 1 — le socle tourne**
- [x] M0 — Importer le core dans le fork (`xerboxion/core/`) + spec d'intégration.
- [x] **M1 — Containeriser le core** (`xerboxion/Dockerfile`) + overlay compose (`docker-compose.xerboxion.yaml`).  ← FAIT
- [ ] M2 — Build + run vérifiés : l'image se construit, le xer répond `:8730/xer`, les `web-*` se servent.
- [ ] M3 — **Plugion `nomad`** : lit le catalogue NOMAD (`service_seeder.ts` → 16 services) → `plobions.json`
        (descripteurs : id, image, friendly_name, icon, offline, provides/requires). Le xer affiche ces plobions.
- [ ] M4 — Pont xer ↔ NOMAD : ouvrir un service NOMAD depuis une tuile du xer (URL/port résolus).

**Semaine 2 — l'offline-first + le polish**
- [ ] M5 — Enrichir 3–4 ploxions existants en offline (carte→tuiles ProtoMaps, hub data depuis
        convertisseur/regex/morse, inventaire→Homebox, notes→FlatNotes).
- [ ] M6 — Flux d'install (script ou bouton) qui ajoute le xerboxion à une install NOMAD.
- [ ] M7 — Passe offline (aucune requête externe au runtime) + README alpha + **tag `v0.1.0-alpha`**.

## Hors-scope alpha (plus tard)
- Les bions WASM (protocoles/organes) packagés dans l'image — l'alpha sert d'abord les ploxions UI.
- Le viewer 3D cubique de topologie (piste **parallèle**, ma lane — avance à côté).
- IA locale (Ollama) intégrée comme plobion — cadrage à part (≠ xerbion).
- PR vers upstream `Crosstalk-Solutions/project-nomad` (après l'alpha, quand le projet sera mûr).

## Risques / notes honnêtes
- Le build Rust en Docker (~2 min + deps) — à valider en CI/release, pas sur le VPS labo (mémoire serrée).
- `web-*` servis via `assets_dir/web-<id>/` : l'image copie les `web-*` dans `/app/ploxions/` (vérifié contre `serve.rs`).
- `Dockerfile`/overlay **non encore buildés** ici — à valider au premier `docker build`.
