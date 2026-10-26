// `clock held` (scripts/agent-held.mjs, LLP 1016.000 D9) over a stand-in session: it waits for each held target's
// count of replies to rise, never takes a request let go (or no longer counted held) for a reply, and says who had
// none past its bound. `clock until state` (the same file) over a stand-in whose state changes on the wall clock.
import { test, expect } from 'bun:test';
import { heldClock, inFlightNote, untilClock } from './agent-held.mjs';

/** A session whose `state` is `states[i]`, the last repeated, answered through the host's light `held` op (or,
 * `heldOp: false`, only through `state`), and whose `clock data` is recorded. */
function session(states, host = 'web', { heldOp = true, slow = 0 } = {}) {
  let i = 0;
  const said = [];
  const s = {
    now: 7, carrier: { host },
    async op(req) {
      said.push(req);
      if (req.take) return { clock: 9 };
      if (req.op === 'tags') return { epoch: 3, incarnation: 1 };
      if (req.op === 'held' && !heldOp) throw new Error('held is not carried');
      if (slow && i > 0) await new Promise((ok) => setTimeout(ok, slow));
      const st = states[Math.min(i++, states.length - 1)];
      return req.op === 'held' ? { held: (st.pending ?? []).filter((p) => p.held), heldReplies: st.heldReplies ?? {} } : st;
    },
    async clock(spec) { said.push({ clock: spec }); return { clock: s.now, settled: true }; },
  };
  return { s, said };
}
const held = (ticket, name = 'item') => ({ name, ticket, held: true });

test('it waits for the reply, then lands its work with clock data', async () => {
  const { s, said } = session([{ pending: [held(1)] }, { pending: [held(1)] }, { pending: [], heldReplies: { item: 1 } }]);
  expect(await heldClock(s, [])).toEqual({ clock: 7, settled: true });
  expect(said.at(-1)).toEqual({ clock: 'data' });
});

test('a request let go, or no longer counted held, is no reply: past the bound it names the target', async () => {
  // Superseded: a new ticket for the same target, no reply counted.
  const { s } = session([{ pending: [held(1)], heldReplies: { item: 2 } }, { pending: [held(2)], heldReplies: { item: 2 } }]);
  const started = performance.now();
  expect(await heldClock(s, ['100ms'])).toMatchObject({ settled: false, reason: 'held', held: 1, heldTargets: ['item'] });
  expect(performance.now() - started).toBeGreaterThanOrEqual(90);
  // Out of the held list for a while (a loose fetch), no reply counted.
  const { s: t } = session([{ pending: [held(1)] }, { pending: [{ name: 'item', ticket: 1 }] }]);
  expect(await heldClock(t, ['item', '50ms'])).toMatchObject({ settled: false, heldTargets: ['item'] });
});

test('a reply followed at once by another held ask of the same answer counts', async () => {
  const { s } = session([{ pending: [held(1)], heldReplies: { item: 3 } }, { pending: [held(1)], heldReplies: { item: 4 } }]);
  expect(await heldClock(s, [])).toMatchObject({ settled: true });
});

test('a named target waits alone; an unknown one, or a bound that is no duration, refuses', async () => {
  const { s } = session([{ pending: [held(1), held(2, 'feed')] }, { pending: [held(2, 'feed')], heldReplies: { item: 1 } }]);
  expect(await heldClock(s, ['item'])).toMatchObject({ settled: true });
  const { s: u } = session([{ pending: [held(1)] }]);
  await expect(heldClock(u, ['other'])).rejects.toThrow('clock held: other is not a held request in flight (held: item)');
  await expect(heldClock(u, [`${'9'.repeat(400)}s`])).rejects.toThrow('a bound is up to an hour');
});

test('with nothing held it waits for none, and says so; on Apple it takes the clock over first', async () => {
  const { s, said } = session([{ pending: [] }], 'macos');
  expect(await heldClock(s, [])).toMatchObject({ settled: true, diagnostic: 'clock held: no held request was in flight, so it waited for none' });
  expect(said[0]).toEqual({ op: 'clock', take: true });
  expect(s.now).toBe(9);
});

test('it looks through the light held op, or through state where a host has none', async () => {
  const { s, said } = session([{ pending: [held(1)] }, { pending: [], heldReplies: { item: 1 } }]);
  await heldClock(s, []);
  expect(said.filter((r) => r.op === 'state')).toEqual([]);
  const { s: t, said: told } = session([{ pending: [held(1)] }, { pending: [], heldReplies: { item: 1 } }], 'web', { heldOp: false });
  expect(await heldClock(t, [])).toMatchObject({ settled: true });
  expect(told.some((r) => r.op === 'state')).toBe(true);
});

test('its looks back off to 200 ms and the bound stays exact', async () => {
  const { s, said } = session([{ pending: [held(1)] }]);
  const started = performance.now();
  expect(await heldClock(s, ['1s'])).toMatchObject({ settled: false, reason: 'held', epoch: 3 });
  const took = performance.now() - started;
  expect(took).toBeGreaterThanOrEqual(990);
  expect(took).toBeLessThan(1150);
  // 25, 50, 100, 200, 200, 200, then what is left: about nine looks in a second, not forty.
  expect(said.filter((r) => r.op === 'held').length).toBeLessThan(13);
});

