import { test, expect } from 'bun:test';
import { conformArgs } from './conform-args.mjs';
import { clockOnAll, clockVerdict, parseStep } from './conform-steps.mjs';
import { open } from '../../scripts/agent.mjs';
import { launchFacts } from '../../scripts/agent-launch.mjs';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

test('malformed steps fail as not a step, including steps marked refused', async () => {
  const s = await open({ reuse: { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
    ask() { throw new Error('malformed input reached a host'); } } });
  const lines = ['refused key field Enter banana', 'refused key missing Banana', 'refused prefer prefers-color-scheme banana',
    'refused mediasession missing seekto -1', 'refused mediasession missing seekto',
    'refused prefer display-mode fullscreen segments 0x1', 'refused prefer display-mode fullscreen segments 2x1 gap -1',
    'drag a to b 300 hold -1', 'drag a 1 2 300 h0ld -1', 'refused', 'refused tpa x',
    'refused resize', 'refused resize 10', 'refused key field Enter for', 'refused key field Enter for -1',
    'refused prefer online true has-focus', 'refused clock banana', 'clock banana', 'clock +100 real',
    'refused wheel strip x', 'refused drag a 1 2 3 hold', 'refused move 1', 'refused pinch a big',
    'refused pinch a 0', 'drag a 1 2 -5', 'refused toString x', 'tap a extra', 'up extra'];
  for (const line of lines) {
    const error = await (async () => parseStep(line).run(s))().catch(e => e);
    expect(error.malformed, line).toBe(true);
    expect(error.message, line).toStartWith(`not a step: ${line}: `);
  }
});

test('malformed input fails at parse time before a browser can skip it', () => {
  for (const mark of ['', 'refused ']) for (const line of [
    'drag a 1 2 -5', 'drag a to b -1', 'drag a 1 2 300 hold -1',
    'move 1 2 -1', 'move NaN 2', 'pinch a 0', 'pinch a Infinity',
    'into missing row banana', 'into list row Start',
  ]) expect(() => parseStep(mark + line)).toThrow(`not a step: ${mark}${line}: `);
  expect(() => parseStep('refused')).toThrow('not a step: refused: missing operation');
});

test('host malformed replies fail before refusal comparison; state refusals keep their message', async () => {
  for (const marked of ['', 'refused ']) {
    const step = parseStep(`${marked}tap field`);
    await expect(step.run({ tap: async () => ({ error: 'bad argument', malformed: true }) })).rejects.toThrow(`not a step: ${marked}tap field: bad argument`);
    const refusal = new Error('hidden or inert\ndetail');
    expect(await step.run({ tap: async () => { throw refusal; } }).catch(e => e)).toBe(refusal);
  }
});

test('every existing step parses and keeps the driver call forms', async () => {
  const root = resolve(import.meta.dir, 'conformance');
  const files = readdirSync(root, { recursive: true }).filter(f => f.endsWith('.steps'));
  const record = async line => {
    const calls = [], s = Object.fromEntries(['tap', 'type', 'pointer', 'clock', 'resize', 'prefer'].map(method => [method, async (...args) => { calls.push([method, args]); }]));
    await parseStep(line).run(s); return calls;
  };
  let count = 0;
  for (const file of files) for (const line of readFileSync(resolve(root, file), 'utf8').split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'))) {
    expect((await record(line)).length, `${file}: ${line}`).toBeGreaterThan(0); count++;
  }
  expect(count).toBeGreaterThan(0);
  const forms = [
    ['tap a', [['tap', ['a']]]],
    ['menu a', [['tap', ['a', { contextmenu: true }]]]],
    ['type field several words', [['type', ['field', 'several words']]]],
    ['key field Enter down', [['type', ['field', { key: 'Enter', phase: 'down' }]]]],
    ['key field Enter for 500', [['type', ['field', { key: 'Enter', for: 500 }]]]],
    ['clock +100', [['clock', ['+100']]]],
    ['back a', [['tap', ['a', { history: -1 }]]]],
    ['wheel strip 20', [['tap', ['strip', { wheel: [0, 20] }]]]],
    ['wheel strip 20 10', [['tap', ['strip', { wheel: [10, 20] }]]]],
    ['into list row center', [['tap', ['list', { into: { key: 'row', block: 'center' } }]]]],
    ['pinch a 2', [['tap', ['a', { pinch: 2 }]]]],
    ['down a', [['tap', ['a', { down: true }]]]],
    ['up', [['pointer', ['up']]]],
    ['move 1 2', [['pointer', ['move', { dx: 1, dy: 2, ms: 200 }]]]],
    ['drag a to b', [['tap', ['a', { drag: { to: 'b', over: 200 } }]]]],
    ['drag a 1 2 300 hold 100', [['tap', ['a', { drag: { dx: 1, dy: 2, over: 300, hold: 100 } }]]]],
    ['drag a 1 2 300', [['tap', ['a', { down: true }]], ['pointer', ['move', { dx: 1, dy: 2, ms: 300 }]], ['pointer', ['up']]]],
    ['mediasession a play', [['tap', ['a', { mediaSession: 'play' }]]]],
    ['mediasession a seekto 5', [['tap', ['a', { mediaSession: 'seekto', seconds: 5 }]]]],
    ['prefer online true has-focus false', [['prefer', [{ online: 'true', 'has-focus': 'false' }]]]],
    ['resize 800x600', [['resize', [800, 600]]]],
  ];
  for (const [line, calls] of forms) expect(await record(line), line).toEqual(calls);
});

