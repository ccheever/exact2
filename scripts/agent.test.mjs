// The driver's own words (scripts/agent.mjs): its grammar, how it renders a tree, and what it says when a native app hangs up.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { PassThrough } from 'node:stream';
import { open, jsonLines, MalformedOperation, main } from './agent.mjs';
import { launchFacts } from './agent-launch.mjs';
import { typeCommand } from './agent-keys.mjs';

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
const reported = async (result, nodes, A, B) => (await import('./agent-compare.mjs')).report(result, nodes, { reference: 'r', diff: 'd', A, B });
const page = { id: 1, type: 'View', depth: 0, x: 0, y: 0, w: 120, h: 200 };

test('compare: content moved down is one shift, in the box that grew, with its height measured in the reference', async () => {
  // A 20-pt key at y=30 grows to 28: the bars below it sit 8 pt lower.
  const { A, B, result } = await compared([[10, 30, 30, 20], [10, 90, 100, 6], [10, 130, 60, 6]], [[10, 30, 30, 28], [10, 98, 100, 6], [10, 138, 60, 6]]);
  assert.deepEqual(result.changes.map((c) => c.to - c.from), [8]);
  assert.equal(result.regions.length, 0);
  assert.ok(result.alignedMatch > 0.999 && result.match < 0.99, `${result.match} → ${result.alignedMatch}`);
  const text = await reported(result, [{ id: 2, type: 'View', testId: 'key', depth: 3, x: 10, y: 30, w: 30, h: 28 }, page], A, B);
  assert.match(text, /shift {2}from y≈\d+ down: 8 pt lower than the reference; key \[10,30 30×28\] is 30×28, 30×20 in the reference/);
});

