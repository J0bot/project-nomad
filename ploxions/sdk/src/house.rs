//! `house` — le **MAISON-BION** : une maison autonome = 157 features, chaque
//! feature = un **cubion** partagé.
//!
//! José : « la maison autonome, 157 features, chaque feature = un ploxion cubion ».
//! Tout ce qui est commun à *toutes* les features d'une maison (le cycle PLC, le
//! manifeste, le décodage d'événement, la maj du mode, l'application de la règle,
//! la gravure du tsoin, la ré-émission de l'acte) vit **ici, une seule fois**. Ce
//! qui RESTE par feature — le domaine, le nom, ce qu'elle SENT (`sense`), ce sur
//! quoi elle AGIT (`act`), sa RÈGLE de décision, son caractère vital — est le
//! **résidu** : une seule [`HouseDef`] const. La grammaire `bion -> ploxion`
//! appliquée à la maison : `house-brain-core`/`house-mode-manager` ne sont que ce
//! résidu + un appel de macro. C'est l'exact pendant du [`crate::block`]-bion, mais
//! pour la maison plutôt que pour Minecraft.
//!
//! ## Le DÉCIDE déclaratif partagé ([`Rule`])
//! Chaque feature DÉCIDE de la même façon — appliquer une règle à une valeur
//! sentie selon le [`Mode`] courant. La règle est donc DÉCLARATIVE (une donnée,
//! pas du code) : `Hysteresis`, `Threshold`, `Palier`, `Greedy`, `Fsm`,
//! `Aggregate`, `Passthru`. Le moteur qui APPLIQUE la règle ([`Rule::decide`])
//! est le bion partagé ; chaque feature ne choisit QUE quelle variante elle est.
//!
//! ## Les 4 modes de la maison ([`Mode`])
//! `Absent`, `Present`, `Urgence`, `Nuit`. Le mode est broadcasté sur
//! `house.mode.state` par la feature `house-mode-manager` et CHAQUE feature
//! l'écoute (il est ajouté d'office aux `requires` par la macro). Le mode module
//! la décision : en `Urgence` la sécurité prime, en `Nuit`/`Absent` on économise.
//!
//! ## Le cycle généré par [`house_ploxion!`]
//! Depuis une `HouseDef` const, la macro génère **exactement** le jeu d'exports
//! PLC v1 (`plc_manifest`/`plc_init`/`plc_health`/`plc_goodbye`/`plc_on_event`) :
//!   - **manifest** : `id="house-<domaine>-<feature>"`, provides
//!     `["house.<d>.<f>.state","house.<d>.<f>.act","tsoin.record"]`, requires =
//!     les topics de `sense` **plus** `"house.mode.state"` (toute feature suit le
//!     mode). `tsoin.record` est déclaré car toute feature grave un tsoin de
//!     décision (même convention que `block`/`mc-adapter`).
//!   - **init** : log + `emit "house.<d>.<f>.state" {state:"registered",…}`. Le
//!     topic est **namespacé par domaine+feature** : il matche le provide
//!     `house.<d>.<f>.state` (provide == emit) et distingue chaque feature côté
//!     graphe map/ecosystem.
//!   - **on_event** (multi-topic, via [`crate::bions::decode_event`]) :
//!     - sur `house.mode.state` `{mode}` -> met à jour le [`Mode`] thread_local
//!       (jamais d'acte émis : on absorbe juste le mode courant). **Sauf** si la
//!       feature est elle-même le **producteur du mode** (`act[0] ==
//!       "house.mode.state"`, le mode-manager) : elle n'absorbe PAS son propre
//!       broadcast (anti-self-loop) — sinon son émission se ré-injecterait dans son
//!       on_event et la verrouillerait sur son mode courant.
//!     - sur un topic **de `sense`** -> lit la valeur. Soit le payload porte une
//!       clé `"v"` numérique (capteur), soit une clé `"mode"` string (consigne
//!       `house.mode.set`, présence pré-nommée…) que [`Rule::label_index`] traduit
//!       en l'index FSM/Greedy attendu (une consigne `"urgence"` ne tombe donc plus
//!       à tort sur le premier état). Applique [`Rule::decide`] selon le mode -> émet
//!       l'acte ET grave un tsoin `house:<d>:<f>:<seq>` (résidu = le payload brut).
//!     - **producteur de mode** (`act[0] == "house.mode.state"`) : l'acte émis est
//!       le **broadcast** `{mode:<label>,…}` — le nouveau mode DÉCIDÉ (le label
//!       FSM), pas le mode absorbé. Toute autre feature lit `mode` et le suit. Pour
//!       une feature ordinaire, `mode` reste le mode courant absorbé (contexte).
//!
//! Aucun nouveau bion bas-niveau : 100% [`crate::bions`] existants
//! (`json_str`/`json_num`/`decode_event`/`tsoin_record`/`json_esc`). Aucune
//! dépendance hôte : tout passe par [`crate::emit`] / [`crate::log`].
//! Déterministe.

