import { renderMarkup, reportPlace } from "./navigation.js";
// the JS target's runtime: fine-grained signals over the DOM, for a
// plan compiled ahead of time by `exact-web-js`. Everything here is imported
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
let Listener = null, Owner = null, Queue = [], Flushing = false, Rev = 0;
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
  if (Owner) { (Owner.kids ??= []).push(n); n.up = Owner; }
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
function dispose(n) { n.gone = 1; drop(n); if (n.ends) for (const f of n.ends.splice(0)) f(); }
/** Run `f` when the scope that owns this code ends (a region's arm or row). */
function onEnd(f) { if (Owner) (Owner.ends ??= []).push(f); }
function run(n) {
  if (n.busy) throw new Refusal("a cycle: a derive or resource reads itself");
  drop(n);
  const [l, o] = [Listener, Owner];
  Listener = Owner = n;
  n.busy = 1;
  let v;
  try { v = n.fn(n.v); } finally { Listener = l; Owner = o; n.busy = 0; }
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
  n.v = v; Rev++;
  for (const o of n.obs) stale(o, DIRTY);
}
function flush() {
  if (Flushing) return;
  Flushing = true;
  try { for (let i = 0; i < Queue.length; i++) if (!Queue[i].gone) fresh(Queue[i]); }
  finally { Queue = []; Flushing = false; }
}
function untracked(f) { const l = Listener; Listener = null; try { return f(); } finally { Listener = l; } }

/** A slot: a getter, `.n` its node; `t` its declared type (writes conform). */
export function sig(v, t) { const n = node(null, v); n.t = t; const g = () => read(n); g.n = n; return g; }
const Settle = [];
/** A derive: lazy, cached, equal results keep their object; settled at
 * every commit before the tree; its value conforms to its type. */
export function memo(fn, t) {
  const n = node(t ? () => { const v = fn(); if (!conforms(v, t)) throw new Refusal("a derive's value does not conform to its type"); return v; } : fn);
  Settle.push(n);
  return () => read(n);
}
export function effect(fn) { const n = node(fn, undefined, 1); fresh(n); return n; }
/** A scope whose effects `dispose` ends, owned by `parent` (a region's
 * arms and rows belong to the region's scope, never to its effect, which
 * drops what it owns each time it reruns). */
function scope(f, parent = Owner) {
  const o = Owner; Owner = parent;
  const n = node(null);
  Owner = n;
  try { f(); } catch (e) { dispose(n); throw e; } finally { Owner = o; }
  return n;
}
function end(n) { dispose(n); const k = n.up?.kids; if (k) k.splice(k.indexOf(n), 1); }
// For loaded pieces (list.js): scopes, untracked reads, writes, the owner in
// force, and a count of what commits changed (an edge's no-op, runner/collection.rs).
export { scope, end, untracked, write, onEnd };
export const owner = () => Owner, rev = () => Rev, ticket = () => Ticket;

// ---------------------------------------------------------------- commits
/** A typed refusal: the commit rolls back (LLP 1005 §6 atomicity). */
export class Refusal extends Error {}
/** Whether `v` conforms to type code `t` (`n b s u ?T [T {T…}`), from `i`;
 * numbers are finite, as the runner's shape checks require. */
export function conforms(v, t, i = [0]) {
  const c = t[i[0]++];
  if (c === "n") return typeof v === "number" && isFinite(v);
  if (c === "b") return typeof v === "boolean";
  if (c === "s") return typeof v === "string";
  if (c === "u") return v == null;
  if (c === "?") { if (v == null) { skip(t, i); return true; } return conforms(v, t, i); }
  if (c === "[") { const at = i[0]; if (!Array.isArray(v)) return false; for (const x of v) { i[0] = at; if (!conforms(x, t, i)) return false; } i[0] = at; skip(t, i); return true; }
  if (c === "{") { let k = 0; for (; t[i[0]] !== "}"; k++) if (!Array.isArray(v) || !conforms(v[k], t, i)) return false; i[0]++; return v.length === k; }
  return true;
}
function skip(t, i) { const c = t[i[0]++]; if (c === "?" || c === "[") skip(t, i); else if (c === "{") { while (t[i[0]] !== "}") skip(t, i); i[0]++; } }

let Writes = null, Commands = [], Out = [], Landed = [], Sends = [], Refresh = [], Poisoned = false;
export const journal = [];
const say = line => journal.push(`t=${clock.now} ${line}`);
/** A write inside an action: collected, applied at commit. */
export function W(s, v) { Writes.push([s.n, v]); }
/** A host command inside an action: run after the commit. */
export function C(name, args) { Commands.push([name, args]); Rev++; }
/** `refresh r`: forced at this commit's settlement (merged, LLP 1054.000.000 D2). */
export function R(r) { Refresh.push(r.r ?? r); }
/** Pull every derive and resource in plan order: the settlement pass. */
function settle() { for (const n of Settle) fresh(n); }
/** One commit: `f` runs; its writes, sends and refreshes land; settlement
 * runs; a refusal anywhere up to here puts everything back and leaves the
 * tree untouched; then the tree updates, requests go out, commands run. */
export function commit(f, what = "commit") {
  if (Writes) return f();
  if (Poisoned) return say(`refused ${what}: the runner is poisoned; reload`);
  Writes = []; Commands = []; Out = []; Landed = []; Sends = []; Refresh = [];
  stamp();
  const undo = [], saved = Resources.map(r => r.save()), store = Store.save();
  let ok = true;
  try {
    untracked(f);
    for (const [n, v] of Writes) {
      if (n.t && !conforms(v, n.t)) throw new Refusal(`a write does not conform to its slot's type: ${JSON.stringify(v)}`);
      undo.push([n, n.v]); write(n, v);
      if (n.m && !n.landing) n.m.forget(undo);
    }
    for (const [m, source, args] of Sends) m.send(source, args, undo);
    for (const r of Refresh) r.force(undo);
    settle();
    // A store write re-asks the resources that read the store, until quiet.
    for (let k = 0; Store.dirty && k < 4; k++) { Store.dirty = false; for (const r of Resources) if (r.store) r.revise(undo); settle(); }
  } catch (e) {
    ok = false;
    for (const [n, v] of undo.reverse()) write(n, v);
    Resources.forEach((r, k) => r.restore(saved[k]));
    Store.restore(store);
    Out = []; Commands = []; Landed = [];
    say(`refused ${what}: ${e.message}`);
    if (!(e instanceof Refusal)) console.error(e);
    try { settle(); } catch {}
  }
  const [out, cmds, landed] = [Out, Commands, Landed];
  Writes = null;
  try { flush(); } catch (e) { Poisoned = true; say(`poisoned: ${e.message}`); console.error(e); return false; }
  settled();
  if (!ok) return false;
  Store.persist();
  for (const go of out) go();
  for (const c of cmds) command(...c);
  // An answer's `then` runs as its own commit, after this one stood.
  for (const m of landed) if (m.then) setTimeout(() => m.then());
  return true;
}
/** What runs after each commit's tree update (a loaded piece's publication). */
export const After = [];
/** Authored scroll offsets (`scrollTop`, `scrollLeft`), set once the
 * commit's tree is in place, as the web host's `pendingScrolls`; a
 * virtualized list builds the rows there first (`$jump`, list.js). */
