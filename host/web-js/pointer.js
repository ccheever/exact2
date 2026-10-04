// DOM's own `pointerdown`, `pointerup` and `pointermove` (LLP 1005 §3; LLP
// 1056 §3 stage 3), as the web host's input-glue `pointer`: the primary
// button or a touch going down on an element, then up or cancelled (a cancel
// is an up), and the pointer moving over it or, once down on it, anywhere
// until it lifts. The element's handlers share one held pointer; its up and
// moves are heard on the document, so they arrive wherever the pointer goes
// (a pointer capture would retarget the click there too). Each handler gets
// the `PointerEvent` record; moves go out at most once a frame, the latest.

// The record: the point from the element's content box in its own CSS px
// (a scale undone), DOM's buttons and pressure, the device and its id.
export function record(e, ev, lifted = false) {
  const r = e.getBoundingClientRect(), cs = getComputedStyle(e);
  const sx = e.offsetWidth ? r.width / e.offsetWidth : 1, sy = e.offsetHeight ? r.height / e.offsetHeight : 1;
  const left = parseFloat(cs.borderLeftWidth) + parseFloat(cs.paddingLeft), top = parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop);
  const type = ev.pointerType === "pen" || ev.pointerType === "touch" ? ev.pointerType : "mouse";
  return [(ev.clientX - r.left) / (sx || 1) - left, (ev.clientY - r.top) / (sy || 1) - top,
    lifted ? 0 : ev.buttons, lifted ? 0 : Math.min(1, Math.max(0, ev.pressure || 0)), type, ev.pointerId];
}
export function pointer(e, kind, f) {
  let s = e.$pointer;
  if (!s) {
    s = e.$pointer = { held: null, last: null, move: null, frame: 0 };
    const live = () => e.isConnected && !e.closest("[data-exiting]");
    const flush = () => {
      cancelAnimationFrame(s.frame); s.frame = 0;
      const m = s.move; s.move = null;
      if (m && live()) s.pointermove?.(record(e, m));
    };
    const moved = ev => { s.last = ev; s.move = ev; if (!s.frame) s.frame = requestAnimationFrame(flush); };
    // While held, the moves of its pointer anywhere in the document.
    const held = ev => { if (ev.pointerId === s.held && s.pointermove) moved(ev); };
    // The end: an up or cancel anywhere in the document, or the pointer
    // leaving it (out of the window, into a frame) or the window losing
    // focus; never for an element the tree has since removed.
    const ends = [["pointerup", document], ["pointercancel", document], ["pointerout", document], ["blur", window]];
    const up = ev => {
      if (ev.type === "blur" ? ev.target !== window : ev.pointerId !== s.held || (ev.type === "pointerout" && ev.relatedTarget && ev.relatedTarget.localName !== "iframe")) return;
      s.held = null;
      for (const [type, target] of ends) target.removeEventListener(type, up, target === document);
      document.removeEventListener("pointermove", held, true);
      flush();
      // Never for a removed element, or one kept only for its exit animation.
      if (live()) s.pointerup?.(record(e, ev.type === "blur" ? s.last : ev, true));
    };
    e.addEventListener("pointerdown", ev => {
      // The innermost enabled pointer node takes it, as the web host's does.
      if (ev.$pointerOwner || !ev.isPrimary || ev.button !== 0 || s.held !== null || e.matches(":disabled") || e.hasAttribute("disabled") || e.closest("[inert]")) return;
      ev.$pointerOwner = e;
      s.held = ev.pointerId; s.last = ev;
      for (const [type, target] of ends) target.addEventListener(type, up, target === document);
      document.addEventListener("pointermove", held, true);
      flush();
      s.pointerdown?.(record(e, ev));
    });
    // A free pointer over it (no button down, as AppKit sends `mouseMoved`
    // only then): the innermost element hearing moves takes them.
    e.addEventListener("pointermove", ev => {
      if (ev.$pointerMover || !s.pointermove) return;
      ev.$pointerMover = e;
      if (s.held === null && ev.buttons === 0 && !e.matches(":disabled") && !e.hasAttribute("disabled") && !e.closest("[inert]")) moved(ev);
    });
  }
  s[kind] = f;
}
