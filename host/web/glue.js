// The web host's glue: apply batches, forward events, tick the clock.
//
// @ref LLP 1007 §3. This is host code, not app code: it knows nothing about
// the app. The app is the wasm (runner + kernel + data crate + baked plan).
import { grantOrigins, createGrantSet, grantError, rawGrantText, scopedGrantSet, deferredFulfill, refusal, guestOutline, guestTap, guestType, focusController, runFocusCommands, environment, preferences, onPreferences, inertAncestor, navigation, afterPaintPieces, presenceLoader, animationClock, scrollFollowers, renderMarkup, navigableURL, navigates, refuseURL, devFirst, reportPlace, reportTime, pageReporter, valuedControl, typedControl, settleValue, typeControl, reveal, viewBox, foldBits, foldEnv, onFold, preferFold, fold } from "./navigation.js";
const AGENT_ADMITTED = true; // false in a production bake: host/web/build.mjs rewrites this line (LLP 1069.007 D2)
let httpModule, pickerModule, documentsModule; // the file picker (LLP 1069.002) and documents (LLP 1069.010), loaded on first use
const picker = () => pickerModule ??= loadAfterPaint('./picker-glue.js', 'picker').then(install => install({ appId: globalThis.exact.compat?.inputs?.app, dispatch: (id, kind, payload) => { if (views.has(id)) send(wasm.exact_dispatch(id, kind, writeIn(payload), now())); }, pickedPath: (name) => loadStage('inspection').then(() => ask({ op: "pickedPath", name }).path), log }));
const documentsGlue = () => documentsModule ??= loadAfterPaint('./documents-glue.js', 'documents').then(d => d.install({ dispatch: (id, kind, payload) => { if (views.has(id)) send(wasm.exact_dispatch(id, kind, writeIn(payload), now())); }, log, openFile: (value) => { const id = [...views].find(([, el]) => el.dataset?.testid === "open-file")?.[0]; if (id == null) return false; send(wasm.exact_dispatch(id, 1, writeIn(value), now())); return true; } }));
function httpHelpers() {
  return httpModule ??= moduleReady.then(() => loadAfterPaint('./http-body.js', 'httpHelpers'));
}
const root = document.getElementById("exact-root");
const views = new Map(); // view id -> element
// Springs, holds, drags and virtualized collections: after-paint pieces, fetched on first use (LLP 1047 D5).
const pieces = afterPaintPieces(loadAfterPaint, { root, views, applyBatch, agent: () => agentMode, inert: inertAncestor, now: () => now(), generation: () => incarnation, ready: () => inputReady,
  replayed() { motion.commit(); arrange.commit(); if (agentMode) { register(agentClock); seek(agentClock); } else motion.followTimelines(); },
  wasm(name, bytes) { if (!wasm) return null; new Uint8Array(memory.buffer, wasm.exact_in(bytes.length), bytes.length).set(bytes); return JSON.parse(readOut(wasm[name](bytes.length))); } });
const { collections, motion, arrange } = pieces, retiredViews = new WeakSet(); // committed removals must not dispatch teardown events
const presence = presenceLoader(loadAfterPaint, root, batch => applyBatch(batch), log); // exit-animation and layout-transition, after paint at first use (LLP 1063)
let mediaModule, imageHold, geometry = null; // animated images held to the agent's clock (image-glue.js, LLP 1011.000); geometry reads (geometry-glue.js, LLP 1051.000 D4)
function syncMedia(el, set = {}, clear = []) {
  if (!(el instanceof HTMLVideoElement)) return;
  el.exactMedia ??= { props: {}, handlers: [] };
  Object.assign(el.exactMedia.props, set);
  for (const name of clear) delete el.exactMedia.props[name];
  mediaModule ??= new Promise(resolve => requestAnimationFrame(() => resolve(loadAfterPaint('./media-glue.js', 'installMedia'))));
  mediaModule.then(install => { if (el.isConnected) install(el, payload => { if (views.get(Number(el.dataset.view)) === el && inputReady) send(wasm.exact_dispatch(Number(el.dataset.view), 19, writeIn(payload), now())); }); }).catch(console.error);
}
const iframeLoading = new WeakMap(); // iframe -> true until its latest src load
const iframeOrigins = new WeakMap(); // iframe -> authored/committed guest origin
const messageViews = new Set(), messageFrames = new Set(); // the latter: iframes whose node handles `message`
let messageListening = false, keyEvent = null; // keyEvent: the keydown a `key` handler is running for, which its `preventDefault()` command prevents
let wasm = null, memory = null, inputReady = false, inputHandlers;
// Native modules (LLP 1024 D3): a module node is its custom element, empty until the adapter and the app's module load after first paint (the browser's paint entry; two frames and a beat where it records none).
let nativePaint = null;
function afterNativePaint() {
  return nativePaint ??= new Promise(resolve => {
    let observer;
    const done = () => { observer?.disconnect(); resolve(); };
    try { observer = new PerformanceObserver(() => requestAnimationFrame(done)); observer.observe({ type: 'paint', buffered: true }); } catch {}
    requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(done, 250)));
  });
}
let nativeHost = null;
function nativeCreate(el, id) {
  const st = el.exactNative = {
    id, name: el.localName, state: "loading",
    status() { return { name: this.name, state: this.state, ...(this.error ? { error: this.error } : {}) }; },
    destroy() { this.destroyed = true; nativeHost?.then(h => h.destroy(el)); },
  };
  (nativeHost ??= afterNativePaint()
    .then(() => loadAfterPaint('./native-glue.js', 'nativeHost'))
    .then(make => make({ log, dispatch(el, kind, text) {
      const id = Number(el.dataset.view);
      if (inputReady && views.get(id) === el && !retiredViews.has(el))
        send(wasm.exact_dispatch(id, kind, text == null ? 0 : writeIn(text), now()));
    } }))).then(h => h.attach(el), e => {
      nativeHost = null;
      st.state = "unavailable";
      st.error = String(e?.message ?? e);
      log(`native ${st.name} #${id}: unavailable: ${st.error}`);
    });
}
const authoredDisabled = new WeakMap();
let logicInfo = null, moduleLoader = null, activeModule = null, moduleResponse = new Uint8Array();
const pageNative = Boolean(document.querySelector('meta[name="exact-native"]'));
let pageNativeModule = null;
const loadPageNative = () => pageNativeModule ??= afterNativePaint().then(() => loadAfterPaint('./native-glue.js', 'pageNative'))
  .then(load => load(pageNative, {
    ready: () => inputReady, generation: () => incarnation, agent: agentMode, now,
    changed: topic => { if (wasm.exact_changed) applyBatch(JSON.parse(readOut(wasm.exact_changed(writeIn(topic))))); },
  })).catch(error => { pageNativeModule = null; throw error; });
