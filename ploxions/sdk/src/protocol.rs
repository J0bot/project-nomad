//! `protocol` — le **PROTOCOL-BION** : un protocole réseau = un ploxion.
//!
//! ⚠️ **SCAFFOLD NON-COMPILÉ, à valider en batch calme.** Ce module est posé pour
//! matcher le style des bions générateurs ([`crate::block`], [`crate::house`])
//! mais n'a PAS été compilé ni testé (RAM serrée : un serveur Minecraft + le xion
//! tournent). Il est honnête sur ce qu'il est : un **modèle déclaratif** d'un
//! protocole sur le bus (parse d'un PDU minimal + émission + gravure de tsoin),
//! PAS une ré-implémentation de la pile TCP/IP du noyau. À recompiler avec le lot
//! réseau (vague 1 = les ~11 protocoles de la table [`PROTOCOLS`]).
//!
//! José : « Tous les paquets, les ports, les protocoles de réseau, fais des
//! ploxions pour chaque truc. **Tout est un ploxion, l'ami.** » C'est la directive
//! [le bion Linux] appliquée à la **pile réseau** (cf. `docs/network-as-ploxions.md`).
//! On ne taille pas la pile à la main : on fait comme le `block-bion` (qui génère
//! tous les blocs Minecraft depuis `BlockDef`) — **un bion partagé + une table
//! compacte (le résidu) qui GÉNÈRE un ploxion par protocole.**
//!
//! Tout ce qui est commun à *tous* les protocoles (le cycle PLC, le manifeste, le
//! décodage d'événement, le parse d'un PDU minimal, la ré-émission, la gravure du
//! tsoin) vit **ici, une seule fois**. Ce qui RESTE par protocole — le nom, le n°
//! IANA, le transport, la couche OSI, le brief — est le **résidu** : une seule
//! [`ProtocolDef`] const. La grammaire `bion -> ploxion` appliquée au réseau :
//! `proto-tcp`/`proto-dns` ne sont que ce résidu + un appel de macro.
//!
//! ## Le contrat de topics (PLC)
//! Un protocole `<name>` est un connecteur sur le bus :
//!   - **provides** `["net.<name>.out", "net.<name>.<name>.registered",
//!     "tsoin.record"]` : `net.<name>.out` = le PDU normalisé qu'il émet vers le
//!     haut (vers son consommateur) ; `tsoin.record` car tout event grave un tsoin
//!     (même convention que `block`/`house`/`mc-adapter`).
//!   - **requires** `["net.<name>.in"]` : le PDU brut qu'il reçoit du bas (du
//!     transport / d'un autre protocole). `net.<name>.in -> parse -> net.<name>.out`.
//!
//! ## Le cycle généré par [`protocol_ploxion!`]
//! Depuis une `ProtocolDef` const, la macro génère **exactement** les 5 exports
//! PLC v1 (`plc_manifest`/`plc_init`/`plc_health`/`plc_goodbye`/`plc_on_event`) :
//!   - **manifest** : `id="proto-<name>"`, provides/requires ci-dessus. Bâti une
//!     fois, caché dans un `OnceCell` thread_local (pas de fuite par appel) — même
//!     convention que `block`/`house`.
//!   - **init** : log (n°/transport/couche) + `emit "net.<name>.<name>.registered"
//!     {name,number,transport,layer}`. Topic namespacé par name (provide == emit).
//!   - **on_event** : sur `net.<name>.in`, [`parse_pdu`] extrait le PDU minimal
//!     `{src,dst,kind,len}` (un *modèle* d'unité de données : on lit ce que le
//!     payload JSON porte, on ne décode pas les octets du wire ici), grave un tsoin
//!     `proto:<name>:<seq>` (résidu = le payload `in` brut, hex) et ré-émet le PDU
//!     normalisé sur `net.<name>.out`. Déterministe -> rejouable -> tsoin.
//!
//! ## Honnête (rappel)
//! - **Posé** : le pattern bion→génération est déjà prouvé (block-bion, house-bion).
//!   Un protocole-comme-ploxion-sur-le-bus est un **modèle** (déclaratif +
//!   parse/encode d'un PDU JSON), utile pour composer/observer la pile, pas un
//!   remplacement de `net/ipv4` du kernel.
//! - **Spéculatif** : « remplacer la pile réseau du noyau par des ploxions » = la
//!   direction (le bion-Linux), pas l'état. Couche par couche.
//!
//! Aucune dépendance hôte : tout passe par [`crate::bions`] + [`crate::emit`] /
//! [`crate::log`]. Déterministe.

