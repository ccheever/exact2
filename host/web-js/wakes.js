// Wakes on the JS target (LLP 1133.017 D5, D6), as the runner's `modules/wakes.rs`: a module's event declared
// `"raised": "wake"` is raised only by its implementation through the context's `wakes.raise` (the web's wake is the
// page's own `online` event), or under the agent by `tap @ref` on a registration or `--wake '<entry>'`. Each is an
// invocation of origin `wake`, delivered once, in order, one at a time, each as its own commit. The page cannot tell
// one request from another, so an invocation owns every answer that went out after its delivery, `then`s and queued
// sends included, and a request the server holds (`heldAsks`, which `inflight` leaves out); not a stream, which is a
// view's (a resource's live answer the boot or the view opened meanwhile). The origin of every call is `wake` until
// that settles (`busy`), the least privileged in flight, as an emitted event's `device` is. Nothing it started is cancelled at the deadline: the invocation ends `expired`, and
// what it started settles under the app's origin. A wake in a hidden page fires no timer (`clock.stopped`); under the
// agent the wake hides the page (`exactPage().visibilityState`) and shows it again after, as the platform's background.
//
// Registrations (§10): a keyed list declared with a `kind` keeps, beside each entry's options, its runs, attempts and
// backoff, by the web's rules (D1) as `modules/declared/wake_list.rs` keeps them.
import { clock, inflight, After, Clocked, Mutations, drive, data } from "./rt.js";
import { Counting, heldTickets } from "./held.js";

const BACKOFF = 5 * 60_000, LONGEST = 60 * 60_000, ATTEMPTS = 3;
/** The host commands a commit of a wake's origin may not run: each opens system UI, asks a person, or plays sound. */
export const REFUSED = new Set(["copyText", "openURL", "playSound", "playSounds", "requestFullscreen", "requestMIDIAccess", "requestPermission", "saveFile", "share",
  "showDirectoryPicker", "showOpenFilePicker", "showPicker", "showSaveFilePicker"]);

/** `events`: extension.event → [action, raised, hold, types, name, budget]; `value(json, type, key)` checks an argument;
 * `run(f)` runs `f` as a commit of origin `wake`; `toEpoch(ms)` a clock time's date; `onLine()`; `lists` the declared lists by
 * state; `changed(state)` re-asks the sources that answer from a list. */
