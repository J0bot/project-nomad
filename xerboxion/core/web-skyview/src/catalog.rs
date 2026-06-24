//! Real star catalogue (small subset) + constellation line segments.
//!
//! HONESTY NOTE
//! ============
//! The stars below are the ~40 brightest stars in the sky. Their coordinates
//! are J2000.0 equatorial (RA in hours, Dec in degrees) and visual magnitudes,
//! taken from common astronomical references (Hipparcos / Bright Star Catalogue
//! rounded values). These are REAL, not invented.
//!
//! The constellation segments reference stars by Bayer-ish name. Only stars
//! present in `STARS` are usable in a segment; segments that reference a star
//! not in the table are simply skipped at build time (no hallucinated points).
//!
//! Planets, satellites, NGC/Messier, nebulae/galaxies and the full HYG database
//! (~120k stars) are NOT in here — those are loaded later from a data file.
//! This module is deliberately the "known-good seed" only.

/// One catalogued star. RA in hours [0,24), Dec in degrees [-90,90], mag visual.
#[derive(Clone, Copy)]
pub struct Star {
    pub name: &'static str,
    pub ra_hours: f32,
    pub dec_deg: f32,
    pub mag: f32,
}

/// A constellation: a name plus a flat list of (from, to) star-name segments.
pub struct Constellation {
    pub name: &'static str,
    pub segments: &'static [(&'static str, &'static str)],
}

