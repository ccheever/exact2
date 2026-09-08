// The web host's glue: apply batches, forward events, tick the clock.
//
// @ref LLP 1007 §3. This is host code, not app code: it knows nothing about
// the app. The app is the wasm (runner + kernel + data crate + baked plan).
// Nothing here runs per frame; layout and motion are the browser's.

const root = document.getElementById("exact-root");
const views = new Map(); // view id -> element
const animations = new Map(); // "view/property" -> Animation (a spring in flight)
const iframeLoading = new WeakMap(); // iframe -> true until its latest src load
const iframeOrigins = new WeakMap(); // iframe -> authored/committed guest origin
const messageFrames = new Set(); // iframes whose node handles `message`
let messageListening = false;
let wasm = null;
let memory = null;
let inputReady = false;
const authoredDisabled = new WeakMap();
let logicInfo = null, moduleLoader = null, activeModule = null, moduleResponse = new Uint8Array();
let resolveModuleReady;
const moduleReady = new Promise(resolve => { resolveModuleReady = resolve; });
// Baked content is readable and scrollable before the data executor arrives.
// `inert` would remove the entire tree from hit testing (including scrollers)
// and accessibility. Gate actions and editing, not browser layout/navigation.
function setInputReady(ready) {
  inputReady = ready;
  root.setAttribute("aria-busy", String(!ready));
  if (ready) for (const el of views.values()) {
    if (authoredDisabled.has(el)) {
      el.disabled = authoredDisabled.get(el);
      authoredDisabled.delete(el);
    }
  }
}
for (const kind of ["click", "beforeinput", "submit"]) {
  root.addEventListener(kind, event => {
    if (!inputReady) { event.preventDefault(); event.stopImmediatePropagation(); }
  }, true);
}
function moduleCall(op, ptr, len) {
  if (op === 1) {
    if (len !== moduleResponse.length) throw new Error('module response buffer mismatch');
    new Uint8Array(memory.buffer, ptr, len).set(moduleResponse); return len;
  }
  try {
    const request = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len)));
    moduleResponse = new TextEncoder().encode(JSON.stringify(moduleLoader?.call(request) ?? { error: 'browser module not loaded' }));
  } catch (error) { moduleResponse = new TextEncoder().encode(JSON.stringify({ error: String(error) })); }
  return moduleResponse.length;
}
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
let bootAttempt = 0;
let devAssets = null;
let installedFonts = [];

function commitGuestOrigin(el) {
  const sandbox = new Set((el.getAttribute("sandbox") ?? "").split(/\s+/).filter(Boolean));
  const opaque = el.hasAttribute("sandbox") && !sandbox.has("allow-same-origin");
  let origin = null;
  if (!opaque) {
    const src = el.getAttribute("src");
    try { origin = !src || src === "about:blank" ? location.origin : new URL(src, document.baseURI).origin; }
    catch { origin = null; }
    if (origin === "null") origin = null;
  }
  iframeOrigins.set(el, { origin, opaque });
}

function guestMessageAuthorized(el, eventOrigin) {
  const committed = iframeOrigins.get(el);
  if (!committed) return false;
  // Opaque sandboxed guests retain source identity but have no targetable
  // origin. Source identity still bounds their guest→app string messages;
  // application protocols and any replies belong to the app.
  return committed.opaque ? eventOrigin === "null" : eventOrigin === committed.origin;
}

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

