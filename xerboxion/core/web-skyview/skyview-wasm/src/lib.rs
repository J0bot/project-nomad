//! SkyView — le bion de wasm du ploxion planetarium.
//!
//! Un ploxion = "juste un bion de wasm, du code machine" (José #627). Ce module
//! ne fait QUE la math celeste, en `no_std`, sans allocateur : un catalogue des
//! 50 etoiles les plus brillantes (RA/Dec J2000 + magnitude) baked en const, et
//! une fonction `project()` qui transforme (RA,Dec) -> (alt,az) horizontal pour
//! l'observateur -> coordonnees ecran via une projection gnomonique centree sur
//! la direction visee (le telephone pointe le ciel). Le rendu WebGL + les
//! capteurs (orientation/tactile) vivent en JS ; ici c'est le reel : la sphere
//! celeste. Sortie = un buffer statique [x, y, taille, visible] x N que le JS lit
//! par `out_ptr()` dans la memoire lineaire. Deterministe -> rejouable -> tsoin.

#![no_std]

use core::panic::PanicInfo;
use libm::{asinf, atan2f, cosf, sinf, tanf};

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

const N: usize = 50;
const D2R: f32 = 0.017_453_292_5; // pi/180

// (RA en heures, Dec en degres, magnitude) — J2000, les 50 plus brillantes.
// L'ordre EST l'index : il doit matcher NAMES[] et les segments de
// constellations cote JS. Ne pas reordonner sans mettre a jour le JS.
const CAT: [(f32, f32, f32); N] = [
    (6.752, -16.716, -1.46),  // 0  Sirius
    (6.399, -52.696, -0.74),  // 1  Canopus
    (14.261, 19.182, -0.05),  // 2  Arcturus
    (18.616, 38.784, 0.03),   // 3  Vega
    (5.278, 45.998, 0.08),    // 4  Capella
    (5.242, -8.202, 0.13),    // 5  Rigel
    (7.655, 5.225, 0.34),     // 6  Procyon
    (5.919, 7.407, 0.50),     // 7  Betelgeuse
    (1.629, -57.237, 0.46),   // 8  Achernar
    (14.064, -60.373, 0.61),  // 9  Hadar
    (19.846, 8.868, 0.77),    // 10 Altair
    (4.599, 16.509, 0.85),    // 11 Aldebaran
    (16.490, -26.432, 1.09),  // 12 Antares
    (13.420, -11.161, 0.98),  // 13 Spica
    (7.755, 28.026, 1.14),    // 14 Pollux
    (22.961, -29.622, 1.16),  // 15 Fomalhaut
    (20.690, 45.280, 1.25),   // 16 Deneb
    (12.795, -59.689, 1.25),  // 17 Mimosa
    (10.139, 11.967, 1.35),   // 18 Regulus
    (6.977, -28.972, 1.50),   // 19 Adhara
    (7.577, 31.888, 1.58),    // 20 Castor
    (12.519, -57.113, 1.63),  // 21 Gacrux
    (5.418, 6.350, 1.64),     // 22 Bellatrix
    (5.438, 28.608, 1.65),    // 23 Elnath
    (5.604, -1.202, 1.69),    // 24 Alnilam
    (5.679, -1.943, 1.74),    // 25 Alnitak
    (12.900, 55.960, 1.76),   // 26 Alioth
    (5.533, -0.299, 2.23),    // 27 Mintaka
    (11.062, 61.751, 1.79),   // 28 Dubhe
    (11.031, 56.382, 2.37),   // 29 Merak
    (11.897, 53.695, 2.44),   // 30 Phecda
    (12.257, 57.033, 3.31),   // 31 Megrez
    (13.792, 49.313, 1.85),   // 32 Alkaid
    (13.399, 54.925, 2.23),   // 33 Mizar
    (2.530, 89.264, 1.98),    // 34 Polaris
    (0.675, 56.537, 2.24),    // 35 Schedar
    (0.153, 59.150, 2.28),    // 36 Caph
    (0.945, 60.717, 2.47),    // 37 Gamma Cas
    (1.430, 60.235, 2.66),    // 38 Ruchbah
    (1.907, 63.670, 3.38),    // 39 Segin
    (12.443, -63.099, 0.77),  // 40 Acrux
    (12.252, -58.749, 2.79),  // 41 Imai
    (5.796, -9.670, 2.06),    // 42 Saiph
    (5.992, 44.947, 1.90),    // 43 Menkalinan
    (9.460, -8.659, 1.98),    // 44 Alphard
    (2.119, 23.462, 2.00),    // 45 Hamal
    (0.726, -17.987, 2.04),   // 46 Diphda
    (18.921, -26.297, 2.05),  // 47 Nunki
    (3.405, 49.861, 1.79),    // 48 Mirfak
    (3.136, 40.956, 2.12),    // 49 Algol
];

