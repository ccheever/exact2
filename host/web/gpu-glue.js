// The GPU module's page side (host code, LLP 1009 D2/D4): injected by glue.js
// after the first painted frame, only when the page has a canvas. It fetches
// the app's GPU wasm (wgpu on the browser's WebGPU, wasm-bindgen glue) and
// runs each canvas's surface: bind on new inputs, render while dirty or
// wanted, resize from the element's box and devicePixelRatio.
import { pacer } from "./pace.js";
import { assetDelivery, assetName } from "./gpu-assets.js";
let gpu;
const WORLD_LIMIT = 256 * 1024 * 1024;
const worldSize = bytes => { if (bytes.length > WORLD_LIMIT) throw new Error("world carrier exceeds 256 MiB limit"); return bytes; };
let terminalRestoreReported = false;
const restoreJournal = [];

const exact = globalThis.exact;
const publishers = new Map(); // name -> first live entry
const pendingRecords = [];
let drainingRecords = false;
let planCarries = new Map();
let planStage = null;
let swapRequest = 0;
const reload = { host: "web", capability: "transactional-game-replacement", phase: "idle", intent: null,
  loaded: null, requested: null, lastSuccessfulSwap: null, restoreOutcome: null, attempts: 0, failures: 0, stale: 0, buildRequests:0, buildFailures:0, successes:0,
  native: { ui: "restart-with-carry", game: "rebuild-relaunch", liveGameReplacement: false },
  firstFrameMeaning: "rendering opportunity after GPU submission; not GPU completion or scanout" };
const diagnostics = () => structuredClone(reload);
const surfaces = new Map(); // view id -> surface, input listeners and journal cursor
const inputStyle = document.createElement("style");
inputStyle.textContent = "[data-gpu-input]:focus{outline:none}";
document.head.append(inputStyle);
let loaded = false, owned = false;
const deviceModules = new WeakSet();
let recoveringDevice;
let pendingCutover;
let recoveryTimer, recoveryFailures = 0, lossDuringRecovery = false;
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
    if (pending.checkpoint) checkpointReply(entry, "surface-load:error", entry.restoreError);
    return;
  }
  const world = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({op:"state"})) || "null")?.world;
  if (world && !world.restored) return;
  if (pending.carrier?.worldCarry === pending.bytes) {
    delete pending.carrier.worldCarry; delete pending.carrier.worldMode;
    if (pending.carrier === exact) { delete globalThis.exactWorldCarry; delete globalThis.exactWorldMode; }
  }
  delete entry.pendingRestore; delete entry.attemptedCarry; delete entry.carry;
  delete entry.restoreError; delete entry.restoreReported;
  entry.restoredCarry = true;
  entry.resampleHeld?.();
  if (pending.checkpoint) checkpointReply(entry, "surface-load:loaded");
}
async function settled() {
  await ready;
  await recoveringDevice;
  const pending = await delivery.settled(() => surfaces.values());
  if (!pending.length) {
      // Agent operations return after presentation reaches the committed clock,
      // including a child-text update published by the rendered world.
      if (exact.now) for (let pass = 0; pass < 3; pass++) {
        let drew = false;
        for (const entry of surfaces.values()) if (entry.id && (gpu.gpu_dirty(entry.id) || entry.renderedAt !== exact.now())) {
          render(entry, exact.now()); drew = true;
        }
        if (!drew) break;
      }
      if (exact.gpu.recovery?.status === "recovered") exact.gpu.recovery.instances = [...surfaces.values()].filter(e => e.id).map(e => ({id:e.id, preparation:JSON.parse(gpu.gpu_agent(e.id, '{"op":"state"}') || "null")}));
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
const clockFor = (frameNow) => exact.clockNow?.() ?? exact.now?.() ?? frameNow;

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
  const children = [...(entry.host.children ?? [])].filter(el => !el.hasAttribute("data-surface"));
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
  cancelAssets(old); old.observer?.disconnect(); old.checkpointObserver?.disconnect(); old.unlisten?.();
  old.el.replaceWith(entry.el);
  if (old.host === old.el) { entry.host = entry.el; exact.views.set(entry.view, entry.el); }
}
function recoverDevice() {
  if (recoveringDevice || recoveryTimer || recoveryFailures >= 5 || !loaded) return recoveringDevice;
  lossDuringRecovery = false;
  const module = gpu, entries = [...surfaces.values()].filter(e => e.id && !e.headless);
  const staged = pendingCutover?.module === module ? pendingCutover.staged : entries.map(old => [old, {...old, el:replacementCanvas(old), observer:null, checkpointObserver:null, unlisten:null}]);
  const detached = new Set();
  recoveringDevice = (async () => {
    const outcome = pendingCutover?.module === module ? pendingCutover.outcome : JSON.parse(await module.gpu_recover(new Uint32Array(entries.map(e => e.id)), staged.map(([,e]) => e.el)));
    if (gpu !== module) { for (const [, e] of staged) e.el.remove(); return; }
    if (["healthy", "no device"].includes(outcome.status)) {
      for (const [, e] of staged) e.el.remove();
      exact.gpu.recovery = outcome; recoveryFailures = 0; return;
    }
    if (outcome.status !== "recovered") throw new Error(JSON.stringify(outcome));
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
    exact.gpu.recovery = outcome;
    recoveryFailures = 0;
    schedule();
  })().catch(error => {
    for (const [old, entry] of staged) {
      cancelAssets(entry); entry.observer?.disconnect(); entry.checkpointObserver?.disconnect(); entry.unlisten?.();
      if (detached.has(old) && surfaces.has(old.view)) {
        entry.el.replaceWith(old.el); surfaces.set(old.view, old);
        if (publishers.get(old.name) === entry) publishers.set(old.name, old);
        if (old.host === old.el) exact.views.set(old.view, old.el);
      }
      if (detached.has(old) && live(old.view) === old) {
        old.observer?.disconnect(); old.checkpointObserver?.disconnect(); old.unlisten?.(); attach(old);
      }
      if (!pendingCutover) { entry.el.width = 0; entry.el.height = 0; entry.el.remove(); }
    }
    recoveryFailures++;
    if (recoveryFailures < 5) recoveryTimer = setTimeout(() => { recoveryTimer = null; recoverDevice(); }, 100 * 2 ** (recoveryFailures - 1));
    exact.gpu.recovery = {status:"failed", error:String(error)};
    exact.devError?.(String(error)); console.error("exact gpu recovery:", error);
  }).finally(() => { recoveringDevice = null; if (lossDuringRecovery && !recoveryFailures) queueMicrotask(recoverDevice); });
  return recoveringDevice;
}

function render(entry, now) {
  if ((hidden && !exact.now) || (recoveringDevice && !entry.headless)) return;
  const { w, h, s } = size(entry.el);
  const pw = Math.max(1, Math.round(w * s)), ph = Math.max(1, Math.round(h * s));
  if (entry.el.width !== pw || entry.el.height !== ph) { entry.el.width = pw; entry.el.height = ph; }
  supplyChildren(entry);
  if (entry.headless) {
    const reply = JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:"clock", now:clockFor(now), width:w, height:h})) || "null");
    if (reply?.error) { entry.wants = false; exact.devError?.(reply.error); return; }
    entry.renderedAt = clockFor(now); entry.wants = true;
    entry.firstTickMs ??= performance.now();
    messages(entry); return;
  }
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
    if (!entry.id) continue;
    if (entry.wants || gpu.gpu_dirty(entry.id)) render(entry, at);
    more ||= entry.wants;
  }
  // Under the agent's clock a frame is asked for by `clock`, never by the
  // last frame: a surface that wants more renders again when time moves.
  if (more && !exact.now) schedule();
}