test('conformance selects menus without treating the job count as an app', () => {
  const { options, named } = conformArgs(['menus', '--jobs', '1', '--build', '--strict']);
  expect(named).toEqual(['menus']);
  expect(options['--jobs']).toBe('1');
});

test('conformance consumes every value option between named apps', () => {
  const values = {
    '--wasm-root': 'wasm', '--out': 'report', '--steps': '0', '--browser': 'firefox',
    '--only': 'menus', '--label': 'probe', '--jobs': '2', '--js-from': 'earlier',
  };
  const { options, named } = conformArgs(['caltrain', ...Object.entries(values).flat(), 'menus']);
  expect(named).toEqual(['caltrain', 'menus']);
  expect(options).toEqual(values);
  const defaults = conformArgs([]);
  expect(defaults.named).toEqual([]);
  expect(defaults.options['--steps']).toBe(10);
  expect(defaults.options['--jobs']).toBeGreaterThanOrEqual(1);
  expect(defaults.options['--jobs']).toBeLessThanOrEqual(4);
});

test('URL comparison does not select its URLs or label as apps', () => {
  const { options, named } = conformArgs([
    '--urls', 'caltrain', 'http://localhost:8000', 'http://localhost:8001', '--label', 'preview',
  ]);
  expect(named).toEqual([]);
  expect(options['--label']).toBe('preview');
});

test("a { slots } Linux entry skips only the slots it names; a stop skips none", async () => {
  const { withoutSlots } = await import('./conform-linux.mjs');
  const slots = { geometry: 'caret width 0', sizes: 'caret 0×18', draft: 'hi' };
  expect(withoutSlots({ slots: ['sizes'], why: 'fonts' }, slots)).toEqual({ geometry: 'caret width 0', draft: 'hi' });
  expect(withoutSlots({ slots: ['nope'], why: 'x' }, slots)).toEqual(slots);
  expect(withoutSlots(['tap measure', 'why'], slots)).toEqual(slots);
  expect(withoutSlots(undefined, slots)).toEqual(slots);
  expect(withoutSlots({ slots: ['sizes'] }, undefined)).toBe(undefined);
});

test('a clock refused alike with Poisoned on every target is a step; one-sided, differing or other refusals fail', () => {
  const wasm = 'clock: Poisoned', js = 'clock: t=0 refused advance: Poisoned';
  expect(clockVerdict([['wasm', { error: wasm }], ['js', { error: js }]])).toEqual({ refused: 'Poisoned' });
  expect(clockVerdict([['wasm', { error: wasm }], ['js', { error: js }], ['linux', { error: 'clock: Poisoned' }]])).toEqual({ refused: 'Poisoned' });
  expect(clockVerdict([['wasm', { value: { clock: 5 } }], ['js', { value: { clock: 5 } }]])).toEqual({ answered: [{ clock: 5 }, { clock: 5 }] });
  expect(clockVerdict([['wasm', { error: wasm }], ['js', { value: { clock: 0 } }]]).failure).toBe('wasm refused «Poisoned», js answered');
  expect(clockVerdict([['wasm', { value: {} }], ['js', { error: js }]]).failure).toBe('wasm answered, js refused «Poisoned»');
  expect(clockVerdict([['wasm', { error: wasm }], ['js', { error: 'clock: t=0 refused advance: NonFiniteClock (NaN)' }]]).failure)
    .toBe('wasm refused «Poisoned», js refused «NonFiniteClock (NaN)»');
  expect(clockVerdict([['wasm', { error: 'clock: TimerFireLimit' }], ['js', { error: 'clock: TimerFireLimit' }]]).failure)
    .toBe('wasm refused «TimerFireLimit», js refused «TimerFireLimit»');
});

