// Input-only glue: loaded after the baked first pixel, independently of data readiness.
export function createInputHandlers({ root, views, retiredViews, ready, inertAncestor, dispatch }) {
  // @ref LLP 1038 §7 — a plain click on a same-origin link to a declared
  // route stays in this document: a link with its own `press` navigates by
  // it; any other goes to the root's `navigate` handler, as popstate does.
  // A modified or other-button click, a `target` or `download`, is the
  // browser's alone — a pressing link's press does not also run — and so are
  // other origins, fragments of this page and undeclared paths (a file).
  // The page's own `exact` names the route table and the root's handler.
  document.addEventListener("click", event => {
    const a = event.target.closest?.("a[href]");
    if (!a || !root.contains(a) || event.defaultPrevented || !ready()) return;
    const press = a.exactHandlers?.includes("press");
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
      || (a.target && a.target !== "_self") || a.hasAttribute("download")) { if (press) event.stopPropagation(); return; }
    const url = new URL(a.href), to = url.pathname + url.search, here = to === location.pathname + location.search;
    const { wasm, writeIn, navigate } = globalThis.exact;
    if (url.origin !== location.origin || (here && url.hash) || wasm.exact_route_match(writeIn(to)) !== 1) return;
    const nav = root.firstElementChild;
    if (!press && !(nav?.hasAttribute("navigationBack") && nav.exactHandlers?.includes("navigate"))) return;
    event.preventDefault();
    if (press || here) return;
    if (!navigate(to)?.ops?.some(op => op.op === "router")) wasm.exact_log(writeIn(`history: link ${JSON.stringify(to)} refused`));
  }, true);
  document.addEventListener("keydown", (event) => {
    if (event.isComposing || !ready() || event.defaultPrevented) return;
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
  return {
    pan(el, id, on) {
      // @ref LLP 1043.000 §3 D8: one coalesced action per display frame.
      // LLP 1057.001 §1: an inner swipe that is still deciding (its pointer
      // 'pending' in exact.contacts) goes first; the pan waits for its verdict,
      // and a pan that began cancels the click (rule 4).
      let contact = null, frame = 0, suppressClick = false;
      const contacts = () => (globalThis.exact ??= {}).contacts ??= new Map();
      const live = () => views.get(id) === el && !retiredViews.has(el) && ready() && !inertAncestor(el) && !el.closest(":disabled,[disabled='true']");
      const flush = () => {
        cancelAnimationFrame(frame); frame = 0;
        if (!contact || !live()) { contact = null; return; }
        const [x,y] = contact.to, [px,py] = contact.from;
        // exact_motion::gesture::SLOP; a pan-only plan links no motion export to ask.
        if (!contact.active && Math.max(Math.abs(x-px),Math.abs(y-py)) <= 4) return;
        contact.active = true; contact.from = [x,y];
        if (x !== px || y !== py) dispatch(id, `${x-px},${y-py}`);
      };
      const waiting = e => {
        if (!contact.deferred) return false;
        const state = contacts().get(e.pointerId);
        if (state === "pending") return true;
        if (state === "claimed") { contact = null; return true; }
        contact.deferred = false; el.setPointerCapture(e.pointerId); return false;
      };
      on("pointermove", e => { if (contact?.pointer !== e.pointerId || waiting(e)) return; contact.to=[e.clientX,e.clientY]; if (!frame) frame=requestAnimationFrame(flush); });
      on("pointerup", e => { if (contact?.pointer !== e.pointerId || contact.deferred && contacts().get(e.pointerId) === "claimed") { if (contact?.pointer === e.pointerId) contact = null; return; } contact.to=[e.clientX,e.clientY]; flush(); suppressClick = !!contact?.active; contact=null; });
      on("pointercancel", () => { cancelAnimationFrame(frame); frame=0; contact=null; });
      // A child's implicit touch capture, lost when a deferred pan takes it, bubbles here.
      on("lostpointercapture", e => { if (e.target !== el) return; cancelAnimationFrame(frame); frame=0; contact=null; });
      el.addEventListener("click", e => { if (suppressClick) { suppressClick = false; e.preventDefault(); e.stopImmediatePropagation(); } }, true);
      return e => {
        // Only the click right after a pan is suppressed; a drag makes none.
        suppressClick = false;
        if (!live() || !e.isPrimary || e.button !== 0 || contact || el.matches("input,textarea,[contenteditable]")) return;
        // A control or press handler between the contact and this node keeps it (rule 3).
        const inner = e.target.closest("input,textarea,select,button,a[href],[contenteditable],[data-exact-on~='press']");
        if (inner && inner !== el && el.contains(inner)) return;
        const deferred = contacts().get(e.pointerId) === "pending";
        e.preventDefault(); e.stopPropagation(); if (!deferred) el.setPointerCapture(e.pointerId);
        contact = {pointer:e.pointerId,from:[e.clientX,e.clientY],to:[e.clientX,e.clientY],active:false,deferred};
      };
    },
  };
}

if (globalThis.exact) globalThis.exact.createInputHandlers = createInputHandlers;
