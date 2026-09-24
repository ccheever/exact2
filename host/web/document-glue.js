// A page the build rendered (LLP 1048.000 D6, 1a), and the head's fields at
// runtime (LLP 1048.003 D1). Loaded after paint, only by a page that carries
// a checkpoint or a runtime that sends a `head` op.
//
// The document remains visible while its runtime loads and adopts it. Idle
// pages download their wasm from first paint (the capture script) and start
// after load or on input; interaction pages start on a handler's intent or
// when a device has kept sign-in state. Real links work immediately.
// Discrete edits and actions replay once, in order, on matching controls. An
// active IME composition keeps the document in place through compositionend.

const IDLE_FALLBACK_MS = 200;

// The runtime's first loads after it boots, fetched beside its wasm: each
// would otherwise wait for the one before it, a round trip apiece (0.45 s of
// RealWorld's first press on a slow phone). A checkpoint whose logic answered
// names a data module: its realm's glue and prelude, and the reader of its
// fetches' bodies. On an idle page they start when this evaluates, the wasm
// being in flight from the first paint; on an interaction page, at intent.
function preloadRuntime(checkpoint) {
  const logic = /"logic":(null|"(?:[^"\\]|\\.)*")/.exec(checkpoint?.textContent ?? "")?.[1];
  const files = ["./input-glue.js"];
  if (!new URL(location.href).searchParams.has("agent")) files.push("./timer-glue.js");
  if (logic && logic !== "null") files.push("./module-glue.js", "./http-body.js", "./module-prelude.js");
  for (const file of files) {
    const link = document.createElement("link");
    link.href = new URL(file, import.meta.url).href;
    // module-glue reads the prelude with `fetch`; the rest are imported.
    if (file === "./module-prelude.js") { link.rel = "preload"; link.as = "fetch"; link.crossOrigin = ""; }
    else link.rel = "modulepreload";
    document.head.append(link);
  }
}

