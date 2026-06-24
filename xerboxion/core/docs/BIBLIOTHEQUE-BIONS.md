# Bibliothèque universelle de bions + ploxions — XERBOXION

> Découverte (workflow 9 agents) de tous les building blocks à implémenter, depuis l'analyse de tous les
> logiciels. Chaque bion = fonction pure déterministe content-adressée. Voir le cadre dans
> docs/research/analyse-optimisation-decouverte-bions.md.

# Bibliotheque universelle de bions + ploxions du XERBOXION

> Directive Jose: « tous les logiciels -> des building blocks optimaux ». Chaque BION = fonction PURE deterministe content-adressee rejouable. Chaque PLOXION = service WASM sur le bus.
> Verifie READ-ONLY contre `ploxions/sdk/src/` (bions.rs 62 fn, math.rs 91 fn) et `docs/CATALOGUE.md`.
> Legende: **[A CREER]** / **[EXISTE]** (ne pas doubler) / **[EXPOSER]** (existe en interne host/tsoin, pas comme bion bus-callable) / **[EXTRAIRE]** (enferme inline dans un ploxion).
> Principe: « la primitive OPTIMALE, pas une de plus » — paires symetriques `decode(encode(x))==x` bit-exact.

---

## 0. SOCLE DEJA EN PLACE (ne PAS re-creer)
- **bions.rs**: JSON-read (`json_str/int/uint/num/array_body/split_objects`), `esc` (HTML aller seul) + `json_esc`, `to_hex/from_hex`, `fnv1a64/fnv1a64_bytes/addr64` (hash FAIBLE non-crypto, collisions documentees), `xor_delta/popcount_bytes/residu_minimal/residu_expand` (delta + RLE-de-zeros seulement), `lcg_next/lcg_bytes` (RNG NON-crypto predictible), `gen_apply` (replay), `DedupTable` (set exact refcount), `CoherenceWindow/GatedTick` (fenetre/seuil sync), `SeqItem/SeqCursor/schedule` (timeline triee (t,addr) — deja un index temporel + scheduler), `is_url`.
- **math.rs**: i32 complet (arith/bitwise/gcd/lcm/pow/log2/nextpow2/gray/popcount/clz/ctz/fib/lucas/collatz/isqrt/xorshift) + f64 SEULEMENT ~24 (add/sub/mul/div/affine, lerp, hypot, recip, sqrt, abs/neg, floor/ceil/round-ties-even/trunc, fmin/fmax, clamp, c2f/f2c, pct). **AUCUN trig/exp/ln/powf/vec/mat/quat/noise.**
- **vendor/tsoin**: BLAKE3 + Merkle + XOR-delta en INTERNE host (`blake3="1"`) — PAS exposes comme bions.
- **spectre/src** (DFT Hann+pics) et **synthe/src** (osc sine/saw/square/tri) = math audio ENFERME inline, pas des bions.
- Note collision: `sort_by` apparait a bions.rs:1144 mais c'est le tri INTERNE de SeqCursor (timeline), PAS un bion collection generique.

---

## 1. TEXTE / PARSING / FORMAT
La couche string est quasi VIDE (CATALOGUE l.39). Tout part du split.

