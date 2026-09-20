// Input-only glue: loaded after the baked first pixel, independently of data readiness.
export function createInputHandlers({ root, views, retiredViews, ready, inertAncestor, dispatch }) {
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
      let contact = null, frame = 0;
      const live = () => views.get(id) === el && !retiredViews.has(el) && ready() && !inertAncestor(el) && !el.closest(":disabled,[disabled='true']");
      const flush = () => {
        cancelAnimationFrame(frame); frame = 0;
        if (!contact || !live()) { contact = null; return; }
        const [x,y] = contact.to, [px,py] = contact.from;
        if (!contact.active && Math.max(Math.abs(x-px),Math.abs(y-py)) <= 4) return;
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
