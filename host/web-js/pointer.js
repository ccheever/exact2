// DOM's own `pointerdown`, `pointerup` and `pointermove` (LLP 1005 §3; LLP
// 1056 §3 stage 3), as the web host's input-glue `pointer`: a button (any,
// studio diary R22) or a touch going down on an element, then up or cancelled (a cancel
// is an up), and the pointer moving over it or, once down on it, anywhere
// until it lifts. The down and the up bubble (Charlie, 2026-10-15): every
// enabled element from the target up that hears either holds the pointer
// for its up, innermost first, and the innermost hearing any of the three
// also for its moves. The down goes along the event's own path from one
// listener at the document's capture, before any element's, so a recognizer
// that stops the event (a pan taking its contact) takes nothing from the
// pointer events, as on the native hosts. The up and moves are heard on the
// document, so they arrive wherever the pointer goes (a pointer capture
// would retarget the click there too). Each handler gets the `PointerEvent`
// record; moves go out at most once a frame, the latest.

// The record: the point from the element's content box in its own CSS px
// (a scale undone), DOM's buttons and pressure, the device and its id, the
// viewport point (`frame()`'s space, LLP 1094 D11), the modifiers held.
export function record(e, ev, lifted = false) {
  const r = e.getBoundingClientRect(), cs = getComputedStyle(e);
  const sx = e.offsetWidth ? r.width / e.offsetWidth : 1, sy = e.offsetHeight ? r.height / e.offsetHeight : 1;
  const left = parseFloat(cs.borderLeftWidth) + parseFloat(cs.paddingLeft), top = parseFloat(cs.borderTopWidth) + parseFloat(cs.paddingTop);
  const type = ev.pointerType === "pen" || ev.pointerType === "touch" ? ev.pointerType : "mouse";
  return [(ev.clientX - r.left) / (sx || 1) - left, (ev.clientY - r.top) / (sy || 1) - top,
    lifted ? 0 : ev.buttons, lifted ? 0 : Math.min(1, Math.max(0, ev.pressure || 0)), type, ev.pointerId ?? 1, ev.clientX, ev.clientY, ev.shiftKey, ev.ctrlKey, ev.altKey, ev.metaKey];
}
let walking = false;
export function pointer(e, kind, f) {
  if (!walking) {
    walking = true;
    document.addEventListener("pointerdown", ev => { for (const n of ev.composedPath()) n.$pointer?.down(ev); }, true);
  }
  let s = e.$pointer;
  if (!s) {
    s = e.$pointer = { held: null, owner: false, last: null, move: null, frame: 0 };
    const live = () => e.isConnected && !e.closest("[data-exiting]");
    const flush = () => {
      cancelAnimationFrame(s.frame); s.frame = 0;
      const m = s.move; s.move = null;
      if (m && live()) s.pointermove?.(record(e, m));
    };
    const moved = ev => { s.last = ev; s.move = ev; if (!s.frame) s.frame = requestAnimationFrame(flush); };
    // While held, its pointer anywhere in the document: the owner's moves,
    // and every holder's latest point, which a blur's up reports.
    const held = ev => { if (ev.pointerId !== s.held) return; s.last = ev; if (s.owner && s.pointermove) moved(ev); };
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
    s.down = ev => {
      // Any button, as the DOM's (studio diary R22: a secondary click's down; `buttons` says which).
      // An ancestor a handler further in removed is still on the path.
      if (!ev.isPrimary || s.held !== null || !live() || e.matches(":disabled") || e.hasAttribute("disabled") || e.closest("[inert]")) return;
      // The innermost enabled pointer element holds its moves, as the web
      // host's does; an ancestor it bubbles to holds only its up.
      s.owner = !ev.$pointerOwner;
      if (!s.owner && !s.pointerdown && !s.pointerup) return;
      ev.$pointerOwner = e;
      s.held = ev.pointerId; s.last = ev;
      for (const [type, target] of ends) target.addEventListener(type, up, target === document);
      document.addEventListener("pointermove", held, true);
      flush();
      s.pointerdown?.(record(e, ev));
    };
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
