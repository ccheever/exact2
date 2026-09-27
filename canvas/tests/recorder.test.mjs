// The TypeScript recorder against the shared cases (LLP 1056 §4): the same
// getters and throws Chrome gave, as the Rust recorder is held to them.
import { test, expect } from 'bun:test';
import { runCases } from './cases.mjs';
import { Recorder, parseColor, serializeColor } from '../recorder.js';

test('the shared cases hold for the TypeScript recorder', () => {
  const failed = runCases(() => new Recorder()).filter((r) => r.fail.length);
  expect(failed).toEqual([]);
});

test('colours serialise as Chrome serialises them', () => {
  const c = (s) => { const v = parseColor(s); return v === 'current' ? 'current' : v ? serializeColor(v) : 'none'; };
  expect(c('rgba(0,0,0,0.123456)')).toBe('rgba(0, 0, 0, 0.12)');
  expect(c('rgb(1 2 3 / 0.999)')).toBe('#010203');
  expect(c('hsl(120 50 50)')).toBe('#40bf40');
  expect(c('hsl(120, 50, 50)')).toBe('none');
  expect(c('#abcd')).toBe('rgba(170, 187, 204, 0.867)');
});
