// `drag`'s reply carries what the host journaled about a reorder during it (LLP 1102 §3.17), and
// keeps every field of the carrier's reply. `bun test ./scripts/agent-drag.test.mjs`.
import { test, expect } from 'bun:test';
import { dragTap, heldDown, heldPhase, heldRead, heldTick, HELD_LEAD, HELD_READ, HELD_STEP, STROKES, TICK_STEP } from './agent-drag.mjs';
import { DRAG_BOUNDS, realTap, REACH_MS, TICK_MARGIN } from '../host/apple/touches.mjs';

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
  expect(r.note).toBeUndefined();
});

// euxy FRICTION 9: a sheet's scroller as tall as its form took drags that moved nothing, and said nothing.
test('an iOS drag that scrolls nothing says what to look at', async () => {
  const r = await iosDrive(() => ({ tapped: 9, at: [200, 370], scrolledBy: [0, 0], delivery: 'recognized', native: 'scroll' })).run();
  expect(r.note).toContain('nothing scrolled');
  expect(r.note).toContain('`layout content`');
  expect(r.note).toContain('lower-scroll-grows');
});

test('an iOS drag the host cannot recognize fails, naming --touch platform', async () => {
  const d = iosDrive(() => ({ error: 'drag #9 starts on nothing the agent scrolls, so it is a gesture only a real touch makes (a row\'s swipe actions, a slider, a pan): drive with --touch platform' }));
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
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], fixed: [30, 20], fixedAxes: [[1, 0], [0, 1]], display: [400, 800], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'NativeButtonIOS', reports: true, node: 7, session: 'main', generation: 1 };
  const t = { reads: 0, after: 0, lifted: false };
  const ask = async (req) => {
    if (req.aim) return { aim };
    // Ended in the read after the one that follows the ops, or once the runner lifted on its own.
    t.reads += 1;
    if (t.lift?.done) t.after += 1;
    const lifted = t.after > 1 || t.lifted;
    const moved = t.end?.some((v, i) => v !== began.at[i]) ? [{ ...began, seq: 6, phase: 'moved', at: t.end }] : [];
    return { now: 1 + (Date.now() - t.started) / 1000, log: lifted ? [began, ...moved, { ...began, seq: 7, phase: 'ended', t: 2, at: t.end ?? began.at }] : [began], reached: reachedAt != null && t.reads >= reachedAt ? [1] : [], truncated: false };
  };
  const touches = { async ask(req) {
    if (req.op === 'foreground') return { state: 'runningForeground' };
    t.started = Date.now();
    t.end = req.path?.at(-1)?.to ?? req.to;
    return new Promise((done) => {
      t.lift = () => { t.lift.done = true; done({ injected: {} }); };
      if (liftAfter) setTimeout(() => { t.lifted = true; done({ injected: {} }); }, liftAfter);
    });
  } };
  return { t, run: (drag) => realTap({ ask, touches, id: 7, drag: { dx: 0, dy: 0, press: 0, over: 0, hold: 400, ...drag } }) };
}

// A tap whose view left the window before the lift (the log's `viewGone`) says so in its reply, and the driver refuses
// nothing for it: the touch smoke taps a grouped row's detail button again when the state is unchanged.
test('a real tap whose view left the window before the lift says so', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 };
  const tap = (lift) => realTap({ id: 7,
    ask: async (req) => (req.aim ? { aim } : { log: [began, { ...began, seq: 6, phase: 'ended', t: 2, ...lift }], truncated: false }),
    touches: { async ask(req) { return req.op === 'foreground' ? { state: 'runningForeground' } : { injected: {} }; } } });
  expect(await tap({ viewGone: true })).toEqual(expect.objectContaining({ delivery: 'platform', viewGone: true }));
  expect((await tap({})).viewGone).toBeUndefined();
});