### Bions [A CREER]
- `str_split` — `split(s,sep)->[str]; split_n(s,sep,max)` — COEUR universel: CSV brut, args commande, chemins, headers, lignes.
- `str_trim` — `trim(s)->str; trim_chars(s,set)` — 1ere etape de tout parseur (config INI/.env, chat, REPL).
- `str_join` — `join(parts,sep)->str` — inverse de split; reconstruit CSV/chemins/listes. **Paire: split<->join.**
- `str_case` — `upper/lower/title/ascii_fold(s)->str` — recherche insensible-casse, normalisation.
- `str_pad` — `pad(s,width,fill,align)->str` — alignement colonnes terminal/tableaux ASCII, padding hex.
- `str_replace` — `replace(s,from,to); replace_n(...,max)` — edition texte, sed, sanitization.
- `str_find` — `find/rfind/contains/starts_with/ends_with` — recherche litterale (BM-Horspool), base grep/autocomplete/routing, indices octets UTF-8-safe.
- `str_slice` — `slice(s,start,end)->str char-safe` — substring sans panique sur frontiere UTF-8 (bug Rust classique).
- `str_lines` — `lines(s)->[str]; wrap(s,width)->[str]` — decoupage retours-ligne; wrap mot-a-mot terminal/markdown.
- `str_count` — `char_len/byte_len/count_sub(s)->n` — stats editeur (char vs octet UTF-8), curseur, limites chat.
- `glob_match` — `glob(pattern,s)->bool` — filtrage fichiers/topics du bus, routing, gitignore-like (bien plus petit qu'un regex).
- `levenshtein` — `lev(a,b)->n; lev_bounded(a,b,max)` — fuzzy-search, did-you-mean, dedup quasi-doublons (CATALOGUE l.39).
- `jaro_winkler` — `jw(a,b)->milli 0..1000` — ranking fuzzy noms courts (meilleur que Lev sur prefixes), palette commandes.
- `url_codec` — `url_encode(s); url_decode(s)->opt` — liens xi0n/RepoVerse, query params. **Paire.** (`is_url` existe, pas l'encode.)
- `html_unescape` — `html_unescape(s)->str` — symetrique de `esc()` existant (aller seul); lire HTML/feeds. **Paire: esc<->html_unescape.**
- `base64` — `b64_encode(b); b64_decode(s)->opt; +url-safe` — liens/export/data-URI/tokens (CATALOGUE l.47). **Paire.**
- `query_kv` — `qs_parse(s)->[(k,v)]; qs_build(pairs)` — query-string + form-urlencoded (proto-http, adapters API, SSO). **Paire.**
- `csv_row` — `csv_parse_line(s,sep); csv_quote(field)` — import/export tabulaire, gere guillemets+sep-echappe. **Paire.**
- `kv_parse` — `kv_parse(s)->[(k,v)] INI/.env` — config plate (.env labo, INI); 80% du TOML/INI sans parseur complet.
- `tokenize` — `tokenize(s)->[Token kind/start/end]` — lexer generique: brique de TOUT compilateur/REPL/coloration/xerlang.
- `text_diff` — `diff_lines(a,b)->[Eq/Ins/Del] Myers` — diff TEXTE ligne-a-ligne (le ploxion diff existant = XOR BINAIRE, casse si longueur change, CATALOGUE l.48).
- `template_render` — `render(tpl,vars)->str ({{key}})` — interpolation deterministe (chat, prompts, gen config/code).
- `slugify` — `slugify(s)->str ascii-lower-tirets` — identifiants stables (URLs RepoVerse, noms ploxions, ancres markdown).

### Ploxions [A CREER]
- **parse** — `{format,raw}` (json/yaml/toml/ini/csv/querystring/kv) -> AST normalise. Symetrique **serialize**. Reutilise csv_row/kv_parse/query_kv + JSON-read existant.
- **textops** — coreutils-texte streamable: grep (find+glob), sed (replace_n), wrap/align, wc, sort/uniq/dedup.
- **textdiff** — diff/patch TEXTE (sur text_diff/Myers): patch unifie (@@ hunks), apply, 3-way merge ligne. Coeur versioning RepoVerse + undo-tree.
- **lexer** — `{source,langspec}` -> flux tokens pour coloration editeur, REPL xerlang. Sur tokenize, table mots-cles en config.
- **markdown** — markdown->html (et ->texte) deterministe: wiki labo, chat, README. Sur str_lines/wrap/html-escape. Content-adressable.
- **template** — `{{var}}/{{#if}}/{{#each}}`, partials, echappement contextuel html/json/url. Utile a la forge + auto-replication von Neumann.

---

## 2. DONNEES / COLLECTIONS / STRUCTURES
Couche VIDE = P0 socle explicite (CATALOGUE l.40, l.251). Aucun sort/search/group n'existe. Map/filter/fold NON proposes (deja fournis par Iterator de Rust).

### Bions [A CREER]
- `sort_u64` — `sort_u64(&mut [u64])` — tri stable in-place; base de TOUT (ORDER BY, leaderboard, merge timelines).
- `sort_by_key` — `sort_by_key(&mut [u64], key)` — tri par cle extraite (addr64, prix, t). Le ORDER BY universel.
- `argsort` — `argsort(&[u64])->Vec<u32>` — indices tries sans bouger les donnees; jointures, vues triees (pandas/SQL index).
- `binary_search` — `binary_search(&[u64],x)->Result<usize,usize>` — O(log n) + point d'insertion; lookup store trie, autocomplete.
- `lower_bound` — `lower_bound(&[u64],x)->usize` — borne inf range-scan [a,b); index B-tree, fenetres temporelles (cf SeqCursor).
- `dedup_sorted` — `dedup_sorted(&mut [u64])->usize` — unique sur slice trie (DISTINCT SQL, set-union).
- `merge_sorted` — `merge_sorted(&[u64],&[u64])->Vec<u64>` — fusion 2 listes triees; merge-sort externe, fusion 2 timelines.
- `partition3` — `partition3(&mut [u64],pivot)->(usize,usize)` — Dutch-flag <,=,>; quickselect, top-k, WHERE.
- `group_by_key` — `group_by_key(&[(u64,u64)])->Vec<(u64,&[..])>` — GROUP BY sur entrees triees -> runs; agreg inventaire.
- `run_length` — `run_length(&[u64])->Vec<(u64,u32)>` — RLE/group adjacent, compression de blocs identiques.
- `chunk_bounds` — `chunk_bounds(len,n)->Iter<(usize,usize)>` — pagination, batching reseau, world-chunks 16x16.
- `window_bounds` — `window_bounds(len,w)->Iter<(usize,usize)>` — n-grams, moyenne mobile (generalise CoherenceWindow).
- `bsearch_insert` — `insert_sorted(&mut Vec<u64>,x)->usize` — index incremental, priorite sans heap pour petit N.
- `heap_push/heap_pop` — min-heap sur Vec — priority-queue Dijkstra, scheduler tsoin, top-k streaming.
- `RingBuf` — `push(x)/iter() cap fixe` — logs recents, audio/frame buffer, replay window borne (generalise CoherenceWindow).
- `UnionFind` — `new(n)/find/union/components` — composantes connexes, cycle, clustering; merge graphes RepoVerse, Kruskal.
- `bfs_order` — `bfs(&[Vec<u32>],start)->Vec<u32>` — largeur; plus-court-chemin non-pondere, flood-fill, propagation deps.
- `topo_sort` — `topo_sort(n,&[(u32,u32)])->Option<Vec<u32>>` — ordre topo (None si cycle); resolveur deps (CATALOGUE l.93), build-order, eval dataflow xerlang.
- `dijkstra` — `dijkstra(&[Vec<(u32,u32)>],src)->Vec<u32>` — plus-court-chemin pondere; routing net, carte 4D.
- `bloom` — `Bloom::new(bits,k)/add(h)/maybe(h)->bool` — set probabiliste compact (FNV deja la); dedup gros corpus, anti-rejeu.
- `trie_radix` — `Trie::insert/get/prefix` — index par prefixe d'octets; autocomplete, routing topics (osiris.*).
- `lru_touch` — `Lru::new(cap)/touch/get` — cache borne (eviction); cache decode WASM, hot-set du store.

### Ploxions [A CREER]
- **data-index** (nommer index2 pour eviter collision avec le ploxion stats `index` existant) — index secondaire LIVE: ingere (cle->addr64), garde slice TRIE, repond `index.get/range/prefix`. Le CREATE INDEX du bus. Sur binary_search+lower_bound+trie.
- **cache** — LRU partage devant le tsoin-store content-addressed: `cache.get/put`, hit RAM bornee / miss delegue. Le memcached du xerboxion, capacite fixe (anti-OOM). Sur lru_touch.
- **graph** — organe graphe: `graph.edge`, `graph.bfs/topo/dijkstra/components`. Sert resolveur deps (CATALOGUE l.93), build-order, carte 4D, RepoVerse. Sur bfs/topo/dijkstra/UnionFind.
- **dedup-set** — set probabiliste: `dedup.seen?/add` via Bloom borne. Anti-rejeu, idempotence d'ingest, filtre amont avant le store exact.

---

## 3. MATH / NUMERIQUE / GEOMETRIE / SIMULATION
math.rs n'a que le scalaire. AUCUN trig/vec/mat/quat/noise (CATALOGUE l.44,52,54).

### Bions [A CREER]
- `f64 trig` — `sin/cos/tan(x)` — socle de TOUTE rotation/onde/oscillation (Blender, Unity, shaders, LFO Ableton).
- `f64 inv-trig` — `atan2(y,x)/asin/acos` — heading d'un vecteur, IK, navigation (prerequis vec2::angle).
- `f64 trans` — `exp/ln/log2/powf` — croissance, gamma-correction, softmax ML, dB audio (log2 i32 existe, pas f64).
- `f64 glue` — `deg2rad/rad2deg/fmod/fract/copysign/smoothstep/sign` — pipeline graphique/anim (smoothstep GPU/CSS, fmod wrap d'angle).
- `vec2` — `v2_add/sub/scale/dot/len/normalize/perp/angle/rotate` — UI/2D/tilemaps/physique 2D (Box2D, carte 4D).
- `vec3` — `v3_add/sub/scale/dot/cross/len/normalize/dist/reflect/lerp` — **debloque le monde 3D** (Unity/Blender, raycast, eclairage, chunk-coord Minecraft-Nexus).
- `vec4 / rgba` — `v4_add/scale/dot/lerp + rgba pack/unpack` — couleurs (blend, premultiply) + coords homogenes mat4.
- `mat4` — `m4_identity/mul/transpose/translate/scale/rotate/perspective/look_at/transform_point` — pipeline rendu/CAO model->view->clip. Sans elle pas de camera/3d ploxion.
- `mat3 / mat2` — `m3_det/inverse/mul, m2_det/inverse` — normal-matrix (eclairage), systemes 2D, IK.
- `quat` — `q_from_axis_angle/from_euler/mul/normalize/slerp/rotate_vec3/conjugate` — rotations 3D sans gimbal-lock; slerp = replay lisse keyframes du xion.
- `pcg32/rng_f64` — `pcg_next(state)->(u32,state); rng_f64(state)->(f64,state)` — **DERIVE de lcg_next existant**, pas un nouveau RNG. Procgen, Monte-Carlo, dropout, des. Deterministe-rejouable.
- `noise` — `perlin3(x,y,z,seed); fbm(...,oct,lacun,gain)` — terrain procedural (CATALOGUE l.44 IMPOSSIBLE sans). Derive du meme PCG = rejouable.
- `voronoi/worley` — `worley2(x,y,seed)->(f1,f2); voronoi_cell->id` — biomes, ecailles, cracks, Whittaker maps.
- `interp` — `bezier3/catmull_rom/hermite/ease_*(t)` — courbes After-Effects/Figma/CSS, splines camera, lissage trajectoire entre tsoins.
- `aabb` — `aabb_contains/intersect/union/center/half/expand` — broadphase collision, frustum-culling, BVH, hit-test UI, chunk bounds.
- `raycast` — `ray_aabb/ray_sphere/ray_plane/ray_triangle->Option<t>` — picking souris, line-of-sight, voxel-DDA (place/break MC).
- `sdf` — `sd_sphere/box/torus + op_union/smooth_union/subtract` — modeling implicite (geometry-nodes, ShaderToy), collision lisse, MSDF font.
- `stats` — `mean/variance/std/median/percentile/min_max/correlation/histogram` sur &[f64] — analytics, profiling, ML, audio RMS.
- `fft` — `fft_inplace(re,im); rfft_mag(samples)->Vec<f64>` — spectral audio, filtrage image, MFCC (nextpow2 i32 deja la pour pad).

### Bions [EXPOSER]
- `merkle` — `hash_leaf(b)->[u8;32]; merkle_root(leaves); merkle_proof/verify` — le store fait deja BLAKE3/Merkle mais PAS en fn pure (CATALOGUE l.35). content-addressing = coeur du xer.

### Bions [A CREER — determinisme]
- `fixed-point Q16.16 / u128` — `fx_from_f64/fx_mul/fx_div/fx_to_f64; mul128_hi` — **rejouabilite bit-exact cross-CPU** (trig f64 via libm peut diverger entre archi). Lockstep multijoueur (Factorio/StarCraft).

### Ploxions [A CREER]
- **px-noise** — champs bruit/voronoi parametres (seed,octaves,freq) en streaming pour world-gen/textures. Wrappe noise/fbm/worley -> chunks consommables par world-grid. Rejouable car seede.
- **px-rng** — source d'alea seedee: `uniform/normal/shuffle/pick` + log du state. Un seul lignage d'alea auditable pour TOUS les ploxions.
- **px-stats** — agregateur online (mean/std/percentile/histogram/correlation glissants via CoherenceWindow existante). Dashboard OSIRIS, profiling, anomalie.
- **px-spectrum** — flux echantillons -> magnitude spectrale/RMS/peak/MFCC via fft. Visualiseur audio, EQ, feature ML. Pont zones-son + web-heart.
- **px-geom** — index AABB/BVH des entites + raycast (picking, line-of-sight, place/break voxel). Debloque la boucle de jeu 3D.
- **px-transform** — compose hierarchies mat4/quat parent->enfant -> matrices monde + interpole keyframes (slerp/bezier). Camera + replay cinematique.

> CONTRAINTE DETERMINISME a signaler a Jose: trig/exp f64 (libm) diverge bit-a-bit entre archi -> pour lockstep/replay bit-exact, fournir AUSSI la voie fixed-point Q16.16.

---

## 4. CRYPTO / HASH / SECURITE
addr64/FNV = adressage approx (collisions documentees), PAS la securite. BLAKE3/Merkle existent host-only.

### Bions [EXPOSER]
- `blake3` — `blake3(bytes)->[u8;32]` — l'adresse-contenu VRAIE (vs addr64). Deja dans le moteur tsoin, a exposer. Store/saves/RepoVerse/dedup.
- `merkle_root` — `merkle_root(leaves)->[u8;32]` — integrite d'un save = 1 hash, sync multi (compare racines), commit RepoVerse.

### Bions [A CREER]
- `blake3_keyed` — `blake3_keyed(key[32],bytes)->[u8;32]` — MAC/derivation en un appel; remplace HMAC+SHA pour 80% (tokens, integrite warp).
- `blake3_kdf` — `blake3_kdf(context,key)->[u8;32]` — KDF context-separe (HKDF); cle de chiffrement par scope (warp E2E, vault). Evite argon2/hkdf separes.
- `sha256` — `sha256(bytes)->[u8;32]` — interop git objects/blockchain/JWT/checksums. Le seul SHA a garder.
- `hmac_sha256` — `hmac_sha256(key,bytes)->[u8;32]` — auth format-standard (webhooks, AWS sig, OAuth, cookies). Interop quand l'autre bout n'est pas BLAKE3.
- `crc32` — `crc32(bytes)->u32` — checksum non-crypto rapide (zip/png/ethernet/gzip), 90% moins cher que blake3.
- `merkle_proof` — `merkle_proof(leaves,index)->[[u8;32]]` — prouver 'ce tsoin est dans ce save' sans le save entier; sync incrementale, light-client.
- `merkle_verify` — `merkle_verify(leaf,index,proof,root)->bool` — verif inclusion O(log n). Socle confiance multi-machine flotte.
- `ed25519_sign` — `ed25519_sign(sk[32],msg)->[u8;64]` — qui a produit ce bion? signatures save, identite machine.
- `ed25519_verify` — `ed25519_verify(pk[32],msg,sig[64])->bool` — le bus accepte SI signe par cle connue; gate ingress /boxion/*.
- `ed25519_pk` — `ed25519_pk(sk[32])->[u8;32]` — derive pubkey (identite = pubkey).
- `chacha20poly1305` — `seal(key,nonce,pt)->ct / open(...)->Option<pt>` — la SEULE AEAD (constant-time, pas besoin AES-NI sur le VPS). warp E2E, vault, save chiffre.
- `base64url` — `b64url_encode/decode` — URL-safe sans padding; hash/cles dans URLs xi0n, tokens.
- `base58` — `base58_encode/decode` — sans 0/O/l/I (bitcoin/IPFS); adresses-tsoin lisibles a la voix. Optionnel.
- `ct_eq` — `ct_eq(a,b)->bool` — comparaison temps-constant. **NON-NEGOCIABLE**: sans lui tout verify (token/MAC) fuit par timing.
- `csprng_bytes` — `csprng_fill(seed[32],n)->[u8;n]` — ChaCha20-CSPRNG; cles ed25519, nonces AEAD, salts. DISTINCT de lcg (lcg predictible, interdit pour cles).

### Ploxions [A CREER]
- **hash** (priorite #1) — expose la hash-family (blake3/keyed/kdf/sha256/hmac/crc32 + merkle root/proof/verify). `hash.blake3 / hash.merkle.*`. Debloque integrite saves + sync multi + RepoVerse.
- **sign** — garde cles ed25519, signe sortant / verifie entrant contre trousseau. `sign.sign/verify/pubkey/trust`. Gate ingress + valide package-bion entrant.
- **vault** — KDF par scope, seal/open ChaCha20-Poly1305, CSPRNG, secrets chiffres labo. `vault.seal/open/derive/gen`. Save chiffre, .env chiffres.
- **captoken** — capability-tokens xi0n: scope+expiry+ressource signe encode base64url. `cap.mint/check` (ct_eq sur verif). Remplace les sessions par des liens-capability.

> Optimalite « une primitive de plus, pas deux »: BLAKE3 sert hash+MAC+KDF (sha256/hmac gardes UNIQUEMENT interop externe). ChaCha20-Poly1305 = seule AEAD. xxhash/murmur3 ECARTES (crc32 + blake3 couvrent).

---

## 5. COMPRESSION / ENCODING / SERIALISATION (bion 16Go)
CATALOGUE l.36/46/47 flague compress/varint/base64 comme manques bloquant le bion 16Go. residu_minimal = RLE-de-ZEROS seulement.

### Bions [A CREER] (paires enc/dec round-trip bit-exact)
- `varint_enc/dec` — LEB128 1-10 octets; **brique de TOUTE serialisation binaire compacte** (protobuf/msgpack/sqlite), tue le hex-in-JSON 2x.
- `zigzag_enc/dec` — `(n<<1)^(n>>63)` <-> inverse; map signe->non-signe pour que varint compresse les petits negatifs (deltas).
- `delta_enc/dec` — premieres differences <-> somme prefixe; compresse sequences monotones (timestamps tsoins, coords carte 4D). Base du replay.
- `delta2_enc/dec` — delta-of-delta (Gorilla); cadence reguliere -> quasi-zero (ticker CoherenceWindow, frames cam).
- `rle_enc/dec` — RLE GENERAL (run de n'importe quel octet, pas que zero); textures/blocs MC uniformes, masques, bitmaps.
- `bitpack/bitunpack` — N entiers sur exactement `bits` bits; **coeur du stockage chunk MC** (palette 16 blocs=4 bits), LOD, flags, colonnes faible cardinalite.
- `base64_enc/dec` — base64url sans padding; liens xi0n/RepoVerse, tokens, export (33% overhead vs 100% hex). (cf base64 domaine texte/crypto — unifier.)
- `lz_enc/dec` — LZ77/LZ4-like (match offset/len + litteraux, fenetre glissante); **la vraie compression generique**, le seul chemin credible vers le bion 16Go. Decodeur deterministe et borne (anti-OOM).
- `crc32` — checksum integrite rapide; detecte corruption d'un chunk AVANT decompression (cf crc32 domaine crypto — unifier).

### Ploxions [A CREER]
- **compress** — pipeline complete: `compress.varint/rle/lz/delta/bitpack` (enc+dec), choisit la methode par heuristique entropie/ratio + header 1-octet auto-descriptif. Manque #1 du catalogue.
- **store-compresse** — store content-addressed ou chaque blob est compresse (lz+delta selon type) puis adresse par le hash du CLAIR. Round-trip verifie (re-decompresse/re-hash/match) avant ack. Socle du bion 16Go et des saves portables.
- **codec-columnar** — struct-of-arrays: records (tsoins/events/lignes carte 4D) stockes colonne par colonne (delta2+zigzag+varint+bitpack selon type) -> ratio bien meilleur que row-wise. Blob msgpack-like auto-descriptif.

---

## 6. SYSTEME / FICHIERS / TEMPS / CONCURRENCE / SCHEDULING
Distinction CLEF: temps IN-GAME (tick) vs COHERENCE (clock-coherence existant) — deux horloges, ne PAS fusionner. clock.now est consomme partout mais JAMAIS produit (trou critique).

### Bions [A CREER]
- `path_join` — `path_join(parts)->String` — normalise les `/` multiples; base de tout fs-adapter, save-path, resolution mods/ploxions.
- `path_normalize` — `path_normalize(p)->String` — resout ./.. SANS toucher le disque (pur); anti `../escape` contributeurs RepoVerse.
- `path_split` — `path_split(p)->(dir,base)` — basename/dirname de coreutils/git en un passage.
- `path_ext` — `path_ext(p)->Option<&str>` — routage par type (codec image, loader, mime).
- `parse_iso8601` — `parse_iso8601(s)->Option<i64>` — ISO-8601 -> epoch (pur, gregorien); ingestion adapters rss/cal/wikidata. Le strptime universel.
- `format_iso8601` — `format_iso8601(epoch)->String` — inverse; horodatage humain triable des saves.
- `civil_from_days` — `civil_from_days(days)->(y,m,d)` — Howard Hinnant; brique PURE sous parse/format ISO + calendrier in-game. Rien n'est correct sans elle.
- `tick_to_gameclock` — `tick_to_gameclock(tick,tpd)->(h,m)` — temps LOGIQUE in-game (distinct coherence); cycle jour/nuit MC.
- `daynight_phase` — `daynight_phase(tick,tpd)->u8` — aube/jour/crepuscule/nuit; lumiere, mob-spawn, PNJ.
- `duration_fmt` — `duration_fmt(secs)->'1h02m03s'` — cooldowns, uptime, ETA, logs scheduler.
- `cooldown_ready` — `cooldown_ready(last,now,cd)->bool` — sorts/outils/portails/throttle (redstone, rate PNJ).
- `backoff_delay` — `backoff_delay(attempt,base,cap)->u64` — backoff exponentiel borne (jitter via lcg); reconnexion adapters, retry net.
- `cron_due` — `cron_due(spec,tick,period)->bool` — `*/N`,`0`,`5..10` sur t logique; brique du scheduler, rejouable.
- `rate_token` — `rate_token(&mut tokens,cap,refill,dt)->bool` — token-bucket; anti-flood bus, limite spawn/commandes/IO (nginx/envoy).

### Bions [A CREER — string socle] (recoupe domaine 1, unifier)
- `str_split` — `str_split(s,sep)->Vec<&str>` — le seul split du SDK est split_objects JSON; commandes /give, chemins, topics.
- `str_trim_pad` — `str_trim_pad(s,width,pad)->String` — trim+pad largeur fixe; HUD, tables d'index, console.

### Ploxions [A CREER]
- **scheduler** — SCHEDULER sur temps LOGIQUE: requires `clock.now` + `sched.add{at|every|after,topic,payload}` + `sched.cancel`; provides `sched.fire`. Maintient un SeqCursor (deja SDK) de jobs, refire via cron_due/backoff_delay. Timers, delais redstone, croissance, respawn, retry. Rejouable.
- **clock-adapter** — PRODUIT `clock.now` que personne n'emet (trou critique). Cote host, bat un tick cadence fixe + emet temps in-game (tick_to_gameclock/daynight_phase) DISTINCT de la coherence.
- **file-watcher** — WATCH+DIFF (le watcher actuel ne fait que service.health): requires `fs.entry{path,hash,size,mtime}`; provides `fs.changed{kind}`. DedupTable d'empreintes, diffe par scan + tsoin.record. inotify/watchman du xion, hot-reload mods.
- **lock** — VERROUS LOGIQUES + atomic sans threads OS: `lock.acquire/release`; `lock.granted/denied`. Mutex/sema deterministes (file (t,owner)), idempotents rejouables. Serialise place/break, edits save-slot. Aucune primitive de concurrence n'existe.
- **fsm** — state-machine data-driven (house.rs n'a qu'un Rule.decide cable maison): `fsm.load{states,transitions}` + `fsm.event`; `fsm.state{from,to}`. Transitions table-drivees, grave chaque pas en tsoin. PNJ, portes/redstone, quetes, sessions, retry.

---

## 7. MEDIA / GRAPHIQUE / AUDIO / SIGNAL / RENDU
Tout l'audio-math existe mais ENFERME inline dans spectre/synthe; 1er travail bas-cout = EXTRAIRE.

### Bions [A CREER]
- `vec3` — `v3_add/sub/scale/dot/cross/len/norm` — socle 3D (cf domaine 3, unifier). Rien de mesh/camera/voxel/physique sans lui.
- `clamp01_smoothstep` — `clamp01/smoothstep(e0,e1,x)/remap(x,a,b,c,d)` — normalisation signal partout (easing, gradient, fade, antialias edge).
- `ease` — `ease(kind:u8,t01)->f64 (lin/quad/cubic/sine/expo/back/elastic in-out)` — UNE fn parametree = 30+ courbes (CSS/Unity/Lottie/GSAP).
- `rgb_hsv` — `rgb2hsv/hsv2rgb` — color picker / teinte de tout editeur image + palettes.
- `color_blend` — `blend(mode:u8,src,dst)->rgba (normal/mult/screen/overlay/add/alpha)` — compositing Photoshop/Figma/CSS/GPU (standard fige).
- `color_pack` — `pack_rgba8/unpack_rgba8/lerp_rgba` — pixel canonique 0xAARRGGBB; gradients, antialias, transitions. Unite de tout framebuffer.
- `srgb_lin` — `srgb2lin/lin2srgb` — gamma correct OBLIGATOIRE pour blend/blur/resize justes (PBR, OpenColorIO).
- `luma_contrast` — `luma(r,g,b)/adjust(c,bright,contrast,gamma)` — grading/desat/N&B Rec.709 (Lightroom, ffmpeg eq, OBS).
- `quantize_dither` — `quant(c,levels); bayer4(x,y)` — reduction palette + tramage (GIF/PNG-8, pixel-art, e-ink).
- `bilinear` — `bilerp(c00,c10,c01,c11,fx,fy)` — noyau atomique resize/rotate/UV-sampling (tout resampler GPU/ImageMagick).
- `conv3` — `conv3x3(p[9],k[9]); box/gauss/sobel_kernel` — flou/nettete/edge; 1 noyau 3x3 + kernels couvre tout editeur.
- `value_noise` — `vnoise2(x,y,seed); fbm2(...,oct)` — terrain procedural (CATALOGUE), branche sur le LCG existant (plus simple que Perlin).
- `line_circle` — `bresenham; midpoint_circle` — raster de base (crayon, formes, HUD); entiers exacts, content-adressables.
- `bezier` — `bezier2/bezier3(t); arc_len` — SVG/Illustrator/fonts/animation paths.
- `mat4` — `m4_mul/identity/translate/scale/rotate/perspective/look_at` — pipeline camera/projection (cf domaine 3, unifier).
- `aabb` — `aabb_overlap; ray_aabb` — collision + picking + culling chunks voxel (cf domaine 3, unifier).
- `morton` — `morton3_encode(x,y,z)->u64; morton3_decode` — indexation spatiale voxels/octree (cle chunk locale au cache); MC, MagicaVoxel, GPU sparse.
- `rle_codec` — `rle_encode/rle_decode` — compression palette/run d'image et chunk voxel (cf domaine 5, unifier).
- `midi_note` — `midi2freq/freq2midi` — clavier->frequence (A440/12-TET); piano-roll, WebMIDI, sequenceurs.
- `db_gain` — `db2lin/lin2db; pan2(g,pan)` — faders dB, panoramique equal-power (table de mixage DAW/OBS).

### Bions [EXTRAIRE de spectre/synthe — bas cout]
- `audio_osc` — `osc(wave:u8,phase01); detune; pcm_i16(f64)` — extraire l'oscillateur de synthe.rs (sine/saw/square/tri); tout synth/DAW.
- `adsr` — `adsr(t,a,d,s,r,dur)->gain 0..1` — enveloppe d'amplitude de toute note; A/D/S/R standard, rejouable.
- `biquad` — `biquad_coeffs(kind,fc,q,sr)->[5]; biquad_step(state,coeffs,x)->(y,state)` — lowpass/highpass/bandpass/peak (RBJ cookbook); coeur DSP de tout EQ/synth.

### Ploxions [A CREER]
- **voxel-mesh** (PIECE MANQUANTE #1, CATALOGUE l.194) — greedy-meshing d'un chunk -> faces visibles (pos+UV+normales). `voxel.chunk -> mesh.faces`. DONNE UNE APPARENCE aux blocs MC qui n'existent qu'en logique.
- **image-codec** — pixels-bions <-> PNG/BMP (decode+encode via rle_codec+color_pack). `image.decode/encode`. Vignettes/textures/atlas sur le bus.
- **image-op** — resize/crop/rotate/convolve/grade/quantize+dither sur RGBA. Editeur image headless du xion (compose les bions). `image.op`.
- **sprite-atlas** — packe N sprites en 1 atlas (bin-packing) + table UV deterministe; rend une tilemap. Textures blocs/glyphes koin-kion-wormion.
- **dsp-chain** — chaine audio rejouable osc->adsr->biquad->gain/pan->PCM en JSON. Comble web-synthe VIDE. `synth.patch -> audio.pcm` content-adresse.
- **waveform-viz** — PCM -> waveform (min/max par pixel) + spectrogramme (reutilise la DFT spectre par fenetres). Comble web-spectre VIDE; visualiseur du giga-tsoin sonore.
- **keyframe-anim** — interpole pistes (pos/scale/color/glyph) via ease+bezier a l'instant t du tsoin. Timeline commune (After Effects/Lottie) sur l'horloge de coherence.
- **palette** — extrait palette N couleurs (median-cut/k-means en rgb_hsv) + gradients/harmonies. Sert quantize, themes labo --xr-*, dither.
- **camera-project** — mat4 model/view/proj + ray_aabb: projette le monde voxel en ecran + picking/culling. Camera partagee (replay cinematique, picking au clic) pour web-3d/cub4ion.

---

## 8. BASE DE DONNEES / REQUETES / ETAT / SYNC
CATALOGUE flague merge/merkle/query/index/export comme ◻. Se compose sur les collection-bions (domaine 2) + CRDT.

### Bions [A CREER]
- `filter_idx` — `filter_idx(n,pred)->Vec<u32>` — WHERE: indices ou pred vrai sans materialiser. Base de query-ploxion.
- `order_by_key` — `order_by_key(idx,key)` — ORDER BY: tri STABLE des indices (timsort-like). Stable = deterministe/rejouable.
- `top_k` — `top_k(idx,k,score)->Vec<u32>` — LIMIT+ORDER fusionnes via heap borne O(n log k); top-k tsoins surprenants, leaderboard.
- `group_count` — `group_count(keys)->Vec<(u64,u32)>` — GROUP BY + COUNT une passe; count-by-gen du store, histogrammes.
- `aggregate` — `aggregate(vals,op)->i64` — SUM/MIN/MAX/AVG/COUNT; deterministe sur entiers (pas de drift float = rejouable).
- `join_hash` — `join_hash(l,r)->Vec<(u32,u32)>` — INNER JOIN hash-join (build droit, probe gauche); refs entre tsoins.
- `range_scan` — `range_scan(sorted,lo,hi)->(usize,usize)` — BETWEEN via double binary_search; range-temps du corpus (t0..t1).
- `merge_lww` — `merge_lww(a,b)->&[u8]` — CRDT Last-Write-Wins (max timestamp, tie-break addr64); register multijoueur. Le merge le plus simple qui converge.
- `merge_gcounter` — `merge_gcounter(a,b)` — CRDT G-Counter (max par replica, somme=valeur); compteurs commutatifs/idempotents (vues, kills, ressources).
- `merge_orset` — `merge_orset(a,b,tombs)` — CRDT OR-Set (ajouts tagges moins tombstones); inventaire multijoueur, listes, presence.
- `merge3` — `merge3(base,ours,theirs)->Result<_,Conflict>` — 3-way merge git-like; signale hunks en conflit. **LE chainon multijoueur** (fork/branch/merge timelines, saves, RepoVerse).
- `merkle_root` — `merkle_root(leaves)->u64` — 1 empreinte de tout l'etat; integrite saves, anti-entropie (comparer racines).
- `merkle_proof` — `merkle_proof(leaves,i) + verify(leaf,proof,root)->bool` — preuve inclusion compacte (log n); light-client RepoVerse.
- `validate_schema` — `validate_schema(json,schema)->Vec<Error>` — subset JSON-Schema; INSERT validation, contrat PLC, onboarding contributeur sans Rust.
- `varint_enc/dec` — LEB128 (cf domaine 5, unifier); WAL, index keys, delta-of-delta.
- `wal_append` — `wal_append(&mut log,lsn,op)->u64` — Write-Ahead-Log (LSN+len+crc); transaction/journal, undo-redo, event-sourcing.
- `wal_replay` — `wal_replay(log,apply)->u64` — rejoue jusqu'au dernier LSN valide (stop a la 1ere corruption); recovery apres crash.
- `cdc_diff` — `cdc_diff(old,new)->Vec<Change>` — Change-Data-Capture (Insert/Update/Delete depuis 2 snapshots key->hash); replication, sync, dirty-tracking.

### Ploxions [A CREER]
- **query** — moteur SELECT sur le corpus addr64: `query.run{filter,project,join,group,order,limit/top_k} -> query.result`. Compose filter_idx/range_scan/group_count/aggregate/top_k/join_hash. Top-k surprenants, count-by-gen, BETWEEN range-temps.
- **merge** — reconciliation d'etats divergents (multijoueur): `merge.request{base,ours,theirs,strategy} -> merge.result`. Dispatch merge3/merge_lww/merge_gcounter/merge_orset. Saves multiples, forks timeline, multi-joueur convergent (Nexus).
- **merkle** (CATALOGUE 'merkle organe' ◻) — `merkle.root/proof/verify/diff{rootA,rootB}`. Integrite saves, audit du store, sync anti-entropie (echanger seulement ce qui differe), light-client.
- **index** — index secondaires persistants: `index.build{field,kind:btree|hash|inverted}/lookup/range`. B-tree (range/order), hash (egalite), inverted (plein-texte labels/topics). Requetes rapides sans full-scan.
- **wal** — journal transactionnel + snapshot: `tx.begin/append/commit` + `journal.replay` + `snapshot.take/restore`. Durabilite, recovery, export/import save portable (CATALOGUE ◻).
- **replicate** — gossip flotte: cdc_diff produit les changements, merkle.diff borne le transfert, merge reconcilie. Anti-entropie pull/push idempotent. Sync multi-noeud (boxion<->PC<->prod), RepoVerse distribue, sans serveur central.

---

## CONVENTIONS TRANSVERSES
- **Unification cross-domaine** (meme bion propose par plusieurs agents — implementer UNE fois): `vec3`/`mat4`/`aabb`/`raycast` (3+7), `varint_enc/dec` (5+8), `base64/base64url` (1+4+5), `crc32` (4+5), `rle` (5+7), `merkle_root/proof/verify` (3+4+8), `str_split` (1+6), `glob_match` (1+6), `ease/bezier` (3+7), `value_noise/fbm` (3+7), `stats` (3). Eviter les doublons.
- **Piege UTF-8**: str_slice/str_count travaillent en frontieres-char (byte-index!=char-boundary fait paniquer Rust).
- **Determinisme**: tout decodeur borne (anti-OOM/boucle infinie). RNG jeu = lcg (predictible OK), RNG cles = csprng (interdit lcg). trig f64 epsilon-pres cross-archi -> fixed-point Q16.16 pour lockstep bit-exact.
- **Collision de nommage**: le ploxion data-index s'appelle `index2`/`data-index` (le ploxion `index` existant = stats/scoreboard).


---

## Les 12 bions fondamentaux (à faire en premier)
- str_split — split(s,sep)->[str] : toute donnee texte ENTRE par un split (CSV, args, chemins, headers, lignes, topics). Le bion le plus reutilise, prerequis du parsing/routing/UI/chat/commandes. [A CREER]
- str_find/contains — find/rfind/contains/starts_with/ends_with : toute recherche/routing/parsing = 'ou est ce motif'. Base grep/autocomplete/routing URL, indices octets UTF-8-safe. [A CREER]
- sort_by_key — sort_by_key(&mut[u64],key) : tri stable par cle, primitive-racine du domaine data (ORDER BY/leaderboard/merge/group reposent dessus). La couche collection est P0 socle VIDE. [A CREER]
- binary_search/lower_bound — recherche+range O(log n) : socle de TOUT index, range-query, BETWEEN, lookup dans store trie, autocomplete. [A CREER]
- group_by_key/group_count — agregation : base de GROUP BY et de toute statistique/histogramme/count-by-generateur. [A CREER]
- vec3 — v3_add/sub/scale/dot/cross/len/normalize/lerp : la cellule du monde 3D, debloque rendu+physique+raycast+camera+chunk-coord Minecraft-Nexus. Monde 3D IMPOSSIBLE sans (CATALOGUE l.54). [A CREER]
- f64 sin/cos/atan2 + exp/ln — le socle transcendant absent de math.rs : prerequis de TOUTE rotation/onde/quaternion/noise/easing. [A CREER]
- pcg32/rng_f64 — DERIVE de lcg_next existant : sans alea DETERMINISTE-rejouable, ni noise, ni procgen, ni Monte-Carlo, ni shuffle. Respecte bion=pur/rejouable. [A CREER, derive existant]
- blake3 — l'adresse-contenu VRAIE (vs addr64/FNV qui collisionne) : store tsoin, integrite saves, RepoVerse, dedup. Deja dans le moteur, juste a EXPOSER comme bion bus-callable. [EXPOSER]
- merkle_root/proof/verify — transforme integrite d'un save/sync en O(log n) : socle de la confiance multi-machine de la flotte et de la sync anti-entropie. [EXPOSER root + A CREER proof/verify]
- varint_enc/dec — LEB128 : brique de TOUTE serialisation binaire compacte (protobuf/msgpack/WAL/index/delta-of-delta), tue le hex-in-JSON 2x. Le seul chemin credible vers le bion 16Go avec lz. [A CREER]
- text_diff (Myers/LCS) — diff_lines(a,b)->[Eq/Ins/Del] : le RESIDU du domaine texte = la moitie record du tsoin appliquee au texte. Sans elle pas de versioning/undo/merge ni rejouabilite (le ploxion diff actuel = XOR BINAIRE, casse si longueur change, CATALOGUE l.48). [A CREER]

## Stratégie

STRATEGIE — par ou commencer pour des building blocks optimaux, alignee sur le P0 du catalogue (docs/CATALOGUE.md l.35-58, l.251-253 qui flague EXACTEMENT ces trous: hash/string/collection/vec3/compression/merge/noise).

CONSTAT VERIFIE READ-ONLY: l'arithmetique scalaire est riche (math.rs 91 fn) mais 7 couches entieres sont VIDES — aucun bion string, aucune collection (sort/search/group), aucun vec/mat/quat/trig/noise, aucune vraie compression (RLE-de-zeros seulement), BLAKE3/Merkle existent mais host-only (pas exposes), aucun merge/CRDT (timeline LINEAIRE = pas de saves multiples ni multijoueur). Le scalaire ne sert a rien tant que ces socles manquent.

ORDRE D'IMPLEMENTATION (chaque vague debloque la suivante):
VAGUE 0 — SOCLE STRING+COLLECTION (debloque UI/chat/commandes/terminal/DB): les 10 string-bions purs (split/trim/join/case/pad/replace/find/slice/lines/count) + sort_by_key/binary_search/lower_bound/group_by_key. C'est le verrou n.1 du catalogue (l.39-40): tant qu'il manque, rien d'interactif ne se construit.
VAGUE 1 — HASH+SERIAL (debloque integrite/store/16Go): EXPOSER blake3+merkle_root (deja dans le moteur), AJOUTER merkle_proof/verify + varint_enc/dec + zigzag + lz_enc/dec. Transforme le store addr64 (FNV faible) en base verifiable et sync-able, et ouvre la voie au bion 16Go.
VAGUE 2 — VEC3+MATH3D+RNG (debloque le monde 3D/procgen): vec3/mat4/aabb/raycast + sin/cos/atan2/exp/ln + pcg/rng_f64 (DERIVE de lcg, pas un nouveau RNG) + value_noise/fbm. Debloque voxel-mesh-ploxion (piece manquante #1 du catalogue: donner une APPARENCE aux blocs MC).
VAGUE 3 — MERGE+QUERY (debloque multijoueur/sync): merge3 + trio CRDT (lww/gcounter/orset) + filter_idx/range_scan/top_k. Les seuls merge qui CONVERGENT sans coordination = la math du multijoueur sans serveur (le chainon Nexus).
VAGUE 4 — CRYPTO+ENCODE complets (ed25519/chacha20poly1305/ct_eq/base64url) + ploxions organes (hash/sign/vault/scheduler/clock-adapter).

LIEN P0 CATALOGUE: mes 8 domaines CIBLENT 1-pour-1 les ◻ flagues (merkle l.35, compress l.36, string l.39, collection l.40, noise l.44, base64 l.47, diff l.48, interpolate l.52, vec3/spatial l.54, export l.58, merge/query/wal en P5 l.253) — aucune invention hors-roadmap.

PRINCIPE 'la primitive optimale, pas une de plus' applique: paires symetriques enc/dec bit-exact partout; map/filter/fold ECARTES (Iterator Rust); BLAKE3 sert hash+MAC+KDF (pas 3 primitives); ChaCha20-Poly1305 = seule AEAD; crc32+blake3 couvrent (xxhash/murmur3 ecartes). UNIFICATION cross-domaine obligatoire: vec3/varint/base64/crc32/rle/merkle/str_split/glob/ease/noise/stats sont proposes par plusieurs agents = implementer UNE seule fois.

ALERTE A JOSE (decision): trig/exp f64 (libm) peut diverger bit-a-bit entre archi CPU -> pour le replay/lockstep bit-exact, fournir AUSSI la voie fixed-point Q16.16, sinon les bions trig ne sont rejouables qu'a epsilon pres (contredit bion=deterministe).
