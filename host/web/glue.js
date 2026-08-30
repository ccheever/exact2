// The web host's glue: apply batches, forward events, tick the clock.
//
// @ref LLP 1007 §3. This is host code, not app code: it knows nothing about
// the app. The app is the wasm (runner + kernel + data crate + baked plan).
// Nothing here runs per frame; layout and motion are the browser's.

const root = document.getElementById("exact-root");
const views = new Map(); // view id -> element
const animations = new Map(); // "view/property" -> Animation (a spring in flight)
let wasm = null;
let memory = null;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const t0 = performance.now();
// Agent mode (`?agent=1`, LLP 1012): the driver owns the clock. Time is the
// last `clock` operation's value — events carry it, no ticker runs, and the
// browser's animations are seeked to it, never played.
const agentMode = new URL(location.href).searchParams.has("agent");
let agentClock = agentMode ? 0 : null;
const starts = new WeakMap(); // Animation -> the agent clock when it began
const now = () => agentClock ?? performance.now() - t0;

function readOut(len) {
  const ptr = wasm.exact_out();
  return decoder.decode(new Uint8Array(memory.buffer, ptr, len));
}

function writeIn(text) {
  const bytes = encoder.encode(text);
  const ptr = wasm.exact_in(bytes.length);
  new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
  return bytes.length;
}

function applyProps(el, set, clear) {
  for (const name of clear || []) {
    if (name === "text") el.textContent = "";
    else if (name === "value") el.value = "";
    else if (name === "checked") el.checked = false;
    else el.removeAttribute(name);
  }
  for (const [name, value] of Object.entries(set || {})) {
    if (name === "text") {
      if (el.childElementCount === 0) el.textContent = value;
    } else if (name === "value") {
      if (el.value !== value) el.value = value;
    } else if (name === "checked") {
      el.checked = value === "true";
    } else if (name === "disabled") {
      if (value === "true") el.setAttribute("disabled", ""); else el.removeAttribute("disabled");
    } else {
      el.setAttribute(name, value);
    }
  }
}

function attach(el, id, handlers) {
  el.dataset.view = String(id);
  // A node with focus, blur, or key handlers can take the focus (an input
  // or a button does by itself): the web's rule that only a focusable
  // element hears these.
  if (handlers.some((k) => k === "focus" || k === "blur" || k === "key") && !(el instanceof HTMLInputElement || el instanceof HTMLButtonElement) && !el.hasAttribute("tabindex")) el.tabIndex = 0;
  for (const kind of handlers) {
    if (kind === "press") {
      el.addEventListener("click", (e) => { e.stopPropagation(); send(wasm.exact_dispatch(id, 0, 0, now())); });
    } else if (kind === "change") {
      el.addEventListener("input", () => { const n = writeIn(el.value); send(wasm.exact_dispatch(id, 1, n, now())); });
    } else if (kind === "hover") {
      // pointerenter/pointerleave: the element's own, not a bubbling mouseover.
      el.addEventListener("pointerenter", () => send(wasm.exact_dispatch(id, 2, 0, now())));
      el.addEventListener("pointerleave", () => send(wasm.exact_dispatch(id, 3, 0, now())));
    } else if (kind === "focus") {
      el.addEventListener("focus", () => send(wasm.exact_dispatch(id, 4, 0, now())));
    } else if (kind === "blur") {
      el.addEventListener("blur", () => send(wasm.exact_dispatch(id, 5, 0, now())));
    } else if (kind === "key") {
      // keydown, the key's name as the web spells it (`e.key`).
      el.addEventListener("keydown", (e) => { const n = writeIn(e.key); send(wasm.exact_dispatch(id, 6, n, now())); });
    }
  }
}

