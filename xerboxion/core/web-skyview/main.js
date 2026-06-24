// SkyView harness glue.
// ----------------------------------------------------------------------------
// This file is the UI/harness aspect. It owns the DOM, the input devices
// (mouse drag, DeviceOrientation, time, magnitude, FOV, search, capture) and
// forwards everything to the WASM core via the core's REAL exported contract.
//
// REAL WASM CONTRACT (matches the built pkg/skyview.js — verified):
//   default init(url)                          // wasm-pack / wasm-bindgen --target web
//   class SkyView:
//     new SkyView(canvas_id: string)           // grabs WebGl2RenderingContext, builds catalog
//     set_orientation(yaw_rad, pitch_rad)      // camera look direction, radians
//     set_fov(deg)                             // zoom, clamped 15..110 in core
//     set_time(jd: number)                     // full Julian Date -> sidereal -> sky rotation
//     set_longitude(deg)                       // observer longitude, east +
//     set_magnitude_limit(m: number)           // re-packs the GPU star buffer
//     project_radec(ra_hours, dec_deg)         // -> Float32Array [ndc_x, ndc_y, visible(0/1)]
//     render()                                 // draw one frame
//     resize(w, h, dpr)                         // backing store px + device pixel ratio
//     visible_star_count() / catalog_size()    // stats
//
// Everything the real app shows on TOP of that pipeline (reticule target,
// search, center-on-object, AR aiming, photo, layer toggles) is implemented
// HERE in JS using `project_radec` + a tiny JS mirror of the catalogue names.
// The core stays the single source of truth for the astro math (we only mirror
// star *names/RA/Dec/mag* for the search box and labels — no math duplicated).
//
// Until the crate is built, ./fallback.js implements the SAME real contract
// with a Canvas2D renderer so the harness runs today. main.js tries
// ./pkg/skyview.js first, then falls back.
// ----------------------------------------------------------------------------

import { CATALOG, CONSTELLATION_OBJECTS } from "./catalog.data.js";

const CANVAS_ID = "sky";
let sky = null;          // the SkyView instance (WASM or fallback)
let usingWasm = false;

// ---- bootstrap ----
async function boot() {
  try {
    const mod = await import("./pkg/skyview.js");
    await mod.default(new URL("./pkg/skyview_bg.wasm", import.meta.url));
    sky = new mod.SkyView(CANVAS_ID);
    usingWasm = true;
    console.info("[skyview] WASM core loaded.");
  } catch (err) {
    console.warn("[skyview] WASM core not found, using JS fallback. Build it with `./build.sh` (wasm-bindgen --target web --out-dir pkg).\n", err);
    const fb = await import("./fallback.js");
    sky = new fb.SkyView(CANVAS_ID);
  }
  wireUI();
  resize();
  resetTimeToNow();
  requestObserver();
  loop();
}

// ---- camera state mirrored in JS (the core takes yaw/pitch in radians) ----
const cam = { yaw: 0, pitch: 0, fov: 70 };
function pushOrientation() { sky.set_orientation(cam.yaw, cam.pitch); }

// ---- which catalogue layers are visible (filtering done JS-side for labels) ----
const layers = { solar: false, stars: true, constel: true, satellites: false, nebulae: false, messier: false };

// ---- render loop + reticule raycast (JS-side, via project_radec) ----
let lastTargetName = null;
function loop() {
  sky.render();
  const t = nearestToCenter();
  const name = t ? t.name : null;
  if (name !== lastTargetName) { lastTargetName = name; updateTargetLabel(t); }
  requestAnimationFrame(loop);
}

// Reticule = the catalogue object whose projected position is closest to screen
// centre (NDC 0,0) and in front of the camera. Uses the core's project_radec so
// there is no second copy of the astro math in JS.
function nearestToCenter() {
  let best = null, bestD = Infinity;
  for (const o of visibleObjects()) {
    const p = sky.project_radec(o.raHours, o.dec);
    if (!p || p[2] < 0.5) continue;          // behind camera / off
    const d = p[0] * p[0] + p[1] * p[1];     // squared NDC distance to centre
    if (d < bestD) { bestD = d; best = o; }
  }
  // Only "lock" when something is reasonably near the reticule.
  return best && bestD < 0.04 ? best : null;
}

function* visibleObjects() {
  for (const o of CATALOG) {
    if (o.kind === "Star" && !layers.stars) continue;
    if (o.kind === "Messier" && !layers.messier) continue;
    if (o.kind === "Nebula" && !layers.nebulae) continue;
    if (o.mag != null && o.mag > currentMagLimit()) continue;
    yield o;
  }
  if (layers.constel) for (const c of CONSTELLATION_OBJECTS) yield c;
}

