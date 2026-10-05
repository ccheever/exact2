// The GPU module's page side (host code, LLP 1009 D2/D4): injected by glue.js
// after the first painted frame, only when the page has a canvas. It fetches
// the app's GPU wasm (wgpu on the browser's WebGPU, wasm-bindgen glue) and
// runs each canvas's surface: bind on new inputs, render while dirty or
// wanted, resize from the element's box and devicePixelRatio.
//
// One artifact per instance (LLP 1009 D6): with no query this is the app's
// one module, `gpu.js`, and it is `exact.gpu`. An app with declared modules
// loads gpu-modules.js instead, which imports this file once per artifact as
// `gpu-glue.js?artifact=<stem>` — a URL is one module instance, so each
// artifact keeps its own device, surfaces, recovery and frame loop — and each
// instance registers with that router rather than taking `exact.gpu`.
import { pacer } from "./pace.js";
import { assetDelivery } from "./gpu-assets.js";
const artifact = new URL(import.meta.url).searchParams.get("artifact");
const stem = artifact ?? "gpu";
let gpu;
const WORLD_LIMIT = 256 * 1024 * 1024;
const HOST_WORK_LIMIT = 16 * 1024 * 1024;
const worldSize = bytes => { if (bytes.length > WORLD_LIMIT) throw new Error("world carrier exceeds 256 MiB limit"); return bytes; };
let terminalRestoreReported = false;
const restoreJournal = [];

const exact = globalThis.exact;
const publishers = new Map(); // name -> first live entry
const pendingRecords = [];
let drainingRecords = false;
let planCarries = new Map();
const surfaces = new Map(); // view id -> surface, input listeners and journal cursor
// postMessage(text, name): held per surface name (at most exact.postBound, glue.js's
// POST_BOUND = Linux POST_BOUND = Apple Canvases.postBound) until a canvas of
// that name is live, then delivered in order to the live one with the lowest view.
const posts = new Map(); // surface name -> posted message events awaiting its canvas
function deliverPosts(name) {
  const queued = posts.get(name);
  const entry = [...surfaces.values()].filter(e => e.name === name && e.id && !e.terminal && live(e.view) === e).sort((a, b) => a.view - b.view)[0];
  if (!entry || !queued?.length) return;
  posts.delete(name);
  for (const json of queued) if (!gpu.gpu_input(entry.id, json)) console.error("exact gpu:", gpu.gpu_error());
  messages(entry); schedule();
}
const inputStyle = document.createElement("style");
inputStyle.textContent = "[data-gpu-input]:focus{outline:none}";
document.head.append(inputStyle);
let loaded = false, loadMs;
let recoveringDevice;
let pendingCutover;
const RECOVERY_LIMIT = 5;
let recoveryTimer, recoveryFailures = 0, recoveryErrorLogged = false, recoveryTerminalLogged = false, lossDuringRecovery = false;
let terminalDevice = false;
let raf = null;
let finishReady;
const ready = new Promise((resolve) => { finishReady = resolve; });

const delivery = assetDelivery({getModule:() => gpu, live, baseURI:() => document.baseURI,
  devAssets:() => exact.devAssets, delivered(entry, module, error) {
    finishRestore(entry, module, error);
    messages(entry, false); entry.resampleHeld?.(); schedule();
  }});
const {assets, cancelAssets} = delivery;
function reportRestore(entry) {
  if (!entry.restoreError || entry.restoreReported === entry.restoreError) return;
  entry.restoreReported = entry.restoreError;
  restoreJournal.push({canvas:entry.view, error:entry.restoreError});
  exact.devError?.(entry.restoreError); console.error(entry.restoreError);
}
function finishRestore(entry, module, error) {
  const pending = entry.pendingRestore;
  if (!pending) return;
  if (error) {
    entry.restoreError = `surface ${entry.name}: restore refused: ${String(error).replace(/^restore refused: /, "")}`;
    delete entry.pendingRestore;
    reportRestore(entry);
    return;
  }
  const world = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({op:"state"})) || "null")?.world;
  if (world && !world.restored) return;
  if (pending.carrier?.worldCarry === pending.bytes) {
    delete pending.carrier.worldCarry;
    if (pending.carrier === exact) delete globalThis.exactWorldCarry;
  }
  delete entry.pendingRestore; delete entry.attemptedCarry; delete entry.carry;
  delete entry.restoreError; delete entry.restoreReported;
  entry.restoredCarry = true;
  entry.resampleHeld?.();
}
const pendingRecovery = () => {
  const entries = [...surfaces.values()].filter(entry => !entry.terminal);
  return entries.length ? entries.map(entry => ({name:`GPU recovery ${entry.name}`,canvas:entry.view})) : [{name:"GPU recovery"}];
};
async function settled() {
  await ready;
  const deadline = performance.now() + 2500;
  while ((recoveringDevice || recoveryTimer) && performance.now() < deadline) await new Promise(resolve => setTimeout(resolve, 0));
  if (recoveringDevice || recoveryTimer) return pendingRecovery();
  const pending = await delivery.settled(() => surfaces.values());
  if (!pending.length) {
      // Agent operations return after presentation reaches the committed clock,
      // including a child-text update published by the rendered world.
      // GPU pipeline validation completes on browser promises, independently of
      // simulation time. Surface::preparing keeps gpu_dirty true until usable.
      if (exact.now) for (;;) {
        if (recoveringDevice || recoveryTimer) {
          if (performance.now() >= deadline) {
            pending.push(...pendingRecovery());
            break;
          }
          await new Promise(resolve => setTimeout(resolve, 0));
          continue;
        }
        let drew = false;
        for (const entry of surfaces.values()) if (entry.id && !entry.terminal && (gpu.gpu_dirty(entry.id) || entry.renderedAt !== exact.now())) {
          render(entry, exact.now()); drew = true;
        }
        flush();
        if (!drew) break;
        if (performance.now() >= deadline) {
          for (const entry of surfaces.values()) if (entry.id && !entry.terminal && gpu.gpu_dirty(entry.id)) pending.push({name:`GPU presentation ${entry.name}`,canvas:entry.view});
          break;
        }
        await new Promise(resolve => setTimeout(resolve, 0));
      }
      if (api.recovery?.status === "recovered") api.recovery.instances = [...surfaces.values()].filter(e => e.id && !e.terminal).map(e => ({id:e.id, preparation:JSON.parse(gpu.gpu_agent(e.id, '{"op":"state"}') || "null")}));
  }
  return pending;
}

function size(el) {
  const r = el.getBoundingClientRect();
  return { w: Math.max(r.width, 1), h: Math.max(r.height, 1), s: devicePixelRatio || 1 };
}

// exact.now() follows each batch `at` marker while surface commits are applied.
// The surfaces' clock: the page's in agent mode (LLP 1012: the driver owns
// time, and a picture is a function of it), else the frame's.
const clockFor = (frameNow) => exact.now?.() ?? frameNow;

