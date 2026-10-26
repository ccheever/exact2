// LLP 1144 D1: the fling report's arithmetic, without a simulator.
import { test, expect } from 'bun:test';
import { parseArgs, framesBetween, report } from './fling.mjs';

test('the frames between two reads are the window\'s, with the late frames\' phases summed', () => {
  const before = { window: { to: 10 }, lifetime: { presented: 100, late: 2, missed: 1, overruns: 1 } };
  const after = {
    period: { ms: 16.67 }, lifetime: { presented: 220, late: 5, missed: 3, overruns: 2 },
    records: [{ t: 9, interval: 50 }, { t: 11, interval: 16 }, { t: 12, interval: 17 }, { t: 13, interval: 33 }],
    late: [{ t: 9, phases: [{ phase: 'rows', ms: 40 }] }, { t: 13, phases: [{ phase: 'runtime', ms: 12 }, { phase: 'image', ms: 3 }], unattributed: 4 }],
  };
  const f = framesBetween(before, after);
  expect([f.presented, f.late, f.missed, f.overruns, f.samples, f.max]).toEqual([120, 3, 2, 1, 3, 33]);
  expect(f.phases).toEqual([{ phase: 'runtime', ms: 12 }, { phase: 'image', ms: 3 }]);
  const text = report({ frames: f, work: { created: 40, sites: [], handlers: [{ at: 'app.contract:575 (App)', event: 'scroll', dispatched: 180, committed: 180 }] },
    images: { late: 2, blankMs: { p50: 120, p95: 300 }, leadMs: { p50: 90 }, waitingOn: { download: 2 } }, flings: 6, distance: 600 });
  expect(text).toContain('120 frames presented · 3 late (2 missed, 1 overruns)');
  expect(text).toContain('late frames spent: runtime 12 · image 3 · unattributed 4 ms');
  expect(text).toContain('scroll at app.contract:575 (App): 180 dispatched, 180 changed the tree');
  expect(text).toContain('images shown blank since launch: 2 · blank p50 120 ms p95 300 ms · lead p50 90 ms · waiting on download 2');
});

test('arguments: a target is required, and --to ops keep their order', () => {
  expect(() => parseArgs(['ios', 'excord'])).toThrow(/--target/);
  const a = parseArgs(['ios', 'excord', '--target', 'transcript', '--to', 'tap channel-general', '--to', 'wait 500', '--flings', '4']);
  expect([a.target, a.to, a.flings]).toEqual(['transcript', ['tap channel-general', 'wait 500'], 4]);
});

test('motion: a step far past its neighbours while gliding is a jump; standing still mid-glide is a stall; a bounce is neither', async () => {
  const { motionOf } = await import('./fling.mjs');
  const glide = [0, 40, 78, 114, 148, 300, 330, 330, 358, 384].map((y, i) => ({ t: i, scroll: [y, 'd'] }));
  const m = motionOf([{ t: -1, scroll: [0, 't'] }, ...glide, { t: 20, scroll: [500, 'b'] }]);
  expect(m.jumps.map((j) => j.t)).toEqual([5]);
  expect(m.stalls).toEqual([7]);
  expect(m.gliding).toBe(10);
});

test('motion: a late frame\'s step covers the frames it missed', async () => {
  const { motionOf } = await import('./fling.mjs');
  const glide = [0, 30, 60, 90, 180, 210, 240].map((y, i) => ({ t: i, missed: i === 4 ? 2 : 0, scroll: [y, 'd'] }));
  expect(motionOf(glide).jumps).toEqual([]);
});