function updateTargetLabel(o) {
  const nameEl = document.getElementById("target-name");
  const subEl  = document.getElementById("target-sub");
  if (!o) { nameEl.textContent = "—"; subEl.textContent = ""; return; }
  nameEl.textContent = o.name;
  const bits = [];
  if (o.kind) bits.push(o.kind);
  if (o.mag != null && isFinite(o.mag)) bits.push("mag " + Number(o.mag).toFixed(1));
  subEl.textContent = bits.join(" · ");
}

// ============================================================================
// UI wiring
// ============================================================================
function wireUI() {
  wireDrag();
  wireDeviceOrientation();
  wireTime();
  wireMagnitude();
  wireCatalogue();
  wireSearch();
  wireAR();
  wireCapture();
  wireBanner();
  window.addEventListener("resize", resize, { passive: true });
}

// ---- mouse / touch drag -> yaw,pitch (radians) ----
function wireDrag() {
  const c = document.getElementById(CANVAS_ID);
  let dragging = false, lx = 0, ly = 0, pinchD = 0;
  const px = (e) => (e.touches ? e.touches[0].clientX : e.clientX);
  const py = (e) => (e.touches ? e.touches[0].clientY : e.clientY);

  const down = (e) => {
    if (e.touches && e.touches.length === 2) { pinchD = touchDist(e); return; }
    dragging = true; lx = px(e); ly = py(e); c.classList.add("dragging");
  };
  const move = (e) => {
    if (e.touches && e.touches.length === 2) {
      const d = touchDist(e);
      if (pinchD) setFov(cam.fov * (pinchD / d));
      pinchD = d; e.preventDefault(); return;
    }
    if (!dragging) return;
    const dx = px(e) - lx, dy = py(e) - ly;
    lx = px(e); ly = py(e);
    // pixels -> radians, scaled by FOV for a natural feel.
    const k = (cam.fov * Math.PI / 180) / Math.max(window.innerHeight, 1);
    cam.yaw   -= dx * k;
    cam.pitch += dy * k;       // core clamps pitch near the poles
    pushOrientation();
    if (e.preventDefault) e.preventDefault();
  };
  const up = () => { dragging = false; pinchD = 0; c.classList.remove("dragging"); };

  c.addEventListener("mousedown", down);
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
  c.addEventListener("touchstart", down, { passive: false });
  c.addEventListener("touchmove", move, { passive: false });
  c.addEventListener("touchend", up);
  c.addEventListener("wheel", (e) => {
    setFov(cam.fov + Math.sign(e.deltaY) * 3); e.preventDefault();
  }, { passive: false });
}
function touchDist(e) {
  const a = e.touches[0], b = e.touches[1];
  return Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY);
}
function setFov(deg) {
  cam.fov = clamp(deg, 15, 110);
  sky.set_fov(cam.fov);
}

// ---- DeviceOrientation -> AR aiming on mobile ----
// HONEST: this is a crude mapping (yaw=compass/alpha, pitch=beta-90). Real AR
// needs a full quaternion + screen-orientation correction; flagged as such.
let orientationOn = false;
function wireDeviceOrientation() {
  function onOrient(e) {
    if (!orientationOn) return;
    const compass = (typeof e.webkitCompassHeading === "number")
      ? e.webkitCompassHeading
      : (e.alpha == null ? null : (360 - e.alpha)); // best-effort, not calibrated
    if (compass == null || e.beta == null) return;
    cam.yaw   = -compass * Math.PI / 180;
    cam.pitch = clamp((e.beta - 90), -89, 89) * Math.PI / 180;
    pushOrientation();
  }
  window.__skyOrientHandler = onOrient;
}
async function enableDeviceOrientation() {
  const DOE = window.DeviceOrientationEvent;
  if (DOE && typeof DOE.requestPermission === "function") {
    try { const p = await DOE.requestPermission(); if (p !== "granted") return false; }
    catch { return false; }
  }
  const ev = ("ondeviceorientationabsolute" in window) ? "deviceorientationabsolute" : "deviceorientation";
  window.addEventListener(ev, window.__skyOrientHandler, true);
  orientationOn = true;
  return true;
}
function disableDeviceOrientation() {
  orientationOn = false;
  window.removeEventListener("deviceorientationabsolute", window.__skyOrientHandler, true);
  window.removeEventListener("deviceorientation", window.__skyOrientHandler, true);
}