const Scrolls = new Map();
/** What a commit does once its tree is in place: authored scrolls, then the
 * loaded pieces' publications (also after a list's report, list.js). */
export function settled() { drain(); for (const f of After) f(); }
function drain() {
  for (const [e, o] of Scrolls) for (const name in o) {
    const at = o[name];
    if (e.$jump) e.$jump(name, at);
    else if (e[name] !== at) { if (clock.agent && e.style.scrollBehavior === "smooth") e.scrollTo({ [name === "scrollTop" ? "top" : "left"]: at, behavior: "instant" }); else e[name] = at; }
  }
  Scrolls.clear();
}
/** An action: each call is one commit. */
export function act(fn) { return (...a) => commit(() => fn(...a), "action"); }
const Hosts = {
  focus: id => document.getElementById(id)?.focus(),
  blur: id => document.getElementById(id)?.blur(),
  setScheme: s => { document.documentElement.style.colorScheme = s === "system" ? "" : s; },
  copyText: t => navigator.clipboard?.writeText(t),
};
function command(name, args) {
  const f = Hosts[name];
  say(`command ${name}`);
  if (f) f(...args); else say(`refused: ${name} is not a command this runtime carries`);
}

// ---------------------------------------------------------------- the clock and timers
export const clock = { now: 0, timers: [], agent: false };
// `now()` is elapsed time: the driver's clock under the agent and in a
// render, else the page's since it started; a timer commits at its due time.
// A reader of `now()` is re-evaluated at each commit made at a later time,
// as the runner marks the clock read dirty (instance/deps.rs), and never by
// the clock moving alone. Not a write: no commit counts it as a change.
const Now = node(null, 0);
let Timing = false;
function stamp() {
  if (!clock.agent && !Timing && start) clock.now = Math.max(clock.now, performance.now() - start);
  if (Now.v !== clock.now) { Now.v = clock.now; for (const o of Now.obs) stale(o, DIRTY); }
}
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
    Timing = true;
    try { next.action(); } finally { Timing = false; }
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
/** The app's data sources (LLP 1016). `answer(source, args, store)` gives
 * `{v}` now, `{req}` (an HTTP request the host runs, then `parse`s),
 * a Promise (an executor-local continuation: TypeScript), or `null` while
 * the source is not ready; `ready(f)` calls `f` once it is. */
export const data = { answer: () => null, parse: null, q: [], ready: f => data.q.push(f) };
/** The durable store (LLP 1018): name → text, persisted as the web host
 * does (`localStorage` "exact.secret.<name>") after a commit stands. */