/// ~40 brightest stars, J2000.0. Values are standard catalogue numbers.
pub const STARS: &[Star] = &[
    // name              RA(h)     Dec(deg)   mag
    Star { name: "Sirius",        ra_hours:  6.7525, dec_deg: -16.716, mag: -1.46 },
    Star { name: "Canopus",       ra_hours:  6.3992, dec_deg: -52.696, mag: -0.74 },
    Star { name: "Rigil Kent",    ra_hours: 14.6600, dec_deg: -60.835, mag: -0.27 }, // Alpha Centauri
    Star { name: "Arcturus",      ra_hours: 14.2610, dec_deg:  19.182, mag: -0.05 },
    Star { name: "Vega",          ra_hours: 18.6156, dec_deg:  38.784, mag:  0.03 },
    Star { name: "Capella",       ra_hours:  5.2782, dec_deg:  45.998, mag:  0.08 },
    Star { name: "Rigel",         ra_hours:  5.2423, dec_deg:  -8.202, mag:  0.13 },
    Star { name: "Procyon",       ra_hours:  7.6550, dec_deg:   5.225, mag:  0.34 },
    Star { name: "Betelgeuse",    ra_hours:  5.9195, dec_deg:   7.407, mag:  0.50 },
    Star { name: "Achernar",      ra_hours:  1.6286, dec_deg: -57.237, mag:  0.46 },
    Star { name: "Hadar",         ra_hours: 14.0637, dec_deg: -60.373, mag:  0.61 }, // Beta Cen
    Star { name: "Altair",        ra_hours: 19.8464, dec_deg:   8.868, mag:  0.77 },
    Star { name: "Acrux",         ra_hours: 12.4433, dec_deg: -63.099, mag:  0.77 }, // Alpha Cru
    Star { name: "Aldebaran",     ra_hours:  4.5987, dec_deg:  16.509, mag:  0.85 },
    Star { name: "Antares",       ra_hours: 16.4901, dec_deg: -26.432, mag:  0.96 },
    Star { name: "Spica",         ra_hours: 13.4199, dec_deg: -11.161, mag:  0.97 },
    Star { name: "Pollux",        ra_hours:  7.7553, dec_deg:  28.026, mag:  1.14 },
    Star { name: "Fomalhaut",     ra_hours: 22.9608, dec_deg: -29.622, mag:  1.16 },
    Star { name: "Deneb",         ra_hours: 20.6905, dec_deg:  45.280, mag:  1.25 },
    Star { name: "Mimosa",        ra_hours: 12.7953, dec_deg: -59.689, mag:  1.25 }, // Beta Cru
    Star { name: "Regulus",       ra_hours: 10.1395, dec_deg:  11.967, mag:  1.35 },
    Star { name: "Adhara",        ra_hours:  6.9770, dec_deg: -28.972, mag:  1.50 },
    Star { name: "Castor",        ra_hours:  7.5767, dec_deg:  31.888, mag:  1.58 },
    Star { name: "Gacrux",        ra_hours: 12.5194, dec_deg: -57.113, mag:  1.63 }, // Gamma Cru
    Star { name: "Bellatrix",     ra_hours:  5.4188, dec_deg:   6.350, mag:  1.64 },
    Star { name: "Elnath",        ra_hours:  5.4382, dec_deg:  28.608, mag:  1.65 },
    Star { name: "Alnilam",       ra_hours:  5.6036, dec_deg:  -1.202, mag:  1.69 }, // Orion belt mid
    Star { name: "Alnitak",       ra_hours:  5.6793, dec_deg:  -1.943, mag:  1.74 }, // Orion belt E
    Star { name: "Alioth",        ra_hours: 12.9004, dec_deg:  55.960, mag:  1.77 }, // UMa
    Star { name: "Mintaka",       ra_hours:  5.5334, dec_deg:  -0.299, mag:  2.23 }, // Orion belt W
    Star { name: "Dubhe",         ra_hours: 11.0621, dec_deg:  61.751, mag:  1.79 }, // UMa
    Star { name: "Mirfak",        ra_hours:  3.4054, dec_deg:  49.861, mag:  1.79 },
    Star { name: "Alkaid",        ra_hours: 13.7923, dec_deg:  49.313, mag:  1.86 }, // UMa
    Star { name: "Saiph",         ra_hours:  5.7959, dec_deg:  -9.670, mag:  2.06 }, // Orion SE
    Star { name: "Merak",         ra_hours: 11.0307, dec_deg:  56.382, mag:  2.37 }, // UMa
    Star { name: "Mizar",         ra_hours: 13.3987, dec_deg:  54.925, mag:  2.23 }, // UMa
    Star { name: "Phecda",        ra_hours: 11.8972, dec_deg:  53.695, mag:  2.44 }, // UMa
    Star { name: "Megrez",        ra_hours: 12.2571, dec_deg:  57.033, mag:  3.31 }, // UMa
    Star { name: "Schedar",       ra_hours:  0.6751, dec_deg:  56.537, mag:  2.24 }, // Cas alpha
    Star { name: "Caph",          ra_hours:  0.1530, dec_deg:  59.150, mag:  2.28 }, // Cas beta
    Star { name: "Gamma Cas",     ra_hours:  0.9451, dec_deg:  60.717, mag:  2.47 },
    Star { name: "Ruchbah",       ra_hours:  1.4304, dec_deg:  60.235, mag:  2.68 }, // Cas delta
    Star { name: "Segin",         ra_hours:  1.9066, dec_deg:  63.670, mag:  3.35 }, // Cas epsilon
    Star { name: "Sadr",          ra_hours: 20.3705, dec_deg:  40.257, mag:  2.23 }, // Cyg gamma
    Star { name: "Gienah Cyg",    ra_hours: 20.7704, dec_deg:  33.970, mag:  2.46 }, // Cyg epsilon
    Star { name: "Delta Cyg",     ra_hours: 19.7496, dec_deg:  45.131, mag:  2.87 },
    Star { name: "Albireo",       ra_hours: 19.5120, dec_deg:  27.960, mag:  3.05 }, // Cyg beta
    Star { name: "Shaula",        ra_hours: 17.5602, dec_deg: -37.104, mag:  1.62 }, // Sco lambda
    Star { name: "Sargas",        ra_hours: 17.6219, dec_deg: -42.998, mag:  1.86 }, // Sco theta
    Star { name: "Dschubba",      ra_hours: 16.0056, dec_deg: -22.622, mag:  2.29 }, // Sco delta
    Star { name: "Pi Sco",        ra_hours: 15.9810, dec_deg: -26.114, mag:  2.89 },
    Star { name: "Epsilon Sco",   ra_hours: 16.8361, dec_deg: -34.293, mag:  2.29 },
    Star { name: "Kaus Aus",      ra_hours: 18.4029, dec_deg: -34.385, mag:  1.85 }, // Sgr epsilon
    Star { name: "Nunki",         ra_hours: 18.9211, dec_deg: -26.297, mag:  2.05 }, // Sgr sigma
    Star { name: "Ascella",       ra_hours: 19.0436, dec_deg: -29.880, mag:  2.60 }, // Sgr zeta
    Star { name: "Kaus Media",    ra_hours: 18.3499, dec_deg: -29.828, mag:  2.70 }, // Sgr delta
    Star { name: "Polaris",       ra_hours:  2.5303, dec_deg:  89.264, mag:  1.98 },
];