let hidden = document.hidden;
function lifecycle(code) {
  if (code === 0 || code === 1) hidden = code === 0;
  const deliver = () => { for (const entry of surfaces.values()) if (entry.id) gpu?.gpu_lifecycle(entry.id, code); };
  if (recoveringDevice) recoveringDevice.then(deliver); else deliver();
  if (hidden && !exact.now && raf !== null) { cancelAnimationFrame(raf); raf = null; }
  if (!hidden) schedule();
}
document.addEventListener("visibilitychange", () => lifecycle(document.hidden ? 0 : 1));
window.addEventListener("pagehide", () => lifecycle(0));
window.addEventListener("pageshow", event => lifecycle(event.persisted || !document.hidden ? 1 : 0));

// A homography maps child-local points to canvas points. CSS adds the child's
// kernel offset after its transform, so subtract that offset in homogeneous space.
function childFrames(entry) {
  const children = [...entry.host.children].filter(el => !el.hasAttribute("data-surface"));
  return children.map(el => ({el, name:el.getAttribute("data-testid") ?? "", frame:[el.offsetLeft, el.offsetTop, el.offsetWidth, el.offsetHeight]}));
}
function restoreChild(row) {
  if (!row.original) return;
  Object.assign(row.el.style, row.original.style); row.el.inert = row.original.inert;
  delete row.original;
}
function supplyChildren(entry, module = gpu, staging = false) {
  if (module.gpu_children_mode?.(entry.id) !== 3) {
    if (!staging) for (const row of entry.children ?? []) restoreChild(row);
    if (entry.children) {
      module.gpu_children_count(entry.id, 0);
      if (!staging) entry.el.style.zIndex = "-1";
    }
    entry.children = undefined; return;
  }
  const previous = entry.children ?? [];
  const rows = childFrames(entry);
  if (!staging) for (const row of previous) if (!rows.some(next => next.el === row.el)) restoreChild(row);
  for (const [i, row] of rows.entries()) {
    const old = previous.find(old => old.el === row.el);
    row.original = old?.original;
    row.hidden = old?.hidden;
    if (staging || previous[i]?.el !== row.el || previous[i]?.name !== row.name || row.frame.some((n, j) => n !== previous[i].frame[j]))
      module.gpu_child_view(entry.id, i, row.name, ...row.frame);
  }
  if (staging || !entry.children || previous.length !== rows.length) module.gpu_children_count(entry.id, rows.length);
  entry.children = rows;
}
function placeChildren(entry) {
  if (!entry.children) return;
  const h = new Float32Array(10), placed = [];
  for (const [i, row] of (entry.children ?? []).entries()) {
    const outcome = gpu.gpu_placement(entry.id, i, h), el = row.el;
    row.hidden = outcome === 2;
    if (outcome === 0) { restoreChild(row); continue; }
    row.original ??= {style:Object.fromEntries(["transform","transformOrigin","zIndex","visibility","position"].map(k => [k, el.style[k]])), inert:el.inert};
    el.style.visibility = outcome === 2 ? "hidden" : row.original.style.visibility;
    el.inert = outcome === 2 || row.original.inert;
    if (outcome === 2) continue;
    const [x,y] = row.frame;
    el.style.transformOrigin = "0 0";
    // Relative positioning makes z-index apply to ordinary block children too.
    if (!row.original.style.position || row.original.style.position === "static") el.style.position = "relative";
    el.style.transform = `matrix3d(${[h[0]-x*h[6],h[3]-y*h[6],0,h[6],h[1]-x*h[7],h[4]-y*h[7],0,h[7],0,0,1,0,h[2]-x*h[8],h[5]-y*h[8],0,h[8]].join(",")})`;
    placed.push({el, depth:h[9], index:i});
  }
  // Integer CSS ranks preserve the full float ordering. At equal depth the later
  // Contract child draws/hits last, on native and web alike.
  placed.sort((a,b) => a.depth-b.depth || a.index-b.index);
  entry.el.style.zIndex = String(-placed.length-1);
  placed.forEach((row,i) => { row.el.style.zIndex = String(i-placed.length); });
}