export const Store = {
  map: new Map(), writes: [], dirty: false,
  get(k) { return this.map.get(k); },
  set(k, v) { if (v == null) this.map.delete(k); else this.map.set(k, v); this.writes.push([k, v]); this.dirty = true; Rev++; },
  save() { return [new Map(this.map), this.writes.length]; },
  restore([m, n]) { this.map = m; this.writes.length = n; this.dirty = false; },
  persist() { for (const [k, v] of this.writes.splice(0)) try { v == null ? localStorage.removeItem("exact.secret." + k) : localStorage.setItem("exact.secret." + k, v); } catch {} },
  load() { try { for (let i = 0; i < localStorage.length; i++) { const k = localStorage.key(i); if (k.startsWith("exact.secret.")) this.map.set(k.slice(13), localStorage.getItem(k)); } } catch {} },
};
/** Every resource, in plan order: the checkpoint a render writes reads them. */
export const Resources = [];
let Ticket = 0;
const sameReq = (a, b) => a && b && a.method === b.method && a.url === b.url && a.body === b.body && JSON.stringify(a.headers) === JSON.stringify(b.headers) && a.http === b.http;
/** Run a request after the commit publishes; `land(outcome)` on reply. */
/** Requests in flight, for the agent's `clock settle`. */
export const inflight = { n: 0 };
function send(t, land) {
  Out.push(() => {
    inflight.n++;
    const done = o => { inflight.n--; land(o); };
    if (t.req) data.fetch(t.req).then(done, e => done({ failed: 1, message: String(e?.message ?? e) }));
    else t.promise.then(v => done({ v }), e => done({ error: String(e?.message ?? e) }));
  });
}
function ask(source, args, name) {
  const a = data.reserved?.[source] ? { v: data.reserved[source](source, args, name) } : data.answer(source, args, Store, name);
  if (a && a.then) { const p = a; return { promise: p }; }
  return a;
}
/** A resource: its value, the arguments it settled with, one ticket in flight. */
export function res(name, source, args, initial, initialArgs, type, ph) {
  const ver = sig(0), pend = sig(false), fail = sig(null);
  const kept = checkpoint().kept?.get(name);
  if (kept) [initialArgs, initial] = kept;
  const r = { name, source, type, value: initial, settled: initialArgs, ticket: null, forced: false, reread: false, rev: false, store: false };
  const flag = (s, v, undo) => { if (!eq(s.n.v, v)) { undo?.push([s.n, s.n.v]); write(s.n, v); } };
  // Nothing kept: the placeholder shows, pending (LLP 1048.003 D6).
  const hold = () => {
    if (r.value !== undefined) return;
    const v = typeof ph === "function" ? ph() : ph;
    if (v === undefined) throw new Refusal(`${name} answers later and has nothing to show; give it an \`else\``);
    r.value = v;
  };
  const take = (v, a) => {
    if (type && !conforms(v, type)) throw new Refusal(`${name}: the answer does not conform to its shape`);
    r.value = v; r.settled = a;
  };
  const land = t => outcome => commit(() => {
    if (r.ticket !== t) return say(`dropped reply for ${name}: ticket ${t.id} is no longer held`);
    const p = outcome.v !== undefined ? { v: outcome.v } : outcome.error ? (() => { throw new Refusal(outcome.error); })() : data.parse(source, t.args, outcome, Store);
    if (p.req) { t.req = p.req; t.id = ++Ticket; send(t, land(t)); return; }
    take(p.v, t.args); r.ticket = null;
    W(pend, false); W(fail, null); W(ver, ver.n.v + 1);
  }, `reply ${name}`);
  const m = memo(() => {
    ver();
    const a = args();
    const forced = r.forced, reread = r.reread, rev = r.rev;
    r.forced = r.reread = r.rev = false;
    if (!forced && !reread && !rev) {
      if (r.settled !== undefined && eq(a, r.settled)) return r.value;
      if (r.ticket && eq(a, r.ticket.args)) return r.value;
    }
    let ans;
    try { ans = ask(source, a, name); }
    catch (e) { if (e instanceof Refusal) throw e; flag(fail, String(e.message)); say(`resource ${name}: ${e.message}`); return r.value; }
    if (ans && ans.store) r.store = true;
    if (ans && "v" in ans) {
      take(ans.v, a);
      if (r.ticket && !reread) { say(`forget ticket ${r.ticket.id} (${name})`); r.ticket = null; }
      if (!r.ticket) flag(pend, false);
      flag(fail, null);
      return r.value;
    }
    // A declared refresh at a send re-reads: a request is discarded, and one in flight stays.
    if (reread) return r.value;
    if (ans && ans.req && !forced && !rev && r.ticket?.req && !eq(a, r.ticket.args) && sameReq(ans.req, r.ticket.req)) {
      say(`keep request ${r.ticket.id} (${name}): the same request for newer arguments`);
      r.ticket.args = a;
      return r.value;
    }
    if (ans && (ans.req || ans.promise)) {
      hold();
      const t = { id: ++Ticket, args: a, req: ans.req, promise: ans.promise };
      if (r.ticket) say(`forget ticket ${r.ticket.id} (${name})`);
      r.ticket = t; flag(pend, true); send(t, land(t));
      return r.value;
    }
    // The source is not ready: the compiled value stands, stale, and the
    // resource is asked again, forced, when it is (LLP 1027 D4).
    hold();
    flag(pend, true);
    if (!r.waiting) { r.waiting = true; data.ready(() => { r.waiting = false; commit(() => { r.forced = true; W(ver, ver.n.v + 1); W(pend, false); }, `data ready ${name}`); }); }
    return r.value;
  }, type);
  Object.assign(r, {
    save: () => [r.value, r.settled, r.ticket, r.ticket?.args, r.store],
    restore: x => { [r.value, r.settled, r.ticket] = x; if (r.ticket) r.ticket.args = x[3]; r.store = x[4]; },
    force: undo => { r.forced = true; flag(ver, ver.n.v + 1, undo); },
    reread_: undo => { r.reread = true; flag(ver, ver.n.v + 1, undo); },
    revise: undo => { r.rev = true; flag(ver, ver.n.v + 1, undo); },
  });
  Resources.push(r);
  m.p = () => (m(), pend());
  m.f = () => (m(), fail() != null);
  m.r = r;
  return m;
}
/** A mutation (LLP 1016): its slot (`option<T>`), the resources it
 * declares it refreshes, and one ticket per send, the newest winning. */
export const Mutations = [];
export function mut(name, slot, refreshes, type) {
  const pend = sig(false);
  const m = { ticket: null, then: null };
  Mutations.push(m);
  slot.n.m = m;
  const landWrite = (v, undo) => { slot.n.landing = 1; try { undo.push([slot.n, slot.n.v]); write(slot.n, v); } finally { slot.n.landing = 0; } Landed.push(m); };
  const land = t => outcome => commit(() => {
    if (m.ticket !== t) return say(`dropped reply for ${name}: ticket ${t.id} is no longer held`);
    const p = outcome.v !== undefined ? { v: outcome.v } : outcome.error ? (() => { throw new Refusal(outcome.error); })() : data.parse(t.source, t.args, outcome, Store);
    if (p.req) { t.req = p.req; send(t, land(t)); return; }
    if (type && !conforms(p.v, type)) throw new Refusal(`${name}: the answer does not conform to its shape`);
    m.ticket = null; W(pend, false);
    slot.n.landing = 1; W(slot, p.v); Landed.push(m);
    // At the reply, the declared refreshes are forced (LLP 1054.000.000 D1).
    for (const r of refreshes) R(r.r);
    queueMicrotask(() => { slot.n.landing = 0; });
  }, `reply ${name}`);
  Object.assign(m, {
    forget(undo) { if (m.ticket) { say(`forget ticket ${m.ticket.id} (${name})`); m.ticket = null; undo.push([pend.n, pend.n.v]); write(pend.n, false); } },
    send(source, args, undo) {
      const a = ask(source, args);
      if (a && "v" in a) {
        if (type && !conforms(a.v, type)) throw new Refusal(`${name}: the answer does not conform to its shape`);
        landWrite(a.v, undo);
      } else if (a && (a.req || a.promise)) {
        const t = { id: ++Ticket, source, args, req: a.req, promise: a.promise };
        m.ticket = t; undo.push([pend.n, pend.n.v]); write(pend.n, true); send(t, land(t));
      } else throw new Refusal(`${name}: its source is not ready`);
      // At the send, the declared refreshes re-read (D1).
      for (const r of refreshes) r.r.reread_(undo);
    },
  });
  m.p = () => pend();
  return m;
}
/** `send m = source(args)` inside an action: asked at commit, after its writes. */
export function M(m, source, args) { Sends.push([m, source, args]); }

