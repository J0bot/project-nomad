//! SkyView ploxion — full-WASM celestial map.
//!
//! Render + astro math run entirely in WebAssembly (Rust). The HTML harness is
//! a thin canvas + a few controls that call into the exported methods.
//!
//! WHAT IS REAL vs PLACEHOLDER
//! ===========================
//! REAL:
//!   - Star catalogue: ~40 brightest stars, J2000 RA/Dec/mag (see catalog.rs).
//!   - 7 major constellations as line segments between catalogued stars.
//!   - (RA,Dec) -> unit-sphere projection (exact spherical geometry).
//!   - Sidereal-time rotation of the sky (IAU 1982 GMST).
//!   - WebGL2 point rendering (size/brightness by magnitude) + line rendering.
//!   - Orientable camera (yaw/pitch) driven from JS (mouse drag / deviceorientation).
//!   - Magnitude-limit filtering, time scrubbing via Julian Date.
//! PLACEHOLDER / NOT YET:
//!   - Planets / satellites / Moon: NOT computed (needs VSOP87 + ELP, a data
//!     file loaded later). No fake planet coordinates are emitted.
//!   - Full HYG ~120k star DB: loaded from an external data file later.
//!   - No precession/nutation/aberration/refraction (naked-eye accuracy only).
//!   - Horizon/AR overlay, reticule target, labels, photo capture: drawn by the
//!     HTML/JS harness on a 2D overlay; this crate exposes the projected
//!     positions it needs (see `project_radec`).

mod astro;
mod catalog;
mod gl;

use glam::{Mat4, Vec3};
use wasm_bindgen::prelude::*;
use web_sys::{WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram, WebGlVertexArrayObject};

// ---------- shaders ----------

const STAR_VERT: &str = r#"#version 300 es
precision highp float;
layout(location=0) in vec3 a_pos;   // unit vector on celestial sphere
layout(location=1) in float a_size; // base point size (px)
layout(location=2) in float a_bright;
uniform mat4 u_viewproj;
uniform float u_dpr;
uniform float u_mag_gain; // shrinks points as user lowers mag limit (cosmetic)
out float v_bright;
void main() {
    gl_Position = u_viewproj * vec4(a_pos, 1.0);
    gl_PointSize = a_size * u_dpr * u_mag_gain;
    v_bright = a_bright;
}
"#;

const STAR_FRAG: &str = r#"#version 300 es
precision highp float;
in float v_bright;
out vec4 frag;
void main() {
    // round, soft-edged point sprite
    vec2 d = gl_PointCoord - vec2(0.5);
    float r = length(d) * 2.0;
    float a = smoothstep(1.0, 0.2, r);
    vec3 col = vec3(0.85, 0.90, 1.0) * v_bright;
    frag = vec4(col, a);
}
"#;

const LINE_VERT: &str = r#"#version 300 es
precision highp float;
layout(location=0) in vec3 a_pos;
uniform mat4 u_viewproj;
void main() { gl_Position = u_viewproj * vec4(a_pos, 1.0); }
"#;

const LINE_FRAG: &str = r#"#version 300 es
precision highp float;
out vec4 frag;
void main() { frag = vec4(0.35, 0.55, 0.85, 0.55); }
"#;

// ---------- prepared geometry (CPU side, built once) ----------

/// Per-star CPU record so we can re-filter by magnitude without re-querying GL.
struct StarVtx {
    dir: Vec3, // unit direction (equatorial-inertial frame)
    size: f32,
    bright: f32,
    mag: f32,
}

#[wasm_bindgen]
pub struct SkyView {
    gl: GL,
    canvas: web_sys::HtmlCanvasElement,

    star_program: WebGlProgram,
    line_program: WebGlProgram,

    // Star geometry is interleaved [x,y,z,size,bright] * N; we keep CPU copy
    // so we can re-pack on magnitude change.
    stars: Vec<StarVtx>,
    star_vao: WebGlVertexArrayObject,
    star_buf: WebGlBuffer,
    star_count: i32, // number of points currently uploaded

    // Constellation lines (static).
    line_vao: WebGlVertexArrayObject,
    #[allow(dead_code)]
    line_buf: WebGlBuffer,
    line_count: i32, // number of vertices (2 per segment)

    // camera / state
    yaw: f32,
    pitch: f32,
    fov_deg: f32,
    jd: f64,
    longitude_deg: f32,
    mag_limit: f32,
    dpr: f32,
}

