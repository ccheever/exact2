// `clock held` (scripts/agent-held.mjs, LLP 1016.000 D9) over a stand-in session: it waits for each held target's
// count of replies to rise, never takes a request let go (or no longer counted held) for a reply, and says who had
// none past its bound.
import { test, expect } from 'bun:test';
import { heldClock, inFlightNote } from './agent-held.mjs';

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