/// Le **PDU-BION** minimal : l'unité de données de protocole, modèle partagé.
///
/// Tous les protocoles parlent en termes de `{src, dst, kind, len}` à ce niveau
/// d'abstraction (qui parle, à qui, quel genre, quelle taille). Le décodage réel
/// des octets du wire (en-tête IP, segment TCP, …) est laissé au transport ; ici
/// on lit un PDU déjà porté en JSON sur le bus. C'est le pendant du `FLOW-BION` de
/// [`crate::block`] : un petit bloc de code réutilisable par tous les protocoles.
#[derive(Clone, Default, Debug)]
pub struct Pdu {
    /// source (adresse/port/host selon la couche), `""` si inconnue.
    pub src: String,
    /// destination, `""` si inconnue.
    pub dst: String,
    /// genre de PDU (ex `"echo"`, `"syn"`, `"query"`, `"req"`), `""` si absent.
    pub kind: String,
    /// taille utile en octets (modèle : 0 si non porté par le payload).
    pub len: i64,
}

/// **PDU-BION** : parse un PDU minimal depuis un payload JSON `{src,dst,kind,len}`.
///
/// Lecture défensive (clés absentes -> valeurs par défaut), 100% via les bions
/// `json_str`/`json_int` existants. Ne décode PAS les octets du wire : c'est un
/// modèle d'unité de données sur le bus, pas un décodeur binaire de la pile.
pub fn parse_pdu(payload: &str) -> Pdu {
    Pdu {
        src: crate::bions::json_str(payload, "src").unwrap_or_default(),
        dst: crate::bions::json_str(payload, "dst").unwrap_or_default(),
        kind: crate::bions::json_str(payload, "kind").unwrap_or_default(),
        len: crate::bions::json_int(payload, "len", 0),
    }
}

/// Le **résidu** d'un protocole : tous les champs const-compatibles
/// (`&'static`/copies). Deux protocoles ne diffèrent QUE par cette structure ;
/// tout le reste est le bion. Pendant exact de [`crate::block::BlockDef`].
#[derive(Clone, Copy)]
pub struct ProtocolDef {
    /// nom court / token IANA, ex `"tcp"`, `"dns"` (sert au manifest `proto-<name>`
    /// et au namespace des topics `net.<name>.{in,out}`). Token `[a-z0-9_-]` ; la
    /// macro l'échappe quand même en JSON par robustesse.
    pub name: &'static str,
    /// n° de protocole/port IANA (ex tcp=6, udp=17, http=80, dns=53). `0` si sans
    /// numéro canonique (ws : porté sur tcp, pas de n° propre).
    pub number: u16,
    /// transport sous-jacent, ex `"-"` (le protocole EST le transport, ip/icmp),
    /// `"tcp"`, `"udp"`, `"udp/tcp"`. Texte libre court (modèle, pas une enum
    /// fermée : la pile réelle est plus riche).
    pub transport: &'static str,
    /// couche OSI principale (3=réseau, 4=transport, 6=présentation, 7=appli). Une
    /// approximation : tls chevauche 6/7, quic 4/7 — on garde la dominante.
    pub layer: u8,
    /// brief lisible (rôle), ex `"flux fiable ordonné"`. Le résidu humain.
    pub brief: &'static str,
}

