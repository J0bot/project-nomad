/* tslint:disable */
/* eslint-disable */
export class SkyView {
  free(): void;
  /**
   * Total stars in the seed catalogue (REAL, bright stars only).
   */
  catalog_size(): number;
  /**
   * Project a single (RA hours, Dec deg) onto current normalized device
   * coords, returning [ndc_x, ndc_y, visible(0/1)]. The JS harness uses this
   * to place the reticule target, labels and the AR overlay without
   * re-implementing the astro math. visible=1 means in front of the camera.
   */
  project_radec(ra_hours: number, dec_deg: number): Float32Array;
  /**
   * Observer longitude in degrees, east positive. Affects local sidereal time.
   */
  set_longitude(longitude_deg: number): void;
  /**
   * Orient the camera. `yaw` and `pitch` in radians. Pitch is clamped to
   * avoid gimbal flip at the poles. Driven by mouse drag or deviceorientation.
   */
  set_orientation(yaw: number, pitch: number): void;
  /**
   * Number of star points currently rendered (after magnitude filtering).
   */
  visible_star_count(): number;
  /**
   * Limiting magnitude: only stars with mag <= limit are shown. Re-packs the
   * GPU buffer. Drives the magnitude slider.
   */
  set_magnitude_limit(m: number): void;
  /**
   * Build a SkyView bound to the canvas with the given id. Compiles shaders,
   * uploads the real catalogue, and prepares the render state. Call
   * `render()` from a requestAnimationFrame loop (driven by JS), or use the
   * convenience `start_loop()` below.
   */
  constructor(canvas_id: string);
  /**
   * Render one frame. Safe to call every animation frame from JS.
   */
  render(): void;
  /**
   * Set the canvas backing-store size (CSS pixels * devicePixelRatio).
   * Call on resize from JS. `dpr` is used for crisp point sizes.
   */
  resize(width: number, height: number, dpr: number): void;
  /**
   * Field of view in degrees (zoom). Clamped to a sane range.
   */
  set_fov(fov_deg: number): void;
  /**
   * Set the observation time as a full Julian Date (UTC). Scrubbing the
   * calendar in the harness maps date -> JD -> this call -> sky rotates.
   */
  set_time(jd: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_skyview_free: (a: number, b: number) => void;
  readonly skyview_catalog_size: (a: number) => number;
  readonly skyview_new: (a: number, b: number) => [number, number, number];
  readonly skyview_project_radec: (a: number, b: number, c: number) => [number, number];
  readonly skyview_render: (a: number) => void;
  readonly skyview_resize: (a: number, b: number, c: number, d: number) => void;
  readonly skyview_set_fov: (a: number, b: number) => void;
  readonly skyview_set_longitude: (a: number, b: number) => void;
  readonly skyview_set_magnitude_limit: (a: number, b: number) => [number, number];
  readonly skyview_set_orientation: (a: number, b: number, c: number) => void;
  readonly skyview_set_time: (a: number, b: number) => void;
  readonly skyview_visible_star_count: (a: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_2: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
