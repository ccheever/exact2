// The GPU module's page side (host code, LLP 1009 D2/D4): injected by glue.js
// after the first painted frame, only when the page has a canvas. It fetches
// the app's GPU wasm (wgpu on the browser's WebGPU, wasm-bindgen glue) and
// runs each canvas's surface: bind on new inputs, render while dirty or
// wanted, resize from the element's box and devicePixelRatio.
import init, * as gpu from "./gpu.js";

const exact = globalThis.exact;
const surfaces = new Map(); // view id -> surface, input listeners and journal cursor
const inputStyle = document.createElement("style");
inputStyle.textContent = "[data-gpu-input]:focus{outline:none}";
document.head.append(inputStyle);
let loaded = false;
let raf = null;
let finishReady;
const ready = new Promise((resolve) => { finishReady = resolve; });

function size(el) {
  const r = el.getBoundingClientRect();
  return { w: Math.max(r.width, 1), h: Math.max(r.height, 1), s: devicePixelRatio || 1 };
}

// exact.now() follows each batch `at` marker while surface commits are applied.
// The surfaces' clock: the page's in agent mode (LLP 1012: the driver owns
// time, and a picture is a function of it), else the frame's.
const clockFor = (frameNow) => exact.now?.() ?? frameNow;

function render(entry, now) {
  const { w, h, s } = size(entry.el);
  const pw = Math.max(1, Math.round(w * s)), ph = Math.max(1, Math.round(h * s));
  if (entry.el.width !== pw || entry.el.height !== ph) { entry.el.width = pw; entry.el.height = ph; }
  const r = gpu.gpu_render(entry.id, w, h, s, clockFor(now));
  if (r === 2) console.error("exact gpu:", gpu.gpu_error());
  if (r !== 2 && entry.firstFrameSubmittedMs === undefined) {
    entry.firstFrameSubmittedMs = performance.now();
    entry.inputMs = performance.getEntriesByName('exact-agent-input').at(-1)?.startTime ?? null;
    // A rendering opportunity after submission, not a GPU timestamp or scanout.
    requestAnimationFrame(() => requestAnimationFrame(() => { entry.firstFrameMs = performance.now(); }));
  }
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
  if (!gpu.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.())) console.error("exact gpu:", gpu.gpu_error());
  messages(entry);
  if (live(entry.view) !== entry) return;
  entry.wantsInput = gpu.gpu_wants_input(entry.id);
  if (entry.wantsInput) listen(entry);
  schedule();
}

function live(view) {
  const entry = surfaces.get(view);
  return entry?.id && exact.views.get(view) === entry.host && entry.el.isConnected ? entry : null;
}
function surfaceRecord(name, json) {
  exact.send(exact.wasm.exact_surface_record(exact.writeIn(json == null ? name : `${name}\0${json}`)));
}
function messages(entry) {
  const record = gpu.gpu_published(entry.id);
  if (record !== undefined && live(entry.view) === entry) surfaceRecord(entry.name, record);
  for (const text of JSON.parse(gpu.gpu_messages(entry.id))) {
    if (live(entry.view) !== entry) break;
    exact.message(entry.host, text);
  }
}
function listen(entry) {
  const el = entry.host, listeners = [];
  const previous = { touchAction: entry.el.style.touchAction, tabindex: el.getAttribute("tabindex") };
  entry.el.style.touchAction = "none";
  el.dataset.gpuInput = "";
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
    if (phase === "down") { el.focus({ preventScroll: true }); try { el.setPointerCapture(event.pointerId); } catch {} }
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
    entry.el.style.touchAction = previous.touchAction; delete el.dataset.gpuInput;
    if (previous.tabindex === null) el.removeAttribute("tabindex"); else el.setAttribute("tabindex", previous.tabindex);
  };
  queueMicrotask(() => { if (live(entry.view) === entry && (!document.activeElement || document.activeElement === document.body)) el.focus({ preventScroll: true }); });
}
function agent(view, request) {
  const entry = live(view);
  if (!entry) return null;
  const { w, h, s } = size(entry.host);
  const reply = gpu.gpu_agent(entry.id, JSON.stringify({ ...request, ...(exact.now ? { now: exact.now() } : {}), width: w, height: h, scale: s }));
  messages(entry);
  schedule();
  if (!reply) return null;
  try {
    const value = JSON.parse(reply);
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("world reply must be an object");
    return value;
  } catch (error) { console.error(`exact gpu: view ${view}:`, error); return null; }
}
function worlds(request) {
  const out = [];
  for (const view of surfaces.keys()) {
    const reply = agent(view, request);
    const world = reply?.world ?? (request.op === "clock" ? reply : null);
    if (world) {
      if (request.op === "state") {
        const entry = surfaces.get(view);
        world.perf = { ...world.perf, wallClock: true,
          navigationToFirstContentfulPaintMs: performance.getEntriesByName('first-contentful-paint')[0]?.startTime ?? null,
          gpuMs: Number(exact.root.dataset.gpuMs),
          inputMs: entry.inputMs ?? null,
          firstFrameSubmittedMs: entry.firstFrameSubmittedMs ?? null,
          firstFrameMs: entry.firstFrameMs ?? null,
          inputToFirstFrameMs: entry.inputMs != null && entry.firstFrameMs != null ? entry.firstFrameMs - entry.inputMs : null,
          firstFrameMeaning: 'rendering opportunity after GPU submission; not scanout',
          resources: performance.getEntriesByType('resource')
            .filter(r => /\/(?:app\.wasm|gpu(?:-glue)?\.js|gpu_bg\.wasm)$/.test(new URL(r.name).pathname))
            .map(r => ({ name: new URL(r.name).pathname, startMs: r.startTime, endMs: r.responseEnd, bytes: r.decodedBodySize })),
        };
      }
      out.push({ ...world, canvas: view });
    }
  }
  return out;
}

