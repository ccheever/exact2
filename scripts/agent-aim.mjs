// A tap addressed by id presses what it names (LLP 1012 §1 `tap`), the web's
// half of `host/apple/Sources/ExactKit/AgentAddressedTap.swift`: a plain
// `tap <target>` aims at the target's middle, and a click there reaches the
// innermost element with a press of its own — a link card or a Like button
// inside a post row (the Bluesky clone, 2026-10-09). A target with a press
// of its own is clicked where a click reaches it (its middle, else the point
// of its box nearest the middle that does), or refused; a target without one
// never presses a control inside it in its name. `tap <target> at <x> <y>` is
// a point and keeps a point's semantics.

/** In the page (self-contained: Chrome evaluates its source, Playwright passes it): where a plain tap on view `id`, aimed
 *  at (`x`, `y`) in the viewport, must land. Null: there. `{at, avoided}`: at that point instead, its middle reaching
 *  `avoided`. `{error, pressing}`: refused. A press is an element's own when it has a `press` handler, is a link, takes
 *  a canvas's pointer, or is a button invoking a command or a popover; a form control takes the click itself; a disabled
 *  element stops it. */
export function pageAim({ id, x, y }) {
  const el = globalThis.exact?.views?.get(id);
  if (!el) return null;
  const takes = (e) => String(e.dataset?.exactOn ?? '').split(' ').includes('press') || e.matches('a[href], [data-gpu-input]')
    || (e.localName === 'button' && (e.hasAttribute('commandfor') || e.hasAttribute('popovertarget')));
  const control = (e) => e.matches('input:not([type="hidden"]), select, textarea, [contenteditable=""], [contenteditable="true"]');
  // What a click at (px, py) reaches, or null where it would not land on the target.
  const reach = (px, py) => {
    const hit = document.elementFromPoint(px, py);
    if (!hit || !(el === hit || el.contains(hit))) return null;
    for (let e = hit; e && e !== document.documentElement; e = e.parentElement) {
      if (e.matches(':disabled')) return { el: e, blocked: true };
      if (takes(e) || control(e)) return { el: e, control: control(e) && !takes(e) };
    }
    return { el: null };
  };
  // A node's id, by the element that is its view or holds it.
  const ids = new Map();
  for (const [key, view] of globalThis.exact.views) ids.set(view, key);
  const pressing = (r) => { for (let e = r.el; e; e = e.parentElement) if (ids.has(e)) return Number(ids.get(e)); return null; };
  const name = (r) => { const v = pressing(r); return r.control ? `the form control of #${v}` : r.blocked ? `disabled #${v}` : `#${v}`; };
  // An ancestor under the middle (the target passes its presses through) or nothing: the caller's checks decide.
  const middle = reach(x, y);
  if (!middle || middle.el === el) return null;
  const a = `#${id}`, d = name(middle), how = `tap ${pressing(middle) != null ? `#${pressing(middle)}` : d}, or tap ${a} at <x> <y> (a point in its box) for whatever a finger there reaches`;
  if (takes(el)) {
    const b = el.getBoundingClientRect();
    const left = Math.max(0, b.left), top = Math.max(0, b.top), right = Math.min(innerWidth, b.right), bottom = Math.min(innerHeight, b.bottom);
    if (right > left && bottom > top) {
      // The middles of a grid of cells of about 12 px (at most 32 a side), nearest the middle first, ties in reading order.
      const w = right - left, h = bottom - top, cols = Math.min(32, Math.max(3, Math.ceil(w / 12))), rows = Math.min(32, Math.max(3, Math.ceil(h / 12)));
      const points = [];
      for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
        const px = left + (c + 0.5) * w / cols, py = top + (r + 0.5) * h / rows;
        points.push([px, py, Math.hypot(px - x, py - y), r * cols + c]);
      }
      points.sort((p, q) => p[2] - q[2] || p[3] - q[3]);
      for (const [px, py] of points) if (reach(px, py)?.el === el) return { at: [px, py], avoided: { pressing: pressing(middle), what: d } };
    }
    return { error: `tap ${a} would press ${d} inside it, at its middle; no point of ${a} a finger can reach presses ${a} itself; ${how}`, pressing: pressing(middle), addressed: id };
  }
  if (middle.el && !middle.blocked && el.contains(middle.el)) {
    return { error: `tap ${a} would press ${d} inside it; ${a} has no press of its own, and a tap that names it never presses a control inside it; ${how}`, pressing: pressing(middle), addressed: id };
  }
  return null;
}

/** A refusal's node ids as a driver names them: `#12` becomes its testId (quoted when it has a space), else stays. */
export function namedRefusal(message, nodes) {
  if (!/^tap #\d+ would press /.test(message ?? '')) return message;
  return message.replace(/#(\d+)/g, (m, id) => {
    const testId = nodes.find((n) => n.id === Number(id))?.props?.testId;
    return testId ? (/\s/.test(testId) ? JSON.stringify(testId) : testId) : m;
  });
}