test('compare: a gap that widened is named by the boxes either side; a page is never given a size it was not measured at', async () => {
  const { A, B, result } = await compared([[10, 30, 50, 10], [10, 60, 50, 10]], [[10, 30, 50, 10], [10, 66, 50, 10]]);
  const text = await reported(result, [page, { id: 2, type: 'Text', text: 'Note', depth: 2, x: 10, y: 30, w: 50, h: 10 }, { id: 3, type: 'Text', text: 'Track', depth: 2, x: 10, y: 66, w: 50, h: 10 }], A, B);
  assert.match(text, /the space between Text "Note" \[10,30 50×10\] and Text "Track" \[10,66 50×10\] is 6 pt wider than the reference's/);
  // The children unknown: the page holds the change, but its size in the reference is not what it is less the move.
  const alone = await reported(result, [page], A, B);
  assert.doesNotMatch(alone, /120×194|in the reference \(/);
  assert.match(alone, /6 pt lower than the reference/);
});

test('compare: a whole screen moved is a shift from the top, never "nothing differs"', async () => {
  const { A, B, result } = await compared([[10, 30, 40, 10], [10, 70, 60, 10], [10, 110, 30, 10]], [[10, 38, 40, 10], [10, 78, 60, 10], [10, 118, 30, 10]]);
  const text = await reported(result, [], A, B);
  assert.match(text, /shift {2}from the top down: 8 pt lower than the reference/);
  assert.doesNotMatch(text, /nothing differs/);
});

test('compare: content only the screen has is a region, whatever the alignment could skip', async () => {
  const { A, B, result } = await compared([], [[30, 60, 60, 60]]);
  assert.equal(result.changes.filter((c) => Math.abs(c.to - c.from) >= 2).length, 0);
  assert.ok(result.alignedMatch < 0.9);
  assert.deepEqual([result.regions[0].screen.x, result.regions[0].screen.y, result.regions[0].screen.w, result.regions[0].screen.h], [30, 60, 60, 60]);
  // Between content that did move: the new row is a region beside the shift.
  const moved = await compared([[10, 30, 100, 6], [10, 60, 100, 6]], [[10, 30, 100, 6], [40, 50, 20, 10], [10, 80, 100, 6]]);
  // (Its edge cells, a point from what matched, are within the anti-aliasing tolerance.)
  assert.ok(moved.result.regions.some((r) => r.screen.x <= 41 && r.screen.x + r.screen.w >= 59 && r.screen.y <= 51 && r.screen.y + r.screen.h >= 59), JSON.stringify(moved.result.regions.map((r) => r.screen)));
});

test('compare: a recoloured row among look-alikes is a colour, not the screen moving', async () => {
  const bars = (first) => [0, 1, 2, 3, 4].map((k) => [10, 30 + 20 * k, 100, 10, k ? [0, 0, 0] : first]);
  const { A, B, result } = await compared(bars([0, 0, 0]), bars([255, 0, 0]));
  const text = await reported(result, [page], A, B);
  assert.doesNotMatch(text, /shift/);
  assert.match(text, /differs \(\d+ pt²\): here #f00, in the reference #000; cause unclear/);
});

test('compare: a box that grew only downward is its measured height, never a sideways move', async () => {
  const { boxDiffs } = await import('./agent-compare.mjs');
  const { A, B, result } = await compared([[10, 40, 30, 16]], [[10, 40, 30, 24]]);
  const box = { id: 2, type: 'View', testId: 'box', depth: 1, x: 10, y: 40, w: 30, h: 24 };
  const text = await reported(result, [page, box], A, B);
  // Its bottom edge moved: a shift whose box was measured, never a sideways run or a screen narrower.
  assert.doesNotMatch(text, /right of|left of|narrower|wider/);
  assert.match(text, /box \[10,40 30×24\] is 30×24, 30×16 in the reference/);
  const [found] = boxDiffs(A, B, result, [page, box]);
  assert.deepEqual([found.node.testId, found.ref.w, found.ref.h], ['box', 30, 16]);
});

test('compare: a band moved sideways is a horizontal run; a recoloured box is a region with both colours', async () => {
  const { A, B, result } = await compared([[20, 40, 8, 12], [40, 40, 8, 12], [60, 120, 30, 20, [0, 0, 0]]], [[24, 40, 8, 12], [44, 40, 8, 12], [60, 120, 30, 20, [255, 0, 0]]]);
  assert.equal(result.changes.length, 0);
  const moved = result.regions.find((r) => r.screen.y < 60), recoloured = result.regions.find((r) => r.screen.y > 100);
  assert.deepEqual(moved.runs.map((r) => r.d), [4]);
  assert.ok(moved.explained > 0.9);
  assert.deepEqual([recoloured.screen.x, recoloured.screen.y, recoloured.screen.w, recoloured.screen.h], [60, 120, 30, 20]);
  const text = await reported(result, [], A, B);
  assert.match(text, /x \d+–\d+: 4 pt right of the reference/);
  assert.match(text, /differs \(600 pt²\): here #f00, in the reference #000/);
});

test('compare: edges a device pixel apart match; a transparent reference is what a viewer shows; the status bar is left out', async () => {
  const { grid, compareGrids } = await import('./agent-compare.mjs');
  // At 3 pixels a point, a box one pixel wider and lower: anti-aliasing, not a difference.
  const A = grid(picture(100, 100, [[10, 10, 30, 30]], 3), 100, 100), B = grid(picture(100, 100, [[10 + 1 / 3, 10 + 1 / 3, 30, 30]], 3), 100, 100);
  const same = compareGrids(A, B);
  assert.equal(same.match, 1);
  assert.equal(same.regions.length, 0);
  // At 2×, opaque red beside transparent black is pink over white, as the flattened picture is.
  const striped = { width: 40, height: 20, data: new Uint8Array(40 * 20 * 4) };
  for (let i = 0; i < 40 * 20; i++) if (i % 2 === 0) striped.data.set([255, 0, 0, 255], i * 4);
  assert.equal(compareGrids(grid(striped, 20, 10), grid(picture(20, 10, [[0, 0, 20, 10, [255, 128, 128]]]), 20, 10)).match, 1);
  const C = grid(picture(100, 100, [[0, 0, 100, 8]]), 100, 100), D = grid(picture(100, 100, [[50, 2, 40, 4, [0, 200, 0]]]), 100, 100);
  assert.equal(compareGrids(C, D, { top: 10 }).match, 1);
  assert.ok(compareGrids(C, D).match < 1);
});

test('compare: a visible box is found in the reference by its edges, at its size there', async () => {
  const { boxDiffs } = await import('./agent-compare.mjs');
  // A card 100×60 at x=10 in the reference is 90×60 at x=20 here.
  const { A, B, result } = await compared([[10, 50, 100, 60, [30, 30, 30]]], [[20, 50, 90, 60, [30, 30, 30]]]);
  const card = { type: 'View', testId: 'card', depth: 2, x: 20, y: 50, w: 90, h: 60 };
  const [found] = boxDiffs(A, B, result, [card]);
  assert.equal(found.node, card);
  assert.deepEqual([found.ref.x, found.ref.w, found.ref.h], [10, 100, 60]);
});

test('compare: only nodes that paint are named; a covered screen\'s node shows above a sheet, not under it', async () => {
  const { viewNodes, under } = await import('./agent-compare.mjs');
  const tree = { nodes: [
    { id: 1, depth: 0, type: 'View', props: {} },
    { id: 2, parent: 1, depth: 1, type: 'Text', props: { text: 'Ghost', visibility: 'hidden' } },
    { id: 3, parent: 1, depth: 1, type: 'Text', props: { text: 'Shown' } },
    { id: 4, parent: 1, depth: 1, type: 'View', props: { testId: 'scroller' } },
    { id: 5, parent: 4, depth: 2, type: 'Text', props: { text: 'Scrolled away' } },
    { id: 6, parent: 1, depth: 1, type: 'View', props: { testId: 'list' }, inactive: true },
    { id: 7, parent: 6, depth: 2, type: 'View', props: { testId: 'row-top' }, inactive: true },
    { id: 8, parent: 6, depth: 2, type: 'View', props: { testId: 'row-under' }, inactive: true },
    { id: 9, parent: 1, depth: 1, type: 'View', props: { testId: 'sheet' } },
  ] };
  const layout = { nodes: [{ id: 1, x: 0, y: 0, w: 120, h: 200 }, { id: 2, x: 10, y: 10, w: 40, h: 10 }, { id: 3, x: 10, y: 10, w: 40, h: 10 },
    { id: 4, x: 0, y: 30, w: 120, h: 20, sy: 0 }, { id: 5, x: 0, y: 60, w: 120, h: 10 },
    { id: 6, x: 0, y: 80, w: 120, h: 120 }, { id: 7, x: 0, y: 80, w: 120, h: 20 }, { id: 8, x: 0, y: 150, w: 120, h: 20 }, { id: 9, x: 0, y: 110, w: 120, h: 90 }] };
  const nodes = viewNodes(layout, tree), ids = nodes.map((n) => n.id);
  assert.ok(!ids.includes(2) && !ids.includes(5) && !ids.includes(8), JSON.stringify(ids));
  assert.ok(ids.includes(3) && ids.includes(7));
  assert.deepEqual(under(nodes, { x: 10, y: 10, w: 40, h: 10 }).map((n) => n.id), [3]);
  // Placed where the window has the viewport (an iPad's split): boxes move with it.
  assert.equal(viewNodes(layout, tree, [678, 0]).find((n) => n.id === 3).x, 688);
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
  // One percent short: not one picture a point of it.
  assert.throws(() => referenceScale({ width: 390, height: 836 }, 390, 844), (e) => e.malformed === true && /under a pixel a point/.test(e.message));
  assert.equal(referenceScale({ width: 1206, height: 2622 }, 402, 874), 3);
  const { helpText } = await import('./help.mjs');
  assert.match(helpText('agent'), /screenshot <png> \[over <ms> every <ms>\] \[window\] {2}window: bars/);
  assert.match(helpText('agent'), /screenshot <png> compare <ref\.png>/);
});

test('compare on an iOS window: nodes where the window has them, the capture\'s notes said, a missing home indicator left out', async () => {
  const { compareOp } = await import('./agent-compare.mjs');
  const { encodePng } = await import('./png.mjs');
  const { mkdtempSync, writeFileSync: write, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'compare-test-'));
  try {
    // The reference's home indicator (a bar at the bottom) is absent from the capture; a card moved 10 pt right.
    write(resolve(dir, 'ref.png'), encodePng(picture(120, 200, [[10, 40, 60, 30], [40, 192, 40, 3]], 2)));
    write(resolve(dir, 'shot.png'), encodePng(picture(120, 200, [[20, 40, 60, 30]], 2)));
    const s = {
      host: 'ios',
      layout: async () => ({ nodes: [{ id: 1, x: 0, y: 0, w: 60, h: 200 }, { id: 2, x: -658, y: 40, w: 60, h: 30 }], screen: { x: 678, y: 0 } }),
      tree: async () => ({ nodes: [{ id: 1, depth: 0, type: 'View', props: {} }, { id: 2, parent: 1, depth: 1, type: 'View', props: { testId: 'card' } }] }),
      op: async () => ({ node: { space: { viewport: { x: 0, y: 0 }, window: { x: 678, y: 0 } } } }),
    };
    const r = await compareOp(s, resolve(dir, 'ref.png'), async () => ({ screenshot: resolve(dir, 'shot.png'), w: 120, h: 200, scale: 2, window: true, statusBar: false, statusBarHeight: 10, homeIndicator: 8, note: 'the status bar is not in the capture (simctl io screenshot: failed)' }));
    assert.match(r.text, /card \[20,40 60×30\]/);
    assert.match(r.text, /the status bar's 10 pt and the bottom 8 pt \(a home indicator the capture lacks\) left out/);
    assert.match(r.text, /capture: the status bar is not in the capture \(simctl io screenshot: failed\)/);
    assert.doesNotMatch(r.text, /\[40,192/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
