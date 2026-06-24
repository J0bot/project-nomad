//! `block` — le **BLOCK-BION** : un bloc de Minecraft = un cubion partagé.
//!
//! José : « recoder TOUS les blocs du jeu comme ploxions, bâtis depuis des bions
//! PARTAGÉS ». Un bloc = un **cubion** = cube + id + state. Tout ce qui est commun
//! à *tous* les blocs (cycle PLC, manifeste, décodage d'événement, gravure du
//! tsoin de pose, ré-émission) vit **ici, une seule fois**. Ce qui RESTE par bloc
//! — l'id, le nom, solide/fluide, dureté, lumière, drop, niveau max — est le
//! **résidu** : une seule [`BlockDef`] const. La grammaire `bion -> ploxion`
//! appliquée aux blocs : `block-stone`/`block-water` ne sont que ce résidu + un
//! appel de macro.
//!
//! ## Le cycle généré par [`block_ploxion!`]
//! Depuis une `BlockDef` const, la macro génère **exactement** le jeu d'exports
//! PLC v1 (`plc_manifest`/`plc_init`/`plc_health`/`plc_goodbye`/`plc_on_event`) :
//!   - **manifest** : `id="block-<id>"`, provides `["block.<id>.registered",
//!     "block.placed","block.broken","tsoin.record"]` (+ `"block.flow"` si
//!     fluide), requires `["block.place","block.break"]`. `tsoin.record` est
//!     déclaré car tout bloc grave un tsoin de pose (même convention que
//!     `mc-adapter`).
//!   - **init** : log + `emit "block.<id>.registered"
//!     {id,name,solid,fluid,hardness_milli,light}`. Le topic est **namespacé par
//!     id** : il matche le provide `block.<id>.registered` du manifest (provide ==
//!     emit) et distingue stone de water côté graphe map/ecosystem.
//!   - **on_event** (multi-topic, via [`crate::bions::decode_event`]) :
//!     - `block.place` `{block,dim,x,y,z}` **filtré sur `block==id`** -> grave un
//!       tsoin `block:<id>:place:<seq>` (résidu = le payload de pose, hex) + émet
//!       `block.placed`. Le tsoin grave le payload **brut** reçu : c'est l'event
//!       `block.place` d'origine qui, rejoué par le moteur de tsoins, re-déclenche
//!       la pose (le cycle replay -> re-pose n'est pas démontré par un test ici,
//!       seulement la gravure de la frame canonique `tsoin.record`).
//!     - si le bloc **coule** (`fluid`), émet en plus le **FLOW-BION**
//!       `block.flow {block,dim,x,y,z,level}` : la case **source** s'auto-annonce
//!       comme fluide à un niveau décrémenté (`flow_level(max,max) = max-1`). Le
//!       calcul des cases voisines réelles (x±1, z±1, y-1) est laissé au
//!       consommateur (carte/Nexus) ; ce payload décrit la source, pas un voisin.
//!     - `block.break` -> émet `block.broken {block,drop}`. `drop==""` signifie
//!       **aucun drop** (eau) : le consommateur ne doit pas créer d'entité item.
//!
//! Le FLOW-BION ([`flow_level`] + l'émission `block.flow`) est lui-même un bion
//! partagé : tout futur bloc fluide (lava, …) le réutilise. C'est « faire plus de
//! bions » — l'eau, en coulant, révèle un bloc de code réutilisable.
//!
//! ## Contrat de payload (figé)
//! Le payload `block.place`/`block.break` est `{block,dim,x,y,z}` (clés contrôlées,
//! pas de clé substring piège pour les bions `json_*`). `BlockDef.id` DOIT être un
//! token `[a-z0-9_-]` ; la macro échappe néanmoins `id`/`name`/`drop` en JSON pour
//! rester robuste si un id futur contenait un caractère spécial.
//!
//! ## Producteurs amont (état du xion)
//! Les blocs `require` `["block.place","block.break"]` mais AUCUN ploxion ne les
//! émet encore (le pont Minecraft `mc-adapter` n'émet que `mc.event`, pas le
//! payload structuré `{block,dim,x,y,z}`). Tant que ce chaînon manque, stone/water
//! sont des **stubs de grammaire** corrects mais non déclenchés en live : ce sont
//! des consommateurs pendants. Le bridge `mc.event -> block.place/block.break`
//! (parsing `setblock`/`fill`/destroy) est un livrable du prochain batch
//! Minecraft-Nexus.
//!
//! Aucune dépendance hôte : tout passe par [`crate::bions`] + [`crate::emit`] /
//! [`crate::log`]. Déterministe.