// ---------------------------------------------------------------- the DOM
const SVG = "http://www.w3.org/2000/svg";
/** An element under `p`: its static class, attributes and text. */
export function h(p, tag, cls, attrs, text, ns) {
  if (Adopt) return adopt(p, tag, cls, attrs);
  const e = ns ? document.createElementNS(ns, tag) : document.createElement(tag);
  if (cls !== 0) e.setAttribute("class", "c" + cls);
  if (attrs) for (const k in attrs) e.setAttribute(k, attrs[k]);
  if (text !== 0) e.textContent = text;
  p.append(e);
  return e;
}
/** An SVG element (the compiler knows the node's type; element.rs's tag). */
export const hs = (p, tag, cls, attrs, text) => h(p, tag, cls, attrs, text, SVG);
/** A canvas: the host's surface element under its children (`glue.js`). */
export function cv(e) {
  if (Adopt) { const s = at(e); if (s?.dataset?.surface !== undefined) { e.$n = s.nextSibling; return; } }
  const s = document.createElement("canvas");
  s.dataset.surface = "";
  s.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1";
  if (Adopt) e.insertBefore(s, at(e)); else e.append(s);
}

// ---------------------------------------------------------------- adoption (LLP 1048.000 D6)
// A page rendered ahead (by the Rust render host or by this runtime under
// Bun) is adopted, not rebuilt: construction walks the document with a
// cursor per parent, takes each element whose tag matches, gives it the
// class it would have had and drops the renderer's inline style and view
// ids, and inserts the region anchors a fresh build would have made. A
// mismatch abandons adoption and builds afresh.
let Adopt = false;
class Mismatch extends Refusal {}
// The next node of the document to adopt under `p`; what adoption inserts
// goes before it and never moves it.
const at = p => (p.$n === undefined ? (p.$n = p.firstChild) : p.$n);
function adopt(p, tag, cls, attrs) {
  let e = at(p);
  while (e && e.nodeType !== 1) e = e.nextSibling;
  if (!e || e.localName.toLowerCase() !== tag.toLowerCase()) throw new Mismatch(`adoption: expected <${tag}>, found ${e ? "<" + e.localName + ">" : "nothing"}`);
  p.$n = e.nextSibling;
  e.removeAttribute("style"); e.removeAttribute("data-view");
  if (cls !== 0) e.setAttribute("class", "c" + cls);
  if (attrs) for (const k in attrs) if (e.getAttribute(k) !== attrs[k]) e.setAttribute(k, attrs[k]);
  return e;
}
function mark(p) { const c = document.createComment(""); p.insertBefore(c, at(p)); return c; }
/** Build fresh inside an adopted page (a virtualized list's rows). */
export function unadopted(f) { const a = Adopt; Adopt = false; try { return f(); } finally { Adopt = a; } }

const BOOL = /^(disabled|readonly|inert|checked|autoplay|controls|loop|muted|playsinline|disablepictureinpicture|disableremoteplayback)$/;
/** A loaded piece's own handling of a prop (symbols.js's `src`): true when handled. */
export const PropHooks = {};
/** A dynamic prop, by the DOM name the live host uses (`applyProps`). */
export function P(e, name, f) {
  effect(() => {
    let v = f();
    v = v == null ? null : typeof v === "boolean" ? String(v) : String(v);
    if (PropHooks[name]?.(e, v)) return;
    if (name === "text") { if (!e.childElementCount && e.textContent !== (v ?? "")) e.textContent = v ?? ""; }
    else if (name === "value") { if (e.value !== (v ?? "")) e.value = v ?? ""; }
    else if (name === "scrollTop" || name === "scrollLeft") { if (v != null) (Scrolls.get(e) ?? Scrolls.set(e, {}).get(e))[name] = Number(v); }
    else if (name === "paused") {
      if (v === "true") e.pause(); else e.play().catch(err => e.dispatchEvent(new CustomEvent("exact-error", { detail: err.message })));
    }
    else if (BOOL.test(name)) { e.toggleAttribute(name, v === "true"); if (name === "checked") e.checked = v === "true"; if (name === "muted") e.muted = v === "true"; }
    else if (v == null) e.removeAttribute(name);
    else if (e.getAttribute(name) !== v) e.setAttribute(name, v);
  });
}
/** A `markup="markdown"` text (LLP 1045 D3): its source as pieces, built
 * into spans by the web host's own `renderMarkup`; the pieces come from the
 * web host's Markdown, a wasm fetched at the first such node (a loaded
 * capability). A rendered page's spans stand until the source changes. */
let Markdown = null;
export function md(e, f) {
  let first = Adopt && e.childElementCount > 0;
  effect(() => {
    const v = f() ?? "";
    e.$source = v;
    if (first) { first = false; return; }
    if (!Markdown) e.textContent = v;
    inflight.n++;
    (Markdown ??= markdown()).then(pieces => { if (f() === v) renderMarkup(e, pieces(v)); }).finally(() => inflight.n--);
  });
}
async function markdown() {
  const bytes = globalThis.__files ? globalThis.__files("markdown.wasm") : await fetch("./markdown.wasm").then(r => r.arrayBuffer());
  const made = await WebAssembly.instantiate(bytes, {}), x = (made.instance ?? made).exports;
  return source => {
    const b = new TextEncoder().encode(source), p = x.alloc(b.length);
    new Uint8Array(x.memory.buffer, p, b.length).set(b);
    const n = x.pieces(p, b.length);
    return new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.output(), n));
  };
}
/** Views by id, one id space for the agent and the GPU module (the web
 * host's `exact.views`): an element gets an id when either first asks. */
export const Views = new Map();
const Ids = new WeakMap();
let NextView = 1;
export function viewId(e) { let i = Ids.get(e); if (!i) { i = NextView++; Ids.set(e, i); } Views.set(i, e); return i; }
/** A canvas's surface (LLP 1009 D2): its inputs, evaluated as the runner
 * evaluates them (records as arrays), to the app's GPU module — the web
 * host's own `gpu-glue.js` over `gpu.js`, fetched after the first painted
 * frame, only when a canvas is on the page (a loaded capability). */