test("the drive's clock step holds Linux to the pages' Poisoned refusal, and asks it nothing when they answer", async () => {
  const pair = (left, right) => Promise.all([left(), right()]);
  const asked = [], session = (who, answer) => ({ clock: async arg => { asked.push([who, arg]); return answer(); } });
  const refuse = text => () => { throw new Error(text); };
  const pages = [['wasm', session('wasm', refuse('clock: clock: Poisoned'))], ['js', session('js', refuse('clock: clock: t=0 refused advance: Poisoned'))]];
  expect(await clockOnAll('settle', pages, session('linux', refuse('clock: clock: Poisoned')), pair))
    .toEqual({ refused: 'Poisoned', compared: ['wasm', 'js', 'linux'] });
  expect(await clockOnAll('settle', pages, null, pair)).toEqual({ refused: 'Poisoned', compared: ['wasm', 'js'] });
  expect(await clockOnAll('+60000', pages, session('linux', () => ({ clock: 50, settled: true })), pair))
    .toEqual({ refused: 'Poisoned', linux: 'wasm refused «Poisoned», js refused «Poisoned», linux answered' });
  expect((await clockOnAll('settle', pages, session('linux', refuse('clock: clock: TimerFireLimit { limit: 4096 }')), pair)).linux)
    .toBe('wasm refused «Poisoned», js refused «Poisoned», linux refused «TimerFireLimit { limit: 4096 }»');
  asked.length = 0;
  const answered = [['wasm', session('wasm', () => ({ clock: 5 }))], ['js', session('js', () => ({ clock: 5 }))]];
  expect(await clockOnAll('settle', answered, session('linux', () => ({ clock: 5 })), pair)).toEqual({ answered: [{ clock: 5 }, { clock: 5 }], compared: ['wasm', 'js'] });
  expect(asked.map(([who]) => who)).toEqual(['wasm', 'js']);
  const malformed = Object.assign(new Error('bad'), { malformed: true });
  await expect(clockOnAll('settle', [pages[0], ['js', session('js', () => { throw malformed; })]], null, pair)).rejects.toBe(malformed);
});

test('the wasm reference marker covers exactly photo-editor, recorder and markdown, with exactly its reason', async () => {
  const { WASM_UNBOUND, WASM_UNBOUND_REASON } = await import('./conform-marked.mjs');
  // Interim (theme2 + the lead, 2026-10-30): the landing fix removes it. A fourth app or another reason fails here.
  expect(WASM_UNBOUND_REASON).toBe('reference does not bind packages (LLP 1071 retirement)');
  expect(Object.keys(WASM_UNBOUND).sort()).toEqual(['markdown', 'photo-editor', 'recorder']);
  for (const reason of Object.values(WASM_UNBOUND)) expect(reason).toBe('reference does not bind packages (LLP 1071 retirement)');
  expect(Object.isFrozen(WASM_UNBOUND)).toBe(true);
  expect(Object.getPrototypeOf(WASM_UNBOUND)).toBe(Object.prototype);
});

test("the marker records a marked target's differences, counted; other targets' differences and every non-difference fail", async () => {
  const { WASM_UNBOUND_REASON, markUnbound } = await import('./conform-marked.mjs');
  const report = { failures: [
    { target: 'recorder', step: 'boot', what: 'state slots.voice: wasm null js "x"', difference: true },
    { target: 'recorder', step: 'boot', what: 'tree #3: wasm «a» js «b»', difference: true },
    { target: 'recorder', step: 'tap record', what: 'screenshot: 4.00% of pixels differ', difference: true },
    { target: 'recorder', step: 'wasm-open', what: 'refused: stale dist' },
    { target: 'recorder', step: 'tap stop', what: 'js: hidden or inert' },
    { target: 'recorder', step: 'tap stop state', what: 'TypeError: crash' },
    { target: 'markdown', step: 'wasm-build', what: 'error[E0425]' },
    { target: 'photo-editor', step: 'tests', what: 'wasm test x FAIL' },
    { target: 'caltrain', step: 'boot', what: 'tree #0: wasm «a» js «b»', difference: true },
    { target: 'synthetic-recorder', step: 'boot', what: 'state x', difference: true },
    { target: 'toString', step: 'boot', what: 'state x', difference: true },
  ] };
  markUnbound(report);
  expect(report.marked).toEqual({ recorder: { reason: WASM_UNBOUND_REASON, differences: 3, steps: ['boot', 'tap record'], lines: [
    'boot: state slots.voice: wasm null js "x"', 'boot: tree #3: wasm «a» js «b»', 'tap record: screenshot: 4.00% of pixels differ'] } });
  expect(report.failures.map(f => `${f.target} ${f.step}`)).toEqual(['recorder wasm-open', 'recorder tap stop', 'recorder tap stop state',
    'markdown wasm-build', 'photo-editor tests', 'caltrain boot', 'synthetic-recorder boot', 'toString boot']);
  const clean = markUnbound({ failures: [] });
  expect(clean).toEqual({ failures: [], marked: {} });
});

