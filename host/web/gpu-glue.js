// The GPU module's page side (host code, LLP 1009 D2/D4): injected by glue.js
// after the first painted frame, only when the page has a canvas. It fetches
// the app's GPU wasm (wgpu on the browser's WebGPU, wasm-bindgen glue) and
// runs each canvas's surface: bind on new inputs, render while dirty or
// wanted, resize from the element's box and devicePixelRatio.
import init, * as gpu from "./gpu.js";

const exact = globalThis.exact;
const surfaces = new Map(); // view id -> { el, name, values, id, wants }
let loaded = false;
let raf = null;
let finishReady, failReady;
const ready = new Promise((resolve, reject) => { finishReady = resolve; failReady = reject; });
ready.catch(() => {});

function size(el) {
  const r = el.getBoundingClientRect();
  return { w: Math.max(r.width, 1), h: Math.max(r.height, 1), s: devicePixelRatio || 1 };
}

// The surfaces' clock: the page's in agent mode (LLP 1012: the driver owns
// time, and a picture is a function of it), else the frame's.
const clockFor = (frameNow) => exact.now?.() ?? frameNow;

function render(entry, now) {
  const { w, h, s } = size(entry.el);
  const pw = Math.max(1, Math.round(w * s)), ph = Math.max(1, Math.round(h * s));
  if (entry.el.width !== pw || entry.el.height !== ph) { entry.el.width = pw; entry.el.height = ph; }
  const r = gpu.gpu_render(entry.id, w, h, s, clockFor(now));
  if (r === 2) console.error("exact gpu:", gpu.gpu_error());
  entry.wants = r === 1;
}

function frame(now) {
  raf = null;
  let more = false;
  for (const entry of surfaces.values()) {
    if (!entry.id) continue;
    if (entry.wants || gpu.gpu_dirty(entry.id)) render(entry, now);
    more ||= entry.wants;
  }
  // Under the agent's clock a frame is asked for by `clock`, never by the
  // last frame: a surface that wants more renders again when time moves.
  if (more && !exact.now) schedule();
}

function schedule() { if (raf === null) raf = requestAnimationFrame(frame); }

function ensure(entry) {
  if (entry.id || !loaded) return;
  const { w, h, s } = size(entry.el);
  entry.el.width = Math.max(1, Math.round(w * s));
  entry.el.height = Math.max(1, Math.round(h * s));
  entry.id = gpu.gpu_create(entry.name, entry.el, entry.el.width, entry.el.height);
  if (!entry.id) { console.error("exact gpu:", gpu.gpu_error()); return; }
  entry.observer = new ResizeObserver(() => { if (entry.id) { render(entry, performance.now()); } });
  // (`render` takes the agent's clock over that timestamp in agent mode.)
  entry.observer.observe(entry.el);
  if (!gpu.gpu_bind(entry.id, JSON.stringify(entry.values))) console.error("exact gpu:", gpu.gpu_error());
  schedule();
}

exact.gpu = {
  surface(view, name, values) {
    // The node's element hosts its surface <canvas> (glue.js, LLP 1014 D2).
    const host = exact.views.get(view);
    const el = host?.matches("canvas") ? host : host?.querySelector(":scope > canvas[data-surface]");
    if (!el) return;
    let entry = surfaces.get(view);
    if (entry && entry.el !== el) { this.destroy(view); entry = null; } // a reload reuses ids
    if (entry && entry.name !== name) { this.destroy(view); entry = null; } // one id cannot retain another plan's surface
    if (!entry) { entry = { el, name, values, id: 0, wants: false }; surfaces.set(view, entry); ensure(entry); return; }
    entry.values = values;
    if (entry.id) { if (!gpu.gpu_bind(entry.id, JSON.stringify(values))) console.error("exact gpu:", gpu.gpu_error()); schedule(); }
  },
  destroy(view) {
    const entry = surfaces.get(view);
    if (entry?.id) gpu.gpu_destroy(entry.id);
    // The observer would fire once more as the element leaves the page, for
    // a surface the module no longer has (found by the agent smoke, which
    // is the first thing to navigate away from a canvas and back).
    if (entry) { entry.observer?.disconnect(); entry.id = 0; }
    surfaces.delete(view);
  },
  /// A restart: every surface goes with its element.
  reset() { for (const view of [...surfaces.keys()]) this.destroy(view); },
  /// A shader's text (LLP 1030 D8) — the dev loop's edit, or the first
  /// registration: validated, its interface checked against the module's;
  /// every surface renders again through the new pipeline. False, with the
  /// reason on the console, when the module refuses it.
  shader(name, text) {
    if (!loaded) return false;
    const ok = gpu.gpu_shader(name, text);
    if (!ok) console.error("exact gpu:", gpu.gpu_error()); else schedule();
    return ok;
  },
  // Registration is checked before the host commits. Replacement then runs
  // synchronously in the same turn, including clearing omitted names.
  async prepareShaders(assets) {
    await ready;
    const rows = shaderRows(assets);
    for (const [name, text] of rows) if (!await gpu.gpu_shader_check(name, text)) throw new Error(gpu.gpu_error());
    return () => replaceShaders(rows);
  },
  /// Time moved (the agent's `clock`): render what wants a frame, once.
  schedule() { for (const entry of surfaces.values()) if (entry.id && entry.wants) { entry.wants = false; gpu.gpu_bind(entry.id, JSON.stringify(entry.values)); } schedule(); },
};

function shaderRows(assets) {
  const names = new Set(JSON.parse(gpu.gpu_shader_names())), decoder = new TextDecoder("utf-8", { fatal: true });
  return [...assets].filter(([path]) => path.startsWith("shaders/") && path.endsWith(".wgsl"))
    .map(([path, card]) => [path.slice(8, -5), decoder.decode(card.bytes)])
    .filter(([name]) => names.has(name));
}
function replaceShaders(rows) {
  gpu.gpu_shaders_clear();
  for (const [name, text] of rows) if (!gpu.gpu_shader(name, text)) throw new Error(gpu.gpu_error());
}
const t0 = performance.now();
try {
  await init();
  await gpu.gpu_load();
  const rows = [];
  if (exact.devAssets === null) for (const name of JSON.parse(gpu.gpu_shader_names())) {
    const r = await fetch(new URL(`./shaders/${name}.wgsl`, import.meta.url));
    if (!r.ok) { console.error("exact gpu:", `shaders/${name}.wgsl: HTTP ${r.status}`); continue; }
    rows.push([name, await r.text()]);
  }
  // A generation may have committed while the module or baked shaders
  // downloaded. Its pinned complete namespace wins before any surface exists.
  replaceShaders(exact.devAssets === null ? rows : shaderRows(exact.devAssets));
  loaded = true;
  finishReady();
} catch (error) { failReady(error); throw error; }
exact.root.dataset.gpuMs = (performance.now() - t0).toFixed(1);
// A smoke run asks (`?smoke=1`) to be told when the module is up.
if (new URLSearchParams(location.search).get("smoke") === "1") navigator.sendBeacon(`/__gpu?ms=${exact.root.dataset.gpuMs}`);
for (const s of exact.pendingSurfaces ?? []) if (s.generation === exact.generation) exact.gpu.surface(s.id, s.name, s.values);
exact.pendingSurfaces = [];
for (const entry of surfaces.values()) ensure(entry);