/// Les 4 **modes** d'une maison autonome. Broadcastés sur `house.mode.state` par
/// `house-mode-manager` ; chaque feature les absorbe et module sa décision dessus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Personne : on économise au maximum, sécurité en veille active.
    Absent,
    /// Présence détectée : confort normal.
    Present,
    /// Alerte (intrusion / incident) : la sécurité prime sur tout.
    Urgence,
    /// Nuit : confort réduit, on économise, surveillance douce.
    Nuit,
}

impl Mode {
    /// Décode un mode depuis sa string canonique (`"absent"`/`"present"`/
    /// `"urgence"`/`"nuit"`). `None` si inconnue — l'appelant garde alors le mode
    /// courant (on ne casse pas l'état sur un payload douteux).
    pub fn from_str(s: &str) -> Option<Mode> {
        match s {
            "absent" => Some(Mode::Absent),
            "present" => Some(Mode::Present),
            "urgence" => Some(Mode::Urgence),
            "nuit" => Some(Mode::Nuit),
            _ => None,
        }
    }

    /// La string canonique du mode (l'inverse de [`Mode::from_str`]).
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Absent => "absent",
            Mode::Present => "present",
            Mode::Urgence => "urgence",
            Mode::Nuit => "nuit",
        }
    }
}

/// La **RÈGLE de décision** déclarative, partagée par toutes les features.
///
/// Une règle est une DONNÉE (pas du code) : la feature choisit sa variante, le
/// moteur [`Rule::decide`] l'applique uniformément. C'est le DÉCIDE partagé du
/// MAISON-BION — l'analogue du FLOW-BION de [`crate::block`], mais côté décision.
#[derive(Clone, Copy)]
pub enum Rule {
    /// Hystérésis : sortie binaire à mémoire de seuils (`lo`/`hi`). Sous `lo` ->
    /// `0`, au-dessus de `hi` -> `1000` (milli), entre les deux -> état neutre
    /// `500` (zone morte, anti-pompage). Pour un thermostat / une régul.
    Hysteresis { lo: f64, hi: f64 },
    /// Seuil simple : `0` sous `trip`, `1000` au-dessus ou égal. Pour un capteur
    /// tout-ou-rien (présence, fumée…).
    Threshold { trip: f64 },
    /// Paliers croissants : renvoie l'index (×1000) du dernier seuil franchi.
    /// Pour une consigne à plusieurs niveaux (vitesse de VMC, intensité lumière).
    Palier(&'static [f64]),
    /// Greedy : choisit l'acte d'index = `floor(v) % len` parmi une liste
    /// d'options nommées. Le `decide` renvoie cet index (×1000) ; le label sort via
    /// [`Rule::greedy_pick`]. Pour un arbitrage glouton (quelle source d'énergie).
    Greedy(&'static [&'static str]),
    /// Machine à états : la valeur sélectionne un état nommé d'index
    /// `floor(v) % len`. Pour un séquenceur (mode-manager). Renvoie l'index ×1000.
    Fsm(&'static [&'static str]),
    /// Agrégateur : combine plusieurs `sense` en une directive — ici le `decide`
    /// renvoie la valeur telle quelle (l'arbitrage réel est porté par le mode +
    /// la priorité sécurité>confort>éco câblée dans la feature `brain-core`).
    Aggregate,
    /// Passe-plat : renvoie la valeur sentie inchangée (×1, en milli). Pour une
    /// feature qui relaie/normalise sans décider.
    Passthru,
}

