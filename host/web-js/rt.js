import { renderMarkup, reportPlace, onSelection, textField, settleRadios } from "./navigation.js"; export { animationClocks, launchLocation } from "./navigation.js"; // synced animations (LLP 1055.002, emit.rs `clocks`)
import { Docs, Head, head, markDocument, projectRoots } from "./document.js"; export { Head }; import { conforms, eq, equal } from "./shape.js"; import { pointer, record } from "./pointer.js"; import { commands } from "./commands.js"; import { media, mediaProp, mediaOn, mediaPiece, MEDIA_EVENTS } from "./media.js";
let Paint; export function usePaint(pass) { Paint = pass; } export { conforms, eq, equal }; // the compiler installs `Paint` only when a plan can layer boxes
// The JS target's runtime: fine-grained DOM signals for a plan compiled ahead by `exact-web-js`. Everything here is imported
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

/** A slot: a getter, `.n` its node; `t` its declared type (writes conform); `name` a string slot's, which a write past MAX_STRING names. */
export function sig(v, t, name) { // an initial value outside `t`: the runner refuses the boot, or the row's creation poisons (SlotType)
  if (t && !conforms(v, t)) throw new Error(`a slot's initial value does not conform to its type: ${v}`);
  const n = node(null, v); n.t = t; n.name = name; const g = () => read(n); g.n = n; return g; }
/** A string past MAX_STRING's UTF-8 bytes (runner `too_long`): one test, TextEncoder's U+FFFD for a lone surrogate (LLP 1090 D6). */
const long = s => typeof s === "string" && s.length > 22369621 && (s.length > 67108864 || new TextEncoder().encode(s).length > 67108864);
const tooLong = name => new Refusal(`StringTooLong { name: ${JSON.stringify(name)} }`);
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
/** A data or shape refusal (the runner's `RunnerError::Data` or `Shape`): one in a reply's commit lets its ticket go (`reply`). */
class Failed extends Refusal {}
let Writes = null, Commands = [], Out = [], Landed = [], Sends = [], Refresh = [], Poisoned = false, Refused = null, Sched = null;
/** Queued sends and gated tasks (schedule.js, LLP 1092), installed by a plan that declares them; the last commit's refusal. */
export const useSchedule = s => { Sched = s; }, refused = () => Refused;
export const journal = Object.assign([], { start: 0, push(...l) { const over = Array.prototype.push.apply(this, l) - 4096; if (over > 0) this.start += this.splice(0, over).length; return this.length; } }); // the runner's ring (JOURNAL_RING): `start` is the oldest line's index
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
  Writes = []; Commands = []; Out = []; Landed = []; Sends = []; Refresh = []; Refused = null;
  time();
  const was = Now.v, undo = [], saved = Resources.map(r => r.save()), held = Mutations.map(m => m.ticket), store = Store.save(), queued = Sched?.save();
  let ok = true;
  try {
    untracked(f);
    tick();
    for (const [n, v] of Writes) {
      if (n.t && !conforms(v, n.t)) throw new Refusal(`a write does not conform to its slot's type: ${JSON.stringify(v)}`);
      if (n.name && long(v)) throw tooLong(n.name);
      undo.push([n, n.v]); write(n, v);
      if (n.m && !n.landing) n.m.forget(undo);
    }
    for (const [m, source, args, own] of Sends) m.send(source, args, undo, own);
    for (const r of Refresh) r.force(undo);
    if (!routerValid()) throw new Refusal("invalid router value"); // runner/src/runner/router.rs `change`
    settle();
    // A store write re-asks the resources that read the store, until quiet.
    for (let k = 0; Store.dirty && k < 4; k++) { Store.dirty = false; for (const r of Resources) if (r.store) r.revise(undo); settle(); }
    Sched?.gates(); // gated tasks over the settled state, inside the rollback (LLP 1092 D8)
  } catch (e) {
    ok = false;
    for (const [n, v] of undo.reverse()) write(n, v);
    if (Now.v !== was) { Now.v = was; for (const o of Now.obs) stale(o, DIRTY); } // nor its time: the clock's readers read as they did
    Resources.forEach((r, k) => r.restore(saved[k])); Mutations.forEach((m, k) => { m.ticket = held[k]; }); Sched?.restore(queued);
    Store.restore(store);
    Out = []; Commands = []; Landed = []; Refused = e;
    say(`refused ${what}: ${e.message}`);
    if (!(e instanceof Refusal)) console.error(e);
    try { settle(); } catch {}
  }
  const [out, cmds, landed] = [Out, Commands, Landed];
  // A key handler's preventDefault/stopPropagation act on its event before the dispatch ends, a tree update a view transition defers too (review C3).
  if (KeyEvent) for (const c of cmds.filter(c => c[0] === "preventDefault" || c[0] === "stopPropagation")) { cmds.splice(cmds.indexOf(c), 1); command(...c); }
  Writes = null;
  unpark();
  const tail = () => { // the tree update; inside a view transition when it may hand on a shared element's name (LLP 1013.000 D7)
    for (const f of Before) f();
    Pres?.before({ ops: [] }, Views); // presence measures what it tracks before the tree changes (LLP 1063)
    try { flush(); } catch (e) { Poisoned = true; say(`poisoned: ${e.pc != null ? `Instance(${e.message})` : e.message}`); console.error(e); Sched?.forget(); return false; } // a trap as the runner's InstanceError (LLP 1090 D6)
    settled(); if (!ok) return Sched?.scan(false), false;
    clock.epoch++; Store.persist();
    for (const go of out) go(); if (Open.size) closeLetGo(); for (const c of cmds) command(...c);
    // An answer's `then` is armed, due now, once however many land: the next advance runs it as its own commit (LLP 1016.001 D3).
    for (const m of landed) if (m.then) { m.due = clock.now; if (!clock.agent) drive(); }
    Sched?.scan(true); // a free queue's waiting send is due (LLP 1092 D3)
    return true;
  };
  return Sh && ok ? Sh.commit(tail, Queue, inflight, After) : tail();
}
/** What runs after each commit's tree update (a loaded piece's publication), before it (the text flow piece puts
 * flowed paragraphs back), and as the clock moves: before each timer or `then` fires, and where an advance lands. */
export const After = [], Before = [], Clocked = [];
/** Authored scroll offsets (`scrollTop`, `scrollLeft`), set once the commit's tree is in place, as the web host's `pendingScrolls`
 * (a virtualized list builds the rows there first: `$jump`, list.js); and a select's committed
 * `$value` once written or its options change, as glue.js `settleValue` (a reader's pick stands until its action). */
const Scrolls = new Map(), Selects = new Set();
/** What a commit does once its tree is in place: authored scrolls, then the
 * loaded pieces' publications (also after a list's report, list.js). */
