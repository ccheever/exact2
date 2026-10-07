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

import { hold, within } from './focus.js';

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

/** A `layout-transition` as the group's duration, delay and timing
 * function. The row is carried as the host writes it: times in
 * milliseconds without a unit (`300 0 ease`), or with one as authored. */
function curve(el) {
  const text = el?.style.getPropertyValue('--exact-layout-transition').trim();
  if (!text || text === 'none') return null;
  const decl = text.split(/,(?![^(]*\))/).pop().trim(); // the last declaration; a spring's commas are inside it
  const times = [];
  let timing = 'ease';
  for (const t of decl.match(/(?:cubic-bezier|steps|linear|spring)\([^)]*\)|\S+/g) ?? []) {
    const ms = /^(-?[\d.]+)(ms|s)?$/.exec(t);
    if (ms) { times.push(+ms[1] * (ms[2] === 's' ? 1000 : 1)); continue; }
    if (t !== 'all') timing = t;
  }
  const delay = times[1] ?? 0;
  const spring = /^spring\(\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+))?\s*\)$/.exec(timing);
  if (spring) return { ...springCurve(+spring[1], +spring[2], +(spring[3] ?? 1)), delay };
  return times[0] > 0 ? { duration: times[0], delay, timing } : null;
}

/** A spring from rest to its target as `linear()`, over its settle time:
 * a damped harmonic oscillator, settling as the native flight's does (its
 * progress runs 0 to 1000; at rest under a thousandth in displacement and
 * speed, on a 240 Hz grid, at most ten seconds: motion/src/spring.rs). */
function springCurve(k, c, m) {
  const w0 = Math.sqrt(k / m), z = c / (2 * Math.sqrt(k * m));
  const x = t => {
    if (z < 1) { const wd = w0 * Math.sqrt(1 - z * z); return Math.exp(-z * w0 * t) * (Math.cos(wd * t) + (z * w0 / wd) * Math.sin(wd * t)); }
    if (z === 1) return Math.exp(-w0 * t) * (1 + w0 * t);
    const r1 = -w0 * (z - Math.sqrt(z * z - 1)), r2 = -w0 * (z + Math.sqrt(z * z - 1));
    return (r2 * Math.exp(r1 * t) - r1 * Math.exp(r2 * t)) / (r2 - r1);
  };
  const at = n => { const t = n / 240, h = 1e-6; return Math.abs(1000 * x(t)) < 1e-3 && Math.abs(1000 * (x(t + h) - x(Math.max(0, t - h))) / (t > h ? 2 * h : h)) < 1e-3; };
  let n = 1;
  while (n < 2400 && !at(n)) n++;
  const end = n / 240;
  const points = [];
  for (let i = 0; i <= 48; i++) points.push(+(1 - x(end * i / 48)).toFixed(4));
  points[48] = 1;
  return { duration: Math.round(end * 1000), timing: `linear(${points.join(', ')})` };
}

function ident() { return `exact-se-${++serial}`; }

/** Run a commit's tree update `tail`, inside a view transition when its
 * flush may hand a name on. Returns what `tail` returns, or true when it
 * waits for the browser. */
export function commit(tail, queue, inflight, after) {
  // A deferred tree update keeps the press that caused it (focus.js), so a field it mounts may still
  // take the focus from the pressed control.
  const deferred = () => {
    const held = hold(), run = () => { run.ran = true; return within(held, tail); };
    run.release = () => held?.release();
    return run;
  };
  if (pending) { pending.push(deferred()); return true; }
  if (typeof document.startViewTransition !== 'function' || matchMedia('(prefers-reduced-motion: reduce)').matches) return tail();
  const old = leavers(queue);
  if (!old.size) return tail();
  const before = new Set(document.querySelectorAll(NAME)); // an arriver is new
  if (running) { running.skipTransition(); running = null; }
  // Every candidate is captured under its own name; which ones left, and
  // whether a name left once and arrived once, is known after the flush.
  const ids = new Map(), curves = new Map(); // element → ident, its curve
  for (const el of old.keys()) {
    const id = ident();
    ids.set(el, id);
    curves.set(el, curve(el));
    el.style.viewTransitionName = id;
  }
  // Only the pairs move: the root and unpaired leavers are not shown.
  style ??= document.head.appendChild(document.createElement('style'));
  style.textContent = ':root{view-transition-name:none}';
  pending = [deferred()];
  inflight.n++;
  let paired = false, result = true;
  const rules = [], named = [...old.keys()];
  const t = document.startViewTransition(() => {
    const tails = pending;
    pending = null;
    // A tail that throws skips the rest; their presses are let go all the same (focus.js).
    try { result = tails.map(f => f())[0]; } finally { for (const f of tails) if (!f.ran) f.release(); }
    // A leaver is gone, or kept only as an exit ghost (its own or an
    // ancestor's): it has left. One that stayed keeps its name and moves.
    const gone = new Map(); // name → the leavers that left
    for (const [el, name] of old) {
      const ghost = el.isConnected ? el.closest('[data-exiting]') : null;
      // Stayed: shown as it is, not moved on the browser's own curve (its
      // own layout transition, if any, moves it).
      if (el.isConnected && !ghost && el.getAttribute('data-shared-element') === name) { rules.push(`::view-transition-group(${ids.get(el)}),::view-transition-old(${ids.get(el)}),::view-transition-new(${ids.get(el)}){animation:none}`); continue; }
      gone.set(name, [...(gone.get(name) ?? []), [el, ghost]]);
    }
    for (const [name, left] of gone) {
      const arrivers = [...document.querySelectorAll(`[data-shared-element="${CSS.escape(name)}"]`)].filter(e => !before.has(e));
      const to = left.length === 1 && arrivers.length === 1 ? arrivers[0] : null;
      const [el, ghost] = left[0];
      const c = to && (curve(to) ?? curves.get(el));
      if (!c) { for (const [e] of left) rules.push(`::view-transition-group(${ids.get(e)}){display:none}`); continue; }
      // The transition is its exit (D7): a ghost of its own goes; inside an
      // ancestor's, it is hidden and the rest leaves as it would.
      if (ghost === el) el.remove(); else if (ghost) el.style.visibility = 'hidden';
      const id = ids.get(el);
      to.style.viewTransitionName = id;
      named.push(to);
      to.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
      rules.push(`::view-transition-group(${id}),::view-transition-old(${id}),::view-transition-new(${id}){animation-duration:${c.duration}ms;animation-delay:${c.delay}ms;animation-timing-function:${c.timing}}`);
      paired = true;
    }
    style.textContent = `:root{view-transition-name:none}${rules.join('')}`;
  });
  running = t;
  // Its own names only: a transition skipped by the next must not clear the
  // names the next just gave.
  const own = new Set(ids.values());
  const clean = () => {
    if (running === t) running = null;
    for (const el of named) if (own.has(el.style.viewTransitionName)) el.style.viewTransitionName = '';
  };
  t.updateCallbackDone.then(() => { if (!paired) t.skipTransition(); }, () => {});
  // Its animations exist once it is ready: what runs after a commit's tree
  // (the agent's clock registers and seeks them) runs again then. The agent
  // waits for that before it moves the clock (agent.js).
  const ready = t.ready.then(() => { for (const f of after ?? []) f(); }, () => {}).finally(() => inflight.n--);
  (globalThis.exact ??= {}).viewTransition = () => ready;
  t.finished.then(clean, clean);
  return result;
}
