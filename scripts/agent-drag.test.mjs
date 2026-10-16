// `drag`'s reply carries what the host journaled about a reorder during it (LLP 1102 §3.17), and
// keeps every field of the carrier's reply. `bun test ./scripts/agent-drag.test.mjs`.
import { test, expect } from 'bun:test';
import { dragTap } from './agent-drag.mjs';

/** A session and carrier whose journal says `lines` during the drag; replies settle a tick later. */
function drive(lines, { touches = false } = {}) {
  const later = (v) => new Promise((done) => setTimeout(() => done(v), 1));
  const s = {
    contact: null,
    held: null,
    now: 0,
    async layout() { return later({ nodes: [{ id: 7, x: 10, y: 10, w: 40, h: 20 }], viewport: { w: 400, h: 800 } }); },
    async pointer(phase) { return later({ phase, at: [30, 60], delivery: 'platform' }); },
    async op(req) {
      if (req.op !== 'logs') return later({});
      return later(req.since === Number.MAX_SAFE_INTEGER ? { lines: [], next: 5 } : { lines, next: 5 + lines.length });
    },
    async tagged(r) { return later({ ...r, epoch: 3, incarnation: 1, clock: 0 }); },
    async landed(r) { return later({ ...r, epoch: 3, incarnation: 1, clock: 0 }); },
  };
  const carrier = {
    touches,
    async input(id, phase) { return later(phase === 'drag' ? { tapped: id, delivery: 'platform' } : { at: [30, 20], delivery: 'platform', contact: true }); },
  };
  return dragTap({ s, carrier, node: { id: 7 }, target: 'grip-a', host: 'web', timing: 'agent', tapRefusal: async (_, __, e) => e }, { dx: 0, dy: 40 });
}

const refusal = 't=12 reorder: a drag refused: the last drop is held until its move shows (LLP 1094 D8)';

test('a refused drag names the refusal and keeps the reply', async () => {
  const r = await drive([refusal, 't=13 act move → epoch 2']);
  expect(r.note).toBe('reorder: a drag refused: the last drop is held until its move shows (LLP 1094 D8)');
  expect(r.target).toBe('grip-a');
  expect(r.delivery).toBe('platform');
  expect(r.epoch).toBe(3);
  expect(r.drag).toEqual(expect.objectContaining({ dx: 0, dy: 40 }));
});

test('a drag the host said nothing about has no note', async () => {
  const r = await drive(['t=13 act move → epoch 2']);
  expect(r.note).toBeUndefined();
  expect(r.target).toBe('grip-a');
});

test('a touch drag keeps its reply beside the note', async () => {
  const r = await drive([refusal], { touches: true });
  expect(r.note).toContain('reorder: a drag refused');
  expect(r.tapped).toBe(7);
  expect(r.target).toBe('grip-a');
  expect(r.epoch).toBe(3);
});

/** An iOS drive without real touches: its carrier holds no contact, and the host answers the scroll drag `scrollDrag`. */
function iosDrive(host, opts = { dx: 0, dy: -300 }) {
  const later = (v) => new Promise((done) => setTimeout(() => done(v), 1));
  const asked = [];
  const s = {
    contact: null, held: null, now: 0,
    async layout() { return later({ nodes: [{ id: 9, x: 0, y: 50, w: 400, h: 640 }], viewport: { w: 400, h: 800 } }); },
    async pointer(phase) { return later({ phase, delivery: 'unsupported' }); },
    async op(req) { return later(req.op === 'logs' ? { lines: [], next: 5 } : {}); },
    async tagged(r) { return later({ ...r, epoch: 3, incarnation: 1, clock: 0 }); },
  };
  const carrier = {
    touches: null,
    async input(id, phase) { return later({ phase, delivery: 'unsupported', reason: 'no held contact across requests on iOS (LLP 1080.000 P3)' }); },
    async ask(req) { asked.push(req); return later(host(req)); },
  };
  return { asked, run: () => dragTap({ s, carrier, node: { id: 9 }, target: 'content', host: 'ios', timing: 'agent', tapRefusal: async (_, __, e) => e }, opts) };
}

// LLP 1116.003: a drag without --touch platform answered `delivery: unsupported` and exited 0 (r5 weather, mood journal).
test('an iOS drag without real touches scrolls the scroller it starts on', async () => {
  const d = iosDrive(() => ({ tapped: 9, at: [200, 370], scrolledBy: [0, 300], delivery: 'recognized', native: 'scroll' }));
  const r = await d.run();
  expect(d.asked).toEqual([{ op: 'tap', id: 9, scrollDrag: [0, -300] }]);
  expect(r.delivery).toBe('recognized');
  expect(r.scrolledBy).toEqual([0, 300]);
  expect(r.drag).toEqual(expect.objectContaining({ dx: 0, dy: -300 }));
});

test('an iOS drag the host cannot recognize fails, naming --touch platform', async () => {
  const d = iosDrive(() => ({ error: 'drag #9: without --touch platform a drag scrolls a scroller it starts on' }));
  await expect(d.run()).rejects.toThrow('--touch platform');
});

test('an iOS drag with ops during it needs a held finger', async () => {
  const d = iosDrive(() => ({}), { dx: 0, dy: -300, hold: 100, during: [async () => ({})] });
  await expect(d.run()).rejects.toThrow('--touch platform');
  expect(d.asked).toEqual([]);
});

test('drag-to movement depends on layout and fails as a refusal', async () => {
  for (const [x, options, message] of [
    [50, { over: 0 }, 'a drag that moves needs over > 0'],
    [50, { press: 5000, hold: 5000, over: 200 }, 'the gesture lasts past 10000 ms'],
    [NaN, {}, 'expected finite dx, dy'],
  ]) {
    const s = { find: async () => ({ id: 2 }), layout: async () => ({ nodes: [
      { id: 1, x: 0, y: 0, w: 10, h: 10 }, { id: 2, x, y: 0, w: 10, h: 10 },
    ] }) };
    const error = await dragTap({ s, node: { id: 1 } }, { to: 'b', ...options }).catch(e => e);
    expect(error.message).toContain(message);
    expect(error.malformed).toBeUndefined();
  }
  const error = await dragTap({}, { to: 'b', over: -1 }).catch(e => e);
  expect(error.malformed).toBe(true);
});
