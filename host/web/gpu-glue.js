// The GPU module's page side (host code, LLP 1009 D2/D4): injected by glue.js
// after the first painted frame, only when the page has a canvas. It fetches
// the app's GPU wasm (wgpu on the browser's WebGPU, wasm-bindgen glue) and
// runs each canvas's surface: bind on new inputs, render while dirty or
// wanted, resize from the element's box and devicePixelRatio.
import { pacer } from "./pace.js";
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
let loaded = false;
let raf = null;
let finishReady;
const ready = new Promise((resolve) => { finishReady = resolve; });

// Track the entire delivery chain, including names discovered by a delivery.
const assetFlights = new Set();
function assets(entry) {
  if (!entry.id || !gpu) return;
  const module = gpu, id = entry.id;
  for (const name of JSON.parse(module.gpu_assets(id))) {
    const task = (async () => {
      let bytes = null;
      try {
        if (exact.devAssets instanceof Map) bytes = exact.devAssets.get(`assets/${name}`)?.bytes ?? null;
        else {
          const response = await fetch(new URL(`./assets/${name}`, document.baseURI));
          if (response.ok) bytes = new Uint8Array(await response.arrayBuffer());
          else if (response.status !== 404) throw new Error(`asset ${name}: HTTP ${response.status}`);
        }
      } catch (error) { console.error("exact gpu:", error); }
      if (gpu !== module || live(entry.view) !== entry || entry.id !== id) return;
      if (!module.gpu_asset(id, name, bytes)) console.error("exact gpu:", module.gpu_error());
      messages(entry); schedule();
    })();
    assetFlights.add(task);
    task.finally(() => assetFlights.delete(task));
  }
}
async function settled() {
  await ready;
  do {
    for (const entry of surfaces.values()) assets(entry);
    if (assetFlights.size) await Promise.all([...assetFlights]);
  } while (assetFlights.size);
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
  for (const entry of surfaces.values()) if (entry.id) gpu?.gpu_lifecycle(entry.id, code);
  if (hidden && !exact.now && raf !== null) { cancelAnimationFrame(raf); raf = null; }
  if (!hidden) schedule();
}
document.addEventListener("visibilitychange", () => lifecycle(document.hidden ? 0 : 1));
window.addEventListener("pagehide", () => lifecycle(0));
window.addEventListener("pageshow", () => lifecycle(document.hidden ? 0 : 1));