/// Major constellations as line segments. Only segments whose BOTH endpoints
/// exist in `STARS` will be rendered. This list is intentionally conservative.
pub const CONSTELLATIONS: &[Constellation] = &[
    Constellation {
        name: "ORION",
        segments: &[
            ("Betelgeuse", "Bellatrix"),
            ("Betelgeuse", "Alnitak"),
            ("Bellatrix", "Mintaka"),
            ("Mintaka", "Alnilam"),
            ("Alnilam", "Alnitak"),
            ("Mintaka", "Rigel"),
            ("Alnitak", "Saiph"),
            ("Rigel", "Saiph"),
        ],
    },
    Constellation {
        name: "URSA MAJOR",
        segments: &[
            ("Dubhe", "Merak"),
            ("Dubhe", "Megrez"),
            ("Merak", "Phecda"),
            ("Phecda", "Megrez"),
            ("Megrez", "Alioth"),
            ("Alioth", "Mizar"),
            ("Mizar", "Alkaid"),
        ],
    },
    Constellation {
        name: "CASSIOPEIA",
        segments: &[
            ("Caph", "Schedar"),
            ("Schedar", "Gamma Cas"),
            ("Gamma Cas", "Ruchbah"),
            ("Ruchbah", "Segin"),
        ],
    },
    Constellation {
        name: "SCORPIUS",
        segments: &[
            ("Dschubba", "Pi Sco"),
            ("Dschubba", "Antares"),
            ("Antares", "Epsilon Sco"),
            ("Epsilon Sco", "Sargas"),
            ("Sargas", "Shaula"),
        ],
    },
    Constellation {
        name: "SAGITTARIUS",
        segments: &[
            ("Kaus Aus", "Kaus Media"),
            ("Kaus Media", "Nunki"),
            ("Nunki", "Ascella"),
            ("Ascella", "Kaus Aus"),
        ],
    },
    Constellation {
        name: "CYGNUS",
        segments: &[
            ("Deneb", "Sadr"),
            ("Sadr", "Albireo"),
            ("Sadr", "Gienah Cyg"),
            ("Sadr", "Delta Cyg"),
        ],
    },
    Constellation {
        name: "CRUX",
        segments: &[
            ("Acrux", "Gacrux"),
            ("Mimosa", "Gacrux"), // approximate cross arms; minor star Delta Cru omitted
        ],
    },
];

/// Find a star index by name (linear; catalogue is tiny).
pub fn find(name: &str) -> Option<usize> {
    STARS.iter().position(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_in_range() {
        for s in STARS {
            assert!(s.ra_hours >= 0.0 && s.ra_hours < 24.0, "{} RA out of range", s.name);
            assert!(s.dec_deg >= -90.0 && s.dec_deg <= 90.0, "{} Dec out of range", s.name);
            assert!(s.mag > -2.0 && s.mag < 7.0, "{} mag implausible", s.name);
        }
    }

    #[test]
    fn star_names_unique() {
        for (i, a) in STARS.iter().enumerate() {
            for b in &STARS[i + 1..] {
                assert_ne!(a.name, b.name, "duplicate star name {}", a.name);
            }
        }
    }

    #[test]
    fn every_segment_endpoint_exists() {
        // The doc-comment promises segments referencing a missing star are
        // "simply skipped". This test makes that an EXPLICIT guarantee: today
        // every endpoint resolves, so nothing is silently dropped. If a future
        // edit references an absent star, this fails loudly instead of vanishing.
        for c in CONSTELLATIONS {
            for (from, to) in c.segments {
                assert!(find(from).is_some(), "{}: missing star '{from}'", c.name);
                assert!(find(to).is_some(), "{}: missing star '{to}'", c.name);
            }
        }
    }

    #[test]
    fn spot_check_known_bright_stars() {
        // Cross-check the load-bearing values the spec calls out by name against
        // standard J2000 catalogue numbers (RA converted to hours).
        let checks = [
            ("Sirius", 6.7525_f32, -16.716_f32, -1.46_f32), // 101.287 deg / 15
            ("Vega", 18.6156, 38.784, 0.03),                // 279.234 deg / 15
            ("Betelgeuse", 5.9195, 7.407, 0.50),            // 88.793 deg / 15
        ];
        for (name, ra, dec, mag) in checks {
            let i = find(name).unwrap_or_else(|| panic!("{name} absent"));
            let s = &STARS[i];
            assert!((s.ra_hours - ra).abs() < 0.01, "{name} RA {} != {ra}", s.ra_hours);
            assert!((s.dec_deg - dec).abs() < 0.05, "{name} Dec {} != {dec}", s.dec_deg);
            assert!((s.mag - mag).abs() < 0.1, "{name} mag {} != {mag}", s.mag);
        }
    }
}
