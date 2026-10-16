import { test, expect } from 'bun:test';
import { conformArgs } from './conform-args.mjs';
import { parseStep } from './conform-steps.mjs';
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