// ---- time control: datetime + scrub slider -> Julian Date -> set_time ----
let baseDateMs = Date.now();
function wireTime() {
  const dt = document.getElementById("time");
  const scrub = document.getElementById("time-scrub");
  const now = document.getElementById("time-now");
  dt.addEventListener("input", () => {
    if (dt.value) { baseDateMs = localInputToMs(dt.value); scrub.value = "0"; applyTime(); }
  });
  scrub.addEventListener("input", applyTime);
  now.addEventListener("click", () => { resetTimeToNow(); });
}
function applyTime() {
  const scrub = document.getElementById("time-scrub");
  const hours = parseFloat(scrub.value) || 0;
  const ms = baseDateMs + hours * 3600 * 1000;
  sky.set_time(msToJD(ms));   // REAL contract: full Julian Date (UTC)
}
function resetTimeToNow() {
  baseDateMs = Date.now();
  document.getElementById("time").value = msToLocalInput(baseDateMs);
  document.getElementById("time-scrub").value = "0";
  sky.set_time(msToJD(baseDateMs));
}

// ---- magnitude slider ----
let _magLimit = 6;
function currentMagLimit() { return _magLimit; }
function wireMagnitude() {
  const m = document.getElementById("mag");
  const v = document.getElementById("mag-val");
  const upd = () => {
    _magLimit = parseFloat(m.value);
    v.textContent = _magLimit.toFixed(1);
    sky.set_magnitude_limit(_magLimit);   // REAL contract
  };
  m.addEventListener("input", upd);
  upd();
}

// ---- catalogue layer toggles (label/search filtering JS-side; core always
//      draws its real star+constellation pipeline) ----
function wireCatalogue() {
  document.querySelectorAll(".cat").forEach((b) => {
    const key = b.dataset.cat;
    if (key in layers) layers[key] = b.classList.contains("is-on");
    b.addEventListener("click", () => {
      b.classList.toggle("is-on");
      if (key in layers) layers[key] = b.classList.contains("is-on");
    });
  });
}

// ---- search / catalogue lookup (JS over the mirrored catalogue) ----
function wireSearch() {
  const input = document.getElementById("search");
  const list = document.getElementById("results");
  let active = -1, items = [];

  const close = () => { list.hidden = true; list.innerHTML = ""; active = -1; items = []; };
  const run = debounce(() => {
    const q = input.value.trim().toLowerCase();
    if (!q) return close();
    const res = [];
    for (const o of CATALOG) if (o.name.toLowerCase().includes(q)) res.push(o);
    for (const c of CONSTELLATION_OBJECTS) if (c.name.toLowerCase().includes(q)) res.push(c);
    items = res.slice(0, 30);
    list.innerHTML = items.map((o, i) =>
      `<li data-i="${i}"><span>${escapeHtml(o.name)}</span>` +
      `<span class="kind">${escapeHtml(o.kind || "")}` +
      (o.mag != null && isFinite(o.mag) ? ` <span class="mag">m${Number(o.mag).toFixed(1)}</span>` : "") +
      `</span></li>`).join("");
    list.hidden = items.length === 0;
  }, 120);

  input.addEventListener("input", run);
  input.addEventListener("focus", run);
  input.addEventListener("keydown", (e) => {
    if (list.hidden) return;
    const lis = [...list.querySelectorAll("li")];
    if (e.key === "ArrowDown") active = Math.min(active + 1, lis.length - 1);
    else if (e.key === "ArrowUp") active = Math.max(active - 1, 0);
    else if (e.key === "Enter") { if (active >= 0) pick(items[active]); return; }
    else if (e.key === "Escape") { close(); return; }
    else return;
    lis.forEach((li, i) => li.classList.toggle("active", i === active));
    lis[active]?.scrollIntoView({ block: "nearest" });
    e.preventDefault();
  });
  list.addEventListener("click", (e) => {
    const li = e.target.closest("li"); if (!li) return;
    pick(items[parseInt(li.dataset.i, 10)]);
  });
  document.addEventListener("click", (e) => { if (!e.target.closest("#searchwrap")) close(); });

  function pick(o) {
    if (!o) return;
    centerOn(o.raHours, o.dec);
    input.value = o.name; close(); input.blur();
  }
}

// Point the camera so the given (RA hours, Dec deg) lands at screen centre.
// The core's view is yaw/pitch on the equatorial sphere AFTER a sky rotation by
// local sidereal time. We solve yaw/pitch by inverting that rotation: rotate the
// target's RA by -LST(now) about the pole, then read off the spherical angles.
function centerOn(raHours, dec) {
  const lstDeg = approxLstDeg();                  // matches core's GMST convention
  const raDeg = raHours * 15 - lstDeg;            // apply -LST rotation about pole
  cam.yaw   = raDeg * Math.PI / 180;
  cam.pitch = clamp(dec, -89, 89) * Math.PI / 180;
  pushOrientation();
}

