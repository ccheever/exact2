// The driver's own words (scripts/agent.mjs): its grammar, how it renders a tree, and what it says when a native app hangs up.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { PassThrough } from 'node:stream';
import { open, jsonLines, MalformedOperation, main } from './agent.mjs';
import { chromium, launchFacts } from './agent-launch.mjs';
import { need } from './coverage.mjs';
import { typeCommand } from './agent-keys.mjs';

need('chrome', chromium().unavailable)('the web agent keeps desktop input facts through preferences and injected touches', async () => {
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: () => new Response(
    '<!doctype html><main id="exact-root" data-boot-ms="0"></main><script>globalThis.exact = {ready: Promise.resolve(), agentSettled: async () => ({})};</script>',
    { headers: { 'content-type': 'text/html' } },
  ) });
  let session;
  try {
    session = await open({ host: 'web', url: server.url.href });
    const { evaluate, call } = session.carrier;
    const input = () => evaluate(() => Object.fromEntries(
      ['pointer', 'any-pointer', 'hover', 'any-hover'].map(feature => [feature,
        (feature.endsWith('pointer') ? ['none', 'coarse', 'fine'] : ['none', 'hover'])
          .filter(value => matchMedia(`(${feature}: ${value})`).matches)]),
    ));
    const desktop = { pointer: ['fine'], 'any-pointer': ['fine'], hover: ['hover'], 'any-hover': ['hover'] };
    assert.deepEqual(await input(), desktop);
    await session.prefer({ 'prefers-color-scheme': 'dark' });
    await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: 20, y: 20 }] });
    await call('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    assert.deepEqual(await input(), desktop);
    await call('Emulation.setTouchEmulationEnabled', { enabled: true });
    assert.deepEqual(await input(), { pointer: ['coarse'], 'any-pointer': ['coarse'], hover: ['none'], 'any-hover': ['none'] });
  } finally {
    try { await session?.close(); } finally { server.stop(true); }
  }
}, 30_000);

test('malformed input is labelled before delivery; a state refusal is not', async () => {
  const carrier = { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
    ask() { throw new Error('unexpected delivery'); } };
  const s = await open({ reuse: carrier });
  for (const run of [() => s.clock('banana'), () => s.clock('+Infinity'),
    () => s.clock(new Number(Infinity)),
    () => s.resize(NaN, 400), () => s.tap('field', { wheel: [0, NaN] }),
    () => s.pointer('move', { dx: 0, dy: Infinity }), () => s.pointer('banana'),
    () => s.type('field', { key: 'Enter', phase: 'banana' }),
    () => s.type('missing', { key: 'Banana' }),
    () => s.type('field', { key: 'Enter', for: -1 }),
    () => s.tap('field', { pinch: 0 }), () => s.tap('field', { drag: { dx: 1, dy: NaN } })]) {
    await assert.rejects(run, e => e instanceof MalformedOperation && e.malformed === true);
  }
  for (const seconds of [undefined, null, -1, NaN, Infinity])
    await assert.rejects(() => s.tap('missing', { mediaSession: 'seekto', seconds }), e => e instanceof MalformedOperation && e.malformed === true);
  for (const opts of [{ dx: 1, dy: 2 }, { dx: 5 }, { x: 30 }, {}])
    await assert.rejects(() => s.pointer('move', opts), e => e.message.includes('no contact is down') && e.malformed === undefined);
  s.held = 'drag';
  await assert.rejects(() => s.type('missing', { key: 'Enter', for: -1 }), e => e.malformed === true);
  await assert.rejects(() => s.answer('type', '@1', ''), e => e.malformed === true);
  s.held = null;
  carrier.ask = async () => ({ error: 'timer refused the advance' });
  await assert.rejects(() => s.clock('+100'), e => e.message.includes('timer refused') && e.malformed === undefined);
  await assert.rejects(() => main(['web', 'resize banana'], { open: async () => ({ close() {} }) }), e => e.malformed === true);
});

test('accepted pointer, wheel, media and zero-time forms still reach the carrier', async () => {
  const calls = [], carrier = { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
    async ask(req) { calls.push(req); return req.op === 'clock' ? { clock: req.to ?? 0 } : { nodes: [{ id: 1, props: { testId: 'a' }, depth: 0 }] }; },
    async input(id, kind, opts) { calls.push([id, kind, opts]); return { at: [30, 40] }; },
    async reveal() { return {}; } };
  const s = await open({ reuse: carrier });
  s.landed = async r => r;
  for (const opts of [{ dx: 5 }, { x: 30 }, {}]) {
    s.contact = { x: 0, y: 0 };
    await s.pointer('move', opts);
    assert.deepEqual(calls.at(-1), [null, 'move', opts]);
  }
  s.contact = null;
  for (const wheel of [[1, 2, 3], { 0: 1, 1: 2 }]) {
    await s.tap('a', { wheel });
    assert.deepEqual(calls.at(-1), [1, 'wheel', { wheel }]);
  }
  for (const media of [{ mediaSession: 'play', seconds: null, at: [1, 2] }, { mediaSession: 'seekto', seconds: 0 }, { mediaSession: 'seekto', seconds: 1.5 }]) {
    await s.tap('a', media);
    assert.deepEqual(calls.at(-1), [1, 'mediasession', media]);
  }
  for (const spec of ['', '+', '   ', null, false, new Number(0)]) {
    await s.clock(spec);
    assert.deepEqual(calls.at(-1), { op: 'clock', to: 0 });
  }
  await s.clock(new Number(125));
  assert.deepEqual(calls.at(-1), { op: 'clock', to: 125 });
  carrier.host = 'macos';
  carrier.ask = async req => { calls.push(req); return { clock: req.take ? 0 : req.to }; };
  s.now = Infinity;
  await s.clock('+0 real');
  assert.deepEqual(calls.slice(-2), [{ op: 'clock', take: true }, { op: 'clock', to: 0, step: true }]); // real time passed: the host waits on no native motion
  for (const throws of [false, true]) {
    carrier.ask = async () => {
      const reply = { error: 'tap resize needs two dimensions', malformed: true };
      return throws ? MalformedOperation.reply(reply) : reply;
    };
    await assert.rejects(() => s.resize(100, 100), e => e instanceof MalformedOperation && e.message === 'tap: tap resize needs two dimensions');
  }
});