// A tap whose down the window had only at or after the finger's lift says so in its reply, with when the window had
// it, and the driver refuses nothing for it: the touch smoke taps a grouped row's switch again when the state is
// unchanged. Only a tap's down and lift arriving together were measured to lose anything: a contact that takes the
// drag path, as a held one does, says nothing.
test('a real tap whose down reached the window after its lift says so', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const tap = (dispatched, drag) => realTap({ id: 7, drag,
    ask: async (req) => (req.aim ? { aim } : { log: [{ seq: 5, touch: 1, phase: 'began', t: 1, dispatched, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 }, { seq: 6, touch: 1, phase: 'ended', t: 1.05, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 }], truncated: false }),
    touches: { async ask(req) { return req.op === 'foreground' ? { state: 'runningForeground' } : { injected: {} }; } } });
  const late = await tap(1.3);
  expect(late).toEqual(expect.objectContaining({ delivery: 'platform', downAfterLift: true }));
  expect(late.touch.dispatched).toBe(1.3);
  expect((await tap(1.05)).downAfterLift).toBe(true);
  expect((await tap(1.01)).downAfterLift).toBeUndefined();
  const unrecorded = await tap(undefined);
  expect(unrecorded.downAfterLift).toBeUndefined();
  expect('dispatched' in unrecorded.touch).toBe(false);
  expect((await tap(1.3, { dx: 0, dy: 0, press: 0, over: 0, hold: 0 })).downAfterLift).toBeUndefined();
});

test('a runner that finishes before a dispatch-log read sees the touch says whether the log has it', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, dispatched: 2.3, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 };
  for (const [log, says] of [[[began], 'before a dispatch-log read saw the touch begin, and it has not lifted:'], [[began, { ...began, seq: 6, phase: 'ended', t: 2 }], 'and it has lifted; lengthen hold'], [[], 'no touch has begun']]) {
    let injections = 0, logReads = 0, heldReads = 0, failure;
    try {
      await realTap({ id: 7, drag: { dx: 0, dy: 50, press: 0, over: 100, hold: 400, during: [async () => { heldReads++; }] },
        ask: async req => req.aim ? { aim } : { log: logReads++ ? log : [], truncated: false },
        touches: { async ask(req) {
          if (req.op === 'foreground') { return { state: 'runningForeground' }; }
          injections++;
          return { injected: { end: 1_700_000_000_000 } };
        } },
      });
    } catch (e) { failure = e; }
    expect(failure).toBeInstanceOf(Error);
    expect(failure.message).toContain(says);
    expect(failure.message.includes('can be lost')).toBe(log.length === 0);
    expect(failure.message).toContain(`"log":${JSON.stringify(log)}`);
    expect(failure.message).toContain('"end":1700000000000');
    expect(failure.message).toContain('"point":[30,20]');
    expect(injections).toBe(1);
    expect(heldReads).toBe(0);
  }
});

test('held timeouts name the unfinished work and advise a longer hold only for an operation', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 };
  await Promise.all(['the dispatch-log read', 'an operation during the hold', 'the held clock step'].map(async (work) => {
    let finishInjection, rejectPending, injections = 0, heldOps = 0, clockSteps = 0, abandoned, failure;
    const pending = new Promise((_, reject) => { rejectPending = reject; });
    const stall = () => { finishInjection({ injected: { end: 1_700_000_000_000 } }); return pending; };
    const ticking = work === 'the held clock step';
    try {
      await realTap({ id: 7,
        drag: { dx: 0, dy: 0, press: 0, over: 0, hold: 400,
          during: ticking ? [] : [async () => { heldOps++; return stall(); }, async () => { heldOps++; }],
          ...(ticking ? { tick: async () => { clockSteps++; return stall(); } } : {}),
        },
        ask: async req => req.aim ? { aim } : work === 'the dispatch-log read' ? stall() : { log: [began], truncated: false },
        touches: { async ask(req) {
          if (req.op === 'foreground') { return { state: 'runningForeground' }; }
          injections++;
          return new Promise(resolve => { finishInjection = resolve; });
        } },
        abandon(reason) { abandoned = reason; rejectPending(new Error(reason)); },
      });
    } catch (e) { failure = e; }
    expect(failure).toBeInstanceOf(Error);
    expect(failure.transport).toBe(true);
    expect(failure.message).toContain(`${work} did not finish within 1 s after the touch runner finished`);
    expect(failure.message.includes('lengthen hold')).toBe(work === 'an operation during the hold');
    expect(abandoned).toBe(failure.message);
    expect(injections).toBe(1);
    expect(heldOps).toBe(work === 'an operation during the hold' ? 1 : 0);
    expect(clockSteps).toBe(ticking ? 1 : 0);
  }));
});

