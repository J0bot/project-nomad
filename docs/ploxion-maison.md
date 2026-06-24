# Le ploxion maison — objectif impossible (maison autonome complète)

> José (2026-06-20) : « je dois te donner des objectifs IMPOSSIBLES, pas des tâches. Fais un ploxion complet
> de toute une maison autonome — le ploxion maison — liste toutes les features A→Z et inclus-les dans le core. »

Énuméré : **157 features**, 10 domaines, **55 buildables tout de suite** (pur-bus, zéro nouveau bion).
Une maison autonome = un **système de feature-ploxions** (des cubions) sur le bus du xion. Le cahier de José :
le **cubion-home** modulaire qui se déroule en maison puis en **vaisseau**.

## Le maison-bion (sœur exacte du block-bion)
Module `ploxions/sdk/src/house.rs` (~250 lignes, copie structurelle de `block.rs`). **Zéro nouveau bion bas-niveau.**
- **`HouseDef`** (le RÉSIDU par feature, const) : `domaine`, `feature`, `sense:&[topic]`, `act:&[topic]`, `rule:Rule`, `modes`, `vital`.
- **`Rule`** (le DECIDE déclaratif partagé) : `Hysteresis{lo,hi}` (thermostat/BMS/pompe) · `Threshold{trip,vital}` (CO/gaz/fumée/fuite) · `Palier{steps}` (CO2/AQI) · `Greedy{tiers}` (délestage vital>confort>luxe) · `Fsm{states}` (alarme/airlock/mode) · `Aggregate` (dashboard/bilan) · `Replay` (présence-sim/layout) · `Passthru` (orchestrateurs).
- **`Mode`** : `Absent|Present|Urgence|Nuit` (+ **Vaisseau** plus tard) = multiplicateur de seuils ; `urgence` = override sécurité absolu (comme `fluid` chez block).
- **`house_ploxion!(DEF)`** : sœur de `block_ploxion!` — même OnceCell manifest, même `__SEQ`, même filtre de préfixe (`house.<d>.<f>.sense`), même `tsoin_record(name,payload)`. **Chaque feature = `HouseDef{…}` (~8 lignes) + 1 appel de macro** (comme `block-stone`).

## Grammaire de topics
`house.<domaine>.<feature>.<sense|state|act>` + transverses `house.mode.state` (broadcast) et `tsoin.record`
(= la machine à tsoins de la maison : record du bus → **journée rejouable**). Manifest id = `house-<domaine>-<feature>`.

## Premier batch (à inclure dans le core — pur bus, maintenant)
1. **house-bion** — le frame (HouseDef + house_ploxion! + Mode + Rule). La fondation qui rend les 156 autres ~gratuites.
2. **house-brain-core + house-mode-manager** — le hub central (arbitre sécurité>confort>éco) + broadcast du mode = **premier comportement vivant** de la maison.
3. **house-tsoin-recorder + house-comms-record-replay** — la machine à tsoins appliquée à la maison (record→rejouer une journée).
4. **house-sec-intrusion-alarm + house-fire-smoke** — FSM sécurité + corrélation multi-capteur (consommateur de topics, comme le watcher).
5. **house-energy-prioritizer + house-energy-dashboard-tsoin** — arbitrage vital>confort>luxe + bilan énergétique rejouable.

## Les highlights impossibles
- **MODE VAISSEAU** : la maison-cubion se déroule (deploy) ↔ se replie (stow) ↔ bascule maison⇄vaisseau ; airlock (jamais 2 portes ouvertes), seal-pressurize (PID ~1 atm), hull-integrity, self-repair-seal, thermal-skin. Un **5e mode**.
- **AUTONOMIE TOTALE** (off-grid) : grid-tie-island, genset, surplus-router, water-balance (puits+pluie+citerne), greywater-recycle, aquaponics (cycle azote), poulailler+ruche+serre. La maison se nourrit/chauffe/éclaire/défend sans réseau.
- **APPRENTISSAGE DES HABITUDES** : `house-habit-xerbion` recâble le **xerbion existant** (MLP en ligne) sur le flux maison → prédit le besoin (chauffer AVANT le retour) ; `house-policy-learner` : chaque override humain = signal d'erreur → ajuste la règle (la maison s'auto-programme).
- **VISION+VOIX** (`gpu.infer`) : feu/chute/personne sans capteur, voix→scène, vision des plantes (carence/maturité), bras qui range (**l'objet→sa place DEVIENT le tsoin = adressage génératif réel**).
- **STRUCTURE REJOUABLE** : la machine à tsoins appliquée à la **forme** — un déroulement de maison gravé se **rejoue** (la maison se souvient de ses formes). Le jump/compression de José sur l'espace physique.

## Capabilities hôte à déclarer (par levier)
1. **world.clock** (tick temps réel injecté — débloque timers/circadien) · 2. **world.sense / world.act** (capteurs/actionneurs réels, le gros du physique, modèle `service-connector`) · 3. **tsoin.replay** (l'hôte ré-injecte un tsoin comme suite d'events → présence-sim/layout-replay) · 4. **gpu.infer** (vision/voix, cf. jump `iris`) · 5. **net.fetch** (déjà dans le SDK — météo/tarif/alerte).

## Inclusion dans le core
**Par accrétion de cubions** : chaque tick 2/h, le forge ajoute N `HouseDef` + N `house_ploxion!` = N `.wasm` sur le bus, **sans toucher le frame** — exactement comme `block-stone`/`water` s'ajoutent au catalogue Minecraft-Nexus. La maison s'INCLUT feature par feature.

Voir [[craft-uncraft-bions]] (le maison-bion = block-bion), [[directive-ploxions-heure-blocs]] (le forge 2/h), `docs/boucle-creation-infinie.md`.
