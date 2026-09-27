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
  // @ref LLP 1061 D3 — press feedback by UIKit's rule, not `:active`'s: the
  // innermost node with a `press` handler takes the press (its pressable
  // ancestors, which `:active` would also match, do not), and shows it only
  // while the pointer is inside the box it had when pressed — leaving
  // releases it, coming back presses again; a pan or a cancel ends it. The
  // node carries `data-pressed`; the shell's rule scales it. Its box is read
  // unpressed: a press re-taken while the last one still eases out is
  // scaled back about its centre.
  let press = null;
  const release = () => { press?.el.removeAttribute("data-pressed"); press = null; };
  root.addEventListener("pointerdown", e => {
    if (e.button !== 0 || !e.isPrimary) return;
    release(); // a press whose release never reached the page
    const el = e.target.closest?.("[data-exact-on~=press]");
    if (!el || !root.contains(el) || !el.style.getPropertyValue("--exact-press") || el.closest(":disabled,[disabled='true']")) return;
    const r = el.getBoundingClientRect(), f = new DOMMatrix(getComputedStyle(el).transform).a || 1;
    const [cx, cy, w, h] = [r.left + r.width / 2, r.top + r.height / 2, r.width / f, r.height / f];
    press = { el, id: e.pointerId, box: [cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2] };
    el.setAttribute("data-pressed", "");
  }, true);
  document.addEventListener("pointermove", e => {
    if (e.pointerId !== press?.id) return;
    const [l, t, r, b] = press.box;
    press.el.toggleAttribute("data-pressed", e.clientX >= l && e.clientX <= r && e.clientY >= t && e.clientY <= b);
  }, true);
  for (const type of ["pointerup", "pointercancel"]) document.addEventListener(type, e => { if (e.pointerId === press?.id) release(); }, true);
  return {
    pan(el, id, on) {
      // @ref LLP 1043.000 §3 D8: one coalesced action per display frame.
      let contact = null, frame = 0;
      const live = () => views.get(id) === el && !retiredViews.has(el) && ready() && !inertAncestor(el) && !el.closest(":disabled,[disabled='true']");
      const flush = () => {
        cancelAnimationFrame(frame); frame = 0;
        if (!contact || !live()) { contact = null; return; }
        const [x,y] = contact.to, [px,py] = contact.from;
        if (!contact.active && Math.max(Math.abs(x-px),Math.abs(y-py)) <= 4) return;
        if (!contact.active) release(); // a pan ends a press, as it cancels a touch
        contact.active = true; contact.from = [x,y];
        if (x !== px || y !== py) dispatch(id, `${x-px},${y-py}`);
      };
      on("pointermove", e => { if (contact?.pointer !== e.pointerId) return; contact.to=[e.clientX,e.clientY]; if (!frame) frame=requestAnimationFrame(flush); });
      on("pointerup", e => { if (contact?.pointer !== e.pointerId) return; contact.to=[e.clientX,e.clientY]; flush(); contact=null; });
      on("pointercancel", () => { cancelAnimationFrame(frame); frame=0; contact=null; });
      on("lostpointercapture", () => { cancelAnimationFrame(frame); frame=0; contact=null; });
      return e => {
        if (!live() || !e.isPrimary || e.button !== 0 || contact || e.target.closest("input,textarea,button,[contenteditable]")) return;
        e.preventDefault(); e.stopPropagation(); el.setPointerCapture(e.pointerId);
        contact = {pointer:e.pointerId,from:[e.clientX,e.clientY],to:[e.clientX,e.clientY],active:false};
      };
    },
  };
}

if (globalThis.exact) globalThis.exact.createInputHandlers = createInputHandlers;