test('a real touch refuses a changed display mapping even when scene coordinates agree', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], fixed: [30, 20], fixedAxes: [[1, 0], [0, 1]], display: [400, 800], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 };
  for (const [key, value] of [['fixed', [230, 20]], ['fixedAxes', [[0, 1], [-1, 0]]], ['display', [951, 669]]]) {
    let aims = 0, injected = 0;
    await expect(realTap({ id: 7,
      ask: async req => req.aim ? { aim: aims++ ? { ...aim, [key]: value } : aim } : { log: [began, { ...began, seq: 6, phase: 'ended', t: 2 }], truncated: false },
      touches: { async ask(req) {
        if (req.op === 'foreground') { return { state: 'runningForeground' }; }
        injected++;
        return { injected: {} };
      } },
    })).rejects.toThrow('the screen rotated or moved since aim');
    expect(injected).toBe(0);
  }
});

// On the Duo a real touch waits out SpringBoard's fold cloak (cloak.mjs) before its first aim, and a touch still lost
// says how long it waited.
test('a real tap waits out the fold cloak before it aims', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 };
  const tap = (log, order) => realTap({ id: 7,
    ask: async (req) => { order.push(req.aim ? 'aim' : 'log'); return req.aim ? { aim } : { log, truncated: false }; },
    touches: {
      async ask(req) { order.push(req.op); return req.op === 'foreground' ? { state: 'runningForeground' } : { injected: {} }; },
      cloak: { async clear() { order.push('clear'); await new Promise((r) => setTimeout(r, 30)); return 30; } },
    } });
  const order = [];
  expect(await tap([began, { ...began, seq: 6, phase: 'ended', t: 2 }], order)).toEqual(expect.objectContaining({ delivery: 'platform' }));
  expect(order.slice(0, 5)).toEqual(['clear', 'aim', 'foreground', 'aim', 'tap']);
  await expect(tap([], [])).rejects.toThrow('after waiting 30 ms for SpringBoard\'s fold cloak');
});

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

/** `realTap`'s still or moving touch on node 7 for a hold's clock (`tick`): the dispatch log answers `pages` in turn
 *  (its last again), an Error rejecting; the runner answers `runner` once `lift` is called or after `liftAfter` ms. */
function tickTouch({ pages, runner = { injected: {} }, liftAfter = 2000 }) {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const t = { reads: 0, settled: false };
  const ask = async (req) => {
    if (req.aim) return { aim };
    let page = pages[Math.min(t.reads++, pages.length - 1)];
    if (typeof page === 'function') page = await page(t);
    if (page instanceof Error) throw page;
    return { truncated: false, reached: [], ...page };
  };
  const touches = { async ask(req) {
    if (req.op === 'foreground') return { state: 'runningForeground' };
    return new Promise((done) => {
      const end = () => { t.settled = true; done(runner); };
      t.lift = end;
      if (liftAfter != null) setTimeout(end, liftAfter);
    });
  } };
  return { t, run: (drag) => realTap({ ask, touches, id: 7, drag: { dx: 0, dy: 0, press: 0, over: 0, hold: 400, ...drag } }) };
}
const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'NodeView', node: 7, session: 'main', generation: 1 };
const ended = { ...began, seq: 6, phase: 'ended', t: 2 };

