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
    const up = ev => {
      if (ev.pointerId !== s.held) return;
      s.held = null;
      document.removeEventListener("pointerup", up, true);
      document.removeEventListener("pointercancel", up, true);
      s.pointerup?.();
    };
    e.addEventListener("pointerdown", ev => {
      if (ev.button !== 0 || s.held !== null || e.disabled) return;
      s.held = ev.pointerId;
      document.addEventListener("pointerup", up, true);
      document.addEventListener("pointercancel", up, true);
      s.pointerdown?.();
    });
  }
  s[kind] = f;
}
