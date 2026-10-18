// `drag`'s reply carries what the host journaled about a reorder during it (LLP 1102 §3.17), and
// keeps every field of the carrier's reply. `bun test ./scripts/agent-drag.test.mjs`.
import { test, expect } from 'bun:test';
import { dragTap, heldDown, heldPhase, heldRead, HELD_LEAD, HELD_READ, HELD_STEP, STROKES } from './agent-drag.mjs';
import { DRAG_BOUNDS, realTap, REACH_MS } from '../host/apple/touches.mjs';

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
 *  which ran while its finger was down; `fail` names a screenshot that throws. `keep(line)` writes a line between
 *  down and up as the CLI does, its `run` performing it as the drive would. */
function heldDrive({ fail } = {}) {
  const asked = [], ran = [];
  let down = false;
  const s = {
    contact: null, held: null,
    async layout() { return { nodes: [{ id: 7, x: 10, y: 10, w: 40, h: 20 }] }; },
    async clock(arg) { ran.push([`clock ${arg}`, down]); return { clock: arg }; },
    async tagged(r) { return { ...r, epoch: 1 }; },
    async screenshot(path, _window, _form, still) { if (path === fail) throw new Error('no picture'); ran.push([`screenshot ${path}`, down, s.held, still]); return { screenshot: path }; },
    async state(name) { ran.push([`state ${name}`, down]); return { slot: name }; },
  };
  // What `step` does with the lines these tests write: a screenshot that does not settle inside the touch (`s.held`).
  const perform = async (line) => {
    const [op, a] = line.split(/\s+/);
    if (op === 'screenshot') return [op, await s.screenshot(a, false, undefined, !!s.held)];
    if (op === 'clock') return [op, await s.clock(a)];
    return [op, await s.state(a)];
  };
  const keep = (line) => heldRead(s, line, () => perform(line));
  const carrier = {
    async input(id, phase, opts) {
      asked.push([id, phase, opts.drag.hold, opts.drag.reach]);
      down = true;
      const during = [];
      for (const op of opts.drag.during) during.push(await op());
      down = false;
      return { tapped: id, delivery: 'platform', during, drag: opts.drag };
    },
  };
  return { s, carrier, asked, ran, keep };
}

// The natural drive: `tap row down` "screenshot held.png" `tap up` shows the press under a real touch. The lines
// between run inside it in the order written, holds with them, a `clock +<ms>` counting toward the press, and each
// answers at the lift in that order.
test('the holds, reads and clock steps between a played contact\'s down and up run inside its touch, in order', async () => {
  const { s, carrier, asked, ran, keep } = heldDrive();
  await heldDown(s, { id: 7 });
  for (const line of ['screenshot held.png', 'clock +250', 'tap hold 100', 'tap hold', 'state counter']) expect(keep(line)).toBe(true);
  expect(keep('tap up')).toBe(false); // the contact's lift goes on to `tap`
  expect(ran).toEqual([]);
  const r = await heldPhase(s, carrier, 'up', {});
  // The whole hold, the lead, the longest wait for the touch to reach its view, a read's time for each read and a
  // step's for each hold and clock step; the reads need the touch to reach its view.
  expect(asked).toEqual([[7, 'drag', 350 + HELD_LEAD + REACH_MS + 2 * HELD_READ + 3 * HELD_STEP, true]]);
  expect(ran).toEqual([['screenshot held.png', true, 'tap #7 down', true], ['clock +250 real', true], ['clock +100 real', true], ['state counter', true]]);
  expect(s.heldReads).toEqual([
    ['screenshot', { screenshot: 'held.png' }], ['clock', { clock: '+250 real' }],
    ['tap', { phase: 'hold', at: [30, 20], delivery: 'platform', held: 350, epoch: 1 }], ['tap', { phase: 'hold', at: [30, 20], delivery: 'platform', held: 350, epoch: 1 }],
    ['state', { slot: 'counter' }],
  ]);
  expect(r).toEqual(expect.objectContaining({ phase: 'up', hold: 350, tapped: 7, delivery: 'platform' }));
  expect(r.during).toBeUndefined();
  expect([s.contact, s.held]).toEqual([null, null]);
});