test('a missed target names held asks apart, with the clock that waits for them', () => {
  expect(inFlightNote([{ name: 'feed', ticket: 1 }, { name: 'changes', ticket: 2, held: true }]))
    .toBe('; 1 request is still in flight (feed): `clock data` waits for data, and `state` shows what is still pending; changes is held by the server: `clock held` waits for its reply');
  expect(inFlightNote([])).toBe('');
});

test('a look that answers past the bound counts nothing', async () => {
  const { s } = session([{ pending: [held(1)] }, { pending: [], heldReplies: { item: 1 } }], 'web', { slow: 150 });
  const started = performance.now();
  expect(await heldClock(s, ['50ms'])).toMatchObject({ settled: false, reason: 'held', heldTargets: ['item'] });
  expect(performance.now() - started).toBeLessThan(140);
});

/** A session whose `gateway.state` is `before` until `after` ms of real time have passed since it was made, then
 * `ready`; it records every op, so a test sees that the clock never moved. */
function flipping(after, host = 'web', { slow = 0 } = {}) {
  const t0 = performance.now(), said = [];
  const s = {
    now: 7, carrier: { host },
    async op(req) {
      said.push(req);
      if (slow && req.op === 'state') await new Promise((ok) => setTimeout(ok, slow));
      if (req.take) return { clock: 9 };
      if (req.op === 'tags') return { epoch: 3, incarnation: 1 };
      if (req.land) return { clock: s.now };
      const state = performance.now() - t0 >= after ? 'ready' : 'identifying';
      return { resources: { gateway: { state, seq: 2 } }, slots: { rows: [{ n: -3 }] }, derives: {}, tasks: { record: 8, beat: 1000 } };
    },
  };
  return { s, said };
}

test('clock until state waits on the wall clock, the clock frozen, until the value holds', async () => {
  const { s, said } = flipping(120);
  const started = performance.now();
  const r = await untilClock(s, 'until state gateway.state == "ready" within 2000');
  expect(r).toEqual({ clock: 7, real: expect.any(Number), until: 'met', value: 'ready' });
  expect(performance.now() - started).toBeGreaterThanOrEqual(110);
  expect(r.real).toBeLessThan(1000);
  // Reads, each after landing what arrived: no clock op moved the app's time.
  const ops = said;
  expect(ops.length).toBeGreaterThanOrEqual(4);
  ops.forEach((q, i) => expect(q).toEqual(i % 2 ? { op: 'state' } : { op: 'clock', land: true }));
  // Already true: met at the first look. A list index, a negative number.
  expect(await untilClock(flipping(0).s, 'until state rows.0.n == -3 within 10')).toMatchObject({ until: 'met', value: -3 });
});

test('a miss fails at its bound naming the last value; a missing field is named as one', async () => {
  const started = performance.now();
  await expect(untilClock(flipping(60_000).s, 'until state gateway.state == "ready" within 150')).rejects.toThrow(
    /^clock until state gateway\.state == "ready" within 150: gateway\.state was "identifying" after \d+ ms of real time, the clock frozen at 7 ms; `record` is due a commit later \(an `after\(1, …\)`\), which the frozen clock never reaches: `clock settle` or `clock \+1` before this step runs it$/);
  expect(performance.now() - started).toBeGreaterThanOrEqual(145);
  await expect(untilClock(flipping(0).s, 'until state gateway.user == null within 60')).rejects.toThrow('gateway has no field "user"');
});

test('a look that answers past the bound counts nothing, a value met late included', async () => {
  const started = performance.now();
  await expect(untilClock(flipping(0, 'web', { slow: 300 }).s, 'until state gateway.state == "ready" within 50')).rejects.toThrow(
    'clock until state gateway.state == "ready" within 50: no look answered within the bound after');
  expect(performance.now() - started).toBeLessThan(250);
});

test('clock until state refuses what is not its form; on Apple it takes the clock over first', async () => {
  const { s, said } = flipping(0, 'ios');
  expect(await untilClock(s, 'until state gateway.state == "ready" within 10')).toMatchObject({ clock: 9, until: 'met' });
  expect(said[0]).toEqual({ op: 'clock', take: true });
  for (const [spec, says] of [
    ['until state gateway.state == ready within 10', 'ready is not one'],
    ['until state gateway == {} within 10', '{} is not one'],
    ['until state gateway == 1e400 within 10', '1e400 is not one'],
    ['until state gateway.state == "ready"', 'not `clock until state'],
    ['until state gateway.state == 1 within 0', 'a bound is 1 ms up to an hour'],
    ['until state nothing == 1 within 10', 'no state named "nothing" (gateway, rows)'],
  ]) await expect(untilClock(flipping(0).s, spec)).rejects.toThrow(says);
});