// Round 2 (Astra, Grok): with no ops the clock waited for nothing; it now waits out the press and the move, as ops do.
test('a hold\'s clock starts after the press and the move', async () => {
  for (const drag of [{ press: 300 }, { dx: 0, dy: 40, over: 300 }]) {
    const a = tickTouch({ pages: [{ log: [began] }], liftAfter: 900 }), at = Date.now(), seen = [];
    await a.run({ ...drag, tick: async (until, down) => { seen.push([Date.now() - at, until - at, await down()]); a.t.lift(); } }).catch(() => {});
    expect(seen.length).toBe(1);
    expect(seen[0][0]).toBeGreaterThanOrEqual(290);
    expect(seen[0][1]).toBeGreaterThanOrEqual(300 + 400 - TICK_MARGIN);
    expect(seen[0][2]).toBe(true);
  }
});

// Round 2: a log reply that shows the touch began and ended moves no clock, nor does a runner that ended with no touch
// seen, whose error is the drag's; a log error is the drag's, said once the runner is done.
test('a hold\'s clock moves only while the touch is known to be down, and a failure is said once the runner is done', async () => {
  const ticked = [];
  const tick = async () => { ticked.push(1); };
  const lifted = tickTouch({ pages: [{ log: [began, ended] }], liftAfter: 10 });
  expect((await lifted.run({ tick })).touch).toEqual(expect.objectContaining({ began: 1, ended: 2 }));
  const failed = tickTouch({ pages: [{ log: [] }], runner: { error: 'XCTest crashed' }, liftAfter: 10 });
  await expect(failed.run({ tick })).rejects.toThrow('the touch runner: XCTest crashed');
  const refused = tickTouch({ pages: [{ error: 'no such session' }], liftAfter: 50 });
  await expect(refused.run({ tick })).rejects.toThrow('the dispatch log: no such session');
  expect(refused.t.settled).toBe(true);
  expect(ticked).toEqual([]);
  // `down` reads the log: false once it shows the lift.
  const late = tickTouch({ pages: [{ log: [began] }, { log: [began] }, { log: [began, ended] }], liftAfter: 300 });
  const downs = [];
  await late.run({ tick: async (_, down) => { downs.push(await down()); } }).catch(() => {});
  expect(downs).toEqual([false]);
});

// Round 2: a log read that rejects (the carrier gone mid-hold) is said only once the runner has let go, its error first.
test('a hold whose log read rejects waits for the runner, whose error wins', async () => {
  const gone = tickTouch({ pages: [new Error('carrier closed')], liftAfter: 50 });
  await expect(gone.run({ tick: async () => {} })).rejects.toThrow('carrier closed');
  expect(gone.t.settled).toBe(true);
  const both = tickTouch({ pages: [new Error('carrier closed')], runner: { error: 'XCTest crashed' }, liftAfter: 50 });
  await expect(both.run({ tick: async () => {} })).rejects.toThrow('the touch runner: XCTest crashed');
});

// heldTick: real-time steps of at most TICK_STEP while `down` says the finger is down, the rest at once before
// `until`; nothing once it has lifted.
test('a hold\'s clock steps while the finger is down and stops when it lifts', async () => {
  const s = { now: 0, held: null, steps: [], async clock(spec) { s.steps.push(spec); s.now += Number(/\+(\d+)/.exec(spec)[1]); } };
  await heldTick(s, 250, 'row')(Date.now() + 10_000, async () => true);
  expect(s.steps).toEqual([`+${TICK_STEP} real`, `+${TICK_STEP} real`, '+50 real']);
  s.steps.length = 0; s.now = 0;
  let reads = 0;
  await heldTick(s, 1000, 'row')(Date.now() + 10_000, async () => ++reads < 3);
  expect(s.steps).toEqual([`+${TICK_STEP} real`, `+${TICK_STEP} real`]);
  expect(s.held).toBe(null);
});

