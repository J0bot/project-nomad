//! `package` — le **PACKAGE-BION** : un paquet (dpkg/apt) = un ploxion.
//!
//! José (réseau/système en ploxions, direction bion-Linux) : « les paquets aussi, fais des
//! ploxions ». Un paquet = un nom + une version + une archi + un rôle. Tout le cycle vit ICI (le
//! PACKAGE-BION partagé) ; un paquet = juste son **résidu** : une [`PackageDef`] const. 3ᵉ
//! générateur du réseau-en-ploxions, après [`crate::protocol`] et [`crate::port`]. Génération
//! paresseuse (les paquets utiles à la demande, pas tout `/var/lib/dpkg/status`).
//!
//! ## Le cycle généré par [`package_ploxion!`]
//! - **manifest** : `id="pkg-<name>"`, provides `["pkg.<name>.installed","pkg.<name>.registered",
//!   "tsoin.record"]`, requires `["pkg.<name>.install","pkg.<name>.remove"]`.
//! - **init** : log + `emit "pkg.<name>.registered" {name,version,arch,brief}`.
//! - **on_event** : `pkg.<name>.install` -> grave `pkg:<name>:install:<seq>` (résidu = payload) +
//!   émet `pkg.<name>.installed` ; `pkg.<name>.remove` -> émet `pkg.<name>.removed`. Déterministe.

/// Le **résidu** d'un paquet : tous les champs const. Deux paquets ne diffèrent QUE par ça.
#[derive(Clone, Copy)]
pub struct PackageDef {
    /// nom du paquet, ex `"bash"`, `"coreutils"` (token `[a-z0-9_-]` ; échappé en JSON).
    pub name: &'static str,
    /// version, ex `"5.2.15"`.
    pub version: &'static str,
    /// architecture, ex `"amd64"`, `"all"`.
    pub arch: &'static str,
    /// brief lisible (rôle). Le résidu humain.
    pub brief: &'static str,
}

/// Génère le ploxion **complet** d'un paquet depuis une [`PackageDef`] const.
#[macro_export]
macro_rules! package_ploxion {
    ($def:expr) => {
        const __PKG_DEF: $crate::package::PackageDef = $def;
        thread_local! {
            static __PKG_SEQ: ::std::cell::RefCell<u64> = const { ::std::cell::RefCell::new(0) };
        }

        #[no_mangle]
        pub extern "C" fn plc_manifest() -> i64 {
            thread_local! {
                static __PKG_MANIFEST: ::std::cell::OnceCell<&'static str> =
                    const { ::std::cell::OnceCell::new() };
            }
            let leaked: &'static str = __PKG_MANIFEST.with(|c| {
                *c.get_or_init(|| {
                    let name = $crate::bions::json_esc(__PKG_DEF.name);
                    let json = ::std::format!(
                        "{{\"id\":\"pkg-{name}\",\"version\":\"1.0.0\",\"capabilities\":[],\"provides\":[\"pkg.{name}.installed\",\"pkg.{name}.registered\",\"tsoin.record\"],\"requires\":[\"pkg.{name}.install\",\"pkg.{name}.remove\"],\"children_types\":[],\"parent_types\":[]}}"
                    );
                    &*::std::boxed::Box::leak(json.into_boxed_str())
                })
            });
            $crate::pack_ptr_len(leaked.as_ptr() as u32, leaked.len() as u32)
        }

        #[no_mangle]
        pub extern "C" fn plc_init() {
            let d = __PKG_DEF;
            $crate::log(&::std::format!(
                "pkg-{}: init ({} {} ; {} ; {})", d.name, d.name, d.version, d.arch, d.brief
            ));
            $crate::emit(
                &::std::format!("pkg.{}.registered", d.name),
                ::std::format!(
                    "{{\"name\":\"{}\",\"version\":\"{}\",\"arch\":\"{}\",\"brief\":\"{}\"}}",
                    $crate::bions::json_esc(d.name),
                    $crate::bions::json_esc(d.version),
                    $crate::bions::json_esc(d.arch),
                    $crate::bions::json_esc(d.brief)
                )
                .as_bytes(),
            );
        }

        #[no_mangle]
        pub extern "C" fn plc_health() -> i32 { 0 }

        #[no_mangle]
        pub extern "C" fn plc_goodbye() {
            $crate::log(&::std::format!("pkg-{}: goodbye", __PKG_DEF.name));
        }

        #[no_mangle]
        pub extern "C" fn plc_on_event(tp: i32, tl: i32, pp: i32, pl: i32) {
            let (topic, payload) = $crate::bions::decode_event(tp, tl, pp, pl);
            let d = __PKG_DEF;
            if topic == ::std::format!("pkg.{}.install", d.name) {
                let seq = __PKG_SEQ.with(|s| { let mut s = s.borrow_mut(); *s += 1; *s });
                $crate::bions::tsoin_record(
                    &::std::format!("pkg:{}:install:{}", $crate::bions::json_esc(d.name), seq),
                    payload.as_bytes(),
                );
                $crate::emit(
                    &::std::format!("pkg.{}.installed", d.name),
                    ::std::format!(
                        "{{\"name\":\"{}\",\"version\":\"{}\",\"seq\":{}}}",
                        $crate::bions::json_esc(d.name), $crate::bions::json_esc(d.version), seq
                    )
                    .as_bytes(),
                );
            } else if topic == ::std::format!("pkg.{}.remove", d.name) {
                $crate::emit(
                    &::std::format!("pkg.{}.removed", d.name),
                    ::std::format!("{{\"name\":\"{}\"}}", $crate::bions::json_esc(d.name)).as_bytes(),
                );
            }
        }
    };
}