function apply(batch) {
  if (batch.error) console.error("exact:", batch.error);
  for (const op of batch.ops) {
    switch (op.op) {
      case "create": {
        // A canvas node is a <div> hosting its surface <canvas> under its
        // children (LLP 1014 D2): the kernel's children are laid out in the
        // box, in flow, over the surface — a bare <canvas>'s children would
        // be fallback content, never rendered. The surface element is the
        // host's, never a child the kernel knows (`data-surface`).
        const el = document.createElement(op.tag === "canvas" ? "div" : op.tag);
        if (op.tag === "canvas") {
          const surface = document.createElement("canvas");
          surface.dataset.surface = "";
          surface.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1";
          el.append(surface);
        }
        applyProps(el, op.props, []);
        el.style.cssText = op.css;
        attach(el, op.id, op.handlers);
        views.set(op.id, el);
        break;
      }
      case "props": applyProps(views.get(op.id), op.set, op.clear); break;
      case "style": views.get(op.id).style.cssText = op.css; break;
      case "children": {
        const el = views.get(op.id);
        const want = op.ids.map((i) => views.get(i)).filter(Boolean);
        // Reorder in place: keyed rows keep their elements (and their state).
        // A canvas's surface element is skipped: not a child, never removed.
        const skip = (n) => { while (n && n.dataset.surface !== undefined) n = n.nextElementSibling; return n; };
        let cursor = skip(el.firstElementChild);
        for (const child of want) {
          if (child === cursor) { cursor = skip(cursor.nextElementSibling); continue; }
          el.insertBefore(child, cursor);
        }
        while (cursor) { const next = skip(cursor.nextElementSibling); cursor.remove(); cursor = next; }
        break;
      }
      case "animate": {
        // A spring: frames from the engine, played by the browser with linear
        // interpolation (LLP 1002 D2). Replaces the spring on that property.
        const key = op.id + "/" + op.property;
        animations.get(key)?.cancel();
        animations.delete(key);
        if (!op.values.length) break;
        const css = (v) => op.property === "translate" ? `${v[0]}px ${v[1]}px` : op.property === "rotate" ? `${v}deg` : String(v);
        const anim = views.get(op.id).animate(op.values.map((v) => ({ [op.property]: css(v) })), { delay: op.delay, duration: op.duration, easing: "linear" });
        animations.set(key, anim);
        anim.finished.then(() => { if (animations.get(key) === anim) animations.delete(key); }, () => {});
        break;
      }
      case "surface": {
        // A canvas's inputs (LLP 1009 D2): to the GPU module when it is
        // loaded, queued until then. The module itself is fetched only
        // after the first painted frame, and only when a canvas exists.
        if (globalThis.exact.gpu) globalThis.exact.gpu.surface(op.id, op.name, op.values);
        else (globalThis.exact.pendingSurfaces ??= []).push({ id: op.id, name: op.name, values: op.values });
        break;
      }
      case "command": {
        // A capability an action called (LLP 1005 §3). `setScheme` is the
        // document's colour scheme — what `prefers-color-scheme` would be.
        if (op.name === "setScheme") document.documentElement.style.colorScheme = String(op.args[0] ?? "");
        else console.warn(`exact: unknown command ${op.name}`);
        break;
      }
      case "destroy": { const el = views.get(op.id); if (el) el.remove(); views.delete(op.id); globalThis.exact.gpu?.destroy(op.id); break; }
      case "roots": {
        root.replaceChildren(...op.ids.map((i) => views.get(i)).filter(Boolean));
        break;
      }
      case "at": {
        // The ops that follow were committed at this clock (a timer's due
        // time inside one advance): what the ops before it started belongs
        // to the clock so far; the animations are then seeked to this
        // instant before the next ops see them (LLP 1012; LLP 1002 D3).
        if (agentMode) { register(agentClock); agentClock = Math.max(agentClock, op.ms); seek(agentClock); }
        break;
      }
    }
  }
  return batch.timers;
}

// Apply a batch; in agent mode, freeze what it started: every animation the
// batch created is registered at the clock it began and seeked there, so
// nothing plays between two operations. The clock lands where the runner
// says (`batch.clock`: an advance a timer refused stops early).
function applyBatch(batch) {
  const timers = apply(batch);
  if (agentMode) {
    // What the ops since the last marker started belongs to that marker's
    // time — register before the clock moves on to where the batch landed.
    register(agentClock);
    if (batch.clock != null && batch.clock > agentClock) agentClock = batch.clock;
    seek(agentClock);
  }
  return { timers, batch };
}

function send(len) {
  return applyBatch(JSON.parse(readOut(len))).timers;
}