let Gpu = null;
export function gs(e, name, values) {
  const id = viewId(e);
  onEnd(() => { try { globalThis.exact?.gpu?.destroy(id); } catch (err) { say(`gpu: ${err.message}`); } });
  effect(() => {
    const v = values();
    const x = globalThis.exact ??= {};
    // Published after the commit applied, as the runner publishes surface
    // inputs: the canvas is in the document by then.
    if (x.gpu) return queueMicrotask(() => { try { e.isConnected && x.gpu.surface(id, name, v); } catch (err) { say(`gpu: ${err.message}`); } });
    const pending = x.pendingSurfaces ??= [], queued = pending.find(p => p.id === id);
    if (queued) queued.values = v; else pending.push({ id, name, values: v, generation: 0 });
    if (!Gpu && typeof requestAnimationFrame === "function" && !globalThis.__exactRender) {
      x.views = Views; x.generation = 0; x.devAssets = null; x.root = document.getElementById("exact-root");
      // A surface's published record (LLP 1009 D6), as the glue hands it to
      // the wasm host: `name` or `name\0json`, to `exactSurface` readers
      // (facts.js); dropped where no resource reads one.
      let written = "";
      x.writeIn ??= t => { written = t; return 0; };
      (x.wasm ??= {}).exact_surface_record ??= () => { const at = written.indexOf("\0"); x.surfaceRecord?.(at < 0 ? written : written.slice(0, at), at < 0 ? null : written.slice(at + 1)); return 0; };
      x.send ??= () => {};
      if (clock.agent) x.now = () => clock.now;
      inflight.n++;
      Gpu = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => import(new URL("gpu-glue.js", document.baseURI).href)).then(() => x.gpu?.settled?.()).catch(err => say(`gpu: ${err.message}`)).finally(() => inflight.n--);
    }
  });
}
/** A Canvas 2D surface (LLP 1056): its arguments, drawn by the data
 * module and replayed by the web host's own glue, both in `canvas2d.js`,
 * fetched two frames after the first 2D canvas mounts (a loaded
 * capability); `types` are the arguments' declared types where known. */
let Canvas2d = null;
export function c2(e, name, values, types, names) {
  const c = { e, name, types, names, mounted: clock.now, args: null };
  onEnd(() => Canvas2d?.then(m => m?.gone(c)));
  effect(() => {
    c.args = values();
    if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
    if (!Canvas2d) {
      inflight.n++;
      Canvas2d = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => import("./canvas2d.js"))
        .then(m => m.engine({ data, clock, inflight, journal, advance, views: Views, viewId, wall: () => performance.now() - start }))
        .catch(err => say(`canvas2d: ${err.message}`)).finally(() => inflight.n--);
    }
    // After the commit applied, as the runner publishes surface inputs.
    queueMicrotask(() => Canvas2d.then(m => m?.args(c)));
  });
}
/** A dynamic style row: a number takes the unit css.rs gives the row. */
export function S(e, prop, unit, f) {
  effect(() => {
    const v = f();
    if (v == null) return e.style.removeProperty(prop);
    // A value the row refuses is invalid at computed-value time: unset,
    // never the earlier declaration (LLP 1005 §6).
    e.style.removeProperty(prop);
    e.style.setProperty(prop, typeof v === "number" ? v + unit : String(v));
    if (!e.style.getPropertyValue(prop)) say(`unset ${prop}: ${JSON.stringify(v)} is not a value it takes`);
  });
}
/** An event handler: the DOM event the live host listens to (`glue.js` `attach`). */
export function on(e, kind, f) {
  const l = (t, g) => e.addEventListener(t, g);
  switch (kind) {
    // A link with a press is the app's navigation: the browser's is prevented.
    case "press": return l("click", ev => { const a = ev.target.closest?.("a[href]"); if (a && a !== e && e.contains(a)) return; ev.stopPropagation(); if (e.localName === "a" && !(ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.button)) ev.preventDefault(); f(); });
    case "change": return l("change", () => f(e.value));
    case "input": return l("input", () => f(e.value));
    case "hover": l("pointerenter", () => f(true)); return l("pointerleave", () => f(false));
    case "key": return l("keydown", ev => f(ev.key));
    case "submit": return l("keydown", ev => { if (ev.key === "Enter" && !ev.isComposing) { ev.preventDefault(); f(); } });
    case "message": return addEventListener("message", ev => { if (ev.source === e.contentWindow) f(typeof ev.data === "string" ? ev.data : JSON.stringify(ev.data)); });
    case "error": l("exact-error", ev => f(ev.detail)); return l("error", () => f(e.error?.message || "Media could not be loaded"));
    case "timeupdate": return l(kind, () => f(e.currentTime));
    // The port's offsets, as the web host sends them (`glue.js` `attach`).
    case "scroll": return l(kind, () => f(e.scrollLeft, e.scrollTop));
    // Pull to refresh is a native port's; the web has none (`glue.js` attaches nothing).
    case "refresh": return;
    case "durationchange": return l(kind, () => Number.isFinite(e.duration) && f(e.duration));
    case "contextmenu": case "dblclick": return l(kind, ev => { ev.preventDefault(); f(); });
    default: return l(kind, () => f());
  }
}
/** The page's `<head>` fields (LLP 1048.003 D1); a field bound to state
 * follows it while its head is in the tree. */
export function hd(p, fields) {
  // The head's node, as the kernel keeps it: an inert element in the tree.
  const t = document.createElement("template");
  if (Adopt) p.insertBefore(t, at(p)); else p.append(t);
  for (const [k, v] of Object.entries(fields)) effect(() => head(k, typeof v === "function" ? v() : v));
}
/** The head's fields as last set, for a renderer. */
export const Head = {};
function head(k, v) {
  Head[k] = v;
  if (k === "headTitle") document.title = v;
  else if (k === "headDescription") {
    let m = document.querySelector('meta[name="description"]');
    if (!m) { m = document.createElement("meta"); m.name = "description"; document.head.append(m); }
    m.content = v;
  }
}

