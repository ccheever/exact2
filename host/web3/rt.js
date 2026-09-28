// The exact3 web spike's runtime: fine-grained signals over the DOM, for a
// plan compiled ahead of time by `exact-web3`. Everything here is imported
// by name, so an app's bundle carries only what its generated module uses.
//
// Semantics kept from the runner (LLP 1005 §6):
// - an action's writes land together at its commit (the VM collects
//   `StoreSlot`s), then one synchronous flush updates the DOM, then its
//   commands run (focus after the tree is there);
// - derives and resources settle lazily and glitch-free (a pull, not a push);
//   an equal value keeps its old object, so nothing downstream reruns;
// - a resource admits its compiled value only while its arguments equal the
//   compiled ones; a source not ready keeps the value and asks again when it
//   is; a later answer lands in its own commit, if its ticket is current;
// - timers fire on a clock the host moves (`advance`), each at its own time.

// ---------------------------------------------------------------- signals
let Listener = null, Owner = null, Queue = [], Flushing = false;
const CLEAN = 0, CHECK = 1, DIRTY = 2;

export function eq(a, b) {
  if (a === b) return a !== 0 || 1 / a === 1 / b;
  if (typeof a === "number" && typeof b === "number") return a !== a && b !== b;
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!eq(a[i], b[i])) return false;
  return true;
}

function node(fn, v, effect) {
  const n = { fn, v, effect, s: fn ? DIRTY : CLEAN, src: [], obs: new Set(), kids: null, gone: 0 };
  if (Owner) (Owner.kids ??= []).push(n);
  return n;
}
function read(n) {
  if (Listener && !n.obs.has(Listener)) { n.obs.add(Listener); Listener.src.push(n); }
  if (n.fn) fresh(n);
  return n.v;
}
function fresh(n) {
  if (n.s === CHECK) for (const s of n.src) { if (s.fn) fresh(s); if (n.s === DIRTY) break; }
  if (n.s === DIRTY) run(n);
  n.s = CLEAN;
}
function drop(n) {
  for (const s of n.src) s.obs.delete(n);
  n.src = [];
  if (n.kids) { for (const k of n.kids) dispose(k); n.kids = null; }
}
function dispose(n) { n.gone = 1; drop(n); }
function run(n) {
  drop(n);
  const [l, o] = [Listener, Owner];
  Listener = Owner = n;
  let v;
  try { v = n.fn(n.v); } finally { Listener = l; Owner = o; }
  if (!n.effect && !eq(n.v, v)) { n.v = v; for (const o of n.obs) o.s = DIRTY; }
}
function stale(n, state) {
  if (n.s < state) {
    if (n.s === CLEAN && n.effect) Queue.push(n);
    n.s = state;
    for (const o of n.obs) stale(o, CHECK);
  }
}
function write(n, v) {
  if (eq(n.v, v)) return;
  n.v = v;
  for (const o of n.obs) stale(o, DIRTY);
}
function flush() {
  if (Flushing) return;
  Flushing = true;
  try { for (let i = 0; i < Queue.length; i++) if (!Queue[i].gone) fresh(Queue[i]); }
  finally { Queue = []; Flushing = false; }
}
function untracked(f) { const l = Listener; Listener = null; try { return f(); } finally { Listener = l; } }

/** A slot: a getter, `.n` its node. */
export function sig(v) { const n = node(null, v); const g = () => read(n); g.n = n; return g; }
/** A derive: lazy, cached, equal results keep their object. */
export function memo(fn) { const n = node(fn); return () => read(n); }
export function effect(fn) { const n = node(fn, undefined, 1); fresh(n); return n; }
/** A scope whose effects `dispose` ends. */
function scope(f) { const n = node(null); const o = Owner; Owner = n; try { f(); } finally { Owner = o; } return n; }