function render(entry, now) {
  if (hidden && !exact.now) return;
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

// The live frame clock is the callback's timestamp paced onto the display's
// lattice (pace.js): a world drawn at the raw timestamp judders by the
// timestamp's own jitter. The agent's clock (exact.now) bypasses this in clockFor.
const pace = pacer();
let frameAt = null; // the last paced frame time: a render outside the frame loop redraws at it, never ahead of it
function frame(now) {
  raf = null;
  if (hidden && !exact.now) return;
  const at = frameAt = pace(now);
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

function bindingValues(entry, values) {
  const requested = structuredClone(values);
  const changed = index => JSON.stringify(values[index]) !== JSON.stringify(entry.requestedValues?.[index]);
  // Unchanged construction keeps its carry; a deliberate setup-counter change
  // uses all requested construction values, including the new authored scene.
  if (!(entry.setupIndices ?? []).some(changed)) values = values.map((value, i) => changed(i) ? value : entry.values[i]);
  entry.requestedValues = requested;
  return entry.values = values;
}

// Creation/binding is a hard result. Staging never publishes or attaches listeners.
function create(entry, module, carry) {
  const { w, h, s } = size(entry.host);
  entry.el.width = Math.max(1, Math.round(w * s));
  entry.el.height = Math.max(1, Math.round(h * s));
  entry.id = module.gpu_create(entry.name, entry.el, entry.el.width, entry.el.height);
  if (!entry.id) throw new Error(`surface ${entry.name}: create: ${module.gpu_error()}`);
  module.gpu_lifecycle(entry.id, hidden ? 0 : 1);
  if (!module.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.())) throw new Error(`surface ${entry.name}: bind: ${module.gpu_error()}`);
  entry.stateful = module.gpu_carry(entry.id) !== undefined;
  if (exact.now && entry.stateful) {
    const owner = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({op:"clock",owner:"agent",now:exact.now()})) || "null");
    entry.ownership = owner?.ownership ?? {unavailable:"module does not acknowledge clock owner"};
  }
  if (carry !== undefined) {
    if (module.gpu_restore(entry.id, worldSize(carry))) entry.restoredCarry = true;
    else entry.restoreError = `surface ${entry.name}: restore refused: ${module.gpu_error()}`;
  }
}
function attach(entry) {
  if (entry.stagedPublication !== undefined) {
    if (publishers.get(entry.name) === entry) surfaceRecord(entry.name, entry.stagedPublication);
    delete entry.stagedPublication;
  }
  entry.observer = new ResizeObserver(() => { if (live(entry.view) === entry) render(entry, frameAt ?? performance.now()); });
  entry.observer.observe(entry.el);
  entry.wantsInput = gpu.gpu_wants_input(entry.id);
  if (entry.wantsInput) listen(entry);
  if (entry.restoreError) {
    restoreJournal.push({ canvas: entry.view, error: entry.restoreError });
    exact.devError?.(entry.restoreError); console.error(entry.restoreError);
  }
  messages(entry); schedule();
}
function restorePending(entry, module = gpu, carrier = exact) {
  if (carrier.worldCarry === undefined || entry.attemptedCarry === carrier.worldCarry || module.gpu_carry(entry.id) === undefined) return;
  entry.attemptedCarry = carrier.worldCarry;
  if (module.gpu_restore(entry.id, worldSize(carrier.worldCarry))) {
    delete carrier.worldCarry; delete entry.attemptedCarry;
    if (carrier === exact) delete globalThis.exactWorldCarry;
    delete entry.restoreError; entry.restoredCarry = true;
  } else entry.restoreError = `surface ${entry.name}: restore refused: ${module.gpu_error()}`;
}

