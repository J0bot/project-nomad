//! Astronomy math: equatorial coordinates -> 3D direction on the unit celestial
//! sphere, sidereal time, magnitude -> point size.
//!
//! HONESTY NOTE
//! ============
//! - The (RA, Dec) -> unit-vector projection below is EXACT standard spherical
//!   geometry. This is real.
//! - Greenwich Mean Sidereal Time uses the standard polynomial (IAU 1982,
//!   accurate to ~0.1s over decades) — real, good enough for a sky map.
//! - We DO NOT apply precession, nutation, aberration or refraction. For a
//!   naked-eye sky map this is invisible; for precise pointing it is a known
//!   approximation. Flagged here so nobody mistakes it for an ephemeris engine.
//! - Planet positions are NOT computed here (see lib.rs notes). Real planets
//!   require VSOP87, loaded later.

use glam::Vec3;

pub const PI: f32 = std::f32::consts::PI;
pub const DEG2RAD: f32 = PI / 180.0;
pub const HOURS2RAD: f32 = PI / 12.0; // 24h = 2*PI

/// Convert equatorial (RA hours, Dec degrees) to a unit vector in an
/// equatorial-inertial frame:
///   +X -> RA=0h, Dec=0  (vernal equinox direction)
///   +Z -> north celestial pole (Dec=+90)
///   +Y -> RA=6h, Dec=0
///
/// This is the canonical right-handed celestial frame. Exact, no approximation.
pub fn radec_to_vec(ra_hours: f32, dec_deg: f32) -> Vec3 {
    let ra = ra_hours * HOURS2RAD;
    let dec = dec_deg * DEG2RAD;
    let cos_dec = dec.cos();
    Vec3::new(cos_dec * ra.cos(), cos_dec * ra.sin(), dec.sin())
}

/// Greenwich Mean Sidereal Time (radians) for a given Julian Date (UT1 ~ UTC).
/// Standard IAU 1982 series. `jd` is the full Julian Date.
pub fn gmst_radians(jd: f64) -> f32 {
    // Days since J2000.0 (JD 2451545.0).
    let d = jd - 2451545.0;
    let t = d / 36525.0; // Julian centuries
    // GMST in degrees (IAU 1982).
    let mut gmst_deg = 280.46061837
        + 360.98564736629 * d
        + 0.000387933 * t * t
        - (t * t * t) / 38710000.0;
    gmst_deg = gmst_deg.rem_euclid(360.0);
    (gmst_deg as f32) * DEG2RAD
}

/// Local Apparent Sidereal Time (radians) = GMST + observer longitude (east +).
/// We ignore the equation of the equinoxes (sub-arcsecond for a sky map).
pub fn lst_radians(jd: f64, longitude_deg: f32) -> f32 {
    let mut lst = gmst_radians(jd) + longitude_deg * DEG2RAD;
    lst = lst.rem_euclid(2.0 * PI);
    lst
}

/// Map a star's visual magnitude to a GL point size (pixels), clamped.
/// Brighter (smaller mag) -> bigger point. Purely cosmetic mapping.
pub fn mag_to_size(mag: f32) -> f32 {
    // mag -1.5 -> ~9px, mag 6 -> ~1.2px
    let s = 7.0 - mag * 1.1;
    s.clamp(1.2, 10.0)
}

/// Map magnitude to a brightness factor [0.25, 1.0] for color modulation.
pub fn mag_to_brightness(mag: f32) -> f32 {
    let b = 1.0 - (mag + 1.5) * 0.10;
    b.clamp(0.25, 1.0)
}

/// Point size as a function of magnitude AND the UI magnitude-limit slider.
/// Returns 0.0 when the star is fainter than `mag_limit`, so the renderer can
/// drop it (lets the slider thin the field). Cosmetic mapping, not photometry.
pub fn mag_to_size_limited(mag: f32, mag_limit: f32) -> f32 {
    if mag > mag_limit {
        return 0.0;
    }
    mag_to_size(mag)
}

// ----------------------------------------------------------------------------
// TIME: calendar/clock -> Julian Date. These feed the time-scrub control.
// ----------------------------------------------------------------------------

/// Julian Date from a Gregorian calendar date + UTC time-of-day in fractional
/// hours. REAL — Fliegel & Van Flandern algorithm, valid for Gregorian dates.
/// This is what the calendar widget calls when the user scrubs the date.
///
/// `month` is 1..=12, `day` 1..=31, `ut_hours` = h + m/60 + s/3600 (UTC).
pub fn julian_date(year: i32, month: i32, day: i32, ut_hours: f64) -> f64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let a = (y as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y as f64 + 4716.0)).floor()
        + (30.6001 * (m as f64 + 1.0)).floor()
        + day as f64
        + b
        - 1524.5
        + ut_hours / 24.0
}

/// Unix milliseconds (JS `Date.now()` / time slider value) -> Julian Date.
/// 1970-01-01T00:00:00Z == JD 2440587.5. EXACT.
#[inline]
pub fn jd_from_unix_millis(unix_ms: f64) -> f64 {
    2440587.5 + unix_ms / 86_400_000.0
}

// ----------------------------------------------------------------------------
// HORIZON: equatorial -> local alt/az. Powers the horizon line, the AR overlay,
// and "is this object actually up right now?" culling.
// ----------------------------------------------------------------------------