// Both code replacement and device recovery replace the presentation context.
const replacementCanvas = entry => entry.el.cloneNode(false);
function installCanvas(old, entry) {
  cancelAssets(old); old.observer?.disconnect(); old.unlisten?.();
  old.el.replaceWith(entry.el);
  if (old.host === old.el) { entry.host = entry.el; exact.views.set(entry.view, entry.el); }
}
function resetRecovery() {
  recoveryFailures = 0;
  recoveryErrorLogged = false;
  recoveryTerminalLogged = false;
  terminalDevice = false;
}
function fallback(entry) {
  if (entry.fallbackApplied) return;
  entry.fallbackApplied = true;
  entry.terminal = true;
  entry.wants = false;
  for (const row of entry.children ?? []) restoreChild(row);
  entry.children = undefined;
  entry.el.style.zIndex = "-1";
  // Resetting the bitmap once discards stale pixels. Its CSS box stays in the
  // wrapper, transparent over the wrapper's background and under its children.
  entry.el.width = 1;
  entry.el.height = 1;
}
function terminalRecovery(outcome) {
  terminalDevice = true;
  recoveryFailures = RECOVERY_LIMIT;
  for (const entry of surfaces.values()) fallback(entry);
  api.recovery = outcome;
}
function reportRecovery(error, terminal) {
  if (!recoveryErrorLogged || (terminal && !recoveryTerminalLogged)) {
    exact.devError?.(String(error)); console.error("exact gpu recovery:", error);
  }
  recoveryErrorLogged = true;
  recoveryTerminalLogged ||= terminal;
}
function failureOutcome(error) {
  const text = typeof error === "string" ? error : error?.message ?? String(error);
  try {
    const outcome = JSON.parse(text);
    if (outcome?.status === "failed" && typeof outcome.code === "string" && typeof outcome.error === "string") return outcome;
  } catch {}
  return {status:"failed", code:error?.code ?? "recovery", error:text};
}
function recoverDevice() {
  if (recoveringDevice || recoveryTimer || recoveryFailures >= RECOVERY_LIMIT || !loaded) return recoveringDevice;
  lossDuringRecovery = false;
  const module = gpu, entries = [...surfaces.values()].filter(e => e.id);
  const staged = pendingCutover?.module === module ? pendingCutover.staged : entries.map(old => [old, {...old, el:replacementCanvas(old), observer:null, unlisten:null}]);
  const detached = new Set();
  recoveringDevice = (async () => {
    const outcome = pendingCutover?.module === module ? pendingCutover.outcome : JSON.parse(await module.gpu_recover(new Uint32Array(entries.map(e => e.id)), staged.map(([,e]) => e.el)));
    if (gpu !== module) { for (const [, e] of staged) e.el.remove(); return; }
    if (outcome.status === "healthy") {
      for (const [, e] of staged) e.el.remove();
      api.recovery = outcome; resetRecovery(); return;
    }
    if (outcome.status === "no device") {
      for (const [, e] of staged) e.el.remove();
      terminalRecovery(outcome); return;
    }
    if (outcome.status !== "recovered") throw Object.assign(new Error(outcome.error ?? JSON.stringify(outcome)), {code:outcome.code});
    pendingCutover = {module, staged, outcome};
    for (const [old, entry] of staged) {
      if (live(old.view) !== old) { module.gpu_destroy(entry.id); continue; }
      entry.values = old.values;
      detached.add(old);
      installCanvas(old, entry);
      surfaces.set(entry.view, entry);
      if (publishers.get(old.name) === old) publishers.set(old.name, entry);
      entry.wants = true; delete entry.renderedAt;
      attach(entry); assets(entry);
    }
    pendingCutover = null;
    api.recovery = outcome;
    resetRecovery();
    schedule();
  })().catch(error => {
    for (const [old, entry] of staged) {
      cancelAssets(entry); entry.observer?.disconnect(); entry.unlisten?.();
      if (detached.has(old) && surfaces.has(old.view)) {
        entry.el.replaceWith(old.el); surfaces.set(old.view, old);
        if (publishers.get(old.name) === entry) publishers.set(old.name, old);
        if (old.host === old.el) exact.views.set(old.view, old.el);
      }
      if (detached.has(old) && live(old.view) === old) {
        old.observer?.disconnect(); old.unlisten?.(); attach(old);
      }
      if (!pendingCutover) { entry.el.width = 0; entry.el.height = 0; entry.el.remove(); }
    }
    const message = String(error);
    // An adapter cannot appear during this page's recovery episode. Retrying
    // requestAdapter after it has said none exists only churns Chromium's
    // failed WebGPU presentation path and can starve the renderer itself.
    // Other device-loss failures retain the bounded exponential retry.
    const noAdapter = error?.code === "no-adapter";
    recoveryFailures = noAdapter ? RECOVERY_LIMIT : recoveryFailures + 1;
    const terminal = recoveryFailures >= RECOVERY_LIMIT;
    const outcome = {status:terminal ? "no device" : "failed", code:error?.code ?? "recovery", error:message};
    if (terminal) terminalRecovery(outcome);
    else {
      api.recovery = outcome;
      recoveryTimer = setTimeout(() => { recoveryTimer = null; recoverDevice(); }, 100 * 2 ** (recoveryFailures - 1));
    }
    reportRecovery(error, terminal);
  }).finally(() => { recoveringDevice = null; if (lossDuringRecovery && !recoveryFailures) queueMicrotask(recoverDevice); });
  return recoveringDevice;
}

// A render records into the module's open frame; `flush` submits every
// canvas recorded since the last one once (LLP 1009 D7). It runs before the
// task that rendered ends, which is when the browser presents the canvases.
function flush(module = gpu) {
  if (module && (module !== gpu || !terminalDevice) && !module.gpu_flush()) console.error("exact gpu:", module.gpu_error());
}

function render(entry, now) {
  if (entry.terminal || (hidden && !exact.now) || recoveringDevice || recoveryTimer) return;
  const { w, h, s } = size(entry.el);
  const pw = Math.max(1, Math.round(w * s)), ph = Math.max(1, Math.round(h * s));
  if (entry.el.width !== pw || entry.el.height !== ph) { entry.el.width = pw; entry.el.height = ph; }
  supplyChildren(entry);
  const r = gpu.gpu_render(entry.id, w, h, s, clockFor(now));
  if (r < 2) {
    exact.drawCallback?.(frameRaw, clockFor(now), frameGeneration, entry.view);
    placeChildren(entry); entry.renderedAt = clockFor(now);
  }
  if (r === 3) {
    entry.wants = false;
    recoverDevice();
    return;
  }
  if (r === 2) console.error("exact gpu:", gpu.gpu_error());
  const initial = entry.firstFrameSubmittedMs === undefined && !entry.firstFrameFailed ? JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:"state"})) || "null")?.world : null;
  if (initial?.assets?.some(a => a.state === "Failed")) entry.firstFrameFailed = true;
  if (r !== 2 && entry.firstFrameSubmittedMs === undefined && !entry.firstFrameFailed && !initial?.loading?.length) {
    entry.firstFrameSubmittedMs = performance.now();
    entry.inputMs = performance.getEntriesByName('exact-agent-input').at(-1)?.startTime ?? null;
    // A rendering opportunity after submission, not a GPU timestamp or scanout.
    requestAnimationFrame(() => requestAnimationFrame(() => { entry.firstFrameMs = performance.now(); }));
  }
  entry.wants = r === 1;
  messages(entry);
}

// The live frame clock is the callback's timestamp paced onto the display's
// lattice (pace.js): a world drawn at the raw timestamp judders by the
// timestamp's own jitter. The agent's clock (exact.now) bypasses this in clockFor.
const pace = pacer();
let frameAt = null; // the last paced frame time: a render outside the frame loop redraws at it, never ahead of it
let sentPeriod = 0, frameGeneration = 0, frameRaw = 0;
function frame(now) {
  raf = null;
  if (hidden && !exact.now) return;
  const at = frameAt = pace(now);
  frameRaw = now; frameGeneration++;
  // Bootstrap and subsequent stable fits reach the module once per real change.
  const period = pace.period_ms;
  if (period !== sentPeriod && gpu) { sentPeriod = period; gpu.gpu_period(period); }
  let more = false;
  for (const entry of surfaces.values()) {
    if (!entry.id || entry.terminal) continue;
    if (entry.wants || gpu.gpu_dirty(entry.id)) render(entry, at);
    more ||= entry.wants;
  }
  flush();
  if (more) globalThis.exact.frames?.activity(); // a development page's sampler watches a canvas in motion (LLP 1079 D3)
  // Under the agent's clock a frame is asked for by `clock`, never by the
  // last frame: a surface that wants more renders again when time moves.
  if (more && !exact.now) schedule();
}

function schedule() { if ((!hidden || exact.now) && raf === null) raf = requestAnimationFrame(frame); }