test('a played contact refuses input and waiting clocks between, cancels with nothing pressed, and keeps the receipt of a failed read', async () => {
  const d = heldDrive();
  await heldDown(d.s, { id: 7 });
  for (const line of ['tap other', 'type field hi', 'screenshot a.png over 300 every 50', 'perf list during "tap x"', 'clock settle', 'clock data', 'clock 5000']) expect(() => d.keep(line)).toThrow('tap up or tap cancel');
  // Nor a step past the runner's bound, the allowance for the steps counted: many zero holds as well as a long one.
  expect(() => d.keep(`tap hold ${DRAG_BOUNDS.hold - HELD_LEAD - REACH_MS}`)).toThrow(`at most ${DRAG_BOUNDS.hold} ms`);
  expect(() => { for (let i = 0; i < DRAG_BOUNDS.hold / HELD_STEP; i++) d.keep('tap hold 0'); }).toThrow(`at most ${DRAG_BOUNDS.hold} ms`);
  d.s.contact.whole.queue.length = 0;
  for (const line of ['tap hold 50', 'screenshot a.png']) d.keep(line);
  expect(await heldPhase(d.s, d.carrier, 'cancel', {})).toEqual(expect.objectContaining({ note: 'nothing was pressed; the steps written before it ran after it, unpressed' }));
  expect(d.asked).toEqual([]);
  expect(d.ran).toEqual([['screenshot a.png', false, null, false]]);
  expect(d.s.heldReads.map(([op]) => op)).toEqual(['tap', 'screenshot']);

  const f = heldDrive({ fail: 'b.png' });
  await heldDown(f.s, { id: 7 });
  f.keep('screenshot b.png');
  f.keep('state');
  const r = await heldPhase(f.s, f.carrier, 'up', {});
  expect(r.landError).toBe('the touch was delivered; `screenshot b.png` while it was down failed: no picture');
  expect(f.s.heldReads).toEqual([['failed', { op: 'screenshot b.png', error: 'no picture' }], ['state', { slot: undefined }]]);
});

/** `realTap`'s still touch on node 7 (a view that reports it reached it, `reports`), its log saying it reached it on
 *  log read `reachedAt` (never, when null); the runner lifts when an op calls `lift`, else after `liftAfter` ms. */
function stillTouch({ reachedAt, liftAfter }) {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'NativeButtonIOS', reports: true, node: 7, session: 'main', generation: 1 };
  const t = { reads: 0, after: 0, lifted: false };
  const ask = async (req) => {
    if (req.aim) return { aim };
    // Ended in the read after the one that follows the ops, or once the runner lifted on its own.
    t.reads += 1;
    if (t.lift?.done) t.after += 1;
    return { log: t.after > 1 || t.lifted ? [began, { ...began, seq: 6, phase: 'ended', t: 2 }] : [began], reached: reachedAt != null && t.reads >= reachedAt ? [1] : [], truncated: false };
  };
  const touches = { async ask(req) {
    if (req.op === 'foreground') return { state: 'runningForeground' };
    return new Promise((done) => {
      t.lift = () => { t.lift.done = true; done({ injected: {} }); };
      if (liftAfter) setTimeout(() => { t.lifted = true; done({ injected: {} }); }, liftAfter);
    });
  } };
  return { t, run: (drag) => realTap({ ask, touches, id: 7, drag: { dx: 0, dy: 0, press: 0, over: 0, hold: 400, ...drag } }) };
}

// The ops during a still real touch wait for the view it landed on to have it (`reached`), which a scroll view delays;
// a held contact's reads, which need it, fail before they run when it never does.
test('a still real touch runs its ops once the touch has reached its view, and a held contact\'s fail without it', async () => {
  const a = stillTouch({ reachedAt: 3 }), seen = [];
  const r = await a.run({ during: [async () => { seen.push(a.t.reads >= 3); a.t.lift(); return 'read'; }] });
  expect(seen).toEqual([true]);
  expect(r).toEqual(expect.objectContaining({ during: ['read'], reached: expect.any(Number) }));

  const b = stillTouch({ reachedAt: null, liftAfter: REACH_MS + 200 }), ran = [];
  await expect(b.run({ reach: true, during: [async () => { ran.push(1); }] })).rejects.toThrow(`did not reach the view it landed on within ${REACH_MS} ms`);
  expect(ran).toEqual([]);

  // An op that outlasts the press stops the rest: none reads the screen unpressed.
  const c = stillTouch({ reachedAt: 1 }), later = [];
  const slow = async () => { c.t.lifted = true; c.t.lift(); return 'slow'; };
  await expect(c.run({ during: [slow, async () => { later.push(1); }] })).rejects.toThrow('the touch lifted before the ops during it finished');
  expect(later).toEqual([]);
});

// A still drag's ops wait for the touch to reach its view inside the finger's time: the runner holds that much longer,
// and the reply says the hold asked.
test('a still real drag with ops during it holds the wait for delivery longer, and says the hold asked', async () => {
  const asked = [];
  const s = { contact: null, held: null,
    async layout() { return { nodes: [{ id: 7, x: 10, y: 10, w: 40, h: 20 }], viewport: { w: 400, h: 800 } }; },
    async op() { return { lines: [], next: 0 }; },
    async landed(r) { return r; } };
  const carrier = { touches: true, async input(id, phase, opts) { asked.push(opts.drag.hold); return { tapped: id, delivery: 'platform', drag: { dx: 0, dy: 0, press: 0, over: 0, hold: opts.drag.hold } }; } };
  const run = (opts) => dragTap({ s, carrier, node: { id: 7 }, target: 'row', host: 'ios', timing: 'agent', tapRefusal: async (_, __, e) => e }, opts);
  const r = await run({ dx: 0, dy: 0, hold: 200, during: [async () => 'read'] });
  expect(r.drag.hold).toBe(200);
  await run({ dx: 0, dy: 0, hold: 200 });
  await run({ dx: 0, dy: 40, hold: 200, during: [async () => 'read'] });
  expect(asked).toEqual([200 + REACH_MS, 200, 200]);
});