/// Le **résidu** d'un bloc : tous les champs const-compatibles (`&'static`/copies).
/// Deux blocs ne diffèrent QUE par cette structure ; tout le reste est le bion.
#[derive(Clone, Copy)]
pub struct BlockDef {
    /// id court, ex `"stone"` (sert au manifest `block-<id>` et au filtre d'event).
    /// Convention : token `[a-z0-9_-]` (la macro l'échappe quand même en JSON).
    pub id: &'static str,
    /// nom lisible, ex `"Stone"`.
    pub name: &'static str,
    /// solide (occupe la case, collision).
    pub solid: bool,
    /// fluide (coule -> révèle le FLOW-BION).
    pub fluid: bool,
    /// dureté en milli (Minecraft : stone = 1.5 -> `1500`).
    pub hardness_milli: u32,
    /// niveau de lumière émis 0..=15.
    pub light: u8,
    /// id du bloc lâché au cassage (stone -> `"cobblestone"` ; vide = rien).
    pub drop: &'static str,
    /// niveau max pour un fluide (eau : 0..=7 -> `7`) ; `0` pour un solide.
    pub max_level: u8,
}

/// **FLOW-BION** : niveau de fluide d'une case voisine quand le fluide coule
/// depuis une source de niveau `level` (max `max`). Un fluide décroît de 1 par
/// case ; arrivé à 0 il ne coule plus (renvoie 0). Partagé par tous les blocs
/// fluides (eau, lave, …) — c'est le bloc de code que l'eau « fait » en coulant.
///
/// Convention Minecraft inversée pour rester lisible ici : `level` = quantité
/// restante (source = `max`), donc le voisin reçoit `level.saturating_sub(1)`.
///
/// Le `.min(max)` est **défensif** : dans le cycle généré on appelle toujours
/// `flow_level(max, max)` (donc `<= max-1`, le clamp est no-op). Il ne s'active
/// que si un futur appelant passe une source `> max` (cf. test `flow_level(8,7)`).
#[inline]
pub fn flow_level(level: u8, max: u8) -> u8 {
    level.saturating_sub(1).min(max)
}