export function settled() { drain(); markDocument(); Paint?.flush(); Present?.(); for (const f of After) f(); }
let Booting = false; // the boot's own offsets are no reader's scroll (the web host hears none: its input opens after them): `scroll` skips one
function drain() {
  for (const [e, o] of Scrolls) for (const name in o) {
    const at = o[name];
    if (e.$jump) e.$jump(name, at);
    else if (e[name] !== at) { if (Booting) e.$bootScroll = true; if (clock.agent && e.style.scrollBehavior === "smooth") e.scrollTo({ [name === "scrollTop" ? "top" : "left"]: at, behavior: "instant" }); else e[name] = at; }
  }
  // Options compare as nodes and values: a branch that replaces them with equal values still resets the pick.
  Scrolls.clear(); for (const e of Selects) if (!e.isConnected) Selects.delete(e); else { const o = [...e.options], v = o.map(x => x.value); if (e.$set || o.length !== e.$options?.length || o.some((x, i) => x !== e.$options[i] || v[i] !== e.$values[i])) { e.$set = false; e.$options = o; e.$values = v; if (e.value !== e.$value) e.value = e.$value; } }
}
/** An action: each call is one commit. Its arguments conform to its parameters' types (`types`, after `skip` leading
 * arguments: a row action's row), each then within MAX_STRING when `names` names it a string's, or it is refused before
 * its body runs, as the runner's ArgumentType and StringTooLong. `.t(f)` is a handler whose arguments `f` makes inside
 * the commit (a trap refuses it, LLP 1090 D1), then the event's; on a poisoned runner `f` still runs first, as the
 * runner evaluates them before its poisoned check. */
export function act(fn, types, skip = 0, names) {
  const go = a => {
    for (let i = 0; types && i < types.length; i++) {
      if (!conforms(a[i + skip], types[i])) throw new Refusal(`argument ${i + 1} does not conform to its parameter's type`);
      if (names?.[i] && long(a[i + skip])) throw tooLong(names[i]);
    }
    fn(...a);
  };
  const a = (...x) => commit(() => go(x), "action");
  a.t = f => (...v) => {
    if (Poisoned) try { f(); } catch (e) { return say(`refused action: ${e.message}`); }
    return commit(() => go([...f(), ...v]), "action");
  };
  return a;
}
/** The host commands, by name; a loaded piece adds its own (list.js `scrollIntoView`). */
export const Hosts = {
  focus: id => document.getElementById(id)?.focus(),
  blur: id => { const a = document.activeElement; if (a && a !== document.body && (id == null || a.id === id)) a.blur(); }, ...commands(say), // commands.js's: selectText, openURL, postMessage, reload, delivery's; `blur()` drops whatever holds focus; `blur(id)` only when that node holds it (navigation.js runFocusCommands)
  setScheme: s => { document.documentElement.style.colorScheme = s === "system" ? "" : s; },
  copyText: t => navigator.clipboard?.writeText(t), haptic: k => navigator.vibrate?.(k === "selection" ? 5 : 12), /* LLP 1077 D14: vibration where the browser has it */ scrollIntoView: (id, block, inline, behavior) => { const e = document.getElementById(id); if (e) e.scrollIntoView({ block: block ?? "start", inline: inline ?? "nearest", behavior: behavior ?? "auto" }); else say(`scrollIntoView "${id}" refused: no live node with that id`); }, // an element's, by id (minesweeper F3); list.js takes a row's
};
let KeyEvent = null; Hosts.preventDefault = () => { KeyEvent?.preventDefault(); if (KeyEvent?.type === "beforeunload") KeyEvent.returnValue = ""; }; Hosts.stopPropagation = () => { if (KeyEvent) KeyEvent.$stopped = true; }; // the keydown, wheel or beforeunload whose handler is running (`on`): commands run before its commit returns; a stopped key reaches no ancestor's `key` handler, its default still does (files diary F8); a prevented beforeunload is the browser's "Leave site?" (Safari reads `returnValue`)
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
// A commit takes its time first (`time`); its clock readers see it once the body has run (`tick`): the body reads the pre-state.
function time() { if (!clock.agent && !Timing && start) clock.now = Math.max(clock.now, performance.now() - start); }
function tick() { if (Now.v !== clock.now) { Now.v = clock.now; for (const o of Now.obs) stale(o, DIRTY); } }
// A release build never enters agent mode (LLP 1069.007 D2): its build
// writes this false, as the wasm host's files are gated.
const AGENT_ADMITTED = true;
/** A timer: `every(ms, action, once)`, due from mount; `i` its plan order, which breaks ties (schedule.js inserts by it). */
let Order = 0; export const order = () => Order++, Tasks = []; // every task, for the agent's `state.tasks`
export function every(ms, action, once, name) {
  clock.timers.push({ due: clock.now + ms, ms, action, once, name, i: Order++ }); Tasks.push(clock.timers.at(-1));
  if (!clock.agent) drive();
}
// A frame task (LLP 1073): once per presented frame, never caught up; on the
// agent's seekable clock, a virtual frame every 1000/60 ms after it last fired.
// The kth virtual frame after `base`, the product first, as the runner's
// `virtual_frame`: sixty frames are exactly a second.
const vf = (base, k) => base + k * 1000 / 60;
/** `every(frame, action)`. */
export function frames(action, name) {
  clock.timers.push({ due: vf(clock.now, 1), base: clock.now, k: 1, frame: true, action, name, i: Order++ }); Tasks.push(clock.timers.at(-1));
  if (!clock.agent) paint();
}
/** Move the clock to `to`, firing each due timer and armed `then` at its own time, in order; a seek fires frame
 * tasks' virtual frames too, the wall clock's (`wall`) none. `stop()`, asked after each, ends it there (the agent's:
 * one that sent a request): true. A refusal, or 4096 commits (TIMER_FIRE_LIMIT), stops it at that time, and a
 * non-finite `to` (NonFiniteClock) leaves the clock where it was: its journal line, as the runner's error. Under the
 * agent the journal gets the runner's line for an advance that fired. `timers` false fires none: the armed `then`s
 * alone, at `to` = now, as an agent's input ends (Runner::land_then; trivia F3). */