// The agent API's page half (LLP 1012). `tree`, `state`, `logs`, and
// `settle` go to the wasm (`exact_agent`); `layout` reads the browser's
// boxes — the only layout the web host has; `clock` moves both clocks to
// one instant: the runner's (`exact_advance`, each timer at its own time)
// and every animation the browser holds (`Animation.currentTime`, LLP 1002
// D3), and the GPU module's picture. `tap`, `type`, and `screenshot` are
// the driver's, over CDP: real input, real pixels.
function ask(request) {
  const n = writeIn(JSON.stringify(request));
  return JSON.parse(readOut(wasm.exact_agent(n)));
}
function register(t) {
  for (const a of document.getAnimations()) if (!starts.has(a)) starts.set(a, t);
}
function seek(to) {
  for (const a of document.getAnimations()) {
    const timing = a.effect?.getComputedTiming();
    if (!timing) continue;
    const t = to - (starts.get(a) ?? agentClock);
    if (t >= timing.endTime) a.finish();
    else { a.pause(); a.currentTime = t; }
  }
}
// When the last thing in flight ends: the springs' engine knows its own
// (`settle`), the browser's animations report theirs, never before now.
function settleCandidate() {
  let to = agentClock;
  const s = ask({ op: "settle" }).settle;
  if (s != null) to = Math.max(to, s);
  for (const a of document.getAnimations()) {
    const timing = a.effect?.getComputedTiming();
    if (timing) to = Math.max(to, (starts.get(a) ?? agentClock) + timing.endTime);
  }
  return to;
}
function agent(request) {
  try {
    if (!wasm) return { error: "not booted" };
    switch (request.op) {
      case "layout": {
        // Every view in the document (attached, whether or not it lies in
        // the viewport), by id, in the viewport's space with every scroll
        // offset and transform folded in, to two decimals.
        const r2 = (x) => Math.round(x * 100) / 100;
        const nodes = [];
        for (const [id, el] of [...views].sort((a, b) => a[0] - b[0])) {
          if (!el.isConnected) continue;
          const r = el.getBoundingClientRect();
          const n = { id, x: r2(r.x), y: r2(r.y), w: r2(r.width), h: r2(r.height) };
          if (el.dataset.scroll === "true") { n.sx = r2(el.scrollLeft); n.sy = r2(el.scrollTop); }
          nodes.push(n);
        }
        return { clock: now(), viewport: { w: innerWidth, h: innerHeight }, nodes };
      }
      case "focus": {
        const el = views.get(request.id);
        if (!el) return { error: `no view ${request.id}` };
        if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return { error: `view ${request.id} is not an input` };
        el.focus();
        el.select();
        return { ok: true };
      }
      case "clock": {
        // To `to`, or to `settle`: a fixed point — advance to when the last
        // thing in flight ends, and if the timers crossed on the way started
        // more, again (bounded; `settled: false` at the bound). The clock
        // lands where the runner says; a timer's refusal is the error.
        const settle = !!request.settle;
        let to = settle ? settleCandidate() : request.to;
        if (!(to >= agentClock)) return { error: `the clock cannot go backwards (${agentClock} → ${to})` };
        for (let rounds = 0; ; rounds++) {
          const { batch } = applyBatch(JSON.parse(readOut(wasm.exact_advance(to))));
          globalThis.exact.gpu?.schedule?.();
          if (batch.error) return { error: `clock: ${batch.error}`, clock: agentClock };
          if (!settle) return { clock: agentClock };
          const next = settleCandidate();
          if (next <= agentClock) return { clock: agentClock, settled: true };
          if (rounds >= 15) return { clock: agentClock, settled: false };
          to = next;
        }
      }
      default:
        return ask(request);
    }
  } catch (e) {
    return { error: String(e) };
  }
}

let ticker = null;

// Boot the app — from the plan baked into the wasm, or from `bytes` (the
// dev loop's restart, LLP 1004 D5: a reload is a restart from initial
// state). Returns the milliseconds from call to first frame in the DOM.
function boot(bytes) {
  const t = performance.now();
  if (ticker) clearInterval(ticker);
  ticker = null;
  for (const a of animations.values()) a.cancel();
  animations.clear();
  globalThis.exact?.gpu?.reset();
  views.clear();
  root.replaceChildren();
  let timers;
  if (bytes) {
    const ptr = wasm.exact_in(bytes.length);
    new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    timers = send(wasm.exact_boot_plan(bytes.length));
  } else {
    timers = send(wasm.exact_boot());
  }
  if (timers && !agentMode) ticker = setInterval(() => send(wasm.exact_advance(now())), 250);
  if (bytes) requestAnimationFrame(loadGpuIfNeeded);
  return performance.now() - t;
}

// `agent` and `now` exist only in agent mode: a normal page has no agent
// surface and no clock but the browser's.
globalThis.exact = { reload: (bytes) => (wasm ? boot(bytes) : NaN), ...(agentMode ? { agent, now } : {}), views, root, pendingSurfaces: [] };

// The GPU module, on demand: a script element after the first painted
// frame — never an import, which the boot check counts — and only when a
// canvas is on the page.
let gpuRequested = false;
function loadGpuIfNeeded() {
  if (gpuRequested || !(globalThis.exact.pendingSurfaces ?? []).length) return;
  gpuRequested = true;
  const s = document.createElement("script");
  s.type = "module";
  s.src = new URL("./gpu-glue.js", import.meta.url).href;
  document.head.append(s);
}

async function main() {
  const url = new URL("./app.wasm", import.meta.url);
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {});
  wasm = instance.exports;
  memory = wasm.memory;
  boot(null);
  // The first frame is in the DOM: stamp the time from script start, so a
  // headless run can read it. A second stamp lands when it is painted.
  root.dataset.bootMs = (performance.now() - t0).toFixed(1);
  requestAnimationFrame(() => {
    root.dataset.paintMs = (performance.now() - t0).toFixed(1);
    loadGpuIfNeeded();
  });
}

main().catch((e) => { console.error(e); root.dataset.error = String(e); });
