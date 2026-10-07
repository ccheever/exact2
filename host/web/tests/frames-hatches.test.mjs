// The web's frame sampler (host/web/frames.js) and the hatches (LLP
// 1075.003.000.001 §3.1): a late frame's record names the hatch calls that
// overlapped its interval, as `exact.hatchPerf.window` answers; an on-time
// frame's record names none. The page is a small fake.
import { test, expect } from 'bun:test';

test('a late frame names the hatches that ran in it', async () => {
  let frame = null;
  const asked = [];
  globalThis.requestAnimationFrame = f => { frame = f; return 1; };
  globalThis.cancelAnimationFrame = () => { frame = null; };
  globalThis.addEventListener = () => {};
  globalThis.document = { visibilityState: 'visible', addEventListener() {}, getAnimations: () => [] };
  globalThis.PerformanceObserver = class { observe() {} };
  globalThis.exact = { hatchPerf: { window(from, to) { asked.push([from, to]); return to - from > 30 ? { hatches: [{ hatch: 'element avatar', calls: 12, ms: 9.8 }], coverage: 'partial' } : null; } } };
  const { createFrameSampler } = await import(new URL('../frames.js', import.meta.url).pathname);
  const sampler = createFrameSampler({ origin: () => 0, log() {}, gather: async () => ({}), covers: [], target: 'js' });
  sampler.batch(1, 0);                       // activity: a segment opens
  // Nine frames at 10 ms set the floor; then one 40 ms after the last.
  let ts = performance.now();
  const step = (ms) => { ts += ms; const f = frame; frame = null; f(ts); };
  for (let i = 0; i < 10; i++) step(10);
  step(40);
  const reply = sampler.reply(20, true);
  expect(reply.late.length).toBe(1);
  expect(reply.late[0]).toMatchObject({ missed: 3, hatches: [{ hatch: 'element avatar', calls: 12, ms: 9.8 }], coverage: 'partial' });
  // The window asked is the sample's own interval, ending at its callback.
  const [from, to] = asked.at(-1);
  expect(Math.round(to - from)).toBe(40);
  // An on-time frame carries neither the join nor what it was read into.
  for (const r of reply.records.filter(r => !r.missed)) { expect(r.hatches).toBeUndefined(); expect(r.ran).toBeUndefined(); }
});