// ---------------------------------------------------------------- commits
let Writes = null, Commands = [];
export const journal = [];
/** A write inside an action: collected, applied at commit. */
export function W(s, v) { Writes.push([s.n, v]); }
/** A host command inside an action: run after the commit. */
export function C(name, args) { Commands.push([name, args]); }
/** One commit: `f` runs, its writes land, the tree updates, its commands run. */
export function commit(f) {
  const outer = Writes;
  Writes = [];
  try { untracked(f); for (const [n, v] of Writes) write(n, v); }
  finally { const w = Writes; Writes = outer; if (!outer) flush(); }
  if (!outer) for (const c of Commands.splice(0)) command(...c);
}
/** An action: each call is one commit. */
export function act(fn) { return (...a) => commit(() => fn(...a)); }
const Hosts = {
  focus: id => document.getElementById(id)?.focus(),
  blur: id => document.getElementById(id)?.blur(),
  setScheme: s => { document.documentElement.style.colorScheme = s === "system" ? "" : s; },
  copyText: t => navigator.clipboard?.writeText(t),
};
function command(name, args) {
  const f = Hosts[name];
  journal.push(`command ${name}`);
  if (f) f(...args); else journal.push(`refused: ${name} is not a command this runtime carries`);
}

// ---------------------------------------------------------------- the clock and timers
export const clock = { now: 0, timers: [], agent: false };
/** A timer: `every(ms, action, once)`, due from mount. */
export function every(ms, action, once) {
  clock.timers.push({ due: clock.now + ms, ms, action, once });
  if (!clock.agent) drive();
}
/** Move the clock to `to`, firing each due timer at its own time, in order. */
export function advance(to) {
  for (;;) {
    let next = null;
    for (const t of clock.timers) if (t.due <= to && (!next || t.due < next.due)) next = t;
    if (!next) break;
    clock.now = next.due;
    if (next.once) clock.timers.splice(clock.timers.indexOf(next), 1); else next.due += next.ms;
    next.action();
  }
  clock.now = Math.max(clock.now, to);
}
let driving = 0, start = 0;
function drive() {
  clearTimeout(driving);
  const next = Math.min(...clock.timers.map(t => t.due));
  if (!isFinite(next)) return;
  driving = setTimeout(() => { advance(performance.now() - start); drive(); }, Math.max(0, next - (performance.now() - start)));
}

// ---------------------------------------------------------------- the data seam
/** The app's data sources: `answer(source, args)` returns `{v}` now, a
 * Promise for later, or `null` while the source is not ready; `ready(f)`
 * calls `f` once it is. */
export const data = { answer: () => null, ready: () => {} };
/** A resource. */
export function res(source, args, initial, initialArgs) {
  const bump = sig(0);
  const pending = sig(false), failed = sig(null);
  let value = initial, settled = initialArgs, ticket = 0, waiting = false;
  const m = memo(() => {
    bump();
    const a = args();
    if (settled !== undefined && eq(a, settled)) return value;
    const t = ++ticket;
    let r;
    try { r = data.answer(source, a); }
    catch (e) { journal.push(`resource ${source}: ${e.message}`); later(() => W(failed, String(e.message))); return value; }
    if (r && "v" in r) { settled = a; value = r.v; later(() => { W(pending, false); W(failed, null); }); return value; }
    if (r && r.then) {
      later(() => W(pending, true));
      r.then(v => { if (t === ticket) commit(() => { settled = a; value = v; W(pending, false); W(bump, bump.n.v + 1); }); },
        e => { if (t === ticket) commit(() => { W(failed, String(e?.message ?? e)); W(pending, false); }); });
      return value;
    }
    // Not ready: keep the value (stale), ask again when the source is.
    later(() => W(pending, true));
    if (!waiting) { waiting = true; data.ready(() => { waiting = false; commit(() => W(bump, bump.n.v + 1)); }); }
    return value;
  });
  m.p = pending; m.f = () => failed() != null;
  m.refresh = () => { settled = undefined; W(bump, bump.n.v + 1); };
  return m;
}
// Flag writes a settling resource makes: their own commit, after this one.
const Later = [];
function later(f) { if (Later.push(f) === 1) queueMicrotask(() => commit(() => { for (const g of Later.splice(0)) g(); })); }

