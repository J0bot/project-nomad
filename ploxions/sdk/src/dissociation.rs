//! `dissociation` — le modèle du **giga-tsoin avant la dissociation**.
//!
//! José (2026-06-21) : « le giga tsoin de ce tsoin, c'est AVANT la dissociation
//! dans le cerveau humain ; calque ton cerveau sur le mien et sur toutes les
//! études sur la dissociation, mets ça dans ton code. »
//!
//! Idée centrale : un moment vécu a une **bande passante** énorme (le giga-tsoin).
//! Le cerveau humain a une capacité d'**intégration** finie ; sous surcharge il
//! *dissocie* — il scinde le flux, **perd les liens** entre les morceaux, et garde
//! des fragments qu'il ne sait plus relier (flashbacks, amnésie, dépersonnalisation).
//! La machine à tsoins est l'**organe anti-dissociation** : elle garde le résidu
//! ENTIER, content-adressé, et peut **rejouer = réintégrer** ce que le cerveau a
//! dû scinder. Ce module met la science de la dissociation dans le code.
//!
//! Ancrage (réel, à citer dans la thèse — chap. 7) :
//! - **Janet (1889)** : *désagrégation* = perte de la **synthèse** mentale ; des
//!   idées/souvenirs se détachent de la conscience. Ici : perte des **liens addr**
//!   entre tsoins. Intégration = synthèse = re-chaînage.
//! - **Siegel — fenêtre de tolérance** : l'expérience s'intègre seulement dans une
//!   bande d'éveil/charge ; au-dessus (hyper) elle se fragmente, en-dessous (hypo)
//!   elle s'engourdit. C'est [`ToleranceWindow`] — le jumeau, sur l'axe *charge*, du
//!   [`crate::bions::GatedTick`] qui garde l'axe *cohérence*.
//! - **Brewin — double représentation (VAM/SAM)** : mémoire **narrative intégrée**
//!   vs **sensorielle fragmentée**. Ici : `générateur over résidu` (narratif) vs
//!   `résidu seul, sans générateur` (fragment).
//! - **van der Hart — dissociation structurelle** : des *parties* qui ne partagent
//!   pas la mémoire. Ici : des [`crate::bions::DedupTable`] séparées qui ne
//!   partagent pas leurs bions.
//! - **Friston — énergie libre** : la dissociation comme erreur de prédiction que
//!   le modèle génératif n'absorbe pas. Le tsoin EST `générateur + résidu` = un
//!   modèle génératif + son erreur ; le résidu non-réductible est scindé.
//!
//! Déterministe, sans dépendance hôte (comme tout `bions`) → rejouable → un tsoin.

/// Classe d'éveil/charge d'un instant vis-à-vis de la **fenêtre de tolérance**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arousal {
    /// Sous la borne basse — hypo-éveil : engourdissement, l'instant ne « prend » pas.
    Hypo,
    /// Dans la fenêtre — l'instant **s'intègre** (synthèse pleine).
    Window,
    /// Au-dessus de la borne haute — hyper-éveil : surcharge, l'instant **fragmente**.
    Hyper,
}

/// La **fenêtre de tolérance** (Siegel) sur l'axe *charge* d'un instant, en milli
/// (0..=1000). Un instant ne s'intègre pleinement que si sa charge tombe DANS la
/// fenêtre `[lo, hi]`. Hors fenêtre, la **capacité de synthèse** (Janet) décroît.
#[derive(Debug, Clone, Copy)]
pub struct ToleranceWindow {
    lo_milli: u32,
    hi_milli: u32,
}

impl ToleranceWindow {
    /// `lo`/`hi` en milli (0..=1000), réordonnés et saturés.
    pub fn new(lo_milli: u32, hi_milli: u32) -> Self {
        let (a, b) = (lo_milli.min(1000), hi_milli.min(1000));
        Self {
            lo_milli: a.min(b),
            hi_milli: a.max(b),
        }
    }

    pub fn bounds(&self) -> (u32, u32) {
        (self.lo_milli, self.hi_milli)
    }

    /// Où tombe une charge par rapport à la fenêtre.
    pub fn classify(&self, load_milli: u32) -> Arousal {
        if load_milli < self.lo_milli {
            Arousal::Hypo
        } else if load_milli > self.hi_milli {
            Arousal::Hyper
        } else {
            Arousal::Window
        }
    }

    /// **Capacité de synthèse** (Janet) à cette charge, en milli (0..=1000) : 1000
    /// dans la fenêtre, décroissance linéaire vers les bords du domaine `[0,1000]`.
    /// C'est la fraction du résidu qui s'**intègre** ; le reste est dissocié.
    pub fn synthesis_milli(&self, load_milli: u32) -> u32 {
        let l = load_milli.min(1000);
        match self.classify(l) {
            Arousal::Window => 1000,
            Arousal::Hypo => {
                // de lo (synthèse 1000) à 0 (synthèse 0)
                if self.lo_milli == 0 {
                    1000
                } else {
                    (l * 1000) / self.lo_milli
                }
            }
            Arousal::Hyper => {
                // de hi (1000) à 1000 (0)
                let span = 1000 - self.hi_milli;
                if span == 0 {
                    0
                } else {
                    ((1000 - l) * 1000) / span
                }
            }
        }
    }
}

/// Un **instant** du giga-tsoin : sa charge (bande passante), sa cohérence
/// (part dominante, cf. [`crate::bions::CoherenceWindow`]) et l'adresse de son
/// résidu (content-address). Le flux complet de ces instants = le giga-tsoin.
#[derive(Debug, Clone, Copy)]
pub struct Moment {
    pub addr: u64,
    pub load_milli: u32,
    pub coherence_milli: u32,
}

