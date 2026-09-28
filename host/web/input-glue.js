// Input-only glue: loaded after the baked first pixel, independently of data readiness.
export function createInputHandlers({ root, views, retiredViews, ready, inertAncestor, dispatch, agentMode = false }) {
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
  // browser eases a separate factor, multiplied into CSS `scale`, so
  // authored transforms, transitions and keyframes keep their values.
  const feedback = new WeakMap();
  const showPress = (el, down) => {
    el.toggleAttribute("data-pressed", down);
    const to = down ? Number(el.style.getPropertyValue("--exact-press")) : 1;
    const old = feedback.get(el);
    if (old?.to === to || !old && to === 1) return;
    const progress = old?.animation.effect.getComputedTiming().progress ?? 0;
    const from = old ? old.from + (old.to - old.from) * progress : 1;
    old?.animation.cancel();
    const animation = el.animate([{ "--exact-press-factor": from }, { "--exact-press-factor": to }], {
      duration: agentMode ? 0 : 120, easing: "cubic-bezier(0.16,1,0.3,1)", fill: "both",
    });
    const state = { animation, from, to };
    feedback.set(el, state);
    animation.onfinish = () => {
      if (to === 1 && feedback.get(el) === state) { animation.cancel(); feedback.delete(el); }
    };
    if (agentMode) animation.finish();
  };
  // A re-press may catch the release easing out. Neutralize only our effect
  // for this synchronous measurement, then restore it before a frame. The
  // browser undoes exactly that factor about transform-origin, including
  // SVG's reference box and transformed ancestors; no transform is guessed.
  const unpressedBox = el => {
    const effect = feedback.get(el)?.animation.effect;
    const frames = effect?.getKeyframes();
    effect?.setKeyframes([{ "--exact-press-factor": 1 }]);
    const box = el.getBoundingClientRect();
    if (effect) effect.setKeyframes(frames);
    return box;
  };
  let press = null;
  const release = () => { if (press) showPress(press.el, false); press = null; };
  root.addEventListener("pointerdown", e => {
    if (e.button !== 0 || !e.isPrimary) return;
    release(); // a press whose release never reached the page
    const el = e.target.closest?.("[data-exact-on~=press]");
    if (!el || !root.contains(el) || !el.style.getPropertyValue("--exact-press") || el.closest(":disabled,[disabled='true']")) return;
    const r = unpressedBox(el);
    press = { el, id: e.pointerId, box: [r.left, r.top, r.right, r.bottom] };
    showPress(el, true);
  }, true);
  document.addEventListener("pointermove", e => {
    if (e.pointerId !== press?.id) return;
    const [l, t, r, b] = press.box;
    showPress(press.el, e.clientX >= l && e.clientX <= r && e.clientY >= t && e.clientY <= b);
  }, true);
  for (const type of ["pointerup", "pointercancel"]) document.addEventListener(type, e => { if (e.pointerId === press?.id) release(); }, true);
  return {
    pan(el, id, on, hears = []) {
      // @ref LLP 1043.000 §3 D8: one coalesced action per display frame.
      // LLP 1057.001 §1: an inner swipe that is still deciding (its pointer
      // 'pending' in exact.contacts) goes first; the pan waits for its verdict,
      // and a pan that began cancels the click (rule 4).
      // LLP 1057.002 §6.7 (spike B): `panstart` before the first delta and
      // `panend` with the release velocity, where the node hears them.
      let contact = null, frame = 0, suppressClick = false;
      const contacts = () => (globalThis.exact ??= {}).contacts ??= new Map();
      const live = () => views.get(id) === el && !retiredViews.has(el) && ready() && !inertAncestor(el) && !el.closest(":disabled,[disabled='true']");
      const flush = () => {
        cancelAnimationFrame(frame); frame = 0;
        if (!contact || !live()) { contact = null; return; }
        const [x,y] = contact.to, [px,py] = contact.from;
        // exact_motion::gesture::SLOP; a pan-only plan links no motion export to ask.
        if (!contact.active && Math.max(Math.abs(x-px),Math.abs(y-py)) <= 4) return;
        if (!contact.active) { release(); if (hears.includes("panstart")) dispatch(id, "", 26); } // a pan ends a press, as it cancels a touch
        contact.active = true; contact.from = [x,y];
        if (x !== px || y !== py) dispatch(id, `${x-px},${y-py}`);
      };
      // exact_motion::VelocityTracker's estimate: recency-weighted least
      // squares over the last 100 ms of samples, zero with fewer than two.
      const velocity = (samples, now) => {
        const recent = samples.filter(([t]) => now - t <= 0.1);
        if (recent.length < 2) return [0, 0];
        let sw = 0, st = 0, sx = 0, sy = 0;
        for (const [t, x, y] of recent) { const w = 1 - (now - t) / 0.1; sw += w; st += w * t; sx += w * x; sy += w * y; }
        const [mt, mx, my] = [st / sw, sx / sw, sy / sw];
        let stt = 0, stx = 0, sty = 0;
        for (const [t, x, y] of recent) { const w = 1 - (now - t) / 0.1, dt = t - mt; stt += w * dt * dt; stx += w * dt * (x - mx); sty += w * dt * (y - my); }
        return stt > 0 ? [stx / stt, sty / stt] : [0, 0];
      };
      const end = (e, cancelled) => {
        if (!contact?.active || !hears.includes("panend") || !live()) return;
        const [vx, vy] = cancelled ? [0, 0] : velocity(contact.samples, e.timeStamp / 1000);
        dispatch(id, `${vx},${vy}`, 27);
      };
      const waiting = e => {
        if (!contact.deferred) return false;
        const state = contacts().get(e.pointerId);
        if (state === "pending") return true;
        if (state === "claimed") { contact = null; return true; }
        contact.deferred = false; el.setPointerCapture(e.pointerId); return false;
      };
      on("pointermove", e => { if (contact?.pointer !== e.pointerId || waiting(e)) return; contact.to=[e.clientX,e.clientY]; contact.samples.push([e.timeStamp/1000,e.clientX,e.clientY]); if (contact.samples.length > 16) contact.samples.shift(); if (!frame) frame=requestAnimationFrame(flush); });
      on("pointerup", e => { if (contact?.pointer !== e.pointerId || contact.deferred && contacts().get(e.pointerId) === "claimed") { if (contact?.pointer === e.pointerId) contact = null; return; } contact.to=[e.clientX,e.clientY]; flush(); end(e, false); suppressClick = !!contact?.active; contact=null; });
      on("pointercancel", e => { cancelAnimationFrame(frame); frame=0; end(e, true); contact=null; });
      // A child's implicit touch capture, lost when a deferred pan takes it, bubbles here.
      on("lostpointercapture", e => { if (e.target !== el) return; cancelAnimationFrame(frame); frame=0; end(e, true); contact=null; });
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
        contact = {pointer:e.pointerId,from:[e.clientX,e.clientY],to:[e.clientX,e.clientY],active:false,deferred,samples:[[e.timeStamp/1000,e.clientX,e.clientY]]};
      };
    },
  };
}

if (globalThis.exact) globalThis.exact.createInputHandlers = createInputHandlers;