// Creation/binding is a hard result. Staging never publishes or attaches listeners.
function create(entry, module, carry) {
  const { w, h, s } = size(entry.host);
  entry.el.width = Math.max(1, Math.round(w * s));
  entry.el.height = Math.max(1, Math.round(h * s));
  entry.id = module.gpu_create(entry.name, entry.el, entry.el.width, entry.el.height);
  if (!entry.id) throw new Error(`surface ${entry.name}: create: ${module.gpu_error()}`);
  module.gpu_lifecycle(entry.id, hidden ? 0 : 1);
  if (!module.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.())) throw new Error(`surface ${entry.name}: bind: ${module.gpu_error()}`);
  if (carry !== undefined) {
    entry.carry = carry;
    if (module.gpu_restore(entry.id, worldSize(carry), 1)) {
      entry.pendingRestore = {bytes:carry};
      finishRestore(entry, module);
    }
    else entry.restoreError = `surface ${entry.name}: restore refused: ${module.gpu_error().replace(/^restore refused: /, "")}`;
  }
}
function attach(entry) {
  entry.observer = new ResizeObserver(() => { if (entry.id && !entry.terminal) { render(entry, frameAt ?? performance.now()); flush(); } });
  entry.observer.observe(entry.el);
  entry.wantsInput = gpu.gpu_wants_input(entry.id);
  if (entry.wantsInput) listen(entry);
  reportRestore(entry);
  messages(entry); schedule();
  deliverPosts(entry.name);
}
function restorePending(entry, module = gpu, carrier = exact) {
  if (carrier.worldCarry === undefined || entry.attemptedCarry === carrier.worldCarry) return;
  let carried;
  try { carried = module.gpu_carry(entry.id); } catch { carried = null; }
  if (carried === undefined) {
    try { if (!JSON.parse(module.gpu_agent(entry.id, JSON.stringify({op:"state"})) || "null")?.world) return; }
    catch { return; }
  }
  entry.attemptedCarry = carrier.worldCarry;
  if (module.gpu_restore(entry.id, worldSize(carrier.worldCarry), 0)) {
    entry.pendingRestore = {bytes:carrier.worldCarry, carrier};
    finishRestore(entry, module);
  } else entry.restoreError = `surface ${entry.name}: restore refused: ${module.gpu_error().replace(/^restore refused: /, "")}`;
}

function ensure(entry) {
  if (entry.id) return;
  if (terminalDevice) { fallback(entry); return; }
  if (!loaded) return;
  if (recoveringDevice) { recoveringDevice.then(() => { if (surfaces.get(entry.view) === entry) ensure(entry); }); return; }
  try {
    create(entry, gpu, entry.carry);
    if (!entry.restoreError && !entry.pendingRestore) delete entry.carry;
    restorePending(entry);
  } catch (error) {
    if (entry.id) gpu.gpu_destroy(entry.id);
    entry.id = 0;
    throw error;
  }
  attach(entry);
}
function restoreReply(reply) {
  if (exact.worldCarry !== undefined && !terminalRestoreReported) {
    const candidates = [...surfaces.values()];
    if (candidates.length && candidates.every(e => e.id && (e.attemptedCarry === exact.worldCarry || JSON.parse(gpu.gpu_agent(e.id, '{"op":"state"}') || "null")?.world === undefined))
        && candidates.some(e => e.restoreError)) {
      terminalRestoreReported = true;
      return { ...reply, error: candidates.filter(e => e.restoreError).map(e => e.restoreError).join("; ") };
    }
  }
  return reply;
}