exact.gpu = {
  agent,
  settled: () => ready,
  wantsInput: (view) => live(view)?.wantsInput === true,
  answers: (request) => request.entity !== undefined || request.world === true,
  handle(request, ask, tagged) {
    const entry = live(request.id);
    if (!entry) return { error: `view ${request.id} has no world` };
    if (request.op === "focus") {
      if (!entry.wantsInput) return { error: `view ${request.id}'s surface does not take input` };
      entry.host.focus({ preventScroll: true }); return tagged({ ok: document.activeElement === entry.host });
    }
    if (!["layout", "state", "tree"].includes(request.op)) return { error: `world does not answer ${request.op}` };
    if (request.op === "layout" && request.entity === undefined) {
      const r = entry.host.getBoundingClientRect();
      request = { ...request, x: request.x - r.left, y: request.y - r.top };
    }
    const reply = agent(request.id, request) ?? { error: `view ${request.id} has no world` };
    const r = entry.host.getBoundingClientRect();
    for (const box of [reply.entity?.screen, reply.hit?.screen]) if (box) { box.x += r.left; box.y += r.top; }
    return tagged(reply);
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
    if (request.op === "logs") {
      const world = [];
      for (const [canvas, entry] of surfaces) {
        const journal = agent(canvas, { op: "logs", since: entry.logCursor });
        if (!journal || journal.error || !Array.isArray(journal.lines)) continue;
        const { from, next, lines } = journal;
        world.push({ canvas, from, next, lines, dropped: Math.max(0, from - entry.logCursor) });
        entry.logCursor = next;
      }
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
    if (!entry) { entry = { view, host, el, name, values, id: 0, wants: false, wantsInput: false, logCursor: 0 }; surfaces.set(view, entry); ensure(entry); return; }
    entry.values = values;
    if (entry.id) { if (!gpu.gpu_bind_at(entry.id, JSON.stringify(values), exact.now?.())) console.error("exact gpu:", gpu.gpu_error()); messages(entry); schedule(); }
  },
  destroy(view) {
    const entry = surfaces.get(view);
    if (entry?.id) gpu.gpu_destroy(entry.id);
    // The observer would fire once more as the element leaves the page, for
    // a surface the module no longer has (found by the agent smoke, which
    // is the first thing to navigate away from a canvas and back).
    if (entry) { entry.observer?.disconnect(); entry.unlisten?.(); entry.id = 0; }
    surfaces.delete(view);
    if (entry) surfaceRecord(entry.name, null);
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
  schedule() { for (const entry of surfaces.values()) if (entry.id && entry.wants) { entry.wants = false; gpu.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.()); } schedule(); },
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
} catch (error) { console.error("exact gpu:", error); }
if (loaded) exact.root.dataset.gpuMs = (performance.now() - t0).toFixed(1);
// A smoke run asks (`?smoke=1`) to be told when the module is up.
if (loaded && new URLSearchParams(location.search).get("smoke") === "1") navigator.sendBeacon(`/__gpu?ms=${exact.root.dataset.gpuMs}`);
try {
  for (const s of exact.pendingSurfaces ?? []) if (s.generation === exact.generation) exact.gpu.surface(s.id, s.name, s.values);
  exact.pendingSurfaces = [];
  for (const entry of surfaces.values()) ensure(entry);
} finally { finishReady(loaded); }