function ensure(entry) {
  if (entry.id || !loaded) return;
  try {
    create(entry, gpu, entry.carry);
    if (!entry.restoreError) delete entry.carry;
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
    if (candidates.length && candidates.every(e => e.id && (e.attemptedCarry === exact.worldCarry || gpu.gpu_carry(e.id) === undefined))
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
function messages(entry) {
  assets(entry);
  const record = gpu.gpu_published(entry.id);
  if (record !== undefined && live(entry.view) === entry && publishers.get(entry.name) === entry) surfaceRecord(entry.name, record);
  const texts = gpu.gpu_messages(entry.id);
  if (texts === undefined) return;
  for (const text of JSON.parse(texts)) {
    if (live(entry.view) !== entry) break;
    if (text !== "exact:audio") exact.message(entry.host, text);
  }
}

// A canvas opts into durable checkpoints with two monotonic props. The host
// stores opaque bytes; the surface alone defines and validates their format.
function checkpointKey(entry) {
  const app = exact.compat?.inputs?.app;
  if (typeof app !== "string" || !app) throw new Error("missing app identity");
  return `exact.surface.${encodeURIComponent(app)}.${encodeURIComponent(entry.name)}`;
}
function checkpointReply(entry, text, error) {
  if (error) console.warn(`exact: ${text}:`, String(error));
  if (live(entry.view) === entry) exact.message(entry.host, text);
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
    const raw = atob(encoded);
    const bytes = worldSize(Uint8Array.from(raw, c => c.charCodeAt(0)));
    if (!gpu.gpu_restore(entry.id, bytes)) throw new Error(gpu.gpu_error() || "surface refused saved state");
    messages(entry); schedule();
    checkpointReply(entry, "surface-load:loaded");
  } catch (error) {
    checkpointReply(entry, `surface-${kind}:error`, error);
  }
}
function watchCheckpoints(entry) {
  entry.saveToken ??= 0;
  entry.loadToken ??= 0;
  entry.checkpointObserver = new MutationObserver(records => {
    if (records.some(record => record.attributeName === "surface-save")) checkpoint(entry, "save");
    if (records.some(record => record.attributeName === "surface-load")) checkpoint(entry, "load");
  });
  entry.checkpointObserver.observe(entry.host, { attributes: true, attributeFilter: ["surface-save", "surface-load"] });
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
    if (!gpu.gpu_input(entry.id, JSON.stringify({ ...value, at: clockFor(event.timeStamp) }))) console.error("exact gpu:", gpu.gpu_error());
    messages(entry);
    schedule();
  };
  const fallsThrough = (event) => event.target === el || event.target === entry.el;
  const point = (event) => { const r = el.getBoundingClientRect(); return { x: event.clientX - r.left, y: event.clientY - r.top }; };
  for (const phase of ["down", "move", "up", "cancel"]) on(`pointer${phase}`, (event) => {
    if (!fallsThrough(event)) return;
    if (phase === "down") { contacts.add(event.pointerId); el.focus({ preventScroll: true }); try { el.setPointerCapture(event.pointerId); } catch {} }
    if (phase === "up" || phase === "cancel") contacts.delete(event.pointerId);
    send(event, { t: "pointer", phase, id: event.pointerId, ...point(event), kind: event.pointerType || "mouse", buttons: event.buttons });
  });
  on("wheel", (event) => {
    if (!fallsThrough(event)) return;
    event.preventDefault();
    send(event, { t: "wheel", dx: event.deltaX, dy: event.deltaY, ...point(event) });
  }, { passive: false });
  // A restored keydown (including a queued one) still owns its future keyup.
  const held = new Set(entry.restoredCarry ? agent(entry.view, {op:"state"})?.world?.input?.forwarded ?? [] : []);
  delete entry.restoredCarry;
  const editable = target => target instanceof Element && target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])');
  const blur = event => { entry.releaseInput?.(); send(event, { t: "blur" }); };
  on("keydown", event => {
    const target = event.target instanceof Element ? event.target : null;
    if (event.defaultPrevented || event.isComposing || event.code === "Tab" || event.metaKey || event.ctrlKey || editable(target)) return;
    if (["Space", "Enter"].includes(event.code) && target?.closest('button, a[href], [role="button"], [role="link"]')) return;
    if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Space", "PageUp", "PageDown", "Home", "End"].includes(event.code)) event.preventDefault();
    held.add(event.code);
    send(event, { t: "key", code: event.code, key: event.key, down: true, repeat: event.repeat });
  });
  on("keyup", event => {
    if (!held.delete(event.code)) return;
    send(event, { t: "key", code: event.code, key: event.key, down: false, repeat: false });
  });
  on("focusin", event => { if (editable(event.target)) blur(event); });
  on("focusout", event => { if (!el.contains(event.relatedTarget)) blur(event); });
  entry.releaseInput = () => {
    held.clear();
    const owned = [...contacts]; contacts.clear();
    for (const id of owned) { try { el.releasePointerCapture(id); } catch {} }
  };
  const windowBlur = () => blur({timeStamp:performance.now()});
  globalThis.addEventListener?.("blur", windowBlur);
  on("lostpointercapture", event => { if (contacts.delete(event.pointerId)) send(event, {t:"pointer",phase:"cancel",id:event.pointerId,...point(event),kind:event.pointerType || "mouse",buttons:0}); });
  entry.unlisten = () => {
    globalThis.removeEventListener?.("blur", windowBlur);
    entry.releaseInput();
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
      if (result.reload.values) entry.values = result.reload.values;
      world.push({canvas:entry.view,tick:result.tick,hash:result.hash,ownership:result.ownership,releasedInput:true});
    }
    return {world,releasedInput:true};
  },
  resumeClock(controlled) {
    if (!loaded) return;
    gpu.gpu_seekable(controlled);
    if (raf !== null) { cancelAnimationFrame(raf); raf = null; }
    for (const entry of surfaces.values()) entry.wants = true;
    schedule();
  },
  drainRecords,
  agent,
  settled,
  wantsInput: (view) => live(view)?.wantsInput === true,
  answers: (request) => request.entity !== undefined || request.world === true,
  handle(request, ask, tagged) {
    const entry = live(request.id);
    if (!entry) return { error: `view ${request.id} has no world` };
    if (request.op === "clock" && request.owner !== undefined) return exact.control(request.owner).then(tagged);
    if (request.op === "clock" && !exact.now) return {error:"world time is live; request clock owner agent before stepping"};
    if (request.op === "focus") {
      if (!entry.wantsInput) return { error: `view ${request.id}'s surface does not take input` };
      entry.host.focus({ preventScroll: true }); return tagged({ ok: document.activeElement === entry.host });
    }
    if (request.op === "screenshot" && request.form === "save") {
      const bytes = gpu.gpu_carry(entry.id);
      if (bytes === undefined) return { error: `canvas ${entry.name} carries no state` };
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
    if (!entry) { entry = { view, host, el, name, values, id: 0, wants: false, wantsInput: false, logCursor: 0 }; surfaces.set(view, entry);
      const staged = planStage?.get(view);
      if (staged) {
        el.replaceWith(staged.el);
        Object.assign(entry, staged, { view, host });
        planStage.delete(view);
      }
      watchCheckpoints(entry);
      const carried = planCarries.get(name);
      if (carried?.name === name) entry.carry = carried.bytes;
      planCarries.delete(name);
      if (!publishers.has(name)) publishers.set(name, entry);
      else console.error(`exact gpu: surface ${name}: duplicate live publisher ignored`);
      if (staged) attach(entry); else ensure(entry);
      checkpoint(entry, "save"); checkpoint(entry, "load");
      return; }
    values = bindingValues(entry, values);
    if (entry.id) { if (!gpu.gpu_bind_at(entry.id, JSON.stringify(values), exact.now?.())) console.error("exact gpu:", gpu.gpu_error()); messages(entry); schedule(); }
  },
  destroy(view) {
    const entry = surfaces.get(view);
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
      const bytes = gpu.gpu_carry(entry.id);
      if (bytes !== undefined) planCarries.set(entry.name, { name: entry.name, bytes });
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
  /// Time moved (the agent's `clock`): render what wants a frame, once.
  schedule() { for (const entry of surfaces.values()) if (entry.id && entry.wants) { entry.wants = false; gpu.gpu_bind_at(entry.id, JSON.stringify(entry.values), exact.now?.()); } schedule(); },
};

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
  if (exact.loadGpuModule) return exact.loadGpuModule(version);
  if (!version) {
    const module = await import("./gpu.js");
    await module.default({ module_or_path: new URL("./gpu_bg.wasm", import.meta.url) });
    return module;
  }
  throw new Error("verified development GPU loader unavailable; rebuild/relaunch required");
}
// All calls below the capture boundary are synchronous. No live input, clock,
// DOM publication, save observer or audio executor can interleave with staging.
function validateStage(entry, module, at, releaseInput = true) {
  if (entry.restoreError) throw new Error(entry.restoreError);
  const {w,h,s} = size(entry.host);
  const state = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({ op:"state" })) || "null");
  if (state?.error) throw new Error(`surface ${entry.name}: validation: ${state.error}`);
  if (state?.world && releaseInput) {
    const reply = JSON.parse(module.gpu_agent(entry.id, JSON.stringify({ op:"clock", reload:true, now:at, releaseInput:true })) || "null");
    if (!reply?.reload) throw new Error(`surface ${entry.name}: reload input/clock handoff unsupported; rebuild the game module`);
    if (reply.error) throw new Error(`surface ${entry.name}: reload: ${reply.error}`);
    entry.reloadReport = { rebased:reply.reload.rebased, releasedInput:reply.reload.releasedInput,
      retainedConstruction:(reply.reload.setupIndices ?? []).map(index=>({index, changed:JSON.stringify(entry.requestedValues?.[index])!==JSON.stringify(reply.reload.values?.[index])})),
      resetFields:(reply.reload.values ?? []).flatMap((value,index)=>(reply.reload.setupIndices ?? []).includes(index) || JSON.stringify(value)===JSON.stringify(entry.requestedValues?.[index]) ? [] : [index]),
      constructionMeaning:"Continue keeps instantiated setup; Restart uses requested authored values" };
    if (reply.reload.values) entry.values = reply.reload.values;
    entry.setupIndices = reply.reload.setupIndices ?? [];
    delete entry.restoredCarry;
  } else if (!state?.world && module.gpu_carry(entry.id) !== undefined) {
    throw new Error(`surface ${entry.name}: stateful executor has no safe staging contract`);
  }
  if (module.gpu_render(entry.id, w, h, s, at) === 2) throw new Error(`surface ${entry.name}: render: ${module.gpu_error()}`);
  const publication = module.gpu_published(entry.id);
  if (publication !== undefined) {
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
  return { ...old, values, requestedValues:structuredClone(values), el: old.el.cloneNode(false), id:0, observer:null, checkpointObserver:null, unlisten:null,
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
function validatePublications(batch, staged, module, at, mount = true) {
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
      validateStage(entry, module, at, false);
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
function stagePlan(batch, beforeSlots = {}, afterSlots = {}) {
  if (!loaded) throw new Error("GPU is not ready for a transactional plan restart");
  if (batch.ops.some(op => ["store", "command", "storage"].includes(op.op))) throw new Error("candidate plan has irreversible effects; restart required");
  const rows = batch.ops.filter(op => op.op === "surface"), staged = new Map(), start = performance.now();
  const old = [...surfaces.values()], at = clockFor(frameAt ?? start);
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
      create(entry, gpu, previous?.id ? gpu.gpu_carry(previous.id) : undefined);
      validateStage(entry, gpu, at);
    }
    for (const entry of old) if (entry.id && gpu.gpu_carry(entry.id) !== undefined && !rows.some(row => row.name === entry.name)) {
      throw new Error(`surface ${entry.name}: participating world removed; explicit restart required`);
    }
    validatePublications(batch, staged, gpu, at);
    rebaseStage(gpu, staged.values());
  } catch (error) { disposeStage(gpu, staged.values()); reload.phase = "failed"; reload.error = String(error); reload.failures++; reload.restoreOutcome = {status:"refused",retained:true,reason:String(error)}; throw error; }
  finally { gpu.gpu_seekable(Boolean(exact.now)); }
  planStage = staged;
  return {
    abort() { disposeStage(gpu, staged.values()); planStage = null; },
    commit() {
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
  if (!current()) return {ms:0,errors:[],stale:true};
  const start = performance.now(), staged = [];
  let next, hostStage, hostCommitted = false;
  const assertCurrent = () => { if (!current()) throw Object.assign(new Error("obsolete reload candidate"), {stale:true}); };
  try {
    reload.phase = "loading";
    next = await loadModule(version);
    assertCurrent();
    for (const name of ["load","unload","create","destroy","bind_at","restore","carry","render","agent","seekable","published","messages"]) if (typeof next[`gpu_${name}`] !== "function") throw new Error(`GPU ABI missing gpu_${name}; rebuild/relaunch required`);
    await next.gpu_load();
    assertCurrent();
    // The game Presentation trait discards audio under this flag. No physical
    // input, durable checkpoint observer or app message ingress is attached.
    next.gpu_seekable(true);
    const rows = await loadShaders(next);
    for (const [name, text] of rows) if (!await next.gpu_shader_check(name, text)) throw new Error(next.gpu_error());
    assertCurrent();
    replaceShaders(rows, next);
    reload.phase = "staging";
    const at = clockFor(frameAt ?? performance.now());
    const carrier = {worldCarry:exact.worldCarry};
    for (const old of surfaces.values()) {
      const entry = candidateEntry(old, options.values?.get(old.view) ?? old.requestedValues ?? old.values);
      staged.push([old, entry]);
      let carry;
      if (intent === "continue") carry = old.id ? gpu.gpu_carry(old.id) : old.carry;
      if (intent === "restore") {
        carry = options.checkpoints?.get(old.view);
        if (carry === undefined) throw new Error(`surface ${old.name}: selected checkpoint missing`);
      }
      create(entry, next, carry);
      if (intent === "continue" && carry === undefined) restorePending(entry, next, carrier);
      validateStage(entry, next, at);
    }
    if (staged.some(([,entry])=>entry.stateful || entry.stagedPublication !== undefined)) {
      if (new Set(staged.map(([,entry])=>entry.name)).size !== staged.length) throw new Error("ambiguous duplicate surface identity; restart required");
      hostStage = exact.stageCurrent();
      validatePublications(hostStage.batch, new Map(staged.map(([,entry])=>[entry.view,entry])), next, at, false);
    }
    assertCurrent();
    rebaseStage(next, staged.map(([,entry])=>entry));
    reload.phase = "ready";
    // The only commitment point. No awaited operation from capture to here.
    hostStage?.commit(); hostCommitted = true;
    if (raf !== null) { cancelAnimationFrame(raf); raf = null; }
    const oldModule = gpu;
    for (const [old, entry] of staged) {
      old.observer?.disconnect(); old.checkpointObserver?.disconnect(); old.unlisten?.();
      old.el.replaceWith(entry.el);
      if (publishers.get(old.name) === old) publishers.set(old.name, entry);
      surfaces.set(entry.view, entry);
    }
    gpu = next; loaded = true; exact.gpu.version = version;
    next.gpu_seekable(Boolean(exact.now));
    for (const [old] of staged) if (old.id) oldModule.gpu_destroy(old.id);
    oldModule?.gpu_unload();
    if (carrier.worldCarry === undefined) { delete exact.worldCarry; delete globalThis.exactWorldCarry; }
    const depth = exact.applyDepth ?? 0; exact.applyDepth = depth + 1;
    try { hostStage?.present(); for (const [,entry] of staged) { watchCheckpoints(entry); attach(entry); } }
    finally { exact.applyDepth = depth; drainRecords(); }
    successfulSwap(start, exact.gpuArtifacts?.get(version) ?? options.artifact ?? {version}, staged.map(([,e])=>e), intent, options.timing);
    return { ms:performance.now()-start, errors:[], ...diagnostics() };
  } catch (error) {
    if (hostStage && !hostCommitted) hostStage.abort();
    if (next && next !== gpu) disposeStage(next, staged.map(([,e])=>e), true);
    if (error.stale) { reload.stale++; return {ms:performance.now()-start,errors:[],stale:true}; }
    if (current()) { reload.phase = "failed"; reload.error = String(error); reload.failures++;
      reload.restoreOutcome = {status:"refused", reason:String(error), retained:true}; exact.devError?.(String(error)); }
    throw error;
  }
}
const t0 = performance.now();
try {
  const version = exact.gpuVersion ?? 0;
  gpu = await loadModule(version);
  await gpu.gpu_load();
  if (exact.now) gpu.gpu_seekable(true);
  replaceShaders(await loadShaders(gpu));
  exact.gpu.version = version;
  reload.loaded = exact.gpuArtifacts?.get(version) ?? { version, identity:"unavailable: static host did not provide artifact receipt" };
  loaded = true;
} catch (error) { gpu?.gpu_unload(); gpu = undefined; console.error("exact gpu:", error); }
if (loaded) exact.root.dataset.gpuMs = (performance.now() - t0).toFixed(1);
if (loaded && new URLSearchParams(location.search).get("smoke") === "1") navigator.sendBeacon(`/__gpu?ms=${exact.root.dataset.gpuMs}`);
try {
  const waiting = [...surfaces.values()];
  const report = error => { exact.devError?.(String(error)); console.error("exact gpu:", error); };
  for (const s of exact.pendingSurfaces ?? []) if (s.generation === exact.generation) {
    try { exact.gpu.surface(s.id, s.name, s.values); } catch (error) { report(error); }
  }
  exact.pendingSurfaces = [];
  // A refused initial surface must not prevent independent canvases from loading.
  for (const entry of waiting) { try { ensure(entry); } catch (error) { report(error); } }
} finally { finishReady(loaded); }