// A deployed page owns one immutable local namespace. Absolute app asset
// paths (including Caltrain's /deck) need the same binding as relative ones;
// ordinary network/data URLs retain their authored meaning.
function localAssetURL(source, assets = devAssets) {
  let url;
  try { url = new URL(source, document.baseURI); } catch { return source; }
  let name;
  try { name = decodeURIComponent(url.pathname.replace(/^\//, "")); } catch { return source; }
  if (assets !== null && url.origin === location.origin && /^(assets|deck|shaders)\//.test(name)) {
    const card = assets.get(name);
    if (!card) return `/__dev/absent/${name.split("/").map(encodeURIComponent).join("/")}`;
    if (card.objectURL) return card.objectURL;
    const resolved = new URL(card.url); resolved.search = url.search; resolved.hash = url.hash;
    return resolved.href;
  }
  if (/^\/\.exact\/root\/web\/releases\/[0-9a-f]{64}\/$/.test(new URL(document.baseURI).pathname)
    && /^\/(assets|deck|shaders)\//.test(source)) return new URL('.' + source, document.baseURI).href;
  return source;
}
function assetNamespace(cards) {
  const assets = new Map();
  const types = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", svg: "image/svg+xml", gif: "image/gif", webp: "image/webp", woff2: "font/woff2", woff: "font/woff", ttf: "font/ttf", otf: "font/otf" };
  for (const [name, card] of cards) {
    const type = types[name.split(".").pop().toLowerCase()];
    assets.set(name, { ...card, objectURL: type ? URL.createObjectURL(new Blob([card.bytes], { type })) : null });
  }
  return assets;
}
function releaseAssets(assets) { for (const card of assets?.values() ?? []) if (card.objectURL) URL.revokeObjectURL(card.objectURL); }

function applyProps(el, set, clear) {
  let sandboxChanged = false;
  for (const name of clear || []) {
    if (el instanceof HTMLIFrameElement && name === "src") iframeLoading.set(el, true);
    if (el instanceof HTMLIFrameElement && name === "sandbox" && el.hasAttribute("sandbox")) sandboxChanged = true;
    if (name === "text") el.textContent = "";
    else if (name === "value") el.value = "";
    else if (name === "checked") el.checked = false;
    else el.removeAttribute(name);
  }
  for (const [name, value] of Object.entries(set || {})) {
    if (el instanceof HTMLIFrameElement && name === "sandbox" && el.getAttribute("sandbox") !== value) sandboxChanged = true;
    if (name === "text") {
      if (el.childElementCount === 0) el.textContent = value;
    } else if (name === "value") {
      if (el.value !== value) el.value = value;
    } else if (name === "checked") {
      el.checked = value === "true";
    } else if (name === "disabled" || name === "readonly") {
      if (value === "true") el.setAttribute(name, ""); else el.removeAttribute(name);
    } else {
      if (el instanceof HTMLIFrameElement && name === "src") iframeLoading.set(el, true);
      el.setAttribute(name, (name === "src" || name === "href") ? localAssetURL(value) : value);
    }
  }
  // Native range/date controls can edit on pointer/key defaults without
  // beforeinput. Disable controls until activation, keeping the plan's own
  // disabled value through any intervening prop updates. Scrollers stay live.
  if (!inputReady && (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLButtonElement)) {
    const disabled = set && "disabled" in set ? set.disabled === "true"
      : clear?.includes("disabled") ? false : authoredDisabled.get(el) ?? el.disabled;
    authoredDisabled.set(el, disabled);
    el.disabled = true;
  }
  if (el instanceof HTMLIFrameElement && sandboxChanged) {
    // Sandbox tokens take effect on navigation. Re-set the authored source
    // so an immutable-per-mount sandbox change remounts as it does on Apple.
    iframeLoading.set(el, true);
    const source = el.getAttribute("src");
    el.setAttribute("src", source ?? "about:blank");
    if (source === null) el.removeAttribute("src");
  }
  if (el instanceof HTMLIFrameElement) commitGuestOrigin(el);
  if ((set && ("viewportFit" in set || "interactiveWidget" in set)) || clear?.some((n) => n === "viewportFit" || n === "interactiveWidget")) syncViewportFit();
}

// @ref LLP 1020 D2 — one page listener routes a guest by source identity.
// Strings cross unchanged; every other structured-clone value narrows to
// JSON, and a value JSON cannot represent is not an event.
function ensureMessageListener() {
  if (messageListening) return;
  messageListening = true;
  window.addEventListener("message", (event) => {
    if (!inputReady) return;
    for (const el of messageFrames) {
      if (event.source !== el.contentWindow) continue;
      if (!guestMessageAuthorized(el, event.origin)) return;
      let payload = event.data;
      if (typeof payload !== "string") {
        try { payload = JSON.stringify(payload); } catch { return; }
      }
      if (typeof payload !== "string") return;
      const id = Number(el.dataset.view);
      if (views.get(id) !== el) return;
      const n = writeIn(payload);
      send(wasm.exact_dispatch(id, 9, n, now()));
      return;
    }
  });
}

// The viewport meta follows the first root's `viewport-fit` and
// `interactive-widget` (LLP 1008 §9): `cover` lays the page out under a
// phone's status bar and home indicator, and `env(safe-area-inset-*)` in the
// CSS carry the insets; `resizes-content` shrinks the layout viewport to the
// software keyboard (Chrome; Safari knows only the default, the visual
// viewport). Safari re-reads the meta when its content changes.
function syncViewportFit() {
  const first = root.firstElementChild;
  const cover = first?.getAttribute("viewportFit") === "cover";
  const widget = first?.getAttribute("interactiveWidget");
  const meta = document.querySelector('meta[name="viewport"]');
  const want = "width=device-width, initial-scale=1" + (cover ? ", viewport-fit=cover" : "") + (widget ? `, interactive-widget=${widget}` : "");
  if (meta && meta.content !== want) meta.content = want;
}

// The page's environment as the browser resolves it (LLP 1012 §1): the
// safe-area insets read off a hidden element padded by `env()`, and the
// software keyboard's height as the visual viewport reports it (zero on a
// desktop, or when the page is not zoomed).
let probe;
function environment() {
  if (!probe) {
    probe = document.createElement("div");
    probe.style.cssText = "position:fixed;inset:0;visibility:hidden;pointer-events:none;padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)";
    document.body.append(probe);
  }
  const r2 = (x) => Math.round(x * 100) / 100;
  const cs = getComputedStyle(probe);
  return {
    "safe-area-inset-top": r2(parseFloat(cs.paddingTop) || 0),
    "safe-area-inset-right": r2(parseFloat(cs.paddingRight) || 0),
    "safe-area-inset-bottom": r2(parseFloat(cs.paddingBottom) || 0),
    "safe-area-inset-left": r2(parseFloat(cs.paddingLeft) || 0),
    "keyboard-inset-height": r2(Math.max(0, innerHeight - (visualViewport?.height ?? innerHeight))),
  };
}

function attach(el, id, handlers) {
  el.dataset.view = String(id);
  // Teardown can synchronously blur the old input after the new runner is
  // live. Only the element currently owning this id may dispatch into it.
  const on = (event, handle) => el.addEventListener(event, (e) => {
    if (views.get(id) === el && (inputReady || event === "load")) handle(e);
  });
  if (el instanceof HTMLIFrameElement) {
    if (!iframeLoading.has(el)) iframeLoading.set(el, true);
    const dispatchLoad = handlers.includes("load");
    on("load", () => {
      iframeLoading.set(el, false);
      if (dispatchLoad && inputReady) send(wasm.exact_dispatch(id, 8, 0, now()));
    });
    if (handlers.includes("message")) {
      messageFrames.add(el);
      ensureMessageListener();
    }
  }
  // A node with focus, blur, or key handlers can take the focus (an input
  // or a button does by itself): the web's rule that only a focusable
  // element hears these.
  if (handlers.some((k) => k === "focus" || k === "blur" || k === "key") && !(el instanceof HTMLInputElement || el instanceof HTMLButtonElement) && !el.hasAttribute("tabindex")) el.tabIndex = 0;
  for (const kind of handlers) {
    if (kind === "press") {
      on("click", (e) => { e.stopPropagation(); send(wasm.exact_dispatch(id, 0, 0, now())); });
    } else if (kind === "change") {
      on("input", () => { const n = writeIn(el.value); send(wasm.exact_dispatch(id, 1, n, now())); });
    } else if (kind === "hover") {
      // pointerenter/pointerleave: the element's own, not a bubbling mouseover.
      on("pointerenter", () => send(wasm.exact_dispatch(id, 2, 0, now())));
      on("pointerleave", () => send(wasm.exact_dispatch(id, 3, 0, now())));
    } else if (kind === "focus") {
      on("focus", () => send(wasm.exact_dispatch(id, 4, 0, now())));
    } else if (kind === "blur") {
      on("blur", () => send(wasm.exact_dispatch(id, 5, 0, now())));
    } else if (kind === "key") {
      // keydown, the key's name as the web spells it (`e.key`).
      on("keydown", (e) => { const n = writeIn(e.key); send(wasm.exact_dispatch(id, 6, n, now())); });
    }
    if (kind === "submit" && el.tagName !== "TEXTAREA") {
      // The web's implicit submission: Enter in a text input submits — here
      // to the node's `submit` handler, no form needed (and no reload).
      on("keydown", (e) => { if (e.key === "Enter" && !e.isComposing) { e.preventDefault(); send(wasm.exact_dispatch(id, 7, 0, now())); } });
    }
  }
}

// App-declared ARIA shortcuts activate the same mounted buttons as a click.
// Browsers may reserve a chord before it reaches the page (notably Meta+N).
document.addEventListener("keydown", (event) => {
  if (event.isComposing || !wasm || !inputReady || event.defaultPrevented) return;
  const matches = (chord) => {
    const parts = chord.split("+");
    const key = parts.pop();
    const modifiers = new Set(parts);
    if (key === "Escape" && !parts.length) return event.key === "Escape"
      && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey;
    return key?.length === 1 && [...modifiers].every(m => ["Meta", "Control", "Alt", "Shift"].includes(m))
      && (modifiers.has("Meta") || modifiers.has("Control"))
      && event.metaKey === modifiers.has("Meta") && event.ctrlKey === modifiers.has("Control")
      && event.altKey === modifiers.has("Alt") && event.shiftKey === modifiers.has("Shift")
      && event.key.toLowerCase() === key.toLowerCase();
  };
  for (const el of root.querySelectorAll("button[aria-keyshortcuts]")) {
    if (!el.isConnected || !el.getClientRects().length || el.closest("[inert]") || getComputedStyle(el).visibility !== "visible") continue;
    if (!(el.getAttribute("aria-keyshortcuts") ?? "").split(/\s+/).some(matches)) continue;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (!event.repeat && !el.disabled) el.click();
    return;
  }
}, true);

function viewFor(op, id) {
  const el = views.get(id);
  if (!el) console.error(`exact: ${op} names missing view ${id}`);
  return el;
}

function apply(batch) {
  const focusCommands = [];
  if (batch.error) console.error("exact:", batch.error);
  for (const op of batch.ops ?? []) {
    try {
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
      case "props": {
        const el = viewFor("props", op.id);
        if (el) applyProps(el, op.set, op.clear);
        break;
      }
      case "style": {
        const el = viewFor("style", op.id);
        if (el) el.style.cssText = op.css;
        break;
      }
      case "children": {
        const el = viewFor("children", op.id);
        if (!el) break;
        const want = [];
        for (const id of op.ids) {
          const child = viewFor("children", id);
          if (child) want.push(child);
        }
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
        const el = viewFor("animate", op.id);
        if (!el) break;
        const css = (v) => op.property === "translate" ? `${v[0]}px ${v[1]}px` : op.property === "rotate" ? `${v}deg` : String(v);
        const anim = el.animate(op.values.map((v) => ({ [op.property]: css(v) })), { delay: op.delay, duration: op.duration, easing: "linear" });
        animations.set(key, anim);
        anim.finished.then(() => { if (animations.get(key) === anim) animations.delete(key); }, () => {});
        break;
      }
      case "surface": {
        // A canvas's inputs (LLP 1009 D2): to the GPU module when it is
        // loaded, queued until then. The module itself is fetched only
        // after a rendering opportunity, and only when a canvas exists.
        if (globalThis.exact.gpu) globalThis.exact.gpu.surface(op.id, op.name, op.values);
        else {
          const pending = (globalThis.exact.pendingSurfaces ??= []);
          const queued = pending.find((entry) => entry.id === op.id && entry.generation === incarnation);
          if (queued) { queued.name = op.name; queued.values = op.values; }
          else pending.push({ id: op.id, name: op.name, values: op.values, generation: incarnation });
        }
        break;
      }
      case "grants": { grants = op.lines; break; }
      case "store": {
        // A secret the app kept or forgot (LLP 1018 D6): `localStorage`,
        // origin-scoped, is the web's secret store. Never in agent mode — a
        // drive starts from nothing and leaves nothing.
        if (agentMode) break;
        try {
          if (op.value == null) localStorage.removeItem("exact.secret." + op.name);
          else localStorage.setItem("exact.secret." + op.name, op.value);
        } catch (e) { console.warn("exact: store", op.name, String(e)); }
        break;
      }
      case "continue": {
        const requestIncarnation = incarnation;
        const p = Promise.resolve().then(() => moduleLoader.run(op.token))
          .then(result => safelyFulfill(requestIncarnation, op.ticket, 0, 200, "", enc.encode(JSON.stringify(result))))
          .catch(error => safelyFulfill(requestIncarnation, op.ticket, 3, 0, "", enc.encode(String(error))));
        inflight.add(p);
        p.finally(() => inflight.delete(p));
        break;
      }
      case "request": {
        // A request the runner handed the page to run (LLP 1016 D2): the
        // browser is the executor and the authority (CORS); the app's grant
        // is checked here too, so a refusal is the same on every host. The
        // reply — any status, or no response — goes back through
        // `exact_fulfill` on this thread; the batch it makes is applied
        // like any other.
        const { ticket, method, url, headers, body, cache } = op;
        const requestIncarnation = incarnation;
        if (!granted(url)) {
          deferFulfill(requestIncarnation, ticket, 2, 0, "", enc.encode(`refused by grant: ${url}`));
          break;
        }
        let decodedBody;
        try {
          if (body) decodedBody = Uint8Array.from(atob(body), (c) => c.charCodeAt(0));
        } catch (e) {
          deferFulfill(requestIncarnation, ticket, 4, 0, "", enc.encode(`invalid request body: ${String(e)}`));
          throw e;
        }
        const controller = new AbortController();
        controllers.add(controller);
        const init = { method, headers, cache: cache === "reload" ? "reload" : "default", signal: controller.signal };
        if (decodedBody) init.body = decodedBody;
        const p = fetch(url, init)
          .then(async (r) => safelyFulfill(requestIncarnation, ticket, 0, r.status, [...r.headers].map(([k, v]) => `${k}: ${v}`).join("\n"), new Uint8Array(await r.arrayBuffer())))
          .catch((e) => safelyFulfill(requestIncarnation, ticket, controller.signal.aborted ? 4 : 1, 0, "", enc.encode(String(e?.message ?? e))));
        inflight.add(p);
        p.finally(() => { inflight.delete(p); controllers.delete(controller); });
        break;
      }
      case "command": {
        // A capability an action called (LLP 1005 §3). `setScheme` is the
        // document's colour scheme — what `prefers-color-scheme` would be.
        // `system` is CSS's `light dark`: the page supports both and the
        // user's preference decides, which is what "follow the system" is on
        // the web. `light`/`dark` are the property's own values.
        if (op.name === "setScheme") { const s = String(op.args[0] ?? ""); document.documentElement.style.colorScheme = s === "system" ? "light dark" : s; }
        else if (op.name === "focus") focusCommands.push(op.args);
        else console.warn(`exact: unknown command ${op.name}`);
        break;
      }
      case "destroy": { const el = views.get(op.id); if (el) { messageFrames.delete(el); el.remove(); } views.delete(op.id); globalThis.exact.gpu?.destroy(op.id); break; }
      case "roots": {
        const roots = [];
        for (const id of op.ids) {
          const el = viewFor("roots", id);
          if (el) roots.push(el);
        }
        root.replaceChildren(...roots);
        syncViewportFit();
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
    } catch (e) {
      // A malformed op is isolated: the runner already committed the whole
      // batch, so leaving the DOM at a prefix would be the worst outcome.
      console.error(`exact: ${String(op?.op ?? "unknown")} op failed`, e);
    }
  }
  // Focusing can dispatch an action; every node/value in this batch must be
  // committed before its focus handler runs.
  for (const args of focusCommands) {
    if (args?.length !== 1 || typeof args[0] !== "string" || !inputReady) continue;
    const el = [...root.querySelectorAll("[id]")].find(node => node.id === args[0]);
    if (!el || !el.isConnected || el.matches(":disabled") || el.closest("[inert]")
        || !el.getClientRects().length || getComputedStyle(el).visibility !== "visible") continue;
    el.focus();
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

// LLP 1019 D5: FontFace loading is part of host boot. The DOM remains empty
// until every local face loaded, or 100 ms elapsed. At the barrier, install
// every face already ready unless its family has a failed sibling; faces that
// finish later remain unused for this generation (no post-paint swap).
async function prepareFonts(faces, assets) {
  if (!faces?.length) return [];
  const rows = faces.map((face) => ({ face, state: "pending", loaded: null }));
  const pending = rows.map(async (row) => {
    const { face } = row;
    try {
      const url = new URL(localAssetURL(face.source, assets), document.baseURI).href;
      row.loaded = await new FontFace(face.family, `url(${JSON.stringify(url)})`, {
        weight: String(face.weight),
        style: face.style,
      }).load();
      row.state = "loaded";
    } catch (e) {
      row.state = "failed";
      console.error("exact: font.registration.failed", face.family, face.source, String(e));
    }
  });
  let timer;
  const ready = Promise.all(pending);
  const timedOut = await Promise.race([
    ready.then(() => false),
    new Promise((resolve) => { timer = setTimeout(() => resolve(true), 100); }),
  ]);
  clearTimeout(timer);
  const failed = new Set(rows.filter((row) => row.state === "failed").map((row) => row.face.family));
  if (timedOut) console.error("exact: font.registration.timeout", faces.length);
  return rows.filter((row) => row.state === "loaded" && !failed.has(row.face.family)).map((row) => row.loaded);
}
function commitFonts(faces) {
  for (const face of installedFonts) document.fonts.delete(face);
  for (const face of faces) document.fonts.add(face);
  installedFonts = faces;
}

// LLP 1016: the app's grants (`net.fetch <url prefix>` lines, from the boot
// batch), the fetches in flight (the agent's `settle` waits on them), and
// the reply path into the wasm.
let grants = [];
const inflight = new Set();
const controllers = new Set();
let incarnation = 0;
const enc = new TextEncoder();
function granted(url) {
  // A `net.fetch` grant is an origin — scheme, host, port — matched whole,
  // as ibex2 matches it on the native hosts (LLP 0067): the same refusal
  // everywhere. A URL that does not parse is outside every grant.
  let origin;
  try { origin = new URL(url).origin; } catch { return false; }
  return grants.some((g) => {
    const [kind, granted] = g.split(/\s+/, 2);
    if (kind !== "net.fetch" || !granted) return false;
    try { return new URL(granted).origin === origin; } catch { return false; }
  });
}
function fulfill(requestIncarnation, ticket, kind, status, headersText, body) {
  // `boot` starts tickets again at one. A completion from the program that
  // owned an old ticket must never be delivered into the new incarnation.
  if (!wasm || requestIncarnation !== incarnation) return;
  const h = enc.encode(headersText);
  const ptr = wasm.exact_in(h.length + body.length);
  const mem = new Uint8Array(memory.buffer, ptr, h.length + body.length);
  mem.set(h);
  mem.set(body, h.length);
  send(wasm.exact_fulfill(ticket, kind, status, h.length, body.length, now()));
}
function safelyFulfill(...args) {
  try { fulfill(...args); }
  catch (e) { console.error("exact: request fulfillment failed", e); }
}
function deferFulfill(...args) {
  // Refusals and malformed request bodies are known while their enclosing
  // batch is still applying. Deliver them on the next microtask so their
  // commits cannot re-enter `apply` halfway through that batch.
  queueMicrotask(() => safelyFulfill(...args));
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

// A same-origin guest joins `tree` as a compact, bounded outline. Access to
// a sandboxed or cross-origin document is simply absent (@ref LLP 1020 D4).
function guestOutline(frame) {
  let doc;
  try { doc = frame.contentDocument; } catch { return null; }
  if (!doc) return null;
  const outline = [];
  const visit = (el, depth) => {
    if (depth > 4 || outline.length >= 32) return;
    const id = el.id || undefined;
    const testId = el.getAttribute("data-testid") ?? el.getAttribute("testId") ?? undefined;
    const text = [...el.childNodes]
      .filter((n) => n.nodeType === Node.TEXT_NODE)
      .map((n) => n.textContent.trim())
      .filter(Boolean)
      .join(" ")
      .replace(/\s+/g, " ")
      .slice(0, 160) || undefined;
    if (id || testId || text) {
      outline.push({ guest: true, depth, tag: el.localName, ...(id ? { id } : {}), ...(testId ? { testId } : {}), ...(text ? { text } : {}) });
    }
    for (const child of el.children) visit(child, depth + 1);
  };
  for (const child of doc.body?.children ?? []) visit(child, 0);
  return outline;
}

function tree() {
  const reply = ask({ op: "tree" });
  for (const node of reply.nodes ?? []) {
    const el = views.get(node.id);
    if (!(el instanceof HTMLIFrameElement)) continue;
    node.url = el.getAttribute("src") ?? "";
    node.loading = iframeLoading.get(el) !== false;
    const guest = guestOutline(el);
    if (guest !== null) node.guest = guest;
  }
  return reply;
}

function guestDocument(frame) {
  try {
    const document = frame.contentDocument;
    return document ? { document } : { error: "guest is cross-origin" };
  } catch {
    return { error: "guest is cross-origin" };
  }
}

function guestTap(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const x = Number.isFinite(request.x) ? request.x : guest.innerWidth / 2;
  const y = Number.isFinite(request.y) ? request.y : guest.innerHeight / 2;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, error: "guest tap has an invalid selector" }; }
  target ||= document.elementFromPoint(x, y) || document.body;
  if (!target) return { guest: true, error: "guest tap found no target" };
  // Script input is intentionally untrusted (@ref LLP 1020 D4;
  // exact1 20260806-webview-frame-guest-click-delivery).
  target.dispatchEvent(new guest.PointerEvent("pointerdown", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 1 }));
  target.dispatchEvent(new guest.PointerEvent("pointerup", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0, buttons: 0 }));
  target.dispatchEvent(new guest.MouseEvent("click", { bubbles: true, composed: true, clientX: x, clientY: y, button: 0 }));
  return { tapped: request.id, guest: true };
}

function guestType(frame, request) {
  const access = guestDocument(frame);
  if (access.error) return { guest: true, error: access.error };
  const { document } = access;
  const guest = document.defaultView;
  const active = document.activeElement;
  const editable = active?.matches?.("input,textarea,[contenteditable]") ? active : null;
  let target;
  try { target = request.selector ? document.querySelector(request.selector) : null; }
  catch { return { guest: true, error: "guest type has an invalid selector" }; }
  target ||= editable || document.querySelector("input,textarea,[contenteditable]");
  if (!target) return { guest: true, error: "guest type found no target" };
  // These are the Apple guest script's event shapes, including focus and
  // isTrusted:false (@ref LLP 1020 D4).
  target.focus();
  if (request.key != null) {
    const key = String(request.key);
    target.dispatchEvent(new guest.KeyboardEvent("keydown", { key, bubbles: true, composed: true }));
    target.dispatchEvent(new guest.KeyboardEvent("keyup", { key, bubbles: true, composed: true }));
    return { typed: request.id, guest: true, key, value: "value" in target ? target.value : target.textContent };
  }
  const text = String(request.text ?? "");
  if ("value" in target) target.value = text; else target.textContent = text;
  target.dispatchEvent(new guest.InputEvent("input", { data: text, inputType: "insertText", bubbles: true, composed: true }));
  target.dispatchEvent(new guest.Event("change", { bubbles: true, composed: true }));
  return { typed: request.id, guest: true, value: "value" in target ? target.value : target.textContent };
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
const SETTLE_DEADLINE_MS = 20_000;
async function waitForInflight(deadline) {
  while (inflight.size) {
    const remaining = deadline - performance.now();
    if (remaining <= 0) return false;
    let timer;
    const completed = await Promise.race([
      Promise.race([...inflight]).then(() => true),
      new Promise((resolve) => { timer = setTimeout(() => resolve(false), remaining); }),
    ]);
    clearTimeout(timer);
    if (!completed) return false;
  }
  return true;
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
          if (el instanceof HTMLIFrameElement) {
            const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
            n.hit = hit === el;
          }
          if (el.dataset.scroll === "true") { n.sx = r2(el.scrollLeft); n.sy = r2(el.scrollTop); }
          nodes.push(n);
        }
        return { clock: now(), viewport: { w: innerWidth, h: innerHeight }, env: environment(), nodes };
      }
      case "focus": {
        const el = views.get(request.id);
        if (!el) return { error: `no view ${request.id}` };
        if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return { error: `view ${request.id} is not an input` };
        el.focus();
        el.select();
        return { ok: true };
      }
      case "tap": {
        const frame = views.get(request.id);
        return frame instanceof HTMLIFrameElement ? guestTap(frame, request) : { guest: false };
      }
      case "type": {
        const frame = views.get(request.id);
        return frame instanceof HTMLIFrameElement ? guestType(frame, request) : { guest: false };
      }
      case "clock":
        return clock(request);
      case "tree":
        return tree();
      default:
        return ask(request);
    }
  } catch (e) {
    return { error: String(e) };
  }
}

// To `to`, or to `settle`: a fixed point — advance to when the last thing
// in flight ends, and if the timers crossed on the way started more, again
// (bounded; `settled: false` at the bound). A request in flight (LLP 1016)
// is waited for first: its reply commits, and may start motion or ask for
// more, before the fixed point is measured. The clock lands where the
// runner says; a timer's refusal is the error. A promise: the driver awaits it.
async function clock(request) {
  const settle = !!request.settle;
  const deadline = settle ? performance.now() + SETTLE_DEADLINE_MS : 0;
  for (let rounds = 0; ; rounds++) {
    if (settle && !(await waitForInflight(deadline))) return { clock: agentClock, settled: false };
    const to = settle ? settleCandidate() : request.to;
    if (!(to >= agentClock)) return { error: `the clock cannot go backwards (${agentClock} → ${to})` };
    const { batch } = applyBatch(JSON.parse(readOut(wasm.exact_advance(to))));
    globalThis.exact.gpu?.schedule?.();
    if (batch.error) return { error: `clock: ${batch.error}`, clock: agentClock };
    if (!settle) return { clock: agentClock };
    if (inflight.size) { if (rounds >= 15) return { clock: agentClock, settled: false }; continue; }
    const next = settleCandidate();
    if (next <= agentClock) return { clock: agentClock, settled: true };
    if (rounds >= 15) return { clock: agentClock, settled: false };
  }
}

let ticker = null;

// Boot the app — from the plan baked into the wasm, or from `bytes` (the
// dev loop's restart carrying compatible state, LLP 1007 §6).
// Returns the milliseconds from call to first frame in the DOM.
async function boot(bytes, assets = devAssets, current = () => true, module = null) {
  const t = performance.now(), request = ++bootAttempt;
  // Decode and load private font faces while the live page keeps running.
  // Carry state only at the synchronous host acceptance point below.
  const bakedLength = bytes ? 0 : wasm.exact_plan();
  const plan = bytes ?? new Uint8Array(memory.buffer, wasm.exact_out(), bakedLength).slice();
  let ptr = wasm.exact_in(plan.length);
  new Uint8Array(memory.buffer, ptr, plan.length).set(plan);
  const faces = JSON.parse(readOut(wasm.exact_plan_fonts(plan.length)));
  if (faces.error) throw new Error(faces.error);
  const preparedFonts = await prepareFonts(faces, assets);
  const shaderCommit = assets !== null && globalThis.exact.gpu ? await globalThis.exact.gpu.prepareShaders(assets) : null;
  if (!current() || request !== bootAttempt) return null;
  let len;
  if (module) {
    const id = new TextEncoder().encode(JSON.stringify(module.realm.id));
    const payload = new Uint8Array(plan.length + module.receipt.length + id.length);
    payload.set(plan); payload.set(module.receipt, plan.length); payload.set(id, plan.length + module.receipt.length);
    ptr = wasm.exact_in(payload.length); new Uint8Array(memory.buffer, ptr, payload.length).set(payload);
    len = wasm.exact_boot_module(plan.length, module.receipt.length, id.length);
  } else if (bytes) {
    ptr = wasm.exact_in(bytes.length);
    new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    len = wasm.exact_boot_plan(bytes.length);
  } else len = wasm.exact_boot();
  const batch = JSON.parse(readOut(len));
  if (batch.error) throw new Error(batch.error);
  if (module) { activeModule?.realm.dispose(); activeModule = module; setInputReady(true); }
  const oldAssets = devAssets;
  devAssets = assets;
  shaderCommit?.();
  // The candidate is now the live Rust Host. Tear down without yielding;
  // attach's ownership guard refuses synchronous events from removed nodes.
  incarnation += 1;
  globalThis.exact.generation = incarnation;
  // A queued surface belongs to the plan that named it. The GPU device may
  // finish loading across a reload; no old surface request may join the new
  // plan even when view ids are reused.
  globalThis.exact.pendingSurfaces = [];
  if (ticker) clearInterval(ticker);
  ticker = null;
  for (const a of animations.values()) a.cancel();
  animations.clear();
  globalThis.exact?.gpu?.reset();
  views.clear();
  messageFrames.clear();
  grants = [];
  for (const controller of controllers) controller.abort();
  controllers.clear();
  inflight.clear();
  root.replaceChildren();
  commitFonts(preparedFonts);
  const timers = applyBatch(batch).timers;
  if (oldAssets !== assets) releaseAssets(oldAssets);
  if (timers && !agentMode) ticker = setInterval(() => send(wasm.exact_advance(now())), 250);
  if (bytes) requestAnimationFrame(() => requestAnimationFrame(loadGpuIfNeeded));
  return performance.now() - t;
}

// `agent` and `now` exist only in agent mode: a normal page has no agent
// surface and no clock but the browser's.
let ready;
globalThis.exact = {
  // A dev-plan event can arrive while the wasm is still fetching. Queue it
  // behind the initial boot instead of acknowledging a reload that did not
  // happen.
  reload: async (bytes) => { await ready; if (logicInfo) throw new Error('module reload requires a paired generation'); return boot(bytes); },
  reloadGeneration: async (bytes, cards, current, module = null) => {
    await ready;
    await moduleReady;
    if (module && !logicInfo) throw new Error('this web client has binary-bound logic; rebuild with the browser module executor');
    if (!module && logicInfo) throw new Error('a module client requires a paired plan/module generation');
    const assets = assetNamespace(cards);
    let candidate = null;
    try {
      if (module) candidate = { ...module, realm: await moduleLoader.prepare(module, logicInfo) };
      return await boot(bytes, assets, current, candidate) !== null;
    } finally {
      if (devAssets !== assets) releaseAssets(assets);
      if (candidate && activeModule !== candidate) candidate.realm.dispose();
    }
  },
  get devAssets() { return devAssets; },
  get ready() { return ready.then(async () => { if (logicInfo) { await moduleReady; if (!inputReady) throw new Error(root.dataset.error || 'browser module not ready'); } }); },
  ...(agentMode ? { agent, now } : {}), views, root, generation: 0, pendingSurfaces: [],
};

// The GPU module, on demand: a script element after a rendering opportunity
// (two animation-frame callbacks), never an eager import, and only when a
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
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), { exact_js: { call: moduleCall } });
  wasm = instance.exports;
  memory = wasm.memory;
  globalThis.exact.compat = JSON.parse(readOut(wasm.exact_compat()));
  logicInfo = wasm.exact_logic ? JSON.parse(readOut(wasm.exact_logic())) : null;
  setInputReady(!logicInfo); // First pixel is baked; actions need the deferred executor.
  // The kept secrets, before boot (LLP 1018 D6): every `exact.secret.*` key,
  // handed to the runner, which keeps the granted names — so the first frame
  // is a returning user's. Agent mode starts from nothing.
  if (!agentMode) {
    const kept = [];
    try {
      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        if (key?.startsWith("exact.secret.")) kept.push(key.slice("exact.secret.".length), localStorage.getItem(key) ?? "");
      }
    } catch (e) { console.warn("exact: store", String(e)); }
    if (kept.length) wasm.exact_store(writeIn(kept.join("\0")));
  }
  await boot(null);
  // The first frame is in the DOM: stamp the time from script start, so a
  // headless run can read it. rAF runs before paint; its stamp is only the
  // first frame callback. The nested callback gives the browser a rendering
  // opportunity before optional module loading begins.
  root.dataset.bootMs = (performance.now() - t0).toFixed(1);
  requestAnimationFrame(() => {
    root.dataset.frameCallbackMs = (performance.now() - t0).toFixed(1);
    requestAnimationFrame(async () => {
      loadGpuIfNeeded();
      try {
        if (logicInfo) {
          moduleLoader = await new Promise((resolve, reject) => {
            const script = document.createElement('script'); script.type = 'module';
            script.src = new URL('./module-glue.js', import.meta.url).href;
            script.onload = () => resolve(globalThis.exact.moduleRuntime);
            script.onerror = () => reject(new Error('browser module loader failed'));
            document.head.append(script);
          });
          const payload = await moduleLoader.baked();
          const realm = await moduleLoader.prepare(payload, logicInfo, 0);
          activeModule = { ...payload, realm };
          const batch = JSON.parse(readOut(wasm.exact_data_ready()));
          if (batch.error) throw new Error(batch.error);
          applyBatch(batch);
          setInputReady(true);
          root.dataset.moduleReady = 'true';
        }
      } catch (error) { root.dataset.error = String(error); console.error(error); }
      finally { resolveModuleReady(); }
    });
  });
}

ready = main();
ready.catch((e) => { console.error(e); root.dataset.error = String(e); });