export function advance(to, wall, stop, timers = true) {
  if (!Number.isFinite(to)) return say(`refused advance: NonFiniteClock (${to})`), journal.at(-1);
  let fired = 0, stopped = false;
  for (;;) {
    let next = null, then = null, head = null;
    if (timers) for (const t of clock.timers) if (t.due <= to && !(wall && t.frame) && (!next || t.due < next.due)) next = t;
    // An answer's `then` goes before a timer due at the same time: the answer landed first; a queue's `next` between them (LLP 1092 D3).
    for (const m of Mutations) if (m.due <= to && (!then || m.due < then.due) && (!next || m.due <= next.due)) then = m;
    for (const m of Mutations) if (m.next <= to && (!head || m.next < head.next) && (!next || m.next <= next.due) && (!then || m.next < then.due)) head = m;
    if (head) then = null;
    if (!next && !then && !head) break;
    if (fired === 4096) return say("refused advance: 4096 commits in one advance (TIMER_FIRE_LIMIT)"), journal.at(-1);
    if (head) clock.now = Math.max(clock.now, head.next);
    else if (then) { clock.now = Math.max(clock.now, then.due); then.due = Infinity; }
    else { clock.now = next.due; if (next.once) clock.timers.splice(clock.timers.indexOf(next), 1); else next.due = next.frame ? vf(next.base, ++next.k) : next.due + next.ms; }
    if (fire(head ? () => Sched.next(head) : then ? () => commit(then.then, `${then.name} then`) : next.action) !== true) return journal.at(-1);
    fired++;
    if (stop?.()) { stopped = true; break; }
  }
  if (!stopped) clock.now = Math.max(clock.now, to);
  for (const f of Clocked) f();
  // Under the agent only: this journal is not a ring, and a page's own
  // clock would add a line a tick.
  if (fired && clock.agent) say(`advance → ${fired} timer${fired === 1 ? "" : "s"} fired, epoch ${clock.epoch}`);
  return stopped;
}
function fire(f) { for (const c of Clocked) c(); Timing = true; try { return f(); } finally { Timing = false; } }
let driving = 0, start = 0, painting = 0;
export function drive() {
  clearTimeout(driving);
  let next = Infinity;
  for (const t of [...clock.timers, ...Mutations]) if (!t.frame && t.due < next) next = t.due;
  for (const m of Mutations) if (m.next < next) next = m.next;
  if (!isFinite(next)) return;
  driving = setTimeout(() => { advance(performance.now() - start, true); drive(); }, Math.max(0, next - (performance.now() - start)));
}
// Presented frames: before each paint, timers due by the frame's time, then
// every frame task once at it (Runner::frame).
export function paint() {
  if (painting || typeof requestAnimationFrame !== "function") return;
  painting = requestAnimationFrame(function frame(ts) {
    painting = 0;
    if (clock.agent || !clock.timers.some(t => t.frame)) return;
    painting = requestAnimationFrame(frame);
    advance(Math.max(clock.now, ts - start), true);
    const at = clock.now, rev = Rev, ticket = Ticket;
    NowRead = false;
    // A frame task's commit may arm or drop one (a gate, LLP 1092 D10): each armed at the frame's start fires once.
    for (const t of clock.timers.filter(t => t.frame)) if (clock.timers.includes(t)) { t.base = at; t.k = 1; t.due = vf(at, 1); fire(t.action); }
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
    inflight.n++; t.waiting = true;
    const started = performance.now();
    // A stream's message (`more`) keeps its ticket; anything else ends it.
    const done = (o, more) => {
      if (t.closed) return;
      if (t.waiting) { t.waiting = false; inflight.n--; }
      t.elapsed = Math.max(0, Math.round(performance.now() - started));
      if (more) { t.messages++; t.coalesced += (o.streamed ?? o).coalesced ?? 0; o.more = true; } else Open.delete(t);
      say(`${more ? "message" : "reply"} ${t.id}; wall ${t.elapsed} ms`); land(o);
    };
    const failed = e => done({ failed: 1, message: String(e?.message ?? e) });
    // An answer that keeps coming (LLP 1016.000): a Rust source's streamed request, or a TypeScript
    // source's `exactStream` (ts-data.js). It is in flight until its first message (D5), then open
    // until its end or until its ticket is let go (`Open`, after each commit).
    if (t.stream || t.req?.stream) {
      t.ctl = new AbortController(); t.messages = t.coalesced = 0; Open.add(t);
      (t.stream ? t.stream(o => done(o, true), t.ctl) : data.fetch(t.req, m => done({ streamed: m }, true), t.ctl)).then(done, failed);
    } else if (t.req) data.fetch(t.req).then(done, failed);
    else t.promise.then(v => done({ v }), e => done({ error: String(e?.message ?? e) }));
  });
}
/** Open streams: one whose ticket its resource or mutation let go (new arguments, `refresh`, a failure,
 * its region ended) is closed once the commit that let it go stands, as the runner's forget path (D2). */
const Open = new Set();
function closeLetGo() {
  for (const t of Open) if (!t.held() || t.r?.gone) {
    Open.delete(t); t.closed = true; t.ctl.abort();
    if (t.waiting) { t.waiting = false; inflight.n--; }
    say(`close stream ${t.id}: its ticket was let go`);
  }
}
function ask(source, args, name) {
  const a = data.reserved?.[source] ? { v: data.reserved[source](source, args, name) } : data.answer(source, args, Store, name);
  return a && a.then ? { promise: a } : a;
}
/** The reply to ticket `t` while its target `held()` it: a commit in which `f` takes the source's answer (a TypeScript
 * promise's value, or the parse of the outcome). A data or shape refusal there (no answer, or one outside its shape) lets
 * the ticket go in a commit of its own, as the runner's `release_failed` (admission.rs): `gone` takes it out of pending. */