impl Rule {
    /// **LE DÉCIDE** : applique la règle à la valeur sentie `v` sous le `mode`
    /// courant, et renvoie une **décision en milli** (`i64`, signée pour laisser
    /// passer des index/valeurs négatives éventuelles). Déterministe.
    ///
    /// Le `mode` module l'agrégat : en [`Mode::Urgence`] un [`Rule::Aggregate`]
    /// SATURE à `1000` (sécurité = priorité absolue) ; en [`Mode::Absent`] /
    /// [`Mode::Nuit`] il est atténué de moitié (économie). Les autres règles sont
    /// indépendantes du mode (un seuil de fumée reste un seuil de fumée la nuit).
    pub fn decide(&self, v: f64, mode: Mode) -> i64 {
        match self {
            Rule::Hysteresis { lo, hi } => {
                if v < *lo {
                    0
                } else if v >= *hi {
                    1000
                } else {
                    500
                }
            }
            Rule::Threshold { trip } => {
                if v >= *trip {
                    1000
                } else {
                    0
                }
            }
            Rule::Palier(seuils) => {
                let mut idx = 0i64;
                for (i, s) in seuils.iter().enumerate() {
                    if v >= *s {
                        idx = (i as i64) + 1;
                    }
                }
                idx * 1000
            }
            Rule::Greedy(opts) | Rule::Fsm(opts) => {
                if opts.is_empty() {
                    0
                } else {
                    let i = (v.max(0.0) as i64) % (opts.len() as i64);
                    i * 1000
                }
            }
            Rule::Aggregate => {
                let base = (v * 1000.0) as i64;
                match mode {
                    Mode::Urgence => 1000,
                    Mode::Absent | Mode::Nuit => base / 2,
                    Mode::Present => base,
                }
            }
            Rule::Passthru => (v * 1000.0) as i64,
        }
    }

    /// Pour [`Rule::Greedy`]/[`Rule::Fsm`] : le **label** choisi pour la valeur `v`
    /// (l'option d'index `floor(v) % len`). Renvoie `""` si la règle n'est pas à
    /// options ou si la liste est vide. Le label est le résidu humain de la
    /// décision (l'index ×1000 reste la décision machine de [`Rule::decide`]).
    pub fn greedy_pick(&self, v: f64) -> &'static str {
        match self {
            Rule::Greedy(opts) | Rule::Fsm(opts) => {
                if opts.is_empty() {
                    ""
                } else {
                    opts[(v.max(0.0) as usize) % opts.len()]
                }
            }
            _ => "",
        }
    }

    /// Pour [`Rule::Fsm`]/[`Rule::Greedy`] : l'**index** (`f64`) de l'option dont le
    /// label vaut `name`, ou `None` si absente / règle sans options. C'est l'inverse
    /// de [`Rule::greedy_pick`] : il traduit une consigne **string** (ex
    /// `house.mode.set {"mode":"urgence"}`) en la valeur numérique que [`decide`]
    /// attend, de sorte qu'une FSM puisse être pilotée par un nom d'état autant que
    /// par un `v` numérique pré-encodé. Corrige le cas où une consigne d'urgence
    /// portée par une clé `"mode"` string était sinon lue `v=0.0` → premier état.
    pub fn label_index(&self, name: &str) -> Option<f64> {
        match self {
            Rule::Greedy(opts) | Rule::Fsm(opts) => {
                opts.iter().position(|o| *o == name).map(|i| i as f64)
            }
            _ => None,
        }
    }
}