// ---------------------------------------------------------------- the DOM
const SVG = "http://www.w3.org/2000/svg";
/** An element under `p`: its static class, attributes and text. */
export function h(p, tag, cls, attrs, text) {
  const e = /^(svg|g|path|circle|rect|line|polyline|polygon|ellipse)$/.test(tag) ? document.createElementNS(SVG, tag) : document.createElement(tag);
  if (cls !== 0) e.setAttribute("class", "c" + cls);
  if (attrs) for (const k in attrs) e.setAttribute(k, attrs[k]);
  if (text !== 0) e.textContent = text;
  p.append(e);
  return e;
}
/** A canvas: the host's surface element under its children (`glue.js`). */
export function cv(e) {
  const s = document.createElement("canvas");
  s.dataset.surface = "";
  s.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1";
  e.append(s);
}
const BOOL = /^(disabled|readonly|inert|checked|autoplay|controls|loop|muted|playsinline|disablepictureinpicture|disableremoteplayback)$/;
/** A dynamic prop, by the DOM name the live host uses (`applyProps`). */
export function P(e, name, f) {
  effect(() => {
    let v = f();
    v = v == null ? null : typeof v === "boolean" ? String(v) : String(v);
    if (name === "text") { if (!e.childElementCount && e.textContent !== (v ?? "")) e.textContent = v ?? ""; }
    else if (name === "value") { if (e.value !== (v ?? "")) e.value = v ?? ""; }
    else if (name === "paused") {
      if (v === "true") e.pause(); else e.play().catch(err => e.dispatchEvent(new CustomEvent("exact-error", { detail: err.message })));
    }
    else if (BOOL.test(name)) { e.toggleAttribute(name, v === "true"); if (name === "checked") e.checked = v === "true"; if (name === "muted") e.muted = v === "true"; }
    else if (v == null) e.removeAttribute(name);
    else if (e.getAttribute(name) !== v) e.setAttribute(name, v);
  });
}
/** A dynamic style row: `d` a dimension (a number is px), `n` a number, `s` as written. */
export function S(e, prop, kind, f) {
  effect(() => {
    const v = f();
    if (v == null) return e.style.removeProperty(prop);
    e.style.setProperty(prop, kind === "d" && typeof v === "number" ? v + "px" : String(v));
    // Invalid at computed-value time: the declaration is unset (LLP 1005 §6).
  });
}
/** An event handler: the DOM event the live host listens to (`glue.js` `attach`). */
export function on(e, kind, f) {
  const l = (t, g) => e.addEventListener(t, g);
  switch (kind) {
    case "press": return l("click", ev => { const a = ev.target.closest?.("a[href]"); if (a && a !== e && e.contains(a)) return; ev.stopPropagation(); f(); });
    case "change": return l("change", () => f(e.value));
    case "input": return l("input", () => f(e.value));
    case "hover": l("pointerenter", () => f(true)); return l("pointerleave", () => f(false));
    case "key": return l("keydown", ev => f(ev.key));
    case "submit": return l("keydown", ev => { if (ev.key === "Enter" && !ev.isComposing) { ev.preventDefault(); f(); } });
    case "message": return addEventListener("message", ev => { if (ev.source === e.contentWindow) f(typeof ev.data === "string" ? ev.data : JSON.stringify(ev.data)); });
    case "error": l("exact-error", ev => f(ev.detail)); return l("error", () => f(e.error?.message || "Media could not be loaded"));
    case "timeupdate": return l(kind, () => f(e.currentTime));
    case "durationchange": return l(kind, () => Number.isFinite(e.duration) && f(e.duration));
    case "contextmenu": case "dblclick": return l(kind, ev => { ev.preventDefault(); f(); });
    default: return l(kind, () => f());
  }
}
/** The page's `<head>` fields (LLP 1048.003 D1). */
export function hd(fields) {
  if (fields.headTitle) document.title = fields.headTitle;
  if (fields.headDescription) {
    let m = document.querySelector('meta[name="description"]');
    if (!m) { m = document.createElement("meta"); m.name = "description"; document.head.append(m); }
    m.content = fields.headDescription;
  }
}

