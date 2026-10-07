// The JS target's hatches (host/web-js/hatches.js; LLP 1075.003.000.001
// §3.1–3.2, §4.4): what a hatch's code records is bounded and refused past
// its bounds, a call's time is its own, a throw stops one node's hatch and
// leaves its end, and a production page keeps nothing. The DOM is a small
// fake; the runtime and the page module are stand-ins.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-hatches-'));
copyFileSync(resolve(new URL('../../web-js/hatches.js', import.meta.url).pathname), resolve(dir, 'hatches.js'));
writeFileSync(resolve(dir, 'rt.js'), 'export const { onEnd, journal, clock, viewId, inflight, painted } = globalThis.rtStandIn;\n');
writeFileSync(resolve(dir, 'native.js'), 'export const pageTable = () => globalThis.pageModule;\n');

const ends = [], journal = [], clock = { now: 0, epoch: 1 };
globalThis.rtStandIn = { onEnd: f => ends.push(f), journal, clock, viewId: e => e.id, inflight: { n: 0 }, painted: () => Promise.resolve() };
globalThis.requestAnimationFrame = f => setTimeout(f);
// A development page: its plan's digest is there (web-js/build.mjs).
globalThis.exact = { plan: 'digest' };
const turn = () => new Promise(done => setTimeout(done, 2));

let ids = 0;
const node = (word, site) => ({
  id: ++ids, dataset: { hatch: word, ...(site != null ? { site: String(site) } : {}) },
  getAttribute: k => (k === 'data-hatch' ? word : null), closest: () => null,
});
/** Mount a hatched node; returns its end. */
const mount = async (hatches, e) => { const before = ends.length; hatches.ht(e); await turn(); return ends[before]; };

const handles = [], ended = [];
let thrown = 0;
globalThis.pageModule = {
  element(e) {
    handles.push(e);
    if (e.hatch === 'thrower') { thrown++; throw new Error('boom'); }
    if (e.hatch === 'outer' && e.isNew) { const t = performance.now(); while (performance.now() - t < 4); }
  },
  elementEnded(e) { ended.push(e.hatch); },
};

test('diagnostics are bounded, refused past their bounds, and scoped', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  const end = await mount(hatches, node('meter', 7));
  const d = handles.at(-1).diagnostics, state = () => globalThis.exact.hatchState();

  // A counter adds; a bad name or amount is refused and counted, and the
  // first bad names are journaled.
  d.count('swipes'); d.count('swipes', 4); d.count('Bad Name'); d.count('swipes', -1);
  expect(state().words.meter.counters).toEqual({ swipes: 5 });
  expect(state().rejected).toBe(2);
  expect(journal.some(l => /hatch element meter: diagnostics refused the name "Bad Name"/.test(l))).toBe(true);

  // 64 counters a page module; the 65th name is refused.
  for (let i = 0; i < 70; i++) globalThis.exact.diagnostics.count(`c${i}`);
  expect(Object.keys(state().scopes.module.counters).length).toBe(63);
  expect(state().rejected).toBe(2 + 7);

  // A timing keeps a cumulative count, sum and max, and its last 256 samples.
  for (let i = 1; i <= 300; i++) d.measure('frame', i);
  const perf = globalThis.exact.hatchPerf.reply({ incarnation: 1 });
  expect(perf.timings['element meter'].frame).toMatchObject({ count: 300, sum: 45150, max: 300, samples: 256, dropped: 44, measured: true });
  expect(perf.timings['element meter'].frame.p50).toBeGreaterThan(44);

  // A span is on the session clock; 32 may be open; one open at its node's end is abandoned.
  const span = d.begin('swipe');
  clock.now = 250;
  span.end(); span.end();
  expect(globalThis.exact.hatchPerf.reply({}).timings['element meter'].swipe).toMatchObject({ count: 1, sum: 250, max: 250 });
  const open = Array.from({ length: 33 }, () => d.begin('held'));
  expect(state().rejected).toBe(2 + 7 + 1);
  open[0].end();

  // A snapshot is the latest, whole or refused: 4 KB, and JSON.
  d.publish('last', { worst: 3 }); d.publish('last', { worst: 9 });
  d.publish('big', 'x'.repeat(5000));
  const loop = {}; loop.self = loop;
  d.publish('loop', loop);
  expect(state().words.meter.published).toEqual({ last: { worst: 9 } });
  expect(state().rejected).toBe(2 + 7 + 1 + 2);

  // 20 lines a second of session clock a scope, then one line for the rest;
  // a line is cut at 256 bytes.
  const from = journal.length;
  for (let i = 0; i < 25; i++) d.log(`line ${i}`);
  clock.now = 1300;
  d.log('é'.repeat(300));
  const said = journal.slice(from);
  expect(said.length).toBe(22);
  expect(said[20]).toBe('t=1300 hatch element meter: … 5 more');
  expect(new TextEncoder().encode(said[21].slice('t=1300 hatch element meter: '.length)).length).toBeLessThanOrEqual(256);
  expect(said[21].endsWith('…')).toBe(true);
  expect(state().limited).toBe(5);

  // The node ends: its hatch hears it, and its open spans are abandoned.
  end();
  expect(state().abandoned).toBe(31);
  expect(ended).toEqual(['meter']);
  // A read changes nothing.
  expect(JSON.stringify(state())).toBe(JSON.stringify(state()));
});

test('a call is timed by site and moment, and a nested call is charged to itself', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  await mount(hatches, node('outer', 12));
  const perf = globalThis.exact.hatchPerf.reply({});
  const built = perf.calls.find(c => c.hatch === 'element outer');
  expect(built).toMatchObject({ site: 12, moment: 'built', calls: 1 });
  expect(built.ms).toBeGreaterThanOrEqual(3);
  expect(perf.hatches['element outer']).toMatchObject({ calls: 1 });
  expect(globalThis.exact.hatchPerf.site(12)).toMatchObject({ hatch: { calls: 1 } });
  expect(globalThis.exact.hatchPerf.site(99)).toBe(null);
});

test("a throw stops that node's hatch, and its end is still called", async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  const a = node('thrower', 20), b = node('thrower', 20);
  const endA = await mount(hatches, a);
  expect(thrown).toBe(1);
  expect(journal.some(l => /element thrower #\d+: element threw boom; this node's hatch is not called again, its end still is/.test(l))).toBe(true);
  // Its words change: the hatch that threw is not called again.
  a.$ht(); await turn();
  expect(thrown).toBe(1);
  // Another node with the same word still runs.
  await mount(hatches, b);
  expect(thrown).toBe(2);
  endA();
  expect(ended).toEqual(['meter', 'thrower']);
});