function reply(t, name, source, held, f, next, gone) {
  t.held = held;
  return o => {
    if (commit(() => {
      if (!held()) return say(`dropped reply for ${name}: ticket ${t.id} is no longer held`);
      let p;
      try { if (o.error !== undefined) throw new Failed(o.error); p = o.v !== undefined ? { v: o.v } : data.parse(source, t.args, o, Store); }
      catch (e) { throw e instanceof Failed ? e : new Failed(String(e?.message ?? e)); }
      f(p, o);
    }, `${o.more ? "message" : "reply"} ${name}; wall ${t.elapsed} ms`) !== false || !(Refused instanceof Failed) || !held()) return;
    say(`request ${t.id} (${name}) failed and is no longer pending: ${next}`);
    gone(); commit(() => {}, "a failed request");
  };
}
const revalidated = (name, same) => `${name} answered: ${same ? "equal to its build-time answer" : "replaces its build-time answer"}`; // runner lines.rs
/** A resource: its value, the arguments it settled with, one ticket in flight. A bake's answer is a first frame: `baked` asks at launch, as a native runner at `data_ready` (LLP 1048.003 D6; feed F24); a document's `kept` was asked for its page. */
export function res(name, source, args, initial, initialArgs, type, ph, carried = false) { // `carried`: settled, not a bake to ask again — a dev reload's (checkpoint.js), an `else` row's (emit.rs)
  const ver = sig(0), pend = sig(false), fail = sig(null);
  const kept = checkpoint().kept?.get(name);
  if (kept) [initialArgs, initial] = kept;
  const r = { name, source, type, value: initial, settled: initialArgs, baked: !kept && !carried && initialArgs !== undefined, ticket: null, forced: false, reread: false, rev: false, store: false };
  const flag = (s, v, undo) => { if (!eq(s.n.v, v)) { undo?.push([s.n, s.n.v]); write(s.n, v); } };
  // Nothing kept: the placeholder shows, pending (LLP 1048.003 D6).
  const hold = () => {
    if (r.value !== undefined) return;
    const v = typeof ph === "function" ? ph() : ph;
    if (v === undefined) throw new Failed(`${name} answers later and has nothing to show; give it an \`else\``);
    r.value = v;
  };
  const take = (v, a) => {
    if (type && !conforms(v, type, [0], r.checked)) throw new Failed(`${name}: the answer does not conform to its shape`);
    r.checked = v;
    r.value = v; r.settled = a;
  };
  // A reply the source cannot take leaves the value, failed for its arguments (`r.failed`, the runner's `failed_args`).
  // A stream's message (`o.more`) is a settlement that keeps the ticket (LLP 1016.000 D1); a message
  // that re-asks (a cursor across a gap) is a new ticket, the old one closed with the commit (`Open`).
  const land = t => reply(t, name, source, () => r.ticket === t, (p, o) => {
    if (p.req) { if (o.more) { const n = { id: ++Ticket, args: t.args, req: p.req, r }; r.ticket = n; send(n, land(n)); } else { t.req = p.req; t.id = ++Ticket; send(t, land(t)); } return; }
    if (t.baked) say(revalidated(name, eq(p.v, r.value))); t.baked = false; take(p.v, t.args); r.failed = null; if (!o.more) r.ticket = null;
    W(pend, false); W(fail, null); W(ver, ver.n.v + 1);
  }, "it keeps its last value", () => { r.ticket = null; r.failed = t.args; write(pend.n, false); write(fail.n, t.args); });
  const m = memo(() => {
    ver();
    const a = args();
    const forced = r.forced, reread = r.reread, rev = r.rev;
    r.forced = r.reread = r.rev = false;
    // A failure keeps the value for its arguments, asking nothing; `refresh` or new ones ask again (settlement.rs). `fail` follows.
    if (r.failed && (forced || !equal(a, r.failed))) r.failed = null;
    flag(fail, r.failed);
    if (r.failed) return r.value;
    const baked = r.baked; r.baked = false;
    if (!forced && !reread && !rev) {
      // Arguments compare as the runner's do (`equal`: `-0` is `0`, NaN asks again); a bake is asked once anyway (LLP 1048.003 D6).
      if (r.settled !== undefined && equal(a, r.settled) && !baked) return r.value;
      if (r.ticket && equal(a, r.ticket.args)) return r.value;
    }
    let ans;
    try { ans = ask(source, a, name); }
    catch (e) {
      if (e instanceof Refusal) throw e;
      if (e.refuse) throw new Failed(`resource ${name}: ${e.message}`);
      r.failed = a; flag(fail, a); say(`resource ${name} failed: ${e.message}`); return r.value;
    }
    if (ans && ans.store) r.store = true;
    if (ans && "v" in ans) {
      if (baked) say(revalidated(name, eq(ans.v, r.value))); take(ans.v, a);
      if (r.ticket && !reread) { say(`forget ticket ${r.ticket.id} (${name})`); r.ticket = null; }
      if (!r.ticket) flag(pend, false);
      return r.value;
    }
    // A declared refresh at a send re-reads: a request is discarded, and one in flight stays.
    if (reread) return r.value;
    if (ans && ans.req && !forced && !rev && r.ticket?.req && !equal(a, r.ticket.args) && sameReq(ans.req, r.ticket.req)) {
      say(`keep request ${r.ticket.id} (${name}): the same request for newer arguments`);
      r.ticket.args = a;
      return r.value;
    }
    if (ans && (ans.req || ans.promise || ans.stream)) {
      hold();
      const t = { id: ++Ticket, args: a, req: ans.req, promise: ans.promise, stream: ans.stream, baked, r }; if (baked) say(`${name} shows its build-time answer until its source answers`);
      if (r.ticket) say(`forget ticket ${r.ticket.id} (${name})`);
      r.ticket = t; flag(pend, true); send(t, land(t));
      return r.value;
    }
    // Not ready (Rust loads after first paint): the bake's answer for its arguments stands, not pending, and is asked at `ready` as a
    // native runner asks at data_ready (review B3); another compiled value stands, stale, asked again, forced, when it is (LLP 1027 D4).
    const shown = baked && eq(a, r.settled);
    if (!shown) { hold(); flag(pend, true); }
    if (!r.waiting) { r.waiting = true; data.ready(() => { r.waiting = false; commit(() => { r.forced = true; r.baked ||= shown; W(ver, ver.n.v + 1); W(pend, false); }, `data ready ${name}`); }); }
    return r.value;
  }, type);
  Object.assign(r, {
    save: () => [r.value, r.settled, r.ticket, r.ticket?.args, r.store, r.failed, r.baked],
    restore: x => { [r.value, r.settled, r.ticket] = x; if (r.ticket) r.ticket.args = x[3]; r.store = x[4]; r.failed = x[5]; r.baked = x[6]; },
    force: undo => { r.forced = true; flag(ver, ver.n.v + 1, undo); },
    reread_: undo => { r.reread = true; flag(ver, ver.n.v + 1, undo); },
    revise: undo => { r.rev = true; flag(ver, ver.n.v + 1, undo); },
  });
  onEnd(() => { r.gone = true; }); // its region ended: an open stream closes (`Open`)
  Resources.push(r);
  m.p = () => (m(), pend());
  m.f = () => (m(), fail() != null);
  m.r = r;
  return m;
}
/** A mutation (LLP 1016): its slot, resources, and newest-winning ticket — or, a `queue` (LLP 1092), one ticket
 * and the sends waiting their turn (`wait`, schedule.js), each asked at its `next`. */
