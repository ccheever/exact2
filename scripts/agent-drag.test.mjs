// `drag`'s reply carries what the host journaled about a reorder during it (LLP 1102 §3.17), and
// keeps every field of the carrier's reply. `bun test ./scripts/agent-drag.test.mjs`.
import { test, expect } from 'bun:test';
import { dragTap, heldDown, heldPhase, heldRead, HELD_LEAD, HELD_READ, STROKES } from './agent-drag.mjs';
import { realTap } from '../host/apple/touches.mjs';

/** A session and carrier whose journal says `lines` during the drag; replies settle a tick later. */
function drive(lines, { touches = false, notes = {} } = {}) {
  const later = (v) => new Promise((done) => setTimeout(() => done(v), 1));
  const s = {
    contact: null,
    held: null,
    now: 0,
    async layout() { return later({ nodes: [{ id: 7, x: 10, y: 10, w: 40, h: 20 }], viewport: { w: 400, h: 800 } }); },
    async pointer(phase) { return later({ phase, at: [30, 60], delivery: 'platform', ...(notes[phase] ? { note: notes[phase] } : {}) }); },
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

// A phase's reply says a refused write once a drive (`landed`): the drag's reply carries it on (round-2 review).
test('a note a phase of the drag carried is the drag\'s', async () => {
  const r = await drive([], { notes: { move: 'a data source\'s write was refused' } });
  expect(r.note).toBe('a data source\'s write was refused');
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
    async input(id, phase, opts) { asked.push([phase, opts]); return later({ phase, delivery: 'unsupported', reason: 'no held contact across requests on iOS (LLP 1080.000 P3)' }); },
    async ask(req) { asked.push(req); return later(host(req)); },
  };
  return { asked, run: () => dragTap({ s, carrier, node: { id: 9 }, target: 'content', host: 'ios', timing: 'agent', tapRefusal: async (_, __, e) => e }, opts) };
}

// LLP 1116.003: a drag without --touch platform answered `delivery: unsupported` and exited 0 (r5 weather, mood journal).
test('an iOS drag without real touches scrolls the scroller it starts on', async () => {
  const d = iosDrive(() => ({ tapped: 9, at: [200, 370], scrolledBy: [0, 300], delivery: 'recognized', native: 'scroll' }));
  const r = await d.run();
  // Its contact is asked for a pan's alone (`pan`): a finger held elsewhere would drag nothing.
  expect(d.asked).toEqual([['down', { x: 200, y: 370, pan: true }], { op: 'tap', id: 9, scrollDrag: [0, -300] }]);
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
  expect(d.asked.filter((a) => !Array.isArray(a))).toEqual([]);
});

// euxy FRICTION 14: `drag 0 -1500` on a scroller was refused for leaving the viewport; it scrolls the whole way.
function strokeDrive(replies = () => ({})) {
  const strokes = [], node = { id: 9, type: 'ScrollView', x: 0, y: 0, w: 400, h: 800 };
  const s = { contact: null, async layout() { return { nodes: [node], viewport: { w: 400, h: 800 } }; },
    async op() { return { lines: [], next: 0 }; },
    async tagged(r) { return r; },
    async landed(r) { return { ...r, landed: true }; },
    async tap(target, { drag }) { strokes.push(drag); return { tapped: 9, drag, ...replies(strokes.length) }; } };
  const run = (opts, { host = 'web', carrier = {} } = {}) => dragTap({ s, carrier, node: { id: 9 }, target: 'list', host, timing: 'agent' }, opts);
  return { strokes, node, run };
}

test('a scroller dragged past the viewport scrolls in strokes that stay in it and lift at rest', async () => {
  const d = strokeDrive((i) => ({ scrolledBy: [0, i < 4 ? 399 : 303], ...(i === 1 ? { note: 'a write was refused' } : {}) }));
  const r = await d.run({ dx: 0, dy: -1500, over: 300, hold: 40 });
  // From the middle (400) to a point short of the top, each stroke 399 of the 1,500: three, then what is left.
  expect(d.strokes.map((k) => Math.round(k.dy))).toEqual([-399, -399, -399, -303]);
  // Each but the last rests before it lifts (no fling); the last keeps the drag's hold.
  expect(d.strokes.map((k) => k.hold)).toEqual([STROKES.rest, STROKES.rest, STROKES.rest, 40]);
  expect(r).toEqual(expect.objectContaining({ strokes: 4, scrolledBy: [0, 1500], note: 'a write was refused', drag: expect.objectContaining({ dx: 0, dy: -1500 }) }));
});

test('a stroke that fails or lands with an error stops the drag, and too many strokes are refused before any', async () => {
  const d = strokeDrive((i) => (i === 2 ? { landError: 'the answer failed' } : {}));
  await expect(d.run({ dx: 0, dy: -1500 })).rejects.toThrow('stroke 2 of 4: the answer failed');
  expect(d.strokes.length).toBe(2);
  const far = strokeDrive();
  await expect(far.run({ dx: 0, dy: -100000 })).rejects.toThrow(`past ${STROKES.most}`);
  expect(far.strokes).toEqual([]);
  far.node.type = 'View';
  await expect(far.run({ dx: 0, dy: -1500 })).rejects.toThrow('leaves the viewport');
});

test('on iOS a scroller takes the whole drag at once, and under real touches says it was no real touch', async () => {
  for (const touches of [null, {}]) {
    const d = strokeDrive(), asked = [];
    const carrier = { touches, async ask(req) { asked.push(req); return { tapped: 9, scrolledBy: [0, 1500], delivery: 'recognized' }; } };
    const r = await d.run({ dx: 0, dy: -1500 }, { host: 'ios', carrier });
    expect(asked).toEqual([{ op: 'tap', id: 9, scrollDrag: [0, -1500] }]);
    expect(d.strokes).toEqual([]);
    expect(r.scrolledBy).toEqual([0, 1500]);
    expect(Boolean(r.note?.includes('not a real touch'))).toBe(!!touches);
    expect(r.landed).toBe(true); // what the scroll settled lands before the reply
  }
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

/** A played contact (`--touch platform`) on node 7 and a carrier whose one touch runs the steps inside it, saying
 *  which ran while its finger was down; `fail` names a screenshot that throws. */
function heldDrive({ fail } = {}) {
  const asked = [], ran = [];
  let down = false;
  const s = {
    contact: null, held: null,
    async layout() { return { nodes: [{ id: 7, x: 10, y: 10, w: 40, h: 20 }] }; },
    async clock(arg) { ran.push([`clock ${arg}`, down]); return { clock: arg }; },
    async screenshot(path) { if (path === fail) throw new Error('no picture'); ran.push([`screenshot ${path}`, down, s.held]); return { screenshot: path }; },
    async state() { ran.push(['state', down]); return { slots: {} }; },
  };
  const carrier = {
    async input(id, phase, opts) {
      asked.push([id, phase, opts.drag.hold]);
      down = true;
      const during = [];
      for (const op of opts.drag.during) during.push(await op());
      down = false;
      return { tapped: id, delivery: 'platform', during, drag: opts.drag };
    },
  };
  return { s, carrier, asked, ran };
}

// The natural drive: `tap row down` "screenshot held.png" `tap up` shows the press under a real touch (the steps
// between run inside it, in order with its holds, a `clock +<ms>` counting toward the hold, answered at the lift).
test('the reads and clock steps between a played contact\'s down and up run inside its touch', async () => {
  const { s, carrier, asked, ran } = heldDrive();
  await heldDown(s, { id: 7 });
  for (const line of ['screenshot held.png', 'clock +250']) expect(heldRead(s, line)).toBe(true);
  expect(heldRead(s, 'tap hold 100')).toBe(false); // the contact's own phase goes on to `tap`
  expect(await heldPhase(s, carrier, 'hold', { ms: 100 })).toEqual(expect.objectContaining({ held: 350 }));
  expect(heldRead(s, 'state')).toBe(true);
  expect(ran).toEqual([]);
  const r = await heldPhase(s, carrier, 'up', {});
  // The whole hold, then the lead and a read's time for each of the two reads.
  expect(asked).toEqual([[7, 'drag', 350 + HELD_LEAD + 2 * HELD_READ]]);
  expect(ran).toEqual([['screenshot held.png', true, 'tap #7 down'], ['clock +250 real', true], ['clock +100 real', true], ['state', true]]);
  expect(s.heldReads.map(([op, reply]) => [op, Object.keys(reply)[0]])).toEqual([['screenshot', 'screenshot'], ['clock', 'clock'], ['state', 'slots']]);
  expect(r).toEqual(expect.objectContaining({ phase: 'up', hold: 350, tapped: 7, delivery: 'platform' }));
  expect(r.during).toBeUndefined();
  expect(s.contact).toBe(null);
  expect(s.held).toBe(null);
});

test('a played contact refuses input between, cancels with nothing pressed, and keeps the receipt of a failed read', async () => {
  const d = heldDrive();
  await heldDown(d.s, { id: 7 });
  for (const line of ['tap other', 'type field hi', 'screenshot a.png over 300 every 50', 'perf list during "tap x"']) expect(() => heldRead(d.s, line)).toThrow('tap up or tap cancel');
  expect(heldRead(d.s, 'screenshot a.png')).toBe(true);
  expect(await heldPhase(d.s, d.carrier, 'cancel', {})).toEqual(expect.objectContaining({ note: 'nothing was pressed; the steps written before it ran after it, unpressed' }));
  expect(d.asked).toEqual([]);
  expect(d.ran).toEqual([['screenshot a.png', false, null]]);

  const f = heldDrive({ fail: 'b.png' });
  await heldDown(f.s, { id: 7 });
  heldRead(f.s, 'screenshot b.png');
  heldRead(f.s, 'state');
  const r = await heldPhase(f.s, f.carrier, 'up', {});
  expect(r.landError).toBe('the touch was delivered; `screenshot b.png` while it was down failed: no picture');
  expect(f.s.heldReads).toEqual([['failed', { op: 'screenshot b.png', error: 'no picture' }], ['state', { slots: {} }]]);
});

// The ops during a still real touch wait for its node's `touchesBegan` (`reached`), which a scroll view delays.
test('a still real touch runs its ops once the touch has reached its node', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'NodeView', node: 7, session: 'main', generation: 1 };
  let reads = 0, after = 0, lift;
  const ask = async (req) => {
    if (req.aim) return { aim };
    // Reached on the third read of the log; ended in the read after the one that follows the ops.
    reads += 1;
    if (lift.done) after += 1;
    return { log: after > 1 ? [began, { ...began, seq: 6, phase: 'ended', t: 2 }] : [began], reached: reads >= 3 ? [1] : [], truncated: false };
  };
  const touches = { async ask(req) { return req.op === 'foreground' ? { state: 'runningForeground' } : new Promise((done) => { lift = () => { lift.done = true; done({ injected: {} }); }; }); } };
  const seen = [];
  const read = async () => { seen.push(reads >= 3); lift(); return 'read'; };
  const r = await realTap({ ask, touches, id: 7, drag: { dx: 0, dy: 0, press: 0, over: 0, hold: 400, during: [read] } });
  expect(seen).toEqual([true]);
  expect(r).toEqual(expect.objectContaining({ during: ['read'], reached: expect.any(Number) }));
});
