# SkyView — ploxion

Carte du ciel **full-WASM + WebGL2**, style SkyView / chill-tekk. Le *render* et
la *math astro* tournent entièrement dans un crate **Rust compilé en `wasm32`**
(via `wasm-bindgen` + `web-sys` `WebGl2RenderingContext`). Le harness HTML est une
fine couche : un canvas + des contrôles qui appellent les méthodes exportées.

Vit comme ploxion navigateur (site / jOS) **ou** standalone (ouvrir une page).

## Fichiers

| Fichier            | Rôle |
|--------------------|------|
| `Cargo.toml`       | Manifest du crate `skyview` (standalone `[workspace]`). Deps : `wasm-bindgen =0.2.100`, `js-sys`, `glam` (libm), `web-sys` (WebGL2). |
| `src/lib.rs`       | **Cœur** : struct `#[wasm_bindgen] SkyView` (canvas + WebGL2, 2 programmes shaders points/lignes, VAO/buffers, `render()`), `view_proj()` (perspective + look-at yaw/pitch + rotation ciel par LST), et les méthodes exposées. |
| `src/astro.rs`     | Math astro RÉELLE : `radec_to_vec` (RA/Dec → vecteur unité, exact), `gmst_radians` (IAU 1982), `lst_radians`, `julian_date`, `jd_from_unix_millis`, `equatorial_to_horizontal`, `mag_to_size/brightness`. 7 tests. |
| `src/catalog.rs`   | Données RÉELLES : ~57 étoiles brillantes J2000 (RA heures / Dec deg / mag) + 7 constellations en segments. 4 tests. |
| `src/gl.rs`        | Helpers WebGL2 (compile shader, link program, buffers, VAO). |
| `index.html`       | Harness racine : canvas plein écran, recherche, filtres catalogue, temps, magnitude, AR, photo, réticule + label. |
| `style.css`        | Thème sombre responsive (mobile-first, safe-area). |
| `main.js`          | Glue : inputs (drag, DeviceOrientation, temps→JD, magnitude, recherche, capture) → **contrat WASM réel**. Réticule/recherche/center-on en JS via `project_radec`. |
| `catalog.data.js`  | Miroir JS du catalogue (mêmes noms/RA/Dec/mag que `catalog.rs`) pour la recherche + les labels. **Pas** de math dupliquée. |
| `fallback.js`      | Cœur JS de secours (Canvas2D) implémentant le **même contrat réel**, pour tourner **avant** le build wasm. Importe `catalog.data.js`. |
| `www/index.html`   | Second harness autonome (overlay 2D pour le réticule), déjà aligné sur le contrat réel. Charge `../pkg/skyview.js`. |
| `build.sh`         | Build reproductible : `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen --target web --out-dir pkg`. |
| `pkg/`             | Sortie `wasm-bindgen` (`skyview.js` + `skyview_bg.wasm` + `.d.ts`). |

## Build

Pré-requis : `rustup target add wasm32-unknown-unknown` et `wasm-bindgen-cli`
**version 0.2.100** (épinglée dans `Cargo.toml` pour matcher le schéma du CLI).

### Option A — wasm-bindgen direct (utilisé ici, vérifié)

```bash
cd web-skyview
./build.sh
# = cargo build --release --target wasm32-unknown-unknown
#   wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/skyview.wasm
```

### Option B — wasm-pack

```bash
cd web-skyview
wasm-pack build --target web --out-dir pkg
```

### Tests de la math astro

```bash
cd web-skyview && cargo test --lib   # 11/11 passent (astro + catalogue)
```

## Servir + ouvrir

```bash
cd web-skyview
python3 -m http.server 8088
# Harness racine : http://localhost:8088/index.html
# Harness www    : http://localhost:8088/www/index.html
```

`main.js` essaie `./pkg/skyview.js` d'abord. S'il n'est pas buildé → bascule
automatique sur `./fallback.js` (Canvas2D, mêmes données réelles), donc le
harness racine tourne **même sans wasm**.

## Contrat WASM (RÉEL — vérifié contre `pkg/skyview.d.ts`)

`main.js`, `www/index.html` et `fallback.js` ciblent **exactement** ces méthodes :