test('every wasm-vs-JS comparison site records through differ, which alone tags a difference; Linux and refusals do not', () => {
  const source = readFileSync(resolve(import.meta.dir, 'conform.mjs'), 'utf8');
  expect(source).toContain('const differ = (step, what, field = null) => fail(step, what, field, !crossBrowser);');
  const sites = source.split('\n').filter(l => /\bdiffer\(step,/.test(l));
  expect(sites.length).toBe(7);
  for (const [i, mark] of ["differ(step, 'state ' + x)", 'differ(step, x, crossBrowser ? `tree.${i}` : null)', 'differ(step, x.what, crossBrowser ? x.field : null)',
    'op.forEach(x => differ(step, x))', 'differ(step, `screenshot: ', 'differ(step, `bound refusals: ', 'differ(step, `sound lines: '].entries()) expect(sites[i], mark).toContain(mark);
  // No other call passes `fail`'s difference flag, and Linux's comparisons fail as they are.
  expect(source.split('\n').filter(l => /\bfail\([^\n]*,\s*(?:true|!crossBrowser)\)/.test(l) && !l.includes('const differ ='))).toEqual([]);
  expect(source).toContain("o.forEach(x => fail(step, 'linux ' + x));");
  expect(source).toContain('ol.forEach(x => fail(step, x.what));');
  expect(source).toContain('referenceTapRefused({ target: t.name, step: `tap ${id}`');
});

test("a marked target's refused reference tap stays a failure, never marked; elsewhere it is skipped as before", async () => {
  const { markUnbound, referenceTapRefused } = await import('./conform-marked.mjs');
  const report = { failures: [], steps: [] };
  // The drive's own `fail` (conform.mjs `target`): no field, no difference flag.
  const failFor = target => (step, what, field = null, difference = false) => { report.failures.push({ target, step, what, ...(field == null ? {} : { field }), ...(difference ? { difference: true } : {}) }); };
  referenceTapRefused({ target: 'recorder', step: 'tap record', why: 'wasm: tap record: disabled', crossBrowser: undefined, fail: failFor('recorder'), report });
  referenceTapRefused({ target: 'caltrain', step: 'tap open', why: 'wasm: tap open: hidden', crossBrowser: undefined, fail: failFor('caltrain'), report });
  referenceTapRefused({ target: 'recorder', step: 'tap stop', why: 'chrome: tap stop: disabled', crossBrowser: 'firefox', fail: failFor('recorder'), report });
  markUnbound(report);
  expect(report.marked).toEqual({});
  expect(report.failures).toEqual([{ target: 'recorder', step: 'tap record', what: 'wasm: tap record: disabled (the reference refused the tap, so the JS page never took it)' }]);
  expect(report.steps).toEqual([{ target: 'caltrain', step: 'tap open', skipped: 'wasm: tap open: hidden' }, { target: 'recorder', step: 'tap stop', skipped: 'chrome: tap stop: disabled' }]);
});

test('the marker marks every comparison kind on a marked target and leaves Linux differences failing', async () => {
  const { markUnbound } = await import('./conform-marked.mjs');
  const d = (step, what) => ({ target: 'markdown', step, what, difference: true });
  const report = { failures: [d('boot', 'layout editor.h: wasm 10.00 js 12.00 (delta 2.00 px, tolerance 0.5 px)'), d('boot', 'paint.0.1: wasm "auto" js "isolate"'),
    d('tap a', 'bound refusals: wasm «—» js «Trap(IterationLimit { pc: 3 })»'), d('tap a', 'sound lines: wasm «—» js «t=0 sound x»'),
    { target: 'markdown', step: 'tap a', what: 'linux state slots.x: wasm 1 linux 2' }, { target: 'markdown', step: 'tap a', what: 'layout editor.h: wasm 10.00 kernel 12.00' }] };
  markUnbound(report);
  expect(report.marked.markdown.differences).toBe(4);
  expect(report.marked.markdown.steps).toEqual(['boot', 'tap a']);
  expect(report.failures.map(f => f.what)).toEqual(['linux state slots.x: wasm 1 linux 2', 'layout editor.h: wasm 10.00 kernel 12.00']);
});
