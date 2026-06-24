// JS mirror of the REAL bright-star catalogue (src/catalog.rs).
// ----------------------------------------------------------------------------
// Purpose: the search box, the reticule label, and "center on object" need
// object *names* + coordinates in JS. The astro MATH (projection, sidereal
// rotation) is NOT duplicated here — main.js calls the WASM `project_radec`
// for positions. This file only carries the same name/RA/Dec/mag rows so the
// two stay in sync.
//
// HONESTY: these are the SAME ~40 brightest stars as the Rust crate, J2000,
// RA in HOURS, Dec in degrees, visual magnitude. Real catalogue values.
// Planets / satellites / full HYG (~120k) are NOT here — placeholder, loaded
// from a data file later (see README).
//
// Keep in sync with src/catalog.rs (RA is in hours in BOTH).
// ----------------------------------------------------------------------------

// [name, RA(hours), Dec(deg), mag, kind]
const STAR_ROWS = [
  ["Sirius",      6.7525, -16.716, -1.46],
  ["Canopus",     6.3992, -52.696, -0.74],
  ["Rigil Kent", 14.6600, -60.835, -0.27],
  ["Arcturus",   14.2610,  19.182, -0.05],
  ["Vega",       18.6156,  38.784,  0.03],
  ["Capella",     5.2782,  45.998,  0.08],
  ["Rigel",       5.2423,  -8.202,  0.13],
  ["Procyon",     7.6550,   5.225,  0.34],
  ["Betelgeuse",  5.9195,   7.407,  0.50],
  ["Achernar",    1.6286, -57.237,  0.46],
  ["Hadar",      14.0637, -60.373,  0.61],
  ["Altair",     19.8464,   8.868,  0.77],
  ["Acrux",      12.4433, -63.099,  0.77],
  ["Aldebaran",   4.5987,  16.509,  0.85],
  ["Antares",    16.4901, -26.432,  0.96],
  ["Spica",      13.4199, -11.161,  0.97],
  ["Pollux",      7.7553,  28.026,  1.14],
  ["Fomalhaut",  22.9608, -29.622,  1.16],
  ["Deneb",      20.6905,  45.280,  1.25],
  ["Mimosa",     12.7953, -59.689,  1.25],
  ["Regulus",    10.1395,  11.967,  1.35],
  ["Adhara",      6.9770, -28.972,  1.50],
  ["Castor",      7.5767,  31.888,  1.58],
  ["Gacrux",     12.5194, -57.113,  1.63],
  ["Bellatrix",   5.4188,   6.350,  1.64],
  ["Elnath",      5.4382,  28.608,  1.65],
  ["Alnilam",     5.6036,  -1.202,  1.69],
  ["Alnitak",     5.6793,  -1.943,  1.74],
  ["Alioth",     12.9004,  55.960,  1.77],
  ["Mintaka",     5.5334,  -0.299,  2.23],
  ["Dubhe",      11.0621,  61.751,  1.79],
  ["Mirfak",      3.4054,  49.861,  1.79],
  ["Alkaid",     13.7923,  49.313,  1.86],
  ["Saiph",       5.7959,  -9.670,  2.06],
  ["Merak",      11.0307,  56.382,  2.37],
  ["Mizar",      13.3987,  54.925,  2.23],
  ["Phecda",     11.8972,  53.695,  2.44],
  ["Megrez",     12.2571,  57.033,  3.31],
  ["Schedar",     0.6751,  56.537,  2.24],
  ["Caph",        0.1530,  59.150,  2.28],
  ["Gamma Cas",   0.9451,  60.717,  2.47],
  ["Ruchbah",     1.4304,  60.235,  2.68],
  ["Segin",       1.9066,  63.670,  3.35],
  ["Sadr",       20.3705,  40.257,  2.23],
  ["Gienah Cyg", 20.7704,  33.970,  2.46],
  ["Delta Cyg",  19.7496,  45.131,  2.87],
  ["Albireo",    19.5120,  27.960,  3.05],
  ["Shaula",     17.5602, -37.104,  1.62],
  ["Sargas",     17.6219, -42.998,  1.86],
  ["Dschubba",   16.0056, -22.622,  2.29],
  ["Pi Sco",     15.9810, -26.114,  2.89],
  ["Epsilon Sco",16.8361, -34.293,  2.29],
  ["Kaus Aus",   18.4029, -34.385,  1.85],
  ["Nunki",      18.9211, -26.297,  2.05],
  ["Ascella",    19.0436, -29.880,  2.60],
  ["Kaus Media", 18.3499, -29.828,  2.70],
  ["Polaris",     2.5303,  89.264,  1.98],
];

// A few REAL deep-sky objects (Messier). Placeholder category coverage; real
// positions only for these well-known ones.
const DSO_ROWS = [
  ["M31 Andromeda Galaxy",  0.7123,  41.269, 3.4, "Messier"],
  ["M42 Orion Nebula",      5.5881,  -5.391, 4.0, "Nebula"],
  ["M45 Pleiades",          3.7900,  24.117, 1.6, "Messier"],
  ["M44 Beehive",           8.6683,  19.667, 3.7, "Messier"],
  ["M13 Hercules Cluster", 16.6948,  36.461, 5.8, "Messier"],
];

export const CATALOG = [
  ...STAR_ROWS.map(([name, raHours, dec, mag]) => ({ name, raHours, dec, mag, kind: "Star" })),
  ...DSO_ROWS.map(([name, raHours, dec, mag, kind]) => ({ name, raHours, dec, mag, kind })),
];

// Lookup by name -> the catalogue row (for constellation centroids below).
const BY_NAME = new Map(CATALOG.map((o) => [o.name, o]));

// Constellation "objects" for search/center: a name + a representative star so
// searching e.g. "SCORPIUS" centers the sky near it. The real line geometry is
// drawn by the WASM core (src/catalog.rs CONSTELLATIONS); here we only need a
// pointable position. Anchor on a bright member that exists in CATALOG.
const CONSTELLATION_ANCHORS = [
  ["ORION", "Betelgeuse"],
  ["URSA MAJOR", "Dubhe"],
  ["CASSIOPEIA", "Schedar"],
  ["SCORPIUS", "Antares"],
  ["SAGITTARIUS", "Kaus Aus"],
  ["CYGNUS", "Deneb"],
  ["CRUX", "Acrux"],
];

export const CONSTELLATION_OBJECTS = CONSTELLATION_ANCHORS
  .filter(([, anchor]) => BY_NAME.has(anchor))
  .map(([name, anchor]) => {
    const a = BY_NAME.get(anchor);
    return { name, raHours: a.raHours, dec: a.dec, mag: null, kind: "Constellation" };
  });
