import { describe, expect, test } from 'bun:test';
import { pacer } from '../pace.js';

const P120 = 1000 / 120;
// A deterministic jitter: callbacks land late by 0–2 ms, never early, like Chrome's.
function lcg(seed) { let s = seed >>> 0; return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32); }
function callbacks(count, period, { jitter = 2, seed = 7, start = 1000, drop = [] } = {}) {
  const rand = lcg(seed), times = [];
  for (let k = 0; k < count; k++) if (!drop.includes(k)) times.push(start + k * period + rand() * jitter);
  return times;
}
const stats = values => {
  const mean = values.reduce((s, v) => s + v, 0) / values.length;
  const sd = Math.sqrt(values.reduce((s, v) => s + (v - mean) ** 2, 0) / values.length);
  return { mean, cv: sd / mean };
};
const deltas = values => values.slice(1).map((v, i) => v - values[i]);

describe('the paced frame clock', () => {
  test('exports zero until fitted, then a stable period through jitter and stall recovery', () => {
    const pace = pacer(), raw = callbacks(900, P120);
    expect(pace.period_ms).toBe(0);
    raw.slice(0, 300).forEach(pace);
    const period = pace.period_ms;
    expect(Math.abs(period - P120)).toBeLessThan(0.02);
    for (let i = 300; i < raw.length; i++) {
      pace(raw[i] + (i >= 400 ? 40 : 0));
      expect(pace.period_ms).toBe(period);
    }
  });
  test('publishes a new fitted period after a display-rate change', () => {
    const pace = pacer(), first = callbacks(400, P120);
    first.forEach(pace);
    const period = pace.period_ms;
    const next = callbacks(800, 1000 / 90, { start: first.at(-1) + 1000 / 90, seed: 11 });
    next.forEach(pace);
    expect(pace.period_ms).not.toBe(period);
    expect(Math.abs(pace.period_ms - 1000 / 90)).toBeLessThan(0.02);
  });
  test('snaps jittered callbacks to the display lattice: per-frame deltas become uniform', () => {
    const pace = pacer(), raw = callbacks(1200, P120), paced = raw.map(pace);
    const before = stats(deltas(raw.slice(300))), after = stats(deltas(paced.slice(300)));
    expect(before.cv).toBeGreaterThan(0.08);
    expect(after.cv).toBeLessThan(0.005);
    expect(Math.abs(after.mean - P120)).toBeLessThan(0.02);
  });
  test('stays within one period of the callback clock and never runs backwards', () => {
    const pace = pacer(), raw = callbacks(2000, P120, { jitter: 2.5, seed: 3 });
    let previous = -Infinity;
    for (const now of raw) {
      const paced = pace(now);
      expect(Math.abs(paced - now)).toBeLessThan(P120);
      expect(paced).toBeGreaterThanOrEqual(previous);
      previous = paced;
    }
  });
  test('a clean lattice passes through unchanged', () => {
    const pace = pacer(), raw = callbacks(400, P120, { jitter: 0 });
    raw.slice(10).forEach((now, i) => expect(Math.abs(pace(now) - now)).toBeLessThan(1e-9 * (i + 1)));
  });
  test('a dropped frame advances the paced clock by two periods', () => {
    const pace = pacer(), raw = callbacks(600, P120, { drop: [400] }), paced = raw.map(pace);
    const d = deltas(paced);
    const gap = d[399]; // between callback 399 and 401 (index 400 is the one after the drop)
    expect(Math.abs(gap - 2 * P120)).toBeLessThan(0.1);
    expect(stats(d.slice(300, 398)).cv).toBeLessThan(0.005);
    expect(stats(d.slice(400, 500)).cv).toBeLessThan(0.005);
  });
  test('a stall advances the clock by whole slots and pacing continues on the shifted lattice', () => {
    const pace = pacer(), raw = callbacks(500, P120);
    for (let i = 300; i < raw.length; i++) raw[i] += 40;
    const paced = raw.map(pace), d = deltas(paced);
    expect(Math.abs(d[299] - Math.round((40 + P120) / P120) * P120)).toBeLessThan(0.2);
    expect(stats(d.slice(300, 499)).cv).toBeLessThan(0.005);
    for (let i = 300; i < raw.length; i++) expect(Math.abs(paced[i] - raw[i])).toBeLessThan(P120);
  });
  test('a late callback snaps to its slot: the displacement it draws is one period, not its lateness', () => {
    const pace = pacer(), raw = callbacks(500, P120, { jitter: 0.5 });
    raw[400] += 2.5; // 10.8 ms after the previous callback, as the loaded machine produced
    const paced = raw.map(pace), d = deltas(paced);
    expect(Math.abs(d[399] - P120)).toBeLessThan(0.1);
    expect(Math.abs(d[400] - P120)).toBeLessThan(0.1);
  });
  test('a refresh-rate change is followed: never worse than the raw clock, settled within seconds', () => {
    const pace = pacer(), P90 = 1000 / 90;
    const raw = callbacks(400, P120), tail = callbacks(800, P90, { start: raw.at(-1) + P90, seed: 11 });
    const all = [...raw, ...tail], paced = all.map(pace);
    const adapting = { raw: stats(deltas(all.slice(400, 640))), paced: stats(deltas(paced.slice(400, 640))) };
    expect(adapting.paced.cv).toBeLessThanOrEqual(adapting.raw.cv);
    expect(Math.abs(adapting.paced.mean - P90)).toBeLessThan(0.05);
    const settled = stats(deltas(paced.slice(640)));
    expect(Math.abs(settled.mean - P90)).toBeLessThan(0.02);
    expect(settled.cv).toBeLessThan(0.005);
  });
  test('never runs backwards even when a callback lands before the last slot', () => {
    const pace = pacer(), raw = callbacks(300, P120, { jitter: 0.5 });
    raw.splice(200, 0, raw[199] + 0.3); // a second callback 0.3 ms after the previous one
    let previous = -Infinity;
    for (const now of raw) { const at = pace(now); expect(at).toBeGreaterThanOrEqual(previous); previous = at; }
  });
  test('under the agent clock the host never paces (the pacer is only for live frames)', () => {
    // Documented at the call site: exact.now bypasses pace(). Here: a pacer given
    // agent-clock-like steps (exact multiples of 1000 ms) leaves them exact.
    const pace = pacer(), steps = Array.from({ length: 20 }, (_, i) => i * 1000);
    steps.forEach(now => expect(pace(now)).toBeCloseTo(now, 6));
  });
});
