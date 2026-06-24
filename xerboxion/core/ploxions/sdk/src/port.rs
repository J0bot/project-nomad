//! `port` — le **PORT-BION** : un port réseau = une *prise* (socket), en ploxion.
//!
//! José (réseau-en-ploxions) : « tous les ports… fais des ploxions ». Un port = un numéro +
//! un service + les protocoles qui s'y branchent. Tout le cycle (manifest/init/health/goodbye/
//! on_event, gravure du tsoin d'ouverture, ré-émission) vit ICI, une seule fois ; un port = juste
//! son **résidu** : une [`PortDef`] const. Génération PARESSEUSE : les *well-known* à la demande,
//! pas 65535 binaires inutiles. Frère du [`crate::protocol`]-bion (un port est plus simple : pas
//! de PDU à parser, juste open/close).
//!
//! ## Le cycle généré par [`port_ploxion!`]
//! - **manifest** : `id="port-<n°>"`, provides `["port.<n°>.up","port.<n°>.registered",
//!   "tsoin.record"]`, requires `["port.<n°>.open","port.<n°>.close"]`.
//! - **init** : log + `emit "port.<n°>.registered" {number,name,protos,brief}`.
//! - **on_event** : `port.<n°>.open` -> grave `port:<n°>:open:<seq>` (résidu = payload brut) +
//!   émet `port.<n°>.up` ; `port.<n°>.close` -> émet `port.<n°>.down`. Déterministe, aucune dép hôte.

/// Le **résidu** d'un port : tous les champs const. Deux ports ne diffèrent QUE par ça.
#[derive(Clone, Copy)]
pub struct PortDef {
    /// numéro de port IANA, ex `22`, `80`, `443`.
    pub number: u16,
    /// nom du service, ex `"ssh"`, `"http"`, `"https"`.
    pub name: &'static str,
    /// transport(s) qui s'y branchent, ex `"tcp"`, `"tcp/udp"`.
    pub protos: &'static str,
    /// brief lisible (rôle). Le résidu humain.
    pub brief: &'static str,
}

/// Génère le ploxion **complet** d'un port depuis une [`PortDef`] const.
#[macro_export]
macro_rules! port_ploxion {
    ($def:expr) => {
        const __PORT_DEF: $crate::port::PortDef = $def;
        thread_local! {
            static __PORT_SEQ: ::std::cell::RefCell<u64> = const { ::std::cell::RefCell::new(0) };
        }

        #[no_mangle]
        pub extern "C" fn plc_manifest() -> i64 {
            thread_local! {
                static __PORT_MANIFEST: ::std::cell::OnceCell<&'static str> =
                    const { ::std::cell::OnceCell::new() };
            }
            let leaked: &'static str = __PORT_MANIFEST.with(|c| {
                *c.get_or_init(|| {
                    let n = __PORT_DEF.number;
                    let json = ::std::format!(
                        "{{\"id\":\"port-{n}\",\"version\":\"1.0.0\",\"capabilities\":[],\"provides\":[\"port.{n}.up\",\"port.{n}.registered\",\"tsoin.record\"],\"requires\":[\"port.{n}.open\",\"port.{n}.close\"],\"children_types\":[],\"parent_types\":[]}}"
                    );
                    &*::std::boxed::Box::leak(json.into_boxed_str())
                })
            });
            $crate::pack_ptr_len(leaked.as_ptr() as u32, leaked.len() as u32)
        }

        #[no_mangle]
        pub extern "C" fn plc_init() {
            let d = __PORT_DEF;
            $crate::log(&::std::format!(
                "port-{}: init ({} ; {} ; {})", d.number, d.name, d.protos, d.brief
            ));
            $crate::emit(
                &::std::format!("port.{}.registered", d.number),
                ::std::format!(
                    "{{\"number\":{},\"name\":\"{}\",\"protos\":\"{}\",\"brief\":\"{}\"}}",
                    d.number,
                    $crate::bions::json_esc(d.name),
                    $crate::bions::json_esc(d.protos),
                    $crate::bions::json_esc(d.brief)
                )
                .as_bytes(),
            );
        }

        #[no_mangle]
        pub extern "C" fn plc_health() -> i32 { 0 }

        #[no_mangle]
        pub extern "C" fn plc_goodbye() {
            $crate::log(&::std::format!("port-{}: goodbye", __PORT_DEF.number));
        }

        #[no_mangle]
        pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
            let (topic, payload) = $crate::bions::decode_event(tp, tl, pp, pl);
            let d = __PORT_DEF;
            if topic == ::std::format!("port.{}.open", d.number) {
                let seq = __PORT_SEQ.with(|s| { let mut s = s.borrow_mut(); *s += 1; *s });
                $crate::bions::tsoin_record(
                    &::std::format!("port:{}:open:{}", d.number, seq),
                    payload.as_bytes(),
                );
                $crate::emit(
                    &::std::format!("port.{}.up", d.number),
                    ::std::format!(
                        "{{\"number\":{},\"name\":\"{}\",\"seq\":{}}}",
                        d.number, $crate::bions::json_esc(d.name), seq
                    )
                    .as_bytes(),
                );
            } else if topic == ::std::format!("port.{}.close", d.number) {
                $crate::emit(
                    &::std::format!("port.{}.down", d.number),
                    ::std::format!(
                        "{{\"number\":{},\"name\":\"{}\"}}",
                        d.number, $crate::bions::json_esc(d.name)
                    )
                    .as_bytes(),
                );
            }
        }
    };
}