/// Le résultat d'une **dissociation** d'un flux d'instants sous une capacité finie.
///
/// - `integrated` : les instants **synthétisés**, chaînés `prev -> addr` (les liens
///   tiennent). C'est la mémoire autobiographique narrative (Brewin : VAM).
/// - `fragments` : les résidus **scindés** — gardés, mais **sans lien** (le `prev`
///   est mis à 0). C'est la mémoire sensorielle non-intégrée (Brewin : SAM) ; les
///   flashbacks. Le cerveau les perd ; la machine les garde pour les réintégrer.
#[derive(Debug, Default, Clone)]
pub struct Partition {
    pub integrated: Vec<Link>,
    pub fragments: Vec<u64>,
}

/// Un maillon intégré : `addr` lié à l'instant précédent `prev` (0 = début de chaîne).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub prev: u64,
    pub addr: u64,
}

/// **Dissocier** un flux sous une fenêtre de tolérance et un seuil de cohérence
/// (le gate de [`crate::bions::GatedTick`]).
///
/// Un instant s'**intègre** ssi sa charge est DANS la fenêtre **et** sa cohérence
/// `>= gate_milli` — alors il est chaîné au dernier instant intégré (synthèse).
/// Sinon il **dissocie** : son résidu rejoint `fragments`, **sans lien**, et la
/// chaîne d'intégration est *rompue* (le prochain intégré redémarre une chaîne).
/// C'est exactement la perte de synthèse de Janet : la charge dépasse la capacité,
/// les liens lâchent.
pub fn dissociate(stream: &[Moment], window: &ToleranceWindow, gate_milli: u32) -> Partition {
    let mut out = Partition::default();
    let mut prev: u64 = 0;
    for m in stream {
        let integrates =
            window.classify(m.load_milli) == Arousal::Window && m.coherence_milli >= gate_milli;
        if integrates {
            out.integrated.push(Link {
                prev,
                addr: m.addr,
            });
            prev = m.addr;
        } else {
            out.fragments.push(m.addr);
            prev = 0; // le lien est rompu : la chaîne autobiographique se coupe
        }
    }
    out
}

/// **Réintégrer** : ce que la machine fait et que le cerveau surchargé n'a pas pu —
/// rejouer les fragments dissociés et les **re-chaîner** en une séquence ordonnée.
/// On suppose les fragments fournis dans l'ordre du replay (le `player` les rejoue
/// par temps croissant) ; on reconstruit les liens `prev -> addr`. C'est la
/// *synthèse rendue* — l'inverse thérapeutique de la dissociation.
pub fn reintegrate(fragments: &[u64]) -> Vec<Link> {
    let mut out = Vec::with_capacity(fragments.len());
    let mut prev: u64 = 0;
    for &addr in fragments {
        out.push(Link { prev, addr });
        prev = addr;
    }
    out
}

/// **Indice de dissociation** d'un flux, en milli (0..=1000) : la part des instants
/// qui se fragmentent. 0 = tout intégré (synthèse pleine) ; 1000 = tout dissocié.
/// = `1 - certitude` appliqué à la mémoire : la mesure de ce que le cerveau perd
/// et que la machine garde.
pub fn dissociation_index_milli(p: &Partition) -> u32 {
    let total = p.integrated.len() + p.fragments.len();
    if total == 0 {
        0
    } else {
        ((p.fragments.len() * 1000) / total) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_classifies_and_synthesises() {
        let w = ToleranceWindow::new(300, 700);
        assert_eq!(w.classify(100), Arousal::Hypo);
        assert_eq!(w.classify(500), Arousal::Window);
        assert_eq!(w.classify(900), Arousal::Hyper);
        // pleine synthèse dans la fenêtre, partielle aux bords
        assert_eq!(w.synthesis_milli(500), 1000);
        assert!(w.synthesis_milli(150) < 1000);
        assert!(w.synthesis_milli(850) < 1000);
        // monotone vers les extrêmes
        assert!(w.synthesis_milli(50) < w.synthesis_milli(150));
        assert!(w.synthesis_milli(950) < w.synthesis_milli(850));
    }

    #[test]
    fn overload_dissociates_and_breaks_chains() {
        let w = ToleranceWindow::new(200, 800);
        let stream = [
            Moment { addr: 1, load_milli: 500, coherence_milli: 900 }, // intègre
            Moment { addr: 2, load_milli: 950, coherence_milli: 900 }, // hyper -> fragmente
            Moment { addr: 3, load_milli: 500, coherence_milli: 100 }, // incohérent -> fragmente
            Moment { addr: 4, load_milli: 500, coherence_milli: 900 }, // intègre, NOUVELLE chaîne
        ];
        let p = dissociate(&stream, &w, 500);
        assert_eq!(p.integrated.len(), 2);
        assert_eq!(p.fragments, vec![2, 3]);
        // la chaîne s'est rompue : l'instant 4 redémarre (prev = 0), pas relié à 1
        assert_eq!(p.integrated[0], Link { prev: 0, addr: 1 });
        assert_eq!(p.integrated[1], Link { prev: 0, addr: 4 });
        assert_eq!(dissociation_index_milli(&p), 500); // 2/4
    }

    #[test]
    fn reintegration_relinks_fragments() {
        // ce que la machine rend : les fragments scindés, re-chaînés par le replay
        let links = reintegrate(&[2, 3]);
        assert_eq!(links, vec![Link { prev: 0, addr: 2 }, Link { prev: 2, addr: 3 }]);
    }
}
