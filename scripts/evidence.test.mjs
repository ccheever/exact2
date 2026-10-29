// What has been run against an app's current sources (LLP 1146 D2).
import { expect, test } from 'bun:test';
import { hostsTouched, standing } from './evidence.mjs';

test('a run is current while its build is the host\'s and fresh', () => {
  const runs = [
    { host: 'web', build: 'b1', what: 'test a' },
    { host: 'macos', build: 'm1', what: 'test a' },
    { host: 'ios', build: 'i0', what: 'test a' },
    { host: 'linux', build: null, what: 'drive tree' },
  ];
  const now = { web: { id: 'b1', changed: [] }, macos: { id: 'm1', changed: ['apps/x/app.contract'] }, ios: { id: 'i1', changed: [] } };
  expect(standing(null, runs, (h) => now[h] ?? null).map((r) => [r.host, r.standing, r.why])).toEqual([
    ['web', 'current', undefined],
    ['macos', 'stale', 'apps/x/app.contract changed after'],
    ['ios', 'stale', 'a later build replaced the one it ran on'],
    ['linux', 'unchecked', 'a linux build is not identified yet'],
  ]);
});

test('a change touches the hosts whose code it is', () => {
  expect(hostsTouched(['host/apple/Sources/ExactKit/Mac/PresenterMac.swift'])).toEqual(['macos', 'ios', 'tvos']);
  expect(hostsTouched(['host/web-js/rt.js', 'host/windows/src/main.rs'])).toEqual(['web', 'windows']);
  expect(hostsTouched(['runner/src/perf.rs'])).toHaveLength(7);
  expect(hostsTouched(['docs/testing.md', 'llp/1146-x.rfc.md'])).toEqual([]);
});
