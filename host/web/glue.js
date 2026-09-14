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
let rustLoader = null, rustLoading = null;
const rustImports = Object.fromEntries(['load', 'call', 'read', 'drop'].map(name => [name, (...args) => {
  if (!rustLoader) throw new Error('Rust module loader is not ready');
  return rustLoader[name](...args);
}]));
function loadAfterPaint(file, exported) {
  return new Promise((resolve,reject)=>{
    const script=document.createElement('script');script.type='module';script.src=new URL(file,import.meta.url).href;
    script.onload=()=>resolve(globalThis.exact[exported]);script.onerror=()=>reject(new Error('host module loader failed: '+file));document.head.append(script);
  });
}
async function loadRust() {
  if (rustLoader) return;
  rustLoading ??= loadAfterPaint('./rust-glue.js','createRustRuntime')
    .then(create=>{rustLoader=create(()=>memory);}).catch(error=>{rustLoading=null;throw error;});
  await rustLoading;
}
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

// DOM scrollTop/scrollLeft writes apply after this batch's new children and styles exist.
// An unchanged binding never overrides a user's scroll position.
const pendingScrolls = new Map();
// An explicit chat/log policy, not CSS overflow anchoring: keep the end
// visible across resizing only while the reader is already there.
const followedScrolls = new Map();
function rememberScroll(s) {
  s.top = s.el.scrollTop; s.height = s.el.scrollHeight; s.port = s.el.clientHeight;
  s.end = s.top >= s.height - s.port - 1;
}
function settleFollow(s) {
  if (!s.el.isConnected) return;
  // A reader above the end uses the browser's CSS scroll anchoring. Writing
  // the remembered numeric offset here would undo its adjustment when content
  // above the visible message changes.
  if (s.end) s.el.scrollTop = s.el.scrollHeight - s.el.clientHeight;
  rememberScroll(s);
  const children = [...s.el.children];
  if (children.length !== s.children.length || children.some((el, i) => el !== s.children[i])) {
    s.observer.disconnect(); s.observer.observe(s.el);
    for (const child of children) s.observer.observe(child);
    s.children = children;
  }
}
function followScroll(el, enabled) {
  const old = followedScrolls.get(el);
  if (old || !enabled) {
    if (old && !enabled) { old.observer.disconnect(); el.removeEventListener("scroll", old.scrolled); followedScrolls.delete(el); }
    return;
  }
  const s = { el, top: 0, height: 0, port: 0, end: true, children: [] };
  s.scrolled = () => {
    // ResizeObserver settles a changed geometry before a queued scroll
    // notification is allowed to change whether the reader follows the end.
    if (el.scrollHeight === s.height && el.clientHeight === s.port) rememberScroll(s);
  };
  s.observer = new ResizeObserver(() => { settleFollow(s); positionContexts(); });
  s.observer.observe(el); el.addEventListener("scroll", s.scrolled, { passive: true });
  followedScrolls.set(el, s);
}
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
  // Focus commands can resize the keyboard viewport in this same batch.
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
  // Native context presentation magnifies the preview without reflowing its
  // text. Keep authored individual transforms; this is the panel projection.
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
    // iPhone 17 / iOS 26.5: 15%, with growth capped on the larger dimension.
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
    // The panel moves up by half the added height. Offset later content once
    // at each enclosing level so receipts keep their source-relative position.
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
    // Native Messages keeps trailing controls inside the available region,
    // overlapping an oversized preview when the whole panel cannot fit.
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
addEventListener("resize", () => requestAnimationFrame(positionContexts));
visualViewport?.addEventListener("resize", () => requestAnimationFrame(positionContexts));
// LLP 1035.004: portable roles are images; their artwork and tint are host-owned.
const symbolStyle = document.createElement("style");
symbolStyle.textContent = 'img[data-symbol-path]{background-color:var(--exact-symbol-tint,#000)!important;mask-image:var(--exact-symbol-mask);mask-repeat:no-repeat;mask-position:center;mask-size:var(--exact-symbol-fit,100% 100%);mask-origin:content-box;mask-clip:content-box}';
document.head.append(symbolStyle);
function refreshSymbols() {
  for (const el of views.values()) {
    if (!(el instanceof HTMLImageElement) || !el.hasAttribute("data-symbol-path")) continue;
    const cs = getComputedStyle(el), size = parseFloat(cs.fontSize), weight = Number(cs.fontWeight);
    const path = el.getAttribute("data-symbol-path"), key = `${path}:${size}:${weight}`;
    if (!path && el.symbolRefusal !== el.symbolSource) {
      log(`image ${el.symbolSource} refused: unknown symbol role`); el.symbolRefusal = el.symbolSource;
    }
    if (el.symbolKey !== key) {
      el.symbolKey = key;
      const point = path ? size : 0, stroke = 1.1 + (Math.max(100, Math.min(900, weight)) - 100) / 400;
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}" viewBox="0 0 24 24"><path d="${path}" fill="none" stroke="black" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
      el.symbolMask = `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
      el.symbolPlaceholder = `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}"/>`)}`;
    }
    // A transparent source supplies intrinsic dimensions; the mask supplies ink.
    if (el.getAttribute("src") !== el.symbolPlaceholder) el.src = el.symbolPlaceholder;
    el.style.setProperty("--exact-symbol-mask", el.symbolMask);
    el.style.setProperty("--exact-symbol-tint", el.style.getPropertyValue("--exact-tint") || "#000");
    const paddingX = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight), paddingY = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const fits = size <= el.clientWidth - paddingX && size <= el.clientHeight - paddingY;
    const fit = cs.objectFit === "none" || (cs.objectFit === "scale-down" && fits) ? `${size}px ${size}px` : cs.objectFit === "scale-down" ? "contain" : cs.objectFit === "fill" ? "100% 100%" : cs.objectFit;
    el.style.setProperty("--exact-symbol-fit", fit);
  }
}