export const Mutations = [];
export function mut(name, slot, refreshes, type, queue) {
  const pend = sig(false);
  const m = { name, ticket: null, then: null, due: Infinity, next: Infinity, queue: !!queue, pend };
  Mutations.push(m);
  slot.n.m = m;
  // An answer now lands before the action's own writes, as the runner's do: an assignment in the same action wins.
  const landWrite = (v, undo) => { if (!Writes.some(w => w[0] === slot.n)) { slot.n.landing = 1; try { undo.push([slot.n, slot.n.v]); write(slot.n, v); } finally { slot.n.landing = 0; } } Landed.push(m); };
  // A reply the source cannot take ends it unsent: its slot as it was, its `then` unarmed (`reply`).
  const land = t => reply(t, name, t.source, () => m.ticket === t, (p, o) => {
    if (p.req) { if (o.more) { const n = { id: ++Ticket, source: t.source, args: t.args, req: p.req }; m.ticket = n; send(n, land(n)); } else { t.req = p.req; send(t, land(t)); } return; }
    if (type && !conforms(p.v, type, [0], m.checked)) throw new Failed(`${name}: the answer does not conform to its shape`);
    m.checked = p.v;
    if (!o.more) m.ticket = null; W(pend, !!m.wait?.length);
    slot.n.landing = 1; W(slot, p.v); Landed.push(m);
    for (const r of refreshes) R(r.r);
    queueMicrotask(() => { slot.n.landing = 0; });
  }, "it ends unsent", () => { m.ticket = null; write(pend.n, !!m.wait?.length); });
  Object.assign(m, {
    // A queue's assignment forgets nothing (LLP 1092 D4).
    forget(undo) { if (m.ticket && !m.wait) { say(`forget ticket ${m.ticket.id} (${name})`); m.ticket = null; undo.push([pend.n, pend.n.v]); write(pend.n, false); } },
    send(source, args, undo, own) {
      // A queue's send waits unless the mutation is free; `own` is a `next`'s, whose ask refusing drops it (LLP 1092 D3).
      if (m.wait && !own && Sched.hold(m, source, args, undo)) return;
      let a;
      try {
        a = ask(source, args, name);
        if (!a || !("v" in a || a.req || a.promise || a.stream)) throw new Refusal(`${name}: its source is not ready`);
        if ("v" in a && type && !conforms(a.v, type, [0], m.checked)) throw new Refusal(`${name}: the answer does not conform to its shape`);
      } catch (e) { if (own) Sched.own = e; throw e; }
      if ("v" in a) {
        m.checked = a.v;
        landWrite(a.v, undo);
        if (m.wait && pend.n.v !== !!m.wait.length) { undo.push([pend.n, pend.n.v]); write(pend.n, !!m.wait.length); } // a queue: pending while one waits
      } else {
        const t = { id: ++Ticket, source, args, req: a.req, promise: a.promise, stream: a.stream };
        m.ticket = t; undo.push([pend.n, pend.n.v]); write(pend.n, true); send(t, land(t));
      }
      // Answered at once, it has landed: what it changes is forced, as a reply's landing forces it (a re-read
      // drops a source that answers later, and no reply would come to ask again; runner commit.rs `landed_now`).
      for (const r of refreshes) if ("v" in a) R(r.r); else r.r.reread_(undo);
    },
  });
  m.p = () => pend();
  return m;
}
export function M(m, source, args, own) { Sends.push([m, source, args, own]); }
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
  if (attrs?.["aria-keyshortcuts"] != null) input();
  if (Adopt) { const e = adopt(p, tag, cls, attrs); if (tag === "video" || tag === "audio") media(e, attrs); return e; }
  const e = ns ? document.createElementNS(ns, tag) : document.createElement(tag);
  if (cls !== 0) e.setAttribute("class", "c" + cls);
  if (attrs) { for (const k in attrs) e.setAttribute(k, rel(k, attrs[k])); if ("data-scrolldocument" in attrs) Docs.add(e); }
  if (text !== 0) e.textContent = text;
  p.append(e);
  if (tag === "video" || tag === "audio") media(e, attrs); // an `audio` is the same media host (LLP 1042 §8)
  Paint?.list(e); return e;
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
// A page rendered ahead (by the Rust render host or by this runtime under
// Bun) is adopted, not rebuilt: construction walks the document with a
// cursor per parent, takes each element whose tag matches, gives it the
// class it would have had and drops the renderer's view ids, and inserts
// the region anchors a fresh build would have made. A
// mismatch abandons adoption and builds afresh (LLP 1048.000 D6).
let Adopt = false;
class Mismatch extends Refusal {}
// The next node to adopt under `p`; insertions go before it and never move it.
const at = p => (p.$n === undefined ? (p.$n = p.firstChild) : p.$n);
function adopt(p, tag, cls, attrs) {
  let e = at(p);
  while (e && e.nodeType !== 1) e = e.nextSibling;
  if (!e || e.localName.toLowerCase() !== tag.toLowerCase()) throw new Mismatch(`adoption: expected <${tag}>, found ${e ? "<" + e.localName + ">" : "nothing"}`);
  p.$n = e.nextSibling;
  e.$paintWaiting = false; e.$paintFacts = null; e.removeAttribute("data-exact-paint");
  // The renderer's inline style stays: it is the class's declarations and
  // the live rows, which the node's style bindings rewrite as they change.
  if (e.hasAttribute("data-view")) e.removeAttribute("data-view");
  if (attrs?.["data-exact-own-isolation"] === undefined && !e.hasAttribute("data-exact-own-isolation") && !e.hasAttribute("data-exact-policy") && e.style.isolation) e.style.removeProperty("isolation");
  if (cls !== 0 && e.getAttribute("class") !== "c" + cls) e.setAttribute("class", "c" + cls);
  if (attrs) { for (const k in attrs) { const v = rel(k, attrs[k]); if (e.getAttribute(k) !== v) e.setAttribute(k, v); } if ("data-scrolldocument" in attrs) Docs.add(e); }
  Paint?.list(e); return e;
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
const BOOL = /^(disabled|readonly|inert|checked|multiple|autoplay|controls|loop|muted|playsinline|disablepictureinpicture|disableremoteplayback)$/;
/** A loaded piece's own handling of a prop (symbols.js's `src`): true when handled. */
export const PropHooks = {};
/** A dynamic prop, by the DOM name the live host uses (`applyProps`). */
const navigable = v => { try { return ["http:", "https:", "mailto:", "tel:"].includes(new URL(v, document.baseURI).protocol); } catch { return false; } };
export function P(e, name, f) {
  if (name === "aria-keyshortcuts") input();
  if (name === "data-scrolldocument") Docs.add(e);
  effect(() => {
    let v = f();
    v = rel(name, v == null ? null : typeof v === "boolean" ? String(v) : String(v));
    // A URL that navigates is written only if the web host's policy takes it
    // (navigation.js `navigableURL`: http, https, mailto, tel): a refused
    // link loses its `href`, an iframe shows about:blank.
    if (v != null && (name === "href" || (name === "src" && e.localName === "iframe")) && !navigable(v)) v = name === "src" ? "about:blank" : null;
    if (PropHooks[name]?.(e, v)) return;
    if (e.$media) mediaProp(e, name, v); // media.js: `paused`, `volume`, `currentTime` … are the glue's
    if (name === "text") { if (!e.childElementCount && e.textContent !== (v ?? "")) e.textContent = v ?? ""; }
    else if (name === "value") { if (e.localName === "select") { Selects.add(e); e.$value = v ?? ""; e.$set = true; } if (e.value !== (v ?? "")) e.value = v ?? ""; }
    else if (name === "scrollTop" || name === "scrollLeft") { if (v != null) (Scrolls.get(e) ?? Scrolls.set(e, {}).get(e))[name] = Number(v); }
    else if (BOOL.test(name)) { e.toggleAttribute(name, v === "true"); if (name === "disabled" && v === "true" && document.activeElement === e && e.matches(":disabled")) e.blur(); /* HTML focus fixup, now, for a control only (glue.js) */ if (name === "checked") e.checked = e.$checked = v === "true"; if (name === "muted") e.muted = v === "true"; }
    else if (v == null) { if (e.hasAttribute(name)) { e.removeAttribute(name); if (name.startsWith("data-exact-")) Paint?.facts(e); } }
    else if (e.getAttribute(name) !== v) { e.setAttribute(name, v); if (name.startsWith("data-exact-")) Paint?.facts(e); }
    if (e.localName === "a" && (name === "target" || name === "href" && (!e.hasAttribute("target") || e.rel === "external noopener"))) { const out = name === "href" && v != null && /^\s*(https?:)?\/\//i.test(v), t = name === "target" ? v : out ? "_blank" : null; if (t) e.setAttribute("target", t); else e.removeAttribute("target"); if (t === "_blank") e.rel = out ? "external noopener" : "noopener"; else e.removeAttribute("rel"); } // a link to an absolute URL leaves the app in a new browsing context unless its `target` is authored (element.rs `leaves_app`, `props_of`; chat F11)
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
/** When native code may load (LLP 1024 D7): after the browser's first paint entry, or two frames and 250 ms where it records none (glue.js `afterNativePaint`). */
let Painted = null, Native = null;
export const painted = () => Painted ??= new Promise(r => { let o; const done = () => { o?.disconnect(); r(); }; try { o = new PerformanceObserver(() => requestAnimationFrame(done)); o.observe({ type: "paint", buffered: true }); } catch {} requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(done, 250))); });
/** A native module's element (LLP 1024 D3): the real custom element, empty until the web host's adapter (`native.js`)
 * and the app's module artifact load (`painted`); the module renders into it, its events reaching the handlers as `exact-native` events (`on`). */
export function nm(e) {
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  const id = viewId(e), st = e.exactNative = { id, name: e.localName, state: "loading", status() { return { name: this.name, state: this.state, ...(this.error ? { error: this.error } : {}) }; } };
  onEnd(() => { st.destroyed = true; Native?.then(h => h.destroy(e), () => {}); });
  // In flight until the module is attached and its first events are in (`clock settle` waits for them).
  inflight.n++;
  (Native ??= painted().then(() => import("./native.js")).then(m => m.viewHost(say)))
    .then(h => { h.attach(e); return h.loaded; }, err => { st.state = "unavailable"; st.error = String(err?.message ?? err); say(`native ${st.name} #${id}: unavailable: ${st.error}`); })
    .finally(() => setTimeout(() => inflight.n--));
}
export function S(e, prop, unit, f) { let rendered = Adopt; effect(() => { css(e, prop, unit, f(), rendered); rendered = false; }); }
let Scratch = null;
const Normal = new Map(), same = v => v.replace(/\btransparent\b/g, "rgba(0, 0, 0, 0)");
function normal(prop, t) {
  const k = prop + "\0" + t;
  let v = Normal.get(k);
  if (v === undefined) { const s = Scratch ??= document.createElement("i").style; s.removeProperty(prop); s.setProperty(prop, t); Normal.set(k, v = same(s.getPropertyValue(prop))); if (Normal.size > 4096) Normal.clear(); }
  return v;
}
function withoutLines(s) { let o = "", bracket = 0; for (let i = 0; i < s.length; i++) { if (s[i] === "\\") { if (!bracket) o += s[i]; if (++i < s.length && !bracket) o += s[i]; } else if (s[i] === "[") bracket++; else if (s[i] === "]" && bracket) bracket--; else if (!bracket) o += s[i]; } return o; }
function trackCount(s) { let count = 0; for (let i = 0; i < s.length;) { while (/\s/.test(s[i])) i++; if (i >= s.length) break; if (s[i] === "[") { for (i++; i < s.length && s[i] !== "]"; i += s[i] === "\\" ? 2 : 1); i++; continue; } const start = i; let depth = 0; for (; i < s.length; i++) { if (s[i] === "(") depth++; else if (s[i] === ")") depth--; else if (!depth && /\s/.test(s[i])) break; } const part = s.slice(start, i); if (/^repeat\(/i.test(part)) { const comma = part.indexOf(","), n = part.slice(7, comma).trim(); count += (/^\d+$/.test(n) ? Number(n) : 1) * trackCount(part.slice(comma + 1, -1)); } else count++; } return count; }
export function gridValue(kind, value) {
  if (value == null) return value;
  const prop = kind === "tracks" ? "grid-template-columns" : kind === "placement" ? "grid-column" : kind === "flow" ? "grid-auto-flow" : "justify-items", v = normal(prop, String(value));
  const refuse = why => { say(`unset ${prop}: ${JSON.stringify(value)} (${why})`); return null; };
  if (!v) return refuse("the browser rejected it");
  const lower = v.toLowerCase(), wide = /^(?:inherit|initial|unset|revert|revert-layer)$/;
  if (wide.test(lower)) return refuse("CSS-wide values have no kernel cascade");
  if (kind === "tracks") { const sizes = withoutLines(lower); if (/^subgrid(?:\s|\[|$)/.test(lower) || /\b(?:calc|min|max|clamp|var|env)\s*\(/.test(sizes)) return refuse("Taffy has no value for it"); for (const m of sizes.matchAll(/(?:^|[^\w.-])(?:\d*\.)?\d+([a-z]+)\b/g)) if (!/^(?:px|fr)$/.test(m[1])) return refuse("Taffy has no value for its unit"); if (trackCount(lower) > 10000) return refuse("Taffy supports at most 10000 explicit tracks"); }
  if (kind === "placement" && lower.split(/[ \/]+/).some(x => /^-?\d+$/.test(x) && Math.abs(Number(x)) > 10000)) return refuse("Taffy supports grid indexes and spans through 10000");
  if (kind === "justify" && /^(?:last baseline|legacy(?: (?:left|right|center))?|(?:left|right|center) legacy)$/.test(lower)) return refuse("Taffy has no such alignment mode");
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
export { svgTransform } from "./svg-transform.js"; export { ds } from "./dataset.js"; export { hk } from "./hooks.js"; export { pf } from "./perf.js";
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
  if (e.$media && MEDIA_EVENTS.has(kind)) return mediaOn(e, kind, f); // media.js: the glue's reports
  // A module view hears its module's events, and the page's own input as any element does (glue.js `attach`): a click is its press.
  if (e.exactNative) { l("exact-native", ev => { if (ev.detail.kind === kind) f(...(ev.detail.value == null ? [] : [ev.detail.value])); }); if (kind === "message") return; }
  switch (kind) {
    // A link with a press is the app's navigation: the browser's is prevented. A modified or other-button click, a `target` or `download`, is the browser's alone and the press does not run, with a router or without (`router`, input-glue.js).
    case "press": if (!e.matches("button, a[href], input, select, textarea, summary")) input(); /* the input piece presses it by key (input-glue.js `pressesByKey`) */ return l("click", ev => { const a = ev.target.closest?.("a[href]"); if (a && a !== e && e.contains(a)) return; if (e.localName === "a" && (ev.button || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey || (e.target && e.target !== "_self") || e.hasAttribute("download"))) return; ev.stopPropagation(); if (e.localName === "a") ev.preventDefault(); f([ev.shiftKey, ev.ctrlKey, ev.altKey, ev.metaKey]); }); // a press action taking one more parameter hears the MouseEvent's modifiers (gallery F20)
    // A checkbox's value is whether it is checked, a radio's its `value`; the
    // platform moves the control at once, and an action that refuses snaps
    // the box or the radio group back (glue.js, navigation.js). A host's
    // change carries its own text (files.js: a picker's lines, which an
    // input's value would flatten). A range's is a number (the events table).
    // An action taking one more parameter hears the `InputEvent` (x2apps
    // codeedit #2, survey #2).
    case "change": case "input": return l(kind, ev => {
      if (ev instanceof CustomEvent) return f(ev.detail);
      const box = e.type === "checkbox", radio = e.type === "radio", v = box ? e.checked : e.type === "range" ? Number(e.value) : e.value;
      f(v, inputRecord(e, box || radio ? e.value : String(v), box ? e.checked : radio));
      if (box && e.$checked !== undefined && e.checked !== e.$checked) e.checked = e.$checked;
      if (radio) settleRadios(e, r => r.$checked);
    });
    case "hover": l("pointerenter", () => f(true)); return l("pointerleave", () => f(false));
    case "key": return l("keydown", ev => { if (ev.$stopped) return; const outer = KeyEvent; KeyEvent = ev; try { f(ev.key, [ev.key, ev.shiftKey, ev.ctrlKey, ev.altKey, ev.metaKey]); } finally { KeyEvent = outer; } }); // it bubbles to every ancestor's handler; an action taking one more parameter hears the KeyboardEvent record too (contract/types selection.rs's order)
    // The window's, heard by every connected element that declares it (studio diary R17).
    case "beforeunload": return addEventListener("beforeunload", ev => { if (!e.isConnected) return; const outer = KeyEvent; KeyEvent = ev; try { f(); } finally { KeyEvent = outer; } });
    case "submit": return l("keydown", ev => { if (ev.key === "Enter" && !ev.isComposing && !ev.$submit) { ev.$submit = true; addEventListener("keydown", w => { if (w === ev && !ev.defaultPrevented) setTimeout(f); }, { once: true }); } }); // Enter's default: after every `key` handler on the path (the window's listener is last), unless one prevented it, and after the browser's own default, HTML's `change` on Enter (gallery F26)
    // Only from the origin of the src the app committed (glue.js
    // `guestMessageAuthorized`, LLP 1020 D2): a guest that navigated away is
    // not heard; an opaque sandbox's origin is "null".
    case "message": return addEventListener("message", ev => { if (ev.source === e.contentWindow && ev.origin === guestOrigin(e)) f(typeof ev.data === "string" ? ev.data : JSON.stringify(ev.data)); });
    // The port's offsets, as the web host sends them (`glue.js` `attach`); an action taking one more parameter hears the `ScrollEvent` record.
    case "scroll": return l(kind, () => { if (e.$bootScroll) { e.$bootScroll = false; return; } f(e.scrollLeft, e.scrollTop, [e.scrollLeft, e.scrollTop, e.scrollWidth, e.scrollHeight, e.clientWidth, e.clientHeight]); });
    // Pull to refresh is a native port's; the web has none (`glue.js` attaches nothing).
    case "refresh": return;
    case "dblclick": return l(kind, ev => { ev.preventDefault(); f(); }); case "pointerdown": case "pointerup": case "pointermove": return pointer(e, kind, f); // pointer.js (LLP 1005 §Events, 1056 §3)
    // UI Events' `contextmenu` is a PointerEvent: where the secondary click was (studio diary R22).
    // A field's own edit menu stays the browser's; the nearest handler alone hears it, as on the wasm host and macOS (review b5-b 3).
    case "contextmenu": return l(kind, ev => { if (ev.target.closest("input,textarea,[contenteditable]")) return; ev.preventDefault(); ev.stopPropagation(); f(record(e, ev)); });
    // DOM's own, bubbling to every ancestor's handler; one that calls `preventDefault()` keeps the scroll (a pinch is a Control-held wheel) from happening (studio diary R3).
    case "wheel": return e.addEventListener("wheel", ev => { if (e.matches(":disabled") || e.closest("[inert]")) return; const outer = KeyEvent; KeyEvent = ev; try { f([...record(e, ev).slice(0, 2), ev.deltaX, ev.deltaY, ev.deltaMode, ev.shiftKey, ev.ctrlKey, ev.altKey, ev.metaKey]); } finally { KeyEvent = outer; } }, { passive: false });
    // Files dropped from outside, each a `doc:` handle (files.js, documents-glue.js; studio diary R19).
    case "drop": return OnHooks.drop?.(e, f);
    // Chrome blurs an element it is removing (still connected); a retired view's blur is dropped (glue.js).
    case "blur": return l(kind, () => queueMicrotask(() => e.isConnected && f())); case "copy": case "cut": case "paste": return l(kind, ev => { ev.stopPropagation(); f([ev.clipboardData?.getData("text/plain") ?? ""]); }); // the nearest handler hears the ClipboardEvent record; the default (a field's own paste) proceeds
    case "selectionchange": return onSelection(e, (text, a, b) => f([text, a, b])); // its part of the page's selection, the `Selection` record (navigation.js)
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
let Pres = null, Presence = null, Present = null, Leave = null, Sh = null; // Sh: shared.js, loaded with presence-glue.js, runs the commits that hand on a shared element's name in a view transition (LLP 1013.000 D7)
/** The after-paint pieces on their way (the agent waits for them before an
 * operation, as glue.js's `agentSettled` waits for `pieces.pending()`). */
export const pieces = () => Promise.all([Motion, Inputs, Presence, Flow, Native, mediaPiece()].filter(Boolean)).then(() => {}, () => {});
/** A view leaves with the exit animation `css` names (a virtualized list's
 * row wrapper, list.js): whether it stays, leaving, for presence-glue.js to remove. */
export function exitView(el, css) { if (!Pres || !css) return false; Pres.exit(el, css); return exiting(el); }
const Created = [];
export function pr(e) {
  Created.push(e);
  if (Presence || typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  Presence = new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))).then(() => { globalThis.exact ??= {}; return Promise.all([import("./presence-glue.js"), import("./shared.js").then(m => { Sh = m; })]); })
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
export function onReorder(e, l) {
  const id = viewId(e);
  e.$reorderList = l; e.dataset.view = id;
  onEnd(() => motion(m => m.gone(id)));
  motion(m => m.reorderHandle(e));
}
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
/** `resize=action`, the element resize event: the browser's ResizeObserver
 * (the web host's own resize-glue.js, fetched after first paint by a plan
 * with one), the action hearing the content box's width and height, then
 * its `DOMRectReadOnly` record. */
let Resizes = null;
export function onResize(e, f) {
  if (typeof ResizeObserver !== "function" || globalThis.__exactRender) return;
  inflight.n++;
  (Resizes ??= new Promise(r => requestAnimationFrame(() => r())).then(() => import("./resize-glue.js")))
    .then(m => m.observeResize(e, r => { if (e.isConnected) f(r.width, r.height, [r.x, r.y, r.width, r.height, r.top, r.right, r.bottom, r.left]); }))
    .catch(err => say(`resize: ${err.message}`)).finally(() => inflight.n--);
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
/** `elementFromPoint(x, y)` (LLP 1094 D10): the `id` nearest the front-most
 * of the same untransformed boxes at the viewport point; `none` until the
 * reader has loaded. */
export const x_elementFromPoint = (x, y) => { const root = document.getElementById("exact-root"), n = Geo?.point(x, y)?.closest("[id]"); return n && n !== root && root.contains(n) && n.id || null; };
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
/** `select`: the editor's facts at each selection change (runner Event::Select);
 * a text field's own, HTML's, its `InputEvent` (x2apps codeedit #2). */
export function onSelect(e, f) { e.$select = f; e.addEventListener("select", () => { if (textField(e)) f(inputRecord(e, e.value, false)); }); }
/** The `InputEvent` record (contract/types selection.rs's order): the value,
 * whether checked, and a text field's selection (UTF-16, the DOM's); a type
 * with none has its caret after its text, a control 0, 0, `none`. */
function inputRecord(e, value, checked) {
  const n = e.value?.length ?? 0, field = textField(e);
  return [value, checked, field ? e.selectionStart ?? n : 0, field ? e.selectionEnd ?? n : 0, field ? e.selectionDirection ?? "none" : "none"];
}
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
      velocity: { sample: (...a) => Mo?.pan.sample(...a), velocity: (...a) => Mo?.pan.velocity(...a) }, log: say });
  }).catch(err => say(`input: ${err.message}`)).finally(() => inflight.n--);
}
/** A `head` (LLP 1048.003 D1): its node, as the kernel keeps it, an inert
 * element in the tree, and its fields, the page's while it is the innermost
 * active head (document.js, as runner/src/head.rs). */
export function hd(p, fields) {
  const t = document.createElement("template");
  if (Adopt) p.insertBefore(t, at(p)); else p.append(t);
  onEnd(head(t, fields, effect, After));
}

function range(p) {
  if (Adopt) return [mark(p), null];
  const a = document.createComment(""), b = document.createComment("");
  p.append(a, b);
  return [a, b];
}
// A view leaving with its exit animation stays where it was until it ends
// (presence-glue.js removes it): never moved, since moving cancels a CSS animation.
const exiting = n => n.nodeType === 1 && n.hasAttribute("data-exiting");
function clear(a, b) { Paint?.list(a.parentNode); for (let n = a.nextSibling; n !== b;) { const m = n.nextSibling; if (!exiting(n)) Leave ? Leave(n, b) : n.remove(); n = m; } }
function build(b, f, own) {
  const frag = document.createDocumentFragment();
  const s = scope(() => f(frag), own);
  b.before(frag); Paint?.list(b.parentNode);
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
  }).$r = () => [a, b]; // the region's range, for shared.js
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
  }).$r = () => [a, b]; // the region's range, for shared.js
}
/** `each`: rows by key in item order; a kept row keeps its elements, its
 * item and position are signals its bindings read. */
