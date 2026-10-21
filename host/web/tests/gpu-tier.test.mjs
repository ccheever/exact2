import { expect, test } from 'bun:test';
import { classify } from '../gpu-tier.js';
import { pageReporter } from '../navigation.js';

// LLP 1139.003 D2: the web's table is gpu/src/tier.rs `classify`'s, case for case.
test('the web classifies adapters as the native table does', () => {
  for (const [vendor, architecture, software, handheld, tier] of [
    ['qualcomm', 'adreno-6xx', false, true, 'low'], ['qualcomm', 'adreno-7xx', false, true, 'mid'],
    ['qualcomm', 'adreno-10xx', false, true, 'mid'], ['qualcomm', 'Adreno (TM) 620', false, true, 'low'],
    ['qualcomm', 'Adreno (TM) 740', false, true, 'mid'], ['arm', 'valhall', false, true, 'mid'],
    ['arm', 'bifrost', false, true, 'low'], ['arm', 'Mali-G76', false, true, 'low'], ['arm', 'Mali-T880', false, true, 'low'],
    ['arm', 'Mali-G78', false, true, 'mid'], ['apple', 'metal-3', false, false, 'high'], ['apple', 'metal-3', false, true, 'mid'],
    ['apple', 'Apple A13 GPU', false, true, 'low'], ['apple', 'Apple A17 Pro GPU', false, true, 'mid'],
    ['nvidia', 'ampere', false, false, 'high'], ['google', 'swiftshader', true, false, 'low'],
    ['', '', false, true, 'low'], ['', '', false, false, 'high'],
  ]) expect([vendor, architecture, classify(vendor, architecture, software, handheld)]).toEqual([vendor, architecture, tier]);
});

// The page facts word carries the tier in bits 11–12, beside `persisted` (bit 10): 0 high, 1 mid, 2 low (runner/src/page.rs).
test('the page facts word carries the tier in bits 10 and 11', () => {
  const agent = pageReporter(true, { document: { documentElement: { style: {} } }, navigator: {}, devicePixelRatio: 1 });
  const tier = () => (agent.bits() >> 11) & 3;
  expect(tier()).toBe(0);
  agent.prefer({ 'gpu-tier': 'mid' });
  expect(tier()).toBe(1);
  const mid = agent.bits();
  agent.prefer({ 'gpu-tier': 'low' });
  expect(tier()).toBe(2);
  expect(agent.bits() - mid).toBe(1 << 11, 'no other fact moves');
  const page = pageReporter(false, { document: { visibilityState: 'visible', hasFocus: () => true, documentElement: {} }, navigator: { onLine: true }, screen: { width: 392, height: 851 }, performance: {}, getComputedStyle: () => ({ fontSize: '16px' }), devicePixelRatio: 1 });
  expect(page.read()['gpu-tier']).toBe('low');
  page.tell('mid');
  expect((page.bits() >> 11) & 3).toBe(1);
});
