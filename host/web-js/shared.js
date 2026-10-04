// Shared elements on the web (LLP 1013.000 D7): CSS View Transitions, the
// browser's own, for the commits that hand a `sharedElement` name from one
// element to another. Loaded after first paint with presence-glue.js, by a
// plan with a `layout-transition` (rt.js `pr`), which a pair needs at one
// end at least; until then a commit cuts.
//
// Before a commit's flush, the named elements inside each region or list
// about to rerun are the possible leavers. If there are any, the commit's
// tree update runs inside `document.startViewTransition`: the leavers get a
// `view-transition-name` before the browser captures the old state; in the
// update, after the flush, each name's new holder gets the same one, its
// group the pair's curve (the arriver's `layout-transition`, else the
// leaver's; a spring as `linear()`), and is scrolled into view. A leaver
// still there keeps its name (it moves, as the browser moves it). With no
// pair that has a curve the transition is skipped and the update still
// runs. The browser calls the update asynchronously: until it has run, later
// commits' updates wait behind it, in order.

const NAME = '[data-shared-element]';
let pending = null; // the tails waiting for a transition's update
let running = null; // the transition animating, if any
let style = null;
let serial = 0;

/** The named elements a flush may remove: those inside each region or
 * list `queue` reruns. */
function leavers(queue) {
  const out = new Map();
  for (const n of queue) {
    if (!n.$r || n.gone) continue;
    const [a, b] = n.$r();
    if (!a || !b || a.parentNode !== b.parentNode) continue;
    for (let e = a.nextSibling; e && e !== b; e = e.nextSibling) {
      if (e.nodeType !== 1) continue;
      if (e.matches(NAME)) out.set(e, e.getAttribute('data-shared-element'));
      for (const d of e.querySelectorAll(NAME)) out.set(d, d.getAttribute('data-shared-element'));
    }
  }
  return out;
}

/** A `layout-transition` as the group's duration and timing function. */
function curve(el) {
  const text = el?.style.getPropertyValue('--exact-layout-transition').trim();
  if (!text || text === 'none') return null;
  const decl = text.split(/,(?![^(]*\))/).pop().trim(); // the last declaration; a spring's commas are inside it
  const spring = /spring\(\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+))?\s*\)/.exec(decl);
  if (spring) return springCurve(+spring[1], +spring[2], +(spring[3] ?? 1));
  let duration = 0, timing = 'ease';
  for (const t of decl.match(/(?:cubic-bezier|steps|linear)\([^)]*\)|\S+/g) ?? []) {
    const ms = /^(-?[\d.]+)(ms|s)$/.exec(t);
    if (ms) { if (!duration) duration = +ms[1] * (ms[2] === 's' ? 1000 : 1); continue; }
    if (t !== 'all') timing = t;
  }
  return duration > 0 ? { duration, timing } : null;
}

/** A spring from rest to its target as `linear()`, over its settle time
 * (a damped harmonic oscillator; the engine's rest threshold). */
function springCurve(k, c, m) {
  const w0 = Math.sqrt(k / m), z = c / (2 * Math.sqrt(k * m));
  const x = t => {
    if (z < 1) { const wd = w0 * Math.sqrt(1 - z * z); return Math.exp(-z * w0 * t) * (Math.cos(wd * t) + (z * w0 / wd) * Math.sin(wd * t)); }
    if (z === 1) return Math.exp(-w0 * t) * (1 + w0 * t);
    const r1 = -w0 * (z - Math.sqrt(z * z - 1)), r2 = -w0 * (z + Math.sqrt(z * z - 1));
    return (r2 * Math.exp(r1 * t) - r1 * Math.exp(r2 * t)) / (r2 - r1);
  };
  let end = 0.05;
  while (end < 4 && !(Math.abs(x(end)) < 0.001 && Math.abs(x(end) - x(end - 0.004)) < 0.0005)) end += 0.004;
  const points = [];
  for (let i = 0; i <= 48; i++) points.push(+(1 - x(end * i / 48)).toFixed(4));
  points[48] = 1;
  return { duration: Math.round(end * 1000), timing: `linear(${points.join(', ')})` };
}

function ident() { return `exact-se-${++serial}`; }

/** Run a commit's tree update `tail`, inside a view transition when its
 * flush may hand a name on. Returns what `tail` returns, or true when it
 * waits for the browser. */
export function commit(tail, queue, inflight) {
  if (pending) { pending.push(tail); return true; }
  if (typeof document.startViewTransition !== 'function' || matchMedia('(prefers-reduced-motion: reduce)').matches) return tail();
  const old = leavers(queue);
  if (!old.size) return tail();
  if (running) { running.skipTransition(); running = null; }
  const names = new Map(); // name → ident
  const curves = new Map(); // ident → the leaver's curve
  for (const [el, name] of old) {
    if (names.has(name)) { names.set(name, null); continue; } // two leavers: no pair
    const id = ident();
    names.set(name, id);
    curves.set(id, curve(el));
    el.style.viewTransitionName = id;
  }
  // Only the pairs move: the root and unpaired leavers are not shown.
  style ??= document.head.appendChild(document.createElement('style'));
  style.textContent = ':root{view-transition-name:none}';
  pending = [tail];
  inflight.n++;
  let paired = false, result = true;
  const rules = [];
  const t = document.startViewTransition(() => {
    const tails = pending;
    pending = null;
    result = tails.map(f => f())[0];
    for (const [el, name] of old) {
      const id = names.get(name);
      if (!id) { el.style.viewTransitionName = ''; continue; }
      if (el.isConnected && el.getAttribute('data-shared-element') === name) continue; // stayed: it moves
      const arrivers = [...document.querySelectorAll(`[data-shared-element="${CSS.escape(name)}"]`)].filter(e => !old.has(e));
      const to = arrivers.length === 1 ? arrivers[0] : null;
      const c = to && (curve(to) ?? curves.get(id));
      if (!c) { rules.push(`::view-transition-group(${id}){display:none}`); continue; }
      to.style.viewTransitionName = id;
      to.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
      rules.push(`::view-transition-group(${id}),::view-transition-old(${id}),::view-transition-new(${id}){animation-duration:${c.duration}ms;animation-timing-function:${c.timing}}`);
      paired = true;
    }
    style.textContent = `:root{view-transition-name:none}${rules.join('')}`;
  });
  running = t;
  const clean = () => {
    if (running === t) running = null;
    for (const el of document.querySelectorAll(NAME)) if (el.style.viewTransitionName.startsWith('exact-se-')) el.style.viewTransitionName = '';
  };
  t.updateCallbackDone.then(() => { if (!paired) t.skipTransition(); }, () => {});
  t.ready.then(() => {}, () => {}).finally(() => inflight.n--);
  t.finished.then(clean, clean);
  return result;
}