function documentBoot(options) {
  const root = options.root;
  let { views, dispatch, log = console.warn } = options;
  let holding = true, adopted, adopting = null, releaseAfterComposition = null, restoringFocus = false;
  const held = [], events = [], composing = new Set();
  const script = document.querySelector('script[type="application/vnd.exact.checkpoint"]');
  const interaction = script?.dataset.activate === "interaction";
  let start;
  const started = new Promise(resolve => { start = resolve; });
  const hears = (el, kind) => (el?.dataset.exactOn ?? "").split(" ").includes(kind);
  const identity = el => ({ id: Number(el.dataset.view), tag: el.localName,
    name: el.getAttribute("name"), test: el.getAttribute("data-testid"), type: el.getAttribute("type"),
    text: /^(input|textarea)$/.test(el.localName) ? null : el.textContent });
  const chain = target => {
    const out = [];
    for (let el = target.closest("[data-view]"); el && root.contains(el); el = el.parentElement?.closest("[data-view]")) out.push(el);
    return out;
  };
  const matches = (el, at) => el && el.localName === at.tag && el.getAttribute("name") === at.name
    && el.getAttribute("data-testid") === at.test && el.getAttribute("type") === at.type
    && (at.text === null || el.textContent === at.text);
  const usable = target => target instanceof Element && root.contains(target)
    && !target.closest("[inert],:disabled,[disabled='true']");
  const enqueue = (el, kind, value = "") => {
    const event = { at: identity(el), kind, value };
    const previous = events.at(-1);
    // Consecutive edits carry their latest value; action ordering is preserved.
    if (kind === "change" && previous?.kind === kind && previous.at.id === event.at.id) events.pop();
    events.push(event); start();
  };
  const record = event => {
    if (!holding || !usable(event.target)) return;
    const els = chain(event.target);
    if (event.type === "click") {
      // Real links navigate before activation, including modified clicks.
      if (event.button > 0 || event.target.closest("a[href]")) return;
      const el = els.find(el => hears(el, "press"));
      if (el) enqueue(el, "press");
    } else if (event.type === "input") {
      const el = els.find(el => hears(el, "change"));
      if (el) enqueue(el, "change", el.value);
    } else if (event.type === "focusin" || event.type === "focusout") {
      const kind = event.type === "focusin" ? "focus" : "blur";
      if (hears(event.target, kind)) enqueue(event.target, kind);
    } else if (event.type === "keydown") {
      for (const el of els) {
        if (hears(el, "key")) enqueue(el, "key", event.key);
        if (hears(el, "submit") && el.localName !== "textarea" && event.key === "Enter" && !event.isComposing) {
          event.preventDefault(); enqueue(el, "submit");
        }
      }
    } else if (event.type === "compositionstart") {
      composing.add(event.target); start();
    } else if (event.type === "compositionend") {
      composing.delete(event.target);
      // The final input event follows compositionend in the same task.
      setTimeout(() => { if (!composing.size && releaseAfterComposition) page.release(releaseAfterComposition); }, 0);
    }
  };
  const intent = event => {
    if (!holding || !usable(event.target) || event.target.closest("a[href]")) return;
    if (chain(event.target).some(el => ["press", "change", "focus", "blur", "key", "submit"].some(kind => hears(el, kind)))) start();
  };
  const kinds = ["click", "input", "focusin", "focusout", "keydown", "compositionstart", "compositionend"];
  for (const kind of kinds) root.addEventListener(kind, record, true);
  for (const kind of ["pointerdown", "keydown", "focusin"]) root.addEventListener(kind, intent, true);
  for (const event of options.early?.splice(0) ?? []) record(event);
  if (!interaction) {
    // Beside the wasm: now, when its download began at the first paint;
    // otherwise after the glue's own fetch, which it makes as this resolves.
    if (globalThis.exact.runtime) preloadRuntime(script); else started.then(() => setTimeout(() => preloadRuntime(script), 0));
    const go = () => { removeEventListener("pointerdown", go, true); removeEventListener("keydown", go, true); start(); };
    addEventListener("pointerdown", go, true); addEventListener("keydown", go, true);
    const idle = () => (globalThis.requestIdleCallback ?? (f => setTimeout(f, IDLE_FALLBACK_MS)))(go);
    if (document.readyState === "complete") idle(); else addEventListener("load", idle, { once: true });
  } else {
    // The server is anonymous. Kept device state must still restore without a click.
    try { for (let i = 0; i < localStorage.length; i++) if (localStorage.key(i)?.startsWith("exact.secret.")) { start(); break; } } catch {}
  }
  const replay = event => {
    const at = event.at;
    const el = at && views.get(at.id);
    if (matches(el, at) && el.exactHandlers?.includes(event.kind) && usable(el)) {
      dispatch(at.id, { press: 0, change: 1, focus: 4, blur: 5, key: 6, submit: 7 }[event.kind], event.value ?? "");
    } else log(`document: an early ${event.kind} was dropped (view ${at.id} is not what received it)`);
  };
  const page = {
    started,
    connect(next) { ({ views, dispatch, log } = next); return page; },
    checkpoint: `${script?.dataset.digest ?? ""}\n${script?.textContent ?? ""}`,
    get holding() { return holding; },
    get adopted() { return adopted; },
    get adopting() { return adopting; },
    get restoringFocus() { return restoringFocus; },
    hold(batch) { if (holding) held.push(batch); return holding; },
    release(apply) {
      if (!holding) return;
      if (composing.size) { releaseAfterComposition = apply; return; }
      releaseAfterComposition = null;
      holding = false;
      for (const kind of kinds) root.removeEventListener(kind, record, true);
      for (const kind of ["pointerdown", "keydown", "focusin"]) root.removeEventListener(kind, intent, true);
      // Preserve edits and selection even when a digest mismatch replaces the DOM.
      const edits = [...root.querySelectorAll("input[data-view],textarea[data-view]")].map(el => ({
        at: identity(el), value: el.value, start: el.selectionStart, end: el.selectionEnd,
        direction: el.selectionDirection, focused: el === document.activeElement,
      }));
      const verdict = held[0]?.ops?.find(op => op.op === "adopt");
      adopted = verdict?.adopted === true;
      if (adopted) {
        adopting = new Map();
        for (const el of root.querySelectorAll("[data-view]")) adopting.set(Number(el.dataset.view), el);
      } else {
        if (verdict) log(`document: not adopted: ${difference(root, held[0])}`);
        root.replaceChildren();
      }
      for (const batch of held.splice(0)) apply(batch);
      adopting = null;
      // After the caller's readiness, which it finishes synchronously; a task
      // would wait behind the next frame.
      queueMicrotask(() => {
        for (const edit of edits) {
          const el = views.get(edit.at.id);
          if (matches(el, edit.at) && events.some(event => event.kind === "change" && event.at.id === edit.at.id)) el.value = edit.value;
        }
        for (const event of events.splice(0)) replay(event);
        for (const edit of edits) {
          const el = views.get(edit.at.id);
          if (!matches(el, edit.at)) continue;
          if (edit.focused) {
            restoringFocus = true;
            try { el.focus({ preventScroll: true }); } finally { restoringFocus = false; }
          }
          // A handler can intentionally replace the value; never undo that result.
          if (el.value === edit.value && edit.start !== null) el.setSelectionRange(edit.start, edit.end, edit.direction);
        }
      });
    },
  };
  return page;
}

