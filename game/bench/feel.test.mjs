import { test, expect } from 'bun:test';
import { analyze, quantile, script } from './feel.mjs';

// Independent 100 Hz presentation / 50 Hz motion fixture. Sampling is five ms
// after the frame timestamp, deliberately exposing the rAF/event clock pitfall.
function fixture(stepped = false) {
  const plan = script();
  const raw = { schema: 1, stride: 8, overflow: false,
    events: plan.schedule.flatMap(e => [1000 + e.at_ms, e.code, +e.down, e.trial]), frames: [] };
  for (let t = 0; t <= plan.duration_ms; t += 10) {
    const motionTime = stepped ? Math.floor(t / 20) * 20 : t;
    let z = Math.min(2500, Math.max(0, motionTime - 500)) * .004;
    for (const e of plan.schedule) if (e.trial >= 0 && t >= e.at_ms + 10) z += .01;
    raw.frames.push(t + 995, t + 1000, 0, .9, z, 0, 15, z + 19);
  }
  return { raw, plan };
}

test('the live script keeps all holds, release order, and twenty separated trials', () => {
  const p = script();
  expect(p.schedule.length).toBe(50);
  expect(p.schedule[3].at_ms - p.schedule[2].at_ms).toBe(2500);
  expect(p.schedule[5].at_ms - p.schedule[4].at_ms).toBe(1500);
  expect(p.schedule[9].at_ms - p.schedule[8].at_ms).toBe(1500);
  expect(p.schedule.filter(e => e.trial >= 0).length).toBe(20);
  expect(script(30).schedule[10].at_ms).toBe(30000);
  expect(script(7.02).schedule[10].at_ms).toBe(10020);
  expect(p.duration_ms).toBe(80900);
});

test('constant velocity has zero judder; alternating move/repeat approaches one', () => {
  const s = fixture(), q = fixture(true);
  const smooth = analyze(s.raw, s.plan), stepped = analyze(q.raw, q.plan);
  expect(smooth.refresh_interval_ms).toBe(10);
  expect(smooth.hitches).toBe(0);
  expect(smooth.player.judder).toBeLessThan(1e-12);
  expect(smooth.player.repeated_fraction).toBe(0);
  expect(stepped.player.judder).toBeCloseTo(1, 1);
  expect(stepped.player.repeated_fraction).toBeCloseTo(.5, 1);
  expect(stepped.camera.judder).toBeCloseTo(stepped.player.judder, 10);
});

test('latency uses the draw sample clock, not the older frame timestamp', () => {
  const { raw, plan } = fixture(), m = analyze(raw, plan);
  expect(m.latency.valid_trials).toBe(20);
  expect(m.latency.median_ms).toBe(10);
  expect(m.latency.p95_ms).toBe(10);
  expect(m.latency.median_intervals).toBe(1);
  expect(m.latency.trials.every(t => t.stationary_before)).toBe(true);
});

test('residual motion invalidates a trial instead of claiming a quick response', () => {
  const { raw, plan } = fixture();
  for (let i = 0; i < raw.frames.length; i += 8) if (raw.frames[i + 1] === 12950) raw.frames[i + 2] = .00001;
  const m = analyze(raw, plan);
  expect(m.latency.valid_trials).toBe(19);
  expect(m.latency.trials[0].latency_ms).toBeNull();
});

test('quantiles interpolate; corrupt, overflowed, or missing records fail explicitly', () => {
  expect(quantile([7, 1, 5, 3], .5)).toBe(4);
  expect(quantile([10, 0], .95)).toBe(9.5);
  const { raw, plan } = fixture();
  expect(() => analyze({ ...raw, overflow: true }, plan)).toThrow('buffer');
  expect(() => analyze({ ...raw, events: raw.events.slice(4) }, plan)).toThrow('delivered');
  raw.frames[8] = raw.frames[0];
  expect(() => analyze(raw, plan)).toThrow('Nonmonotonic');
});