// ---------------------------------------------------------------- regions
function range(p) {
  if (Adopt) return [mark(p), null];
  const a = document.createComment(""), b = document.createComment("");
  p.append(a, b);
  return [a, b];
}
function clear(a, b) { while (a.nextSibling !== b) a.nextSibling.remove(); }
function build(b, f, own) {
  const frag = document.createDocumentFragment();
  const s = scope(() => f(frag), own);
  b.before(frag);
  return s;
}
/** A region's first arm while adopting: built in place, then its end anchor. */
function adoptArm(p, f, own) { const s = f ? scope(() => f(p), own) : null; return [s, mark(p)]; }
/** `when`: arm 0 while the subject holds, else arm 1 (or nothing). */
export function when(p, subject, a0, a1) {
  let [a, b] = range(p), own = Owner;
  let arm = -1, s = null;
  effect(() => {
    const want = subject() ? 0 : a1 ? 1 : -1;
    if (want === arm) return;
    arm = want;
    if (!b) return untracked(() => { [s, b] = adoptArm(p, want < 0 ? null : want ? a1 : a0, own); });
    untracked(() => { if (s) end(s); clear(a, b); s = want < 0 ? null : build(b, want ? a1 : a0, own); });
  });
}
/** `match`: arm 0 with the bound value while the subject is `some`, else arm 1. */
export function match(p, subject, a0, a1) {
  let [a, b] = range(p), own = Owner;
  let arm = -1, s = null;
  const bound = sig(null);
  effect(() => {
    const v = subject(), want = v != null ? 0 : a1 ? 1 : -1;
    if (v != null) write(bound.n, v);
    if (want === arm) return;
    arm = want;
    if (!b) return untracked(() => { [s, b] = adoptArm(p, want < 0 ? null : want ? a1 : p2 => a0(p2, bound), own); });
    untracked(() => { if (s) end(s); clear(a, b); s = want < 0 ? null : build(b, want ? a1 : p2 => a0(p2, bound), own); });
  });
}
/** `each`: rows by key in item order; a kept row keeps its elements, its
 * item and position are signals its bindings read. */
export function each(p, list, key, row) {
  let [a, b] = range(p), own = Owner;
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
        else if (!b) {
          // Adopting: the row's elements are in place, in item order.
          r = { item: sig(item), index: sig(i), start: mark(p) };
          r.s = scope(() => row(p, r.item, r.index), own);
          r.end = mark(p);
        }
        else {
          r = { item: sig(item), index: sig(i), start: document.createComment(""), end: document.createComment("") };
          const frag = document.createDocumentFragment();
          frag.append(r.start);
          r.s = scope(() => row(frag, r.item, r.index), own);
          frag.append(r.end);
          r.frag = frag;
        }
        next.set(k, r);
      });
      for (const r of rows.values()) { end(r.s); let n = r.start; while (n) { const m = n.nextSibling; n.remove(); if (n === r.end) break; n = m; } }
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
      b ??= mark(p);
    });
  });
}

// ---------------------------------------------------------------- boot
/** Build the view into `#exact-root` and start the clock. */
export function mount(f) {
  const root = document.getElementById("exact-root");
  // Under the agent, and in a render, the clock is the driver's: no timer runs by itself.
  clock.agent = !!globalThis.__exactRender || new URLSearchParams(location.search).has("agent");
  Store.load();
  let built = false;
  Adopt = !!(checkpoint().kept && root.firstElementChild);
  // Elapsed time continues from where a render's clock stopped.
  start = performance.now() - clock.now;
  const adopting = Adopt;
  commit(() => { scope(() => f(root)); built = true; }, adopting ? "adopt" : "boot");
  Adopt = false;
  const adopted = adopting && built;
  if (!built && adopting) {
    // The document isn't this plan's projection: build afresh (and say so).
    say(`adoption abandoned: ${journal.at(-1)}`);
    root.textContent = "";
    commit(() => { scope(() => f(root)); built = true; }, "boot");
  }
  if (!built) throw new Error("boot refused: " + journal.at(-1));
  if (adopted) say("adopted the document");
  say(`boot: ${root.getElementsByTagName("*").length} nodes`); // the runner's journal line (LLP 1012 logs)
  root.dataset.bootMs = String(Math.round(performance.now()));
  // What the reader did before the runtime ran (the capture script),
  // replayed once, in order, on the same elements (LLP 1048.001 D5).
  for (const t of globalThis.exact?.taps?.() ?? []) if (t.target.isConnected) t.type === "input" ? t.target.dispatchEvent(new Event("input", { bubbles: true })) : t.target.click();
}

/** The page's checkpoint (LLP 1048.000 D4): the answers its document used,
 * by resource name, so a resource admits the rendered value while its
 * arguments match, and the time the render stopped at. */
let Checkpoint = null;
export function checkpoint() {
  if (Checkpoint) return Checkpoint;
  Checkpoint = { kept: null };
  const el = typeof document !== "undefined" && document.querySelector('script[type="application/vnd.exact.checkpoint"]');
  if (!el) return Checkpoint;
  const cp = JSON.parse(el.textContent);
  Checkpoint.kept = new Map(cp.answers.map(([name, , args, value]) => [name, [value_(args), value_(value)]]));
  Checkpoint.time = cp.time;
  clock.now = cp.time || 0;
  return Checkpoint;
}
/** A checkpoint value (`push_value`, host/web/src/page.rs) as a runtime value:
 * lists and records are arrays, unit and `none` null, `some(v)` v. */
const value_ = v => v === null || typeof v !== "object" ? v : Array.isArray(v) ? v.map(value_) : "r" in v ? v.r.map(value_) : "s" in v ? value_(v.s) : "n" in v ? Number(v.n) : null;

// ---------------------------------------------------------------- the roster (runner/src/stdlib.rs)
export const x_now = () => read(Now);
export const x_length = v => v.length;
export const x_isEmpty = v => v.length === 0;
export const x_floor = Math.floor, x_max = Math.max, x_min = Math.min;
// Numbers print as JavaScript prints them (`push_number`), `-0` as `0`.
export const x_toString = v => String(v);
export const x_includes = (a, b) => a.includes(b), x_startsWith = (a, b) => a.startsWith(b), x_endsWith = (a, b) => a.endsWith(b);
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

