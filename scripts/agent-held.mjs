// The agent's waits on real time with the clock frozen, `clock held` (LLP 1016.000 D9, theme2 2026-10-22) and `clock
// until state` (rules/DEFERRED.md, "Agent API", 2026-10-26), over the session's own ops (scripts/agent.mjs), so one
// implementation serves every host.
import { MalformedOperation } from './agent-keys.mjs';

// `clock held [<target>…] [<n>s|<n>ms]`: real time passes, the clock frozen, until each held request in flight now
// (state.pending's `held`, or those named) has had one reply, a server's own timeout answer included, then `clock
// data`; past the bound (20 s) `settled: false, reason: 'held'`, naming the targets with none (LLP 1016.000 D9).
// A reply is the host's count of replies landed for a target's held requests (`state.heldReplies`) rising: a
// request let go, or no longer counted held for a while, is no reply; a reply followed at once by another held
// ask is one.
export async function heldClock(s, words) {
  const last = words.at(-1), unit = /^(\d+(?:\.\d+)?)(ms|s)$/.exec(last ?? '');
  if (unit) words = words.slice(0, -1);
  const bound = unit ? Number(unit[1]) * (unit[2] === 's' ? 1000 : 1) : 20000;
  if (!Number.isFinite(bound) || bound <= 0 || bound > 3_600_000) throw new MalformedOperation(`clock held: a bound is up to an hour, as 30s or 500ms, not ${last}`);
  // On Apple the driver takes the clock over first (platform timing), as any other clock does, so it stays frozen.
  if (['ios', 'host-ios', 'macos', 'host'].includes(s.carrier.host)) s.now = (await s.op({ op: 'clock', take: true })).clock;
  // The host's `held` op (the held asks and the replies landed for them), lighter than `state`, which a host
  // without it answers instead.
  const look = async () => { const r = await s.op({ op: 'held' }).catch(() => null); return r && Array.isArray(r.held) ? r : (st => ({ held: (st.pending ?? []).filter((p) => p.held), heldReplies: st.heldReplies }))(await s.op({ op: 'state' })); };
  const first = await look();
  const names = [...new Set(first.held.map((p) => p.name))];
  const unknown = words.filter((w) => !names.includes(w));
  if (unknown.length) throw new MalformedOperation(`clock held: ${unknown.join(', ')} ${unknown.length > 1 ? 'are' : 'is'} not a held request in flight (${names.length ? `held: ${names.join(', ')}` : 'none is held'})`);
  const targets = words.length ? [...new Set(words)] : names, before = first.heldReplies ?? {};
  // Looks back off from 25 ms to 200 ms, each wait, and each look itself, cut to what is left of the bound, so the
  // bound stays exact: a look that answers past it counts nothing.
  let left = targets, seen = first;
  const until = (end) => new Promise((ok) => setTimeout(() => ok(null), Math.max(0, end - performance.now())));
  for (let end = performance.now() + bound, gap = 25; left.length && performance.now() < end; gap = Math.min(gap * 2, 200)) {
    await new Promise((ok) => setTimeout(ok, Math.max(0, Math.min(gap, end - performance.now()))));
    const got = await Promise.race([look(), until(end)]);
    if (!got) break;
    seen = got;
    const now = seen.heldReplies ?? {};
    left = left.filter((t) => !((now[t] ?? 0) > (before[t] ?? 0)));
  }
  // Tagged as the host tags a clock reply (its epoch and incarnation), though no host made this one.
  if (left.length) return { ...(await s.op({ op: 'tags' }).catch(() => ({}))), clock: s.now, settled: false, reason: 'held', held: seen.held.length, heldTargets: left, diagnostic: `clock held waited ${bound} ms of real time and ${left.join(', ')} had no reply: the server still holds ${left.length > 1 ? 'them' : 'it'}; give a longer bound (clock held ${Math.ceil(bound / 500)}s) or check the server` };
  const landed = await s.clock('data');
  return targets.length ? landed : { ...landed, diagnostic: 'clock held: no held request was in flight, so it waited for none' };
}

const UNTIL = 'clock until state <path> == <literal> within <ms> (the literal is JSON: a string, a number, true, false or null)';

