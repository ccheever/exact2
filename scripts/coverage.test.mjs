// How a test that cannot run says so (scripts/coverage.mjs).
import { expect, test } from 'bun:test';
import { need, notApplicable, required } from './coverage.mjs';

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