let rustLoader = null, rustLoading = null;
const rustImports = Object.fromEntries(['load', 'call', 'read', 'drop'].map(name => [name, (...args) => {
  if (!rustLoader) throw new Error('Rust module loader is not ready');
  return rustLoader[name](...args);
}]));
// A Rust source's entropy and, under the agent, the seed of its repeatable
// stream (LLP 1069.005 D5, D2b; `exact_data::crypto`). The seed is
// storage-environment.js's `agentSeed`: a loopback page only; -1 is none.
const dataImports = {
  random: (ptr, len) => { crypto.getRandomValues(new Uint8Array(wasm.memory.buffer, ptr, len)); },
  agent_seed: () => {
    const url = new URL(location.href), seed = Number(url.searchParams.get('seed') ?? 1);
    if (!url.searchParams.has('agent') || !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)) return -1;
    return Number.isSafeInteger(seed) && seed >= 0 ? seed : 1;
  },
};
function loadAfterPaint(file, exported) {
  return new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.type = 'module';
    script.src = new URL(file, import.meta.url).href;
    script.onload = () => resolve(globalThis.exact[exported]);
    script.onerror = () => reject(new Error('host module loader failed: ' + file));
    document.head.append(script);
  });
}
async function loadRust() {
  if (rustLoader) return;
  rustLoading ??= loadAfterPaint('./rust-glue.js','createRustRuntime')
    .then(create=>{rustLoader=create(()=>memory);}).catch(error=>{rustLoading=null;throw error;});
  await rustLoading;
}
// @ref LLP 1043.000 §3 D7/D8 — optional executor, absent from ordinary boots.
let textflow = null, flowLoading = null, flowContexts = [], flowDue = null, flowFrames = false, present = timestamp => send(wasm.exact_frame(timestamp - t0)); // an animation frame (LLP 1073 D5)
function flowBatch(batch) {
  const op = batch.ops?.find(op => op.op === "textflow");
  if (op) flowContexts = op.contexts;
  flowDue = batch.timer_due_ms ?? null; flowFrames = !!batch.frames;
  ticker?.update(flowDue, flowFrames);
  if (textflow) { textflow.afterBatch(batch); return; }
  if (!flowContexts.length || flowLoading) return;
  ticker?.dispose(); ticker = null;
  const generation = incarnation;
  flowLoading = loadAfterPaint('./textflow-glue.js', 'createTextFlow').then(async create => {
    if (generation !== incarnation) return;
    const controller = await create({ views, agentMode, log, now,
      advance: () => send(wasm.exact_advance(now(), 0)), present });
    if (generation !== incarnation) { controller.dispose(); return; }
    textflow = controller;
    textflow.afterBatch({ ops: [{ op: "textflow", contexts: flowContexts }], timer_due_ms: flowDue, frames: flowFrames });
  });
  flowLoading.catch(error => { if (generation === incarnation) log(`textflow module: ${error}`); });
}
let resolveModuleReady, headGlue = null, page = document.querySelector('script[type="application/vnd.exact.checkpoint"]') ? { holding: true, early: globalThis.exact?.taps?.() ?? [] } : null; // a built document (LLP 1048.000 D6), held until the runtime settles; the presses its capture script took before now come first
const moduleReady = new Promise(resolve => { resolveModuleReady = resolve; });
const focus = focusController({ready:() => inputReady, elements:() => views.values(), inert:inertAncestor});
const focusAutofocus = focus.autofocus;
function setInputReady(ready) {
  inputReady = ready;
  root.setAttribute("aria-busy", String(!ready));
  if (ready) for (const el of views.values()) {
    if (authoredDisabled.has(el)) {
      el.disabled = authoredDisabled.get(el);
      authoredDisabled.delete(el);
    }
  }
  if (ready) { motion.commit(); focusAutofocus(); }
}
for (const kind of ["click", "beforeinput", "submit"]) {
  root.addEventListener(kind, event => {
    if (!inputReady && !page?.holding) { event.preventDefault(); event.stopImmediatePropagation(); } else if (event.type === "click") page?.early?.push(event);
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
const agentMode = AGENT_ADMITTED && new URL(location.href).searchParams.has("agent");
let agentClock = agentMode ? 0 : null, followOnSeek = true;
// A seek moves drag timelines' sources too (LLP 1057.003 D2): their consumers follow in it.
const { register, seek: seekAnimations, settle: settleCandidate } = animationClock(() => agentClock, () => ask({ op: "settle" }).settle, () => { if(followOnSeek)motion.followTimelines(); presence.live?.sync(); });
const seek = to => { (imageHold ??= loadAfterPaint('./image-glue.js', 'holdImages').then(f => f({ root, now: () => agentClock }))).then(h => h.seek()); seekAnimations(to); };
const POST_BOUND = 64; // posts held per surface before its canvas is live; gpu-glue.js's exact.postBound, Linux POST_BOUND, Apple Canvases.postBound
const now = () => agentClock ?? performance.now() - t0;
let frameSampler = null; // a development page's frame sampler (frames.js, LLP 1079 D3)
let timelinesMoved = false; // a batch's `timelines` op: its consumers are sought once it is applied
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
  const types = { mp4: "video/mp4", webm: "video/webm", vtt: "text/vtt", png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", svg: "image/svg+xml", gif: "image/gif", webp: "image/webp", woff2: "font/woff2", woff: "font/woff", ttf: "font/ttf", otf: "font/otf" };
  for (const [name, card] of cards) {
    const type = types[name.split(".").pop().toLowerCase()];
    assets.set(name, { ...card, objectURL: type ? URL.createObjectURL(new Blob([card.bytes], { type })) : null });
  }
  return assets;
}
function releaseAssets(assets) { for (const card of assets?.values() ?? []) if (card.objectURL) URL.revokeObjectURL(card.objectURL); }
const lists = new Map();
let listSelection, listSelectionLoading;
// A virtualized list has logical rows; its (`collection`) feedback is navigation.js's.
function listView(el, id, collection) {
  if (!collection) return;
  lists.set(el, { id });
  listSelectionLoading ??= new Promise(resolve => requestAnimationFrame(() => resolve(loadAfterPaint('./list-selection.js', 'installListSelection'))))
    .then(install => { listSelection = install({ root, lists,
      index: (el, key) => wasm.exact_list_index(Number(el.dataset.view), writeIn(key)),
      text: (el, a, b) => { const first = encoder.encode(a?.key ?? '').length, len = writeIn((a?.key ?? '') + (b?.key ?? '')); return readOut(wasm.exact_list_text(Number(el.dataset.view), first, len, a?.paragraph ?? 0, a?.offset ?? 0, b?.paragraph ?? 0, b?.offset ?? 0)); },
    }); listSelection.sync(); }).catch(console.error);
}
function syncLists() { listSelection?.sync(); }
function forgetList(el) { listSelection?.forget(el); lists.delete(el); }
const pendingScrolls = new Map();
const { followedScrolls, followScroll, settleFollow, rememberScroll } = scrollFollowers(positionContexts);
root.addEventListener("pointerdown", event => {
  const target = event.target;
  const editor = target.closest?.("input, textarea, select") || target.isContentEditable;
  if (!editor && target.closest?.('[retainFocus="true"]')) { event.preventDefault(); return; }
  const button = target.closest?.("button");
  if (!button) return;
  for (const preview of root.querySelectorAll("[contextTarget]")) {
    if (contextPanel(preview)?.contains(button)) { event.preventDefault(); return; }
  }
});
function contextPanel(preview) {
  for (let parent = preview.parentElement; parent && parent !== root; parent = parent.parentElement) {
    if (getComputedStyle(parent).position === "absolute") return parent;
  }
  return null;
}
const contextTransforms = new Set();
const contextAnchors = new Map(); // preview view id -> mounted source and entry box
function contextAnchor(target, port = root.getBoundingClientRect()) {
  const box = target.getBoundingClientRect();
  const scroll = contextContent(target)?.parentElement;
  return { target, left: box.left - port.left, top: box.top - port.top,
    width: box.width, height: box.height, viewportWidth: port.width,
    scroll, scrollTop: scroll ? scroll.getBoundingClientRect().top - port.top : null };
}
function prepareContexts(batch) {
  for (const op of batch.ops ?? []) {
    const props = op.op === "create" ? op.props : op.op === "props" ? op.set : null;
    if (!props?.contextTarget) continue;
    const target = document.getElementById(props.contextTarget);
    if (target && contextAnchors.get(op.id)?.target !== target) contextAnchors.set(op.id, contextAnchor(target));
  }
}
function contextContent(source) {
  for (let child = source; child?.parentElement && child.parentElement !== root; child = child.parentElement) {
    const parent = child.parentElement;
    if (parent.dataset.scroll === "true" && /^(auto|scroll)$/.test(getComputedStyle(parent).overflowY)) return child;
  }
  return null;
}
function positionContexts() {
  for (const node of contextTransforms) node.style.transform = "";
  contextTransforms.clear();
  for (const [id, anchor] of contextAnchors) {
    const preview = views.get(id);
    if (!preview?.isConnected || !anchor.target.isConnected
      || document.getElementById(preview.getAttribute("contextTarget")) !== anchor.target) contextAnchors.delete(id);
  }
  const project = (node, transform) => { node.style.transform = transform; contextTransforms.add(node); };
  for (const preview of root.querySelectorAll("[contextTarget]")) {
    const target = document.getElementById(preview.getAttribute("contextTarget"));
    const panel = contextPanel(preview);
    if (!target || !panel) continue;
    const box = panel.getBoundingClientRect(), content = preview.getBoundingClientRect();
    if (content.width <= 0 || content.height <= 0) continue;
    const liveSource = target.getBoundingClientRect(), port = root.getBoundingClientRect();
    const id = Number(preview.dataset.view);
    if (!contextAnchors.has(id) || contextAnchors.get(id).viewportWidth !== port.width) {
      contextAnchors.set(id, contextAnchor(target, port));
    }
    const anchor = contextAnchors.get(id);
    const source = { left: port.left + anchor.left, top: port.top + anchor.top,
      right: port.left + anchor.left + anchor.width, width: anchor.width, height: anchor.height };
    const scale = preview.getAttribute("contextMagnify") === "false" ? 1 : Math.min(1.15, 1 + 26 / Math.max(content.width, content.height));
    const extra = content.height * (scale - 1);
    const trailing = source.left + source.width / 2 > port.left + port.width / 2;
    const dx = trailing ? source.right - content.right - content.width * (scale - 1) / 2
      : source.left - content.left + content.width * (scale - 1) / 2;
    for (const sibling of preview.parentElement.children) {
      if (sibling === preview) continue;
      const side = sibling.getBoundingClientRect();
      if (Math.abs(side.top - content.top) >= 0.01) continue;
      if (side.right <= content.left + 0.01) project(sibling, `translateX(${dx - content.width * (scale - 1) / 2}px)`);
      else if (side.left >= content.right - 0.01) project(sibling, `translateX(${dx + content.width * (scale - 1) / 2}px)`);
    }
    for (let child = preview; child && child !== panel; child = child.parentElement) {
      const bottom = child.getBoundingClientRect().bottom;
      for (const sibling of child.parentElement.children) {
        if (sibling !== child && sibling.getBoundingClientRect().top >= bottom - 0.01) {
          project(sibling, `translateY(${extra / 2}px)`);
        }
      }
    }
    project(preview, `translate(${dx}px, ${extra / 2}px) scale(${scale})`);
    const wanted = source.top + source.height / 2 - (content.top - box.top) - content.height * scale / 2;
    const overflow = Math.max(extra / 2, content.bottom + extra - box.bottom);
    const region = panel.offsetParent?.getBoundingClientRect() || port;
    const minimum = Math.max(region.top, port.top + 8);
    const maximum = Math.min(region.bottom, port.bottom - 8) - box.height - overflow;
    const top = Math.max(minimum, Math.min(wanted, maximum));
    panel.style.top = `${parseFloat(getComputedStyle(panel).top) + top - box.top}px`;
    let branch = preview;
    while (branch.parentElement && branch.parentElement !== panel) branch = branch.parentElement;
    const branchBottom = branch.getBoundingClientRect().bottom;
    const trailingControls = [...panel.children].filter(node => node !== branch && node.getBoundingClientRect().top >= branchBottom - 0.01);
    if (trailingControls.length) {
      const first = Math.min(...trailingControls.map(node => node.getBoundingClientRect().top));
      const last = Math.max(...trailingControls.map(node => node.getBoundingClientRect().bottom));
      const overflow = Math.min(Math.max(0, last - Math.min(region.bottom, port.bottom - 8)), Math.max(0, first - minimum));
      if (overflow) for (const node of trailingControls) project(node, `translateY(${-overflow}px) ${node.style.transform}`);
    }
    const contentRoot = contextContent(target);
    if (contentRoot && !contentRoot.contains(panel)) {
      const scroll = contentRoot.parentElement;
      const scrollDelta = scroll === anchor.scroll && !scroll.contains(panel)
        ? port.top + anchor.scrollTop - scroll.getBoundingClientRect().top : 0;
      if (scrollDelta) project(scroll, `translateY(${scrollDelta}px)`);
      project(contentRoot, `translateY(${source.top + top - wanted - liveSource.top - scrollDelta}px)`);
    }
  }
}
// @ref LLP 1039 D2, LLP 1061 D4 — viewport facts and display preferences, on every change, without debounce.
const pageFacts = pageReporter(agentMode), pageChanged = () => { if (wasm?.exact_set_page && root.childElementCount) { applyBatch(JSON.parse(readOut(wasm.exact_set_page(pageFacts.bits())))); if (wasm.exact_set_root_font_size) applyBatch(JSON.parse(readOut(wasm.exact_set_root_font_size(pageFacts.rootFontSize())))); } }; pageFacts.onChange(pageChanged); addEventListener("resize", pageChanged); // @ref LLP 1069.000 D2, D3
const mediaChanged = () => { if (wasm && root.childElementCount) applyBatch(presence.resize(JSON.parse(readOut(wasm.exact_resize(innerWidth, innerHeight, now(), preferences() | foldBits()))))); requestAnimationFrame(positionContexts); }; addEventListener("resize", mediaChanged); onPreferences(mediaChanged); const foldChanged = mediaChanged; onFold(foldChanged); // @ref LLP 1078 D6 — the posture and the segment counts ride the facts word beside the preferences; the browser resolves the `env(viewport-segment-*)` lengths itself
visualViewport?.addEventListener("resize", () => requestAnimationFrame(positionContexts));
const symbolStyle = document.createElement("style"); document.head.append(symbolStyle);
symbolStyle.textContent = '@property --exact-tint{syntax:"<color>";inherits:false;initial-value:#000}img[data-symbol-path]{background-color:var(--exact-tint)!important;mask-image:var(--exact-symbol-mask);mask-repeat:no-repeat;mask-position:center;mask-size:var(--exact-symbol-fit,100% 100%);mask-origin:content-box;mask-clip:content-box}';
// A tinted raster's `scale-down` (element.rs `host_css`, LLP 1011 §3): `contain` unless its natural size fits the content box, known once it loads.
function tintFit(el) {
  if (!el.style.getPropertyValue("mask-size").includes("--exact-tint-fit")) return;
  if (!el.complete) {
    el.addEventListener("load", () => tintFit(el), { once: true });
    return;
  }
  const cs = getComputedStyle(el);
  const fits = el.naturalWidth <= el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight)
    && el.naturalHeight <= el.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
  el.style.setProperty("--exact-tint-fit", fits ? "auto" : "contain");
}
function refreshSymbols() {
  for (const el of views.values()) {
    if (!(el instanceof HTMLImageElement)) continue; if (!el.hasAttribute("data-symbol-path")) { tintFit(el); continue; }
    const cs = getComputedStyle(el), size = parseFloat(cs.fontSize), weight = Number(cs.fontWeight);
    const path = el.getAttribute("data-symbol-path"), filled = el.hasAttribute("data-symbol-fill"), key = `${path}:${filled}:${size}:${weight}`;
    if (!path && !el.dataset.symbolSource?.startsWith("symbol:sf/") && el.symbolRefusal !== el.dataset.symbolSource) {
      log(`image ${el.dataset.symbolSource} refused: unknown symbol role`); el.symbolRefusal = el.dataset.symbolSource;
    }
    if (el.symbolKey !== key) {
      el.symbolKey = key;
      const point = size, stroke = 1.1 + (Math.max(100, Math.min(900, weight)) - 100) / 400;
      const paint = filled ? 'fill="black" fill-rule="evenodd"' : `fill="none" stroke="black" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round"`;
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}" viewBox="0 0 24 24"><path d="${path}" ${paint}/></svg>`;
      el.symbolMask = `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
      el.symbolPlaceholder = `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}"/>`)}`;
    }
    if (el.getAttribute("src") !== el.symbolPlaceholder) el.src = el.symbolPlaceholder;
    el.style.setProperty("--exact-symbol-mask", el.symbolMask);
    const paddingX = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight), paddingY = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const fits = size <= el.clientWidth - paddingX && size <= el.clientHeight - paddingY;
    const fit = cs.objectFit === "none" || (cs.objectFit === "scale-down" && fits) ? `${size}px ${size}px` : cs.objectFit === "scale-down" ? "contain" : cs.objectFit === "fill" ? "100% 100%" : cs.objectFit;
    el.style.setProperty("--exact-symbol-fit", fit);
  }
}
// The app's `value` into an editor as a person types: the changed middle only (`setRangeText` keeps the selection where a whole assignment throws the caret to the end), never mid-composition — held, applied at compositionend. LLP 1045 D5.
const composing = new WeakSet(), heldValues = new WeakMap(), compositionFlush = new WeakMap();
function writeValue(el, value) {
  if (el.exactMarkup) { el.exactMarkup.setValue(value); return; } if (el instanceof HTMLTextAreaElement) el.exactSourceValue = String(value);
  const old = el.value; if (old === value) { heldValues.delete(el); return; } if (composing.has(el)) { heldValues.set(el, value); return; } heldValues.delete(el);
  if (typeof el.setRangeText !== "function" || valuedControl(el) || old === "" || document.activeElement !== el) { el.value = value; return; }
  let a = 0, z = 0; while (a < old.length && a < value.length && old[a] === value[a]) a++; while (z < old.length - a && z < value.length - a && old[old.length - 1 - z] === value[value.length - 1 - z]) z++;
  const end = old.length - z, text = value.slice(a, value.length - z), { selectionStart: s0, selectionEnd: s1 } = el, carry = (p) => p <= a ? p : p >= end ? p + text.length - (end - a) : a + text.length;
  el.setRangeText(text, a, end, "preserve"); if (el.value !== value) el.value = value; else el.setSelectionRange(carry(s0), Math.max(carry(s0), carry(s1))); }