function schedule() { if ((!hidden || exact.now) && raf === null) raf = requestAnimationFrame(frame); }

// Reload values follow the engine's field declaration order. Named bindings keep
// their own shape (including {}); property enumeration never defines ABI order.
function reloadValues(entry, report) {
  if (Array.isArray(entry.values)) return report.values;
  if (!Array.isArray(report.names) || report.names.length !== report.values.length) throw new Error(`surface ${entry.name}: reload field names unavailable; rebuild required`);
  return Object.fromEntries(Object.keys(entry.values).map(name => {
    const index = report.names.indexOf(name);
    if (index < 0) throw new Error(`surface ${entry.name}: reload field ${name} unavailable`);
    return [name, report.values[index]];
  }));
}
function bindingValues(entry, values) {
  const requested = structuredClone(values);
  const changed = key => JSON.stringify(values[key]) !== JSON.stringify(entry.requestedValues?.[key]);
  const keys = entry.setupKeys ?? [];
  if (!keys.some(changed)) {
    const rows = Object.keys(values).map(key => [key, changed(key) ? values[key] : entry.values[key]]);
    values = Array.isArray(values) ? rows.map(([,value]) => value) : Object.fromEntries(rows);
  }
  entry.requestedValues = requested;
  return entry.values = values;
}

// Creation/binding is a hard result. Staging never publishes or attaches listeners.
function create(entry, module, carry, mode = 1) {
  const { w, h, s } = size(entry.host);
  entry.el.width = Math.max(1, Math.round(w * s));
  entry.el.height = Math.max(1, Math.round(h * s));
  entry.headless = !deviceModules.has(module);
  entry.id = entry.headless ? module.gpu_create_headless(entry.name) : module.gpu_create(entry.name, entry.el, entry.el.width, entry.el.height);
  if (!entry.id) throw new Error(`surface ${entry.name}: create: ${module.gpu_error()}`);
  module.gpu_lifecycle(entry.id, hidden ? 0 : 1);
  if (!module.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.())) throw new Error(`surface ${entry.name}: bind: ${module.gpu_error()}`);
  entry.boundMs = performance.now();
  const initial = JSON.parse(module.gpu_agent(entry.id, '{"op":"state"}') || "null")?.world;
  entry.stateful = Boolean(initial);
  entry.deviceFree = initial?.presentation === "none";
  entry.headless ||= entry.deviceFree;
  if (initial?.tick > 0) entry.firstTickMs = entry.boundMs;
  if (!entry.stateful) entry.stateful = module.gpu_carry(entry.id) !== undefined;
  if (exact.now && entry.stateful) {
    const owner = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({op:"clock",owner:"agent",now:exact.now()})) || "null");
    entry.ownership = owner?.ownership ?? {unavailable:"module does not acknowledge clock owner"};
  }
  if (carry !== undefined) {
    entry.carry = carry;
    if (module.gpu_restore(entry.id, worldSize(carry), mode)) {
      entry.pendingRestore = {bytes:carry};
      finishRestore(entry, module);
    }
    else entry.restoreError = `surface ${entry.name}: restore refused: ${module.gpu_error().replace(/^restore refused: /, "")}`;
  }
}
function attach(entry, staging = false) {
  if (!staging && entry.stagedPublication !== undefined) {
    if (publishers.get(entry.name) === entry) surfaceRecord(entry.name, entry.stagedPublication);
    delete entry.stagedPublication;
  }
  entry.observer = new ResizeObserver(() => { if (live(entry.view) === entry) render(entry, frameAt ?? performance.now()); });
  entry.observer.observe(entry.el);
  watchCheckpoints(entry);
  entry.wantsInput = gpu.gpu_wants_input(entry.id);
  if (entry.wantsInput) listen(entry);
  if (!staging) { reportRestore(entry); messages(entry); schedule(); }
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
  if (module.gpu_restore(entry.id, worldSize(carrier.worldCarry), carrier.worldMode === "carry" ? 1 : 0)) {
    entry.pendingRestore = {bytes:carrier.worldCarry, carrier};
    finishRestore(entry, module);
  } else entry.restoreError = `surface ${entry.name}: restore refused: ${module.gpu_error().replace(/^restore refused: /, "")}`;
  return true;
}