// A modal dialog escapes inert attributes above it; its own inert still applies.
function inertAncestor(el) {
  for (let node = el; node; node = node.parentElement) {
    if (node.hasAttribute("inert")) return node;
    if (node.localName === "dialog" && node.matches(":modal")) return null;
  }
  return null;
}

function applyProps(el, set, clear) {
  let sandboxChanged = false;
  for (const name of clear || []) {
    if (el instanceof HTMLIFrameElement && name === "src") iframeLoading.set(el, true);
    if (el instanceof HTMLIFrameElement && name === "sandbox" && el.hasAttribute("sandbox")) sandboxChanged = true;
    if (name === "scrollFollowEnd") followScroll(el, false);
    else if (name === "scrollTop" || name === "scrollLeft") {
      const pending = pendingScrolls.get(el); if (pending) delete pending[name];
    }
    else if (name === "text") el.textContent = "";
    else if (name === "value") el.value = "";
    else if (name === "checked") el.checked = false;
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
    } else if (name === "text") {
      if (el.childElementCount === 0) el.textContent = value;
    } else if (name === "value") {
      if (el.value !== value) el.value = value;
    } else if (name === "checked") {
      el.checked = value === "true";
    } else if (name === "inert") {
      el.authoredInert = value === "true"; el.inert = el.authoredInert;
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
    } else if (kind === "scroll") {
      on("scroll", () => { const n = writeIn(`${el.scrollLeft},${el.scrollTop}`); send(wasm.exact_dispatch(id, 13, n, now())); });
    } else if (kind === "swiperight") {
      let drag = null, suppressClick = false;
      on("pointerdown", (e) => {
        if (!e.isPrimary || e.button !== 0 || el.matches(":disabled") || inertAncestor(el)) return;
        if (e.target.closest("input,textarea,[contenteditable]")) return;
        // The CSS touch-action decides which touch directions the browser
        // keeps for scrolling. A scrolling pointer cancels this observation.
        e.preventDefault(); e.stopPropagation();
        const translate = getComputedStyle(el).translate;
        const parts = translate === "none" ? ["0px", "0px"] : translate.match(/calc\([^)]*\)|\S+/g);
        drag = { pointer: e.pointerId, x: e.clientX, y: e.clientY, distance: 0, active: false,
          translate: el.style.translate, transition: el.style.transition, base: parts,
          indicators: [...el.children].filter(n => n.getAttribute("swipeIndicator") === "true").map(node => {
            // Start from authored targets even if a preceding return is in flight.
            const scale = (node.style.scale || "1").split(" ").map(Number);
            return { node, opacity: node.style.opacity, scale: node.style.scale, transition: node.style.transition,
              baseOpacity: Number(node.style.opacity || "1"), baseScale: [scale[0], scale[1] ?? scale[0]] };
          }) };
        el.setPointerCapture(e.pointerId);
      });
      on("pointermove", (e) => {
        if (!drag || drag.pointer !== e.pointerId) return;
        const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
        if (!drag.active) {
          if (Math.abs(dy) > 8 && Math.abs(dy) >= Math.abs(dx) || dx < -8) { drag = null; return; }
          if (dx < 8 || dx <= Math.abs(dy)) return;
          drag.active = true;
          el.style.transition = "none";
          for (const { node } of drag.indicators) node.style.transition = "none";
        }
        drag.distance = Math.max(0, dx);
        const offset = Math.min(drag.distance, 64) + Math.max(0, drag.distance - 64) * 0.2;
        el.style.translate = `calc(${drag.base[0]} + ${offset}px) ${drag.base[1] || "0px"}`;
        const progress = Math.min(offset / 64, 1);
        for (const indicator of drag.indicators) {
          indicator.node.style.opacity = indicator.baseOpacity + (1 - indicator.baseOpacity) * progress;
          indicator.node.style.scale = indicator.baseScale.map(v => v + (1 - v) * progress).join(" ");
        }
        e.stopPropagation();
      });
      const finish = (e) => {
        if (!drag || drag.pointer !== e.pointerId) return;
        const ended = drag; drag = null;
        if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
        if (!ended.active) return;
        suppressClick = true;
        // Commit the held value before restoring the authored CSS transition.
        el.getBoundingClientRect();
        el.style.transition = ended.transition;
        el.style.translate = ended.translate;
        for (const { node, opacity, scale, transition } of ended.indicators) {
          node.style.transition = transition; node.style.opacity = opacity; node.style.scale = scale;
        }
        if (e.type === "pointerup" && ended.distance >= 64) send(wasm.exact_dispatch(id, 12, 0, now()));
      };
      on("pointerup", finish); on("pointercancel", finish); on("lostpointercapture", finish);
      on("click", (e) => { if (suppressClick) { suppressClick = false; e.preventDefault(); e.stopPropagation(); } });
    } else if (kind === "contextmenu" || kind === "dblclick") {
      on(kind, (e) => {
        if (el.matches(":disabled") || inertAncestor(el)) return;
        if (e.target.closest("input,textarea,[contenteditable]")) return;
        e.preventDefault(); e.stopPropagation();
        send(wasm.exact_dispatch(id, kind === "contextmenu" ? 10 : 11, 0, now()));
      });
    } else if (kind === "change") {
      on("input", (e) => {
        const value = el.value;
        if (el.getAttribute("emojiPicker") === "true") {
          if (e.isComposing) return;
          el.value = "";
          const clusters = [...new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(value)];
          if (clusters.length !== 1 || !(/\p{Emoji_Presentation}/u.test(value)
            || (/[\uFE0F\u20E3]/u.test(value) && /\p{Emoji}/u.test(value)))) return;
        }
        const n = writeIn(value); send(wasm.exact_dispatch(id, 1, n, now()));
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
    const modal = document.activeElement.closest("dialog:modal");
    if (modal && !modal.contains(el)) continue;
    if (!el.isConnected || !el.getClientRects().length || inertAncestor(el) || getComputedStyle(el).visibility !== "visible") continue;
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
  prepareContexts(batch);
  for (const s of followedScrolls.values()) s.scrolled();
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
      case "storage": {
        const requestIncarnation=incarnation;
        const p=Promise.resolve().then(async()=>{
          if(agentMode)throw new Error('storage is unavailable in agent mode');
          await moduleReady; if(!inputReady)throw new Error('data executor is unavailable');
          if(requestIncarnation!==incarnation)throw new Error('storage source unloaded');
          if(!storageRequests){
            const app=globalThis.exact.compat.inputs.app, scope=grants.join('\n');
            const pending=loadAfterPaint('./storage-request.js','createStorageRequests').then(create=>create(app,scope)).catch(error=>{if(storageRequests===pending)storageRequests=null;throw error;});
            storageRequests=pending;
          }
          const service=await storageRequests;
          if(requestIncarnation!==incarnation)throw new Error('storage source unloaded');
          return service.run(op.payload,op.scope);
        }).then(bytes=>safelyFulfill(requestIncarnation,op.ticket,5,0,"",bytes))
          .catch(error=>safelyFulfill(requestIncarnation,op.ticket,3,0,"",enc.encode(String(error))));
        inflight.add(p);p.finally(()=>inflight.delete(p));break;
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
        // Host and source scopes both admit the request (LLP 1027.001 D2).
        const { ticket, method, url, headers, body, cache } = op;
        const requestIncarnation = incarnation;
        const scopeValid = op.scope == null || typeof op.scope === 'string' && op.scope.split('\n').map(s=>s.trim()).filter(Boolean).every(s=>grants.map(g=>g.trim()).includes(s));
        if (!scopeValid || !granted(url) || !granted(url,op.scope)) {
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
        const init = { method, headers, redirect: op.scope == null ? "follow" : "error", cache: cache === "reload" ? "reload" : "default", signal: controller.signal };
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
        else if (op.name === "focus" || op.name === "selectText") focusCommands.push({ args: op.args, selectText: op.name === "selectText" });
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
        else console.warn(`exact: unknown command ${op.name}`);
        break;
      }
      case "destroy": { const el = views.get(op.id); if (el) { followScroll(el, false); messageFrames.delete(el); el.remove(); } views.delete(op.id); globalThis.exact.gpu?.destroy(op.id); break; }
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
  // Navigation keeps its routes mounted; only the selected route receives
  // input or participates in accessibility. UIKit projects these into its
  // own controller stack; the browser retains the same declarative state.
  for (const nav of root.querySelectorAll("[navigationBack]")) {
    const routes = [...nav.children].filter(route => route.hasAttribute("navigationKey"));
    const selected = routes.findIndex(route => route.getAttribute("navigationKey") === nav.getAttribute("navigationKey"));
    const modal = routes[selected]?.getAttribute("navigationPresentation") === "modal";
    for (const [index, route] of routes.entries()) {
      const active = index === selected;
      if (!active && route.contains(document.activeElement)) document.activeElement.blur();
      route.style.visibility = active || (modal && index === selected - 1) ? "" : "hidden";
      route.inert = !active || !!route.authoredInert;
    }
  }
  refreshSymbols();
  for (const s of followedScrolls.values()) settleFollow(s);
  for (const [el, offsets] of pendingScrolls) if (el.isConnected) {
    // Mirroring the current offset must not restart snapping or cancel a pan.
    for (const [name, offset] of Object.entries(offsets)) if (el[name] !== offset) el[name] = offset;
    const s = followedScrolls.get(el); if (s) rememberScroll(s);
  }
  pendingScrolls.clear();
  // Focusing can dispatch an action; every node/value in this batch must be
  // committed before its focus handler runs.
  for (const { args, selectText } of focusCommands) {
    if (args?.length !== 1 || typeof args[0] !== "string" || !inputReady) continue;
    const el = [...root.querySelectorAll("[id]")].find(node => node.id === args[0]);
    // A focus that cannot be delivered is a journal line with its reason,
    // never silence (LLP 1035.001 D6); the reasons are the iOS host's.
    const reason = !el ? "no live node with that id" : !el.isConnected ? "not mounted" : el.matches(":disabled") ? "disabled"
      : inertAncestor(el) ? "inert ancestor" : !el.getClientRects().length ? "zero size"
      : getComputedStyle(el).visibility !== "visible" ? "hidden ancestor" : null;
    if (reason) { log(`focus "${args[0]}" refused: ${reason}`); continue; }
    if (selectText && typeof el.select !== "function") { log(`selectText "${args[0]}" refused: not a text editor`); continue; }
    el.focus();
    if (selectText && document.activeElement === el) el.select();
  }
  positionContexts();
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
let storageRequests = null;
const inflight = new Set();
const controllers = new Set();
let incarnation = 0;
const enc = new TextEncoder();
function granted(url, scope = null) {
  // A `net.fetch` grant is an origin — scheme, host, port — matched whole,
  // as ibex2 matches it on the native hosts (LLP 0067): the same refusal
  // everywhere. A URL that does not parse is outside every grant.
  let origin;
  try { origin = new URL(url).origin; } catch { return false; }
  return (scope == null ? grants : scope.split("\n")).map(g=>g.trim()).some((g) => {
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
function nodeDetail(id) {
  const el = views.get(id);
  if (!el || !el.isConnected) return { error: `stale node #${id}` };
  const node = ask({ op: "node", id });
  if (node.error) return node;
  // The kernel's layout never runs on the web (LLP 1007 §9): its frames are
  // not observations here, so they are absent rather than zeros.
  delete node.frame;
  delete node.absolute;
  delete node.content;
  const r2 = (x) => Math.round(x * 100) / 100;
  const rect = (r) => ({ x: r2(r.x), y: r2(r.y), w: r2(r.width), h: r2(r.height) });
  const idOf = (e) => { for (const [i, v] of views) if (v === e) return i; return null; };
  const r = el.getBoundingClientRect();
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
  node.native = { element: el.localName };
  const cs = getComputedStyle(el);
  node.browser = Object.fromEntries(Object.entries(INHERITED_CSS).map(([row, prop]) => [row, cs.getPropertyValue(prop)]));
  node.observed = { clock: now(), wall: Date.now() };
  return node;
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
      case "state": {
        // The runner's state, then what the page observes (LLP 1035.002
        // D2): the focused element, the software keyboard as the visual
        // viewport reports it, and the routes as the DOM declares them.
        const st = ask(request);
        if (st.error) return st;
        const r2 = (x) => Math.round(x * 100) / 100;
        const idOf = (e) => { for (const [i, v] of views) if (v === e) return i; return null; };
        const active = document.activeElement && document.activeElement !== document.body ? document.activeElement : null;
        const editor = active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement ? idOf(active) : null;
        st.focus = { logical: active ? idOf(active) : null, editor, responder: active ? active.localName : null, pending: null };
        const overlap = Math.max(0, innerHeight - (globalThis.visualViewport?.height ?? innerHeight));
        const policy = document.querySelector("[interactiveWidget]")?.getAttribute("interactiveWidget") ?? "resizes-visual";
        st.keyboard = { visible: overlap > 0, overlap: r2(overlap), policy, interactive: false };
        const nav = document.querySelector("[navigationBack]");
        const routes = nav ? [...nav.children].filter((r) => r.hasAttribute("navigationKey")) : [];
        const key = nav?.getAttribute("navigationKey") ?? null;
        const index = routes.findIndex((r) => r.getAttribute("navigationKey") === key);
        const selected = index >= 0 ? routes[index] : null;
        st.navigation = {
          route: key,
          stack: index >= 0 ? routes.slice(0, index + 1).map((r) => r.getAttribute("navigationKey")) : [],
          presentation: ["modal", "fullscreen"].includes(selected?.getAttribute("navigationPresentation")) ? selected.getAttribute("navigationPresentation") : null,
          source: selected?.getAttribute("navigationSource") ?? null,
          closedby: selected?.getAttribute("closedby") ?? null,
          transition: { interactive: false, phase: "idle" },
        };
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
          const r = el.getBoundingClientRect();
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
          const detail = nodeDetail(request.id);
          if (detail.error) return detail;
          reply.node = detail;
        }
        return tagged(reply);
      }
      case "focus": {
        const el = views.get(request.id);
        if (!el) return { error: `no view ${request.id}` };
        if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return { error: `view ${request.id} is not an input` };
        el.focus();
        if (request.select !== false) el.select();
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
        return tagged(clock(request));
      case "tree":
        return tree();
      case "tags":
        return ask(request);
      default:
        return ask(request);
    }
  } catch (e) {
    return { error: String(e) };
  }
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
function activateData() {
  const batch = JSON.parse(readOut(wasm.exact_data_ready()));
  if (batch.error) throw new Error(batch.error);
  applyBatch(batch); setInputReady(true); root.dataset.moduleReady = 'true';
}

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
    const id = module.rust ?? new TextEncoder().encode(JSON.stringify(module.realm.id));
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
  if (module) { activeModule?.realm?.dispose(); activeModule = module; setInputReady(true); }
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
  for (const el of followedScrolls.keys()) followScroll(el, false);
  pendingScrolls.clear();
  views.clear();
  messageFrames.clear();
  if(storageRequests){storageRequests.then(s=>s.dispose()).catch(()=>{});storageRequests=null;}
  grants = [];
  for (const controller of controllers) controller.abort();
  controllers.clear();
  inflight.clear();
  root.replaceChildren();
  commitFonts(preparedFonts);
  const timers = applyBatch(batch).timers;
  if (bytes && !module) activateData(); // This session has already painted once.
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
  reload: async (bytes) => { await ready; await moduleReady; if (logicInfo || activeModule) throw new Error('module reload requires a paired generation'); return boot(bytes); },
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
  get devAssets() { return devAssets; },
  get ready() { return ready.then(async () => { await moduleReady; if (!inputReady) throw new Error(root.dataset.error || 'data executor not ready'); }); },
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
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), { exact_js: { call: moduleCall }, exact_rust: rustImports });
  wasm = instance.exports;
  memory = wasm.memory;
  globalThis.exact.compat = JSON.parse(readOut(wasm.exact_compat()));
  logicInfo = typeof wasm.exact_module_artifact === 'function' && wasm.exact_logic ? JSON.parse(readOut(wasm.exact_logic())) : null;
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
  await boot(null);
  // Nested rAF gives the baked DOM a rendering opportunity before activation.
  root.dataset.bootMs = (performance.now() - t0).toFixed(1);
  requestAnimationFrame(() => {
    root.dataset.frameCallbackMs = (performance.now() - t0).toFixed(1);
    requestAnimationFrame(async () => {
      loadGpuIfNeeded();
      try {
        if (typeof wasm.exact_module_artifact === 'function') {
          moduleLoader = await loadAfterPaint('./module-glue.js','moduleRuntime');
          const payload = await moduleLoader.baked();
          const realm = await moduleLoader.prepare(payload, logicInfo, 0);
          activeModule = { ...payload, realm };
        }
        activateData();
      } catch (error) { root.dataset.error = String(error); console.error(error); }
      finally {
        resolveModuleReady();
        if (globalThis.exact.compat.inputs.rustModule && globalThis.exact.compat.inputs.rustMode === 'browser') {
          loadRust().then(() => globalThis.exact.followRustUpdates(globalThis.exact, import.meta.url)).catch(error => console.error('Rust update discovery:',error));
        }
      }
    });
  });
}

ready = main();
ready.catch((e) => { console.error(e); root.dataset.error = String(e); });

document.addEventListener("keydown", event => {
  if (event.key !== "Escape" || event.defaultPrevented) return;
  // Making a covered editor inert can leave focus on the page body. That
  // neutral focus still belongs to the presentation; a host input outside
  // Exact keeps its own keys.
  if (!root.contains(event.target) && event.target !== document.body && event.target !== document.documentElement) return;
  // The browser's top layer gets the close request before an authored modal.
  // Preventing Escape here would also prevent the popover's default dismissal.
  if (document.querySelector("dialog:modal") || [...document.querySelectorAll(":popover-open")].some(pop => pop.popover === "auto" || pop.popover === "hint")) return;
  for (const nav of root.querySelectorAll("[navigationBack]")) {
    const route = [...nav.children].find(child => child.getAttribute("navigationKey") === nav.getAttribute("navigationKey"));
    if (!["modal", "fullscreen"].includes(route?.getAttribute("navigationPresentation"))) continue;
    event.preventDefault();
    if (route.getAttribute("closedby") === "none") return;
    const control = [...route.querySelectorAll("[id]")].find(node => node.id === nav.getAttribute("navigationBack"));
    if (control && !control.disabled) control.click();
    return;
  }
});
