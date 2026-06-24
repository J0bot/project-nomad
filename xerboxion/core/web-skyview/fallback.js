// SkyView JS fallback core.
// ----------------------------------------------------------------------------
// Implements the SAME contract the Rust/WASM `SkyView` class exposes (verified
// against pkg/skyview.d.ts), so the harness (main.js) runs TODAY before the wasm
// crate is built. Render here is Canvas2D (NOT WebGL2) — deliberately simple —
// but the *math* (IAU 1982 GMST sidereal rotation, RA/Dec -> unit vector ->
// camera-relative projection) is real, and the catalogue is the same real data
// as the Rust crate (imported from ./catalog.data.js).
//
// CONTRACT (must match pkg/skyview.d.ts exactly):
//   new SkyView(canvas_id)
//   set_orientation(yaw_rad, pitch_rad)
//   set_fov(deg)
//   set_time(jd)
//   set_longitude(deg)
//   set_magnitude_limit(m)
//   project_radec(ra_hours, dec_deg) -> [ndc_x, ndc_y, visible]
//   render()
//   resize(w, h, dpr)
//   visible_star_count() -> int
//   catalog_size() -> int
//
// HONESTY: Canvas2D is a stand-in renderer; the real points/lines/labels live in
// the WASM crate. Planets/satellites/full HYG are NOT here (placeholder).
// ----------------------------------------------------------------------------

import { CATALOG } from "./catalog.data.js";

const HOURS2RAD = Math.PI / 12;
const DEG2RAD = Math.PI / 180;

// Same 7 constellations + segments as src/catalog.rs (by star name).
const CONSTELLATIONS = {
  ORION: [["Betelgeuse","Bellatrix"],["Betelgeuse","Alnitak"],["Bellatrix","Mintaka"],
    ["Mintaka","Alnilam"],["Alnilam","Alnitak"],["Mintaka","Rigel"],["Alnitak","Saiph"],["Rigel","Saiph"]],
  "URSA MAJOR": [["Dubhe","Merak"],["Dubhe","Megrez"],["Merak","Phecda"],["Phecda","Megrez"],
    ["Megrez","Alioth"],["Alioth","Mizar"],["Mizar","Alkaid"]],
  CASSIOPEIA: [["Caph","Schedar"],["Schedar","Gamma Cas"],["Gamma Cas","Ruchbah"],["Ruchbah","Segin"]],
  SCORPIUS: [["Dschubba","Pi Sco"],["Dschubba","Antares"],["Antares","Epsilon Sco"],
    ["Epsilon Sco","Sargas"],["Sargas","Shaula"]],
  SAGITTARIUS: [["Kaus Aus","Kaus Media"],["Kaus Media","Nunki"],["Nunki","Ascella"],["Ascella","Kaus Aus"]],
  CYGNUS: [["Deneb","Sadr"],["Sadr","Albireo"],["Sadr","Gienah Cyg"],["Sadr","Delta Cyg"]],
  CRUX: [["Acrux","Gacrux"],["Mimosa","Gacrux"]],
};

const STARS = CATALOG.filter((o) => o.kind === "Star");
const BY_NAME = new Map(CATALOG.map((o) => [o.name, o]));

// RA(hours)/Dec(deg) -> unit vector, same frame as astro::radec_to_vec.
function radecToVec(raHours, decDeg) {
  const ra = raHours * HOURS2RAD, dec = decDeg * DEG2RAD;
  const cd = Math.cos(dec);
  return [cd * Math.cos(ra), cd * Math.sin(ra), Math.sin(dec)];
}

// GMST (radians), IAU 1982 — identical polynomial to astro::gmst_radians.
function gmstRad(jd) {
  const d = jd - 2451545.0, t = d / 36525.0;
  let g = 280.46061837 + 360.98564736629 * d + 0.000387933 * t * t - (t * t * t) / 38710000.0;
  g = ((g % 360) + 360) % 360;
  return g * DEG2RAD;
}

export class SkyView {
  constructor(canvasId) {
    this.canvas = document.getElementById(canvasId);
    this.ctx = this.canvas.getContext("2d");
    this.yaw = 0; this.pitch = 0; this.fov = 70;
    this.jd = 2451545.0; this.lon = 0; this.magLimit = 6.5;
    this.dpr = 1; this.visible = 0;
  }