/// Génère le ploxion **complet** d'un protocole depuis une [`ProtocolDef`] const.
///
/// Appel (depuis le crate du protocole, la macro est exportée à la **racine** du
/// SDK) :
/// ```ignore
/// use ploxion_sdk::protocol::ProtocolDef;
/// const DEF: ProtocolDef = ProtocolDef {
///     name: "tcp", number: 6, transport: "-", layer: 4,
///     brief: "flux fiable ordonné",
/// };
/// ploxion_sdk::protocol_ploxion!(DEF);
/// ```
///
/// Produit EXACTEMENT les 5 exports PLC `#[no_mangle]`
/// (`plc_manifest`/`plc_init`/`plc_health`/`plc_goodbye`/`plc_on_event`). `alloc`
/// vient du SDK (jamais redéfini ici). Tous les chemins sont `$crate::` =>
/// hygiénique, appelable depuis n'importe quel crate qui dépend du SDK.
///
/// ⚠️ NON-COMPILÉ (cf. en-tête module) : à valider en batch calme.
#[macro_export]
macro_rules! protocol_ploxion {
    ($def:expr) => {
        // Le résidu, figé pour ce module.
        const __PROTO_DEF: $crate::protocol::ProtocolDef = $def;

        // Compteur de PDU, par module (adresse chaque tsoin de façon unique).
        thread_local! {
            static __PROTO_SEQ: ::std::cell::RefCell<u64> = const { ::std::cell::RefCell::new(0) };
        }

        // --- manifest : provides net.<name>.out ; requires net.<name>.in --------
        #[no_mangle]
        pub extern "C" fn plc_manifest() -> i64 {
            // L'id/les topics viennent de la ProtocolDef (pas des littéraux) -> on
            // bâtit le JSON une seule fois et on le cache dans un OnceCell
            // thread_local : appels répétés -> MÊME &'static (pas de fuite par
            // appel). Droppé quand l'hôte drop le Store au goodbye. Même convention
            // que block/house.
            thread_local! {
                static __PROTO_MANIFEST: ::std::cell::OnceCell<&'static str> =
                    const { ::std::cell::OnceCell::new() };
            }
            let leaked: &'static str = __PROTO_MANIFEST.with(|c| {
                *c.get_or_init(|| {
                    let name = $crate::bions::json_esc(__PROTO_DEF.name);
                    let json = ::std::format!(
                        "{{\"id\":\"proto-{name}\",\"version\":\"1.0.0\",\"capabilities\":[],\"provides\":[\"net.{name}.out\",\"net.{name}.{name}.registered\",\"tsoin.record\"],\"requires\":[\"net.{name}.in\"],\"children_types\":[],\"parent_types\":[]}}"
                    );
                    &*::std::boxed::Box::leak(json.into_boxed_str())
                })
            });
            $crate::pack_ptr_len(leaked.as_ptr() as u32, leaked.len() as u32)
        }

        // --- init : log + annonce le protocole enregistré (topic namespacé) -----
        #[no_mangle]
        pub extern "C" fn plc_init() {
            let d = __PROTO_DEF;
            $crate::log(&::std::format!(
                "proto-{}: init ({} ; n°={} transport={} couche={})",
                d.name, d.brief, d.number,
                if d.transport.is_empty() { "-" } else { d.transport },
                d.layer
            ));
            // Topic per-name == provide `net.<name>.<name>.registered` (provide == emit).
            $crate::emit(
                &::std::format!("net.{n}.{n}.registered", n = d.name),
                ::std::format!(
                    "{{\"name\":\"{}\",\"number\":{},\"transport\":\"{}\",\"layer\":{},\"brief\":\"{}\"}}",
                    $crate::bions::json_esc(d.name),
                    d.number,
                    $crate::bions::json_esc(d.transport),
                    d.layer,
                    $crate::bions::json_esc(d.brief)
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
            $crate::log(&::std::format!("proto-{}: goodbye", __PROTO_DEF.name));
        }

        // --- on_event : net.<name>.in -> parse PDU, grave tsoin, émet .out ------
        #[no_mangle]
        pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
            let (topic, payload) = $crate::bions::decode_event(tp, tl, pp, pl);
            let d = __PROTO_DEF;

            // Seul le PDU entrant de CE protocole nous regarde.
            let in_topic = ::std::format!("net.{}.in", d.name);
            if topic != in_topic {
                return;
            }

            // 1) parse le PDU minimal (modèle {src,dst,kind,len}) via le PDU-BION.
            let pdu = $crate::protocol::parse_pdu(&payload);

            // seq local -> adresse unique du tsoin.
            let seq = __PROTO_SEQ.with(|s| {
                let mut s = s.borrow_mut();
                *s += 1;
                *s
            });

            // 2) grave le tsoin : générateur = nom adressé, résidu = le payload
            //    `in` brut (hex). Le nom est échappé JSON (name générique).
            let tname = ::std::format!(
                "proto:{}:{}",
                $crate::bions::json_esc(d.name),
                seq
            );
            $crate::bions::tsoin_record(&tname, payload.as_bytes());

            // 3) ré-émet le PDU normalisé sur net.<name>.out (vers le consommateur).
            $crate::emit(
                &::std::format!("net.{}.out", d.name),
                ::std::format!(
                    "{{\"proto\":\"{}\",\"number\":{},\"src\":\"{}\",\"dst\":\"{}\",\"kind\":\"{}\",\"len\":{},\"seq\":{}}}",
                    $crate::bions::json_esc(d.name),
                    d.number,
                    $crate::bions::json_esc(&pdu.src),
                    $crate::bions::json_esc(&pdu.dst),
                    $crate::bions::json_esc(&pdu.kind),
                    pdu.len,
                    seq
                )
                .as_bytes(),
            );
        }
    };
}

