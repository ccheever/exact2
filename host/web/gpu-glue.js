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
  /// Time moved (the agent's `clock`): render what wants a frame, once.
  schedule() { for (const entry of surfaces.values()) if (entry.id && entry.wants) { entry.wants = false; gpu.gpu_bind(entry.id, JSON.stringify(entry.values)); } schedule(); },
};

const t0 = performance.now();
await init();
await gpu.gpu_load();
loaded = true;
exact.root.dataset.gpuMs = (performance.now() - t0).toFixed(1);
// A smoke run asks (`?smoke=1`) to be told when the module is up.
if (new URLSearchParams(location.search).get("smoke") === "1") navigator.sendBeacon(`/__gpu?ms=${exact.root.dataset.gpuMs}`);
for (const s of exact.pendingSurfaces ?? []) exact.gpu.surface(s.id, s.name, s.values);
exact.pendingSurfaces = [];
for (const entry of surfaces.values()) ensure(entry);