```
default init(url)                          // wasm-bindgen --target web
class SkyView:
  new SkyView(canvas_id)                   // récupère WebGl2RenderingContext, build catalogue
  set_orientation(yaw_rad, pitch_rad)      // direction caméra, RADIANS
  set_fov(deg)                             // zoom (clampé 15..110)
  set_time(jd)                             // Julian Date complète (UTC) -> LST -> rotation ciel
  set_longitude(deg)                       // longitude observateur, est +
  set_magnitude_limit(m)                   // re-pack du buffer GPU étoiles
  project_radec(ra_hours, dec_deg)         // -> Float32Array [ndc_x, ndc_y, visible]
  render()
  resize(w, h, dpr)
  visible_star_count() / catalog_size()
```

> ⚠️ Note de cohérence : une première version du harness visait un contrat
> inventé (`set_time_jd`, `set_observer`, `set_layer`, `reticule_target`,
> `search`, `center_on`, `set_orientation(az,alt,roll)`). Il **ne correspondait
> pas** au crate buildé. Le harness a été réécrit sur le contrat réel ci-dessus :
> recherche, réticule (objet le plus proche du centre), center-on et filtres de
> catalogue sont désormais faits **en JS** par-dessus `project_radec`, sans
> dupliquer la math (le cœur WASM reste la source de vérité).

## RÉEL vs PLACEHOLDER (honnêteté)

**RÉEL (vérifié) :**
- Pipeline complet : `cargo build --target wasm32-unknown-unknown` compile,
  `wasm-bindgen` génère `pkg/`, `cargo test --lib` passe **11/11**.
- Projection (RA,Dec)→vecteur 3D = trigonométrie sphérique **exacte**.
- Heure sidérale (GMST IAU 1982) réelle ; Julian Date (Fliegel & Van Flandern,
  + époque Unix) exacte ; alt/az exact.
- Rendu WebGL2 : points par magnitude, lignes de constellations, caméra
  yaw/pitch, perspective, rotation du ciel par LST.
- Catalogue : ~57 étoiles brillantes (Sirius, Canopus, Rigil Kentaurus, Arcturus,
  Vega, Capella, Rigel, Procyon, Betelgeuse, Achernar, Hadar, Altair, Acrux,
  Aldebaran, Antares, Spica, Pollux, Fomalhaut, Deneb, Regulus, Polaris…),
  coords J2000 RA/Dec/mag de valeurs catalogue standard (Hipparcos/BSC arrondies)
  — **aucune coordonnée hallucinée au-delà des étoiles très connues**.
- 7 constellations en vrais segments étoile↔étoile (Orion, Ursa Major / Grande
  Ourse, Cassiopée, Scorpius, Sagittarius, Cygnus, Crux / Croix du Sud) ; tout
  segment référençant une étoile absente est **skippé au build** (zéro point
  inventé).

**PLACEHOLDER / pas encore fait (déclaré clairement) :**
- **Planètes / satellites / Lune** : AUCUNE coordonnée émise. Vraie éphéméride =
  **VSOP87 + ELP** (planètes/Lune) et **TLE + SGP4** (satellites), à charger
  plus tard. Refus explicite d'inventer des RA/Dec.
- **Catalogue complet HYG (~120k étoiles)** + NGC/Messier complet : à charger en
  **data file externe** ensuite (le contrat ne change pas, juste la source).
- Pas de précession / nutation / aberration / réfraction (précision œil-nu).
- **AR** (DeviceOrientation) = mapping grossier (yaw=compass, pitch=beta−90) ;
  un vrai AR demande un quaternion complet + correction d'orientation écran. Le
  passthrough caméra montre le flux en fond, le ciel par-dessus (clear opaque) —
  un vrai clear transparent demanderait un drapeau côté cœur.
- `fallback.js` rend en **Canvas2D**, pas WebGL2 — uniquement pour tourner sans
  wasm. Le vrai rendu points/lignes/labels est dans le crate.
- `wasm-bindgen` épinglé à `=0.2.100` pour matcher le CLI du host (à relâcher si
  un CLI plus récent est installé).