// Where the document and the runtime's first tree first differ, for the
// journal: the views in document order against the first batch's creates.
function difference(root, batch) {
  const els = [...root.querySelectorAll("[data-view]")];
  const creates = (batch.ops ?? []).filter((op) => op.op === "create");
  for (let i = 0; i < Math.max(els.length, creates.length); i++) {
    const el = els[i], op = creates[i];
    const tag = op && (op.tag === "canvas" ? "div" : op.tag);
    if (!el || !op || Number(el.dataset.view) !== op.id || el.localName !== tag) {
      return `node ${i}: the document has ${el ? `${el.localName} ${el.dataset.view}` : "nothing"}, the runtime ${op ? `${tag} ${op.id}` : "nothing"}`;
    }
  }
  return `the same ${els.length} views; a property or text differs`;
}

// The head's fields in the page's <head>, as the renderer first wrote them:
// what a share sheet, a bookmark or a tab reads while the runtime runs.
let appTitle = null;
function meta(key, name, content) {
  let el = document.head.querySelector(`meta[${key}="${name}"]`);
  if (content == null) { el?.remove(); return; }
  if (!el) { el = document.createElement("meta"); el.setAttribute(key, name); document.head.append(el); }
  if (el.content !== content) el.content = content;
}
function documentHead(op) {
  appTitle ??= document.querySelector('meta[property="og:site_name"]')?.content ?? document.title;
  const title = op.title ?? appTitle;
  if (document.title !== title) document.title = title;
  meta("name", "description", op.description);
  meta("name", "robots", op.robots);
  let canonical = document.head.querySelector('link[rel="canonical"]');
  if (op.canonical == null) canonical?.remove();
  else {
    if (!canonical) { canonical = document.createElement("link"); canonical.rel = "canonical"; document.head.append(canonical); }
    if (canonical.getAttribute("href") !== op.canonical) canonical.setAttribute("href", op.canonical);
  }
  for (const [key, prefix] of [["property", "og:"], ["name", "twitter:"]]) {
    meta(key, `${prefix}title`, title);
    meta(key, `${prefix}description`, op.description);
    meta(key, `${prefix}image`, op.image);
  }
}

globalThis.exact ??= {};
globalThis.exact.documentBoot = documentBoot;
globalThis.exact.documentHead = documentHead;

// On interaction pages this is the only initial script. It records intent before
// loading the ordinary host; no app code or runtime is fetched just for reading.
const checkpoint = document.querySelector('script[type="application/vnd.exact.checkpoint"]');
if (checkpoint?.dataset.activate === "interaction" && !globalThis.exact.documentPage) {
  const root = document.getElementById("exact-root");
  // The presses the page's capture script took before this ran come first.
  const page = globalThis.exact.documentPage = documentBoot({ root, early: globalThis.exact.taps?.() });
  page.started.then(() => requestAnimationFrame(() => requestAnimationFrame(() => {
    // The same parallel downloads as a client page, started only by intent.
    for (const [file, rel] of [["./app.wasm", "preload"], ["./navigation.js", "modulepreload"]]) {
      const link = document.createElement("link"); link.rel = rel; link.href = new URL(file, import.meta.url).href;
      if (rel === "preload") { link.as = "fetch"; link.crossOrigin = ""; }
      document.head.append(link);
    }
    preloadRuntime(checkpoint);
    const script = document.createElement("script");
    script.type = "module"; script.src = new URL("./glue.js", import.meta.url).href;
    script.onerror = () => { root.dataset.error = "The runtime could not load. Reload to retry; page links still work."; };
    document.head.append(script);
  })));
}
