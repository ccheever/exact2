// A page the build rendered (LLP 1048.000 D6, 1a), and the head's fields at
// runtime (LLP 1048.003 D1). Loaded after paint, only by a page that carries
// a checkpoint or a runtime that sends a `head` op.
//
// The document is the page until the runtime has its own settled tree: the
// runtime starts when the page is idle (requestIdleCallback after `load`, or
// a short timeout) or at the first pointer or key interaction, whichever
// comes first; its batches are held while the document stays on screen; when
// its data is ready it replaces the document once, and the reader never sees
// less than the server sent. A press on the document before then is recorded
// against its view ids and replayed once, after, on the element that would
// have taken it — only if that element is still the one the reader pressed.
// Links are real links the whole time; nothing is disabled while it loads.

const IDLE_FALLBACK_MS = 200;

function documentBoot({ root, views, dispatch, log, early = [] }) {
  let holding = true;
  const held = [], presses = [];
  const record = (event) => {
    if (!holding || event.button > 0 || !(event.target instanceof Element)) return;
    // A link keeps its own default: it navigates as a document's link does.
    if (event.target.closest("a[href]")) return;
    const chain = [];
    for (let el = event.target.closest("[data-view]"); el && root.contains(el); el = el.parentElement?.closest("[data-view]")) {
      chain.push({ id: Number(el.dataset.view), tag: el.localName, text: el.textContent });
    }
    if (chain.length) presses.push(chain);
  };
  root.addEventListener("click", record, true);
  // Clicks the glue kept while this module loaded count as if heard here.
  for (const event of early.splice(0)) record(event);
  const replay = (chain) => {
    // The innermost element that takes a press, as a click's bubbling finds it.
    const at = chain.find(({ id }) => views.get(id)?.exactHandlers?.includes("press"));
    const el = at && views.get(at.id);
    if (el && el.localName === at.tag && el.textContent === at.text) dispatch(at.id);
    else log(`document: a press before the runtime started was dropped (view ${chain[0].id} is not what was pressed)`);
  };
  const started = new Promise((resolve) => {
    const kinds = ["pointerdown", "keydown"];
    const go = () => { for (const kind of kinds) removeEventListener(kind, go, true); resolve(); };
    for (const kind of kinds) addEventListener(kind, go, true);
    const idle = () => (globalThis.requestIdleCallback ?? ((f) => setTimeout(f, IDLE_FALLBACK_MS)))(go);
    if (document.readyState === "complete") idle(); else addEventListener("load", idle, { once: true });
  });
  return {
    started,
    get holding() { return holding; },
    /** Hold a batch while the document is the page; false once replaced. */
    hold(batch) { if (holding) held.push(batch); return holding; },
    /** Replace the document with the runtime's tree, once; replay its
     * presses a task later, after the caller's readiness. */
    release(apply) {
      if (!holding) return;
      holding = false;
      root.removeEventListener("click", record, true);
      root.replaceChildren();
      for (const batch of held.splice(0)) apply(batch);
      setTimeout(() => { for (const chain of presses.splice(0)) replay(chain); }, 0);
    },
  };
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

globalThis.exact.documentBoot = documentBoot;
globalThis.exact.documentHead = documentHead;