let markupModule; const markupPending = new WeakSet();
function syncMarkup(el) {
  if (el.exactMarkup) { el.exactMarkup.sync(); return; }
  if (!(el instanceof HTMLTextAreaElement) || el.getAttribute('markup') !== 'markdown' || markupPending.has(el)) return;
  markupPending.add(el);
  (markupModule ??= moduleReady.then(() => loadAfterPaint('./markup-editor.js', 'installMarkupEditor'))).then(install => {
    const id = Number(el.dataset.view), live = node => views.get(id) === node && node.isConnected && !retiredViews.has(node);
    if (composing.has(el)) return; // Keep the native marked range until its final input has committed.
    markupPending.delete(el); if (!live(el) || el.getAttribute('markup') !== 'markdown') return;
    install(el, { live, replace(node) { views.set(id, node); attach(node, id, el.exactHandlers); },
      select(node, payload) { if (live(node) && inputReady && node.exactHandlers.includes('select')) send(wasm.exact_dispatch(id, 21, writeIn(payload), now())); } });
  }).catch(error => console.error('exact: Markdown editor:', error));
}
// LLP 1048.003 D4: `<html data-scrolldocument>` while an element is the page's scroller, which the shell's rule reads (never a `:has()` over the tree).
const scrollDocs = new Set(), markScrollDocument = (on = false) => { for (const el of scrollDocs) if (!el.isConnected) scrollDocs.delete(el); else on ||= el.getAttribute("data-scrolldocument") === "true"; document.documentElement.toggleAttribute("data-scrolldocument", on); };
function applyProps(el, set, clear) {
  syncMedia(el, set, clear); if (set && "data-scrolldocument" in set) scrollDocs.add(el);
  let sandboxChanged = false;
  for (const name of clear || []) {
    if (el instanceof HTMLIFrameElement && name === "src") iframeLoading.set(el, true);
    if (el instanceof HTMLIFrameElement && name === "sandbox" && el.hasAttribute("sandbox")) sandboxChanged = true;
    if (name === "scrollFollowEnd") followScroll(el, false);
    else if (name === "scrollTop" || name === "scrollLeft") {
      const pending = pendingScrolls.get(el); if (pending) delete pending[name];
    }
    else if (name === "text") el.textContent = "";
    else if (name === "value") { el.exactValue = undefined; writeValue(el, ""); }
    else if (name === "checked") { el.exactChecked = undefined; el.checked = false; }
    else if (name === "data-action") { el.removeAttribute(name); el.style.touchAction = ""; }
    else if (name === "autofocus") { el.exactAutofocus = false; el.removeAttribute(name); }
    else if (name === "inert") { el.authoredInert = false; el.inert = false; }
    else el.removeAttribute(name);
  }
  for (const [name, value] of Object.entries(set || {})) {
    if (el instanceof HTMLIFrameElement && name === "sandbox" && el.getAttribute("sandbox") !== value) sandboxChanged = true;
    if (el instanceof HTMLImageElement && name === "src" && value.startsWith("symbol:")) { el.symbolSource = value; }
    else if (name === "scrollFollowEnd") followScroll(el, value === "true");
    else if (name === "scrollTop" || name === "scrollLeft") {
      const offset = Number(value);
      if (Number.isFinite(offset)) pendingScrolls.set(el, { ...pendingScrolls.get(el), [name]: offset });
    } else if (name === "text") { if (el.childElementCount === 0 && el.textContent !== value) el.textContent = value;
    } else if (name === "markupPieces") { renderMarkup(el, value);
    } else if (name === "data-action") {
      el.setAttribute(name, value); el.style.touchAction = "none";
    } else if (name === "value") {
      writeValue(el, value); if (valuedControl(el)) el.exactValue = value;
    } else if (name === "checked") {
      el.exactChecked = value === "true"; el.checked = el.exactChecked;
    } else if (name === "inert") {
      el.authoredInert = value === "true"; el.inert = el.authoredInert;
    } else if (name === "autofocus") { el.exactAutofocus = value === "true"; if (!el.exactAutofocus) el.removeAttribute(name);
    } else if (name === "disabled" || name === "readonly" || (el instanceof HTMLVideoElement && ["autoplay","controls","loop","muted","playsinline","disablepictureinpicture","disableremoteplayback"].includes(name))) {
      if (value === "true") { el.setAttribute(name, ""); if (name === "disabled" && el === document.activeElement) el.blur(); } else el.removeAttribute(name); // a focused node that is disabled loses the focus now, not at the browser's next frame (HTML focus fixup)
    } else {
      const v = (name === "src" || name === "poster") && value.startsWith("app:/") ? globalThis.exact.pickedURL?.(value) ?? "" : (name === "src" || name === "href" || name === "poster") ? localAssetURL(value) : value, same = el.getAttribute(name) === v; // setting what is there reloads an adopted iframe or video
      if (el instanceof HTMLIFrameElement && name === "src" && !same) iframeLoading.set(el, true);
      if (navigates(el, name) && !navigableURL(value)) refuseURL(el, name, value); else if (!same) el.setAttribute(name, v);
    }
  }
  if (!inputReady && (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLButtonElement)) {
    const disabled = set && "disabled" in set ? set.disabled === "true"
      : clear?.includes("disabled") ? false : authoredDisabled.get(el) ?? el.disabled;
    authoredDisabled.set(el, disabled);
    el.disabled = true;
  }
  if (el instanceof HTMLIFrameElement && sandboxChanged) {
    iframeLoading.set(el, true);
    const source = el.getAttribute("src");
    el.setAttribute("src", source ?? "about:blank");
    if (source === null) el.removeAttribute("src");
  }
  if (el instanceof HTMLIFrameElement) commitGuestOrigin(el);
  settleValue(el); syncMarkup(el);
  if ((set && ("viewportFit" in set || "interactiveWidget" in set)) || clear?.some((n) => n === "viewportFit" || n === "interactiveWidget")) syncViewportFit();
}
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
visualViewport?.addEventListener("resize", syncViewportFit);
function syncViewportFit() {
  const first = root.firstElementChild;
  const cover = first?.getAttribute("viewportFit") === "cover";
  const widget = first?.getAttribute("interactiveWidget");
  const meta = document.querySelector('meta[name="viewport"]');
  const want = "width=device-width, initial-scale=1" + (cover ? ", viewport-fit=cover" : "") + (widget ? `, interactive-widget=${widget}` : "");
  if (meta && meta.content !== want) meta.content = want;
  const vv = globalThis.visualViewport;
  root.style.height = widget === "resizes-content" && vv && vv.scale === 1 && !document.documentElement.hasAttribute("data-scrolldocument") ? `${Math.min(innerHeight, vv.height)}px` : "";
}
function attach(el, id, handlers) {
  el.dataset.view = String(id); el.exactHandlers = handlers; if (handlers.length) el.dataset.exactOn = handlers.join(" "); else delete el.dataset.exactOn;
  el.exactFlowEvents = handlers.flatMap(k => ({ press: ["click"], hover: ["pointerenter", "pointerleave"], focus: ["focus"], blur: ["blur"], key: ["keydown"] }[k] ?? []));
  if (el.exactMedia) el.exactMedia.handlers = handlers;
  if (handlers.includes("message")) messageViews.add(id);
  const on = (event, handle) => el.addEventListener(event, (e) => {
    if (views.get(id) === el && !retiredViews.has(el) && (inputReady || event === "load") && !(page?.restoringFocus && (event === "focus" || event === "blur"))) handle(e);
  });
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) { on("compositionstart", () => { clearTimeout(compositionFlush.get(el)); composing.add(el); });
    on("compositionend", () => { compositionFlush.set(el, setTimeout(() => { composing.delete(el); if (views.get(id) !== el || retiredViews.has(el)) return; if (heldValues.has(el)) writeValue(el, heldValues.get(el)); markupPending.delete(el); syncMarkup(el); }, 0)); }); }
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
  // element hears these.
  if (handlers.some((k) => k === "focus" || k === "blur" || k === "key") && !(el instanceof HTMLInputElement || el instanceof HTMLButtonElement) && !el.exactMarkup && !el.hasAttribute("tabindex")) el.tabIndex = 0;
  for (const kind of handlers) {
    if (kind === "press") {
      // A link inside a pressable node is the innermost activation, as a
      // nested press is: the link navigates and the outer press stays out.
      on("click", e => { const a = e.target.closest?.("a[href]"); if (a && a !== el && el.contains(a)) return; focus.press(e, el, () => send(wasm.exact_dispatch(id, 0, 0, now()))); });
    } else if (kind === "pan") {
      let pan;
      on("pointerdown", e => (pan ??= inputHandlers?.pan(el, id, on))?.(e));
    } else if (kind === "scroll") {
      on("scroll", () => { const n = writeIn(`${el.scrollLeft},${el.scrollTop}`); send(wasm.exact_dispatch(id, 13, n, now())); });
    } else if (kind === "swiperight") {
      motion.attachSwipe(el, id, on);
    } else if (kind === "heightrelease") {
      motion.attachHeightDrag(el, id, on);
    } else if (kind === "transformrelease") { motion.attachTransformDrag(el,id,on);
    } else if ((kind === "pointerdown" || kind === "pointerup") && !el.exactPointer) { let p; el.exactPointer = true; on("pointerdown", e => (p ??= inputHandlers?.pointer(el, on, k => { if (views.get(id) === el && !retiredViews.has(el)) send(wasm.exact_dispatch(id, k, 0, now())); }))?.(e)); // @ref LLP 1005 §3: DOM's own pointer down/up (input-glue `pointer`)
    } else if (kind === "contextmenu" || kind === "dblclick") {
      on(kind, (e) => {
        if (el.matches(":disabled") || inertAncestor(el)) return;
        if (e.target.closest("input,textarea,[contenteditable]")) return;
        e.preventDefault(); e.stopPropagation();
        send(wasm.exact_dispatch(id, kind === "contextmenu" ? 10 : 11, 0, now()));
      });
    } else if ((kind === "change" || kind === "cancel") && el.type === "file") { // a picker's files, or its dismissal (LLP 1069.002 D2, D3)
      on(kind, () => { const p = picker().then(m => kind === "change" ? m.change(el, id) : m.cancel(id)); inflight.add(p); p.finally(() => inflight.delete(p)); });
    } else if ((kind === "input" || kind === "change") && el.type === "checkbox") {
      // @ref LLP 1069.001 D4 — the platform flips the box at once; the
      // action decides, and a refusal snaps it back to the committed state.
      on(kind, () => {
        const n = writeIn(String(el.checked)); send(wasm.exact_dispatch(id, kind === "change" ? 24 : 25, n, now()));
        if (el.exactChecked !== undefined && el.checked !== el.exactChecked) el.checked = el.exactChecked;
      });
    } else if (kind === "change") {
      // HTML's `change`: a text field's value committed, on blur or Enter.
      on("change", () => { const n = writeIn(el.value); send(wasm.exact_dispatch(id, 1, n, now())); settleValue(el); });
    } else if (kind === "input") {
      on("input", (e) => {
        const value = el.value;
        if (el.getAttribute("emojiPicker") === "true") {
          if (e.isComposing) return;
          el.value = "";
          const clusters = [...new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(value)];
          if (clusters.length !== 1 || !(/\p{Emoji_Presentation}/u.test(value)
            || (/[\uFE0F\u20E3]/u.test(value) && /\p{Emoji}/u.test(value)))) return;
        }
        const n = writeIn(value); send(wasm.exact_dispatch(id, 23, n, now())); settleValue(el);
      });
    } else if (kind === "hover") {
      // pointerenter/pointerleave: the element's own, not a bubbling mouseover.
      on("pointerenter", () => send(wasm.exact_dispatch(id, 2, 0, now())));
      on("pointerleave", () => send(wasm.exact_dispatch(id, 3, 0, now())));
    } else if (kind === "focus") {
      on("focus", () => send(wasm.exact_dispatch(id, 4, 0, now())));
    } else if (kind === "blur") {
      on("blur", () => send(wasm.exact_dispatch(id, 5, 0, now())));
    } else if (kind === "key") {
      // keydown, the key's name as the web spells it (`e.key`); it bubbles to every ancestor's handler.
      on("keydown", (e) => { const outer = keyEvent; keyEvent = e; try { const n = writeIn(e.key); send(wasm.exact_dispatch(id, 6, n, now())); } finally { keyEvent = outer; } });
    }
    if (kind === "submit" && el.tagName !== "TEXTAREA" && !el.exactMarkup) {
      // The web's implicit submission: Enter in a text input submits — here
      // to the node's `submit` handler, no form needed (and no reload). It is Enter's default: after every `key` handler on the path (the window's listener is last), unless one prevented it.
      on("keydown", (e) => { if (e.key === "Enter" && !e.isComposing && !e.exactSubmit) { e.exactSubmit = true; addEventListener("keydown", w => { if (w === e && !e.defaultPrevented) { e.preventDefault(); send(wasm.exact_dispatch(id, 7, 0, now())); } }, { once: true }); } });
    }
  }
}