/// Le **résidu** d'une feature de maison : tous les champs const-compatibles
/// (`&'static`/copies). Deux features ne diffèrent QUE par cette structure ; tout
/// le reste est le bion. Pendant exact de [`crate::block::BlockDef`].
#[derive(Clone, Copy)]
pub struct HouseDef {
    /// domaine de la feature, ex `"brain"`, `"energy"`, `"sec"` (sert au manifest
    /// `house-<domaine>-<feature>` et au namespace des topics).
    pub domaine: &'static str,
    /// nom court de la feature, ex `"core"`, `"mode"`, `"thermostat"`.
    pub feature: &'static str,
    /// les topics que la feature **SENT** (ses entrées). Sur l'un d'eux, elle
    /// applique sa règle et agit. (`house.mode.state` est ajouté d'office aux
    /// `requires` par la macro — ne le mettez PAS ici.)
    pub sense: &'static [&'static str],
    /// les topics sur lesquels la feature **AGIT** (ses sorties). Le premier sert
    /// de cible canonique de l'acte ; les autres sont déclarés mais à la charge de
    /// la feature si elle veut multi-émettre (le cycle générique émet le premier).
    pub act: &'static [&'static str],
    /// la **RÈGLE de décision** déclarative (le DÉCIDE partagé).
    pub rule: Rule,
    /// `true` si la feature est **vitale** (sécurité/structure) : journalisée comme
    /// telle ; un superviseur peut la prioriser au redémarrage. N'altère pas le
    /// cycle, c'est une étiquette de criticité dans le manifest/log.
    pub vital: bool,
}