// `clock until state <path> == <literal> within <ms>`: real time passes, the clock frozen (no timer fires), until
// `state <path>` reads the literal, for at most <ms>; a miss throws, naming the last value read. It is for state a
// stream reaches after its first message, which no settlement waits for (LLP 1016.000 D5). Each look first lands
// what has arrived, as an input's end does (`clock` with `land`: the `then`s of answers in, no timer fired), so a
// protocol's later steps go on meanwhile: Excord's IDENTIFY is sent from a write's `then`, and READY answers it.
export async function untilClock(s, spec) {
  const m = /^until\s+state\s+(\S+)\s*==\s*(\S.*?)\s+within\s+(\d+)$/s.exec(spec.trim());
  if (!m) throw new MalformedOperation(`${UNTIL}, not \`clock ${spec.trim()}\``);
  const [, path, literal] = m, bound = Number(m[3]);
  let want;
  try { want = JSON.parse(literal); } catch { /* refused below */ }
  if (want === undefined || (want !== null && typeof want === 'object') || (typeof want === 'number' && !Number.isFinite(want))) throw new MalformedOperation(`${UNTIL}: ${literal} is not one`);
  if (bound < 1 || bound > 3_600_000) throw new MalformedOperation(`${UNTIL}: a bound is 1 ms up to an hour, not ${bound}`);
  // A name, then fields and list indexes, read from `state` as an authored `expect state` reads it.
  const [name, ...fields] = path.split('.'), has = (o, k) => o !== null && typeof o === 'object' && Object.hasOwn(o, k);
  const read = async () => {
    const landed = await s.op({ op: 'clock', land: true });
    if (landed?.error) throw new Error(`clock until state ${path}: ${landed.error}`);
    const record = await s.op({ op: 'state' }), bag = { ...record.resources, ...record.derives, ...record.slots };
    const tasks = record.tasks ?? {};
    if (!has(bag, name) && !has(record, name)) return { unknown: Object.keys(bag), tasks };
    let got = has(bag, name) ? bag[name] : record[name], at = name;
    for (const field of fields) {
      if (!has(got, field)) return { missing: `${at} has no field "${field}" (it is ${JSON.stringify(got)?.slice(0, 200)})`, tasks };
      got = got[field]; at += `.${field}`;
    }
    return { value: got, tasks };
  };
  if (['ios', 'host-ios', 'macos', 'host'].includes(s.carrier.host)) s.now = (await s.op({ op: 'clock', take: true })).clock;
  // Every look races the bound: one that answers past it counts nothing, so a value is never met late.
  const t0 = performance.now(), end = t0 + bound, met = (r) => r != null && 'value' in r && r.value === want;
  const look = () => {
    const r = read();
    r.catch(() => {}); // a look abandoned at the bound may fail later, unheard
    let timer;
    const late = new Promise((ok) => { timer = setTimeout(() => ok(null), Math.max(0, end - performance.now())); });
    return Promise.race([r, late]).finally(() => clearTimeout(timer));
  };
  let seen = await look();
  if (seen?.unknown) throw new MalformedOperation(`clock until state ${path}: no state named "${name}" (${seen.unknown.join(', ') || 'none declared'})`);
  while (!met(seen) && performance.now() < end) {
    await new Promise((ok) => setTimeout(ok, Math.max(0, Math.min(50, end - performance.now()))));
    if (performance.now() >= end) break;
    const got = await look();
    if (!got) break;
    seen = got;
  }
  const real = Math.round(performance.now() - t0);
  if (met(seen)) return { clock: s.now, real, until: 'met', value: seen.value };
  const last = seen == null ? 'no look answered within the bound' : 'value' in seen ? `${path} was ${JSON.stringify(seen.value)?.slice(0, 200)}` : seen.missing ?? `no state named "${name}"`;
  // A task due a commit later (`after(1, …)`) is a step the frozen clock never takes (Excord's `record`, which sends IDENTIFY).
  const soon = Object.entries(seen?.tasks ?? {}).filter(([, at]) => Number.isFinite(at) && at > s.now && at <= s.now + 1).map(([task]) => `\`${task}\``);
  const hint = soon.length ? `; ${soon.join(', ')} ${soon.length > 1 ? 'are' : 'is'} due a commit later (an \`after(1, …)\`), which the frozen clock never reaches: \`clock settle\` or \`clock +1\` before this step runs ${soon.length > 1 ? 'them' : 'it'}` : '';
  throw new Error(`clock until state ${path} == ${literal} within ${bound}: ${last} after ${real} ms of real time, the clock frozen at ${s.now} ms${hint}`);
}

/** A missed target's note on what is still out (`state.pending`): requests `clock data` lands, and held asks, whose
 * replies `clock held` waits for (LLP 1016.000 D9). */
export function inFlightNote(pending) {
  const named = (held) => [...new Set(pending.filter((p) => !!p.held === held).map((p) => p.name).filter(Boolean))];
  const waited = named(false), heldBy = named(true);
  return (waited.length ? `; ${waited.length} request${waited.length === 1 ? ' is' : 's are'} still in flight (${waited.join(', ')}): \`clock data\` waits for data, and \`state\` shows what is still pending` : '')
    + (heldBy.length ? `; ${heldBy.join(', ')} ${heldBy.length === 1 ? 'is' : 'are'} held by the server: \`clock held\` waits for ${heldBy.length === 1 ? 'its reply' : 'their replies'}` : '');
}