// ---------------------------------------------------------------- localized strings (LLP 1060)
// The plan's tables, base first: [name, rtl, {key: text}]. The locale slot
// starts at the base, and after boot holds the table the viewer's locale
// reads, as the runner's `set_place` writes it; `t` reads that table, else
// the base.
let Texts = [];
export function strings(tables) { Texts = tables; }
/** RFC 4647 lookup of the page's locale (an agent's `?locale`): the longest
 * subtag prefix a table is named for, without case, else the base. */
function locale() {
  let tag = "en-US";
  try { tag = reportPlace().split("\0")[0]; } catch {}
  for (;;) {
    const t = Texts.find(r => r[0].toLowerCase() === tag.toLowerCase());
    if (t) return t[0];
    const i = tag.lastIndexOf("-");
    if (i < 0) return Texts[0][0];
    tag = tag.slice(0, i);
  }
}
const table = name => Texts.find(r => r[0] === name);
/** `t(key, name=value…)`: MF2 simple messages, `{name}` or `{$name}`; an
 * unfilled name keeps its spelling; `\{ \} \\` escape. */
export function x_t(name, key, pairs) {
  const text = table(name)?.[2][key] ?? Texts[0][2][key];
  if (text == null) throw new Refusal(`t: no text ${key}`);
  const at = n => { for (let i = 0; i < pairs.length; i += 2) if (pairs[i] === n) return pairs[i + 1]; };
  return text.replace(/\\([{}\\])|\{\s*\$?([A-Za-z_][\w-]*)\s*\}/g, (m, e, n) => e ?? at(n) ?? m);
}
/** The resolved table sets the document's `lang` and `dir` (LLP 1060, ruled). */
let LocaleSlot = null;
/** `exactTime.resolvedLocale`: the table the strings read, "" with none (runner/src/runner/time.rs). */
export const resolvedLocale = () => LocaleSlot ? (table(LocaleSlot()) ?? Texts[0])[0] : "";
export function language(slot) {
  LocaleSlot = slot;
  // As the host's first `set_place`: the table the viewer's locale reads,
  // written to the slot, and `exactTime` answered again, in one commit.
  commit(() => { W(slot, locale()); for (const r of Resources) if (r.source === "exactTime") R(r); }, "place");
  if (typeof document !== "object" || !document.documentElement) return;
  effect(() => { const t = table(slot()) ?? Texts[0]; document.documentElement.lang = t[0]; document.documentElement.dir = t[1] ? "rtl" : "ltr"; });
}