function live(view) {
  const entry = surfaces.get(view);
  return entry?.id && exact.views.get(view) === entry.host && entry.el.isConnected ? entry : null;
}
function surfaceRecord(name, json) {
  pendingRecords.push([name, json]);
  drainRecords();
}
function drainRecords() {
  if (exact.applyDepth || drainingRecords) return;
  drainingRecords = true;
  try {
    while (pendingRecords.length) {
      const [name, json] = pendingRecords.shift();
      exact.send(exact.wasm.exact_surface_record(exact.writeIn(json == null ? name : `${name}\0${json}`)));
    }
  } finally { drainingRecords = false; }
}
function messages(entry, drainAssets = true) {
  if (drainAssets) assets(entry);
  finishRestore(entry, gpu);
  reportRestore(entry);
  const record = gpu.gpu_published(entry.id);
  if (record !== undefined && live(entry.view) === entry && publishers.get(entry.name) === entry) surfaceRecord(entry.name, record);
  const texts = gpu.gpu_messages(entry.id);
  if (texts === undefined) return;
  for (const text of JSON.parse(texts)) {
    if (live(entry.view) !== entry) break;
    if (text !== "exact:audio") exact.message(entry.host, text);
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
    // Preserve device ordering. Sim clamps queued live stamps to the paced frame
    // at advance; an already delivered event is never future to that callback.
    const id = entry.id, json = JSON.stringify({ ...value, at: exact.now?.() ?? event.timeStamp });
    const deliver = () => {
      const current = live(entry.view);
      if (current?.id !== id) return;
      if (!gpu.gpu_input(id, json)) console.error("exact gpu:", gpu.gpu_error());
      messages(current); schedule();
    };
    if (recoveringDevice) recoveringDevice.then(deliver); else deliver();
  };
  const controls = entry.controls ??= new Map(), controlKeys = entry.controlKeys ??= new Map();
  const control = target => {
    const node = target instanceof Element ? target.closest('button[data-action]') : null;
    return node && node.closest('[data-gpu-input]') === el && node.getAttribute('data-action') && !node.disabled && !node.hidden && !node.closest('[hidden], [inert]') && !node.closest('[inert]') ? node : null;
  };
  const binding = node => { const r = node.getBoundingClientRect(); return {node, name:node.getAttribute("data-action"), left:r.left, top:r.top}; };
  const sendControl = (event, owner, phase, id, x = 0, y = 0) => send(event, {t:"control", name:owner.name, phase, id, x, y});
  const cancelRemoved = () => {
    for (const owners of [controls, controlKeys]) for (const [key, owner] of owners) {
      if (owner.node ? (!owner.node.isConnected || !owner.node.getAttribute("data-action") || owner.node.closest("[data-gpu-input]") !== el) : ![...el.querySelectorAll("button[data-action]")].some(node => node.isConnected && node.getAttribute("data-action") === owner.name && node.closest("[data-gpu-input]") === el)) {
        sendControl({timeStamp:performance.now()}, owner, "cancel", owners === controls ? key : key === "Space" ? 4294967294 : 4294967293);
        owners.delete(key);
      }
    }
  };
  const mutations = new MutationObserver(cancelRemoved);
  mutations.observe(el, {subtree:true, childList:true, attributes:true, attributeFilter:["data-action"]});
  const fallsThrough = (event) => event.target === el || event.target === entry.el;
  const point = (event) => { const r = el.getBoundingClientRect(); return { x: event.clientX - r.left, y: event.clientY - r.top }; };
  // A canvas marked data-pointer-lock="true" (mouse look) captures the mouse on a
  // press. Locked, a pointer event's motion is the device's (movementX/Y), so the
  // world's deltas never stop at the canvas or screen edge though the position
  // stays put; unlocked it is the position's change, as on every other host. A
  // down or up carries none.
  const lockable = () => (el.dataset.pointerLock ?? entry.el.dataset?.pointerLock) === "true";
  const last = new Map(); // each pointer's last point on the canvas
  on("pointerleave", (event) => { if (document.pointerLockElement !== el) last.delete(event.pointerId); });
  const pointerAt = (event, phase) => {
    const p = point(event), from = last.get(event.pointerId);
    if ((phase === "up" || phase === "cancel") && event.pointerType !== "mouse") last.delete(event.pointerId); else last.set(event.pointerId, p);
    if (phase !== "move") return { ...p, dx: 0, dy: 0 };
    if (document.pointerLockElement === el) return { ...p, dx: event.movementX || 0, dy: event.movementY || 0 };
    return { ...p, dx: from ? p.x - from.x : 0, dy: from ? p.y - from.y : 0 };
  };
  // The secondary button and the middle button are the world's (MouseRight, MouseMiddle).
  on("contextmenu", (event) => { if (fallsThrough(event)) event.preventDefault(); });
  on("mousedown", (event) => { if (event.button === 1 && fallsThrough(event)) event.preventDefault(); });
  for (const phase of ["down", "move", "up", "cancel"]) on(`pointer${phase}`, (event) => {
    const wasControl = controls.has(event.pointerId);
    cancelRemoved();
    if (wasControl && !controls.has(event.pointerId)) { event.preventDefault(); return; }
    let owner = controls.get(event.pointerId);
    const button = phase === "down" ? control(event.target) : null;
    if (button) {
      owner = binding(button); controls.set(event.pointerId, owner);
      try { button.setPointerCapture(event.pointerId); } catch {}
      if (!editable(document.activeElement)) button.focus({preventScroll:true});
    }
    if (owner) {
      sendControl(event, owner, phase, event.pointerId, event.clientX-owner.left, event.clientY-owner.top);
      if (phase === "up" || phase === "cancel") controls.delete(event.pointerId);
      event.preventDefault(); return;
    }
    if (!fallsThrough(event)) return;
    if (phase === "down") { if (!editable(document.activeElement)) el.focus({ preventScroll: true }); try { el.setPointerCapture(event.pointerId); } catch {} }
    if (phase === "down" && event.pointerType === "mouse" && lockable() && document.pointerLockElement !== el) Promise.resolve(el.requestPointerLock?.()).catch(() => {});
    send(event, { t: "pointer", phase, id: event.pointerId, ...pointerAt(event, phase), kind: event.pointerType || "mouse", buttons: event.buttons });
  });
  on("lostpointercapture", event => {
    const button = controls.get(event.pointerId);
    if (button) { controls.delete(event.pointerId); sendControl(event, button, "cancel", event.pointerId); }
  });
  on("wheel", (event) => {
    if (!fallsThrough(event)) return;
    event.preventDefault();
    send(event, { t: "wheel", dx: event.deltaX, dy: event.deltaY, ...point(event) });
  }, { passive: false });
  // A restored keydown (including a queued one) still owns its future keyup.
  const held = entry.heldKeys ??= new Set();
  entry.resampleHeld = () => {
    if (!entry.restoredCarry) return;
    const world = JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:"state"})) || "null")?.world;
    if (!world?.restored) return;
    held.clear(); for (const code of world.input?.forwarded ?? []) held.add(code);
    controls.clear(); controlKeys.clear();
    for (const contact of world.input?.controlContacts ?? []) {
      const candidates = [...el.querySelectorAll("button[data-action]")].filter(node => node.getAttribute("data-action") === contact.action && node.closest("[data-gpu-input]") === el);
      const node = candidates.length === 1 ? candidates[0] : null;
      const owner = {...(node ? binding(node) : {name:contact.action, left:0, top:0}), origin:contact.origin, position:contact.position};
      if (contact.id === 4294967294) controlKeys.set("Space", owner);
      else if (contact.id === 4294967293) controlKeys.set("Enter", owner);
      else controls.set(contact.id, owner);
    }
    delete entry.restoredCarry;
  };
  entry.resampleHeld();
  const editable = target => target instanceof Element && target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])');
  const blur = event => {
    entry.resampleHeld();
    for (const [id, owner] of controls) sendControl(event, owner, "cancel", id);
    for (const [code, owner] of controlKeys) sendControl(event, owner, "cancel", code === "Space" ? 4294967294 : 4294967293);
    held.clear(); controls.clear(); controlKeys.clear(); send(event, { t: "blur" });
  };
  // A pointer-owned control may coexist with a focused editor outside this canvas.
  const pressedKey = (event, down) => {
    if (event.defaultPrevented || event.isComposing || event.metaKey || event.ctrlKey || !["Space", "Enter", "NumpadEnter"].includes(event.code)) return;
    cancelRemoved(); entry.resampleHeld();
    const owner = controlKeys.get(event.code) ?? (down ? controls.values().next().value : null);
    if (!owner) return;
    event.preventDefault();
    if (down && !controlKeys.has(event.code)) {
      controlKeys.set(event.code, owner); sendControl(event, owner, "down", event.code === "Space" ? 4294967294 : 4294967293);
    } else if (!down) {
      controlKeys.delete(event.code); sendControl(event, owner, "up", event.code === "Space" ? 4294967294 : 4294967293);
    }
  };
  const pressedDown = event => pressedKey(event, true), pressedUp = event => pressedKey(event, false);
  window.addEventListener("keydown", pressedDown, true);
  window.addEventListener("keyup", pressedUp, true);
  on("keydown", event => {
    const target = event.target instanceof Element ? event.target : null;
    if (event.defaultPrevented || event.isComposing || event.code === "Tab" || editable(target)) return;
    // The focused game's own canvas owns Ctrl/Meta chords (RTS control groups).
    // An editor or ordinary HUD control keeps its browser shortcuts.
    if (event.metaKey || event.ctrlKey) {
      if (target !== el && target !== entry.el) return;
      event.preventDefault();
    }
    const button = control(target);
    if (button && ["Space", "Enter", "NumpadEnter"].includes(event.code)) {
      event.preventDefault();
      if (!controlKeys.has(event.code)) { controlKeys.set(event.code, binding(button)); sendControl(event, controlKeys.get(event.code), "down", event.code === "Space" ? 4294967294 : 4294967293); }
      return;
    }
    if (["Space", "Enter", "NumpadEnter"].includes(event.code) && target?.closest('button, a[href], [role="button"], [role="link"]')) {
      event.preventDefault(); if (!event.repeat) target.closest('button, a[href], [role="button"], [role="link"]').click(); return;
    }
    if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Space", "PageUp", "PageDown", "Home", "End"].includes(event.code)) event.preventDefault();
    held.add(event.code);
    send(event, { t: "key", code: event.code, key: event.key, down: true, repeat: event.repeat });
  });
  on("keyup", event => {
    entry.resampleHeld();
    const button = controlKeys.get(event.code);
    if (button) { controlKeys.delete(event.code); sendControl(event, button, "up", event.code === "Space" ? 4294967294 : 4294967293); event.preventDefault(); return; }
    entry.resampleHeld();
    if (!held.delete(event.code)) return;
    send(event, { t: "key", code: event.code, key: event.key, down: false, repeat: false });
  });
  const inactive = event => blur(event);
  window.addEventListener("blur", inactive);
  // Assistive technology activates a button without a pointer/key sequence.
  on("click", event => { const button = control(event.target); if (button && event.detail === 0 && !controlKeys.size) { sendControl(event, binding(button), "down", 4294967292); sendControl(event, binding(button), "up", 4294967292); } });
  on("focusin", event => { if (editable(event.target)) blur(event); });
  on("focusout", event => {
    if (controls.size || el.contains(event.relatedTarget)) return;
    if (event.relatedTarget) blur(event);
    // Removing a focused control can return focus to this canvas in the same commit.
    else queueMicrotask(() => {
      if (live(entry.view) === entry && !controls.size && !el.contains(document.activeElement)) blur(event);
    });
  });
  entry.unlisten = () => {
    mutations.disconnect();
    window.removeEventListener("blur", inactive);
    window.removeEventListener("keydown", pressedDown, true);
    window.removeEventListener("keyup", pressedUp, true);
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
    if (request.op === "state" && value.world && entry.restoreError) value.world.restoreError = entry.restoreError;
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
          gpuMs: loadMs === undefined ? NaN : Number(loadMs.toFixed(1)),
          inputMs: entry.inputMs ?? null,
          firstFrameSubmittedMs: entry.firstFrameSubmittedMs ?? null,
          firstFrameMs: entry.firstFrameMs ?? null,
          inputToFirstFrameMs: entry.inputMs != null && entry.firstFrameMs != null ? entry.firstFrameMs - entry.inputMs : null,
          firstFrameMeaning: 'rendering opportunity after GPU submission; not scanout',
          resources: performance.getEntriesByType('resource')
            .filter(r => /\/(?:app\.wasm|gpu(?:-glue)?\.js|gpu_bg\.wasm|gpu\/[^/]+(?:\.js|_bg\.wasm))$/.test(new URL(r.name).pathname))
            .map(r => ({ name: new URL(r.name).pathname, startMs: r.startTime, endMs: r.responseEnd, bytes: r.decodedBodySize })),
        };
      }
      out.push({ ...world, canvas: view });
    }
  }
  return out;
}