// A still drag's ops wait for the touch to reach its view inside the finger's time: the runner holds that much longer,
// and the reply says the hold asked. A hold moves the agent's clock by its length beside the finger (`tick`, after the
// author's ops), never by holding it longer (Astra round 1: a 100 ms hold held 650 ms is a long press); under platform
// timing the clock is the wall's and is not stepped.
function tickDrive() {
  const asked = [], ran = [];
  const s = { contact: null, held: null, now: 1000,
    async layout() { return { nodes: [{ id: 7, x: 10, y: 10, w: 40, h: 20 }], viewport: { w: 400, h: 800 } }; },
    async op() { return { lines: [], next: 0 }; },
    async clock(spec) { ran.push(`clock ${spec} ${s.held}`); s.now += Number(/^\+(\d+)/.exec(spec)[1]); return { clock: s.now }; },
    async landed(r) { return r; } };
  // As realTap: the ops in order, then the tick, given when the finger lifts less the margin.
  const carrier = { touches: true, async input(id, phase, opts) {
    asked.push(opts.drag.hold);
    const during = [];
    for (const op of opts.drag.during) during.push(await op());
    if (opts.drag.tick) await opts.drag.tick(Date.now() + opts.drag.hold - TICK_MARGIN, async () => true);
    return { tapped: id, delivery: 'platform', drag: { dx: 0, dy: 0, press: 0, over: 0, hold: opts.drag.hold }, ...(during.length ? { during } : {}) };
  } };
  const run = (opts, timing = 'agent') => dragTap({ s, carrier, node: { id: 7 }, target: 'row', host: 'ios', timing, tapRefusal: async (_, __, e) => e }, opts);
  return { s, asked, ran, run };
}

test('a real drag holds the finger as asked and moves the clock by the hold beside it', async () => {
  const d = tickDrive();
  const r = await d.run({ dx: 0, dy: 0, hold: 200, during: [async () => { d.ran.push('read'); return 'read'; }] });
  expect(r.drag.hold).toBe(200);
  expect(r.during).toEqual(['read']);
  expect(d.ran[0]).toBe('read');
  expect(d.ran.slice(1).every((line) => line.startsWith('clock +') && line.endsWith(' row'))).toBe(true);
  expect(d.s.now).toBe(1200);
  await d.run({ dx: 0, dy: 40, hold: 200, during: [async () => 'read'] });
  expect(d.s.now).toBe(1400);
  d.ran.length = 0;
  await d.run({ dx: 0, dy: 0, hold: 200 }, 'platform');
  await d.run({ dx: 0, dy: 40 });
  expect(d.ran).toEqual([]);
  expect(d.asked).toEqual([200 + REACH_MS, 200, 200, 0]);
});

// Astra round 1: `hold 100` was held 650 ms, past UIKit's 500 ms long press.
test('a short real hold is held as asked, and the clock still moves by it', async () => {
  const d = tickDrive();
  await d.run({ dx: 0, dy: 0, hold: 100 });
  expect(d.asked).toEqual([100]);
  expect(d.s.now).toBe(1100);
});

// Astra round 1: `hold 1000 during "clock +900 real"` stepped 900 then 1000 more inside a longer hold.
test('a hold whose ops moved the clock moves it only by the rest', async () => {
  const d = tickDrive();
  await d.run({ dx: 0, dy: 0, hold: 1000, during: [() => d.s.clock('+900 real')] });
  expect(d.asked).toEqual([1000 + REACH_MS]);
  expect(d.s.now).toBe(2000);
  const e = tickDrive();
  await e.run({ dx: 0, dy: 0, hold: 1000, during: [() => e.s.clock('+1200 real')] });
  expect(e.s.now).toBe(2200);
  expect(e.ran).toEqual(['clock +1200 real row']);
});

test('a scripted drag validates every leg and its complete duration', async () => {
  const { dragArguments } = await import('./agent-drag.mjs');
  expect(() => dragArguments({ dx: 0, dy: 40, hold: 300, path: [{ dx: 0, dy: -40, over: 0 }] })).toThrow('over > 0');
  expect(() => dragArguments({ dx: 0, dy: 40, hold: 6000, path: [{ dx: 0, dy: -40, hold: 6000 }] })).toThrow('gesture lasts past');
});


