// Optimistic writes on the JS target, as the runner's `writes.rs`: a record per send to a mutation that
// declares `refreshes`, shown over each refreshed resource's answer by the source's `overlay`. A record is
// pending until its final reply lands, then landed until each refreshed resource has an answer asked after
// the landing that the overlay does not keep it over. One that ends without landing goes.
import { conforms, equal } from "./shape.js";
import { data, say } from "./rt.js";

export const Writes = { list: [], seq: 0 };
/** The next number of the one sequence that orders sends, landings and asks. */
export const tick = () => ++Writes.seq;
export const save = () => [Writes.list.map(w => ({ ...w, landed: w.landed && { ...w.landed, showing: [...w.landed.showing] } })), Writes.seq];
export const restore = x => { Writes.list = x[0]; }; // ids stay unique: a rolled-back send's id is never reused
const inFlight = m => { const p = Writes.list.filter(w => w.m === m && !w.landed); return m.queue ? p[0] : p[p.length - 1]; };
// The pending record a ticket carries (its id: a rolled-back commit restores copies), or for a send asked just now,
// `m`'s in flight.
const pending = (m, id) => id ? Writes.list.find(w => w.id === id && !w.landed) ?? null : inFlight(m);
/** The id of the write a send to `m` asked now carries on its ticket. */
export const asked = m => inFlight(m)?.id;
/** A send to `m` accepted; a newer send to a non-queue mutation ends its older pending ones (newest wins).
 * The resources to ask again to reconcile. */
export function accept(m, source, args) {
  if (!m.refreshes.length) return [];
  // Recorded before the older sends end, so ending them does not ask their resources again while this one shows in them
  const w = { id: tick(), m, source, args, landed: null };
  Writes.list.push(w);
  return m.queue ? [] : endPending(m, w);
}
/** `m`'s send (`write`, its ticket's) landed with `reply`; a stream's later messages land nothing more. */
export function land(m, reply, write) {
  const w = pending(m, write); if (!w) return;
  // The runner's own resources show their facts, never a write (`owned`, from the plan's `RUNNER_OWNED_SOURCES`)
  const showing = m.refreshes.filter(r => !r.owned);
  // Shown nowhere, it has nothing to wait for
  if (showing.length) w.landed = { reply, at: tick(), showing }; else Writes.list.splice(Writes.list.indexOf(w), 1);
}
/** `m`'s send (`write`) ended without landing: the resources to ask again, or `null` when none was. */
export function end(m, write) { const w = pending(m, write); if (!w) return null; Writes.list.splice(Writes.list.indexOf(w), 1); return reconcile(m); }
/** Every pending send of `m` ends: an assignment to its slot, or a newer send. */
export function endPending(m, except) { const n = Writes.list.length; Writes.list = Writes.list.filter(w => w.m !== m || w.landed || w === except); return n === Writes.list.length ? [] : reconcile(m); }
// Each refreshed resource no other pending write shows in (that one's own end asks).
const reconcile = m => m.refreshes.filter(r => !Writes.list.some(w => !w.landed && w.m.refreshes.includes(r)));
const showing = r => Writes.list.filter(w => w.landed ? w.landed.showing.includes(r) : w.m.refreshes.includes(r));
/** Whether any write shows in `r`. */
export const written = r => showing(r).length > 0;
/** What `r` shows: `answer` (asked at `origin`, for `args`), or its source's overlay of the writes showing in
 * it. A landed write the answer was asked after is answered by it and retired from `r`, unless the overlay
 * keeps it. An overlay outside the shape, or one that fails, is journaled and counts as none: the answer shows. */
export function shown(r, answer, args, origin) {
  const ws = r.owned ? [] : showing(r);
  if (!ws.length) { r.ov = null; return answer; }
  const answered = w => !!w.landed && w.landed.at < origin;
  const mark = w => `${w.id}${w.landed ? (answered(w) ? "a" : "l") : "p"}`, key = ws.map(mark).join();
  if (r.ov && r.ov.answer === answer && equal(r.ov.args, args) && r.ov.key === key) return r.ov.shown;
  let o, value = answer;
  try { o = data.overlay?.(r.source, args, answer, ws.map(w => ({ id: w.id, mutation: w.m.name, source: w.source, args: w.args, reply: w.landed?.reply, answered: answered(w) }))); }
  catch (e) { say(`overlay ${r.name} failed, so its answer shows: ${e?.message ?? e}`); }
  if (o && r.type && !conforms(o.value, r.type)) { say(`overlay ${r.name}: outside its shape, so its answer shows`); o = null; } // as no overlay
  else if (o) { value = o.value; say(`overlay ${r.name} (${ws.length} writes)`); }
  const retire = ws.filter(w => answered(w) && !o?.keep.includes(w.id));
  for (const w of retire) w.landed.showing = w.landed.showing.filter(x => x !== r);
  if (retire.length) Writes.list = Writes.list.filter(w => !w.landed || w.landed.showing.length);
  r.ov = { answer, args, key: ws.filter(w => !retire.includes(w)).map(mark).join(), shown: value };
  return value;
}