// A hold is the clock's (euxy FRICTION §2: a dice charge did nothing while held): the carrier is told it is virtual, no
// wall time passes, and the app's clock moves by it; under --timing platform it stays real time.
test('a held contact\'s hold moves the app\'s clock by its length', async () => {
  for (const timing of ['agent', 'platform']) {
    const calls = [], carrier = { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
      async ask(req) { calls.push(req); return req.op === 'clock' ? { clock: req.to ?? 0 } : {}; },
      async input(id, kind, opts) { calls.push([id, kind, opts]); return { phase: kind, at: [1, 2], delivery: 'platform' }; } };
    const s = await open({ reuse: carrier, timing });
    s.landed = async r => r;
    s.contact = { x: 1, y: 2 };
    await s.pointer('hold', { ms: 800 });
    if (timing === 'agent') assert.deepEqual(calls, [[null, 'hold', { ms: 800, virtual: true }], { op: 'clock', to: 800 }]);
    else assert.deepEqual(calls, [[null, 'hold', { ms: 800 }]]);
  }
});

// One finger at a time: a press while a contact is held would take over its pointer, as a second `down` would.
test('a press while a contact is held is refused before it reaches the carrier', async () => {
  const calls = [], carrier = { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
    async ask(req) { return req.op === 'clock' ? { clock: 0 } : { nodes: [{ id: 1, props: { testId: 'a' }, depth: 0 }] }; },
    async input(id, kind, opts) { calls.push([id, kind, opts]); return { at: [1, 2] }; },
    async reveal() { return {}; } };
  const s = await open({ reuse: carrier });
  s.landed = async r => r;
  s.contact = { x: 1, y: 2 };
  await assert.rejects(() => s.tap('a'), e => /a contact is already down/.test(e.message));
  assert.deepEqual(calls, []);
  s.contact = null;
  await s.tap('a');
  assert.deepEqual(calls.map(c => c[1]), ['press']);
});