function viewFor(op, id) {
  const el = views.get(id);
  if (!el) console.error(`exact: ${op} names missing view ${id}`);
  return el;
}
function apply(batch) {
  // Retire dispatch before children ops can synchronously blur removed views; a leaving view's box is read before any op moves it (LLP 1063).
  for (const op of batch.ops ?? []) {
    if (op.op === "destroy") { const el = views.get(op.id); if (el) retiredViews.add(el); }
  }
  presence.live?.before(batch, views);
  listSelection?.before();
  prepareContexts(batch);
  for (const s of followedScrolls.values()) s.scrolled();
  const focusCommands = [];
  const collectionOp = batch.ops?.find(op => op.op === "collections");
  if (batch.error) console.error("exact:", batch.error);
  for (const op of batch.ops ?? []) {
    try {
      switch (op.op) {
      case "textflow": break; // consumed once after the complete DOM batch
      case "keyframes": motion.keyframes(op.name, op.css); break; // LLP 1055 D7: the page's @keyframes
      case "language": document.documentElement.lang = op.lang; document.documentElement.dir = op.dir; break;
      case "head": (headGlue ??= loadAfterPaint('./document-glue.js', 'documentHead')).then(head => head(op)); break;
      case "router": navigation.apply(op); break;
      case "create": {
        // Canvas overlays use a div; data-surface is the host-owned drawing leaf.
        const el = page?.adopting?.get(op.id) ?? (op.ns ? document.createElementNS(op.ns, op.tag) : document.createElement(op.tag === "canvas" ? "div" : op.tag)); // an adopted document's element (LLP 1048.000 D6); SVG in its namespace (LLP 1055 D4)
        if (op.tag === "canvas" && el.firstElementChild?.dataset.surface === undefined) {
          const surface = document.createElement("canvas");
          surface.dataset.surface = "";
          surface.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1";
          el.append(surface);
        }
        applyProps(el, op.props, []);
        const css = op.css + (el.hasAttribute("data-action") ? ";touch-action:none" : ""); if ((el.getAttribute("style") ?? "") !== css) el.style.cssText = css; // an adopted element's is already there
        attach(el, op.id, op.handlers);
        views.set(op.id, el); if (op.tag.includes("-") && !op.ns && !el.exactNative) nativeCreate(el, op.id);
        listView(el, op.id, collectionOp?.items.some(item => item.view === op.id));
        break;
      }
      case "props": {
        const el = viewFor("props", op.id);
        if (el) applyProps(el, op.set, op.clear);
        break;
      }
      case "style": {
        const el = viewFor("style", op.id);
        if (el) { exact.gpu?.beforeStyle(el); motion.style(op.id, op.css); if (el.hasAttribute("data-action")) el.style.touchAction="none"; exact.gpu?.afterStyle(el); }
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
        const skip = (n) => { while (n?.hasAttribute("data-surface") || n?.hasAttribute("data-exiting")) n = n.nextElementSibling; return n; }; // a leaving view stays (LLP 1063)
        let cursor = skip(el.firstElementChild);
        for (const child of want) {
          if (child === cursor) { cursor = skip(cursor.nextElementSibling); continue; }
          el.insertBefore(child, cursor);
        }
        while (cursor) { const next = skip(cursor.nextElementSibling); cursor.remove(); cursor = next; } settleValue(el); // a select shows its committed value among its new options
        break;
      }
      case "animate": { motion.animate(op); break; }
      case "timelines": { timelinesMoved = true; break; }
      case "retire-motion": { motion.retire(op.id, op.property, op.token, op.runtime); break; }
      case "height-drag": { motion.heightBinding(op); break; }
      case "transform-drag": { motion.transformBinding(op); break; }
      case "reorder-drag": { arrange.binding(op); break; }
      case "reorder-state": { arrange.state(op); break; }
      case "canvas2d": { pieces.canvas2d(op); break; } // LLP 1056 D7
      case "surface": {
        // A canvas's inputs (LLP 1009 D2): to the GPU module when it is
        // loaded, queued until then. The module itself is fetched only
        // after a rendering opportunity, and only when a canvas exists.
        if (globalThis.exact.gpu) globalThis.exact.gpu.surface(op.id, op.name, op.values);
        else {
          const pending = (globalThis.exact.pendingSurfaces ??= []);
          const queued = pending.find((entry) => entry.id === op.id && entry.generation === incarnation);
          if (queued) { queued.name = op.name; queued.values = op.values; }
          else { pending.push({ id: op.id, name: op.name, values: op.values, generation: incarnation }); requestAnimationFrame(() => requestAnimationFrame(loadGpuIfNeeded)); }
        }
        break;
      }
      case "grants": { grantSet = createGrantSet(op.set); grants = rawGrantText(grantSet).split('\n').filter(Boolean); unparsed = grantError(grantSet) ?? ""; if (unparsed) console.warn("exact:", unparsed); if (grants.some(l => /^\s*auth\.session /.test(l))) authHost ??= afterNativePaint().then(() => loadAfterPaint('./auth-glue.js', 'authHost')).then(h => authHost = h); break; } case "auth": { const inc = incarnation, env = { agent: agentMode, log, call: r => JSON.parse(readOut(wasm.exact_auth(writeIn(JSON.stringify(r))))), deliver: t => deferFulfill(inc, t, 9, 0, "", new Uint8Array()), active: t => holds(t, inc) }; if (authHost?.arm) authHost.arm(op, env); else if (agentMode && authHost) authHost.then(h => h.arm(op, env)); else { env.call({ op: "arm", ticket: op.ticket, origin: location.origin, popup: false }); env.deliver(op.ticket); } break; } // LLP 1069.006 D4: armed in the press's call stack; unloaded glue is 428
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
      case "storage": {
        const requestIncarnation=incarnation;
        const p=Promise.resolve().then(async()=>{
          await moduleReady; if(!inputReady)throw new Error('data executor is unavailable');
          if(requestIncarnation!==incarnation)throw new Error('storage source unloaded');
          if(!storageRequests){
            const app=globalThis.exact.compat.inputs.app;
            const pending=loadAfterPaint('./storage-request.js','createStorageRequests').then(create=>create(app,grantSet)).catch(error=>{if(storageRequests===pending)storageRequests=null;throw error;});
            storageRequests=pending;
          }
          const service=await storageRequests;
          if(requestIncarnation!==incarnation)throw new Error('storage source unloaded');
          return service.run(op.payload,scopedGrantSet(grantSet,op.scope));
        }).then(bytes=>safelyFulfill(requestIncarnation,op.ticket,5,0,"",bytes))
          .catch(error=>safelyFulfill(requestIncarnation,op.ticket,3,0,"",encoder.encode(String(error))));
        track(p,op.ticket);break;
      }
      case "refuse": { deferFulfill(...refusal(op, incarnation)); break; }
      case "continue": {
        const requestIncarnation = incarnation;
        const p = Promise.resolve().then(() => moduleLoader.run(op.token))
          .then(result => safelyFulfill(requestIncarnation, op.ticket, 0, 200, "", encoder.encode(JSON.stringify(result))))
          .catch(error => safelyFulfill(requestIncarnation, op.ticket, 3, 0, "", encoder.encode(String(error))));
        track(p, op.ticket);
        break;
      }
      case "surfaceWork": {
        const requestIncarnation=incarnation;
        const p=Promise.resolve().then(async()=>{
          if(op.refusal)throw Object.assign(new Error(op.refusal),{kind:2});
          if(!surfaceGranted(op))throw Object.assign(new Error(`refused by grant: surface ${op.name}${unparsed&&`: ${unparsed}`}`),{kind:2});
          loadGpuIfNeeded();await gpuLoading;
          if(!exact.gpu)throw Object.assign(new Error(`surface ${op.name}: expected one live surface, found 0`),{kind:2});
          if(requestIncarnation!==incarnation||wasm.exact_request_active(op.ticket)!==1)throw Object.assign(new Error('surface request retired'),{kind:4});
          let bytes;
          if(op.mode==="restore"){
            if((op.body??"").length>HOST_WORK_BASE64)throw Object.assign(new Error('surface restore exceeds 16 MiB'),{kind:2});
            bytes=Uint8Array.from(atob(op.body??""),c=>c.charCodeAt(0));
          }
          return exact.gpu.surfaceWork(op.name,op.mode,bytes,()=>requestIncarnation===incarnation&&wasm.exact_request_active(op.ticket)===1);
        }).then(bytes=>safelyFulfill(requestIncarnation,op.ticket,op.mode==="capture"?6:7,0,"",bytes??new Uint8Array()))
          .catch(error=>safelyFulfill(requestIncarnation,op.ticket,error.kind??3,0,"",encoder.encode(String(error.message??error))));
        track(p,op.ticket);break;
      }
      case "request": {
        // A safe read the runner lets go of is aborted after the commit that
        // forgot it (`letGo`), as the native executor does: a write that
        // was sent was sent, and runs on, uncounted (LLP 1016 D5).
        const requestIncarnation = incarnation, controller = new AbortController(), started = performance.now();
        let p, first, messages = 0; const opened = new Promise(r => { first = r; });
        const host = {
          grantSet, loadPageNative, moduleLoader, localAssetURL, controllers, controller,
          active: () => requestIncarnation === incarnation,
          // A stream's message (LLP 1016.000): after its first, the stream is open, not in flight, so `clock settle`
          // stops waiting on it (D5) — what is counted ends there, so a wait already racing it wakes (LLP 1069.004).
          message: (m) => { if (messages++ === 0) first(); safelyFulfill(requestIncarnation, op.ticket, 8, 0, `event: ${m.event}\nid: ${m.id}\ncoalesced: ${m.coalesced}`, encoder.encode(m.data)); },
        };
        controllers.add(controller);
        if (op.url !== "exact-native:" && /^(GET|HEAD)$/i.test(op.method)) forgettable.set(controller, op.ticket);
        p = httpHelpers().then(({ request }) => request(op, host))
          .then(r => safelyFulfill(requestIncarnation, op.ticket, r.kind, r.status, r.headers, r.body, performance.now() - started))
          .catch(error => safelyFulfill(requestIncarnation, op.ticket, 1, 0, "", encoder.encode(String(error)), performance.now() - started));
        track(Promise.race([p, opened]), op.ticket); p.finally(() => { forgettable.delete(controller); controllers.delete(controller); });
        break;
      }
      case "command": {
        // A capability an action called (LLP 1005 §3). `setScheme` is the
        // document's colour scheme — what `prefers-color-scheme` would be.
        // `system` is CSS's `light dark`: the page supports both and the
        // user's preference decides, which is what "follow the system" is on
        // the web. `light`/`dark` are the property's own values.
        if (op.name === "setScheme") { const s = String(op.args[0] ?? ""); document.documentElement.style.colorScheme = s === "system" ? "light dark" : s; } else if (op.name === "haptic") navigator.vibrate?.(op.args?.[0] === "selection" ? 5 : 12); // LLP 1077 D14
        else if (op.name === "focus" || op.name === "selectText" || op.name === "blur") focusCommands.push({ name: op.name, args: op.args });
        else if (op.name === "preventDefault") keyEvent?.preventDefault();
        else if (op.name === "postMessage") { // the inverse of `message=`: text into the named surface, every one in order
          const text = String(op.args?.[0] ?? ""), name = String(op.args?.[1] ?? ""), at = now();
          if (globalThis.exact.gpu) globalThis.exact.gpu.post(name, text, at);
          else if ((globalThis.exact.pendingPosts ??= []).filter(p => p.name === name).length >= POST_BOUND) log(`postMessage: dropped: ${POST_BOUND} posts already wait for surface "${name}"`);
          else globalThis.exact.pendingPosts.push({ name, text, at, generation: incarnation });
        }
        else if (op.name === "showPicker") { // LLP 1069.002 D2, D9: the element's own picker, inside the press's activation; under the agent, a hold
          const el = [...views.values()].find(el => el.id === op.args?.[0] && el.type === "file");
          if (agentMode) { const r = ask({ op: "showPicker", id: String(op.args?.[0] ?? "") }); if (r.error) console.warn("exact:", r.error); }
          else if (!el) log(`picker: refused: no file input with id "${op.args?.[0]}"`);
          else try { el.showPicker(); } catch (e) { log(`picker: refused: ${e.name}`); const p = picker().then(m => m.cancel(Number(el.dataset.view))); inflight.add(p); p.finally(() => inflight.delete(p)); }
        }
        else if (op.name === "format") { const owner = incarnation, run = () => { const el = [...views.values()].find(el => el.id === op.args?.[0]); if (inputReady && incarnation === owner) el?.exactMarkup?.format(op.args[1], op.args[2] ?? ''); }; if (markupModule) markupModule.then(run); else run(); }
        else if (op.name === "openURL") {
          if (op.args?.length !== 1 || typeof op.args[0] !== "string") {
            console.error("exact: openURL requires one string");
          } else {
            try {
              const target = navigableURL(new URL(op.args[0]).href);
              if (!target) throw Error("unsupported external URL scheme");
              window.open(target, "_blank", "noopener,noreferrer");
            } catch (error) { console.error("exact: openURL refused", String(error)); }
          }
        }
        else if (op.name === "copyText") {
          if (op.args?.length !== 1 || typeof op.args[0] !== "string") {
            console.error("exact: copyText requires one string");
          } else if (!navigator.clipboard?.writeText) {
            console.error("exact: copyText unavailable; a secure clipboard context is required");
          } else {
            // Start inside the input dispatch while browser user activation
            // is live. No clipboard read or focus/selection manipulation.
            const pending = navigator.clipboard.writeText(op.args[0])
              .catch(error => console.error("exact: copyText failed", String(error)));
            inflight.add(pending);
            pending.finally(() => inflight.delete(pending));
          }
        }
        else if (op.name === "saveFile") { const [id, from, suggestedName] = op.args ?? [], r = JSON.parse(readOut(wasm.exact_command(writeIn(JSON.stringify({ command: "saveFile", id, from, suggestedName, agent: agentMode }))))), chosen = r.present && typeof showSaveFilePicker === "function" ? showSaveFilePicker({ suggestedName: r.suggestedName }) : null; // LLP 1069.010 D3: the runner rules; the save picker starts inside the press's activation, else a download
          if (r.present || r.view != null) { chosen?.catch(() => {}); const p = picker().then(m => m.save(r, chosen)); inflight.add(p); p.finally(() => inflight.delete(p)); } }
        else if (/^show(OpenFile|Directory|SaveFile)Picker$/.test(op.name)) { // LLP 1069.010 D2: the runner rules; a browser without the picker refuses
          const [id, second] = op.args ?? [], r = JSON.parse(readOut(wasm.exact_command(writeIn(JSON.stringify({ command: op.name, id, multiple: second === true, suggestedName: typeof second === "string" ? second : undefined, agent: agentMode, available: typeof globalThis[op.name] === "function" }))))); if (r.present || r.view != null) { const p = documentsGlue().then(m => m.show(r, op.name)); inflight.add(p); p.finally(() => inflight.delete(p)); } }
        else if (op.name === "share") {
          // LLP 1069.003: the runner rules (refused, or held for the agent, D6);
          // else the browser's sheet, started inside the input dispatch while
          // activation is live (D4). The outcome is a journal line (D2).
          const [title, text, url] = op.args ?? [], ruling = JSON.parse(readOut(wasm.exact_command(writeIn(JSON.stringify({ command: "share", title, text, url, source: op.source, agent: agentMode })))));
          if (ruling.present && typeof navigator.share !== "function") log("share: refused: unavailable");
          else if (ruling.present) navigator.share(Object.fromEntries(Object.entries({ title, text, url }).filter(([, v]) => v != null)))
            .then(() => "share: shared", e => e?.name === "AbortError" ? "share: dismissed" : `share: refused: ${e?.name ?? e}`).then(log);
        }
        else console.warn(`exact: unknown command ${op.name}`);
        break;
      }
      case "destroy": {
        arrange.destroy(op.id);
        motion.destroy(op.id);
        const el = views.get(op.id); if (el) { retiredViews.add(el); el.exactMarkup?.destroy(); el.exactNative?.destroy(); if (el instanceof HTMLVideoElement) { globalThis.exact.removeMedia?.(el); el.pause(); el.removeAttribute("src"); el.load(); } forgetList(el); followScroll(el, false); messageFrames.delete(el); if (!presence.live?.keeps(el)) el.remove(); }
        views.delete(op.id); messageViews.delete(op.id); globalThis.exact.gpu?.destroy(op.id); break;
      }
      case "roots": {
        const roots = [];
        for (const id of op.ids) {
          const el = viewFor("roots", id);
          if (el) roots.push(el);
        }
        if (roots.length !== root.children.length || roots.some((el, i) => root.children[i] !== el)) root.replaceChildren(...roots);
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
  navigation.project(root, log);
  refreshSymbols();
  for (const snapshot of collectionOp?.items ?? []) followScroll(views.get(snapshot.view), false);
  for (const s of followedScrolls.values()) settleFollow(s);
  const jumps = []; // a collection builds a jump's rows, then moves, on its own axis (LLP 1050.000 §6)
  for (const [el, offsets] of pendingScrolls) if (el.isConnected) {
    // Mirroring the current offset must not restart snapping or cancel a pan.
    // `scroll-behavior: smooth` (the row's CSS) animates the assignment itself (CSSOM View §7),
    // once; the agent's clock cannot seek that animation, so under it the scroll lands at once.
    for (const [name, offset] of Object.entries(offsets)) if (el[name] !== offset) {
      if ((name === "scrollTop" || name === "scrollLeft") && lists.has(el)) jumps.push([Number(el.dataset.view), offset, name]);
      else if (agentMode && el.style.scrollBehavior === "smooth") el.scrollTo({ [name === "scrollTop" ? "top" : "left"]: offset, behavior: "instant" });
      else el[name] = offset;
    }
    const s = followedScrolls.get(el); if (s) rememberScroll(s);
  }
  pendingScrolls.clear();
  if (collectionOp) collections.commit(collectionOp.items);
  for (const [view, offset, name] of jumps) collections.jump(view, offset, name);
  listSelection?.after();
  syncLists();
  // Focusing can dispatch an action: every node/value of the batch is committed before its focus handler runs.
  runFocusCommands(focusCommands, { root, ready: inputReady, inertAncestor, log });
  focusAutofocus();
  positionContexts(); presence.live?.after(batch, views); markScrollDocument();
  return batch.timers;
}
function applyBatch(batch) {
  if (page?.hold(batch) || presence.hold(batch)) return { timers: batch.timers, batch }; textflow?.beforeBatch(batch);
  const began = frameSampler ? performance.now() : 0; // what applying it cost, for the next frame's record (LLP 1079 D3)
  globalThis.exact.applyDepth = (globalThis.exact.applyDepth ?? 0) + 1; try {
  const timers = apply(batch); letGo();
  motion.commit(); arrange.commit();
  if (agentMode) {
    // What the ops since the last marker started belongs to that marker's
    // time — register before the clock moves on to where the batch landed.
    register(agentClock);
    const moved = batch.clock != null && batch.clock > agentClock; if (moved) agentClock = batch.clock;
    // A same-clock seek registers the batch's animations; its timelines op, not the agent, owns post-commit reconciliation.
    followOnSeek = moved; try { seek(agentClock); } finally { followOnSeek = true; }
    if (timelinesMoved) motion.followTimelines(); arrange.commit();
  } else if (timelinesMoved) motion.followTimelines(); // a boot or commit while drag timelines are bound (LLP 1057.003 D4); the agent's seek follows them
  timelinesMoved = false;
  flowBatch(batch);
  return { timers, batch };
  } finally { const outer = --globalThis.exact.applyDepth === 0; if (outer) { globalThis.exact.gpu?.drainRecords(); globalThis.exact.gpu?.layout?.(); } frameSampler?.batch(batch.seq, outer ? performance.now() - began : 0); }
}
function send(len) {
  return applyBatch(JSON.parse(readOut(len))).timers;
}
// LLP 1019 D5: FontFace loading is part of host boot. The DOM remains empty until every local face loaded, or 100 ms elapsed. At the barrier, install
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
// LLP 1016: grants come from one whole-set parse; `unparsed` names why an
// invalid full set admits nothing while retaining raw lines for child scopes.
// The agent's settle waits on active fetches; replies return to the wasm.
let grants = [], grantSet = null, unparsed = "";
let storageRequests = null;
const inflight = new Set();
const controllers = new Set(), forgettable = new Map();
let incarnation = 0, authHost = null; // auth-glue.js once an auth.session grant asks for it (LLP 1069.006 D4)
// A ticket's work, in flight while the runner holds the ticket (LLP 1016 D5):
// a superseded or forgotten one never holds `clock settle`. Work without a
// ticket counts until it ends.
function track(p, ticket) { if (ticket != null) { p.ticket = ticket; p.owner = incarnation; } inflight.add(p); p.finally(() => inflight.delete(p)); }
const holds = (ticket, owner = incarnation) => owner === incarnation && wasm?.exact_request_active(ticket) === 1;
const waiting = () => [...inflight].filter(p => p.ticket == null || holds(p.ticket, p.owner));
// After each commit: abort the reads whose tickets the runner let go of.
function letGo() { for (const [controller, ticket] of forgettable) if (!holds(ticket)) { forgettable.delete(controller); controller.abort(); } }
const HOST_WORK_BYTES=16*1024*1024, HOST_WORK_BASE64=4*Math.ceil(HOST_WORK_BYTES/3);
function surfaceGranted(op) {
  const admitted=grants.map(g=>g.trim()).filter(Boolean), scoped=(op.scope==null?admitted:op.scope.split("\n").map(g=>g.trim()).filter(Boolean));
  const need=`surface.${op.mode==="capture"?"read":"write"} ${op.name}`;
  return scoped.every(g=>admitted.includes(g))&&scoped.includes(need);
}
function fulfill(requestIncarnation, ticket, kind, status, headersText, body, elapsedMs) {
  // Boot reuses ticket IDs; an old incarnation's completion must not land.
  if (!wasm || requestIncarnation !== incarnation) return;
  if (Number.isFinite(elapsedMs)) log(`reply ${ticket}: wall ${Math.max(0, Math.round(elapsedMs))} ms`); // first: its `writeIn` reuses the buffer the reply fills
  const h = encoder.encode(headersText);
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
const deferFulfill = deferredFulfill(safelyFulfill, inflight);
// The agent API's page half (LLP 1012). `tree`, `state`, `logs`, and
// `settle` go to the wasm (`exact_agent`); `layout` reads the browser's
// boxes — the only layout the web host has; `clock` moves both clocks to
// one instant: the runner's (`exact_advance`, each timer at its own time)
// and every animation the browser holds (`Animation.currentTime`, LLP 1002
// D3), and the GPU module's picture. `tap`, `type`, and `screenshot` are
// the driver's, over CDP: real input, real pixels.
function ask(request) { // entering an unloaded stage traps mid-call; callers load it first (LLP 1047.000 §9)
  if (!stageLoaded('inspection')) throw new Error('inspection is a stage that has not loaded (LLP 1047.000): load it before asking');
  return JSON.parse(readOut(wasm.exact_agent(writeIn(JSON.stringify(request)))));
}
// Staged capabilities (the core's `exact.stages`; none unsplit): modules over its memory and tables, named by digest (LLP 1047.000 §9).
let stages = {}; const stageLoads = new Map(), stageLoaded = (name) => !stages[name] || stageLoads.get(name)?.loaded === true;
function loadStage(name) { if (!stages[name] || !wasm) return Promise.resolve(); let load = stageLoads.get(name); if (load) return load.promise;
  load = { loaded: false }; stageLoads.set(name, load); return load.promise = WebAssembly.instantiateStreaming(fetch(new URL(stages[name], import.meta.url)), { primary: wasm })
    .then(() => { load.loaded = true; }, (error) => { stageLoads.delete(name); throw new Error(`the ${name} stage (${stages[name]}) did not load: ${error}`); }); }
// A line for the runner's journal (LLP 1012 §3): what the page refused, and why.
function log(line) {
  if (wasm) wasm.exact_log(writeIn(line));
}
// `layout <node>` (LLP 1035.002 D1): the runner's rows and their sources
// for one node (`node`, answered in the wasm), then what the page knows —
// the box in the viewport and relative to its parent, the scroll and clip
// chains above it, whether it is hidden, inert, in the viewport or clipped
// away, the element that carries it — and the browser's own computed value
// of every inherited row: the oracle printed beside the kernel's answer.
// Spaces the page cannot observe (a window, a screen) are absent.
const INHERITED_CSS = {
  text_color: "color", font_family: "font-family", font_size: "font-size", font_weight: "font-weight",
  font_style: "font-style", line_height: "line-height", letter_spacing: "letter-spacing",
  font_variant_numeric: "font-variant-numeric", direction: "direction", white_space: "white-space", overflow_wrap: "overflow-wrap", text_align: "text-align",
};
function nodeDetail(id, plan = false) {
  const el = views.get(id);
  if (!el || !el.isConnected) return { error: `stale node #${id}` };
  const node = ask({ op: "node", id, ...(plan ? { plan: true } : {}) });
  if (node.error) return node;
  // The kernel's layout never runs on the web (LLP 1007 §9): its frames are
  // not observations here, so they are absent rather than zeros.
  delete node.frame;
  delete node.absolute;
  delete node.content;
  const r2 = (x) => Math.round(x * 100) / 100;
  const rect = (r) => ({ x: r2(r.x), y: r2(r.y), w: r2(r.width), h: r2(r.height) });
  const idOf = (e) => { for (const [i, v] of views) if (v === e) return i; return null; };
  const r = exact.gpu?.placementHidden(el) ? new DOMRect() : viewBox(el);
  node.space = {
    viewport: rect(r),
    local: { w: r2(el.clientWidth), h: r2(el.clientHeight) },
    capture: { scale: devicePixelRatio },
  };
  const scroll = [], clip = [];
  let clipped = r.width === 0 || r.height === 0;
  for (let a = el.parentElement; a; a = a.parentElement) {
    const aid = idOf(a);
    if (aid == null) continue;
    const cs = getComputedStyle(a);
    if (a.dataset.scroll === "true") scroll.unshift({ id: aid, sx: r2(a.scrollLeft), sy: r2(a.scrollTop) });
    const clips = (cs.overflowX !== "visible" || cs.overflowY !== "visible" ? ["overflow"] : []).concat(cs.clipPath !== "none" ? ["clip-path"] : []);
    for (const kind of clips) {
      clip.unshift({ id: aid, kind });
      const c = a.getBoundingClientRect();
      if (r.right <= c.left || r.left >= c.right || r.bottom <= c.top || r.top >= c.bottom) clipped = true;
    }
  }
  if (scrollX || scrollY) scroll.unshift({ viewport: true, sx: r2(scrollX), sy: r2(scrollY) });
  node.scroll = scroll;
  node.clip = clip;
  node.visible = {
    hidden: el.checkVisibility ? !el.checkVisibility({ visibilityProperty: true }) : false,
    inert: !!inertAncestor(el),
    inViewport: r.right > 0 && r.bottom > 0 && r.left < innerWidth && r.top < innerHeight,
    clipped,
  };
  node.native = { element: el.localName }; if (el.hasAttribute("data-symbol-source")) { const source = el.dataset.symbolSource, name = source.slice(source.startsWith("symbol:sf/") ? 10 : 7), found = !!el.dataset.symbolPath; node.native.symbol = { source, name, found, ...(!found ? { reason: source === "symbol:sf/" ? "empty" : source.startsWith("symbol:sf/") ? "platform" : "role" } : {}) }; }
  const cs = getComputedStyle(el);
  node.browser = Object.fromEntries(Object.entries(INHERITED_CSS).map(([row, prop]) => [row, cs.getPropertyValue(prop)]));
  const flow = textflow?.facts(id);
  if (flow) { node.flow = flow; node.flow_shapes = flow.shapes; }
  node.observed = { clock: now(), wall: Date.now() };
  return node;
}
function tree(request) {
  const reply = ask(request);
  for (const node of reply.nodes ?? []) {
    const el = views.get(node.id);
    node.focused = el === document.activeElement;
    if (el?.exactNative) node.module = el.exactNative.status();
    if (!(el instanceof HTMLIFrameElement)) continue;
    node.url = el.getAttribute("src") ?? "";
    node.loading = iframeLoading.get(el) !== false;
    const guest = guestOutline(el);
    if (guest !== null) node.guest = guest;
  }
  return reply;
}
const SETTLE_DEADLINE_MS = 20_000;
async function waitForInflight(deadline) {
  if (!waiting().length) return true;
  let timer;
  const helpers = await Promise.race([httpHelpers(), new Promise(resolve => { timer = setTimeout(() => resolve(null), Math.max(0, deadline - performance.now())); })]);
  clearTimeout(timer);
  return helpers ? helpers.waitForInflight(waiting, deadline) : false;
}
async function settleGpu() { loadGpuIfNeeded(); await gpuLoading; return await globalThis.exact.gpu?.settled() ?? []; }
function gpuPendingReply(request, pending) { const names = pending.map(item => item.name ?? "GPU work"); return request.op === "clock" ? {clock:agentClock, settled:false, reason:"gpu", pending:names} : {error:`GPU is not settled: ${names.join(", ")}`, pending:names}; }
function agent(request) { if (!stageLoaded('inspection')) return loadStage('inspection').then(() => agent(request)); return agentMode && gpuInPlay() ? settleGpu().then(pending => pending.length ? gpuPendingReply(request, pending) : agentNow(request)) : agentNow(request); } // synchronous once inspection is in (LLP 1043.000 D7/D8)
function agentNow(request) { const r = agentReply(request), decorate = globalThis.exact.gpu?.decorate; return decorate ? decorate(request, r) : r; }
function agentReply(request) {
  try {
    if (!wasm) return { error: "not booted" };
    if (request.entity !== undefined || request.world === true || request.contact !== undefined) return globalThis.exact.gpu?.handle(request, ask, tagged) ?? { error: `view ${request.id} has no world` };
    if ((request.op === "tap" || request.op === "type") && request.ticket !== undefined) { // a held device request, by ticket (LLP 1069.007 D4)
      const { files, ...held } = request, r = ask(held); if (r.capability === "auth") deferFulfill(incarnation, r.ticket, 9, 0, "", new Uint8Array()); // a picker's answer is delivered once the runner took it (LLP 1069.002 D9); an auth answer, the runner settled (LLP 1069.006 D7)
      const picked = { "open-file": "showOpenFilePicker", "open-directory": "showDirectoryPicker", "save-file": "showSaveFilePicker" }[r.capability];
      if (picked && r.node != null) return documentsGlue().then(m => m.answer(r, picked, files)).then(() => tagged(r)); // LLP 1069.010 D2
      if (r.capability === "export" && r.node != null) return picker().then(m => m.answerSave(r, held.text)).then(out => tagged({ ...r, ...out })); // LLP 1069.010 D3: the bytes go back to the driver
      return r.capability === "pick" && r.node != null ? picker().then(m => m.answer(r.node, r.answered === "cancel" ? null : files ?? [])).then(() => tagged(r)) : tagged(r);
    }
    if (request.op === "perf" && request.frames) return { virtual: true }; // the agent's clock presents no frame (LLP 1079 D4)
    switch (request.op) {
      case "state": {
        const st = ask(request);
        if (st.error) return st;
        st.presence = presence.live?.observation() ?? [];
        const r2 = (x) => Math.round(x * 100) / 100;
        const idOf = (e) => { for (const [i, v] of views) if (v === e) return i; return null; };
        const active = document.activeElement && document.activeElement !== document.body ? document.activeElement : null;
        const editor = active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement || active?.exactMarkup ? idOf(active) : null;
        st.media = [...views].filter(([, el]) => el instanceof HTMLVideoElement).map(([id, el]) => ({ id, state: { currentTime: el.currentTime, duration: Number.isFinite(el.duration) ? el.duration : null, paused: el.paused, muted: el.muted, volume: el.volume, playbackRate: el.playbackRate, readyState: el.readyState, videoWidth: el.videoWidth, videoHeight: el.videoHeight, src: el.currentSrc, error: el.error ? { code: el.error.code, message: el.error.message } : null, renderer: "HTMLVideoElement" } }));
        st.focus = { logical: active ? idOf(active) : null, editor, responder: active ? active.localName : null, pending: null };
        const overlap = Math.max(0, innerHeight - (globalThis.visualViewport?.height ?? innerHeight));
        const policy = document.querySelector("[interactiveWidget]")?.getAttribute("interactiveWidget") ?? "resizes-visual";
        st.keyboard = { visible: overlap > 0, overlap: r2(overlap), policy, interactive: false };
        st.navigation = navigation.observation(root); st.window = { title: document.title }; if (page) st.adopted = page.adopted === true; // LLP 1048.000 D6
        return st;
      }
      case "layout": {
        // Every view in the document (attached, whether or not it lies in
        // the viewport), by id, in the viewport's space with every scroll
        // offset and transform folded in, to two decimals.
        const r2 = (x) => Math.round(x * 100) / 100;
        const nodes = [];
        for (const [id, el] of [...views].sort((a, b) => a[0] - b[0])) {
          if (!el.isConnected) continue;
          const r = exact.gpu?.placementHidden(el) ? new DOMRect() : viewBox(el);
          const n = { id, x: r2(r.x), y: r2(r.y), w: r2(r.width), h: r2(r.height) };
          if (el instanceof HTMLIFrameElement) {
            const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
            n.hit = hit === el;
          }
          if (el.dataset.scroll === "true") { n.sx = r2(el.scrollLeft); n.sy = r2(el.scrollTop); }
          nodes.push(n);
        }
        const reply = { clock: now(), viewport: { w: innerWidth, h: innerHeight }, env: environment(), nodes };
        if (request.id != null) {
          const detail = nodeDetail(request.id, request.plan === true);
          if (detail.error) return detail;
          reply.node = detail;
        }
        if (request.agree) return tagged({ clock: reply.clock, viewport: reply.viewport, agreement: { unavailable: "not implemented: app drives run the JS target" } }); else if (request.native && reply.node) { delete reply.nodes; reply.node.native.subviews = { unavailable: "the DOM is the tree; layout <target> names the element" }; } // @ref LLP 1080.001 D1, D2
        return tagged(reply);
      }
      case "prefer": { // @ref LLP 1069.000 D6 — the page group; the driver sets media through CDP. @ref LLP 1078 D7 — the fold group: an empty one re-reads the browser (the driver's CDP override), a filled one is the substitute.
        try { if (request.page) pageFacts.prefer(request.page); if (request.fold) preferFold(Object.keys(request.fold).length ? request.fold : null); } catch (e) { return { error: e.message }; }
        pageChanged(); foldChanged();
        return tagged({ page: { ...pageFacts.read() }, fold: foldEnv() });
      }
      case "focus": {
        const el = views.get(request.id);
        if (!el) return { error: `no view ${request.id}` };
        el.focus(); // any element, as the JS target's agent: a key at a grid is its keydown (docs/contract-grammar.md#keys)
        if (request.select !== false) el.select?.();
        return { ok: true };
      }
      case "tap": {
        const i = request.into; // @ref LLP 1070.000 §5 — the runner's request, its batch applied now
        if (i) { send(wasm.exact_into_view(request.id, writeIn(`${i.key ?? ''}\n${i.block ?? 'start'}\n${i.inline ?? 'nearest'}`))); return tagged({ tapped: request.id, into: i }); }
        const frame = views.get(request.id);
        if (request.history !== undefined) return navigation.travel(frame, request.history);
        if (frame && (frame.closest("[inert]") || ["hidden", "collapse"].includes(getComputedStyle(frame).visibility)))
          return { handled: true, error: `view ${request.id} is hidden or inert` };
        return frame instanceof HTMLIFrameElement ? guestTap(frame, request) : { guest: false };
      }
      case "type": {
        const frame = views.get(request.id);
        if (frame?.hasAttribute("navigationBack") && request.key == null) {
          const batch = globalThis.exact.navigate(request.text ?? "");
          return { typed: request.id, delivery: "recognized", handled: true, ...(batch.error ? { error: batch.error } : {}) };
        }
        return typedControl(frame) && request.key == null ? typeControl(frame, request) : frame instanceof HTMLIFrameElement ? guestType(frame, request) : { guest: false }; // a control's value (LLP 1069.001 D9)
      }
      case "clock": // then the offset at the new virtual date, in case it crossed a DST change (LLP 1069.007 D2)
        return clock(request).then((r) => { if (!r.error && wasm.exact_set_time) applyBatch(JSON.parse(readOut(wasm.exact_set_time(...reportTime(agentClock))))); return tagged(r); });
      case "tree": return tree(request);
      case "tags": return ask(request);
      case "reveal": return tagged(reveal(views.get(request.id), request.id)); // before a tap or a type (navigation.js)
      case "axStamp": // @ref LLP 1080.002 D4 — each live view's id where CDP's DOM snapshot reads it, and the document's nonce
        for (const [id, el] of views) if (el.isConnected && el.getAttribute("data-agent-view") !== String(id)) el.setAttribute("data-agent-view", id);
        return tagged({ nonce: performance.timeOrigin });
      default:
        return ask(request);
    }
  } catch (e) {
    return { error: String(e) };
  }
}
// @ref LLP 1043.000 §3 D7/D8 — reads keep the last settled facts (LLP 1012).
// Await flow only when requested; ordinary agent calls retain their return types.
async function agentSettled(request) {
  const pieceLoad = pieces.pending(); if (pieceLoad) await pieceLoad; if (markupModule) await markupModule;
  if (flowLoading) await flowLoading;
  if (textflow) await textflow.settle();
  return agent(request);
}

// Every reply carries the runner's `epoch`, `incarnation` and `clock` (LLP
// 1035.002 D3), read after the operation; a reply's own `clock` (where a
// `clock` call landed) is kept, and an error is left alone. The driver
// tags the input replies it delivers through CDP the same way.
function tagged(reply) {
  if (!reply || reply.error != null) return reply;
  const tags = ask({ op: "tags" });
  if (tags.error != null) return reply;
  for (const key of Object.keys(tags)) if (reply[key] === undefined) reply[key] = tags[key];
  return reply;
}
// To `to`, or to `settle`: a fixed point — advance to when the last thing
// in flight ends, and if the timers crossed on the way started more, again
// (bounded; `settled: false` at the bound). A request in flight (LLP 1016)
// is waited for first: its reply commits, and may start motion or ask for
// more, before the fixed point is measured. What is in flight lands before a
// timer fires — the runner keeps one request per target (LLP 1016 D5), so a
// tick's send would drop the reply of the one before it: a jump that crosses
// timers stops after each timer that sends, and its reply is waited for; past
// the deadline, or 4096 stops, the rest is one advance. The clock lands where
// the runner says; a timer's refusal is the error. A promise: the driver
// awaits it.
async function clock(request) {
  const settle = !!request.settle; if (imageHold) await (await imageHold).ready(); // an animated image starts on the clock it lands at
  const deadline = performance.now() + SETTLE_DEADLINE_MS;
  let world = {};
  const reply = (settled, requests) => ({ clock: agentClock, ...(settled === undefined ? {} : { settled }), ...world.reply, ...(settled === false && world.pending ? { reason: "world" } : settled === false && requests ? { reason: "requests" } : {}) });
  for (let rounds = 0; ; rounds++) {
    if (settle && !(await waitForInflight(deadline))) return reply(false, true); const pieceLoad = pieces.pending(); if (pieceLoad) await pieceLoad; if (settle) collections.settle(); // rows a list shows are built at this clock, before it moves (as the JS agent does): their animations start here
    if (gpuInPlay()) { const pending = await settleGpu(); if (pending.length) return gpuPendingReply(request, pending); }
    const to = settle ? Math.max(settleCandidate(), world.settleAt ?? agentClock) : request.to;
    if (!(to >= agentClock)) return { error: `the clock cannot go backwards (${agentClock} → ${to})` };
    let batch;
    for (let steps = 0; ; steps++) {
      const waited = flowDue != null && flowDue <= to && await waitForInflight(deadline), held = waited && steps < 4096;
      ({ batch } = applyBatch(JSON.parse(readOut(wasm.exact_advance(to, held ? 1 : 0)))));
      if (batch.error || !held) break; // a stop at `to` may leave a timer due there
    }
    globalThis.exact.gpu?.schedule?.();
    if (flowLoading) await flowLoading;
    if (textflow) await textflow.settle();
    if (batch.error) return { error: `clock: ${batch.error}`, clock: agentClock };
    if (gpuInPlay()) { const pending = await settleGpu(); if (pending.length) return gpuPendingReply(request, pending); }
    world = globalThis.exact.gpu?.clock?.(settle) ?? {};
    if (!settle) { if (imageHold) await (await imageHold).ready(); const n = waiting().length; return n ? { ...reply(), inflight: n } : reply(); } // what a jump leaves in flight on real time, which the driver names (calendar F10)
    collections.settle(); // every list built and measured where it shows (LLP 1070 G3)
    if (waiting().length) { if (rounds >= 15) return reply(false, true); continue; }
    const next = Math.max(settleCandidate(), world.settleAt ?? agentClock);
    if (next <= agentClock && !world.pending) { if (imageHold) await (await imageHold).ready(); const held = ask({ op: "holds" }); return held.holds?.length ? { ...reply(false), reason: "device", tickets: held.tickets } : reply(true); } // a hold is never waited on (LLP 1069.007 D3)
    if (rounds >= 15) return reply(false);
  }
}

let ticker = null, timerFactory = null;
function startClock() {
  if (timerFactory && !agentMode && !textflow && !flowLoading) {
    ticker ??= timerFactory({ now, advance: time => send(wasm.exact_advance(time, 0)), present }); ticker.update(flowDue, flowFrames);
  }
}
function activateData() {
  const batch = JSON.parse(readOut(wasm.exact_data_ready()));
  if (batch.error) throw new Error(batch.error);
  let inputOpen; const inputOpened = new Promise(resolve => { inputOpen = resolve; });
  page?.release(applyBatch, inputOpened); applyBatch(batch); // early presses replay once input is ready
  const ready = () => {
    setInputReady(true); collections.dataReady(); root.dataset.moduleReady = 'true'; inputOpen();
    if (pageNative) loadPageNative().then(native => native.drain()).catch(error => log(`native: ${error}`));
  };
  const pending = pieces.pending(); // pieces this tree first uses: input waits for their handlers (LLP 1047 D5)
  return pending ? pending.then(ready) : ready();
}
// Boot the app — from the plan baked into the wasm, or from `bytes` (the
// dev loop's restart carrying compatible state, LLP 1007 §6).
let mutation = Promise.resolve(); function mutate(work) { const next = mutation.then(work); mutation = next.catch(() => {}); return next; }
function boot(...args) { return mutate(() => bootNow(...args)); }
async function bootNow(bytes, assets = devAssets, current = () => true, module = null, fresh = false) {
  const t = performance.now(), request = ++bootAttempt;
  // Decode and load private font faces while the live page keeps running.
  // Carry state only at the synchronous host acceptance point below.
  const bakedLength = bytes ? 0 : wasm.exact_plan();
  const plan = bytes ?? new Uint8Array(memory.buffer, wasm.exact_out(), bakedLength).slice();
  let ptr = wasm.exact_in(plan.length);
  new Uint8Array(memory.buffer, ptr, plan.length).set(plan);
  const faces = JSON.parse(readOut(wasm.exact_plan_fonts(plan.length)));
  if (faces.error) throw new Error(faces.error); globalThis.exact.fontAliases = Object.fromEntries(faces.map((f) => [f.declared, f.family])); // canvas text names the declared family (LLP 1056 D8)
  const preparedFonts = await prepareFonts(faces, assets);
  const shaderCommit = assets !== null && globalThis.exact.gpu ? await globalThis.exact.gpu.prepareShaders(assets) : null;
  // A retiring optional instance cannot delay boot; its generation guard disposes it.
  if (!current() || request !== bootAttempt) return null;
  // A replacement must let the current executor finish its answers before
  // the synchronous swap. Initial module readiness does not drain requests.
  if (module) {
    if (!(await waitForInflight(performance.now() + SETTLE_DEADLINE_MS))) {
      throw new Error('module replacement waits for in-flight requests to settle; retry the update');
    }
    if (!current() || request !== bootAttempt) return null;
  }
  const launch = encoder.encode(location.pathname + location.search); // @ref LLP 1038 D5
  const kept = !fresh && (bytes || module) ? (await loadStage('inspection'), focus.keep(ask({ op: "tree" }), Number(document.activeElement?.closest?.("[data-view]")?.dataset.view))) : undefined;
  let len;
  if (module) {
    const id = module.rust ?? new TextEncoder().encode(JSON.stringify(module.realm.id));
    const payload = new Uint8Array(plan.length + module.receipt.length + id.length);
    payload.set(plan); payload.set(module.receipt, plan.length); payload.set(id, plan.length + module.receipt.length);
    ptr = wasm.exact_in(payload.length); new Uint8Array(memory.buffer, ptr, payload.length).set(payload);
    len = wasm.exact_boot_module(plan.length, module.receipt.length, id.length);
  } else if (bytes) {
    ptr = wasm.exact_in(bytes.length + launch.length);
    const payload = new Uint8Array(memory.buffer, ptr, bytes.length + launch.length);
    payload.set(bytes); payload.set(launch, bytes.length);
    len = wasm.exact_boot_plan(bytes.length, innerWidth, innerHeight, launch.length, preferences() | foldBits());
  } else {
    if (page?.checkpoint) wasm.exact_checkpoint(writeIn(agentMode ? page.checkpoint.replace("\n", " driven\n") : page.checkpoint)); ptr = wasm.exact_in(launch.length); // a drive's clock starts at zero (page.rs)
    new Uint8Array(memory.buffer, ptr, launch.length).set(launch);
    len = wasm.exact_boot(innerWidth, innerHeight, launch.length, preferences() | foldBits());
  }
  const batch = JSON.parse(readOut(len));
  if (batch.error) throw new Error(batch.error);
  frameSampler?.reset(); // a new runner numbers its transactions afresh (LLP 1079 D3)
  if (module) { activeModule?.realm?.dispose(); activeModule = module; setInputReady(true); }
  navigation.reset(batch.ops.find(op => op.op === "router"));
  const oldAssets = devAssets;
  devAssets = assets;
  shaderCommit?.();
  // Tear down without yielding; ownership guards refuse retired views.
  incarnation += 1;
  globalThis.exact.generation = incarnation;
  // A queued surface belongs to the plan that named it. The GPU device may
  // finish loading across a reload; no old surface request may join the new
  // plan even when view ids are reused.
  globalThis.exact.pendingSurfaces = [];
  ticker?.dispose(); ticker = null;
  arrange.reset(); motion.reset();
  if (textflow) for (const el of views.values()) retiredViews.add(el);
  textflow?.dispose(); textflow = null; flowLoading = null; flowContexts = []; flowDue = null; flowFrames = false;
  globalThis.exact?.gpu?.reset(Boolean(bytes));
  for (const el of followedScrolls.keys()) followScroll(el, false);
  pendingScrolls.clear();
  collections.reset();
  for (const el of lists.keys()) forgetList(el);
  for (const el of views.values()) if (el instanceof HTMLVideoElement) { globalThis.exact.removeMedia?.(el); el.pause(); el.removeAttribute("src"); el.load(); }
  for (const el of views.values()) { el.exactMarkup?.destroy(); el.exactNative?.destroy(); } views.clear();
  messageFrames.clear(); messageViews.clear();
  if(storageRequests){storageRequests.then(s=>s.dispose()).catch(()=>{});storageRequests=null;}
  grants = []; grantSet = null; unparsed = "";
  for (const controller of controllers) controller.abort();
  controllers.clear(); forgettable.clear();
  inflight.clear();
  if (!page?.holding) root.replaceChildren();
  commitFonts(preparedFonts);
  focus.restart(kept, () => applyBatch(batch), () => ask({ op: "tree" }), id => views.get(id));
  // @ref LLP 1027.000.000 — the date, as the clock the runner already reads.
  if (wasm.exact_set_time) applyBatch(JSON.parse(readOut(wasm.exact_set_time(...reportTime(now())))));
  if (wasm.exact_set_place) applyBatch(JSON.parse(readOut(wasm.exact_set_place(writeIn(reportPlace())))));
  if (wasm.exact_set_page) applyBatch(JSON.parse(readOut(wasm.exact_set_page(pageFacts.bits()))));
  if (wasm.exact_set_root_font_size) applyBatch(JSON.parse(readOut(wasm.exact_set_root_font_size(pageFacts.rootFontSize()))));
  globalThis.exact?.gpu?.finishRestart();
  if (bytes && !module && (inputReady || root.dataset.error)) activateData(); // A restart after the first activation.
  if (oldAssets !== assets) releaseAssets(oldAssets);
  // @ref LLP 1043.000 §3 D7 — an optional initial flow load cannot gate paint/readiness.
  if (bytes && flowLoading) await flowLoading;
  if (bytes && textflow) await textflow.settle();
  startClock();
  if (bytes) requestAnimationFrame(() => requestAnimationFrame(loadGpuIfNeeded));
  return performance.now() - t;
}

// `agent`, `agentSettled` and `now` exist only in agent mode: a normal page has no agent
// surface and no clock but the browser's.
let ready;
globalThis.exact = { ...globalThis.exact, mutate, devFirst: () => devFirst(),
  // @ref LLP 1038 D8/D11 — synchronous for the serialized popstate caller.
  navigate: (location) => {
    const nav = root.firstElementChild;
    if (!inputReady || !nav?.hasAttribute("navigationBack")) return { ops: [], error: "no navigation root" };
    const batch = JSON.parse(readOut(wasm.exact_dispatch(Number(nav.dataset.view), 14, writeIn(location), now())));
    applyBatch(batch); return batch;
  },
  // A dev-plan event can arrive while the wasm is still fetching. Queue it
  // behind the initial boot instead of acknowledging a reload that did not
  // happen.
  reload: async (bytes, fresh = false) => { await ready; await moduleReady; if (logicInfo || activeModule) throw new Error('module reload requires a paired generation'); return boot(bytes, devAssets, () => true, null, fresh); },
  reloadGeneration: async (bytes, cards, current, module = null, rust = null) => {
    await ready;
    await moduleReady;
    if (module && !logicInfo) throw new Error('this web client has binary-bound logic; rebuild with the browser module executor');
    if (!module && !rust && (logicInfo || activeModule)) throw new Error('a module client requires a paired plan/module generation');
    if (rust && globalThis.exact.compat?.inputs?.rustMode !== 'browser') throw new Error('Rust replacement is disabled in this client; rebuild it');
    const assets = assetNamespace(cards);
    let candidate = null;
    try {
      if (module) candidate = { ...module, realm: await moduleLoader.prepare(module, logicInfo) };
      if (rust) {
        await loadRust();
        if (candidate) {
          const js = new TextEncoder().encode(JSON.stringify(candidate.realm.id));
          const payload = new Uint8Array(js.length + rust.module.length);
          payload.set(js); payload.set(rust.module, js.length);
          const decode = bytes => JSON.parse(new TextDecoder('utf-8', {fatal:true}).decode(bytes));
          candidate = {...candidate, receipt:new TextEncoder().encode(JSON.stringify({version:1,kind:'mixed',javascript:decode(module.receipt),rust:decode(rust.receipt),javascriptBytes:js.length})),rust:payload};
        } else candidate = { receipt: rust.receipt, rust: rust.module };
      }
      return await boot(bytes, assets, current, candidate) !== null;
    } finally {
      if (devAssets !== assets) releaseAssets(assets);
      if (candidate && activeModule !== candidate) candidate.realm?.dispose();
    }
  },
  message: (el, text) => { const id = Number(el?.dataset.view); if (inputReady && el && views.get(id) === el && messageViews.has(id)) send(wasm.exact_dispatch(id, 9, writeIn(text), now())); },
  get devAssets() { return devAssets; },
  get ready() { return ready.then(async () => { await moduleReady; if (!inputReady) throw new Error(root.dataset.error || 'data executor not ready'); }); },
  ...(agentMode ? { agent, agentSettled, now, worldCarry: globalThis.exactWorldCarry } : {}), get wasm() { return wasm; }, assetURL: localAssetURL, stages: () => Object.fromEntries(Object.keys(stages).map(name => [name, stageLoaded(name) ? 'loaded' : 'staged'])), writeIn, send, views, root, generation: 0, pendingSurfaces: [], postBound: POST_BOUND,
};
// The GPU module, on demand: a script element after a rendering opportunity
// (two animation-frame callbacks), never an eager import, and only when a
// canvas is on the page. Until one is, an agent reply has nothing to settle.
let gpuLoading = null; const gpuInPlay = () => Boolean(gpuLoading || globalThis.exact.gpu || globalThis.exact.pendingSurfaces?.length);
function loadGpuIfNeeded() {
  if (gpuLoading || !(globalThis.exact.pendingSurfaces ?? []).length) return;
  gpuLoading = loadAfterPaint(globalThis.exact.compat?.inputs?.gpuModules ? './gpu-modules.js' : './gpu-glue.js', 'gpu').catch(error => console.error("exact gpu:", error)); // @ref LLP 1009 D6
}
async function main() {
  if (page) { const options = { root, views, log, early: page.early, dispatch: (id, kind = 0, value = "") => send(wasm.exact_dispatch(id, kind, value ? writeIn(value) : 0, now())) }; page = globalThis.exact.documentPage?.connect(options) ?? (await loadAfterPaint('./document-glue.js', 'documentBoot'))(options); await page.started; }
  // A served document's download began at its first paint (LLP 1048.000 D6).
  // A navigation that leaves the page stops it, or the glue's own, and their
  // preload, so the next document has the link. A page that stays downloads it
  // again: a task after Stop (`navigateerror`, fired mid-stop) or Back from the
  // bfcache (`pageshow`); a 204 or a download says nothing, so after a second.
  // The page names the build in its preload (`./app.wasm?v=…`, LLP 1047.000 §9), so this file is the same across builds.
  const preload = () => [...document.querySelectorAll('link[rel="preload"]')].find(l => new URL(l.href).pathname.endsWith("/app.wasm")), url = new URL(preload()?.href ?? "./app.wasm", import.meta.url), imports = { exact_grants: grantOrigins(() => memory), exact_js: { call: moduleCall }, exact_rust: rustImports, exact_data: dataImports, exact_geometry: { read: (op, view, out) => geometry?.read(op, views.get(view), new Float64Array(memory.buffer, out, 4)) ?? 0 } }, aborted = e => e?.name === "AbortError";
  const download = () => { const stop = new AbortController(); globalThis.navigation?.addEventListener("navigate", e => e.destination.sameDocument || e.downloadRequest != null || (stop.abort(), preload()?.remove()), { signal: stop.signal }); return fetch(url, { signal: stop.signal }); };
  const stayed = () => new Promise(done => { const later = () => setTimeout(done); globalThis.navigation?.addEventListener("navigateerror", later, { once: true }); addEventListener("pageshow", later, { once: true }); setTimeout(done, 1000); });
  let response = (globalThis.exact.runtime ??= download()).then(r => r.url === url.href ? r : download(), e => aborted(e) ? Promise.reject(e) : download()), instance;
  let compiled; for (;;) try { ({ instance, module: compiled } = await WebAssembly.instantiateStreaming(response, imports)); break; } catch (e) { if (!aborted(e)) throw e; await stayed(); response = download(); }
  wasm = instance.exports; const [staged] = WebAssembly.Module.customSections(compiled, 'exact.stages'); if (staged) stages = JSON.parse(new TextDecoder().decode(staged)).stages;
  memory = wasm.memory; if (WebAssembly.Module.imports(compiled).some(i => i.module === "exact_geometry")) requestAnimationFrame(() => loadAfterPaint('./geometry-glue.js', 'geometry').then(create => { geometry = create(root); })); // an artifact whose actions read geometry imports it (LLP 1051.000 D4)
  globalThis.exact.compat = JSON.parse(readOut(wasm.exact_compat()));
  logicInfo = typeof wasm.exact_module_artifact === 'function' && wasm.exact_logic ? { ...JSON.parse(readOut(wasm.exact_logic())), native: pageNative } : null;
  setInputReady(false); // Every data executor activates after the baked first pixel.
  // Restore granted secrets before the baked frame (LLP 1018 D6).
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
  const first = await globalThis.exact.devFirst?.();
  // A data module's realm: beside the boot on a served document, whose first
  // pixel is painted (LLP 1048.000 D6); a client page's after its baked frame.
  // The baked data source admits only the baked module's revision. A dev
  // generation's module with another (a TypeScript edit since the wasm was
  // built) replaces it at activation through the module path, whose receipt
  // names its revision — no wasm rebuild.
  const realm = () => typeof wasm.exact_module_artifact !== 'function' ? null : loadAfterPaint('./module-glue.js','moduleRuntime')
    .then(async loader => { moduleLoader = loader; const payload = first?.module ?? await loader.baked(); const replaces = Boolean(first?.module) && JSON.parse(decoder.decode(payload.receipt)).module?.sha256 !== logicInfo.revision; return { ...payload, replaces, realm: await loader.prepare(payload, logicInfo, replaces ? undefined : 0) }; });
  const prepared = page ? realm() : null; prepared?.catch(() => {}); // awaited, and reported, at activation
  await boot(first?.plan ?? null, first ? assetNamespace(first.assets) : null, () => true, null, true); // @ref LLP 1007 §6
  root.dataset.bootMs = (performance.now() - t0).toFixed(1);
  const activate = async () => {
    loadGpuIfNeeded(); if (wasm.exact_motion) pieces.preload(); if (globalThis.launchQueue) documentsGlue().catch(console.error); // motion links its export (LLP 1047 D3); an installed app's launch files (LLP 1069.010)
    if (!agentMode) loadAfterPaint('./timer-glue.js', 'createTimerScheduler').then(create => { timerFactory = create; startClock(); }).catch(console.error);
    if (!agentMode && AGENT_ADMITTED) loadAfterPaint('./frames.js', 'createFrameSampler').then(create => { frameSampler = globalThis.exact.frames = create({ origin: () => t0, log, target: 'wasm', covers: ['input', 'scroll', 'batches', 'animations', 'canvas'], gather: () => loadStage('inspection').then(() => ({ journal: ask({ op: 'logs', since: 0 }), perf: ask({ op: 'perf' }) })) }); }).catch(console.error); // a development page's frames (LLP 1079 D3)
    // @ref LLP 1043.000 §3 D8 — one optional load, no activation wait or retry queue.
    loadAfterPaint('./input-glue.js', 'createInputHandlers').then(create => {
      inputHandlers = create({ root, views, retiredViews, agentMode, ready: () => inputReady, inertAncestor,
        dispatch: (id, payload) => send(wasm.exact_dispatch(id, 20, writeIn(payload), now())),
        release: (id, payload) => send(wasm.exact_dispatch(id, 28, writeIn(payload), now())), velocity: motion.pan });
    }).catch(console.error);
    try {
      const module = await (prepared ?? realm());
      if (module?.replaces) await boot(first.plan, devAssets, () => true, module); else activeModule = module ?? activeModule;
      await activateData();
    } catch (error) { root.dataset.error = String(error); console.error(error); }
    finally {
      resolveModuleReady(); if (logicInfo) httpHelpers(); // a data module's first response is read without a load
      if (globalThis.exact.compat.inputs.rustModule && globalThis.exact.compat.inputs.rustMode === 'browser') {
        loadRust().then(() => globalThis.exact.followRustUpdates(globalThis.exact, import.meta.url)).catch(error => console.error('Rust update discovery:',error));
      }
    }
  };
  // Nested rAF gives a client page's baked DOM a rendering opportunity before
  // activation; a served document has painted already.
  if (page) activate();
  else requestAnimationFrame(() => { root.dataset.frameCallbackMs = (performance.now() - t0).toFixed(1); requestAnimationFrame(activate); });
}
ready = main();
ready.catch((e) => { console.error(e); root.dataset.error = String(e); });
// @ref LLP 1038 D7/D8/D11 — the mirror observes the handler's synchronous commit.
function navigate(location) { return globalThis.exact.navigate(location); }
navigation.connect(root, navigate, log);