/// La **table-graine** (le résidu) : la pile réseau réelle, vague 1. Consommée par
/// `protocol_ploxion!` (un ploxion `proto-<name>` par entrée). Les n° sont IANA ;
/// les couches OSI sont la dominante (tls=6/7, quic=4/7 -> on garde la dominante).
///
/// `0` en `number` = pas de n° canonique propre (ws est porté sur tcp). `"-"` en
/// `transport` = le protocole EST son propre transport (icmp/tcp/udp au niveau ip).
pub const PROTOCOLS: &[ProtocolDef] = &[
    ProtocolDef { name: "icmp",  number: 1,   transport: "-",       layer: 3, brief: "echo/erreurs (ping)" },
    ProtocolDef { name: "tcp",   number: 6,   transport: "-",       layer: 4, brief: "flux fiable ordonné" },
    ProtocolDef { name: "udp",   number: 17,  transport: "-",       layer: 4, brief: "datagrammes non fiables" },
    ProtocolDef { name: "dns",   number: 53,  transport: "udp/tcp", layer: 7, brief: "résolution de noms" },
    ProtocolDef { name: "dhcp",  number: 67,  transport: "udp",     layer: 7, brief: "bail d'adresse" },
    ProtocolDef { name: "http",  number: 80,  transport: "tcp",     layer: 7, brief: "requête/réponse web" },
    ProtocolDef { name: "ntp",   number: 123, transport: "udp",     layer: 7, brief: "temps (clock-coherence réseau)" },
    ProtocolDef { name: "tls",   number: 443, transport: "tcp",     layer: 6, brief: "handshake + chiffrement (le /warp transport)" },
    ProtocolDef { name: "quic",  number: 443, transport: "udp",     layer: 4, brief: "transport moderne chiffré" },
    ProtocolDef { name: "ssh",   number: 22,  transport: "tcp",     layer: 7, brief: "shell distant (le boxion ttyd)" },
    ProtocolDef { name: "ws",    number: 0,   transport: "tcp",     layer: 7, brief: "websocket (le bus parle déjà ça)" },
];