test('held request answers bypass tap and key delivery validation', async () => {
  const calls = [], carrier = { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
    async ask(req) {
      if (req.op === 'state') return { pending: [{ ticket: 7, name: 'pick', device: { capability: 'prompt' } }] };
      calls.push(req); return { answered: 7, delivery: 'substituted' };
    } };
  const s = await open({ reuse: carrier });
  s.landed = async r => r;
  for (const value of [{ for: 'alice' }, { key: 'Banana', phase: 'banana', for: -1 }]) {
    await s.type('@pick', value);
    assert.deepEqual(calls.at(-1), { op: 'type', ticket: 7, text: JSON.stringify(value) });
  }
  await s.type(...typeCommand('type @pick key Enter banana'));
  assert.deepEqual(calls.at(-1), { op: 'type', ticket: 7, text: '{"key":"Enter","phase":"banana"}' });
  for (const opts of [{ wheel: [0, NaN] }, { pinch: 0 }, { seconds: Infinity }, { mediaSession: 'seekto' },
    { at: [NaN, 0] }, { modifiers: 'banana' }, { drag: { dx: 1, dy: NaN } }, { mouse: true, hover: true }, { clicks: 4 }]) {
    await s.tap('@pick', { choice: 'cancel', ...opts });
    assert.deepEqual(calls.at(-1), { op: 'tap', ticket: 7, choice: 'cancel' });
  }
  s.held = 'drag';
  await assert.rejects(() => s.type('@pick', { for: 'alice' }), /a drag's finger is down/);
  await assert.rejects(() => s.tap('@pick', { choice: 'cancel', pinch: 0 }), /a drag's finger is down/);
});

test('stdio preserves malformed replies and leaves state refusals unlabelled', async () => {
  const readable = new PassThrough(), writable = new PassThrough();
  const lines = jsonLines(readable, writable, []);
  const bad = lines.ask({ op: 'tap' });
  readable.write(JSON.stringify({ error: 'tap needs an id', malformed: true }) + '\n');
  await assert.rejects(bad, e => e instanceof MalformedOperation && e.reply.malformed === true);
  const refused = lines.ask({ op: 'tap', id: 7 });
  readable.write(JSON.stringify({ error: 'view 7 is hidden or inert' }) + '\n');
  assert.deepEqual(await refused, { error: 'view 7 is hidden or inert' });
  readable.destroy(); writable.destroy();
});

test('the driver grammar names an unquoted mediasession action beside pinch', () => {
  const source = readFileSync(new URL('./agent.mjs', import.meta.url), 'utf8');
  const header = source.split('\n').slice(0, 16).join('\n');
  const form = 'pinch <scale> [at <x> <y>] | mediasession <action> [seconds]';
  assert.ok(header.includes(form), header);
  const usage = spawnSync(process.execPath, [resolve(import.meta.dir, 'agent.mjs')], { encoding: 'utf8' });
  assert.equal(usage.status, 2, usage.stderr);
  assert.ok(usage.stderr.includes(form), usage.stderr);
}, 60_000);

test('rendered tree keeps focus; an accessible name is tree --ax\'s (LLP 1080.002)', async () => {
  const { render } = await import('./agent.mjs');
  const line = render('tree', {nodes:[{id:1, depth:0, type:'View', props:{testId:'play'}, focused:true}]});
  assert.match(line, /\[focused\]/);
  assert.doesNotMatch(line, /name=/);
});

test('a native hangup names how the app ended, its crash reports, and only its last 20 lines', async () => {
  const { hangup } = await import('./agent.mjs');
  const hostLines = Array.from({ length: 30 }, (_, i) => `app: line ${i}`);
  const hung = hangup({ what: 'the app hung up', pid: 999999, exit: { code: null, signal: 'SIGKILL' }, reports: ['/r/Caltrain-1.ips'], hostLines });
  assert.match(hung, /^the app hung up \(killed by SIGKILL\)\ncrash report: \/r\/Caltrain-1\.ips\napp: line 10\n/);
  assert.doesNotMatch(hung, /line 9\n/);
  assert.match(hangup({ what: 'clock did not answer', pid: process.pid }), /^clock did not answer \(pid \d+ still running\)$/);
  assert.match(hangup({ what: 'the app hung up', pid: 999999 }), /\(pid 999999 gone\)$/);
  assert.match(hangup({ what: 'the app exited', exit: { code: 3, signal: null } }), /\(exit code 3\)$/);
});


// `screenshot <png> compare <reference>` (agent-compare.mjs), on synthetic pictures: white, with boxes.
const picture = (w, h, boxes, scale = 1) => {
  const data = new Uint8Array(w * scale * h * scale * 4).fill(255);
  for (const [x, y, bw, bh, c = [20, 20, 20]] of boxes) for (let yy = Math.round(y * scale); yy < Math.round((y + bh) * scale); yy++) for (let xx = Math.round(x * scale); xx < Math.round((x + bw) * scale); xx++) data.set(c.length === 3 ? [...c, 255] : c, (yy * w * scale + xx) * 4);
  return { width: w * scale, height: h * scale, data };
};
const compared = async (ref, screen, w = 120, h = 200, options) => {
  const { grid, compareGrids } = await import('./agent-compare.mjs');
  const A = grid(picture(w, h, ref), w, h), B = grid(picture(w, h, screen), w, h);
  return { A, B, result: compareGrids(A, B, options) };
};
const reported = async (A, B, result, nodes = []) => {
  const { report, boxDiffs } = await import('./agent-compare.mjs');
  return report(result, nodes, { reference: 'r', diff: 'd', boxes: boxDiffs(A, B, result, nodes) });
};
const page = { id: 1, type: 'View', depth: 0, x: 0, y: 0, w: 120, h: 200 };
const box = (r) => [r.screen.x, r.screen.y, r.screen.w, r.screen.h];

test('compare: what one picture has and the other lacks is a region, both ways', async () => {
  // Content only the screen has.
  const added = await compared([], [[30, 60, 60, 60]]);
  assert.deepEqual(added.result.regions.map(box), [[30, 60, 60, 60]]);
  // Four bars in the reference, three here: the fourth is a region, with the reference's colour.
  const bars = (n) => Array.from({ length: n }, (_, k) => [10, 30 + 30 * k, 100, 10]);
  const deleted = await compared(bars(4), bars(3));
  assert.deepEqual(deleted.result.regions.map(box), [[10, 120, 100, 10]]);
  const text = await reported(deleted.A, deleted.B, deleted.result, [page]);
  assert.match(text, /1 \[10,120 100×10\] \(1000 pt²\) under View \[0,0 120×200\]: here #fff, in the reference #222/);
  assert.doesNotMatch(text, /sits/);
});

test('compare: an edge\'s anti-aliasing is no colour of the picture it is in', async () => {
  // The reference's box has a soft left edge (three grey columns), the screen's a hard one 7 pt further left: the
  // reference's differing cells are white and that edge's greys, which are no ink of its own (euxy port: "#bbb").
  const soft = [[24, 60, 1, 40, [200, 200, 200]], [25, 60, 1, 40, [140, 140, 140]], [26, 60, 1, 40, [80, 80, 80]], [27, 60, 40, 40, [0, 0, 0]]];
  const { result } = await compared(soft, [[20, 60, 47, 40, [0, 0, 0]]]);
  const { reference, screen } = result.regions[0].colours;
  assert.equal(reference.fill, '#fff');
  assert.equal(reference.ink, undefined);
  assert.equal(reference.edges, true);
  assert.equal(screen.fill, '#000');
  const { report } = await import('./agent-compare.mjs');
  const text = report({ ...result, regions: [{ ...result.regions[0], hint: null }] }, [], { reference: 'r', diff: 'd' });
  assert.match(text, new RegExp(`here #000, in the reference #fff, its edge's anti-aliasing ${reference.blend} beside it`));
  // A stroke's own colour is kept: a grey bar on white is its grey.
  const bar = await compared([[10, 40, 100, 6, [120, 120, 120]]], []);
  assert.equal(bar.result.regions[0].colours.reference.ink ?? bar.result.regions[0].colours.reference.fill, '#666');
});

test('compare: a 1 pt border between a lighter and a darker side keeps its colour', async () => {
  // A blue card on white with a grey border in the reference and a red one here: the grey lies between white and blue
  // in every channel's range but on no line between them, so it is the reference's own colour, not anti-aliasing.
  const card = (border) => [[29, 59, 42, 42, border], [30, 60, 40, 40, [0, 0, 255]]];
  const { result } = await compared(card([102, 102, 102]), card([204, 0, 0]));
  assert.ok(result.regions.length > 0);
  const { reference, screen } = result.regions[0].colours;
  assert.equal(reference.ink, '#666');
  assert.equal(reference.edges, undefined);
  assert.match(screen.ink, /^#[cd]00$/);
  const { report } = await import('./agent-compare.mjs');
  const text = report({ ...result, regions: [{ ...result.regions[0], hint: null }] }, [], { reference: 'r', diff: 'd' });
  assert.match(text, /here #[cd]00 on #\w{3}, in the reference #666 on #\w{3}/);
  // White, the border, a 1 pt blue strip, then black: grey is a mix of white and black, but the blue between them is
  // not, so the run is no blend and the grey stays the reference's colour.
  const striped = (border) => [[29, 60, 1, 40, border], [30, 60, 1, 40, [0, 0, 255]], [31, 60, 40, 40, [0, 0, 0]]];
  const strip = (await compared(striped([102, 102, 102]), striped([204, 0, 0]))).result;
  assert.ok(strip.regions.length > 0);
  const { reference: ref } = strip.regions[0].colours;
  assert.equal(ref.ink ?? ref.fill, '#666');
  assert.equal(ref.edges, undefined);
  // The same cell judged alone: the run white, grey, blue, black is no blend, nor is white, grey, blue.
  const { blended, grid } = await import('./agent-compare.mjs');
  const G = grid(picture(120, 200, striped([102, 102, 102])), 120, 200);
  assert.equal(blended(G, 70 * 120 + 29, () => true), false);
});

test('compare: a recoloured row among look-alikes is its colours, with no hint', async () => {
  const bars = (first) => [0, 1, 2, 3, 4].map((k) => [10, 30 + 20 * k, 100, 10, k ? [0, 0, 0] : first]);
  const { A, B, result } = await compared(bars([0, 0, 0]), bars([255, 0, 0]));
  const text = await reported(A, B, result);
  assert.match(text, /1 \[10,30 100×10\] \(1000 pt²\): here #f00, in the reference #000$/m);
});

test('compare: content moved is a region with a hint where the move is plain; none where it is not', async () => {
  // A row of uneven marks 4 pt right of the reference's (evenly spaced marks would match at several offsets: no hint).
  const marks = (dx) => [[20, 6], [30, 4], [44, 8], [60, 5]].map(([x, mw]) => [x + dx, 40, mw, 10]);
  const moved = await compared(marks(0), marks(4));
  assert.equal(moved.result.regions.length, 1);
  assert.deepEqual(moved.result.regions[0].hint, { axis: 'x', d: -4 });
  assert.match(await reported(moved.A, moved.B, moved.result), /its content sits 4 pt left in the reference/);
  // A block 8 pt lower here.
  const lower = await compared([[10, 30, 40, 10], [10, 50, 60, 10]], [[10, 38, 40, 10], [10, 58, 60, 10]]);
  assert.match(await reported(lower.A, lower.B, lower.result), /its content sits 8 pt higher in the reference/);
  // A box only here, and nothing like it nearby: no hint.
  assert.equal((await compared([], [[30, 60, 20, 20]])).result.regions[0].hint, null);
});

test('compare: a box is given at its size only where both pictures show all its edges, both sizes measured', async () => {
  // A 30×16 box grows to 40×24; nothing else moves.
  const { A, B, result } = await compared([[10, 40, 30, 16]], [[10, 40, 40, 24]]);
  const node = { id: 2, type: 'View', testId: 'box', depth: 1, x: 10, y: 40, w: 40, h: 24 };
  const text = await reported(A, B, result, [page, node]);
  assert.match(text, /box {4}box \[10,40 40×24\]: 30×16 at 10,40 in the reference/);
  assert.doesNotMatch(text, /40×16|30×24/);
  // A card centred here, at the left with its own width in the reference.
  const card = await compared([[10, 50, 100, 60, [30, 30, 30]]], [[20, 50, 90, 60, [30, 30, 30]]]);
  assert.match(await reported(card.A, card.B, card.result, [{ type: 'View', testId: 'card', depth: 2, x: 20, y: 50, w: 90, h: 60 }]), /card \[20,50 90×60\]: 100×60 at 10,50 in the reference/);
  // A node with no visible edge (text on white) is never sized.
  const text2 = await compared([[10, 40, 30, 6]], [[10, 40, 40, 6]]);
  assert.doesNotMatch(await reported(text2.A, text2.B, text2.result, [{ type: 'View', depth: 1, x: 10, y: 36, w: 50, h: 14 }]), /box {4}/);
});

test('compare: a region is named by the innermost node under its centre, a shown screen\'s before a covered one\'s', async () => {
  const { under } = await import('./agent-compare.mjs');
  const nodes = [page, { id: 2, type: 'View', testId: 'card', depth: 1, x: 10, y: 10, w: 100, h: 60 }, { id: 3, type: 'Text', text: 'Title', depth: 2, x: 20, y: 20, w: 40, h: 10 },
    { id: 4, type: 'View', testId: 'row', depth: 5, x: 0, y: 100, w: 120, h: 40, inactive: true }, { id: 5, type: 'View', testId: 'sheet', depth: 1, x: 0, y: 90, w: 120, h: 110 }];
  assert.deepEqual(under(nodes, { x: 30, y: 22, w: 10, h: 6 }).map((n) => n.id), [3]);
  assert.deepEqual(under(nodes, { x: 70, y: 40, w: 10, h: 6 }).map((n) => n.id), [2]);
  assert.deepEqual(under(nodes, { x: 50, y: 110, w: 10, h: 10 }).map((n) => n.id), [5]);
});

test('compare: anti-aliasing and transparency are what a viewer sees; rows not scored are not', async () => {
  const { grid, compareGrids } = await import('./agent-compare.mjs');
  // At 3 pixels a point, a box one pixel wider and lower: anti-aliasing, not a difference.
  const A = grid(picture(100, 100, [[10, 10, 30, 30]], 3), 100, 100), B = grid(picture(100, 100, [[10 + 1 / 3, 10 + 1 / 3, 30, 30]], 3), 100, 100);
  assert.equal(compareGrids(A, B).regions.length, 0);
  // At 2×, opaque red beside transparent black is pink over white, as the flattened picture is.
  const striped = { width: 40, height: 20, data: new Uint8Array(40 * 20 * 4) };
  for (let i = 0; i < 40 * 20; i++) if (i % 2 === 0) striped.data.set([255, 0, 0, 255], i * 4);
  assert.equal(compareGrids(grid(striped, 20, 10), grid(picture(20, 10, [[0, 0, 20, 10, [255, 128, 128]]]), 20, 10)).match, 1);
  // A status bar's clock and a home indicator only the reference has: not scored.
  const C = grid(picture(100, 100, [[10, 2, 20, 6], [30, 95, 40, 3]]), 100, 100), D = grid(picture(100, 100, [[50, 2, 40, 4, [0, 200, 0]]]), 100, 100);
  const left = compareGrids(C, D, { top: 10, bottom: 8 });
  assert.equal(left.match, 1);
  assert.equal(left.regions.length, 0);
  assert.ok(compareGrids(C, D).regions.length > 0);
});

test('compare: the CLI form and the reference are checked before any comparison', async () => {
  const { regions, compareScreenshot, referenceScale } = await import('./agent-compare.mjs');
  const w = 60, h = 20, mask = new Uint8Array(w * h);
  for (const x0 of [5, 25]) for (let y = 5; y < 10; y++) for (let x = x0; x < x0 + 6; x++) mask[y * w + x] = 1;
  const found = regions(mask, w, h);
  assert.equal(found.length, 1);
  assert.deepEqual([found[0].x, found[0].w], [5, 26]);
  const web = { host: 'web', screenshot() { throw new Error('captured'); } }, mac = { ...web, host: 'macos' };
  for (const [s, args] of [[web, ['compare', 'r.png']], [web, ['x.png', 'compare']], [web, ['x.png', 'window', 'compare', 'r.png']], [mac, ['x.png', 'window', 'compare', 'r.png']], [web, ['x.png', 'compare', 'r.png', 'extra']]])
    assert.throws(() => compareScreenshot(s, args), (e) => e.malformed === true);
  // A pixel short on either axis: under a pixel a point, refused before anything is shrunk.
  for (const [rw, rh, sw, sh] of [[390, 836, 390, 844], [1000, 999, 1000, 1000], [1024, 1365, 1024, 1366]])
    assert.throws(() => referenceScale({ width: rw, height: rh }, sw, sh), (e) => e.malformed === true && /under a pixel a point/.test(e.message));
  assert.equal(referenceScale({ width: 1206, height: 2622 }, 402, 874), 3);
  const { helpText } = await import('./help.mjs');
  assert.match(helpText('agent'), /screenshot <png> \[over <ms> every <ms>\] \[window\] {2}window: bars/);
  assert.match(helpText('agent'), /screenshot <png> compare <ref>/);
  assert.match(helpText('agent'), /\(a select, date or checkbox: its value\)/);
});

test('a native named store is kept unless the drive resets it, under its lock; a web one starts empty unless kept', async () => {
  const quiet = [console.log, console.error];
  console.log = () => {}; console.error = () => {};
  try {
    for (const [host, flags, fresh, locked] of [['ios', [], false, true], ['ios', ['--reset-storage'], true, true], ['android', ['--reset-storage'], true, true],
      ['macos', [], false, true], ['mac', [], false, true], ['linux', ['--reset-storage'], true, true], ['web', [], true, false], ['web', ['--keep-storage'], false, false],
      // A drive that compares starts empty everywhere unless kept (euxy port: dice rolls an earlier drive stored).
      ['ios', ['screenshot a.png compare ref.png'], true, true], ['ios', ['--keep-storage', 'screenshot a.png compare ref.png'], false, true]]) {
      let env = 'unopened', lock = null;
      const fakeOpen = async (o) => { env = o.env; return { state: async () => ({}), close: async () => {} }; };
      const order = [];
      const exclusiveNativeStore = async (key, launch) => { lock = key; order.push('lock'); return launch(); };
      // The device is resolved before the lock, and the lock is keyed by what it resolved to, not by how the drive named it.
      const resolveDevice = (h) => { order.push('resolve'); return `the one ${h} device`; };
      const opening = async (o) => { order.push('open'); return fakeOpen(o); };
      // The fake session takes no screenshot: a compare fails after the launch this reads.
      assert.equal(await main([host, '--storage', 's1', ...flags, 'state'], { open: opening, driveServer: async () => null, exclusiveNativeStore, resolveDevice }).catch((e) => (/compare/.test(e.message) ? 0 : Promise.reject(e))), 0);
      assert.equal(env?.EXACT_AGENT_STORAGE_FRESH === '1', fresh, `${host} ${flags.join(' ')}`);
      assert.equal(lock?.storage === 's1', locked, `${host} lock`);
      if (locked) { assert.equal(lock.device, `the one ${host} device`); assert.deepEqual(order, ['resolve', 'lock', 'open']); }
    }
  } finally { [console.log, console.error] = quiet; }
});

test('storage advice preserves canvas journal entries through the host seam', async () => {
  const { driveNotes } = await import('./agent-test.mjs');
  for (const probe of [s => s.landed({ tapped: true }), s => driveNotes(s, { host: 'linux' })('tap')]) {
    const journal = ['loop wind on'], lines = [], calls = [];
    let cursor = 0;
    const carrier = { host: 'web', browser: 'chrome', hostLines: [], launchFacts: launchFacts({}), reset() {},
      async ask(req) {
        calls.push(req.op);
        if (req.op !== 'logs') return { epoch: 0, clock: 0 };
        const from = cursor; cursor = journal.length;
        return { from: req.since, next: lines.length, lines: lines.slice(req.since),
          world: [{ canvas: 7, from, next: cursor, lines: journal.slice(from) }] };
      } };
    const s = await open({ reuse: carrier });
    carrier.host = 'linux';
    await probe(s);
    assert.ok(calls.includes('logs'), 'the input ran the storage probe');
    journal.push('beacon-1 lit', 'sfx chime');
    const logs = await s.logs();
    assert.deepEqual(logs.world, [
      { canvas: 7, from: 0, next: 1, lines: journal.slice(0, 1) },
      { canvas: 7, from: 1, next: 3, lines: journal.slice(1) },
    ]);
    assert.deepEqual((await s.logs()).world[0].lines, [], 'the caller consumes each entry once');
  }
});

test('canvas journal errors survive before and after buffered entries', async () => {
  const { peekLogs } = await import('./agent-test.mjs');
  const entry = { canvas: 7, from: 100, next: 101, lines: ['saved entry'] };
  const error = { canvas: 7, error: 'journal unavailable' };
  for (const rows of [[entry, error], [error, entry]]) {
    const pending = [...rows];
    const carrier = { host: 'web', browser: 'chrome', hostLines: [], launchFacts: launchFacts({}), reset() {},
      async ask(req) { return req.op === 'logs' ? { from: 0, next: 0, lines: [], world: pending.length ? [pending.shift()] : [] } : {}; } };
    const s = await open({ reuse: carrier });
    await peekLogs(s, 0);
    assert.deepEqual((await s.logs()).world, rows);
    assert.deepEqual((await s.logs()).world, [], 'entries and errors are consumed once');
  }
});

test('a failed journal read retains the buffered entries for retry', async () => {
  const { peekLogs, takeWorldLogs } = await import('./agent-test.mjs');
  const entry = { canvas: 7, from: 0, next: 1, lines: ['saved entry'] };
  const s = { op: async () => ({ world: [entry] }) };
  await peekLogs(s, 0);
  const broken = { [Symbol.iterator]() { throw new Error('cannot read batch'); } };
  assert.throws(() => takeWorldLogs(s, broken), /cannot read batch/);
  assert.deepEqual(takeWorldLogs(s, []), [entry]);
  assert.deepEqual(takeWorldLogs(s, []), []);
});

test('canvas journals separate restarted cursors and reset with a web reload', async () => {
  const { peekLogs, reloadWeb } = await import('./agent-test.mjs');
  const old = { canvas: 7, from: 100, next: 110, lines: ['old canvas'] };
  const fresh = { canvas: 7, from: 0, next: 1, lines: ['new canvas'] };
  let pending = [old, fresh];
  const carrier = { host: 'web', browser: 'chrome', hostLines: [], launchFacts: launchFacts({}),
    reset() { pending = [fresh]; },
    async ask(req) { return req.op === 'logs' ? { from: req.since, next: req.since, lines: [], world: pending.length ? [pending.shift()] : [] } : {}; } };
  const s = await open({ reuse: carrier });
  pending = [old, fresh];
  await peekLogs(s, 0);
  assert.deepEqual((await s.logs()).world, [old, fresh], 'replacement without reload keeps lifetimes separate');
  const restored = { canvas: 7, from: 5000, next: 9096, dropped: 5000, lines: ['restored canvas'] };
  pending = [old, restored];
  await peekLogs(s, 0);
  assert.deepEqual((await s.logs()).world, [old, restored], 'a replacement with a later cursor retains its own span and dropped count');
  assert.deepEqual((await s.logs()).world, [], 'both lifetimes are consumed once');
  const contiguous = { canvas: 7, from: 110, next: 4206, dropped: 110, lines: ['replacement at the old boundary'] };
  pending = [old, contiguous];
  await peekLogs(s, 0);
  assert.deepEqual((await s.logs()).world, [old, contiguous], 'equal boundaries do not merge replacement metadata into the old lifetime');
  assert.deepEqual((await s.logs()).world, [], 'equal-boundary lifetimes are consumed once');
  pending = [old];
  s.logCursor = 99;
  await peekLogs(s, 99);
  await reloadWeb(s);
  assert.equal(s.logCursor, 0);
  assert.deepEqual((await s.logs()).world, [fresh], 'reload discards the old page journal');
  assert.deepEqual((await s.logs()).world, []);

  let finish;
  carrier.ask = () => new Promise(resolve => { finish = resolve; });
  const inFlight = peekLogs(s, 0);
  carrier.ask = async () => ({});
  await reloadWeb(s);
  finish({ world: [old] });
  await inFlight;
  carrier.ask = async () => ({ from: 0, next: 0, lines: [], world: [fresh] });
  assert.deepEqual((await s.logs()).world, [fresh], 'a late probe cannot refill the old page buffer');
});

test('a drive with no store says its storage was refused after the first op that met it, a screenshot too, once', async () => {
  const { driveNotes, STORAGE_NOTE } = await import('./agent-test.mjs');
  const said = [], quiet = console.error;
  const lines = ['t=0 query lib: loadLibrary', 't=0 storage refused (agent): storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)'];
  const logs = (lines) => async ({ since }) => ({ lines: lines.slice(since ?? 0), next: lines.length });
  const s = { launchFacts: {}, op: logs(lines), state: async () => ({}) };
  console.error = (line) => said.push(line);
  try {
    const notes = driveNotes(s, { host: 'ios', storage: undefined });
    await notes('screenshot'); await notes('tap'); await notes('end');
    assert.deepEqual(said.filter((l) => l.includes('--storage')), [`note: ${STORAGE_NOTE}`]);
    said.length = 0;
    await driveNotes(s, { host: 'ios', storage: 's1' })('screenshot');
    assert.deepEqual(said, []);
    // A resource's own failure that only quotes the refusal is not taken for one.
    await driveNotes({ ...s, op: logs(['t=0 lib failed: Unavailable agent storage is unavailable in agent mode']) }, { host: 'web', storage: undefined })('end');
    assert.deepEqual(said, []);
  } finally { console.error = quiet; }
});

test('a screenshot with a system sheet held says it is not drawn, and where the real one is', async () => {
  const { driveNotes } = await import('./agent-test.mjs');
  const said = [], quiet = console.error;
  const pending = [{ name: 'share', ticket: 1, device: { capability: 'share', args: {} } }, { name: 'p', ticket: 2, device: { capability: 'pick', args: {} } }];
  const s = (touch, extra = {}) => ({ launchFacts: {}, carrier: { touch, ...extra }, op: async () => ({ lines: [], next: 0 }), state: async () => ({ pending }) });
  console.error = (line) => said.push(line);
  try {
    await driveNotes(s('agent', { installNote: 'kept the Release build' }), { host: 'ios', storage: 's1' })('screenshot');
    assert.equal(said.length, 3);
    assert.match(said[0], /^note: kept the Release build$/);
    assert.match(said[1], /no share sheet: under the agent it is held \(@1\).*under --touch platform the real one is presented/);
    assert.doesNotMatch(said[2], /--touch platform/);
    said.length = 0;
    // Under --touch platform the share sheet is presented: only the picker, which is not, is said.
    await driveNotes(s('platform'), { host: 'ios', storage: 's1' })('screenshot');
    assert.deepEqual(said.map((l) => /@(\d)/.exec(l)?.[1]), ['2']);
  } finally { console.error = quiet; }
});

test('a native store is one drive\'s at a time: a second is refused until the first closes', async () => {
  const { exclusiveNativeStore } = await import('./agent-launch.mjs');
  const { mkdtempSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const root = mkdtempSync(resolve(tmpdir(), 'store-lock-test-')), key = { host: 'ios', device: 'simulator X', appId: 'com.example.a', storage: 's1' };
  try {
    let closed = 0;
    const first = await exclusiveNativeStore(key, async () => ({ close: async () => { closed++; } }), root);
    await assert.rejects(exclusiveNativeStore(key, async () => ({ close: async () => {} }), root), /another drive of this store is open on simulator X/);
    const other = await exclusiveNativeStore({ ...key, storage: 's2' }, async () => ({ close: async () => {} }), root);
    await other.close();
    await first.close();
    assert.equal(closed, 1);
    const again = await exclusiveNativeStore(key, async () => ({ close: async () => {} }), root);
    await again.close();
    // A launch that fails releases the lock.
    await assert.rejects(exclusiveNativeStore(key, async () => { throw new Error('no app'); }, root), /no app/);
    await (await exclusiveNativeStore(key, async () => ({ close: async () => {} }), root)).close();
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('compare on a capture: nodes where the window has them, its safe area not scored, a scaled viewport refused, its notes said', async () => {
  const { compareOp } = await import('./agent-compare.mjs');
  const { encodePng } = await import('./png.mjs');
  const { mkdtempSync, writeFileSync: write, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'compare-test-'));
  try {
    // The reference's home indicator (a bar at the bottom) is absent from the capture; a card is darker here.
    write(resolve(dir, 'ref.png'), encodePng(picture(120, 200, [[10, 40, 60, 30], [40, 192, 40, 3]], 2)));
    write(resolve(dir, 'shot.png'), encodePng(picture(120, 200, [[10, 40, 60, 30, [120, 0, 0]]], 2)));
    const ios = (space) => ({
      host: 'ios',
      layout: async () => ({ nodes: [{ id: 1, x: 0, y: 0, w: 60, h: 200 }, { id: 2, x: -668, y: 40, w: 60, h: 30 }] }),
      tree: async () => ({ nodes: [{ id: 1, depth: 0, type: 'View', props: {} }, { id: 2, depth: 1, type: 'View', props: { testId: 'card' } }] }),
      op: async () => ({ node: { space } }),
    });
    const shot = { screenshot: resolve(dir, 'shot.png'), w: 120, h: 200, scale: 2, window: true, statusBar: false, safeArea: { top: 10, bottom: 12 }, note: 'the status bar is not in the capture (simctl io screenshot: failed)' };
    const r = await compareOp(ios({ viewport: { x: 0, y: 0, w: 60, h: 200 }, window: { x: 678, y: 0, w: 60, h: 200 } }), resolve(dir, 'ref.png'), async () => ({ ...shot }));
    assert.match(r.text, /under card \[10,40 60×30\]/);
    assert.doesNotMatch(r.text, /draws at/);
    // A floating sheet the platform draws scaled is said, in compare's report and in `layout`'s.
    const floating = { ...ios({ viewport: { x: 0, y: 0, w: 60, h: 200 }, window: { x: 678, y: 0, w: 60, h: 200 } }) };
    const plain = floating.layout;
    floating.layout = async () => ({ ...(await plain()), scaled: [{ route: '3', presentation: 'sheet', scale: 0.96 }] });
    const said = /route 3 is a sheet the platform draws at 0\.96 of its laid-out size: its boxes are the authored frames × 0\.96/;
    assert.match((await compareOp(floating, resolve(dir, 'ref.png'), async () => ({ ...shot }))).text, said);
    const { render } = await import('./agent-inspect.mjs');
    assert.match(render('layout', { viewport: { w: 402, h: 874 }, clock: 0, nodes: [], scaled: [{ route: '3', presentation: 'sheet', scale: 0.96 }] }), said);
    assert.match(r.text, /the top 10 \(status bar\) and the bottom 12 \(home indicator\) not scored/);
    assert.match(r.text, /capture: the status bar is not in the capture \(simctl io screenshot: failed\)/);
    assert.doesNotMatch(r.text, /\[40,192/);
    await assert.rejects(compareOp(ios({ viewport: { x: 0, y: 0, w: 60, h: 200 }, window: { x: 0, y: 0, w: 30, h: 100 } }), resolve(dir, 'ref.png'), async () => ({ ...shot })), (e) => e.malformed === true && /--size/.test(e.message));
    // macOS: what its viewport capture leaves out, as windows of their own.
    const mac = { host: 'macos', layout: async () => ({ nodes: [] }), tree: async () => ({ nodes: [] }) };
    const m = await compareOp(mac, resolve(dir, 'ref.png'), async () => ({ screenshot: resolve(dir, 'shot.png'), w: 120, h: 200, notShown: { windows: ['NSPopover'], see: 'screenshot <png> window' } }));
    assert.match(m.text, /capture: not in the capture, as windows of their own: NSPopover/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