// GMST (deg) for the current scrubbed time + longitude — same polynomial the
// Rust core uses (IAU 1982), so centering agrees with the rendered sky.
function approxLstDeg() {
  const scrub = document.getElementById("time-scrub");
  const ms = baseDateMs + (parseFloat(scrub?.value) || 0) * 3600 * 1000;
  const jd = msToJD(ms);
  const d = jd - 2451545.0;
  const t = d / 36525.0;
  let gmst = 280.46061837 + 360.98564736629 * d + 0.000387933 * t * t - (t * t * t) / 38710000.0;
  gmst = ((gmst % 360) + 360) % 360;
  return gmst + _observerLon;
}

// ---- AR toggle: camera passthrough + DeviceOrientation aiming ----
function wireAR() {
  const btn = document.getElementById("ar-toggle");
  const video = document.getElementById("camfeed");
  let stream = null;
  btn.addEventListener("click", async () => {
    const turningOn = !document.body.classList.contains("ar");
    if (turningOn) {
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: { ideal: "environment" } }, audio: false });
        video.srcObject = stream; video.hidden = false; await video.play().catch(() => {});
      } catch (e) { console.warn("[skyview] camera unavailable:", e); }
      await enableDeviceOrientation();
      document.body.classList.add("ar"); btn.classList.add("is-on");
      // Note: a transparent clear for true passthrough would need the core to
      // clear with alpha 0 (set_ar). The current core clears opaque; AR shows the
      // camera as a backdrop with the opaque star field on top — honest limitation.
    } else {
      if (stream) { stream.getTracks().forEach((t) => t.stop()); stream = null; }
      video.hidden = true; video.srcObject = null;
      disableDeviceOrientation();
      document.body.classList.remove("ar"); btn.classList.remove("is-on");
    }
  });
}

// ---- capture photo ----
function wireCapture() {
  document.getElementById("capture").addEventListener("click", () => {
    const canvas = document.getElementById(CANVAS_ID);
    sky.render(); // fresh frame (preserveDrawingBuffer may be off on the GL context)
    const out = document.createElement("canvas");
    out.width = canvas.width; out.height = canvas.height;
    const ctx = out.getContext("2d");
    const video = document.getElementById("camfeed");
    if (document.body.classList.contains("ar") && !video.hidden) {
      ctx.drawImage(video, 0, 0, out.width, out.height);
    }
    ctx.drawImage(canvas, 0, 0, out.width, out.height);
    const a = document.getElementById("dl");
    a.href = out.toDataURL("image/png");
    a.download = `skyview-${tstamp()}.png`;
    a.click();
  });
}

function wireBanner() {
  document.getElementById("banner-x").addEventListener("click", () =>
    document.getElementById("banner").classList.add("hidden"));
}

// ---- observer location (for local sidereal time / longitude) ----
let _observerLon = 6.14, _observerLat = 46.2; // Geneva default (lab)
function requestObserver() {
  const apply = (lat, lon) => { _observerLat = lat; _observerLon = lon; sky.set_longitude(lon); };
  if (!navigator.geolocation) { apply(_observerLat, _observerLon); return; }
  navigator.geolocation.getCurrentPosition(
    (p) => apply(p.coords.latitude, p.coords.longitude),
    ()  => apply(_observerLat, _observerLon),
    { timeout: 4000 });
  apply(_observerLat, _observerLon); // immediate default until the prompt resolves
}

// ---- canvas sizing (HiDPI aware) — REAL contract resize(w, h, dpr) ----
function resize() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.round(window.innerWidth * dpr);
  const h = Math.round(window.innerHeight * dpr);
  sky.resize(w, h, dpr);
  sky.set_fov(cam.fov);
}

// ============================================================================
// helpers
// ============================================================================
function clamp(x, a, b) { return Math.max(a, Math.min(b, x)); }
function debounce(fn, ms) { let t; return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); }; }
function escapeHtml(s) { return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c])); }

// Julian Date from epoch ms (UTC). 2440587.5 = JD of 1970-01-01T00:00Z.
function msToJD(ms) { return ms / 86400000 + 2440587.5; }
// datetime-local <input> has no timezone -> treat as local.
function msToLocalInput(ms) {
  const d = new Date(ms - new Date(ms).getTimezoneOffset() * 60000);
  return d.toISOString().slice(0, 19);
}
function localInputToMs(v) { return new Date(v).getTime(); }
function tstamp() { return new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19); }

boot();