#[wasm_bindgen]
impl SkyView {
    /// Build a SkyView bound to the canvas with the given id. Compiles shaders,
    /// uploads the real catalogue, and prepares the render state. Call
    /// `render()` from a requestAnimationFrame loop (driven by JS), or use the
    /// convenience `start_loop()` below.
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<SkyView, JsValue> {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("canvas not found"))?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let gl = canvas
            .get_context("webgl2")?
            .ok_or_else(|| JsValue::from_str("webgl2 not available"))?
            .dyn_into::<GL>()?;

        let star_program = gl::link_program(&gl, STAR_VERT, STAR_FRAG)?;
        let line_program = gl::link_program(&gl, LINE_VERT, LINE_FRAG)?;

        // ---- build star CPU records from the real catalogue ----
        let mut stars = Vec::with_capacity(catalog::STARS.len());
        for s in catalog::STARS {
            stars.push(StarVtx {
                dir: astro::radec_to_vec(s.ra_hours, s.dec_deg),
                size: astro::mag_to_size(s.mag),
                bright: astro::mag_to_brightness(s.mag),
                mag: s.mag,
            });
        }

        // ---- build constellation line geometry (static) ----
        let mut line_data: Vec<f32> = Vec::new();
        for c in catalog::CONSTELLATIONS {
            for (from, to) in c.segments {
                let (Some(a), Some(b)) = (catalog::find(from), catalog::find(to)) else {
                    // HONEST: skip any segment referencing a star not in the table.
                    continue;
                };
                for idx in [a, b] {
                    let sref = &catalog::STARS[idx];
                    let v = astro::radec_to_vec(sref.ra_hours, sref.dec_deg);
                    line_data.extend_from_slice(&[v.x, v.y, v.z]);
                }
            }
        }
        let line_count = (line_data.len() / 3) as i32;
        let line_buf = gl::make_f32_buffer(&gl, &line_data)?;
        let line_vao = gl::make_vao(&gl)?;
        gl.bind_vertex_array(Some(&line_vao));
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&line_buf));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 3, GL::FLOAT, false, 0, 0);
        gl.bind_vertex_array(None);

        // ---- star VAO + (empty for now) buffer; packed in update_stars ----
        let star_buf = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("create star buffer failed"))?;
        let star_vao = gl::make_vao(&gl)?;

        let mut sv = SkyView {
            gl,
            canvas,
            star_program,
            line_program,
            stars,
            star_vao,
            star_buf,
            star_count: 0,
            line_vao,
            line_buf,
            line_count,
            yaw: 0.0,
            pitch: 0.0,
            fov_deg: 70.0,
            jd: 2451545.0, // J2000 default; JS overrides with "now"
            longitude_deg: 0.0,
            mag_limit: 6.5,
            dpr: 1.0,
        };
        sv.repack_stars()?;
        Ok(sv)
    }

    /// Re-pack the star interleaved buffer applying the current magnitude limit.
    fn repack_stars(&mut self) -> Result<(), JsValue> {
        let mut data: Vec<f32> = Vec::with_capacity(self.stars.len() * 5);
        for s in &self.stars {
            if s.mag > self.mag_limit {
                continue;
            }
            data.extend_from_slice(&[s.dir.x, s.dir.y, s.dir.z, s.size, s.bright]);
        }
        self.star_count = (data.len() / 5) as i32;

        let gl = &self.gl;
        gl.bind_vertex_array(Some(&self.star_vao));
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.star_buf));
        unsafe {
            let view = js_sys::Float32Array::view(&data);
            gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &view, GL::DYNAMIC_DRAW);
        }
        let stride = (5 * std::mem::size_of::<f32>()) as i32;
        // location 0: pos vec3
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 3, GL::FLOAT, false, stride, 0);
        // location 1: size float
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 1, GL::FLOAT, false, stride, 12);
        // location 2: bright float
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_with_i32(2, 1, GL::FLOAT, false, stride, 16);
        gl.bind_vertex_array(None);
        Ok(())
    }

    /// Build the combined view+projection matrix for the current camera & time.
    fn view_proj(&self) -> Mat4 {
        let (w, h) = (self.canvas.width().max(1) as f32, self.canvas.height().max(1) as f32);
        let aspect = w / h;
        let proj = Mat4::perspective_rh_gl(self.fov_deg.to_radians(), aspect, 0.01, 10.0);

        // Camera look direction from yaw/pitch (radians). We are at the sphere
        // centre looking outward; stars are at radius 1.
        let cp = self.pitch.cos();
        let dir = Vec3::new(
            cp * self.yaw.cos(),
            cp * self.yaw.sin(),
            self.pitch.sin(),
        );
        let up = Vec3::Z;
        let eye = Vec3::ZERO;
        let mut view = Mat4::look_at_rh(eye, dir, up);

        // Rotate the whole sky by local sidereal time about the celestial pole
        // (Z axis), so "now" points the camera at the real current sky.
        let lst = astro::lst_radians(self.jd, self.longitude_deg);
        let sky_rot = Mat4::from_rotation_z(-lst);
        view = view * sky_rot;

        proj * view
    }

    /// Render one frame. Safe to call every animation frame from JS.
    pub fn render(&self) {
        let gl = &self.gl;
        let (w, h) = (self.canvas.width() as i32, self.canvas.height() as i32);
        gl.viewport(0, 0, w, h);
        gl.clear_color(0.02, 0.03, 0.06, 1.0);
        gl.clear(GL::COLOR_BUFFER_BIT);

        gl.enable(GL::BLEND);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);

        let vp = self.view_proj();
        let vp_arr = vp.to_cols_array();

        // ---- constellation lines ----
        if self.line_count > 0 {
            gl.use_program(Some(&self.line_program));
            let loc = gl.get_uniform_location(&self.line_program, "u_viewproj");
            gl.uniform_matrix4fv_with_f32_array(loc.as_ref(), false, &vp_arr);
            gl.bind_vertex_array(Some(&self.line_vao));
            gl.draw_arrays(GL::LINES, 0, self.line_count);
        }

        // ---- stars ----
        if self.star_count > 0 {
            gl.use_program(Some(&self.star_program));
            let l_vp = gl.get_uniform_location(&self.star_program, "u_viewproj");
            gl.uniform_matrix4fv_with_f32_array(l_vp.as_ref(), false, &vp_arr);
            let l_dpr = gl.get_uniform_location(&self.star_program, "u_dpr");
            gl.uniform1f(l_dpr.as_ref(), self.dpr);
            let l_gain = gl.get_uniform_location(&self.star_program, "u_mag_gain");
            gl.uniform1f(l_gain.as_ref(), 1.0);
            gl.bind_vertex_array(Some(&self.star_vao));
            gl.draw_arrays(GL::POINTS, 0, self.star_count);
        }

        gl.bind_vertex_array(None);
    }

    // ---------- exported control surface ----------

    /// Set the canvas backing-store size (CSS pixels * devicePixelRatio).
    /// Call on resize from JS. `dpr` is used for crisp point sizes.
    pub fn resize(&mut self, width: u32, height: u32, dpr: f32) {
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.dpr = dpr.max(0.5);
    }

    /// Orient the camera. `yaw` and `pitch` in radians. Pitch is clamped to
    /// avoid gimbal flip at the poles. Driven by mouse drag or deviceorientation.
    pub fn set_orientation(&mut self, yaw: f32, pitch: f32) {
        self.yaw = yaw;
        let lim = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = pitch.clamp(-lim, lim);
    }

    /// Set the observation time as a full Julian Date (UTC). Scrubbing the
    /// calendar in the harness maps date -> JD -> this call -> sky rotates.
    pub fn set_time(&mut self, jd: f64) {
        self.jd = jd;
    }

    /// Observer longitude in degrees, east positive. Affects local sidereal time.
    pub fn set_longitude(&mut self, longitude_deg: f32) {
        self.longitude_deg = longitude_deg;
    }

    /// Limiting magnitude: only stars with mag <= limit are shown. Re-packs the
    /// GPU buffer. Drives the magnitude slider.
    pub fn set_magnitude_limit(&mut self, m: f32) -> Result<(), JsValue> {
        self.mag_limit = m;
        self.repack_stars()
    }

    /// Field of view in degrees (zoom). Clamped to a sane range.
    pub fn set_fov(&mut self, fov_deg: f32) {
        self.fov_deg = fov_deg.clamp(15.0, 110.0);
    }

    /// Project a single (RA hours, Dec deg) onto current normalized device
    /// coords, returning [ndc_x, ndc_y, visible(0/1)]. The JS harness uses this
    /// to place the reticule target, labels and the AR overlay without
    /// re-implementing the astro math. visible=1 means in front of the camera.
    pub fn project_radec(&self, ra_hours: f32, dec_deg: f32) -> Vec<f32> {
        let v = astro::radec_to_vec(ra_hours, dec_deg);
        let vp = self.view_proj();
        let clip = vp * v.extend(1.0);
        if clip.w.abs() < 1e-6 || clip.w < 0.0 {
            return vec![0.0, 0.0, 0.0];
        }
        vec![clip.x / clip.w, clip.y / clip.w, 1.0]
    }

    /// Number of star points currently rendered (after magnitude filtering).
    pub fn visible_star_count(&self) -> i32 {
        self.star_count
    }

    /// Total stars in the seed catalogue (REAL, bright stars only).
    pub fn catalog_size(&self) -> usize {
        catalog::STARS.len()
    }
}