// ---------------------------------------------------------------- the router (LLP 1038; route/src)
// A Router is [tab, tabs, next]; a Tab [name, stack]; an Entry
// [id, name, url, tab, params], params positional in the table's
// first-declaration order of distinct `:names`.
let Routes = [], Names = [];
const names = p => p.split("/").filter(s => s[0] === ":").map(s => s.slice(1));
/** The plan's route table: [name, pattern, parent, tab, notfound] rows. */
export function routes(table) {
  Routes = table.map(([name, pattern, parent, tab, notfound]) => ({ name, pattern, parent, tab, notfound }));
  Names = [];
  for (const r of Routes) if (!r.notfound) for (const n of names(r.pattern)) if (!Names.includes(n)) Names.push(n);
}
const HEX = "0123456789ABCDEF", utf8 = new TextEncoder();
const enc = (s, esc) => { let o = ""; for (const b of utf8.encode(s)) o += esc(b) ? "%" + HEX[b >> 4] + HEX[b & 15] : String.fromCharCode(b); return o; };
const dec = (s, plus) => { const out = []; for (let i = 0; i < s.length; i++) { const c = s.charCodeAt(i); if (c === 37 && /^[0-9a-f]{2}$/i.test(s.substr(i + 1, 2))) { out.push(parseInt(s.substr(i + 1, 2), 16)); i += 2; } else if (plus && c === 43) out.push(32); else out.push(...utf8.encode(s[i])); } return new TextDecoder().decode(new Uint8Array(out)); };
const clean = s => s.replace(/^[\0- ]+|[\0- ]+$/g, "").replace(/[\t\n\r]/g, "");
/** `canonical` (route/src/location.rs): path and query, dot segments resolved, escaped. */
export function canonical(location) {
  let input = clean(location[0] === "/" ? location : "/" + location).split("#")[0];
  let [path, ...q] = input.split("?"); const query = q.join("?");
  path = path.replace(/\\/g, "/").replace(/^\//, "");
  const segs = [], parts = path.split("/");
  parts.forEach((seg, i) => {
    const d = seg.toLowerCase(), last = i === parts.length - 1;
    if (d === "." || d === "%2e") { if (last) segs.push(""); }
    else if (["..", ".%2e", "%2e.", "%2e%2e"].includes(d)) { segs.pop(); if (last) segs.push(""); }
    else segs.push(enc(seg, b => b < 0x21 || b > 0x7e || '"#<>?^`{}|'.includes(String.fromCharCode(b))));
  });
  return "/" + segs.join("/") + (query ? "?" + enc(query, b => b < 0x21 || b > 0x7e || "\"#<>'".includes(String.fromCharCode(b))) : "");
}
const empty = () => Names.map(() => "");
const segments = p => p === "/" ? [] : p.replace(/^\//, "").split("/");
function matchRoute(url) {
  const path = url.split("?")[0], parts = segments(path);
  if (path === "/" || !path.endsWith("/")) for (let i = 0; i < Routes.length; i++) {
    const r = Routes[i]; if (r.notfound) continue;
    const pat = segments(canonical(r.pattern)); if (pat.length !== parts.length) continue;
    const params = empty();
    if (pat.every((s, k) => s[0] === ":" ? parts[k] !== "" && (params[Names.indexOf(s.slice(1))] = dec(parts[k], false), true) : s === parts[k])) return [i, params];
  }
  const nf = Routes.findIndex(r => r.notfound);
  return nf < 0 ? null : [nf, empty()];
}
const roots = () => { const r = Routes.map((x, i) => x.tab ? i : -1).filter(i => i >= 0); return r.length || !Routes.length ? r : [0]; };
function rootFor(i) {
  const rs = roots();
  if (!Routes[i].notfound) for (let c = i, k = 0; c != null && c >= 0 && k <= Routes.length; c = Routes[c].parent, k++) if (rs.includes(c)) return c;
  return rs[0];
}
const segmentOf = v => { if (["", ".", ".."].includes(v)) throw new Refusal("a path parameter cannot be empty, `.` or `..`"); return enc(v, b => !/[A-Za-z0-9\-_.!~*'()]/.test(String.fromCharCode(b))); };
const path = (r, values) => r.pattern.split("/").map(s => s[0] === ":" ? segmentOf(values.shift() ?? "") : s).join("/");
function chain(location) {
  const url = canonical(location), m = matchRoute(url);
  if (!m) return [];
  const [index, params] = m, root = rootFor(index);
  if (root == null) return [];
  const idx = [index];
  if (!Routes[index].notfound) for (let p = Routes[index].parent; idx.at(-1) !== root && p != null && p >= 0; p = Routes[p].parent) { if (idx.includes(p)) return []; idx.push(p); }
  if (!idx.includes(root)) idx.push(root);
  return idx.reverse().map(i => {
    const r = Routes[i], own = empty();
    for (const n of names(r.pattern)) own[Names.indexOf(n)] = params[Names.indexOf(n)];
    return { name: r.name, url: i === index ? url : canonical(path(r, names(r.pattern).map(n => params[Names.indexOf(n)]))), tab: Routes[root].name, params: own };
  });
}
const entry = (id, d) => [id, d.name, d.url, d.tab, d.params];
function refuse(r, why) { say(`router: ${why}`); return r; }
function mint(r, d) { const id = r[2]; r[2] = id + 1; return entry(id, d); }
const sel = r => r[1].findIndex(t => t[0] === r[0] && t[1].length);
const copy = r => [r[0], r[1].map(t => [t[0], t[1].slice()]), r[2]];
export const launch = location => x_open(["", [], 0], location);
export function x_open(r, location) {
  const c = chain(location);
  if (!c.length) return refuse(r, `no route matches ${canonical(location)}`);
  const out = copy(r);
  if (!out[1].length) for (const i of roots()) out[1].push([Routes[i].name, [mint(out, { name: Routes[i].name, url: canonical(Routes[i].pattern), tab: Routes[i].name, params: empty() })]]);
  const t = out[1].find(t => t[0] === c[0].tab);
  if (!t) return refuse(r, `unknown tab ${c[0].tab}`);
  t[1] = c.map((d, k) => t[1][k]?.[2] === d.url ? entry(t[1][k][0], d) : mint(out, d));
  out[0] = c[0].tab;
  return out;
}
function dest(r, location) { const url = canonical(location), m = matchRoute(url); return m && { name: Routes[m[0]].name, params: m[1], url, tab: r[0] }; }
export function x_push(r, location) {
  if (!r[1].length) return x_open(r, location);
  const d = dest(r, location), i = sel(r);
  if (!d) return refuse(r, `no route matches ${canonical(location)}`);
  if (i < 0) return refuse(r, "router has no selected stack");
  const out = copy(r); out[1][i][1].push(mint(out, d)); return out;
}
export function x_replace(r, location) {
  if (!r[1].length) return x_open(r, location);
  const d = dest(r, location), i = sel(r);
  if (!d) return refuse(r, `no route matches ${canonical(location)}`);
  if (i < 0) return refuse(r, "router has no selected stack");
  if (r[1][i][1].length === 1 && d.name !== r[1][i][0]) return refuse(r, "replace cannot change the tab's root route");
  const out = copy(r), s = out[1][i][1]; s[s.length - 1] = entry(s.at(-1)[0], d); return out;
}
export function x_back(r) { const i = sel(r); if (i < 0 || r[1][i][1].length < 2) return r; const out = copy(r); out[1][i][1].pop(); return out; }
export function x_select(r, name) {
  const i = r[1].findIndex(t => t[0] === name && t[1].length);
  if (i < 0) return refuse(r, `unknown tab ${name}`);
  const out = copy(r); if (r[0] === name) out[1][i][1].length = 1; out[0] = name; return out;
}
export function x_go(r, location) {
  const url = canonical(location);
  if (!matchRoute(url)) return refuse(r, `no route matches ${url}`);
  const s = x_stack(r), at = s.map(e => e[2]).lastIndexOf(url);
  if (at >= 0) { const i = sel(r); if (i < 0) return r; const out = copy(r); out[1][i][1].length = at + 1; return out; }
  const other = r[1].find(t => t[0] !== r[0] && t[1].at(-1)?.[2] === url);
  return other ? x_select(r, other[0]) : x_push(r, location);
}
export const x_stack = r => r[1].find(t => t[0] === r[0])?.[1] ?? [];
export const x_top = r => x_stack(r).at(-1) ?? [0, "", "", "", empty()];
export const x_depth = r => x_stack(r).length;
export const x_params = (r, name) => x_stack(r).map(e => e[4][Names.indexOf(name)]).filter(v => v);
export function x_searchParam(e, name) {
  const q = e[2].split("?")[1]; if (!q) return "";
  for (const pair of q.split("#")[0].split("&").filter(Boolean)) { const [k, ...v] = pair.split("="); if (dec(k, true) === name) return dec(v.join("="), true); }
  return "";
}
export const x_encodeRouteSegment = segmentOf;
export const x_path = (name, ...values) => path(Routes.find(r => r.name === name && !r.notfound), values);
/** The router slot's changes, to the browser's history (`navigation.js`,
 * the web host's own), and a popstate back as the navigation root's
 * `navigate` (LLP 1038 D7, D11). */
let RouterSlot = null, Shown = null, Navigate = null;
export function router(slot, history) {
  RouterSlot = slot;
  history.connect(document.getElementById("exact-root"), location => Navigate?.(location), say);
  effect(() => {
    const r = slot(); if (!r || !r[1].length) return;
    const top = x_top(r), ids = new Set(r[1].flatMap(t => t[1].map(e => e[0])));
    const removed = Shown ? Shown[1].flatMap(t => t[1].map(e => e[0])).filter(id => !ids.has(id)) : [];
    Shown = r;
    history.apply({ top: top[0], url: top[2], removed });
    queueMicrotask(() => history.project(document.getElementById("exact-root"), say));
  });
}
export const navigateTo = f => { Navigate = f; };
/** The route a location matches: its row index (renderers pick a policy by it). */
export const routeAt = location => matchRoute(canonical(location))?.[0] ?? -1;