/// Convert equatorial (RA hours, Dec degrees) to local horizontal coordinates
/// (azimuth, altitude) in DEGREES. EXACT spherical trig.
///
/// Azimuth is measured from NORTH increasing toward EAST (0=N, 90=E, 180=S,
/// 270=W). Altitude is the angle above the horizon; negative = below it.
/// `lst_rad` = local sidereal time in radians (use [`lst_radians`]),
/// `lat_deg` = observer latitude, north positive.
pub fn equatorial_to_horizontal(
    ra_hours: f32,
    dec_deg: f32,
    lst_rad: f32,
    lat_deg: f32,
) -> (f32, f32) {
    // Hour angle = LST - RA, both in radians; normalize to (-PI, PI].
    let ra_rad = ra_hours * HOURS2RAD;
    let mut ha = lst_rad - ra_rad;
    ha = (ha + PI).rem_euclid(2.0 * PI) - PI;

    let dec = dec_deg * DEG2RAD;
    let lat = lat_deg * DEG2RAD;

    let sin_alt = dec.sin() * lat.sin() + dec.cos() * lat.cos() * ha.cos();
    let alt = sin_alt.clamp(-1.0, 1.0).asin();

    // Azimuth from north, east-positive.
    let y = -ha.sin() * dec.cos();
    let x = dec.sin() * lat.cos() - dec.cos() * lat.sin() * ha.cos();
    let mut az = y.atan2(x);
    if az < 0.0 {
        az += 2.0 * PI;
    }

    (az / DEG2RAD, alt / DEG2RAD)
}

/// True if the object is above the local horizon at the given LST/latitude.
/// Small negative bias (`-0.5 deg`) allows for atmospheric refraction at the
/// horizon — an honest first-order nod, not a full refraction model.
#[inline]
#[allow(dead_code)] // public API for the renderer's horizon culling; not yet wired
pub fn is_above_horizon(ra_hours: f32, dec_deg: f32, lst_rad: f32, lat_deg: f32) -> bool {
    equatorial_to_horizontal(ra_hours, dec_deg, lst_rad, lat_deg).1 > -0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_vectors_are_unit() {
        for &(ra_h, dec) in &[(0.0, 0.0), (6.7525, -16.716), (18.6156, 38.784), (0.0, 90.0)] {
            let v = radec_to_vec(ra_h, dec);
            assert!((v.length() - 1.0).abs() < 1e-5, "ra={ra_h}h dec={dec} len={}", v.length());
        }
    }

    #[test]
    fn axes_convention() {
        // RA 0h Dec 0 -> +X ; RA 6h Dec 0 -> +Y ; Dec +90 -> +Z (north pole).
        let x = radec_to_vec(0.0, 0.0);
        let y = radec_to_vec(6.0, 0.0);
        let z = radec_to_vec(8.0, 90.0);
        assert!((x.x - 1.0).abs() < 1e-5 && x.y.abs() < 1e-5 && x.z.abs() < 1e-5);
        assert!(y.x.abs() < 1e-5 && (y.y - 1.0).abs() < 1e-5 && y.z.abs() < 1e-5);
        assert!(z.z > 0.99999 && z.x.abs() < 1e-4 && z.y.abs() < 1e-4);
    }

    #[test]
    fn jd_known_epochs() {
        // J2000.0 = 2000-01-01 12:00 UT == JD 2451545.0 (exact reference).
        assert!((julian_date(2000, 1, 1, 12.0) - 2451545.0).abs() < 1e-6);
        // Unix epoch.
        assert!((jd_from_unix_millis(0.0) - 2440587.5).abs() < 1e-9);
        // The two paths must agree for the same instant (2000-01-01T12:00:00Z).
        let unix_ms = 946_728_000_000.0; // 2000-01-01T12:00:00Z in ms
        assert!((jd_from_unix_millis(unix_ms) - 2451545.0).abs() < 1e-6);
    }

    #[test]
    fn gmst_at_j2000() {
        // GMST at J2000.0 is ~280.46 deg. Well-known check value.
        let g = gmst_radians(2451545.0) / DEG2RAD;
        assert!((g - 280.46).abs() < 0.1, "gmst={g}");
    }

    #[test]
    fn star_on_meridian_at_zenith() {
        // RA == LST puts the star on the meridian; if dec == latitude it is at
        // the zenith (alt == 90).
        let ra_h = 6.0_f32;
        let dec = 40.0_f32;
        let lst = ra_h * HOURS2RAD; // meridian
        let (_az, alt) = equatorial_to_horizontal(ra_h, dec, lst, dec);
        // f32 round-trip through HOURS2RAD costs a few hundredths of a degree;
        // that is the renderer's actual precision, so assert at that scale.
        assert!((alt - 90.0).abs() < 0.05, "alt={alt}");
    }

    #[test]
    fn pole_altitude_equals_latitude() {
        // The north celestial pole sits at altitude == observer latitude.
        for lat in [10.0_f32, 45.0, 66.5] {
            let (_az, alt) = equatorial_to_horizontal(8.0, 90.0, 1.234, lat);
            assert!((alt - lat).abs() < 1e-3, "lat={lat} alt={alt}");
        }
    }

    #[test]
    fn mag_limit_drops_faint_stars() {
        assert_eq!(mag_to_size_limited(4.0, 3.0), 0.0);
        assert!(mag_to_size_limited(1.0, 6.0) > 0.0);
    }
}