test('CLI path reads belong to each held leg and malformed paths refuse before injection', async () => {
  const { parseDrag } = await import('./agent-drag.mjs');
  const s = { state: async () => 'held' };
  const drag = parseDrag(s, '0 120 over 300 hold 500 during "state" then 0 -120 over 300 hold 500 during "state"');
  expect(drag.path).toHaveLength(1);
  expect(drag.path[0]).toEqual(expect.objectContaining({ dx: 0, dy: -120, over: 300, hold: 500 }));
  expect(await drag.during[0]()).toBe('held');
  expect(await drag.path[0].during[0]()).toBe('held');
  expect(() => parseDrag(s, '0 120 hold 500 during "tap other" then 0 -120')).toThrow('not a read');
  expect(() => parseDrag(s, '0 120 then 0')).toThrow('finite');
});


test('an initial stationary checkpoint waits for view delivery even when a later leg moves', async () => {
  const a = stillTouch({ reachedAt: 8, liftAfter: 650 }), seen = [];
  const r = await a.run({ hold: 500, during: [async () => { seen.push(a.t.reads >= 8); return 'pressed'; }],
    path: [{ dx: 0, dy: 100, over: 100, hold: 0 }] });
  expect(seen).toEqual([true]);
  expect(r.during).toEqual(['pressed']);
  expect(r.reached).toEqual(expect.any(Number));

  const b = stillTouch({ reachedAt: null, liftAfter: 250 }), ran = [];
  await expect(b.run({ hold: 100, during: [async () => { ran.push('unpressed'); }],
    path: [{ dx: 0, dy: 100, over: 100, hold: 0 }] })).rejects.toThrow('did not reach the view');
  expect(ran).toEqual([]);
});

test('a missed reversal endpoint reports the net displacement asked', async () => {
  const aim = { orientation: 'portrait', session: 'main', generation: 1, screen: { x: 0, y: 0, w: 400, h: 800 }, point: [30, 20], fixed: [30, 20], fixedAxes: [[1, 0], [0, 1]], display: [400, 800], viewport: [400, 800], at: [30, 20], window: [30, 20], seq: 4, hit: 7 };
  const began = { seq: 5, touch: 1, phase: 'began', t: 1, at: [30, 20], view: 'Control', node: 7, session: 'main', generation: 1 };
  await expect(realTap({ id: 7,
    drag: { dx: 0, dy: 40, press: 0, over: 100, hold: 0, path: [{ dx: 0, dy: -40, over: 100, hold: 0 }] },
    ask: async (req) => req.aim ? { aim } : { log: [began,
      { ...began, seq: 6, phase: 'moved', t: 1.1, at: [30, 60] },
      { ...began, seq: 7, phase: 'ended', t: 1.2, at: [30, 22] }], truncated: false },
    touches: { async ask(req) { return req.op === 'foreground' ? { state: 'runningForeground' } : { injected: {} }; } },
  })).rejects.toThrow('not the asked (0, 0)');
});


test('a runner that finishes during the hold log read advances no remaining clock', async () => {
  const a = tickTouch({ pages: [{ log: [began] }, { log: [began] }, t => { t.lift(); return { log: [began] }; }],
    runner: { error: 'XCTest stopped during the read' }, liftAfter: null });
  const steps = [], s = { now: 0, held: null, async clock(spec) { steps.push(spec); } };
  await expect(a.run({ hold: 100, tick: heldTick(s, 100, 'row') })).rejects.toThrow('XCTest stopped during the read');
  expect(steps).toEqual([]);
  expect(s.held).toBe(null);
});

test('a real clock step stops advancing as soon as its held touch ends', async () => {
  const { open } = await import('./agent.mjs');
  const { launchFacts } = await import('./agent-launch.mjs');
  const requests = [];
  let down = true;
  const s = await open({ reuse: { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
    async ask(req) { requests.push(req); down = false; return { clock: req.to }; } } });
  await heldTick(s, 1000, 'row')(Date.now() + 10_000, async () => down);
  expect(requests).toHaveLength(1);
  expect(s.now).toBe(requests[0].to);
  expect(s.held).toBe(null);
});
