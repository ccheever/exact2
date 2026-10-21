// How a test that cannot run says so (scripts/coverage.mjs), and the async
// lane's side of it: required coverage, this commit's fixtures, no test run.
import { expect, test } from 'bun:test';
import { need, notApplicable, required } from './coverage.mjs';
import { checks, failures, laneEnv } from './async.mjs';

/** A stand-in for bun's `test` that records what was registered. */
function recorder() {
  const seen = [];
  const t = (name, fn) => seen.push({ name, fn, how: 'run' });
  t.skip = (name, fn) => seen.push({ name, fn, how: 'skip' });
  t.serial = t;
  return { t, seen };
}

test('nothing missing is the test itself', () => {
  const { t } = recorder();
  expect(need('chrome', null, { test: t, env: {} })).toBe(t);
  expect(need('chrome', null, { test: t, env: { EXACT_REQUIRED: '1' } })).toBe(t);
  expect(notApplicable(null, { test: t })).toBe(t);
});

test('a missing prerequisite skips, naming its kind and why', () => {
  const { t, seen } = recorder();
  need('chrome', 'Chromium is missing', { test: t, env: {} })('presses');
  expect(seen).toEqual([{ name: 'presses — missing chrome: Chromium is missing', fn: undefined, how: 'skip' }]);
});

test('under EXACT_REQUIRED a missing prerequisite fails rather than skips', () => {
  const { t, seen } = recorder();
  const check = need('fixture', 'no wasm dist', { test: t, env: { EXACT_REQUIRED: '1' } });
  check('adopts the document');
  check.serial('serially too');
  expect(seen.map(s => [s.name, s.how])).toEqual([
    ['adopts the document — missing fixture: no wasm dist', 'run'],
    ['serially too — missing fixture: no wasm dist', 'run'],
  ]);
  expect(() => seen[0].fn()).toThrow('missing fixture: no wasm dist (EXACT_REQUIRED=1)');
});

test('not applicable here is a skip even when coverage is required', () => {
  const { t, seen } = recorder();
  notApplicable('macOS only', { test: t })('placement');
  expect(seen).toEqual([{ name: 'placement — n/a: macOS only', fn: undefined, how: 'skip' }]);
  expect(required({ EXACT_REQUIRED: '1' })).toBe(true);
  expect(required({})).toBe(false);
});

test('the lane requires coverage and gives its web tests this commit\'s builds', () => {
  const steps = Object.fromEntries(checks('new', 'old', { lane: [], apple: false, glue: ['./x.test.mjs'], contract: false }).map(s => [s[0], s]));
  const names = Object.keys(steps);
  expect(names.indexOf('prepare')).toBeLessThan(names.indexOf('test'));
  expect(names.indexOf('prepare')).toBeLessThan(names.indexOf('glue'));
  for (const name of ['test', 'glue', 'web-build-test', 'web-ax-test']) expect(steps[name][2]).toContain('EXACT_REQUIRED=1');
  const glue = steps.glue[2].join(' ');
  for (const v of ['EXACT_WEB_DIST=', 'EXACT_WEB_JS_DIST=', 'EXACT_NO_GPU_WASM_DIST=', 'EXACT_NO_GPU_JS_DIST=', 'EXACT_MOTION_DIST=']) expect(glue).toContain(v);
  expect(steps.prepare[2][1]).toContain('EXACT_WEB_LINK=all');
});

test('the lane drops settings that change what a check tests', () => {
  const env = laneEnv({ PATH: '/p', EXACT_JS_ENGINE: 'stub', EXACT_AGENT: '1', EXACT_AGENT_FAIL_FETCH: '1', EXACT_WEB_DIST: '/old', EXACT_REQUIRED: '0', CARGO_TARGET_DIR: '/t' });
  expect(env).toEqual({ PATH: '/p', HERMES_LEAN_SYS_OFFLINE: '1' });
});

test('a bun step that ran no test is a failure', () => {
  expect(failures('web-build-test', 'bun test v1.4.2\nerror: regex "x" matched 0 tests. Searched 1 file\n', 0)).toEqual(['web-build-test: ran no tests']);
  expect(failures('glue', 'bun test v1.4.2\n 3 pass\n 0 fail\nRan 3 tests across 1 file.\n', 0)).toEqual([]);
});
