// DOM's own `pointerdown` and `pointerup` (LLP 1005 §3), as the web host's
// input-glue `pointer`: the primary button or a touch going down on an
// element, then up or cancelled (a cancel is an up). The two handlers of one
// element share one held pointer, captured so the up arrives wherever it
// lifts.
export function pointer(e, kind, f) {
  let s = e.$pointer;
  if (!s) {
    s = e.$pointer = { held: null };
    e.addEventListener("pointerdown", ev => {
      if (ev.button !== 0 || s.held !== null || e.disabled) return;
      s.held = ev.pointerId;
      if (s.pointerup) try { e.setPointerCapture(ev.pointerId); } catch {}
      s.pointerdown?.();
    });
    const up = ev => { if (ev.pointerId !== s.held) return; s.held = null; s.pointerup?.(); };
    e.addEventListener("pointerup", up);
    e.addEventListener("pointercancel", up);
  }
  s[kind] = f;
}