// Buffer de sortie en memoire lineaire : 4 floats par etoile.
//   [i*4+0] = x ecran (px)   [i*4+1] = y ecran (px)
//   [i*4+2] = taille point   [i*4+3] = 1.0 si visible (devant l'oeil), 0.0 sinon
static mut OUT: [f32; N * 4] = [0.0; N * 4];

#[inline]
fn clamp1(v: f32) -> f32 {
    if v > 1.0 {
        1.0
    } else if v < -1.0 {
        -1.0
    } else {
        v
    }
}

#[inline]
fn clampf(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// Nombre d'etoiles du catalogue.
#[no_mangle]
pub extern "C" fn n_stars() -> u32 {
    N as u32
}

/// Pointeur (offset memoire lineaire) du buffer de sortie [x,y,taille,vis] x N.
#[no_mangle]
pub extern "C" fn out_ptr() -> *const f32 {
    // SAFETY: lecture seule cote JS, le buffer vit pour toute la duree du module.
    unsafe { OUT.as_ptr() }
}

/// Projette tout le catalogue pour l'observateur et la direction visee donnes.
///
/// - `lat_deg`        latitude de l'observateur (deg, +N)
/// - `lst_hours`      temps sideral local (heures) — calcule en JS depuis la date+lon
/// - `look_az_deg`    azimut vise (deg depuis le Nord, +Est) = ou pointe l'ecran
/// - `look_alt_deg`   altitude visee (deg au-dessus de l'horizon)
/// - `roll_deg`       roulis de l'appareil (deg) — rotation de l'image
/// - `fov_deg`        champ de vision vertical (deg)
/// - `w`, `h`         taille du canvas (px)
#[no_mangle]
pub extern "C" fn project(
    lat_deg: f32,
    lst_hours: f32,
    look_az_deg: f32,
    look_alt_deg: f32,
    roll_deg: f32,
    fov_deg: f32,
    w: f32,
    h: f32,
) {
    let lat = lat_deg * D2R;
    let az0 = look_az_deg * D2R;
    let alt0 = look_alt_deg * D2R;
    let roll = roll_deg * D2R;
    let focal = (h * 0.5) / tanf(0.5 * fov_deg * D2R);
    let (sr, cr) = (sinf(roll), cosf(roll));
    let (slat, clat) = (sinf(lat), cosf(lat));
    let (salt0, calt0) = (sinf(alt0), cosf(alt0));

    let mut i = 0usize;
    while i < N {
        let (rah, dec_d, mag) = CAT[i];
        let dec = dec_d * D2R;
        // Angle horaire H = LST - RA (en radians).
        let ha = (lst_hours - rah) * 15.0 * D2R;
        let (sdec, cdec) = (sinf(dec), cosf(dec));
        let (sha, cha) = (sinf(ha), cosf(ha));

        // Equatorial -> horizontal (alt/az depuis le Nord, +Est).
        let salt = clamp1(sdec * slat + cdec * clat * cha);
        let alt = asinf(salt);
        let az = atan2f(-cdec * sha, sdec * clat - cdec * slat * cha);
        let (calt, _) = (cosf(alt), salt);

        // Projection gnomonique (tangente) centree sur (alt0, az0).
        let daz = az - az0;
        let cdaz = cosf(daz);
        let cosc = salt0 * salt + calt0 * calt * cdaz;

        let (mut x, mut y, mut vis) = (0.0f32, 0.0f32, 0.0f32);
        if cosc > 0.02 {
            let xx = calt * sinf(daz) / cosc;
            let yy = (calt0 * salt - salt0 * calt * cdaz) / cosc;
            // Roulis + echelle + origine ecran (y vers le bas).
            let sx = (xx * cr - yy * sr) * focal + w * 0.5;
            let sy = -(xx * sr + yy * cr) * focal + h * 0.5;
            x = sx;
            y = sy;
            vis = 1.0;
        }
        let size = clampf(10.0 - 2.3 * mag, 1.6, 12.0);

        // SAFETY: i < N, ecriture exclusive dans le buffer statique.
        unsafe {
            OUT[i * 4] = x;
            OUT[i * 4 + 1] = y;
            OUT[i * 4 + 2] = size;
            OUT[i * 4 + 3] = vis;
        }
        i += 1;
    }
}
