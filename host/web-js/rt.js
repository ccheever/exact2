import { renderMarkup, reportPlace } from "./navigation.js";
import { conforms, eq } from "./shape.js";
import { paintList, paintFacts, paintFlush } from "./paint.js";
export { conforms, eq }; export { paintOwn } from "./paint.js";
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
  for (const s of n.src) if (!s.dead) s.obs.delete(n);
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
/** A row's new item: an effect `fm` marked re-runs only if a field it reads changed (LLP 1071.000 D3). */
export function writeItem(n, v) {
  const o = n.v;
  if (eq(o, v)) return;
  let m = -1;
  if (Array.isArray(o) && Array.isArray(v) && o.length === v.length) { m = 0; for (let k = 0; k < v.length && k < 31; k++) if (!eq(o[k], v[k])) m |= 1 << k; }
  n.v = v; Rev++;
  for (const x of n.obs) if (!x.m || x.m & m) stale(x, DIRTY);
}
let Mask = 0; // effects made in `fm`'s `g` read a row's item only as the fields in `m`
export function fm(m, g) { const o = Mask; Mask = m; try { g(); } finally { Mask = o; } }
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
  // Against its last value, which conformed: an unchanged part is not checked again.
  const n = node(t ? last => { const v = fn(); if (!conforms(v, t, [0], last)) throw new Refusal("a derive's value does not conform to its type"); return v; } : fn);
  Settle.push(n);
  return () => read(n);
}
export function effect(fn) { const n = node(fn, undefined, 1); if (Mask) n.m = Mask; fresh(n); return n; }
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
/** Leaving rows' scopes end, owner's kids filtered once; nothing unsubscribes from their own signals. */
function endAll(rows) {
  let up = null;
  for (const r of rows.values()) if (r.s) { r.item.n.dead = r.index.n.dead = 1; dispose(r.s); up = r.s.up; }
  if (up?.kids) up.kids = up.kids.filter(k => !k.gone);
}
// For loaded pieces (list.js): scopes, untracked reads, writes, the owner in
// force, and a count of what commits changed (an edge's no-op, runner/collection.rs).
export { scope, end, untracked, write, onEnd };
export const owner = () => Owner, rev = () => Rev, ticket = () => Ticket, nextTicket = () => ++Ticket;

// ---------------------------------------------------------------- commits
/** A typed refusal: the commit rolls back (LLP 1005 §6 atomicity). */
export class Refusal extends Error {}
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
  unpark();
  for (const f of Before) f();
  // Presence measures what it tracks before the tree changes (LLP 1063).
  Pres?.before({ ops: [] }, Views);
  try { flush(); } catch (e) { Poisoned = true; say(`poisoned: ${e.message}`); console.error(e); return false; }
  settled();
  if (!ok) return false;
  clock.epoch++;
  Store.persist();
  for (const go of out) go();
  for (const c of cmds) command(...c);
  // An answer's `then` runs as its own commit, after this one stood.
  for (const m of landed) if (m.then) setTimeout(() => m.then());
  return true;
}
/** What runs after each commit's tree update (a loaded piece's publication),
 * and before it (the text flow piece puts flowed paragraphs back). */
export const After = [], Before = [];
/** Authored scroll offsets (`scrollTop`, `scrollLeft`), set once the
 * commit's tree is in place, as the web host's `pendingScrolls`; a
 * virtualized list builds the rows there first (`$jump`, list.js). */
const Scrolls = new Map();
/** What a commit does once its tree is in place: authored scrolls, then the
 * loaded pieces' publications (also after a list's report, list.js). */
export function settled() { drain(); Present?.(); if (Docs.size || Marked) markDocument(); paintFlush(); for (const f of After) f(); }
/** `<html data-scrolldocument>` while an element is the page's scroller
 * (LLP 1048.003 D4), which the shell's rule reads, as the web host's glue. */