function ensure(entry) {
  if (entry.id || !owned) return;
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
  if (record !== undefined && live(entry.view) === entry && publishers.get(entry.name) === entry) {
    surfaceRecord(entry.name, record); entry.firstPublicationMs ??= performance.now();
  }
  const texts = gpu.gpu_messages(entry.id);
  if (texts === undefined) return;
  for (const text of JSON.parse(texts)) {
    if (live(entry.view) !== entry) break;
    if (text !== "exact:audio") exact.message(entry.host, text);
  }
}
function checkpointKey(entry) {
  const app = exact.compat?.inputs?.app;
  if (typeof app !== "string" || !app) throw new Error("missing app identity");
  return `exact.surface.${JSON.stringify([app, entry.name])}`;
}
function checkpointReply(entry, text, error) {
  if (error) console.warn(`exact: ${text}:`, String(error));
  queueMicrotask(() => { if (live(entry.view) === entry) exact.message(entry.host, text); });
}
function checkpointToken(entry, attribute) {
  const token = Number(entry.host.getAttribute(attribute) ?? 0);
  return Number.isSafeInteger(token) && token >= 0 ? token : 0;
}
function checkpoint(entry, kind) {
  const attribute = kind === "save" ? "surface-save" : "surface-load";
  const field = kind === "save" ? "saveToken" : "loadToken";
  const token = checkpointToken(entry, attribute);
  if (entry[field] === token) return;
  entry[field] = token;
  if (!token) return;
  try {
    if (!entry.id) throw new Error("surface is not ready");
    if (publishers.get(entry.name) !== entry) throw new Error("duplicate canvas cannot checkpoint");
    if (entry.pendingRestore) throw new Error("surface restore is pending");
    const key = checkpointKey(entry);
    if (kind === "save") {
      const bytes = gpu.gpu_carry(entry.id);
      if (bytes === undefined) throw new Error("surface carries no state");
      worldSize(bytes);
      let encoded = "";
      for (let i = 0; i < bytes.length; i += 8192) encoded += String.fromCharCode(...bytes.subarray(i, i + 8192));
      localStorage.setItem(key, btoa(encoded));
      checkpointReply(entry, "surface-save:saved");
      return;
    }
    const encoded = localStorage.getItem(key);
    if (encoded === null) throw new Error("no saved game");
    if (encoded.length > Math.ceil(WORLD_LIMIT / 3) * 4) throw new Error("world carrier exceeds 256 MiB limit");
    const raw = atob(encoded);
    const bytes = worldSize(Uint8Array.from(raw, c => c.charCodeAt(0)));
    if (!gpu.gpu_restore(entry.id, bytes, 0)) throw new Error(gpu.gpu_error() || "surface refused saved state");
    entry.pendingRestore = {bytes, checkpoint:true};
    messages(entry); schedule();
  } catch (error) {
    checkpointReply(entry, `surface-${kind}:error`, error);
  }
}
function watchCheckpoints(entry) {
  entry.saveToken ??= 0;
  entry.loadToken ??= 0;
  entry.checkpointObserver = new MutationObserver((records = []) => {
    if (live(entry.view) !== entry) return;
    if (records.some(record => record.attributeName === "surface-save")) checkpoint(entry, "save");
    if (records.some(record => record.attributeName === "surface-load")) checkpoint(entry, "load");
  });
  entry.checkpointObserver.observe(entry.host, { attributes: true, attributeFilter: ["surface-save", "surface-load"] });
  queueMicrotask(() => { if (live(entry.view) === entry) { checkpoint(entry, "save"); checkpoint(entry, "load"); } });
}
function listen(entry) {
  const el = entry.host, listeners = [];
  const contacts = new Set();
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
      if (phase === "up" || phase === "cancel") { controls.delete(event.pointerId); if (owner.node === document.activeElement) el.focus({preventScroll:true}); }
      event.preventDefault(); return;
    }
    if (!fallsThrough(event)) return;
    if (phase === "down") { contacts.add(event.pointerId); if (!editable(document.activeElement)) el.focus({ preventScroll: true }); try { el.setPointerCapture(event.pointerId); } catch {} }
    if (phase === "up" || phase === "cancel") contacts.delete(event.pointerId);
    send(event, { t: "pointer", phase, id: event.pointerId, ...point(event), kind: event.pointerType || "mouse", buttons: event.buttons });
  });
  on("lostpointercapture", event => {
    if (contacts.delete(event.pointerId)) send(event, {t:"pointer", phase:"cancel", id:event.pointerId, ...point(event), buttons:0});
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
  entry.releaseInput = () => {
    if (live(entry.view) === entry) entry.resampleHeld();
    const captured = [...contacts].map(id => [el, id]).concat([...controls].map(([id, owner]) => [owner.node, id]));
    contacts.clear(); held.clear(); controls.clear(); controlKeys.clear();
    for (const [node,id] of captured) try { node?.releasePointerCapture(id); } catch {}
  };
  const blur = event => {
    entry.resampleHeld();
    for (const [id, owner] of controls) sendControl(event, owner, "cancel", id);
    for (const [code, owner] of controlKeys) sendControl(event, owner, "cancel", code === "Space" ? 4294967294 : 4294967293);
    entry.releaseInput(); send(event, { t: "blur" });
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
    if (event.defaultPrevented || event.isComposing || event.code === "Tab" || event.metaKey || event.ctrlKey || editable(target)) return;
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
  on("focusout", event => { if (!controls.size && !el.contains(event.relatedTarget)) blur(event); });
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
  const reply = gpu.gpu_agent(entry.id, JSON.stringify({
    ...request,
    ...(exact.now && request.op === "clock" && request.ticks === undefined && request.now === undefined ? { now: exact.now() } : {}),
    ...(["state","tree","logs"].includes(request.op) ? {} : {width:w,height:h,scale:s}),
  }));
  messages(entry);
  schedule();
  if (!reply) return null;
  try {
    const value = JSON.parse(reply);
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("world reply must be an object");
    if (request.op === "state" && value.world) {
      value.world.host = { platform: "web", input: "browser-events", provenance:"unavailable: browser and CDP share DOM delivery", clock: exact.now ? "controlled" : "live", owner: exact.now ? "agent" : "human", reload: diagnostics() };
      if (entry.restoreError) value.world.restoreError = entry.restoreError;
    }
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
          moduleInstantiatedMs: Number(exact.root.dataset.worldModuleMs),
          boundMs: entry.boundMs ?? null, firstTickMs: entry.firstTickMs ?? null,
          firstPublicationMs: entry.firstPublicationMs ?? null,
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
  deviceLost() { if (recoveringDevice) lossDuringRecovery = true; else queueMicrotask(() => recoverDevice()); },
  diagnostics,
  participates: () => [...surfaces.values()].some(entry => entry.stateful),
  canvasIds: () => [...surfaces.keys()],
  requested(artifact) { swapRequest++; reload.requested = structuredClone(artifact); reload.phase = "building"; reload.buildRequests++; },
  buildFailed(error) { swapRequest++; reload.phase = "failed"; reload.error = String(error); reload.buildFailures++; },
  stagePlan,
  ownershipReport(owner) {
    const world=[];
    for (const entry of surfaces.values()) if (entry.id && entry.stateful) {
      const state = JSON.parse(gpu.gpu_agent(entry.id, '{"op":"state"}') || 'null')?.world;
      if (state?.ownership?.owner !== owner) return {error:`surface ${entry.name}: clock owner is ${state?.ownership?.owner ?? "unavailable"}; host expected ${owner}`};
      world.push({canvas:entry.view,tick:state.tick,hash:state.hash,ownership:state.ownership});
    }
    return {world};
  },
  handoff(owner, at) {
    const entries = [...surfaces.values()].filter(entry=>entry.id && entry.stateful), world=[];
    for (const entry of entries) {
      const state = JSON.parse(gpu.gpu_agent(entry.id, '{"op":"state"}') || 'null');
      if (!state?.world?.ownership) return {error:`surface ${entry.name}: input/clock handoff unsupported; rebuild the game module`};
    }
    for (const entry of entries) {
      const result = JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:"clock",owner,reload:true,now:at,releaseInput:false})) || 'null');
      if (result?.error || result?.ownership?.owner !== owner || !result?.reload) return {error:`surface ${entry.name}: handoff refused`,world};
      entry.releaseInput?.();
      if (result.reload.values) entry.values = reloadValues(entry, result.reload);
      world.push({canvas:entry.view,tick:result.tick,hash:result.hash,ownership:result.ownership,releasedInput:true});
    }
    return {world,releasedInput:true};
  },
  resumeClock(controlled) {
    if (!owned) return;
    gpu.gpu_seekable(controlled);
    if (raf !== null) { cancelAnimationFrame(raf); raf = null; }
    for (const entry of surfaces.values()) entry.wants = true;
    schedule();
  },
  drainRecords,
  agent,
  settled,
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
    if (request.op === "clock" && request.owner !== undefined) return exact.control(request.owner).then(tagged);
    if (request.op === "clock" && !exact.now) return {error:"world time is live; request clock owner agent before stepping"};
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
    if (!["layout", "state", "tree", "clock"].includes(request.op)) return { error: `world does not answer ${request.op}` };
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
    if (reply?.then) return reply.then((r) => exact.gpu.decorate(request, r));
    if (!reply || reply.error) return reply;
    if (exact.gpu.answers(request)) return restoreReply(reply);
    if (request.op === "tree") for (const node of reply.nodes ?? []) {
      const summary = agent(node.id, { op: "tree", summary: true });
      if (summary?.world) node.world = summary.world;
    }
    if (request.op === "state") {
      reply.reload = diagnostics();
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
      for (const {canvas, error} of restoreJournal.splice(0)) world.push({canvas, lines:[error]});
      if (world.length) reply.world = world;
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
    if (!el) return;
    let entry = surfaces.get(view);
    if (entry && entry.el !== el) { this.destroy(view); entry = null; } // a reload reuses ids
    if (entry && entry.name !== name) { this.destroy(view); entry = null; } // one id cannot retain another plan's surface
    if (!entry) { entry = { view, host, el, name, values, id: 0, wants: false, wantsInput: false, logCursor: 0 }; surfaces.set(view, entry);
      const staged = planStage?.get(view);
      if (staged) {
        el.replaceWith(staged.el);
        Object.assign(entry, staged, { view, host });
        planStage.delete(view);
      }
      const carried = planCarries.get(name);
      if (carried?.name === name) entry.carry = carried.bytes;
      planCarries.delete(name);
      if (!publishers.has(name)) publishers.set(name, entry);
      else console.error(`exact gpu: surface ${name}: duplicate live publisher ignored`);
      if (staged) attach(entry); else ensure(entry);
      return; }
    values = bindingValues(entry, values);
    if (entry.id) {
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
    if (entry) { entry.observer?.disconnect(); entry.checkpointObserver?.disconnect(); entry.unlisten?.(); entry.id = 0; }
    surfaces.delete(view);
    if (entry && publishers.get(entry.name) === entry) { publishers.delete(entry.name); surfaceRecord(entry.name, null); }
  },
  // A plan restart reassigns view ids. Unique surface names can carry across it;
  // duplicate instances are ambiguous and are deliberately left fresh.
  reset(carry = false) {
    planCarries = new Map();
    if (carry && !planStage) for (const entry of surfaces.values()) if (entry.id) {
      if ([...surfaces.values()].filter(e => e.name === entry.name).length !== 1) continue;
      try {
        const bytes = gpu.gpu_carry(entry.id);
        if (bytes !== undefined) planCarries.set(entry.name, { name: entry.name, bytes });
      } catch (error) { restoreJournal.push({canvas:entry.view,error:`carry refused: ${error}`}); }
    }
    if (planStage) publishers.clear();
    for (const view of [...surfaces.keys()]) this.destroy(view);
  },
  finishRestart() {
    planCarries.clear();
    if (planStage) { for (const entry of planStage.values()) if (entry.id) gpu.gpu_destroy(entry.id); planStage = null; }
  },
  // Invalidate at request time, before waiting for the shared plan mutation queue.
  swap(version, options = {}) {
    const request = ++swapRequest, current = () => request === swapRequest && (options.current?.() ?? true);
    const intent = options.intent ?? "continue";
    if (!["continue", "restart", "restore"].includes(intent)) return Promise.reject(new Error(`unknown reload intent: ${intent}`));
    reload.requested = structuredClone(options.artifact ?? { version }); reload.intent = intent;
    reload.phase = "queued"; reload.attempts++; delete reload.error;
    // Freeze caller-owned bytes/values before an async build can yield.
    const checkpoints = options.checkpoints && new Map([...options.checkpoints].map(([id, bytes]) => [id, worldSize(bytes).slice()]));
    const values = options.values && structuredClone(options.values);
    return exact.mutate(() => swap(version, { ...options, intent, checkpoints, values, current }));
  },
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
    const previous = await loadShaders(module);
    const commit = () => {
      if (module !== gpu) throw new Error("GPU changed before shader commit");
      replaceShaders(rows, module);
    };
    commit.rollback = () => replaceShaders(previous, module);
    return commit;
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
  layout() { for (const entry of surfaces.values()) if (entry.id) { supplyChildren(entry); placeChildren(entry); } schedule(); },
  /// Time moved (the agent's `clock`): render what wants a frame, once.
  schedule() { for (const entry of surfaces.values()) if (entry.id && entry.wants) { entry.wants = false; gpu.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.()); } schedule(); },
};