// ---------------------------------------------------------------- regions
function range(p) {
  const a = document.createComment(""), b = document.createComment("");
  p.append(a, b);
  return [a, b];
}
function clear(a, b) { while (a.nextSibling !== b) a.nextSibling.remove(); }
function build(b, f) {
  const frag = document.createDocumentFragment();
  const s = scope(() => f(frag));
  b.before(frag);
  return s;
}
/** `when`: arm 0 while the subject holds, else arm 1 (or nothing). */
export function when(p, subject, a0, a1) {
  const [a, b] = range(p);
  let arm = -1, s = null;
  effect(() => {
    const want = subject() ? 0 : a1 ? 1 : -1;
    if (want === arm) return;
    arm = want;
    untracked(() => { if (s) dispose(s); clear(a, b); s = want < 0 ? null : build(b, want ? a1 : a0); });
  });
}
/** `match`: arm 0 with the bound value while the subject is `some`, else arm 1. */
export function match(p, subject, a0, a1) {
  const [a, b] = range(p);
  let arm = -1, s = null;
  const bound = sig(null);
  effect(() => {
    const v = subject(), want = v != null ? 0 : a1 ? 1 : -1;
    if (v != null) write(bound.n, v);
    if (want === arm) return;
    arm = want;
    untracked(() => { if (s) dispose(s); clear(a, b); s = want < 0 ? null : build(b, want ? a1 : p2 => a0(p2, bound)); });
  });
}
/** `each`: rows by key in item order; a kept row keeps its elements, its
 * item and position are signals its bindings read. */
export function each(p, list, key, row) {
  const [a, b] = range(p);
  let rows = new Map();
  effect(() => {
    const items = list();
    untracked(() => {
      const next = new Map(), seen = new Map();
      items.forEach((item, i) => {
        let k = key(() => item, () => i);
        k = typeof k + ":" + (Object.is(k, -0) ? 0 : k);
        const n = seen.get(k) ?? 0; seen.set(k, n + 1);
        if (n) { k = "d" + n + ":" + k; journal.push(`each: repeated key ${k}`); }
        let r = rows.get(k);
        if (r) { rows.delete(k); write(r.item.n, item); write(r.index.n, i); }
        else {
          r = { item: sig(item), index: sig(i), start: document.createComment(""), end: document.createComment("") };
          const frag = document.createDocumentFragment();
          frag.append(r.start);
          r.s = scope(() => row(frag, r.item, r.index));
          frag.append(r.end);
          r.frag = frag;
        }
        next.set(k, r);
      });
      for (const r of rows.values()) { dispose(r.s); let n = r.start; while (n) { const m = n.nextSibling; n.remove(); if (n === r.end) break; n = m; } }
      // Order: walk the rows, moving a row only when it is not already next.
      let at = a;
      for (const r of next.values()) {
        if (r.frag) { at.after(r.frag); r.frag = null; }
        else if (at.nextSibling !== r.start) {
          const f = document.createDocumentFragment();
          let n = r.start; while (n) { const m = n.nextSibling; f.append(n); if (n === r.end) break; n = m; }
          at.after(f);
        }
        at = r.end;
      }
      rows = next;
    });
  });
}

// ---------------------------------------------------------------- boot
/** Build the view into `#exact-root` and start the clock. */
export function mount(f) {
  const root = document.getElementById("exact-root");
  start = performance.now();
  clock.agent = new URLSearchParams(location.search).has("agent");
  commit(() => scope(() => f(root)));
  root.dataset.bootMs = String(Math.round(performance.now()));
}

// ---------------------------------------------------------------- the roster (runner/src/stdlib.rs)
export const x_now = () => clock.now;
export const x_length = v => v.length;
export const x_isEmpty = v => v.length === 0;
export const x_floor = Math.floor, x_max = Math.max, x_min = Math.min;
// Numbers print as JavaScript prints them (`push_number`), `-0` as `0`.
export const x_toString = v => String(v);
export const x_contains = (a, b) => a.includes(b);
export const x_trim = s => s.trim();
export const x_first = l => l.length ? l[0] : null;
export const x_join = (l, s) => l.map(String).join(s);
export const x_encodeURIComponent = encodeURIComponent;
/** `h:mm AM` of (epoch ms, UTC offset minutes east), U+0020 before the period. */
export function x_formatTime(ms, off) {
  const w = Math.trunc(ms) + off * 60000, m = Math.floor((((w % 864e5) + 864e5) % 864e5) / 6e4), h = m / 60 | 0;
  return `${h % 12 || 12}:${String(m % 60).padStart(2, "0")} ${h < 12 ? "AM" : "PM"}`;
}
export const x_formatCountdownMinutes = (at, now) => String(Math.max(0, Math.ceil((at - now) / 6e4)));
export const x_formatDistance = m => (m /= 1609.344) < 0.1 ? "nearby" : `${(Math.round(m * 10) / 10).toFixed(1)} mi`;
export const x_formatWalk = m => `${Math.max(1, Math.ceil(m / 80))} min walk`;