export function each(p, list, key, row, pure) {
  let [a, b] = range(p), own = Owner;
  let rows = new Map(), single = false, order = null;
  effect(() => {
    const items = list(), parent = b?.parentNode ?? p; // rows go where the end anchor is: at an arm's top, `p` is the fragment the arm was built in
    // A key that reads more than its item and index (a slot) is read tracked, re-keying the rows as the runner's; a pure one only where needed.
    const keys = pure ? null : items.map((item, i) => key(() => item, () => i)), keyAt = i => keys ? keys[i] : key(() => items[i], () => i);
    untracked(() => {
      if (b) p = a.parentNode; // Conditional arms leave their build fragment.
      // Rows moving or leaving are adopted rows (a row waiting for its slice
      // shows its rendered values until then, and adopts at the current ones).
      if (b && LazyAt < Lazy.length) adoptAll();
      // Keys that didn't move: new items to their rows, nothing else (LLP 1071.000 D2).
      if (pure && order && items.length === order.length) {
        let i = 0;
        for (; i < items.length; i++) {
          const item = items[i], r = order[i];
          if (r.item.n.v === item) continue;
          const k = keyAt(i);
          if (typeof k + ":" + (Object.is(k, -0) ? 0 : k) !== r.k) break;
          writeItem(r.item.n, item);
        }
        if (i === items.length) return;
      }
      // A row's place in the last pass is its `at`; repeats count once a key repeats.
      const next = new Map(), seen = new Map();
      items.forEach((item, i) => {
        let k = keyAt(i);
        // A key is a string, a finite number or a bool, or the runner cannot build the rows (`key_text`, InstanceError::KeyKind): it poisons.
        if (!(typeof k === "string" || typeof k === "boolean" || (typeof k === "number" && isFinite(k)))) throw new Error(`a row key that is not a string, finite number or bool: ${k}`);
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
        parent.textContent = "";
        parent.append(a, b);
      }
      else if (rows.size) { endAll(rows); for (const r of rows.values()) { let n = r.start; while (n) { const m = n.nextSibling; Leave ? Leave(n, b) : n.remove(); if (n === r.end) break; n = m; } } }
      // Order, from the last row back: kept rows on the longest run already in
      // order stay; any other moves before the row after it; new rows go in
      // one fragment per run.
      if (b) {
        const stay = inOrder(list);
        let anchor = b, batch = null, first = null;
        const flush = () => { if (batch) { parent.insertBefore(batch, anchor); anchor = first; batch = null; } };
        for (let i = list.length - 1; i >= 0; i--) {
          const r = list[i];
          if (r.frag) { if (batch) batch.prepend(r.frag); else batch = r.frag; first = r.start; r.frag = null; continue; }
          flush();
          if (!stay.has(i)) {
            const f = document.createDocumentFragment();
            let n = r.start; while (n) { const m = n.nextSibling; f.append(n); if (n === r.end) break; n = m; }
            parent.insertBefore(f, anchor);
          }
          anchor = r.start;
        }
        flush();
      }
      rows = next; Paint?.list(parent);
      b ??= mark(p);
    });
  }).$r = () => [a, b]; // the region's range, for shared.js
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
function lazy(x) { Paint?.wait(x[1].start); Lazy.push(x); LazyRows.set(x[1].start, x); }
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
  for (let n = target; n && n.nodeType === 1; n = n.parentNode) { const x = LazyRows.get(n); if (x) { adoptLazy(x); Paint?.flush(); break; } }
}
const onLazy = ev => adoptAt(ev.target);
function slice() {
  LazyTask = null;
  const end = performance.now() + SLICE_MS;
  while (LazyAt < Lazy.length && performance.now() < end) adoptLazy(Lazy[LazyAt++]);
  Paint?.flush();
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
  start = clock.start = performance.now() - clock.now;
  const adopting = Adopt;
  // What the reader did before the runtime ran (the capture script), and
  // what each edited control showed then: the commit writes the state's
  // values, which the reader had changed.
  const early = globalThis.exact?.taps?.() ?? [];
  const shown = early.filter(t => t.type !== "click").map(t => [t.target, t.target.value, t.target.checked]);
  AdoptBy = performance.now() + ADOPT_MS;
  Booting = true; let booted = commit(() => { scope(() => f(root)); built = true; }, adopting ? "adopt" : "boot");
  Adopt = false; AdoptBy = Infinity;
  if (!built) { Lazy.length = LazyAt = 0; LazyRows.clear(); }
  const adopted = adopting && built;
  if (!built && adopting) {
    // The document isn't this plan's projection: build afresh (and say so).
    say(`adoption abandoned: ${journal.at(-1)}`);
    root.textContent = "";
    booted = commit(() => { scope(() => f(root)); built = true; }, "boot");
  }
  // A boot whose settlement refused (a derive outside its type) is refused whole, as `Runner::boot` fails: nothing shows.
  Booting = false; if (!built || booted === false) { root.textContent = ""; throw new Error("boot refused: " + journal.at(-1)); }
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
// Read untracked (an action's body, a handler's curried argument evaluated as the event arrives) it is the clock now:
// the commit's time, or outside one the time the runner would evaluate the arguments at. A derive, resource or the
// tree reads it tracked, as of the last commit: an advance that fired nothing committed no new time.
export const x_now = () => { NowRead = true; if (Listener) return read(Now); if (!Writes) time(); return clock.now; };
export * from "./roster.js"; // the roster's pure entries
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
 * unfilled name keeps its spelling; `\{ \} \\` escape. budget.js's `x_t` bounds it. */
export function text(name, key, pairs) {
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
export * from "./router.js"; import { routerValid } from "./router.js"; // the router (LLP 1038), its own file
export { queues, gated } from "./schedule.js"; // queued sends and gated tasks (LLP 1092), their own file