const Docs = new Set();
let Marked = false;
function markDocument() {
  Marked = false;
  for (const e of Docs) if (!e.isConnected) Docs.delete(e); else Marked ||= e.getAttribute("data-scrolldocument") === "true";
  document.documentElement?.toggleAttribute?.("data-scrolldocument", Marked);
}
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
/** The host commands, by name; a loaded piece adds its own (list.js `scrollIntoView`). */
export const Hosts = {
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
export const clock = { now: 0, timers: [], agent: false, epoch: 0 };
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
// A release build never enters agent mode (LLP 1069.007 D2): its build
// writes this false, as the wasm host's files are gated.
const AGENT_ADMITTED = true;
/** A timer: `every(ms, action, once)`, due from mount. */
export function every(ms, action, once) {
  clock.timers.push({ due: clock.now + ms, ms, action, once });
  if (!clock.agent) drive();
}
// A frame task (LLP 1073): once per presented frame, never caught up; on the
// agent's seekable clock, a virtual frame every 1000/60 ms after it last fired.
// The kth virtual frame after `base`, the product first, as the runner's
// `virtual_frame`: sixty frames are exactly a second.
const vf = (base, k) => base + k * 1000 / 60;
/** `every(frame, action)`. */
export function frames(action) {
  clock.timers.push({ due: vf(clock.now, 1), base: clock.now, k: 1, frame: true, action });
  if (!clock.agent) paint();
}
/** Move the clock to `to`, firing each due timer at its own time, in order;
 * a seek fires frame tasks' virtual frames too, the wall clock's (`wall`) none. */
/** `stop()`, asked after each timer, ends the advance there (the agent's:
 * one that sent a request): true when it stopped short of `to`. Under the
 * agent the journal gets the runner's line for an advance that fired. */
export function advance(to, wall, stop) {
  let fired = 0, stopped = false;
  for (;;) {
    let next = null;
    for (const t of clock.timers) if (t.due <= to && !(wall && t.frame) && (!next || t.due < next.due)) next = t;
    if (!next) break;
    clock.now = next.due;
    if (next.once) clock.timers.splice(clock.timers.indexOf(next), 1); else next.due = next.frame ? vf(next.base, ++next.k) : next.due + next.ms;
    fire(next);
    fired++;
    if (stop?.()) { stopped = true; break; }
  }
  if (!stopped) clock.now = Math.max(clock.now, to);
  // Under the agent only: this journal is not a ring, and a page's own
  // clock would add a line a tick.
  if (fired && clock.agent) say(`advance → ${fired} timer${fired === 1 ? "" : "s"} fired, epoch ${clock.epoch}`);
  return stopped;
}
function fire(t) { Timing = true; try { t.action(); } finally { Timing = false; } }
let driving = 0, start = 0, painting = 0;
function drive() {
  clearTimeout(driving);
  let next = Infinity;
  for (const t of clock.timers) if (!t.frame && t.due < next) next = t.due;
  if (!isFinite(next)) return;
  driving = setTimeout(() => { advance(performance.now() - start, true); drive(); }, Math.max(0, next - (performance.now() - start)));
}
// Presented frames: before each paint, timers due by the frame's time, then
// every frame task once at it (Runner::frame).
function paint() {
  if (painting || typeof requestAnimationFrame !== "function") return;
  painting = requestAnimationFrame(function frame(ts) {
    painting = 0;
    if (clock.agent || !clock.timers.some(t => t.frame)) return;
    painting = requestAnimationFrame(frame);
    advance(Math.max(clock.now, ts - start), true);
    const at = clock.now, rev = Rev, ticket = Ticket;
    NowRead = false;
    for (const t of clock.timers) if (t.frame) { t.base = at; t.k = 1; t.due = vf(at, 1); fire(t); }
    // Frames whose tasks changed nothing and read no clock would change
    // nothing again until state does: the loop parks until a commit writes
    // (skipping a frame that would commit nothing is unobservable).
    if (Rev === rev && Ticket === ticket && !NowRead) { cancelAnimationFrame(painting); painting = 0; Parked = Rev; }
    drive();
  });
}
let NowRead = false, Parked = -1;
/** After a commit that wrote: a parked frame loop runs again. */
function unpark() { if (Parked >= 0 && Rev !== Parked) { Parked = -1; paint(); } }

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
const sameReq = (a, b) => a && b && a.storage === b.storage && a.method === b.method && a.url === b.url && a.body === b.body && JSON.stringify(a.headers) === JSON.stringify(b.headers) && a.http === b.http;
/** Run a request after the commit publishes; `land(outcome)` on reply. */
/** Requests in flight, for the agent's `clock settle`. */
export const inflight = { n: 0 };
function send(t, land) {
  Out.push(() => {
    inflight.n++;
    const started = performance.now();
    const done = o => {
      inflight.n--; t.elapsed = Math.max(0, Math.round(performance.now() - started));
      say(`reply ${t.id}; wall ${t.elapsed} ms`); land(o);
    };
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
    if (type && !conforms(v, type, [0], r.checked)) throw new Refusal(`${name}: the answer does not conform to its shape`);
    r.checked = v;
    r.value = v; r.settled = a;
  };
  const land = t => outcome => commit(() => {
    if (r.ticket !== t) return say(`dropped reply for ${name}: ticket ${t.id} is no longer held`);
    const p = outcome.v !== undefined ? { v: outcome.v } : outcome.error ? (() => { throw new Refusal(outcome.error); })() : data.parse(source, t.args, outcome, Store);
    if (p.req) { t.req = p.req; t.id = ++Ticket; send(t, land(t)); return; }
    take(p.v, t.args); r.ticket = null;
    W(pend, false); W(fail, null); W(ver, ver.n.v + 1);
  }, `reply ${name}; wall ${t.elapsed} ms`);
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
    catch (e) {
      if (e instanceof Refusal) throw e;
      if (e.refuse) throw new Refusal(`resource ${name}: ${e.message}`);
      flag(fail, String(e.message)); say(`resource ${name} failed: ${e.message}`); return r.value;
    }
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
    if (type && !conforms(p.v, type, [0], m.checked)) throw new Refusal(`${name}: the answer does not conform to its shape`);
    m.checked = p.v;
    m.ticket = null; W(pend, false);
    slot.n.landing = 1; W(slot, p.v); Landed.push(m);
    // At the reply, the declared refreshes are forced (LLP 1054.000.000 D1).
    for (const r of refreshes) R(r.r);
    queueMicrotask(() => { slot.n.landing = 0; });
  }, `reply ${name}; wall ${t.elapsed} ms`);
  Object.assign(m, {
    forget(undo) { if (m.ticket) { say(`forget ticket ${m.ticket.id} (${name})`); m.ticket = null; undo.push([pend.n, pend.n.v]); write(pend.n, false); } },
    send(source, args, undo) {
      const a = ask(source, args, name);
      if (a && "v" in a) {
        if (type && !conforms(a.v, type, [0], m.checked)) throw new Refusal(`${name}: the answer does not conform to its shape`);
        m.checked = a.v;
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
/** An app file named from the root (`/assets/…`, `/deck/…`, `/shaders/…`)
 * is its published release's when the page is one (its base, LLP 1038 D7),
 * as the web host's `localAssetURL` resolves it. */
let Release;
const rel = (k, v) => (k === "src" || k === "poster") && /^\/(assets|deck|shaders)\//.test(v ?? "")
  && (Release ??= (() => { try { return /^\/\.exact\/root\/web\/releases\/[0-9a-f]{64}\/$/.test(new URL(document.baseURI).pathname); } catch { return false; } })()) ? "." + v : v;
export function h(p, tag, cls, attrs, text, ns) {
  if (Adopt) return adopt(p, tag, cls, attrs);
  const e = ns ? document.createElementNS(ns, tag) : document.createElement(tag);
  if (cls !== 0) e.setAttribute("class", "c" + cls);
  if (attrs) { for (const k in attrs) e.setAttribute(k, rel(k, attrs[k])); if ("data-scrolldocument" in attrs) Docs.add(e); if ("data-exact-box" in attrs) paintList(p); }
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
// class it would have had and drops the renderer's view ids, and inserts
// the region anchors a fresh build would have made. A
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
  // The renderer's inline style stays: it is the class's declarations and
  // the live rows, which the node's style bindings rewrite as they change.
  if (e.hasAttribute("data-view")) e.removeAttribute("data-view");
  if (attrs?.["data-exact-box"] !== undefined && attrs?.["data-exact-own-isolation"] === undefined && e.style.isolation === "isolate") e.style.removeProperty("isolation");
  if (cls !== 0 && e.getAttribute("class") !== "c" + cls) e.setAttribute("class", "c" + cls);
  if (attrs) { for (const k in attrs) { const v = rel(k, attrs[k]); if (e.getAttribute(k) !== v) e.setAttribute(k, v); } if ("data-scrolldocument" in attrs) Docs.add(e); if ("data-exact-box" in attrs) paintList(p); }
  return e;
}
function mark(p) { const c = document.createComment(""); p.insertBefore(c, at(p)); return c; }
/** Build fresh inside an adopted page (a virtualized list's rows). */
export function unadopted(f) { const a = Adopt; Adopt = false; try { return f(); } finally { Adopt = a; } }
/** Whether construction is adopting a rendered page. */
export const adopting = () => Adopt;
/** Adopt a rendered row under `w` (a virtualized list's, LLP 1048.001):
 * one that isn't this plan's projection is built afresh, alone. */
export function adoptRow(w, f) {
  const a = Adopt; Adopt = true;
  try { return f(); } catch (e) { if (!(e instanceof Mismatch)) throw e; say(`list: a row built afresh: ${e.message}`); } finally { Adopt = a; }
  w.textContent = ""; w.$n = undefined;
  return unadopted(f);
}

const BOOL = /^(disabled|readonly|inert|checked|autoplay|controls|loop|muted|playsinline|disablepictureinpicture|disableremoteplayback)$/;
/** A loaded piece's own handling of a prop (symbols.js's `src`): true when handled. */
export const PropHooks = {};
/** A dynamic prop, by the DOM name the live host uses (`applyProps`). */
const navigable = v => { try { return ["http:", "https:", "mailto:", "tel:"].includes(new URL(v, document.baseURI).protocol); } catch { return false; } };
export function P(e, name, f) {
  if (name === "data-scrolldocument") Docs.add(e);
  effect(() => {
    let v = f();
    v = rel(name, v == null ? null : typeof v === "boolean" ? String(v) : String(v));
    // A URL that navigates is written only if the web host's policy takes it
    // (navigation.js `navigableURL`: http, https, mailto, tel): a refused
    // link loses its `href`, an iframe shows about:blank.
    if (v != null && (name === "href" || (name === "src" && e.localName === "iframe")) && !navigable(v)) v = name === "src" ? "about:blank" : null;
    if (PropHooks[name]?.(e, v)) return;
    if (name === "text") { if (!e.childElementCount && e.textContent !== (v ?? "")) e.textContent = v ?? ""; }
    else if (name === "value") { if (e.value !== (v ?? "")) e.value = v ?? ""; }
    else if (name === "scrollTop" || name === "scrollLeft") { if (v != null) (Scrolls.get(e) ?? Scrolls.set(e, {}).get(e))[name] = Number(v); }
    else if (name === "paused") {
      if (v === "true") e.pause(); else e.play().catch(err => e.dispatchEvent(new CustomEvent("exact-error", { detail: err.message })));
    }
    else if (BOOL.test(name)) { e.toggleAttribute(name, v === "true"); if (name === "checked") e.checked = e.$checked = v === "true"; if (name === "muted") e.muted = v === "true"; }
    else if (v == null) { if (e.hasAttribute(name)) { e.removeAttribute(name); if (name.startsWith("data-exact-")) paintFacts(e); } }
    else if (e.getAttribute(name) !== v) { e.setAttribute(name, v); if (name.startsWith("data-exact-")) paintFacts(e); }
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
/** A native module's element (LLP 1024 D3): the real custom element, empty
 * until the web host's adapter (`native-glue.js`, in `native.js`) and the
 * app's module artifact load after first paint; the module renders into it,
 * and its events reach the handlers as `exact-native` events (`on`). */
let Native = null;
export function nm(e) {
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  const id = viewId(e), st = e.exactNative = { id, name: e.localName, state: "loading", status() { return { name: this.name, state: this.state, ...(this.error ? { error: this.error } : {}) }; } };
  onEnd(() => { st.destroyed = true; Native?.then(h => h.destroy(e), () => {}); });
  // In flight until the module is attached and its first events are in
  // (`clock settle` waits for them).
  inflight.n++;
  (Native ??= new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => import("./native.js")).then(m => m.viewHost(say)))
    .then(h => { h.attach(e); return h.loaded; }, err => { st.state = "unavailable"; st.error = String(err?.message ?? err); say(`native ${st.name} #${id}: unavailable: ${st.error}`); })
    .finally(() => setTimeout(() => inflight.n--));
}
/** A dynamic style row: a number takes the unit css.rs gives the row. */
export function S(e, prop, unit, f) { let rendered = Adopt; effect(() => { css(e, prop, unit, f(), rendered); rendered = false; }); }
let Scratch = null;
const Normal = new Map(), same = v => v.replace(/\btransparent\b/g, "rgba(0, 0, 0, 0)");
/** `t` as the browser serializes it on `prop`, once per value. */
function normal(prop, t) {
  const k = prop + "\0" + t;
  let v = Normal.get(k);
  if (v === undefined) { (Scratch ??= document.createElement("i").style).setProperty(prop, t); Normal.set(k, v = same(Scratch.getPropertyValue(prop))); if (Normal.size > 4096) Normal.clear(); }
  return v;
}
function css(e, prop, unit, v, rendered) {
  // The value this binding last wrote: the same again writes nothing (each
  // write was two style mutations, for every dynamic row of every row a
  // list update touched).
  const last = e.$css ??= {}, t = v == null ? null : typeof v === "number" ? v + unit : String(v);
  if (last[prop] === t) return;
  last[prop] = t;
  // An adopted node's inline style is the renderer's: a value it already
  // shows is not written again (a write restyles and repaints the node).
  // (`transparent` is the color the renderer writes as rgba(0, 0, 0, 0).)
  if (rendered) {
    const now = e.style.getPropertyValue(prop);
    if (t == null ? !now : same(now) === normal(prop, t)) return;
  }
  if (t == null) return e.style.removeProperty(prop);
  // A value the row refuses is invalid at computed-value time: unset,
  // never the earlier declaration (LLP 1005 §6).
  e.style.removeProperty(prop);
  e.style.setProperty(prop, t);
  if (!e.style.getPropertyValue(prop)) say(`unset ${prop}: ${JSON.stringify(v)} is not a value it takes`);
}
/** `S` for a row that can reference an element (`url(#…)`): an authored id
 * names the node the kernel's `resolve_id` would, the nearest in the tree
 * (the deepest common ancestor's first, LLP 1055.000 D3), by its DOM id; a
 * name not yet in the tree is resolved again after the commit builds it. */
let Refs = null;
export function Sr(e, prop, unit, f) {
  effect(() => {
    const v = f(), write = () => {
      let missing = false;
      const r = v == null ? v : String(v).replace(/url\(\s*["']?#([^"')\s]+)["']?\s*\)/g, (m, name) => {
        for (let a = e.parentElement; a; a = a.parentElement) { const c = a.querySelector(`[data-exact-id="${CSS.escape(name)}"]`); if (c) return `url(#${c.id})`; }
        missing = true; return m;
      });
      css(e, prop, unit, r);
      return missing;
    };
    if (write()) {
      if (!Refs) { Refs = []; After.push(() => { for (const w of Refs.splice(0)) w(); }); }
      Refs.push(write);
    }
  });
}
export { svgTransform } from "./svg-transform.js"; export { ds } from "./dataset.js"; export { hk } from "./hooks.js";
/** Loaded pieces' hooks: `style(e, prop, value)` takes a dynamic row's
 * write on a node the motion piece holds (motion.js). */
export const Hooks = {};
/** `S` on a node the motion engine follows (`mo`): while a hold owns it, a
 * write goes to the authored style the hold restores. */
export function Sm(e, prop, unit, f) {
  effect(() => {
    const v = f();
    // Held: the hold's authored style takes it, and what css() last wrote no longer says what shows.
    if (Hooks.style?.(e, prop, v == null ? null : typeof v === "number" ? v + unit : String(v))) { if (e.$css) delete e.$css[prop]; }
    else css(e, prop, unit, v);
  });
}
/** An event handler: the DOM event the live host listens to (`glue.js` `attach`). */
/** A loaded piece's own handling of an event (files.js's file input): true when handled. */
export const OnHooks = {};
function guestOrigin(e) {
  if (e.hasAttribute("sandbox") && !e.getAttribute("sandbox").split(/\s+/).includes("allow-same-origin")) return "null";
  const src = e.getAttribute("src");
  let o; try { o = !src || src === "about:blank" ? location.origin : new URL(src, document.baseURI).origin; } catch { return undefined; }
  return o === "null" ? undefined : o;
}
export function on(e, kind, f) {
  const l = (t, g) => e.addEventListener(t, g);
  if (OnHooks.file && e.localName === "input" && e.type === "file" && OnHooks.file(e, kind, f)) return;
  if (e.exactNative) return l("exact-native", ev => { if (ev.detail.kind === kind) f(...(ev.detail.value == null ? [] : [ev.detail.value])); });
  switch (kind) {
    // A link with a press is the app's navigation: the browser's is prevented.
    case "press": return l("click", ev => { const a = ev.target.closest?.("a[href]"); if (a && a !== e && e.contains(a)) return; ev.stopPropagation(); if (e.localName === "a" && !(ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.button)) ev.preventDefault(); f(); });
    // A checkbox's value is whether it is checked; the platform flips the
    // box at once, and an action that refuses snaps it back (glue.js). A
    // host's change carries its own text (files.js: a picker's lines, which
    // an input's value would flatten).
    case "change": case "input": return l(kind, ev => { if (ev instanceof CustomEvent) return f(ev.detail); if (e.type !== "checkbox") return f(e.value); f(e.checked); if (e.$checked !== undefined && e.checked !== e.$checked) e.checked = e.$checked; });
    case "hover": l("pointerenter", () => f(true)); return l("pointerleave", () => f(false));
    case "key": return l("keydown", ev => f(ev.key));
    case "submit": return l("keydown", ev => { if (ev.key === "Enter" && !ev.isComposing) { ev.preventDefault(); f(); } });
    // Only from the origin of the src the app committed (glue.js
    // `guestMessageAuthorized`, LLP 1020 D2): a guest that navigated away is
    // not heard; an opaque sandbox's origin is "null".
    case "message": return addEventListener("message", ev => { if (ev.source === e.contentWindow && ev.origin === guestOrigin(e)) f(typeof ev.data === "string" ? ev.data : JSON.stringify(ev.data)); });
    case "error": l("exact-error", ev => f(ev.detail)); return l("error", () => f(e.error?.message || "Media could not be loaded"));
    case "timeupdate": return l(kind, () => f(e.currentTime));
    // The port's offsets, as the web host sends them (`glue.js` `attach`).
    case "scroll": return l(kind, () => f(e.scrollLeft, e.scrollTop));
    // Pull to refresh is a native port's; the web has none (`glue.js` attaches nothing).
    case "refresh": return;
    case "durationchange": return l(kind, () => Number.isFinite(e.duration) && f(e.duration));
    case "contextmenu": case "dblclick": return l(kind, ev => { ev.preventDefault(); f(); });
    // Chrome blurs an element it is removing (still connected); a retired view's blur is dropped (glue.js).
    case "blur": return l(kind, () => queueMicrotask(() => e.isConnected && f()));
    default: return l(kind, () => f());
  }
}
// ---------------------------------------------------------------- presence (LLP 1063)
// `exit-animation` and `layout-transition`: the web host's own
// presence-glue.js, fetched after the first painted frame by a plan with
// either row (its node calls `pr`), plays both. Each commit it measures the
// views that declare a layout transition before the tree changes and plays
// back each that moved after; a region's removed root that declares an exit
// stays, out of flow at its last box, until its animations end, where it
// was: `clear` passes over it.
// Until the piece is here, rows jump and leave at once, as the wasm host's
// do when it is unavailable.
let Pres = null, Presence = null, Present = null, Leave = null;
/** The after-paint pieces on their way (the agent waits for them before an
 * operation, as glue.js's `agentSettled` waits for `pieces.pending()`). */
export const pieces = () => Promise.all([Motion, Inputs, Presence, Flow, Native].filter(Boolean)).then(() => {}, () => {});
/** A view leaves with the exit animation `css` names (a virtualized list's
 * row wrapper, list.js): whether it stays, leaving, for presence-glue.js to remove. */
export function exitView(el, css) { if (!Pres || !css) return false; Pres.exit(el, css); return exiting(el); }
const Created = [];
export function pr(e) {
  Created.push(e);
  if (Presence || typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  Presence = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => { globalThis.exact ??= {}; return import("./presence-glue.js"); })
    .then(() => {
      const x = globalThis.exact;
      Pres = x.presenceLive = x.presence(document.getElementById("exact-root"));
      Created.length = 0;
      // After a commit's tree: which views declare the row now (the created
      // ones), and every tracked one that moved plays back from where it was.
      Present = () => Pres.after({ ops: Created.splice(0).filter(e => e.isConnected).map(e => ({ op: "create", id: viewId(e) })) }, Views);
      // A removed region node that declares an exit leaves with it (the
      // kernel's `exit`: a destroyed root whose parent stays).
      Leave = n => {
        const css = n.nodeType === 1 && n.style.getPropertyValue("--exact-exit-animation").trim();
        if (css) Pres.exit(n, css);
        if (!exiting(n)) n.remove();
      };
    })
    .catch(err => say(`presence: unavailable; motion skipped: ${err.message}`)).finally(() => inflight.n--);
}

/** The motion piece (motion.js over `motion.wasm` and the web host's
 * `motion-glue.js`), fetched after the first painted frame by a plan that
 * uses motion; `f(m)` runs at once when it is here, else once it is. */
let Motion = null, Mo = null;
function motion(f) {
  if (Mo) return f(Mo);
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  if (!Motion) {
    inflight.n++;
    Motion = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => import("./motion.js"))
      .then(m => m.engine({ clock, wall: () => performance.now() - start, views: Views, viewId, hooks: Hooks, say, inflight }))
      .then(m => { Mo = m; After.push(() => Mo.flush()); })
      .catch(err => say(`motion: ${err.message}`)).finally(() => inflight.n--);
  }
  Motion.then(() => Mo && f(Mo));
}
/** A node the motion engine follows (the kernel's motion seam): `f` gives
 * its `translate`, `scale`, `rotate`, `opacity` and `transition` as the
 * plan binds them, told the engine at every commit that changes them. */
export function mo(e, f) {
  const id = viewId(e);
  onEnd(() => motion(m => m.gone(id)));
  effect(() => { const v = f(); motion(m => m.observe(id, v)); });
}
/** `swiperight`: the motion piece's (motion-glue.js `attachSwipe`); the
 * action runs when the hold passes the knee (motion.js `action`). */
export function onSwipe(e, f) { e.$swipe = f; motion(m => m.swipe(e)); }
/** A height drag's owner (the node a handle's `heightDragFor` names): its
 * numeric height and `transition`, told the motion engine as they change. */
export function mh(e, f) {
  const id = viewId(e);
  onEnd(() => motion(m => m.gone(id)));
  effect(() => { const v = f(); motion(m => m.height(id, v)); });
}
/** `heightrelease`: motion-glue's height drag on the handle, holding `t`. */
export function onHeight(e, f, t) { e.$heightrelease = f; motion(m => m.heightDrag(e, t)); }
/** `transformgeometry`: the pair's geometry, the page's observation. */
export function onTGeom(e, f) { e.$tgeom = f; }
/** `transformrelease`: motion-glue's transform drag on the handle, holding
 * target `t`'s translate and scale inside its clip `c` (LLP 1057.001 §4). */
export function onTRelease(e, f, t, c) { e.$trelease = f; motion(m => m.transformDrag(e, t, c)); }
/** A `reorderFor` grip on list `l` (the strict ancestor it names), and a
 * virtualized list's `reorderdrop`: Arrange, motion-glue's reorder drag
 * over the list's preview (list.js) and the motion piece (arrange.js). The
 * grip names its view (`data-view`): the list's browser half pins the row
 * a contact on it holds by that name (collection-glue.js `liveView`). */
export function onReorder(e, l) { e.$reorderList = l; e.dataset.view = viewId(e); motion(m => m.reorderHandle(e)); }
export function onDrop(e, f) { e.$reorderdrop = f; motion(() => {}); }
/** `pan`: the web host's input piece's (input-glue.js), after first paint. */
export function onPan(e, f) {
  e.$pan = f; e.exactHandlers ??= e.dataset.exactOn.split(" ");
  const id = viewId(e); let p;
  input();
  e.addEventListener("pointerdown", ev => (p ??= Input?.pan(e, id, (t, g) => e.addEventListener(t, g)))?.(ev));
}
/** `panrelease`: its velocity is the motion piece's tracker (LLP 1057 §10.6). */
export function onPanRelease(e, f) { e.$panrelease = f; motion(() => {}); }
/** A node's `wrap-flow` (LLP 1043.000): an absolutely positioned `both`
 * is an exclusion the text around it flows past, laid out by the text flow
 * piece (flow.js over the web host's `textflow-glue.js`), fetched after
 * first paint by a plan with the row. */
let Flow = null, Fl = null;
const Wraps = new Set();
export function wf(e, f) {
  Wraps.add(e);
  onEnd(() => Wraps.delete(e));
  effect(() => { e.$wrap = f(); });
  if (Flow || typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  Flow = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => import("./flow.js"))
    .then(m => m.flow({ views: Views, viewId, wraps: Wraps, clock, wall: () => performance.now() - start, say }))
    .then(m => { Fl = m; Before.push(() => Fl.before()); After.push(() => Fl.after()); (globalThis.exact ??= {}).flowSettle = () => Fl.settle(); Fl.after(); })
    .catch(err => say(`text flow: ${err.message}`)).finally(() => inflight.n--);
}
/** `frame(id)` and `measure(id)` (LLP 1051.000): the page's answers, from
 * the web host's own geometry-glue.js, fetched after first paint by a plan
 * whose actions read geometry (`geo`); unavailable until it is (D5). The
 * record is the runner's `Geometry`: x, y, width, height, provisional,
 * unavailable. */
let Geo = null, Geometry = null;
export function geo() {
  if (Geometry || typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  Geometry = new Promise(r => requestAnimationFrame(() => r())).then(() => import("./geometry-glue.js"))
    .then(() => { Geo = globalThis.exact.geometry(document.getElementById("exact-root")); })
    .catch(err => say(`geometry: ${err.message}`)).finally(() => inflight.n--);
}
function geoRead(op, id) {
  const out = new Float64Array(4), bits = Geo ? Geo.read(op, document.getElementById(id), out) : 0;
  return bits & 1 ? [out[0], out[1], out[2], out[3], !!(bits & 2), false] : [0, 0, 0, 0, false, true];
}
export const x_frame = id => geoRead(0, id), x_measure = id => geoRead(1, id);
/** A Markdown text field (LLP 1045 D5): the web host's own editor
 * (markup-editor.js over its wasm), fetched at the first one; it replaces
 * the textarea, which then forwards to it what this runtime writes and
 * listens for: its value, attributes, style and events. */
let Editor = null;
export function mde(e) {
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  if (!Editor) {
    Editor = import("./markup-editor.js").then(() => globalThis.exact.installMarkupEditor);
    Hosts.format = (id, name, arg) => Editor.then(() => document.getElementById(id)?.exactMarkup?.format(name, arg ?? ""));
  }
  Editor.then(install => {
    if (!e.isConnected) return;
    const id = viewId(e);
    install(e, {
      live: n => n.isConnected,
      replace(n) {
        Ids.set(n, id); Views.set(id, n);
        Object.defineProperty(e, "value", { configurable: true, get: () => n.value, set: v => { n.value = v; } });
        for (const t of ["input", "change", "focus", "blur", "keydown", "pointerenter", "pointerleave", "click"]) n.addEventListener(t, ev => e.dispatchEvent(new ev.constructor(t, ev)));
        new MutationObserver(ms => { for (const m of ms) { const v = e.getAttribute(m.attributeName); v == null ? n.removeAttribute(m.attributeName) : n.setAttribute(m.attributeName, v); } n.exactMarkup?.sync(); }).observe(e, { attributes: true });
      },
      select: (n, p) => { const [formats, mixed, unavailable, link] = p.split("\n"); e.$select?.([formats, mixed === "1", link ?? "", unavailable]); },
    });
  }).catch(err => say(`markup editor: ${err.message}`)).finally(() => inflight.n--);
}
/** `select`: the editor's facts at each selection change (runner Event::Select). */
export function onSelect(e, f) { e.$select = f; }
/** Press feedback (LLP 1061): the web host's input piece shows it, from
 * the node's own `--exact-press` (css.rs). */
export const pressFeedback = () => input();
/** The web host's input piece (input-glue.js), after first paint: pans. */
let Input = null, Inputs = null;
function input() {
  if (Inputs || typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  // Its link handler asks the wasm's route table; this runtime routes its own.
  const x = globalThis.exact ??= {}; (x.wasm ??= {}).exact_route_match ??= () => 0; x.writeIn ??= () => 0;
  const to = (id, p, k) => Views.get(id)?.[k]?.(...p.split(",").map(Number));
  Inputs = new Promise(r => requestAnimationFrame(() => setTimeout(r))).then(() => import("./input-glue.js")).then(m => {
    Input = m.createInputHandlers({ root: document.getElementById("exact-root"), views: Views, retiredViews: new WeakSet(), ready: () => true,
      inertAncestor: el => el.closest("[inert]"), agentMode: clock.agent, dispatch: (id, p) => to(id, p, "$pan"), release: (id, p) => to(id, p, "$panrelease"),
      velocity: { sample: (...a) => Mo?.pan.sample(...a), velocity: (...a) => Mo?.pan.velocity(...a) } });
  }).catch(err => say(`input: ${err.message}`)).finally(() => inflight.n--);
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
// A view leaving with its exit animation stays where it was until it ends
// (presence-glue.js removes it): never moved, since moving cancels a CSS animation.
const exiting = n => n.nodeType === 1 && n.hasAttribute("data-exiting");
function clear(a, b) { paintList(a.parentNode); for (let n = a.nextSibling; n !== b;) { const m = n.nextSibling; if (!exiting(n)) Leave ? Leave(n, b) : n.remove(); n = m; } }
function build(b, f, own) {
  const frag = document.createDocumentFragment();
  const s = scope(() => f(frag), own);
  b.before(frag); paintList(b.parentNode);
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
export function each(p, list, key, row, pure) {
  let [a, b] = range(p), own = Owner;
  let rows = new Map(), single = false, order = null;
  effect(() => {
    const items = list();
    untracked(() => {
      // Rows moving or leaving are adopted rows (a row waiting for its slice
      // shows its rendered values until then, and adopts at the current ones).
      if (b && LazyAt < Lazy.length) adoptAll();
      // Keys that didn't move: new items to their rows, nothing else (LLP 1071.000 D2).
      if (pure && order && items.length === order.length) {
        let i = 0;
        for (; i < items.length; i++) {
          const item = items[i], r = order[i];
          if (r.item.n.v === item) continue;
          const k = key(() => item, () => i);
          if (typeof k + ":" + (Object.is(k, -0) ? 0 : k) !== r.k) break;
          writeItem(r.item.n, item);
        }
        if (i === items.length) return;
      }
      // A row's place in the last pass is its `at`; repeats count once a key repeats.
      const next = new Map(), seen = new Map();
      items.forEach((item, i) => {
        let k = key(() => item, () => i);
        k = typeof k + ":" + (Object.is(k, -0) ? 0 : k);
        if (next.has(k)) { const n = seen.get(k) ?? 1; seen.set(k, n + 1); k = "d" + n + ":" + k; journal.push(`each: repeated key ${k}`); }
        let r = rows.get(k);
        if (r) { rows.delete(k); r.old = r.at; if (!Object.is(r.item.n.v, item)) writeItem(r.item.n, item); if (r.index.n.v !== i) write(r.index.n, i); }
        else if (!b) {
          // Adopting: the row's elements are in place, in item order.
          r = { item: sig(item), index: sig(i) };
          if (single && i && performance.now() > AdoptBy) {
            // Past the adoption's budget, a row of one element waits for a
            // slice (`lazy`); a press on it or any commit adopts it first.
            let e = at(p);
            while (e && e.nodeType !== 1) e = e.nextSibling;
            if (!e) throw new Mismatch(`adoption: expected a row, found nothing`);
            p.$n = e.nextSibling;
            r.start = r.end = e;
            lazy([p, r, row, own]);
          } else {
            const next = at(p), last = next ? next.previousSibling : p.lastChild;
            r.s = scope(() => row(p, r.item, r.index), own);
            // A row of one element is that element, as a fresh build keeps it
            // (a region at its top would have put its own anchors beside it).
            const first = last ? last.nextSibling : p.firstChild;
            if (first && first.nodeType === 1 && first.nextSibling === at(p)) r.start = r.end = first;
            else { r.start = document.createComment(""); p.insertBefore(r.start, first ?? at(p)); r.end = mark(p); }
            if (!i) single = r.start.nodeType === 1;
          }
        }
        else {
          r = { item: sig(item), index: sig(i) };
          const frag = document.createDocumentFragment();
          r.s = scope(() => row(frag, r.item, r.index), own);
          // A row of one element is that element (a region at its top would
          // have put its own anchors beside it); any other is bracketed.
          if (frag.childNodes.length === 1 && frag.firstChild.nodeType === 1) r.start = r.end = frag.firstChild;
          else { r.start = document.createComment(""); r.end = document.createComment(""); frag.prepend(r.start); frag.append(r.end); }
          r.frag = frag;
        }
        r.k = k; next.set(k, r);
      });
      const list = order = [...next.values()];
      for (let i = 0; i < list.length; i++) list[i].at = i;
      // Every row goes and the region is all its parent holds: emptied at once.
      if (rows.size && b && !Leave && list.every(r => r.frag) && !a.previousSibling && !b.nextSibling) {
        endAll(rows);
        p.textContent = "";
        p.append(a, b);
      }
      else if (rows.size) { endAll(rows); for (const r of rows.values()) { let n = r.start; while (n) { const m = n.nextSibling; Leave ? Leave(n, b) : n.remove(); if (n === r.end) break; n = m; } } }
      // Order, from the last row back: kept rows on the longest run already in
      // order stay; any other moves before the row after it; new rows go in
      // one fragment per run.
      if (b) {
        const stay = inOrder(list);
        let anchor = b, batch = null, first = null;
        const flush = () => { if (batch) { p.insertBefore(batch, anchor); anchor = first; batch = null; } };
        for (let i = list.length - 1; i >= 0; i--) {
          const r = list[i];
          if (r.frag) { if (batch) batch.prepend(r.frag); else batch = r.frag; first = r.start; r.frag = null; continue; }
          flush();
          if (!stay.has(i)) {
            const f = document.createDocumentFragment();
            let n = r.start; while (n) { const m = n.nextSibling; f.append(n); if (n === r.end) break; n = m; }
            p.insertBefore(f, anchor);
          }
          anchor = r.start;
        }
        flush();
      }
      rows = next; paintList(p);
      b ??= mark(p);
    });
  });
}

/** The indices of `list`'s kept rows (`old`, their former places) on a
 * longest run in increasing former order: the rows that need not move. */
function inOrder(list) {
  const tails = [], prev = new Array(list.length);
  for (let i = 0; i < list.length; i++) {
    const o = list[i].old;
    if (o === undefined) continue;
    let lo = 0, hi = tails.length;
    while (lo < hi) { const mid = (lo + hi) >> 1; if (list[tails[mid]].old < o) lo = mid + 1; else hi = mid; }
    prev[i] = lo ? tails[lo - 1] : -1;
    tails[lo] = i;
  }
  const stay = new Set();
  for (let i = tails.length ? tails[tails.length - 1] : -1; i >= 0; i = prev[i]) stay.add(i);
  return stay;
}

// Adoption in slices (LLP 1048.001): a keyed list's rows adopt in the
// adoption's own task until its budget runs out, then in tasks of a few
// milliseconds after it, so no one task holds the page. A press, key or
// edit on a row still waiting adopts that row before the event reaches it;
// a list that changes adopts every row still waiting first. A waiting row
// shows its rendered values, and its bindings take the current ones when it
// adopts (a structure that no longer matches builds that row afresh).
const ADOPT_MS = 16, SLICE_MS = 8;
let AdoptBy = Infinity;
const Lazy = [], LazyRows = new Map();
let LazyAt = 0, LazyTask = null;
const LAZY_EVENTS = ["pointerdown", "mousedown", "touchstart", "click", "keydown", "beforeinput", "input", "change", "focusin"];
// Passive: a waiting row must never make the page's touches wait for script.
const LAZY_OPTS = { capture: true, passive: true };
function lazy(x) { Lazy.push(x); LazyRows.set(x[1].start, x); }
function adoptLazy(x) {
  const [p, r, row, own] = x;
  LazyRows.delete(r.start);
  // A row whose list left (its region's arm ended) has nothing to adopt.
  if (r.s || own?.gone || !r.start.isConnected) return;
  const save = p.$n, a = Adopt;
  p.$n = r.start; Adopt = true;
  try { r.s = scope(() => row(p, r.item, r.index), own); }
  catch (e) {
    if (!(e instanceof Mismatch)) throw e;
    say(`each: a row built afresh: ${e.message}`);
    Adopt = false;
    const frag = document.createDocumentFragment(), old = r.start;
    r.s = scope(() => row(frag, r.item, r.index), own);
    if (frag.childNodes.length === 1 && frag.firstChild.nodeType === 1) r.start = r.end = frag.firstChild;
    else { r.start = document.createComment(""); r.end = document.createComment(""); frag.prepend(r.start); frag.append(r.end); }
    old.replaceWith(frag);
  } finally { Adopt = a; p.$n = save; }
}
function adoptAll() { while (LazyAt < Lazy.length) adoptLazy(Lazy[LazyAt++]); lazyDone(); }
function lazyDone() {
  if (LazyAt < Lazy.length || !Lazy.length) return;
  Lazy.length = LazyAt = 0; LazyRows.clear();
  inflight.n--;
  const root = document.getElementById("exact-root");
  for (const t of LAZY_EVENTS) root?.removeEventListener(t, onLazy, LAZY_OPTS);
}
function adoptAt(target) {
  for (let n = target; n && n.nodeType === 1; n = n.parentNode) { const x = LazyRows.get(n); if (x) { adoptLazy(x); break; } }
}
const onLazy = ev => adoptAt(ev.target);
function slice() {
  LazyTask = null;
  const end = performance.now() + SLICE_MS;
  while (LazyAt < Lazy.length && performance.now() < end) adoptLazy(Lazy[LazyAt++]);
  if (LazyAt < Lazy.length) LazyTask = post(slice); else lazyDone();
}
const post = f => globalThis.scheduler?.postTask ? scheduler.postTask(f, { priority: "user-visible" }) : setTimeout(f);

// ---------------------------------------------------------------- boot
/** Build the view into `#exact-root` and start the clock. */
export function mount(f) {
  const root = document.getElementById("exact-root");
  // A press on a `retainFocus` node keeps the focus where it is (an
  // editor's), as the web host's glue does: the browser's focus move on
  // pointerdown is prevented, unless the contact is on an editor.
  root?.addEventListener("pointerdown", ev => {
    const t = ev.target;
    if (!(t.closest?.("input, textarea, select") || t.isContentEditable) && t.closest?.('[retainFocus="true"]')) ev.preventDefault();
  });
  // Under the agent, and in a render, the clock is the driver's: no timer runs by itself.
  clock.agent = !!globalThis.__exactRender || (AGENT_ADMITTED && new URLSearchParams(location.search).has("agent"));
  Store.load();
  let built = false;
  Adopt = !!(checkpoint().kept && root.firstElementChild);
  // Elapsed time continues from where a render's clock stopped.
  start = performance.now() - clock.now;
  const adopting = Adopt;
  // What the reader did before the runtime ran (the capture script), and
  // what each edited control showed then: the commit writes the state's
  // values, which the reader had changed.
  const early = globalThis.exact?.taps?.() ?? [];
  const shown = early.filter(t => t.type !== "click").map(t => [t.target, t.target.value, t.target.checked]);
  AdoptBy = performance.now() + ADOPT_MS;
  commit(() => { scope(() => f(root)); built = true; }, adopting ? "adopt" : "boot");
  Adopt = false; AdoptBy = Infinity;
  if (!built) { Lazy.length = LazyAt = 0; LazyRows.clear(); }
  const adopted = adopting && built;
  if (!built && adopting) {
    // The document isn't this plan's projection: build afresh (and say so).
    say(`adoption abandoned: ${journal.at(-1)}`);
    root.textContent = "";
    commit(() => { scope(() => f(root)); built = true; }, "boot");
  }
  if (!built) throw new Error("boot refused: " + journal.at(-1));
  say(`boot: ${root.getElementsByTagName("*").length} nodes, epoch ${clock.epoch}`); // the runner's journal line (LLP 1012 logs)
  if (adopted) say("adopted the document");
  // The document's autofocus (LLP 1035.000 D9): once, at boot, the first
  // `autofocus` view, unless the reader already put the focus somewhere.
  if (!document.activeElement || document.activeElement === document.body) root.querySelector("[autofocus]")?.focus({ preventScroll: true });
  // Rows waiting are in flight, for the agent's `clock settle`.
  if (Lazy.length) { inflight.n++; for (const t of LAZY_EVENTS) root.addEventListener(t, onLazy, LAZY_OPTS); LazyTask = post(slice); }
  root.dataset.bootMs = String(Math.round(performance.now()));
  // Replayed once, in order, on the same elements (LLP 1048.001 D5), each
  // edited control first showing what the reader left in it (its row, if it
  // waits for a slice, adopted first, so adoption doesn't write over it).
  for (const t of early) adoptAt(t.target);
  for (const [e, v, c] of shown) if (e.isConnected) { if (e.type === "checkbox" || e.type === "radio") e.checked = c; else if (e.type !== "file") e.value = v; }
  for (const t of early) if (t.target.isConnected) t.type === "click" ? t.target.click() : t.target.dispatchEvent(new Event(t.type, { bubbles: true }));
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
  // A drive starts at zero even when its document was rendered on a wall clock.
  const driven = AGENT_ADMITTED && typeof location !== "undefined" && new URLSearchParams(location.search).has("agent") && !globalThis.__exactRender;
  Checkpoint.time = clock.now = driven ? 0 : cp.time || 0;
  return Checkpoint;
}
/** A checkpoint value (`push_value`, host/web/src/page.rs) as a runtime value:
 * lists and records are arrays, unit and `none` null, `some(v)` v. */
const value_ = v => v === null || typeof v !== "object" ? v : Array.isArray(v) ? v.map(value_) : "r" in v ? v.r.map(value_) : "s" in v ? value_(v.s) : "n" in v ? Number(v.n) : null;

// ---------------------------------------------------------------- the roster (runner/src/stdlib.rs)
/** A native module's props (LLP 1024 D1): key/value pairs to one JSON
 * object of strings, a none left out (`stdlib::native_props`). */
export const NP = p => { const o = {}; for (let i = 0; i < p.length; i += 2) if (p[i + 1] != null) o[p[i]] = String(p[i + 1]); return JSON.stringify(o); };
export const x_now = () => { NowRead = true; return read(Now); };
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
let RouterSlot = null, Shown = null, Navigate = null, History = null; export const pageHistory = () => History; // the page's navigation.js, which the agent observes: its own copy's state is never written
export function router(slot, history) {
  RouterSlot = slot; History = history;
  history.connect(document.getElementById("exact-root"), location => Navigate?.(location), say);
  // @ref LLP 1038 §7 — a plain click on a same-origin link to a declared
  // route stays in this document, as input-glue.js's rule for the wasm host:
  // a link with its own `press` navigates by it; any other goes to the
  // root's `navigate` handler, as popstate does. A modified click, a
  // `target` or `download`, another origin, this page's fragment or an
  // undeclared path (a file) is the browser's alone.
  document.addEventListener("click", ev => {
    const a = ev.target.closest?.("a[href]"), root = document.getElementById("exact-root");
    if (!a || !root?.contains(a) || ev.defaultPrevented) return;
    const press = (a.dataset.exactOn ?? "").split(" ").includes("press");
    if (ev.button !== 0 || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey || (a.target && a.target !== "_self") || a.hasAttribute("download")) { if (press) ev.stopPropagation(); return; }
    const url = new URL(a.href), to = url.pathname + url.search, here = to === location.pathname + location.search, m = matchRoute(canonical(to));
    if (url.origin !== location.origin || (here && url.hash) || !m || Routes[m[0]].notfound) return;
    if (!press && !Navigate) return;
    ev.preventDefault();
    if (press || here) return;
    const before = RouterSlot.n?.v; Navigate(to); if (RouterSlot.n?.v === before) say(`history: link ${JSON.stringify(to)} refused`);
  }, true);
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
