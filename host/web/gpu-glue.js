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
let finishReady;
const ready = new Promise((resolve) => { finishReady = resolve; });

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
  messages(entry);
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
  if (gpu.gpu_wants_input(entry.id)) listen(entry);
  schedule();
}

function live(view) {
  const entry = surfaces.get(view);
  return entry?.id && exact.views.get(view) === entry.host && entry.el.isConnected ? entry : null;
}
function messages(entry) {
  for (const text of JSON.parse(gpu.gpu_messages(entry.id))) {
    if (live(entry.view) !== entry) break;
    exact.message(entry.host, text);
  }
}
function listen(entry) {
  const el = entry.host, listeners = [];
  const previous = { touchAction: el.style.touchAction, outline: el.style.outline, tabindex: el.getAttribute("tabindex") };
  el.style.touchAction = "none";
  el.style.outline = "none";
  if (el.tabIndex < 0) el.tabIndex = 0;
  const on = (name, fn, options) => { el.addEventListener(name, fn, options); listeners.push([name, fn, options]); };
  const send = (event, value) => {
    if (live(entry.view) !== entry) return;
    if (!gpu.gpu_input(entry.id, JSON.stringify({ ...value, at: exact.now?.() ?? event.timeStamp }))) console.error("exact gpu:", gpu.gpu_error());
    messages(entry);
    schedule();
  };
  const fallsThrough = (event) => event.target === el || event.target === entry.el;
  const point = (event) => { const r = el.getBoundingClientRect(); return { x: event.clientX - r.left, y: event.clientY - r.top }; };
  for (const phase of ["down", "move", "up", "cancel"]) on(`pointer${phase}`, (event) => {
    if (!fallsThrough(event)) return;
    if (phase === "down") { el.focus(); el.setPointerCapture(event.pointerId); }
    send(event, { t: "pointer", phase, id: event.pointerId, ...point(event), kind: event.pointerType || "mouse", buttons: event.buttons });
  });
  on("wheel", (event) => {
    if (!fallsThrough(event)) return;
    event.preventDefault();
    send(event, { t: "wheel", dx: event.deltaX, dy: event.deltaY, ...point(event) });
  }, { passive: false });
  for (const name of ["keydown", "keyup"]) on(name, (event) => {
    if (event.target !== el) return;
    if (!event.metaKey && !event.ctrlKey && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Space", "PageUp", "PageDown", "Home", "End"].includes(event.code)) event.preventDefault();
    send(event, { t: "key", code: event.code, key: event.key, down: name === "keydown", repeat: event.repeat });
  });
  on("blur", (event) => send(event, { t: "blur" }));
  entry.unlisten = () => {
    for (const [name, fn, options] of listeners) el.removeEventListener(name, fn, options);
    el.style.touchAction = previous.touchAction; el.style.outline = previous.outline;
    if (previous.tabindex === null) el.removeAttribute("tabindex"); else el.setAttribute("tabindex", previous.tabindex);
  };
}
function agent(view, request) {
  const entry = live(view);
  if (!entry) return null;
  const { w, h, s } = size(entry.host);
  const reply = gpu.gpu_agent(entry.id, JSON.stringify({ ...request, now: clockFor(performance.now()), width: w, height: h, scale: s }));
  messages(entry);
  schedule();
  if (!reply) return null;
  const value = JSON.parse(reply);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`view ${view}: world reply must be an object`);
  return value;
}
function worlds(request) {
  const out = [];
  for (const view of surfaces.keys()) {
    const reply = agent(view, request);
    const world = reply?.world ?? (request.op === "clock" ? reply : null);
    if (world) out.push({ ...world, canvas: view });
  }
  return out;
}

