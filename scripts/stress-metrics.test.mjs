import { test } from 'node:test';
import assert from 'node:assert/strict';
import { summarize, stressOptions } from './stress-metrics.mjs';

test('empty samples do not invent latency or a refresh rate', () => {
  assert.deepEqual(summarize([], 8.333), {
    count: 0, p50_ms: null, p95_ms: null, p99_ms: null, max_ms: null, over_target: 0,
  });
});

test('nearest-rank percentiles retain tails and compare to the named target', () => {
  const samples = [2, 3, 4, 5, 50, NaN, Infinity, -1];
  assert.deepEqual(summarize(samples, 8.333), {
    count: 5, p50_ms: 4, p95_ms: 50, p99_ms: 50, max_ms: 50, over_target: 1,
  });
  assert.equal(samples[0], 2);
});

test('diagnostic refuses unbounded runs and requires an explicit local URL', () => {
  assert.throws(() => stressOptions(['--stress-url', 'https://example.com']), /loopback/);
  assert.throws(() => stressOptions(['--stress-url', 'http://localhost:8000', '--seconds', 'Infinity']), /seconds/);
  assert.throws(() => stressOptions(['--stress-url', 'http://localhost:8000', '--target-hz', '0']), /target-hz/);
  const options = stressOptions(['--stress-url', 'http://127.0.0.1:8000', '--seconds', '3', '--target-hz', '120']);
  assert.equal(options.seconds, 3);
  assert.equal(options.hz, 120);
});
