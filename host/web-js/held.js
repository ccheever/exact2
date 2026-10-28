// Answers whose fetch the server holds (`exactHeld`, LLP 1016.000 D9): in flight, but no clock waits on them.
// A ticket (rt.js `send`) counts in `inflight` while it waits, unless its answer has only held fetches out
// (`call.held`, none of its others, `call.plain`, ts-fetch.js) and no fetch tied to no answer is out (one made
// in a plain `.then`, which the async context does not reach): then it is held, and counts again once that
// changes, for the answer's work after its held reply. A ticket its resource or mutation has let go (or whose
// resource left the screen) is no longer named, and its late reply is no reply (`heldReplies`).
// The invariant this rests on: every host facility a data source can await is counted on its answer, as a held
// fetch (`call.held`) or as ordinary work (`call.plain`, ts-fetch.js `ordinary`: fetches, crypto.subtle, native.later,
// generated module calls, kept keys, storage, openAuthSession), or in `inflight` itself (the storage queue). The
// runner hosts get this for free: an answer with more than one operation out is a continuation, never held.
const HeldAsks = new Set(), Candidates = new Set(), loose = { n: 0 }, Replies = new Map();
/** The tickets `inflight` counts now: a wake owns those that were not counted at its delivery (wakes.js). */
export const Counting = new Set();
const current = t => t?.owner?.ticket === t && !t.owner.gone;
export const heldAsks = () => [...HeldAsks].filter(current).map(t => ({ name: t.name, ticket: t.id }));
/** The held tickets themselves (wakes.js tells a stream's from an answer's). */
export const heldTickets = () => [...HeldAsks].filter(current);
/** Replies landed for held fetches, by target: `clock held`'s evidence (the runner's `held_replies`). */
export const heldReplies = () => Object.fromEntries(Replies);
export function heldReply(call) { if (current(call.ticket)) Replies.set(call.ticket.name, (Replies.get(call.ticket.name) ?? 0) + 1); }
/** A long poll's reply (LLP 1016.000 D8): from its first the stream is held and named, and each is a reply. */
export function longPollReplied(t) { if (current(t)) { HeldAsks.add(t); Replies.set(t.name, (Replies.get(t.name) ?? 0) + 1); } }
/** A long poll's end, or its letting go: no longer held. */
export function longPollEnded(t) { HeldAsks.delete(t); }
export function counted(t, inflight) {
  if (t.call) { t.call.recount ??= () => counted(t, inflight); t.call.ticket ??= t; } // a fetch's start and end (ts-fetch.js)
  const waits = !!t.waiting && !t.closed, candidate = waits && t.call?.held > 0;
  // An answer with only its own timers out waits on the clock, not on I/O (ts-fetch.js `arm`).
  const held = candidate && !t.call.plain && !loose.n, timed = waits && !held && t.call?.timers > 0 && !t.call.plain && !t.call.firing && !loose.n;
  const counts = waits && !held && !timed;
  if (counts !== !!t.counted) { t.counted = counts; inflight.n += counts ? 1 : -1; }
  if (counts) Counting.add(t); else Counting.delete(t);
  if (held) HeldAsks.add(t); else HeldAsks.delete(t);
  if (candidate) Candidates.add(t); else Candidates.delete(t);
}
/** A fetch tied to no answer starts (+1) or ends (-1): every held candidate is counted again meanwhile. */
export function looseFetch(delta) {
  loose.n += delta;
  for (const t of [...Candidates]) t.call.recount();
}