/// Génère le ploxion **complet** d'une feature de maison depuis une [`HouseDef`]
/// const.
///
/// Appel (depuis le crate de la feature, la macro est exportée à la **racine** du
/// SDK) :
/// ```ignore
/// use ploxion_sdk::house::{HouseDef, Rule};
/// const DEF: HouseDef = HouseDef { domaine: "brain", feature: "core", /* … */ };
/// ploxion_sdk::house_ploxion!(DEF);
/// ```
///
/// Produit EXACTEMENT les 5 exports PLC `#[no_mangle]`
/// (`plc_manifest`/`plc_init`/`plc_health`/`plc_goodbye`/`plc_on_event`). `alloc`
/// vient du SDK (jamais redéfini ici). Tous les chemins sont `$crate::` =>
/// hygiénique, appelable depuis n'importe quel crate qui dépend du SDK.
#[macro_export]
macro_rules! house_ploxion {
    ($def:expr) => {
        // Le résidu, figé pour ce module.
        const __HOUSE_DEF: $crate::house::HouseDef = $def;

        // État par module : compteur de tsoins + mode courant de la maison.
        thread_local! {
            static __HOUSE_SEQ: ::std::cell::RefCell<u64> = const { ::std::cell::RefCell::new(0) };
            static __HOUSE_MODE: ::std::cell::RefCell<$crate::house::Mode> =
                const { ::std::cell::RefCell::new($crate::house::Mode::Present) };
        }

        // --- manifest : requires = sense ++ ["house.mode.state"] ----------------
        #[no_mangle]
        pub extern "C" fn plc_manifest() -> i64 {
            // L'id/les topics viennent de la HouseDef (pas des littéraux) -> on
            // bâtit le JSON une seule fois et on le cache dans un OnceCell
            // thread_local : appels répétés -> MÊME &'static (pas de fuite par
            // appel). Droppé quand l'hôte drop le Store au goodbye.
            thread_local! {
                static __HOUSE_MANIFEST: ::std::cell::OnceCell<&'static str> =
                    const { ::std::cell::OnceCell::new() };
            }
            let leaked: &'static str = __HOUSE_MANIFEST.with(|c| {
                *c.get_or_init(|| {
                    let d = __HOUSE_DEF;
                    let dom = $crate::bions::json_esc(d.domaine);
                    let feat = $crate::bions::json_esc(d.feature);
                    // provides : state + act + tsoin.record (provide == emit).
                    let provides = ::std::format!(
                        "\"house.{dom}.{feat}.state\",\"house.{dom}.{feat}.act\",\"tsoin.record\""
                    );
                    // requires : chaque topic de sense (échappé). On ajoute
                    // `house.mode.state` SAUF si la feature le PRODUIT (act[0]) :
                    // un producteur ne se require pas lui-même (anti-self-loop côté
                    // graphe, pendant du garde dans on_event).
                    let produces_mode =
                        d.act.first().map_or(false, |a| *a == "house.mode.state");
                    let mut requires = ::std::string::String::new();
                    for s in d.sense {
                        requires.push('"');
                        requires.push_str(&$crate::bions::json_esc(s));
                        requires.push_str("\",");
                    }
                    if produces_mode {
                        // Retire la virgule de queue éventuelle (sense non vide).
                        if requires.ends_with(',') {
                            requires.pop();
                        }
                    } else {
                        requires.push_str("\"house.mode.state\"");
                    }
                    let json = ::std::format!(
                        "{{\"id\":\"house-{dom}-{feat}\",\"version\":\"1.0.0\",\"capabilities\":[],\"provides\":[{provides}],\"requires\":[{requires}],\"children_types\":[],\"parent_types\":[]}}"
                    );
                    &*::std::boxed::Box::leak(json.into_boxed_str())
                })
            });
            $crate::pack_ptr_len(leaked.as_ptr() as u32, leaked.len() as u32)
        }

        // --- init : log + annonce la feature enregistrée (topic namespacé) ------
        #[no_mangle]
        pub extern "C" fn plc_init() {
            let d = __HOUSE_DEF;
            $crate::log(&::std::format!(
                "house-{}-{}: init (vital={} ; sense={} act={})",
                d.domaine, d.feature, d.vital, d.sense.len(), d.act.len()
            ));
            // Topic per-(domaine,feature) == provide `house.<d>.<f>.state`.
            $crate::emit(
                &::std::format!("house.{}.{}.state", d.domaine, d.feature),
                ::std::format!(
                    "{{\"state\":\"registered\",\"domaine\":\"{}\",\"feature\":\"{}\",\"vital\":{}}}",
                    $crate::bions::json_esc(d.domaine),
                    $crate::bions::json_esc(d.feature),
                    d.vital
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
            $crate::log(&::std::format!(
                "house-{}-{}: goodbye",
                __HOUSE_DEF.domaine, __HOUSE_DEF.feature
            ));
        }

        // --- on_event : mode.state -> maj Mode ; sense -> applique Rule ---------
        #[no_mangle]
        pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
            let (topic, payload) = $crate::bions::decode_event(tp, tl, pp, pl);
            let d = __HOUSE_DEF;

            // Cette feature PRODUIT-elle le mode ? (act[0] cible house.mode.state.)
            // Le mode-manager est ce producteur : il BROADCAST le mode, il ne le
            // suit pas. Distinction figée une fois ici.
            let produces_mode = d.act.first().map_or(false, |a| *a == "house.mode.state");

            // 1) Le mode broadcasté : on l'absorbe, sans agir. ANTI-SELF-LOOP : le
            //    producteur du mode n'absorbe PAS son propre broadcast (sinon son
            //    émission se ré-injecte et le verrouille sur son mode courant).
            if topic == "house.mode.state" {
                if produces_mode {
                    return;
                }
                if let Some(m) = $crate::bions::json_str(&payload, "mode")
                    .and_then(|s| $crate::house::Mode::from_str(&s))
                {
                    __HOUSE_MODE.with(|c| *c.borrow_mut() = m);
                }
                return;
            }

            // 2) Un topic SENTI ? (sinon on ignore — feature non concernée.)
            if !d.sense.iter().any(|s| *s == topic) {
                return;
            }

            // Valeur sentie : d'abord une clé "mode" string (consigne nommée, ex
            // house.mode.set {"mode":"urgence"}) traduite en index FSM/Greedy ;
            // sinon la clé "v" numérique (capteur). Absente -> 0.0 (entrée neutre).
            // Garde-finie : un payload non-fini (inf/NaN) retombe à 0.0 pour ne pas
            // émettre un token JSON invalide en aval.
            let v = $crate::bions::json_str(&payload, "mode")
                .and_then(|s| d.rule.label_index(&s))
                .or_else(|| $crate::bions::json_num(&payload, "v"))
                .unwrap_or(0.0);
            let v = if v.is_finite() { v } else { 0.0 };
            let mode = __HOUSE_MODE.with(|c| *c.borrow());

            // LE DÉCIDE partagé.
            let decision = d.rule.decide(v, mode);
            let label = d.rule.greedy_pick(v);

            // seq local -> adresse unique du tsoin de décision.
            let seq = __HOUSE_SEQ.with(|s| {
                let mut s = s.borrow_mut();
                *s += 1;
                *s
            });

            // 3) grave le tsoin : générateur = nom adressé, résidu = le payload
            //    senti brut (hex via tsoin_record). Le nom est échappé JSON.
            let name = ::std::format!(
                "house:{}:{}:{}",
                $crate::bions::json_esc(d.domaine),
                $crate::bions::json_esc(d.feature),
                seq
            );
            $crate::bions::tsoin_record(&name, payload.as_bytes());

            // 4) émet l'ACTE sur la cible canonique (premier `act`, ou le state si
            //    aucun act déclaré). decision = la sortie machine ; label = le résidu
            //    humain (vide hors Greedy/Fsm).
            let act_topic = if d.act.is_empty() {
                ::std::format!("house.{}.{}.act", d.domaine, d.feature)
            } else {
                d.act[0].to_string()
            };

            // Le champ `mode` du payload : pour le PRODUCTEUR de mode, c'est le
            // nouveau mode DÉCIDÉ (le label FSM = l'état choisi) — c'est CE broadcast
            // que toute autre feature lit et suit. Pour une feature ordinaire, c'est
            // le mode courant absorbé (contexte de sa décision). Fallback : si le
            // producteur n'a pas de label exploitable (règle non-FSM/Greedy), on
            // retombe sur le mode courant pour ne jamais émettre un mode vide.
            let mode_field: &str = if produces_mode && !label.is_empty() {
                label
            } else {
                mode.as_str()
            };
            $crate::emit(
                &act_topic,
                ::std::format!(
                    "{{\"domaine\":\"{}\",\"feature\":\"{}\",\"v\":{},\"decision\":{},\"label\":\"{}\",\"mode\":\"{}\",\"seq\":{}}}",
                    $crate::bions::json_esc(d.domaine),
                    $crate::bions::json_esc(d.feature),
                    v,
                    decision,
                    $crate::bions::json_esc(label),
                    $crate::bions::json_esc(mode_field),
                    seq
                )
                .as_bytes(),
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{Mode, Rule};

    /// `label_index` est l'inverse de `greedy_pick` : une consigne string nommée
    /// (`"urgence"`) retombe sur le BON index FSM, pas sur 0. C'est le cœur du fix
    /// du broadcast mode (mode.set {"mode":"urgence"} ne doit pas être lu "absent").
    #[test]
    fn label_index_round_trips_fsm() {
        let fsm = Rule::Fsm(&["absent", "present", "urgence", "nuit"]);
        assert_eq!(fsm.label_index("absent"), Some(0.0));
        assert_eq!(fsm.label_index("urgence"), Some(2.0));
        assert_eq!(fsm.label_index("nuit"), Some(3.0));
        assert_eq!(fsm.label_index("inconnu"), None);
        // L'index sélectionné redonne bien le label via greedy_pick.
        for name in ["absent", "present", "urgence", "nuit"] {
            let v = fsm.label_index(name).unwrap();
            assert_eq!(fsm.greedy_pick(v), name, "round-trip {name}");
        }
        // Règle sans options -> pas d'index.
        assert_eq!(Rule::Passthru.label_index("x"), None);
    }

    /// Aggregate module bien par le mode (sécurité>confort>éco) — l'arbitrage de
    /// brain-core repose sur ce couple (mode courant × Aggregate).
    #[test]
    fn aggregate_modulated_by_mode() {
        let r = Rule::Aggregate;
        assert_eq!(r.decide(0.8, Mode::Urgence), 1000); // sécurité sature.
        assert_eq!(r.decide(0.8, Mode::Present), 800); // confort = passe.
        assert_eq!(r.decide(0.8, Mode::Nuit), 400); // éco = /2.
        assert_eq!(r.decide(0.8, Mode::Absent), 400);
    }

    /// Mode round-trip string canonique (le broadcast voyage en string).
    #[test]
    fn mode_string_round_trip() {
        for m in [Mode::Absent, Mode::Present, Mode::Urgence, Mode::Nuit] {
            assert_eq!(Mode::from_str(m.as_str()), Some(m));
        }
        assert_eq!(Mode::from_str("bogus"), None);
    }
}