function shaderRows(assets, module = gpu) {
  const names = new Set(JSON.parse(module.gpu_shader_names?.() ?? "[]")), decoder = new TextDecoder("utf-8", { fatal: true });
  return [...assets].filter(([path]) => path.startsWith("shaders/") && path.endsWith(".wgsl"))
    .map(([path, card]) => [path.slice(8, -5), decoder.decode(card.bytes)])
    .filter(([name]) => names.has(name));
}
function replaceShaders(rows, module = gpu) {
  module.gpu_shaders_clear?.();
  for (const [name, text] of rows) if (!module.gpu_shader(name, text)) throw new Error(module.gpu_error());
}
async function loadShaders(module) {
  const rows = [];
  if (exact.devAssets === null) for (const name of JSON.parse(module.gpu_shader_names?.() ?? "[]")) {
    const r = await fetch(new URL(`./shaders/${name}.wgsl`, import.meta.url));
    if (!r.ok) throw new Error(`shaders/${name}.wgsl: HTTP ${r.status}`);
    rows.push([name, await r.text()]);
  }
  return exact.devAssets === null ? rows : shaderRows(exact.devAssets, module);
}
// Only development uses function-scoped bindgen glue: no immortal module-map
// entry owns a candidate's Wasm memory. Production remains the static ES module.
async function loadModule(version) {
  if (exact.loadGpuModule) return exact.loadGpuModule(version);
  if (!version) {
    const module = await import("./gpu.js");
    await module.default({ module_or_path: new URL("./gpu_bg.wasm", import.meta.url) });
    if (typeof module.gpu_child_view !== "function") throw new Error("GPU module is missing gpu_child_view");
    return module;
  }
  throw new Error("verified development GPU loader unavailable; rebuild/relaunch required");
}
// All calls below the capture boundary are synchronous. No live input, clock,
// DOM publication, save observer or audio executor can interleave with staging.
function validateStage(entry, module, at, releaseInput = true, assets = exact.devAssets, budget = {assets:0, bytes:0, publications:0, ops:0}) {
  // Candidate declarations (including texture dependencies) must finish before
  // restore/rebase/publication validation. Verified development bytes are already
  // resident, so no asynchronous input can cross the capture-to-cutover boundary.
  for (let round = 0; ; round++) {
    const {requests:names, retired} = JSON.parse(module.gpu_assets(entry.id));
    for (const name of retired) if (!assetName(name)) throw new Error(`surface ${entry.name}: invalid retired asset name ${name}`);
    if (!names.length) break;
    if (round === 16) throw new Error(`surface ${entry.name}: candidate assets exceeded 16 delivery rounds`);
    for (const name of names) {
      if (++budget.assets > 256) throw new Error(`surface ${entry.name}: candidate exceeds 256 asset deliveries`);
      if (!assetName(name)) throw new Error(`surface ${entry.name}: invalid asset name ${name}`);
      const bytes = assets instanceof Map ? assets.get(`assets/${name}`)?.bytes : undefined;
      if (!bytes) throw new Error(`surface ${entry.name}: candidate asset ${name} is unavailable`);
      if (bytes.length > 64 * 1024 * 1024) throw new Error(`surface ${entry.name}: asset ${name} exceeds 64 MiB limit`);
      budget.bytes += bytes.length;
      if (budget.bytes > WORLD_LIMIT) throw new Error(`surface ${entry.name}: candidate assets exceed 256 MiB shared budget`);
      if (!module.gpu_asset(entry.id, name, bytes)) throw new Error(`surface ${entry.name}: asset ${name}: ${module.gpu_error()}`);
    }
  }
  finishRestore(entry, module);
  if (entry.restoreError) throw new Error(entry.restoreError);
  if (entry.pendingRestore) throw new Error(`surface ${entry.name}: restore remains pending after asset delivery`);
  const {w,h,s} = size(entry.host);
  const state = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({ op:"state" })) || "null");
  if (state?.error) throw new Error(`surface ${entry.name}: validation: ${state.error}`);
  const failed = state?.world?.assets?.find(asset => asset.state === "Failed");
  if (failed) throw new Error(`surface ${entry.name}: asset ${failed.name}: ${failed.reason ?? "preparation failed"}`);
  if (state?.world?.loading?.length) throw new Error(`surface ${entry.name}: assets still pending: ${state.world.loading.join(", ")}`);
  if (state?.world && releaseInput) {
    const reply = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({ op:"clock", reload:true, now:at, releaseInput:true })) || "null");
    if (!reply?.reload) throw new Error(`surface ${entry.name}: reload input/clock handoff unsupported; rebuild the game module`);
    if (reply.error) throw new Error(`surface ${entry.name}: reload: ${reply.error}`);
    const values = reloadValues(entry, reply.reload);
    const setup = reply.reload.setupIndices ?? [];
    entry.setupKeys = Array.isArray(values) ? setup.map(String) : setup.map(i => reply.reload.names[i]);
    entry.reloadReport = { rebased:reply.reload.rebased, releasedInput:reply.reload.releasedInput,
      authored:state.world.reload ?? null,
      resetFields:Object.keys(values).filter(key => !entry.setupKeys.includes(key) && JSON.stringify(values[key]) !== JSON.stringify(entry.requestedValues?.[key])),
      constructionMeaning:"Continue decodes saved state, then merges current authored values; Restart starts fresh" };
    entry.values = values;
    delete entry.restoredCarry;
  } else if (!state?.world && module.gpu_carry(entry.id) !== undefined) {
    throw new Error(`surface ${entry.name}: stateful executor has no safe staging contract`);
  }
  supplyChildren(entry, module, true);
  if (!entry.headless && module.gpu_render(entry.id, w, h, s, at) >= 2) throw new Error(`surface ${entry.name}: render: ${module.gpu_error()}`);
  const after = JSON.parse(module.gpu_agent(entry.id, '{"op":"state"}') || "null")?.world;
  if (after?.ready === false) throw new Error(`surface ${entry.name}: candidate not ready: ${JSON.stringify(after.readyReasons)}`);
  const publication = module.gpu_published(entry.id);
  if (publication !== undefined) {
    budget.publications += publication.length;
    if (budget.publications > 32 * 1024 * 1024) throw new Error("candidate publications exceed 32 MiB shared text limit");
    const value = JSON.parse(publication);
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`surface ${entry.name}: invalid publication`);
    entry.stagedPublication = publication;
  }
  // Candidate messages are irreversible app inputs. Discard them, including
  // audio-triggering messages; the committed world republishes its HUD record.
  const messages = module.gpu_messages(entry.id);
  if (messages !== undefined && !Array.isArray(JSON.parse(messages))) throw new Error(`surface ${entry.name}: invalid messages`);
}
function candidateEntry(old, values = old.values) {
  return { ...old, controls:new Map(), controlKeys:new Map(), heldKeys:new Set(), children:undefined,
    pendingRestore:undefined, restoreReported:undefined, resampleHeld:undefined, releaseInput:undefined, values, requestedValues:structuredClone(values), el: old.el.cloneNode(false), id:0, observer:null, checkpointObserver:null, unlisten:null,
    restoreError:undefined, attemptedCarry:undefined, firstFrameMs:undefined, firstFrameSubmittedMs:undefined,
    restoredCarry:false, wants:true, logCursor:0 };
}
function disposeStage(module, entries, unload = false) {
  for (const entry of entries) if (entry.id) { module.gpu_destroy(entry.id); entry.id = 0; }
  if (unload) module.gpu_unload?.();
}
function rebaseStage(module, entries) {
  const at = clockFor(frameAt ?? performance.now());
  for (const entry of entries) if (entry.reloadReport) {
    const reply = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({op:"clock",reload:true,now:at,releaseInput:false})) || "null");
    if (!reply?.reload || reply.error) throw new Error(`surface ${entry.name}: final clock rebase refused`);
  }
}
function validatePublications(batch, staged, module, at, mount = true, assets = exact.devAssets, budget = {assets:0, bytes:0, publications:0, ops:0}) {
  const published = new Map(), ops = [...batch.ops];
  for (let round = 0; round < 16; round++) {
    const bindings = new Map();
    for (const entry of staged.values()) {
      const json = entry.stagedPublication;
      delete entry.stagedPublication;
      if (json === undefined || published.get(entry.name) === json) continue;
      const result = exact.stageSurfaceRecord(entry.name, json);
      if (result.error) throw new Error(`surface ${entry.name}: publication: ${result.error}`);
      published.set(entry.name, json);
      if (result.ops.some(op => ["store", "command", "storage", "request"].includes(op.op))) throw new Error("candidate publication has irreversible effects; restart required");
      for (const op of result.ops) {
        if (op.op === "destroy" && staged.has(op.id)) throw new Error("candidate publication removed a staged canvas; restart required");
        if (op.op === "surface") {
          if (staged.get(op.id)?.name !== op.name) throw new Error("candidate publication changed the canvas roster; restart required");
          bindings.set(op.id, op.values);
        }
      }
      budget.ops += result.ops.length;
      if (budget.ops > 65536) throw new Error("candidate publications exceed 65536 shared operations");
      ops.push(...result.ops);
      batch.timers = result.timers ?? batch.timers;
      batch.clock = result.clock ?? batch.clock;
    }
    // Apply the complete round's bindings together: one world's HUD can bind
    // another world, and later publications can refine that same binding.
    for (const [id, values] of bindings) {
      const entry = staged.get(id);
      if (JSON.stringify(values) === JSON.stringify(entry.requestedValues)) continue;
      if (!module.gpu_bind_at(entry.id, JSON.stringify(bindingValues(entry, values)), at)) throw new Error(`surface ${entry.name}: publication bind: ${module.gpu_error()}`);
      validateStage(entry, module, at, false, assets, budget);
    }
    if (![...staged.values()].some(entry => entry.stagedPublication !== undefined)) {
      // Present only the final bindings; intermediate ones were already applied
      // privately. The HUD deltas join the initial boot in one outer batch.
      batch.ops = ops.filter(op => op.op !== "surface");
      if (mount) for (const entry of staged.values()) batch.ops.push({op:"surface",id:entry.view,name:entry.name,values:entry.requestedValues ?? entry.values});
      return;
    }
  }
  throw new Error("candidate publications and bindings did not settle after 16 rounds");
}
function successfulSwap(start, artifact, entries, intent, extra = {}) {
  const committed = performance.now();
  reload.successes++; reload.phase = "committed"; reload.loaded = structuredClone(artifact); delete reload.error;
  reload.restoreOutcome = { status: intent === "restart" ? "fresh" : "restored", canvases: entries.map(e => ({canvas:e.view,name:e.name,...e.reloadReport})) };
  const success = { artifact: structuredClone(artifact), intent, committedAt:Date.now(),
    timing:{ ...extra, prepareToCommitMs:committed-start, firstUsableFrameMs:null,
      editToCommitMs:extra.detectedAt ? Date.now()-extra.detectedAt : null, buildCompleteToCommitMs:extra.buildCompleteAt ? Date.now()-extra.buildCompleteAt : null } };
  reload.lastSuccessfulSwap = success;
  requestAnimationFrame(() => requestAnimationFrame(() => {
    if (reload.lastSuccessfulSwap !== success) return;
    if (entries.some(entry=>live(entry.view)!==entry)) { success.timing.firstFrameUnavailable = "participating canvas retired before rendering opportunity"; return; }
    success.timing.firstUsableFrameMs = performance.now()-start;
    success.timing.editToFirstUsableFrameMs = extra.detectedAt ? Date.now()-extra.detectedAt : null;
    if (reload.phase === "committed") reload.phase = "usable";
    exact.gpu.onUsable?.(structuredClone(success));
  }));
}
// A candidate Contract boot has not touched the DOM. Names only identify a
// world when unique on BOTH sides; duplicates never silently lose their carry.
function stagePlan(batch, beforeSlots = {}, afterSlots = {}, assets = exact.devAssets) {
  if (!owned) throw new Error("surface module is not ready for a transactional plan restart");
  if (batch.ops.some(op => ["store", "command", "storage"].includes(op.op))) throw new Error("candidate plan has irreversible effects; restart required");
  const rows = batch.ops.filter(op => op.op === "surface"), staged = new Map(), start = performance.now();
  const old = [...surfaces.values()], at = clockFor(frameAt ?? start);
  const requestedWorld = exact.worldCarry, carrier = {worldCarry:requestedWorld, worldMode:exact.worldMode};
  if (old.length > 256 || rows.length > 256) throw new Error("candidate exceeds 256 canvas limit");
  const budget = {assets:0, bytes:0, publications:0, ops:batch.ops.length};
  reload.requested = { ...reload.loaded, plan:exact.devPlanArtifact ?? null }; reload.intent = "continue"; reload.phase = "staging"; reload.attempts++;
  gpu.gpu_seekable(true);
  try {
    for (const row of rows) {
      const matches = old.filter(e => e.name === row.name);
      if (matches.length > 1 || rows.filter(r => r.name === row.name).length > 1) throw new Error(`surface ${row.name}: ambiguous duplicate identity; restart required`);
      const previous = matches[0];
      const entry = previous ? candidateEntry(previous, row.values) : {
        view:row.id, name:row.name, values:row.values, requestedValues:structuredClone(row.values), el:document.createElement("canvas"),
        host:{getBoundingClientRect:()=>({width:1,height:1})}, id:0, wants:true, logCursor:0,
      };
      entry.view = row.id; staged.set(row.id, entry);
      create(entry, gpu, carrier.worldCarry !== undefined ? undefined : previous?.id ? gpu.gpu_carry(previous.id) : undefined);
      // Initial plan + checkpoint is an explicit saved-input launch. Consume its
      // one-shot carrier privately; an ordinary authored reload releases input.
      const checkpoint = restorePending(entry, gpu, carrier) === true;
      validateStage(entry, gpu, at, !checkpoint, assets, budget);
    }
    for (const entry of old) if (entry.id && !rows.some(row => row.name === entry.name) && gpu.gpu_carry(entry.id) !== undefined) {
      throw new Error(`surface ${entry.name}: participating world removed; explicit restart required`);
    }
    validatePublications(batch, staged, gpu, at, true, assets, budget);
    rebaseStage(gpu, staged.values());
  } catch (error) { disposeStage(gpu, staged.values()); reload.phase = "failed"; reload.error = String(error); reload.failures++; reload.restoreOutcome = {status:"refused",retained:true,reason:String(error)}; throw error; }
  finally { gpu.gpu_seekable(Boolean(exact.now)); }
  planStage = staged;
  return {
    abort() { disposeStage(gpu, staged.values()); planStage = null; },
    commit() {
      if (requestedWorld !== undefined && carrier.worldCarry === undefined && exact.worldCarry === requestedWorld) {
        delete exact.worldCarry; delete exact.worldMode; delete globalThis.exactWorldCarry; delete globalThis.exactWorldMode;
      }
      successfulSwap(start, { ...reload.loaded, plan:exact.devPlanArtifact ?? null }, [...surfaces.values()], "continue");
      reload.restoreOutcome.ui = {scope:"named root slots; row-local state omitted",resetFields:[...new Set([...Object.keys(beforeSlots),...Object.keys(afterSlots)])]
        .filter(name=>JSON.stringify(beforeSlots[name])!==JSON.stringify(afterSlots[name]))
        .map(name=>({name,outcome:!(name in beforeSlots)?"initialized":!(name in afterSlots)?"removed":"reset"}))};
    },
  };
}
async function swap(version, options) {
  const { intent, current } = options;
  await ready;
  await recoveringDevice;
  if (!current()) return {ms:0,errors:[],stale:true};
  const start = performance.now(), staged = [];
  let next, hostStage, hostCommitted = false;
  const oldModule = gpu, detached = new Set();
  const assertCurrent = () => { if (!current()) throw Object.assign(new Error("obsolete reload candidate"), {stale:true}); };
  try {
    reload.phase = "loading";
    next = await loadModule(version);
    assertCurrent();
    for (const name of ["load_headless","create_headless","unload","destroy","bind_at","restore","carry","agent","seekable","published","messages"]) if (typeof next[`gpu_${name}`] !== "function") throw new Error(`GPU ABI missing gpu_${name}; rebuild/relaunch required`);
    if (next.gpu_load) { await next.gpu_load(); deviceModules.add(next); }
    else next.gpu_load_headless();
    assertCurrent();
    // The game Presentation trait discards audio under this flag. No physical
    // input, durable checkpoint observer or app message ingress is attached.
    next.gpu_seekable(true);
    if (sentPeriod) next.gpu_period(sentPeriod);
    const rows = await loadShaders(next);
    for (const [name, text] of rows) if (!await next.gpu_shader_check(name, text)) throw new Error(next.gpu_error());
    assertCurrent();
    replaceShaders(rows, next);
    await recoveringDevice;
    assertCurrent();
    if (gpu !== oldModule) throw new Error("GPU changed during candidate preparation; retry");
    reload.phase = "staging";
    const at = clockFor(frameAt ?? performance.now());
    if (surfaces.size > 256) throw new Error("candidate exceeds 256 canvas limit");
    const budget = {assets:0, bytes:0, publications:0, ops:0};
    const carrier = {worldCarry:exact.worldCarry, worldMode:exact.worldMode};
    for (const old of surfaces.values()) {
      const entry = candidateEntry(old, options.values?.get(old.view) ?? old.requestedValues ?? old.values);
      staged.push([old, entry]);
      let carry;
      if (intent === "continue") carry = old.id ? gpu.gpu_carry(old.id) : old.carry;
      if (intent === "restore") {
        carry = options.checkpoints?.get(old.view);
        if (carry === undefined) throw new Error(`surface ${old.name}: selected checkpoint missing`);
      }
      create(entry, next, carry, intent === "restore" ? 0 : 1);
      if (intent === "continue" && carry === undefined) restorePending(entry, next, carrier);
      validateStage(entry, next, at, true, exact.devAssets, budget);
    }
    if (staged.some(([,entry])=>entry.stateful || entry.stagedPublication !== undefined)) {
      if (new Set(staged.map(([,entry])=>entry.name)).size !== staged.length) throw new Error("ambiguous duplicate surface identity; restart required");
      hostStage = exact.stageCurrent();
      validatePublications(hostStage.batch, new Map(staged.map(([,entry])=>[entry.view,entry])), next, at, false, exact.devAssets, budget);
    }
    assertCurrent();
    rebaseStage(next, staged.map(([,entry])=>entry));
    reload.phase = "ready";
    // Listener/DOM installation can fail. Keep every old executor alive until
    // all canvases have attached and the staged host has accepted publication.
    if (raf !== null) { cancelAnimationFrame(raf); raf = null; }
    gpu = next;
    for (const [old, entry] of staged) {
      detached.add(old);
      installCanvas(old, entry);
      if (publishers.get(old.name) === old) publishers.set(old.name, entry);
      surfaces.set(entry.view, entry);
      attach(entry, true);
    }
    const depth = exact.applyDepth ?? 0; exact.applyDepth = depth + 1;
    try { hostStage?.commit(); hostCommitted = true; hostStage?.present(); }
    finally { exact.applyDepth = depth; }
    owned = true; loaded = deviceModules.has(next); exact.gpu.version = version;
    next.gpu_seekable(Boolean(exact.now));
    for (const [old, entry] of staged) {
      // Transferred child styles still belong to the same Contract nodes.
      for (const row of old.children ?? []) if (!entry.children?.some(next => next.el === row.el)) restoreChild(row);
      for (const row of entry.children ?? []) row.original ??= old.children?.find(old => old.el === row.el)?.original;
      placeChildren(entry);
      old.releaseInput?.();
      if (old.id) { oldModule.gpu_destroy(old.id); old.id = 0; }
      delete entry.stagedPublication;
      if (entry.pendingRestore?.carrier === carrier) entry.pendingRestore.carrier = exact;
      messages(entry); assets(entry);
    }
    oldModule?.gpu_unload();
    if (carrier.worldCarry === undefined) { delete exact.worldCarry; delete exact.worldMode; delete globalThis.exactWorldCarry; delete globalThis.exactWorldMode; }
    drainRecords(); schedule();
    successfulSwap(start, exact.gpuArtifacts?.get(version) ?? options.artifact ?? {version}, staged.map(([,e])=>e), intent, options.timing);
    return { ms:performance.now()-start, errors:[], ...diagnostics() };
  } catch (error) {
    if (!hostCommitted) {
      hostStage?.abort(); gpu = oldModule;
      for (const [old, entry] of [...staged].reverse()) {
        entry.observer?.disconnect(); entry.checkpointObserver?.disconnect(); entry.unlisten?.();
        if (detached.has(old)) {
          entry.el.replaceWith(old.el); surfaces.set(old.view, old);
          if (publishers.get(old.name) === entry) publishers.set(old.name, old);
          if (old.host === old.el) exact.views.set(old.view, old.el);
          old.observer?.disconnect(); old.checkpointObserver?.disconnect(); old.unlisten?.(); attach(old);
        }
      }
    }
    if (hostCommitted) { disposeStage(oldModule, staged.map(([old])=>old), true); schedule(); }
    if (next && next !== gpu) disposeStage(next, staged.map(([,e])=>e), true);
    if (error.stale) { reload.stale++; return {ms:performance.now()-start,errors:[],stale:true}; }
    if (current()) { reload.phase = "failed"; reload.error = String(error); reload.failures++;
      reload.restoreOutcome = {status:"refused", reason:String(error), retained:!hostCommitted}; exact.devError?.(String(error)); }
    throw error;
  }
}
const t0 = performance.now();
try {
  const version = exact.gpuVersion ?? 0;
  gpu = await loadModule(version);
  exact.root.dataset.worldModuleMs = performance.now().toFixed(3);
  gpu.gpu_load_headless();
  owned = true;
  gpu.gpu_seekable(Boolean(exact.now));
  exact.gpu.version = version;
  reload.loaded = exact.gpuArtifacts?.get(version) ?? { version, identity:"unavailable: static host did not provide artifact receipt" };
} catch (error) { gpu?.gpu_unload(); gpu = undefined; console.error("exact gpu:", error); }
if (owned) exact.root.dataset.gpuMs = (performance.now() - t0).toFixed(1);
try {
  const waiting = [...surfaces.values()];
  const report = error => { exact.devError?.(String(error)); console.error("exact gpu:", error); };
  for (const s of exact.pendingSurfaces ?? []) if (s.generation === exact.generation) {
    try { exact.gpu.surface(s.id, s.name, s.values); } catch (error) { report(error); }
  }
  exact.pendingSurfaces = [];
  for (const entry of waiting) { try { ensure(entry); } catch (error) { report(error); } }
} finally { finishReady(owned); }
// Device acquisition must never hold the ownership promise or discard live worlds.
if (owned && gpu.gpu_load) {
  const module = gpu;
  Promise.resolve().then(() => module.gpu_load()).then(async () => {
    replaceShaders(await loadShaders(module), module);
    if (module !== gpu) return;
    deviceModules.add(module); loaded = true;
    for (const entry of surfaces.values()) if (entry.id && entry.headless && !entry.deviceFree) {
      if (module.gpu_attach(entry.id, entry.el, entry.el.width, entry.el.height)) {
        entry.headless = false; entry.wants = true;
      } else console.error("exact gpu attach:", module.gpu_error());
    }
    schedule();
  }).catch(error => { console.warn("exact gpu device unavailable; worlds remain active:", error); });
}