export function wakes({ events, say, value, run, toEpoch, onLine, lists, changed }) {
  const now = () => toEpoch(clock.now);
  const meta = new Map(), waiting = [];
  let current = null, last = null, base = null, timer = 0, ids = 0;
  // The agent's page facts (facts.js): whether the page reads visible, and setting it.
  const seen = () => globalThis.exact?.page?.read?.()["visibility-state"] !== "hidden";
  const see = shown => { globalThis.exact?.page?.prefer?.({ "visibility-state": shown ? "visible" : "hidden" }); globalThis.exact?.foreground?.(shown); };
  const storage = () => { const b = data.background?.(); return b ? b.queued + b.inFlight : 0; };
  // By ticket, so a request of the app's that lands meanwhile does not pass for the wake's end; work counted without
  // a ticket (a file, the storage queue) by its count beside the tickets.
  const answer = t => !t.stream && !t.req?.stream;
  const owns = () => base !== null && ([...Counting].some(t => answer(t) && !base.tickets.has(t)) || inflight.n - Counting.size > base.loose
    || heldTickets().some(t => answer(t) && !base.held.has(t)) || Mutations.some(m => m.due < Infinity || m.wait?.length));
  const deadline = () => current?.started != null && !current.expired ? current.started + current.budget : null;
  const describe = (w, outcome = null) => ({ id: w.id, kind: w.invocations[0]?.event ?? null, cold: w.cold, started: w.started == null ? null : toEpoch(w.started),
    deadline: w.started == null ? null : toEpoch(w.started) + w.budget, invocations: w.invocations.map(i => ({ extension: i.key.split(".")[0], event: i.event, tag: i.tag, state: i.state })), outcome });

  /** Queue an invocation of `key` with `args` in the wake for platform object `how.id`; its outcome settles the promise. */
  const raise = (key, args, how = {}, entry = null) => new Promise(resolve => {
    const declared = events[key], id = how.id ?? ++ids;
    let w = current?.id === id ? current : waiting.find(q => q.id === id);
    if (!w) waiting.push(w = { id, cold: !!how.cold, hidden: !!how.hidden, started: null, budget: 0, invocations: [], expired: false });
    if (!declared) { say(`${key}: no handler hears it`); resolve("unheard"); queueMicrotask(step); return; }
    const [, , hold, , , budget] = declared, event = key.slice(key.indexOf(".") + 1), tag = typeof args?.[0] === "string" ? args[0] : "";
    w.budget = Math.max(w.budget, budget ?? 0);
    w.invocations.push({ key, event, tag, args: Array.isArray(args) ? args : [], state: "queued", delivered: false, entry, resolve });
    say(`wake ${event} ${tag}: raised`);
    // Each event's own bound: past it, its own oldest not yet delivered ends with nothing delivered.
    const queued = () => w.invocations.filter(i => i.key === key && i.state === "queued");
    while (queued().length > hold) { const old = queued()[0]; old.state = "expired"; old.resolve("expired"); say(`wake ${event} ${old.tag}: dropped`); }
    queueMicrotask(step);
  });

  const end = (inv, state) => {
    inv.state = state;
    say(`wake ${inv.event} ${inv.tag}: ${state}`);
    if (inv.entry != null) ended(inv.entry, state, inv.delivered);
    inv.resolve(state);
  };
  const deliver = inv => {
    const [action, , , types, name] = events[inv.key];
    current.started ??= clock.now;
    if (!clock.agent && !timer) timer = setTimeout(expire, current.budget);
    inv.state = "running";
    inv.delivered = true;
    if (inv.entry != null && meta.has(inv.entry)) meta.get(inv.entry).again = false;
    let values;
    try { values = types.map((t, i) => value(inv.args[i] ?? null, t, inv.key)); } catch (e) { say(`${inv.key}: refused: ${e.message}`); end(inv, "refused"); return; }
    say(`wake ${inv.event} ${inv.tag}: heard by ${name}`);
    base = { tickets: new Set(Counting), loose: inflight.n - Counting.size, held: new Set(heldTickets()) };
    run(() => action()(...values));
  };
  const finish = () => {
    const outcome = current.expired || current.invocations.some(i => i.state === "expired") ? "expired" : "completed";
    last = describe(current, outcome);
    say(`wake: ${outcome}`);
    if (current.hid) see(true);
    current = null; base = null; clearTimeout(timer); timer = 0;
    if (clock.stopped) { clock.stopped = false; if (!clock.agent) drive(); }
  };
  /** Deliver what is due: the next invocation once the one running owns nothing, and the wake's end once each ended. */
  function step() {
    for (;;) {
      if (!current) { if (!waiting.length) return; current = waiting.shift(); clock.stopped = current.hidden; current.hid = current.hidden && clock.agent && seen(); if (current.hid) see(false); }
      const running = current.invocations.find(i => i.state === "running"), next = current.invocations.find(i => i.state === "queued");
      if (running) { if (owns()) return again(); base = null; end(running, "completed"); continue; }
      if (next) { deliver(next); continue; }
      // An expired wake ends without waiting for storage: the platform's time is up.
      if (storage() > 0 && !current.expired) return again();
      finish();
    }
  }
  // Work can settle with no commit after it (a storage write draining, a reply that changes nothing): looked at again
  // while the wake waits on it. The deadline is the clock's, never this timer's.
  let poll = 0;
  function again() { if (!poll) poll = setTimeout(() => { poll = 0; step(); }, 20); }
  function expire() {
    if (!current || current.expired) return;
    current.expired = true;
    base = null;
    for (const inv of current.invocations.filter(i => i.state === "queued" || i.state === "running")) end(inv, "expired");
    step();
  }
  After.push(() => { if (current || waiting.length) queueMicrotask(step); });
  Clocked.push(() => { const d = deadline(); if (d != null && clock.now >= d) expire(); else if (current) queueMicrotask(step); });

  // ---- registrations
  const entryOf = ref => { for (const [state, entries] of lists) { const e = entries.find(x => x.ref === ref); if (e) return [state, e]; } return []; };
  // A periodic entry's interval, the option its tap's `due` names; null for a one-shot.
  const interval = e => { const o = e.row.sub.list.tap?.due; return o == null ? null : typeof e.fields[o] === "number" ? e.fields[o] : 0; };
  const due = e => {
    const m = meta.get(e.ref), i = interval(e);
    if (!m) return 0;
    return i != null ? (m.lastRun ?? m.registered) + i : m.notBefore ?? m.registered;
  };
  function ended(ref, state, delivered) {
    const [list, e] = entryOf(ref), m = meta.get(ref);
    if (!e || !m) return;
    const periodic = interval(e) != null;
    let retire = false;
    if (periodic && (state === "completed" || state === "superseded")) { m.lastRun = now(); m.attempts = 0; }
    else if (!periodic && state === "completed" && !m.again) retire = true;
    else if (delivered && ["completed", "expired", "superseded"].includes(state)) {
      m.attempts++;
      if (!periodic) m.notBefore = now() + Math.min(BACKOFF * 2 ** (m.attempts - 1), LONGEST);
    }
    if (!periodic && m.attempts >= ATTEMPTS) { if (m.again) { m.attempts = 0; m.notBefore = now() + LONGEST; } else retire = true; }
    m.again = false;
    if (retire) { meta.delete(ref); lists.set(list, lists.get(list).filter(x => x !== e)); say(`${list} @${ref}: retired`); }
    changed(list);
  }
  const raiseEntry = (e, cold) => {
    const tap = e.row.sub.list.tap ?? {}, m = meta.get(e.ref);
    const fields = { ...e.fields, lastChance: (m?.attempts ?? 0) + 1 >= ATTEMPTS };
    const args = (tap.args ?? []).map(a => typeof a === "string" ? fields[a] ?? null : a && typeof a === "object" ? Object.fromEntries(Object.entries(a).map(([k, f]) => [k, fields[f] ?? null])) : a);
    raise(`${e.row.key.split(".")[0]}.${tap.event}`, args, { id: `entry ${e.ref}`, cold, hidden: true }, e.ref);
    return { ref: e.ref, event: tap.event, as: "wake", delivery: "substituted" };
  };
  return {
    raise,
    /** The origin while a wake's work is in flight. */
    active: () => base !== null,
    /** `state.wake`. */
    state: () => current ? describe(current) : last,
    /** Entry `ref` was listed in place of `replaced` (same key and kind): it keeps that one's runs and attempts. */
    registered(ref, replaced) {
      const m = (replaced != null && meta.get(replaced)) || { registered: now(), lastRun: null, attempts: 0, notBefore: null, again: false };
      if (replaced != null) {
        meta.delete(replaced);
        for (const w of [current, ...waiting].filter(Boolean)) for (const i of w.invocations) if (i.entry === replaced) { if (i.state === "running") m.again = true; i.entry = ref; }
      }
      meta.set(ref, m);
    },
    fields: e => e.fields.kind ? { lastRun: meta.get(e.ref)?.lastRun ?? null, due: due(e) } : {},
    /** `tap @ref` on a registration: a warm wake, when the platform would give one. */
    tap(e) {
      const at = due(e);
      if (now() < at) return { error: `@${e.ref} is not due until ${at}` };
      if (interval(e) == null && !onLine()) return { error: `@${e.ref}: offline` };
      say(`${e.row.sub.list.state} @${e.ref}: tapped`);
      return raiseEntry(e, false);
    },
    /** `--wake '<entry>'`: listed again, then raised as a cold wake. */
    launch(row, entry, ref) {
      const { ref: _ref, delivered: _d, due: _due, lastRun, ...fields } = entry;
      const state = row.sub.list.state, key = row.sub.list.key;
      const kept = (lists.get(state) ?? []).filter(x => x.fields[key] !== fields[key] || x.fields.kind !== fields.kind);
      const e = { ref, row, fields };
      lists.set(state, [...kept, e]);
      meta.set(ref, { registered: now(), lastRun: lastRun ?? null, attempts: 0, notBefore: null, again: false });
      say(`launch --wake (${fields.kind} @${ref})`);
      changed(state);
      return raiseEntry(e, true);
    },
    /** A source declared `"listed"`: the entries of its kind, its fields each. */
    listed(spec) {
      return (lists.get(spec.state) ?? []).filter(e => spec.kind == null || e.fields.kind === spec.kind)
        .map(e => { const all = { ...e.fields, ...(e.fields.kind ? { lastRun: meta.get(e.ref)?.lastRun ?? null, due: due(e) } : {}) }; return Object.fromEntries((spec.fields ?? []).map(f => [f, all[f] ?? null])); });
    },
  };
}
