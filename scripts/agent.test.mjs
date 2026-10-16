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
  assert.deepEqual(calls.slice(-2), [{ op: 'clock', take: true }, { op: 'clock', to: 0 }]);
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
