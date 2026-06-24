# Catalogue maître + roadmap — XERBOXION (le jeu)

> Découverte de TOUS les ploxions et bions du jeu (Jose : « notre but c'est de découvrir tous les bions et
> tous les ploxions »). Existant ✅ vérifié read-only, à-créer ◻ = le scope pour shipper le jeu en entier.

# LE CATALOGUE MAITRE — XERBOXION (le jeu)

Carte de reference pour shipper le jeu en entier et ouvrir aux contributeurs (RepoVerse).
**Couverture : 75 ✅ existant / 197 ◻ a-creer = 272 au total ≈ 27,6 % shippe.**
Verifie READ-ONLY (41 dossiers ploxions + 15 web-* + 6 modules SDK : bions=62, math=91, dissociation=7, protocol=1, block=1, house=5 pub fn).

---

## 1. MOTEUR TSOIN + SDK BIONS (le socle record/replay) — 15 ✅ / 24 ◻

Le coeur : capter le reel -> residu -> rejouer bit-exact. C'est LE socle dont depend tout le reste.

| Unite | Etat | Role |
|---|---|---|
| tsoin (engine) | ✅ | record/replay sur le bus, XOR-delta sur store BLAKE3+Merkle, reconstruit bit-exact |
| tsoin-store | ✅ | adressage genre addr64(gen,residu), dedup+refcount, query-par-generateur |
| diff | ✅ | le RESIDU (moitie record) : XOR-delta, residu_minimal, surprise=Hamming, distance |
| generator | ✅ | la moitie REPLAY : gen_apply raw/lcg/xor-delta, reconstruit le reel + verifie |
| clock-coherence | ✅ | L'HORLOGE : temps=coherence=synchro (pas wall-clock), clock.now |
| player | ✅ | deroule une SEQUENCE de tsoins dans le temps logique (play/pause/loop/seek) |
| sdk::bions (62 fn) | ✅ | JSON, hex, fnv, xor_delta, addr64, DedupTable, LCG, CoherenceWindow, SeqCursor |
| sdk::math (91 fn) | ✅ | arith/bitwise/logic i32 + f64m (~24 floats : lerp/hypot/clamp...) |
| sdk::dissociation (7 fn) | ✅ | modele giga-tsoin (Arousal/ToleranceWindow/dissociate/reintegrate) |
| sdk::protocol (1 fn) | ✅ | Pdu + parse_pdu, partage par les proto-* |
| sdk::block (1 fn) | ✅ | BlockDef + flow_level (le bloc-bion) |
| sdk::house (5 fn) | ✅ | Mode/Rule.decide (cerveau-maison / PNJ a regles) |
| tester | ✅ | drive une cible, grave le tsoin de comportement (test unitaire du bus) |
| xerbion / xerbion-bit | ✅ | neurone predictif (MLP 4-6-1) + version ternaire BitNet, emet le residu |
| spectre / synthe | ✅ | uncraft/craft du son (DFT <-> PCM, un son = un tsoin) |
| **merkle (organe)** | ◻ | exposer l'arbre BLAKE3/Merkle SUR le bus (hash_leaf/root/proof/verify) — integrite saves + RepoVerse + sync multi |
| **compress (organe+bions)** | ◻ | vraie compression (varint/LZ77/dict/delta-of-delta/bit-pack) au-dela du RLE-de-zeros — indispensable pour le bion 16Go |
| **fork / branch** | ◻ | brancher la timeline (LINEAIRE aujourd'hui) : save-slots, mondes alternes, undo-tree, divergence multi |
| **merge / reconcile** | ◻ | 3-way merge CRDT de 2 etats divergents — coeur du multijoueur |
| **string bions (SDK)** | ◻ | split/trim/join/upper/lower/contains/replace/pad/levenshtein — quasi absent, UI/chat/commandes en ont besoin |
| **collection bions (SDK)** | ◻ | sort/dedup/reverse/map/filter/fold/binary_search/group_by/chunk sur slices |
| **time/calendar bions** | ◻ | tick_to_gameclock/day_night_phase/season/cooldown — temps in-game distinct de la coherence |
| **snapshot / checkpoint** | ◻ | squash la chaine de residus en base a t (sinon replay depuis genese = inscalable) |
| **scheduler (organe)** | ◻ | timers/recurring/delayed sur t logique — spawns, croissance, delais redstone |
| **random / noise** | ◻ | RNG seede + Perlin/Simplex/value/fbm 1D-3D — terrain procedural IMPOSSIBLE sans |
| **hash family bions** | ◻ | blake3/sha256/xxhash/murmur3/crc32 — FNV seul collisionne (documente) |
| **varint / serialization** | ◻ | LEB128/zigzag/le-be u16-64/f32-64 — hex-in-JSON = 2x trop gros pour les chunks |
| **base64 / encoding** | ◻ | base64url/base32/base58 pour liens xi0n/RepoVerse/export |
| **diff-binary / patch** | ◻ | vrai diff insert/delete/move (Myers/bsdiff) — diff actuel casse si la longueur change |
| **validate / schema** | ◻ | schema.check(payload,shape) — rejeter le contenu malforme (contributeurs RepoVerse) |
| **replicate / pack** | ◻ | empaqueter un ploxion (wasm+manifest+tsoin-comportement) en 1 artefact content-addressed -> flotte/mods |
| **crypto / sign** | ◻ | ed25519 sign/verify + capability-token — confiance multi + ploxions contributeurs |
| **interpolate / animation** | ◻ | ease/slerp/bezier/catmull-rom — replay lisse entre keyframes |
| **query / select (organe)** | ◻ | requetes corpus : par range-temps, prefixe-addr, top-k surprenants, count-by-gen |
| **vector / spatial bions** | ◻ | vec3/vec2/quat : dot/cross/normalize/AABB/raycast/chunk-coord — monde 3D impossible sans |
| **entropy / metrics bions** | ◻ | shannon_entropy/mutual_info/cosine/jaccard — vraie info, pas que popcount |
| **gc / prune (organe)** | ◻ | recuperer residus refcount==0, prune sous snapshot, budget RAM (premisse machine-a-jump) |
| **audit / integrity** | ◻ | verifier que tout le store se re-hashe/re-adresse/Merkle-match — avant de shipper des saves |
| **export / import (organe)** | ◻ | save-file portable compressee content-addressed (corpus+index+manifest) <-> rehydrate+verify |

---

## 2. RESEAU EN PLOXIONS (proto-* / port-bion / package-bion) — 11 ✅ / 33 ◻

Chaque protocole / port / paquet = un ploxion. Directive bion-Linux. Design fige par Jose (docs/network-as-ploxions.md).

| Unite | Etat | Role |
|---|---|---|
| proto-tcp/udp/icmp/dns/http/ntp/tls | ✅ | 7 ploxions-protocole (residu ProtocolDef + macro), 5-18 l. chacun |
| protocol-bion (sdk/protocol.rs) | ✅ | le generateur : ProtocolDef + macro protocol_ploxion! + table PROTOCOLS (11 entrees) — **marque NON-COMPILE** |
| table-graine PROTOCOLS | ✅ | 11 entrees IANA (dhcp/quic/ssh/ws deja dedans mais PAS generes en dossier) |
| adapters reseau (6) | ✅ | health/gitea/ideas-map/osiris/repoverse/mc : consommateurs net.fetch (gabarit, voir famille Adaptateurs) |
| docs/network-as-ploxions.md | ✅ | feuille de route officielle (3 generateurs, lots anti-OOM, extension paresseuse) |
| **protocol.rs -> compiler** | ◻ | BLOQUANT : passer le scaffold en batch calme anti-OOM avant toute la vague 1 |
| **proto-dhcp/quic/ssh/ws** | ◻ | 4 ploxions, residu DEJA pret dans la table -> cout quasi nul, a sortir tout de suite |
| **proto-arp / ip(v4/v6) / ethernet** | ◻ | couches basses manquantes (liaison/reseau) pour "couvrir le reseau entier" |
| **proto-smtp/imap/pop3** | ◻ | pile mail (gros pan applicatif) |
| **proto-ftp/sftp/tftp** | ◻ | transfert de fichiers |
| **proto-mqtt/amqp** | ◻ | bus IoT/entreprise = cousins directs du bus xion |
| **proto-grpc** | ◻ | RPC binaire moderne (inter-ploxion distant) |
| **proto-bgp/ospf** | ◻ | routage (comment les paquets trouvent leur chemin) |
| **proto-snmp/syslog** | ◻ | supervision (lie au watcher/health-adapter) |
| **proto-ldap/radius/kerberos** | ◻ | auth reseau (modelise le SSO au niveau pile) |
| **proto-stun/turn/webrtc** | ◻ | NAT-traversal + media P2P (mesh bluxion, /warp pair-a-pair) |
| **proto-wireguard/openvpn/ipsec** | ◻ | VPN/tunnels (mesh boxion, /warp E2E) |
| **proto-gossip** | ◻ | anti-entropy du mesh (propre au xerboxion) — decouverte/synchro de la flotte |
| **proto-icmpv6 / ndp** | ◻ | pendants IPv6 (valeur de debug reel : soucis AAAA en memoire) |
| **port-bion (sdk/port.rs)** | ◻ | VAGUE 2 jumeau de protocol.rs : PortDef + macro port_ploxion! — annonce mais NON ECRIT |
| **port-<n> (well-known)** | ◻ | 1 ploxion par port (port-22/80/443/53/25...), generation PARESSEUSE |
| **table-graine PORTS** | ◻ | ~1024 well-known {n°,transport,service}, derivable de /etc/services |
| **port-allocator-bion** | ◻ | attribue/libere les ports ephemeres (49152-65535), refcount — evite 65535 ploxions |
| **package-bion (sdk/package.rs)** | ◻ | VAGUE 2 : PackageDef + macro — annonce mais NON ECRIT. 1ere brique RE de Linux |
| **pkg-<nom> (Debian base)** | ◻ | 1 ploxion par paquet (pkg-coreutils/libc6/bash/openssl), residu lu de dpkg |
| **package-resolver-bion** | ◻ | resout le graphe de deps des pkg-* = remplacant conceptuel d'apt |
| **net-stack-orchestrator** | ◻ | chaine les couches ethernet.out->ipv4.in->tcp.in->http.in (sinon proto-* isoles) |
| **net-router** | ◻ | table de routage (plan de controle) |
| **net-firewall** | ◻ | regles sur topics net.* (rendre ufw observable, gameplay defensif) |
| **net-nat** | ◻ | reecrit src/dst des PDU — des qu'il y a plusieurs boxions |
| **net-sniffer / capture** | ◻ | le "wireshark" du jeu : tsoin pcap-like de tout net.*.out -> carte 4D |
| **net-socket-bion** | ◻ | l'abstraction prise {proto,port-local,port-distant,etat} pour tcp/quic |
| **build-ploxions (lot reseau)** | ◻ | le tsoin de build rejouable qui compile la vague reseau PAR LOTS (anti-OOM ~500Mo) |

---

## 3. LES BLOCS (CUBIONS) + LE MONDE DU JEU — 9 ✅ / 53 ◻

Le plus gros trou (~15% couvert). **CHAINON MANQUANT : personne n'emet block.place/break -> stone/water sont des stubs.**

| Unite | Etat | Role |
|---|---|---|
| sdk/block.rs (BLOCK-BION + FLOW-BION) | ✅ | BlockDef + macro block_ploxion! + flow_level — pret a generer N blocs |
| block-stone / block-water | ✅ | SEULS 2 blocs (residu pur) ; water revele le FLOW-BION |
| mc-adapter | ✅ | pont minecraft.command -> tsoin -> mc.event (manque mc.event->block.place) |
| carte | ✅ | monde 4D par compression (LOD), pas encore generation de terrain |
| player / clock-coherence / kion | ✅ | replay temporel / horloge / collapse rond->grille (reutilisables pour le monde) |
| web-minecart / web-cub4ion / web-3d | ✅ | surfaces web (mais AUCUNE UI voxel jouable cablee aux blocs) |
| **blockdef-extensions (SDK)** | ◻ | **P0 : etendre BlockDef (falls/unbreakable/flammable/light/friction/growth/container/redstone/drop_table) -> debloque ~80% des blocs** |
| **place-break (interaction)** | ◻ | **le chainon manquant : EMET enfin block.place/break (clic G=break, clic D=place)** |
| **world-gen** | ◻ | **genere le terrain (bedrock/stone/dirt/grass/grottes/minerais), nourrit block.place, LCG seede** |
| block-dirt/grass/sand/gravel | ◻ | sol de base (grass->GROWTH-BION, sand/gravel->GRAVITY-BLOCK-BION + DROP-BION) |
| block-wood-log/planks/leaves/sapling | ◻ | arbre (leaves->DECAY-BION, sapling->GROWTH-BION) + materiau central |
| block-glass/cobblestone/bedrock | ◻ | verre (transparent), cobblestone (ferme la boucle du drop de stone), bedrock (UNBREAKABLE) |
| block-coal/iron/gold/diamond/redstone-ore | ◻ | minerais (ORE-BION : drop item != bloc) |
| block-redstone-dust/torch | ◻ | **REDSTONE-BION (signal 0..15) — coeur de la logique du jeu** |
| block-lever/button/pressure-plate | ◻ | entrees redstone (require player.interact) |
| block-piston | ◻ | PUSH-BION (deplace des blocs) |
| block-lava/fire | ◻ | lave (FLOW-BION generalise + IGNITE-BION), feu (SPREAD+DECAY-BION) |
| block-torch/glowstone/lantern | ◻ | LIGHT-BION (propagation lumiere, -1/case) |
| block-tnt | ◻ | EXPLOSION-BION (rayon, casse voisins, damage) — reutilise par creeper |
| block-chest/furnace/crafting-table | ◻ | container.open / SMELT-BION / craft.open (couples a inventory/recipe/craft) |
| block-wool(16)/concrete/terracotta | ◻ | VARIANTE par couleur : 1 code-bion, 16+ residus -> valide la generation en masse |
| block-flower/cactus/sugarcane/wheat-crop | ◻ | vegetation + agriculture (GROWTH-BION, ble=crop a stades) |
| block-stairs/slab/fence/door/trapdoor/ladder | ◻ | SHAPE-BION (collision partielle/orientation/ouvrant) ; door->REDSTONE-BION |
| block-snow/ice/obsidian/clay/sponge/netherrack/end-stone | ◻ | blocs de biome/dimension (ice->FRICTION-BION, sponge teste FLOW inverse) |
| **world-gen / biome / chunk-manager** | ◻ | terrain + biomes (plaine/foret/desert/nether/end) + chunks 16x16x256 (LOD, 1 chunk=1 cubion) |
| **dimension** | ◻ | overworld/nether/end + clopion/seption/oction (le champ dim a deja un sens) |
| **daynight / gravity-engine / fluid-engine / lighting-engine** | ◻ | les 4 moteurs qui CONSOMMENT les bions reveles (cycle solaire, chute, propagation fluide, lumiere) |
| **inventory / item-registry** | ◻ | slots/stacks (max 64) ; ItemDef (jumeau de BlockDef) pour les drops != blocs |
| **craft / recipe-registry / uncraft** | ◻ | grille 3x3 ; RecipeDef residu (genere toutes les recettes) ; uncraft=division-en-bions |
| **place-break / block-registry** | ◻ | interaction joueur ; catalogue central id->props (palette/worldgen/craft) |
| **entity-engine / mob-spawn / mob-ai** | ◻ | EntityDef (pos/vel/health/ai) ; spawn selon light/biome/time ; IA wander/chase/flee (mobs=xerbions) |
| **combat-health / physics-collision** | ◻ | PV/faim/mort/respawn ; collision joueur<->solid + SHAPE + friction |
| **save-load (persistence)** | ◻ | quasi-gratuit : un monde = deja une sequence de tsoins block.place, player sait rejouer |

---

## 4. LES ADAPTATEURS (les "bouches" : 1 ploxion par source externe) — 7 ✅ / 26 ◻

poll/ecoute -> normalise -> emit topic + tsoin.record. **Contrainte host : KNOWN cap = [net.fetch] uniquement, GET/POST http(s), SANS headers/Authorization.**

| Unite | Etat | Role |
|---|---|---|
| health-adapter | ✅ | GABARIT de reference : GET health-URLs -> service.health |
| ideas-map / repoverse / gitea / osiris-adapter | ✅ | poll LIVE -> pin.list / repo.list+item / git.list / osiris.* (+alert) |
| mc-adapter | ✅ | listener pur (sans cap) : minecraft.command -> tsoin -> mc.event |
| service-connector (NATIF host) | ✅ | adaptateur cote host (prouve que le host EST un adaptateur) |
| **weather/rss/wikipedia/wikidata-adapter** | ◻ | FAISABLES AUJOURD'HUI (API publiques sans auth) -> weather.now/feed.item/knowledge.fact (alimente carte 4D + tech-tree) |
| **wallet-adapter (coingecko)** | ◻ | prix/balance sans auth -> dimension eco du jeu |
| **archive-wayback-adapter** | ◻ | POST Save Page Now (labo-only autorise) -> perenniser les tsoins web |
| **spotify-adapter** | ◻ | BLOQUE auth Bearer -> music.track (sync zones-son) |
| **github-adapter** | ◻ | complement externe de gitea (RepoVerse veut les 2 mondes git) |
| **ask-ploxion (llm-adapter)** | ◻ | BLOQUE auth : POST a un LLM serveur (le xion n'embarque PAS de LLM) -> ask.reply |
| **discord/telegram/matrix-adapter** | ◻ | canaux sociaux/contributeurs -> chat.message normalise |
| **youtube/email/calendar-adapter** | ◻ | media.video / mail.received / cal.event (BLOQUE auth pour la plupart) |
| **webcam/audio/geo-position-adapter** | ◻ | le reel visuel/sonore/GPS entre dans le xion (giga-tsoin multimodal) |
| **mqtt-adapter** | ◻ | BLOQUE (non-http) : les yeux de la maison-OS (house-brain existe deja) |
| **websocket-stream-adapter** | ◻ | BLOQUE : flux temps-reel -> necessite NOUVELLE cap host net.ws/net.listen |
| **secrets-vault-adapter (cap auth)** | ◻ | PREREQUIS transverse : sans headers/secrets, ~la moitie du reel reste fermee |
| **fs-adapter (cap fs.read)** | ◻ | PREREQUIS : lire un dossier -> fs.entry (faisable via fs.j0bot.ch en HTTP aujourd'hui) |
| **clock-adapter (cap clock.now)** | ◻ | TROU CRITIQUE : clock.now est attendu sur le bus mais AUCUN ploxion ne le PRODUIT (loi temps=coherence) |
| **steam/itch-adapter** | ◻ | quand le jeu shippe : joueurs/achievements/reviews -> feedback communaute |
| **minecraft-server-adapter (RCON)** | ◻ | evolution du mc-adapter : vrai serveur MC -> mc.event depuis la VRAIE partie |
| **osiris-route-completion** | ◻ | finir les parsers OSIRIS (4 routes -> toutes les couches OSINT) |
| **sensor-generic-adapter (gabarit)** | ◻ | patron config (URL+JSONpath->topic) -> les contributeurs ouvrent une bouche SANS Rust |

---

## 5. SENSORIEL / CAPTURE / RENDU (les wormions) — 20 ✅ / 30 ◻

Capter le reel + le rendre. **CONSTAT : la CAPTURE live est quasi absente (seul web-heart prototype en local).**

| Unite | Etat | Role |
|---|---|---|
| carte / kion | ✅ | rendu spatial 4D / quantification rond->grille |
| spectre / synthe | ✅ | capture (DFT) / rendu (PCM) audio |
| clock-coherence / player | ✅ | cadence la capture / scrubber temporel (play/pause/loop/seek) |
| tsoin / tsoin-store | ✅ | capture+stockage generique d'etat |
| mc-adapter / tracer / ultra-detector | ✅ | capture Minecraft / embryon bus-trace / meta-capture (ULTRA) |
| web-heart (❤️) | ✅ | SEUL vrai enregistreur live : note/clic/frappe AVEC vitesse, rejoue + chiffre AES-GCM |
| web-3d/skyview/cub4ion/wormion/map/nexus/bion | ✅ | surfaces de rendu (chacune autonome, re-implemente son rendu) |
| web-scad/turing/paper/minecart/lausanne/ranger/kardashev | ✅ | surfaces diverses |
| **ploxion-cam / mic / screenshot** | ◻ | getUserMedia/getDisplayMedia -> cam.frame/mic.chunk/screen.frame (l'oeil + l'oreille live) |
| **ploxion-keyboard / mouse / touch / gamepad** | ◻ | capture clavier/souris/tactile/manette horodatee -> tsoin geste |
| **ploxion-recorder** | ◻ | LA machine a tsoins en direct : agrege cam/mic/screen/key/mouse en 1 sequence synchro |
| **ploxion-render3d / voxel-mesh / spritesheet** | ◻ | moteur 3D service + greedy-meshing du monde voxel + atlas textures (AFFICHER les blocs MC qui n'ont pas d'apparence) |
| **ploxion-audio-spatial / midi / waveform-viz** | ◻ | son 3D binaural/HRTF (zones-son) + WebMIDI + waveform/spectrogramme (comble web-spectre/web-synthe VIDES) |
| **ploxion-tsoin-editor / timeline** | ◻ | MONTAGE non-lineaire (cut/splice/branch) + timeline multi-piste (player ne fait que LIRE) |
| **ploxion-bus-viz** | ◻ | visualiseur de bus avec RENDU (graphe topics/debit/latence/heatmap) — tracer logue sans rendre |
| **ploxion-particles / font-glyph / overlay-hud / color-grammar** | ◻ | feedback visuel + rendu glyphes (koin/kion/wormion...) + HUD commun + palette forme=nature |
| **ploxion-replay-cam / snapshot-render / export-media** | ◻ | camera de replay (cinematique) + vignette + GIF/MP4/WebM partageable (coord #scientifiques) |
| **ploxion-video-codec / image-decode** | ◻ | encode inter-frame des cam.frame (diff existe) + PNG/JPEG<->pixels-bions |
| **ploxion-lod** | ◻ | extraire le LOD (carte le mentionne sans l'exposer) = mecanisme jump/compression du rendu |
| **ploxion-haptics / tts-stt / clipboard** | ◻ | vibration + voix (WebSpeech) + presse-papier comme tsoin |

---

## 6. GAMEPLAY + LES ETRES + L'ECONOMIE — 13 ✅ / 31 ◻

Le "on JOUE". **MANQUE quasiment tout : ni inventaire, ni objets, ni craft, ni survie, ni monde persistant, ni position joueur, ni /op, ni eco, ni vrai multi.**

| Unite | Etat | Role |
|---|---|---|
| xp | ✅ | SEULE progression : +XP par tsoin + presence, niveau=floor(sqrt(xp/100)), grave en tsoin |
| player | ✅ | replay d'une sequence = base SAUVEGARDE/REJOUE-DE-PARTIE |
| xerbion / xerbion-bit | ✅ | le PREMIER ETRE (cerveau MLP predictif) + version ternaire ultra-legere |
| kion | ✅ | placement-sur-grille (rond->carre/voxel) |
| science | ✅ | generateur de phenomene fractal deterministe (IFS + box-counting) |
| index | ✅ | stats/scoreboard agrege depuis le bus |
| ultra-detector | ✅ | achievements emergents (flague les ULTRA tsoins) |
| tester / link | ✅ | QA des mecaniques / linkifier le lore (->/wiki/term/<slug>) |
| block-stone / block-water | ✅ | seuls 2 blocs du monde (block-bion pret pour N) |
| house-brain-core / house-mode-manager | ✅ | embryon IA d'ETRE/PNJ a regles (Rule.decide/FSM) |
| sdk bions | ✅ | DedupTable(refcount=qty/rarete), addr64(prix), SeqCursor(save), lcg(worldgen), dissociation(etat mental) |
| **world-grid / chunk-store** | ◻ | le MONDE persistant : retient les blocs places (aujourd'hui events sans grille) — reutilise kion |
| **worldgen / terrain** | ◻ | terrain deterministe (science IFS + lcg_bytes) = un monde a explorer |
| **inventory / item-registry** | ◻ | inventaire (DedupTable=qty) ; ItemDef + item_ploxion! (jumeau block.rs) |
| **recipe / craft-engine / hotbar-equip** | ◻ | craft (inverse de l'uncraft) + slot actif/objet en main |
| **tool / mining** | ◻ | minage selon outil (hardness_milli + tier) -> block.break (aujourd'hui instantane) |
| **quest / achievement / skilltree** | ◻ | objectifs + hauts-faits nommes (formalise ultra-detector) + arbre de competences |
| **health-vitals / combat** | ◻ | hp/faim/stamina (reutilise dissociation) + resolution d'attaques (sinon pas d'enjeu) |
| **spawner / being-ai / being-registry** | ◻ | instancie des xerbions ; comportement (xerbion predit + house arbitre) ; bestiaire (zion/xerion/xeroction...) |
| **market / wallet / trade-npc / trade-ledger** | ◻ | l'ECONOMIE : marche (prix=rarete refcount inverse), monnaie depensable, marchand PNJ, registre grave en tsoins |
| **command / op-console / permissions** | ◻ | le /op et les commandes + roles (op/joueur/invite) — gate les actions destructives |
| **give / loot / teleport-movement / player-state** | ◻ | mode creatif + butin + position/deplacement joueur (inexistant) + identite avatar |
| **multiplayer / nexus-sync / presence-lobby / chat** | ◻ | vrai multi (web-nexus=graphe de CONNAISSANCE, pas du multi) + lobby + chat (via link) |
| **daynight / dimension-portal / physics-gravity** | ◻ | cycle jour/nuit (clock.now) + voyage entre mondes + chute/fluide (gravite=carres / anti=ronds) |
| **save-game / session-snapshot** | ◻ | empaquette TOUT (world+inventory+vitals+player-seq) en 1 tsoin rechargeable |

---

## Synthèse & ordre recommandé

Couverture du jeu complet : 75 unites existent (verifiees, READ-ONLY) sur 272 au scope total (75 ✅ + 197 ◻) = ~27,6 % du jeu shippe. La fondation est solide (moteur de tsoins record/replay + SDK de ~167 bions + 2 cerveaux xerbion + XP), mais TOUT le "on JOUE" et tout le RENDU/CAPTURE reste a batir.

NOTE D'ECART REEL (mineur, pas un blocage) : ls compte 41 dossiers ploxions vs 47 unites listees comme ✅ — l'ecart vient de (a) 4 ploxions reels presents mais hors familles (ping, pong, watcher, state-client + target/tracer comptes ailleurs), et (b) des entrees ✅ qui sont des MODULES du SDK (block.rs, house.rs, protocol.rs, bions.rs, math.rs, dissociation.rs) ou des composants NATIFS host (service-connector) comptes comme unites. Les 75 ✅ comptent ploxions + modules-bions + surfaces-web ; c'est le bon denominateur pour "le jeu en entier".

LES 5 FAMILLES LES PLUS PRIORITAIRES A REMPLIR :
1. BLOCS + MONDE (9✅ / 53◻ = le plus gros trou, ~15% couvert). CHAINON MANQUANT CRITIQUE confirme noir-sur-blanc dans block.rs : PERSONNE n'emet block.place/block.break -> stone/water sont des stubs jamais declenches. Debloqueurs : blockdef-extensions (SDK, debloque ~80% des blocs sans dupliquer de code) + place-break + world-gen. Sans cette famille, il n'y a litteralement pas de monde a jouer.
2. GAMEPLAY + ETRES + ECO (13✅ / 31◻, ~30% couvert). MVP jouable = world-grid + inventory + item-registry + recipe/craft + tool/mining + player-pos/move + command-op + save-game. C'est ce qui transforme le sandbox en JEU.
3. SENSORIEL/CAPTURE/RENDU (20✅ / 30◻, ~40% mais trompeur). La CAPTURE live est quasi absente (aucun getUserMedia/keylog cote ploxion ; seul web-heart prototype en local). La "machine a tsoins en direct" demandee par Jose = cam/mic/screen/key/mouse + recorder. Cote RENDU il manque voxel-mesh/sprites (pour AFFICHER les blocs Minecraft qui existent en logique mais sans apparence) + editeur-de-tsoins + timeline.
4. RESEAU EN PLOXIONS (11✅ / 33◻, ~25% couvert). 2 quick-wins immediats : (a) generer dhcp/quic/ssh/ws (residu DEJA pret dans la table PROTOCOLS, cout quasi nul) ; (b) ecrire port-bion + package-bion (les 2 gros chantiers "chaque PORT/chaque PAQUET = un ploxion" = directive bion-Linux). BLOQUANT prealable : protocol.rs est marque "SCAFFOLD NON-COMPILE", a compiler en batch calme anti-OOM avant tout.
5. MOTEUR TSOIN + SDK BIONS (15✅ / 24◻, ~38% couvert mais c'est le SOCLE). Trous de boite-a-outils qui bloquent tout le reste : AUCUN bion string ni collection general (math a 91 fns numeriques mais strings/listes quasi absents — UI/chat/commandes en ont besoin), pas de vraie compression (RLE-de-zeros seulement, or le but=shipper UN bion de 16Go), pas de Merkle/hash-fort expose sur le bus (collisions FNV documentees), pas de fork/branch/merge (timeline LINEAIRE -> impossible d'avoir saves multiples + multijoueur), pas de noise/RNG (terrain procedural impossible), pas de vec3/spatial (monde 3D impossible cote bions partages).

ORDRE RECOMMANDE pour Jose : (P0 socle) blockdef-extensions + string/collection/noise/vec3 bions + compiler protocol.rs ; (P1 monde vivant) place-break + world-gen + world-grid ; (P2 boucle de jeu) inventory + item-registry + craft + mining + player-pos + command-op + save-game ; (P3 voir le jeu) voxel-mesh + sprites + recorder/cam/mic ; (P4 vague reseau) dhcp/quic/ssh/ws + port-bion + package-bion ; (P5 ouverture contributeurs) pack/replicate + crypto-sign + validate-schema + sensor-generic-adapter (brancher une API sans Rust) -> RepoVerse.