/// Génère le ploxion **complet** d'un bloc depuis une [`BlockDef`] const.
///
/// Appel (depuis le crate du bloc, la macro est exportée à la **racine** du SDK) :
/// ```ignore
/// use ploxion_sdk::block::BlockDef;
/// const DEF: BlockDef = BlockDef { id: "stone", /* … */ };
/// ploxion_sdk::block_ploxion!(DEF);
/// ```
///
/// Produit EXACTEMENT les 5 exports PLC `#[no_mangle]`
/// (`plc_manifest`/`plc_init`/`plc_health`/`plc_goodbye`/`plc_on_event`). `alloc`
/// vient du SDK (jamais redéfini ici). Tous les chemins sont `$crate::` =>
/// hygiénique, appelable depuis n'importe quel crate qui dépend du SDK.
#[macro_export]
macro_rules! block_ploxion {
    ($def:expr) => {
        // Le résidu, figé pour ce module.
        const __BLOCK_DEF: $crate::block::BlockDef = $def;

        // Compteur de pose, par module (adresse chaque tsoin de façon unique).
        thread_local! {
            static __BLOCK_SEQ: ::std::cell::RefCell<u64> = const { ::std::cell::RefCell::new(0) };
        }

        // --- manifest : provides dépend de fluid (block.flow si ça coule) -------
        #[no_mangle]
        pub extern "C" fn plc_manifest() -> i64 {
            // L'id vient de la BlockDef (pas un littéral) -> on bâtit le JSON une
            // seule fois et on le cache dans un OnceCell thread_local : les appels
            // répétés renvoient le MÊME &'static (pas de fuite par appel). Droppé en
            // bloc quand l'hôte drop le Store au goodbye.
            thread_local! {
                static __BLOCK_MANIFEST: ::std::cell::OnceCell<&'static str> =
                    const { ::std::cell::OnceCell::new() };
            }
            let leaked: &'static str = __BLOCK_MANIFEST.with(|c| {
                *c.get_or_init(|| {
                    let id = __BLOCK_DEF.id;
                    // Un fluide annonce en plus le FLOW-BION `block.flow` qu'il émet.
                    let flow = if __BLOCK_DEF.fluid { ",\"block.flow\"" } else { "" };
                    let json = ::std::format!(
                        "{{\"id\":\"block-{id}\",\"version\":\"1.0.0\",\"capabilities\":[],\"provides\":[\"block.{id}.registered\",\"block.placed\",\"block.broken\",\"tsoin.record\"{flow}],\"requires\":[\"block.place\",\"block.break\"],\"children_types\":[],\"parent_types\":[]}}"
                    );
                    &*::std::boxed::Box::leak(json.into_boxed_str())
                })
            });
            $crate::pack_ptr_len(leaked.as_ptr() as u32, leaked.len() as u32)
        }

        // --- init : log + annonce le bloc enregistré (topic namespacé par id) ---
        #[no_mangle]
        pub extern "C" fn plc_init() {
            let d = __BLOCK_DEF;
            $crate::log(&::std::format!(
                "block-{}: init ({} ; solid={} fluid={} hardness={}/1000 light={} drop={} max_level={})",
                d.id, d.name, d.solid, d.fluid, d.hardness_milli, d.light,
                if d.drop.is_empty() { "-" } else { d.drop }, d.max_level
            ));
            // Topic per-id == provide `block.<id>.registered` (provide == emit).
            $crate::emit(
                &::std::format!("block.{}.registered", d.id),
                ::std::format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"solid\":{},\"fluid\":{},\"hardness_milli\":{},\"light\":{}}}",
                    $crate::bions::json_esc(d.id),
                    $crate::bions::json_esc(d.name),
                    d.solid,
                    d.fluid,
                    d.hardness_milli,
                    d.light
                )
                .as_bytes(),
            );
        }

        #[no_mangle]
        pub extern "C" fn plc_health() -> i32 {
            0
        }

        #[no_mangle]
        pub extern "C" fn plc_goodbye() {
            $crate::log(&::std::format!("block-{}: goodbye", __BLOCK_DEF.id));
        }

        // --- on_event : multi-topic via decode_event ---------------------------
        #[no_mangle]
        pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
            let (topic, payload) = $crate::bions::decode_event(tp, tl, pp, pl);
            let d = __BLOCK_DEF;

            if topic == "block.place" {
                // FILTRE : seul l'event qui concerne CE bloc nous regarde.
                let which = $crate::bions::json_str(&payload, "block").unwrap_or_default();
                if which != d.id {
                    return;
                }
                let dim = $crate::bions::json_str(&payload, "dim")
                    .unwrap_or_else(|| "overworld".to_string());
                let x = $crate::bions::json_int(&payload, "x", 0);
                let y = $crate::bions::json_int(&payload, "y", 0);
                let z = $crate::bions::json_int(&payload, "z", 0);

                // seq local -> adresse unique du tsoin de pose.
                let seq = __BLOCK_SEQ.with(|s| {
                    let mut s = s.borrow_mut();
                    *s += 1;
                    *s
                });

                // 1) grave le tsoin : générateur = nom adressé, résidu = le payload
                //    de pose complet (hex). Le nom est échappé JSON (id générique).
                let name = ::std::format!(
                    "block:{}:place:{}",
                    $crate::bions::json_esc(d.id),
                    seq
                );
                $crate::bions::tsoin_record(&name, payload.as_bytes());

                // 2) ré-émet normalisé pour carte/cosmos (le monde 3D / le Nexus).
                $crate::emit(
                    "block.placed",
                    ::std::format!(
                        "{{\"block\":\"{}\",\"dim\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"seq\":{}}}",
                        $crate::bions::json_esc(d.id),
                        $crate::bions::json_esc(&dim),
                        x, y, z, seq
                    )
                    .as_bytes(),
                );

                // 3) FLOW-BION : un fluide qui se pose COULE. On annonce la SOURCE
                //    fluide à un niveau décrémenté (les voisins réels sont dérivés
                //    par le consommateur depuis x,y,z + level).
                if d.fluid {
                    let level = $crate::block::flow_level(d.max_level, d.max_level);
                    $crate::emit(
                        "block.flow",
                        ::std::format!(
                            "{{\"block\":\"{}\",\"dim\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"level\":{}}}",
                            $crate::bions::json_esc(d.id),
                            $crate::bions::json_esc(&dim),
                            x, y, z, level
                        )
                        .as_bytes(),
                    );
                }
            } else if topic == "block.break" {
                let which = $crate::bions::json_str(&payload, "block").unwrap_or_default();
                if which != d.id {
                    return;
                }
                // `drop==""` => aucun drop (eau) : le consommateur ne crée pas
                // d'entité item vide.
                $crate::emit(
                    "block.broken",
                    ::std::format!(
                        "{{\"block\":\"{}\",\"drop\":\"{}\"}}",
                        $crate::bions::json_esc(d.id),
                        $crate::bions::json_esc(d.drop)
                    )
                    .as_bytes(),
                );
            }
        }
    };
}