const api = {
  deviceLost() { if (recoveringDevice) lossDuringRecovery = true; else queueMicrotask(() => recoverDevice()); },
  drainRecords,
  agent,
  settled,
  async surfaceWork(name, mode, bytes, active) {
    await ready;
    const matches=[...surfaces.values()].filter(e=>e.name===name&&live(e.view)===e);
    if(matches.length!==1)throw Object.assign(new Error(`surface ${name}: expected one live surface, found ${matches.length}`),{kind:2});
    const entry=matches[0], id=entry.id;
    await settled();
    if(!active()||live(entry.view)!==entry||entry.id!==id)throw Object.assign(new Error(`surface ${name}: request retired or surface replaced`),{kind:4});
    if(!id)throw Object.assign(new Error(`surface ${name}: module unavailable`),{kind:3});
    if(mode==="capture"){
      const out=gpu.gpu_carry(id);
      if(out===undefined)throw Object.assign(new Error(`surface ${name}: carries no state`),{kind:3});
      if(out.length>HOST_WORK_LIMIT)throw Object.assign(new Error(`surface ${name}: carried state exceeds 16 MiB`),{kind:2});
      return out;
    }
    if(!(bytes instanceof Uint8Array)||bytes.length>HOST_WORK_LIMIT)throw Object.assign(new Error(`surface ${name}: invalid or oversized restore`),{kind:2});
    if(!gpu.gpu_restore(id,bytes,0))throw Object.assign(new Error(`surface ${name}: ${gpu.gpu_error()}`),{kind:2});
    messages(entry);schedule();
  },
  // postMessage(id, text) from Contract: one input event, stamped at the call,
  // delivered in order; held until the canvas's surface exists.
  post(name, text, at) {
    const queued = posts.get(name) ?? posts.set(name, []).get(name);
    if (queued.length >= (exact.postBound ?? 64)) { console.warn(`exact: postMessage: dropped: ${exact.postBound ?? 64} posts already wait for surface "${name}"`); return; }
    queued.push(JSON.stringify({ t: "message", text, at }));
    deliverPosts(name);
  },
  wantsInput: (view) => live(view)?.wantsInput === true,
  answers: (request) => request.entity !== undefined || request.world === true || request.contact !== undefined,
  handle(request, ask, tagged) {
    const entry = live(request.id);
    if (!entry) return { error: `view ${request.id} has no world` };
    if (request.op === "tap" && request.contact !== undefined) {
      entry.resampleHeld?.();
      const owner = entry.controls?.get(request.contact);
      if (!owner || !["up", "cancel"].includes(request.phase)) return {error:"no restored contact to release"};
      const ok = gpu.gpu_input(entry.id, JSON.stringify({t:"control", name:owner.name, id:request.contact, phase:request.phase, x:owner.position?.[0] ?? 0, y:owner.position?.[1] ?? 0, at:exact.now?.() ?? performance.now()}));
      if (ok) entry.controls.delete(request.contact);
      messages(entry); schedule();
      return ok ? tagged({phase:request.phase, delivery:"recognized"}) : {error:gpu.gpu_error()};
    }
    if (request.op === "focus") {
      if (!entry.wantsInput) return { error: `view ${request.id}'s surface does not take input` };
      entry.host.focus({ preventScroll: true }); return tagged({ ok: document.activeElement === entry.host });
    }
    if (request.op === "screenshot" && request.form === "save") {
      let bytes;
      try { bytes = gpu.gpu_carry(entry.id); } catch (error) { return {error: `save refused: ${error}`}; }
      if (bytes === undefined) {
        const state = agent(request.id, {op:"state"})?.world;
        return {error: state?.assets?.length ? `save refused: ${JSON.stringify(state.assets)}` : `canvas ${entry.name} carries no state`, assets:state?.assets};
      }
      worldSize(bytes);
      const state = agent(request.id, { op: "state" });
      let encoded = "";
      for (let i = 0; i < bytes.length; i += 8192) encoded += String.fromCharCode(...bytes.subarray(i, i + 8192));
      return tagged({ data: btoa(encoded), bytes: bytes.length, hash: state?.world?.hash, tick: state?.tick });
    }
    if (!["layout", "state", "tree"].includes(request.op)) return { error: `world does not answer ${request.op}` };
    if (request.op === "layout" && request.entity === undefined) {
      const r = entry.host.getBoundingClientRect();
      request = { ...request, x: request.x - r.left, y: request.y - r.top };
    }
    const reply = agent(request.id, request) ?? { error: `view ${request.id} has no world` };
    const r = entry.host.getBoundingClientRect();
    for (const box of [reply.entity?.screen, reply.hit?.screen]) if (box) { box.x += r.left; box.y += r.top; }
    return restoreReply(tagged(reply));
  },
  decorate(request, reply) {
    if (reply?.then) return reply.then((r) => api.decorate(request, r));
    if (!reply || reply.error) return reply;
    if (api.answers(request)) return restoreReply(reply);
    if (request.op === "tree") for (const node of reply.nodes ?? []) {
      const summary = agent(node.id, { op: "tree", summary: true });
      if (summary?.world) node.world = summary.world;
    }
    if (request.op === "state") {
      const world = worlds({ op: "state" });
      if (world.length) reply.world = [...(reply.world ?? []), ...world]; // another artifact's worlds too
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
      for (const {canvas, error} of restoreJournal.splice(0)) world.push({canvas, lines:[error]});
      if (world.length) reply.world = [...(reply.world ?? []), ...world]; // another artifact's worlds too
    }
    return restoreReply(reply);
  },
  // The existing clock loop owns the 16-round bound; this is its next candidate.
  clock(settle) {
    const world = worlds({ op: "clock", settle });
    const pending = settle && world.some((w) => w.quiescent === false);
    const candidates = world.filter((w) => w.quiescent === false && Number.isFinite(w.settleAt)).map((w) => w.settleAt);
    return { pending, settleAt: candidates.length ? Math.max(...candidates) : undefined,
      reply: world.length ? { world: world.map(({ canvas, tick, hash, quiescent, error, assets, changing }) => ({ canvas, tick, hash, quiescent, changing, ...(error ? {error, assets} : {}) })) } : {} };
  },
  surface(view, name, values) {
    // The node's element hosts its surface <canvas> (glue.js, LLP 1014 D2).
    const host = exact.views.get(view);
    const el = host?.matches("canvas") ? host : host?.querySelector(":scope > canvas[data-surface]");
    if (!el?.isConnected) return; // a queued surface can leave before its lazy module arrives
    let entry = surfaces.get(view);
    if (entry && entry.el !== el) { this.destroy(view); entry = null; } // a reload reuses ids
    if (entry && entry.name !== name) { this.destroy(view); entry = null; } // one id cannot retain another plan's surface
    if (!entry) {
      // A replacement can register before the old owner's destroy reaches us.
      // Only a current, connected canvas holds the publication name.
      const old = publishers.get(name);
      if (old && (!old.el.isConnected || exact.views.get(old.view) !== old.host || surfaces.get(old.view) !== old)) {
        if (surfaces.get(old.view) === old) this.destroy(old.view);
        else { publishers.delete(name); surfaceRecord(name, null); }
      }
      entry = { view, host, el, name, values, id: 0, wants: false, wantsInput: false, logCursor: 0, terminal: terminalDevice }; surfaces.set(view, entry);
      const carried = planCarries.get(name);
      if (carried?.name === name) entry.carry = carried.bytes;
      planCarries.delete(name);
      const publisher = publishers.get(name);
      const publisherRegistered = publisher && surfaces.get(publisher.view) === publisher
        && exact.views.get(publisher.view) === publisher.host;
      if (!publisher || !publisherRegistered || !publisher.el.isConnected) publishers.set(name, entry);
      else console.error(`exact gpu: surface ${name}: duplicate live publisher ignored`);
      ensure(entry); return; }
    entry.values = values;
    if (entry.id && !entry.terminal) {
      const id = entry.id, at = exact.now?.();
      const bind = () => {
        const current = live(view); if (current?.id !== id) return;
        if (!gpu.gpu_bind_at(id, JSON.stringify(values), at)) console.error("exact gpu:", gpu.gpu_error());
        current.values = values; messages(current); schedule();
      };
      if (recoveringDevice) recoveringDevice.then(bind); else bind();
    }
  },
  destroy(view) {
    const entry = surfaces.get(view);
    if (entry) { cancelAssets(entry); for (const row of entry.children ?? []) restoreChild(row); }
    if (entry?.id) gpu.gpu_destroy(entry.id);
    // The observer would fire once more as the element leaves the page, for
    // a surface the module no longer has (found by the agent smoke, which
    // is the first thing to navigate away from a canvas and back).
    if (entry) { entry.observer?.disconnect(); entry.unlisten?.(); entry.id = 0; }
    surfaces.delete(view);
    if (entry && publishers.get(entry.name) === entry) { publishers.delete(entry.name); surfaceRecord(entry.name, null); }
  },
  // A plan restart reassigns view ids. Unique surface names can carry across it;
  // duplicate instances are ambiguous and are deliberately left fresh.
  reset(carry = false) {
    planCarries = new Map();
    if (carry) for (const entry of surfaces.values()) if (entry.id) {
      if ([...surfaces.values()].filter(e => e.name === entry.name).length !== 1) continue;
      try {
        const bytes = gpu.gpu_carry(entry.id);
        if (bytes !== undefined) planCarries.set(entry.name, { name: entry.name, bytes });
      } catch (error) { restoreJournal.push({canvas:entry.view,error:`carry refused: ${error}`}); }
    }
    for (const view of [...surfaces.keys()]) this.destroy(view);
  },
  finishRestart() { planCarries.clear(); },
  // Dev only; callers serialize versions. Agent pages never receive automatic swaps.
  swap(version) { return exact.mutate(() => swap(version)); },
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
    await ready; if (!loaded) return () => {}; // An unavailable optional device cannot block core plans.
    const module = gpu, rows = shaderRows(assets, module);
    for (const [name, text] of rows) if (!await module.gpu_shader_check(name, text)) throw new Error(module.gpu_error());
    if (module !== gpu) throw new Error("GPU changed during shader preparation");
    return () => {
      if (module !== gpu) throw new Error("GPU changed before shader commit");
      replaceShaders(rows, module);
    };
  },
  placementHidden(el) {
    for (const entry of surfaces.values()) for (const row of entry.children ?? [])
      if (row.hidden && (row.el === el || row.el.contains(el))) return true;
    return false;
  },
  beforeStyle(el) {
    for (const entry of surfaces.values()) for (const row of entry.children ?? [])
      if (row.el === el) restoreChild(row);
  },
  afterStyle(el) {
    for (const entry of surfaces.values()) if (entry.children?.some(row => row.el === el)) {
      supplyChildren(entry); placeChildren(entry);
    }
  },
  layout() { for (const entry of surfaces.values()) if (entry.id && !entry.terminal) { supplyChildren(entry); placeChildren(entry); } schedule(); },
  /// Time moved (the agent's `clock`): render what wants a frame, once.
  schedule() { for (const entry of surfaces.values()) if (entry.id && !entry.terminal && entry.wants) { entry.wants = false; gpu.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.()); } schedule(); },
};
// Standalone, the one module is `exact.gpu`. Routed, the router hands this
// instance the surfaces queued for its artifact before its script arrived.
if (artifact === null) exact.gpu = api; else exact.gpu.register(stem, api);

