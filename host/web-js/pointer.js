// DOM's own `pointerdown` and `pointerup` (LLP 1005 §3), as the web host's
// input-glue `pointer`: the primary button or a touch going down on an
// element, then up or cancelled (a cancel is an up). The two handlers of one
// element share one held pointer; its up is heard on the document, so it
// arrives wherever the pointer lifts (a pointer capture would retarget the
// click there too).
export function pointer(e, kind, f) {
  let s = e.$pointer;
  if (!s) {
    s = e.$pointer = { held: null };
    // The end: an up or cancel anywhere in the document, or the pointer
    // leaving it (out of the window, into a frame) or the window losing
    // focus; never for an element the tree has since removed.
    const ends = [["pointerup", document], ["pointercancel", document], ["pointerout", document], ["blur", window]];
    const up = ev => {
      if (ev.type === "blur" ? ev.target !== window : ev.pointerId !== s.held || (ev.type === "pointerout" && ev.relatedTarget && ev.relatedTarget.localName !== "iframe")) return;
      s.held = null;
      for (const [type, target] of ends) target.removeEventListener(type, up, target === document);
      // Never for a removed element, or one kept only for its exit animation.
      if (e.isConnected && !e.closest("[data-exiting]")) s.pointerup?.();
    };
    e.addEventListener("pointerdown", ev => {
      // The innermost enabled pointer node takes it, as the web host's does.
      if (ev.$pointerOwner || !ev.isPrimary || ev.button !== 0 || s.held !== null || e.matches(":disabled") || e.hasAttribute("disabled") || e.closest("[inert]")) return;
      ev.$pointerOwner = e;
      s.held = ev.pointerId;
      for (const [type, target] of ends) target.addEventListener(type, up, target === document);
      s.pointerdown?.();
    });
  }
  s[kind] = f;
}