exact.gpu = {
  agent,
  answers: (request) => request.entity !== undefined || request.world === true,
  handle(request, ask, tagged) {
    const entry = live(request.id);
    if (!entry) return { error: `view ${request.id} has no world` };
    if (request.op === "focus") { entry.host.focus(); return tagged({ ok: document.activeElement === entry.host }); }
    if (!["layout", "state", "tree", "pick"].includes(request.op)) return { error: `world does not answer ${request.op}` };
    if (request.op === "pick") {
      const r = entry.host.getBoundingClientRect();
      request = { ...request, x: request.x - r.left, y: request.y - r.top };
    }
    return tagged(agent(request.id, request) ?? { error: `view ${request.id} has no world` });
  },
  decorate(request, reply) {
    if (reply?.then) return reply.then((r) => exact.gpu.decorate(request, r));
    if (!reply || reply.error || exact.gpu.answers(request)) return reply;
    if (request.op === "tree") for (const node of reply.nodes ?? []) {
      const summary = agent(node.id, { op: "tree", summary: true });
      if (summary?.world) node.world = summary.world;
    }
    if (request.op === "state") {
      const world = worlds({ op: "state" });
      if (world.length) reply.world = world;
    }
    return reply;
  },
  // The existing clock loop owns the 16-round bound; this is its next candidate.
  clock(settle) {
    const world = worlds({ op: "clock", settle });
    const pending = settle && world.some((w) => w.quiescent === false);
    const candidates = world.filter((w) => w.quiescent === false && Number.isFinite(w.settleAt)).map((w) => w.settleAt);
    return { pending, settleAt: candidates.length ? Math.max(...candidates) : undefined,
      reply: world.length ? { world: world.map(({ canvas, tick, hash, quiescent }) => ({ canvas, tick, hash, quiescent })) } : {} };
  },
  surface(view, name, values) {
    // The node's element hosts its surface <canvas> (glue.js, LLP 1014 D2).
    const host = exact.views.get(view);
    const el = host?.matches("canvas") ? host : host?.querySelector(":scope > canvas[data-surface]");
    if (!el) return;
    let entry = surfaces.get(view);
    if (entry && entry.el !== el) { this.destroy(view); entry = null; } // a reload reuses ids
    if (entry && entry.name !== name) { this.destroy(view); entry = null; } // one id cannot retain another plan's surface
    if (!entry) { entry = { view, host, el, name, values, id: 0, wants: false }; surfaces.set(view, entry); ensure(entry); return; }
    entry.values = values;
    if (entry.id) { if (!gpu.gpu_bind(entry.id, JSON.stringify(values))) console.error("exact gpu:", gpu.gpu_error()); schedule(); }
  },
  destroy(view) {
    const entry = surfaces.get(view);
    if (entry?.id) gpu.gpu_destroy(entry.id);
    // The observer would fire once more as the element leaves the page, for
    // a surface the module no longer has (found by the agent smoke, which
    // is the first thing to navigate away from a canvas and back).
    if (entry) { entry.observer?.disconnect(); entry.unlisten?.(); entry.id = 0; }
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
    if (!await ready) return () => {}; // An unavailable optional device cannot block core plans.
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
  if (exact.now) gpu.gpu_seekable(true);
  loaded = true;
  const rows = [];
  if (exact.devAssets === null) for (const name of JSON.parse(gpu.gpu_shader_names())) {
    const r = await fetch(new URL(`./shaders/${name}.wgsl`, import.meta.url));
    if (!r.ok) { console.error("exact gpu:", `shaders/${name}.wgsl: HTTP ${r.status}`); continue; }
    rows.push([name, await r.text()]);
  }
  // A generation may have committed while the module or baked shaders
  // downloaded. Its pinned complete namespace wins before any surface exists.
  replaceShaders(exact.devAssets === null ? rows : shaderRows(exact.devAssets));
} catch (error) { console.error("exact gpu:", error); }
finally { finishReady(loaded); }
if (loaded) exact.root.dataset.gpuMs = (performance.now() - t0).toFixed(1);
// A smoke run asks (`?smoke=1`) to be told when the module is up.
if (loaded && new URLSearchParams(location.search).get("smoke") === "1") navigator.sendBeacon(`/__gpu?ms=${exact.root.dataset.gpuMs}`);
for (const s of exact.pendingSurfaces ?? []) if (s.generation === exact.generation) exact.gpu.surface(s.id, s.name, s.values);
exact.pendingSurfaces = [];
for (const entry of surfaces.values()) ensure(entry);
