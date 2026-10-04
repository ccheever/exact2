// Sends that wait their turn and tasks that wait for state on the JS target (LLP 1092): runner/src/runner/queue.rs
// and gates.rs over rt.js's mutations and clock. Installed only by a plan that declares `mutation … queue`
// (emit.rs, `queues($state)`) or a gated task (timers.rs, `gated(…)`), so no other app carries it.
// A queue mutation keeps one ticket in flight; a later send waits in `m.wait` with its send-time arguments and is
// asked in a `next` commit of its own once the mutation is free: no ticket, no `then` or `next` armed, not stalled.
// A gated task is in `clock.timers` only while its gate holds; a new key re-arms it from the commit's time.
import { Mutations, Tasks, commit, M, journal, clock, drive, paint, order, write, eq, untracked, useSchedule, refused, Refusal } from "./rt.js";
const say = line => journal.push(`t=${clock.now} ${line}`);
const BOUND = 64; // runner QUEUE_BOUND: the 65th waiter refuses its action
let Asked = new Set(), State = [];
const Gates = []; // gated tasks, in plan order
const idle = m => !m.ticket && !(m.due < Infinity) && !(m.next < Infinity) && !m.stall;
// What a stalled `next` saw: every root slot, derive and resource (the runner's `Basis`); a standing commit that
// changes any of them lets it try again, one that changes none leaves it waiting (D3).
const basis = () => State.map(list => list.map(g => g()));
const moved = b => State.some((list, k) => list.some((g, i) => !eq(g(), b[k][i])));
const pending = m => { const v = !!(m.ticket || m.wait.length); if (m.pend.n.v !== v) write(m.pend.n, v); };
const Q = {
  own: null,
  // The commit's checkpoint keeps the waiting sends and the stalls, beside each ticket, and the timers a gate step
  // arms or drops (rt.js `commit`).
  save() {
    Asked = new Set();
    return [Mutations.map(m => m.wait && [m.wait.slice(), m.stall]), Gates.length ? [clock.timers.slice(), Gates.map(g => [g.armed, g.kt, g.due, g.base, g.k])] : null];
  },
  restore([queues, timers]) {
    Mutations.forEach((m, k) => { if (queues[k]) { m.wait = queues[k][0]; m.stall = queues[k][1]; } });
    if (timers) { clock.timers.splice(0, Infinity, ...timers[0]); Gates.forEach((g, k) => { [g.armed, g.kt, g.due, g.base, g.k] = timers[1][k]; }); }
  },
  /** D8's step, inside the commit after its settlement: each gate and key over the settled state, at the commit's
   * time. A key that is no key refuses the commit (`TaskKey`); else a false gate drops its timer, a gate turned true
   * or a changed key arms it from now, and anything else leaves it (an `every`'s phase, a spent `after`). */
  gates() {
    if (!Gates.length) return;
    const seen = untracked(() => Gates.map(g => {
      const on = g.gate() === true, kt = on && g.key ? keyText(g.key()) : undefined;
      if (kt === null) throw new Refusal(`TaskKey { task: ${JSON.stringify(g.name)} }`);
      return [on, kt];
    }));
    Gates.forEach((g, k) => step(g, ...seen[k]));
  },
  /** A send of queue mutation `m`: asked now only when free and the commit's first; else it waits (D2). */
  hold(m, source, args, undo) {
    if (!Asked.has(m) && idle(m) && !m.wait.length) { Asked.add(m); return false; }
    if (m.wait.length >= BOUND) throw new Refusal(`QueueFull { mutation: ${JSON.stringify(m.name)} }`);
    m.wait.push([source, args]);
    if (!m.pend.n.v) { undo.push([m.pend.n, false]); write(m.pend.n, true); }
    return true;
  },
  /** The scan, after every commit, stood or refused: a stall a standing commit moved is let go, and a free queue with
   * a send waiting is due now — off the agent, on the wall clock's `drive`, `then` or no `then`. */
  scan(stood) {
    for (const m of Mutations) if (m.wait) {
      if (stood && m.stall && untracked(() => moved(m.stall))) m.stall = null;
      if (m.wait.length && idle(m)) { m.next = clock.now; if (!clock.agent) drive(); }
    }
  },
  /** Poison or a reload: the waiting sends were never asked, so nothing is told; one line each (D4). */
  forget() {
    for (const m of Mutations) if (m.wait) {
      const n = m.wait.length; m.wait = []; m.next = Infinity; m.stall = null;
      if (n) say(`forgot ${n} waiting send${n === 1 ? "" : "s"} (${m.name})`);
    }
  },
  /** A `next` commit (D3): the head popped and asked as an action's send is. Its own ask's refusal drops it for
   * good (the successor is due); any other refusal puts it back and stalls `m` until a commit changes the state. */
  next(m) {
    m.next = Infinity; Q.own = null;
    const ok = commit(() => { const [source, args] = m.wait.shift(); say(`${m.name} next (${m.wait.length} waiting)`); M(m, source, args, true); }, `${m.name} next`);
    if (ok === true) return true;
    const e = refused();
    if (!e) return ok; // the commit poisoned the runner: `forget` ran
    m.next = Infinity;
    if (Q.own) {
      m.wait.shift();
      say(`${m.name} queued send refused: ${e.message}`);
      // Nothing waits behind it and nothing is in flight: the view hears `pending` end, as after a failed reply.
      if (!m.wait.length && !m.ticket) { pending(m); commit(() => {}, "a refused queued send"); }
    } else {
      m.stall = untracked(basis);
      say(`${m.name} next refused (${e.message}); waits for a change`);
    }
    Q.scan(false);
    return false;
  },
};
/** Install the queues: every `mut(…, queue)` gets its wait list; `state` is the root's slots, derives, resources. */
export function queues(state) {
  State = state;
  for (const m of Mutations) if (m.queue) { m.wait = []; m.stall = null; }
  useSchedule(Q);
}
/** One key's text, as the runner's `key_text`: a string, a finite number (`-0` is `0`), a bool; anything else none. */
const keyText = v => typeof v === "string" ? "s:" + v : typeof v === "number" ? (Number.isFinite(v) ? "n:" + (v === 0 ? 0 : v) : null) : typeof v === "boolean" ? "b:" + v : null;
function step(g, on, kt) {
  const at = clock.timers.indexOf(g);
  if (!on) { if (at >= 0) clock.timers.splice(at, 1); g.armed = false; g.kt = undefined; return; }
  if (g.armed && kt === g.kt) return;
  g.armed = true; g.kt = kt;
  if (g.frame) { g.base = clock.now; g.k = 1; g.due = clock.now + 1000 / 60; } else g.due = clock.now + g.ms;
  if (at < 0) { const i = clock.timers.findIndex(t => t.i > g.i); clock.timers.splice(i < 0 ? clock.timers.length : i, 0, g); }
  if (!clock.agent) g.frame ? paint() : drive();
}
/** `task name when gate key=key` (LLP 1092 D7): idle until its gate holds over the state at mount or at a commit. */
export function gated(ms, action, once, frame, gate, key, name) {
  const g = { ms, action, once: !!once, frame: !!frame, gate, key: key || null, name, i: order(), armed: false, kt: undefined, due: Infinity };
  Gates.push(g); Tasks.push(g);
  useSchedule(Q);
  const on = untracked(() => gate() === true), kt = on && g.key ? keyText(untracked(g.key)) : undefined;
  if (kt === null) throw new Refusal(`TaskKey { task: ${JSON.stringify(name)} }`);
  step(g, on, kt);
}