function shaderRows(assets, module = gpu) {
  const names = new Set(JSON.parse(module.gpu_shader_names())), decoder = new TextDecoder("utf-8", { fatal: true });
  return [...assets].filter(([path]) => path.startsWith("shaders/") && path.endsWith(".wgsl"))
    .map(([path, card]) => [path.slice(8, -5), decoder.decode(card.bytes)])
    .filter(([name]) => names.has(name));
}
function replaceShaders(rows, module = gpu) {
  module.gpu_shaders_clear();
  for (const [name, text] of rows) if (!module.gpu_shader(name, text)) throw new Error(module.gpu_error());
}
async function loadShaders(module) {
  const rows = [];
  if (exact.devAssets === null) for (const name of JSON.parse(module.gpu_shader_names())) {
    const r = await fetch(new URL(`./shaders/${name}.wgsl`, import.meta.url));
    if (!r.ok) throw new Error(`shaders/${name}.wgsl: HTTP ${r.status}`);
    rows.push([name, await r.text()]);
  }
  return exact.devAssets === null ? rows : shaderRows(exact.devAssets, module);
}
// Only development uses function-scoped bindgen glue: no immortal module-map
// entry owns a candidate's Wasm memory. Production remains the static ES module.
async function loadModule(version) {
  if (!version) {
    const module = await import(`./${stem}.js`);
    await module.default({ module_or_path: new URL(`./${stem}_bg.wasm`, import.meta.url) });
    if (typeof module.gpu_child_view !== "function") throw new Error("GPU module is missing gpu_child_view");
    return module;
  }
  const response = await fetch(new URL(`./${stem}.js?g=${version}`, import.meta.url));
  if (!response.ok) throw new Error(`GPU loader HTTP ${response.status}`);
  const module = new Function(`${await response.text()}; return wasm_bindgen;`)();
  await module({ module_or_path: new URL(`./${stem}_bg.wasm?g=${version}`, import.meta.url) });
  if (typeof module.gpu_child_view !== "function") throw new Error("GPU module is missing gpu_child_view");
  return module;
}
async function swap(version) {
  await ready;
  await recoveringDevice;
  if (loaded && version === api.version) return { ms: 0, errors: [] };
  const start = performance.now(), next = await loadModule(version), staged = [];
  const carrier = { worldCarry: exact.worldCarry };
  try {
    await next.gpu_load();
    if (exact.now) next.gpu_seekable(true);
    if (sentPeriod) next.gpu_period(sentPeriod);
    const rows = await loadShaders(next);
    for (const [name, text] of rows) if (!await next.gpu_shader_check(name, text)) throw new Error(next.gpu_error());
    replaceShaders(rows, next);
    // A canvas context belongs to one device. Stage on detached replacement
    // canvases; the old canvases and their worlds remain untouched until commit.
    // No await from carry through cutover; input cannot arrive between them.
    for (const old of surfaces.values()) {
      const entry = { ...old, el: replacementCanvas(old), id: 0, observer: null, unlisten: null, terminal: false };
      delete entry.restoreError; delete entry.attemptedCarry;
      delete entry.pendingRestore; delete entry.restoreReported;
      staged.push([old, entry]);
      const carry = old.id ? gpu.gpu_carry(old.id) : old.carry;
      create(entry, next, carry);
      restorePending(entry, next, carrier);
      const {w,h,s} = size(old.host);
      supplyChildren(entry, next, true);
      if (next.gpu_render(entry.id, w, h, s, clockFor(frameAt ?? performance.now())) === 2) throw new Error(`surface ${entry.name}: render: ${next.gpu_error()}`);
    }
    if (!next.gpu_flush()) throw new Error(`surfaces: flush: ${next.gpu_error()}`);
  } catch (error) {
    for (const [,entry] of staged) if (entry.id) next.gpu_destroy(entry.id);
    next.gpu_unload(); exact.devError?.(String(error)); throw error;
  }
  if (raf !== null) { cancelAnimationFrame(raf); raf = null; }
  const oldModule = gpu;
  for (const [old, entry] of staged) {
    installCanvas(old, entry);
    if (publishers.get(old.name) === old) publishers.set(old.name, entry);
    surfaces.set(entry.view, entry);
    if (old.id) oldModule.gpu_destroy(old.id);
  }
  oldModule?.gpu_unload(); gpu = next; loaded = true; resetRecovery();
  for (const [old, entry] of staged) {
    for (const row of old.children ?? []) if (!entry.children?.some(next => next.el === row.el)) restoreChild(row);
    placeChildren(entry);
  }
  for (const [, entry] of staged) if (entry.pendingRestore?.carrier === carrier) entry.pendingRestore.carrier = exact;
  api.version = version;
  if (carrier.worldCarry === undefined) { delete exact.worldCarry; delete globalThis.exactWorldCarry; }
  const depth = exact.applyDepth ?? 0; exact.applyDepth = depth + 1;
  try { for (const [,entry] of staged) attach(entry); }
  finally { exact.applyDepth = depth; drainRecords(); }
  const errors = staged.flatMap(([,e]) => e.restoreError ? [e.restoreError] : []);
  return { ms: performance.now() - start, errors };
}
const t0 = performance.now();
try {
  const version = (stem === "gpu" ? exact.gpuVersion : exact.gpuVersions?.[stem]) ?? 0;
  gpu = await loadModule(version);
  await gpu.gpu_load();
  if (exact.now) gpu.gpu_seekable(true);
  replaceShaders(await loadShaders(gpu));
  api.version = version;
  loaded = true;
} catch (error) {
  gpu?.gpu_unload(); gpu = undefined;
  const failure = failureOutcome(error);
  terminalRecovery({...failure, status:"no device"});
  reportRecovery(Object.assign(new Error(failure.error), {code:failure.code}), true);
}
if (loaded) { loadMs = performance.now() - t0; if (stem === "gpu") exact.root.dataset.gpuMs = loadMs.toFixed(1); }
try {
  const waiting = [...surfaces.values()];
  const report = error => { exact.devError?.(String(error)); console.error("exact gpu:", error); };
  // Routed, the router delivered this artifact's queued surfaces at registration.
  if (artifact === null) {
    for (const s of exact.pendingSurfaces ?? []) if (s.generation === exact.generation) {
      try { api.surface(s.id, s.name, s.values); } catch (error) { report(error); }
    }
    exact.pendingSurfaces = [];
    for (const p of exact.pendingPosts ?? []) if (p.generation === exact.generation) api.post(p.name, p.text, p.at);
    exact.pendingPosts = [];
  }
  // A refused initial surface must not prevent independent canvases from loading.
  for (const entry of waiting) { try { ensure(entry); } catch (error) { report(error); } }
} finally { finishReady(loaded); }