  // --- REAL contract (mirrors pkg/skyview.d.ts) ---
  set_orientation(yaw, pitch) {
    this.yaw = yaw;
    const lim = Math.PI / 2 - 0.01;
    this.pitch = Math.max(-lim, Math.min(lim, pitch));
  }
  set_fov(deg) { this.fov = Math.max(15, Math.min(110, deg)); }
  set_time(jd) { this.jd = jd; }
  set_longitude(deg) { this.lon = deg; }
  set_magnitude_limit(m) { this.magLimit = m; }
  resize(w, h, dpr) { this.canvas.width = w; this.canvas.height = h; this.dpr = dpr || 1; }
  visible_star_count() { return this.visible; }
  catalog_size() { return STARS.length; }

  // Project (RA hours, Dec deg) the same way the GL pipeline does: rotate sky by
  // -LST about the pole, then into camera (yaw/pitch) space, then a simple
  // perspective. Returns [ndc_x, ndc_y, visible(0/1)].
  project_radec(raHours, decDeg) {
    let v = radecToVec(raHours, decDeg);
    // sky rotation about Z by -lst
    const lst = gmstRad(this.jd) + this.lon * DEG2RAD;
    const cs = Math.cos(-lst), sn = Math.sin(-lst);
    v = [v[0] * cs - v[1] * sn, v[0] * sn + v[1] * cs, v[2]];
    // camera basis from yaw/pitch (eye at origin looking outward, up = +Z)
    const cp = Math.cos(this.pitch);
    const fwd = [cp * Math.cos(this.yaw), cp * Math.sin(this.yaw), Math.sin(this.pitch)];
    const upW = [0, 0, 1];
    let right = cross(fwd, upW); right = norm(right);
    const up = cross(right, fwd);
    const camZ = dot(v, fwd); // forward component
    if (camZ <= 0.0001) return [0, 0, 0]; // behind camera
    const camX = dot(v, right), camY = dot(v, up);
    const f = 1 / Math.tan((this.fov * DEG2RAD) / 2);
    const aspect = (this.canvas.width || 1) / (this.canvas.height || 1);
    return [(camX / camZ) * (f / aspect), (camY / camZ) * f, 1];
  }

  render() {
    const ctx = this.ctx, W = this.canvas.width, H = this.canvas.height;
    ctx.fillStyle = "#05080f"; ctx.fillRect(0, 0, W, H);
    const cx = W / 2, cy = H / 2;

    const toPx = (raH, dec) => {
      const p = this.project_radec(raH, dec);
      if (p[2] < 0.5) return null;
      return [cx + p[0] * cx, cy - p[1] * cy];
    };

    // constellation lines
    ctx.strokeStyle = "rgba(90,140,220,.55)"; ctx.lineWidth = 1.2 * this.dpr;
    for (const segs of Object.values(CONSTELLATIONS)) {
      for (const [a, b] of segs) {
        const sa = BY_NAME.get(a), sb = BY_NAME.get(b);
        if (!sa || !sb) continue;
        const pa = toPx(sa.raHours, sa.dec), pb = toPx(sb.raHours, sb.dec);
        if (!pa || !pb) continue;
        ctx.beginPath(); ctx.moveTo(pa[0], pa[1]); ctx.lineTo(pb[0], pb[1]); ctx.stroke();
      }
    }

    // stars
    let n = 0;
    for (const s of STARS) {
      if (s.mag > this.magLimit) continue;
      const p = toPx(s.raHours, s.dec); if (!p) continue;
      n++;
      const size = Math.max(1.2, Math.min(10, 7 - s.mag * 1.1)) * this.dpr * 0.6;
      const bright = Math.max(0.25, Math.min(1, 1 - (s.mag + 1.5) * 0.1));
      ctx.beginPath();
      ctx.fillStyle = `rgba(${Math.round(217 * bright)},${Math.round(230 * bright)},255,1)`;
      ctx.arc(p[0], p[1], size, 0, Math.PI * 2); ctx.fill();
    }
    this.visible = n;

    ctx.fillStyle = "rgba(150,180,255,.5)";
    ctx.font = `${12 * this.dpr}px system-ui,sans-serif`;
    ctx.fillText("JS fallback (Canvas2D) — build wasm for the real WebGL2 core", 10 * this.dpr, H - 12 * this.dpr);
  }
}

function dot(a, b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
function cross(a, b) { return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]; }
function norm(a) { const l = Math.hypot(a[0], a[1], a[2]) || 1; return [a[0]/l, a[1]/l, a[2]/l]; }
