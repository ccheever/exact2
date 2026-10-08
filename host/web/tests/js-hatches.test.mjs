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
writeFileSync(resolve(dir, 'rt.js'), 'export const { onEnd, journal, clock, viewId, inflight, painted, time, paint, drive } = globalThis.rtStandIn;\n');
writeFileSync(resolve(dir, 'native.js'), 'export const pageTable = () => globalThis.pageModule;\n');

const ends = [], journal = [], clock = { now: 0, epoch: 1, timers: [], agent: true };
const rt = globalThis.rtStandIn = { onEnd: f => ends.push(f), journal, clock, viewId: e => e.id, inflight: { n: 0 }, painted: () => Promise.resolve(), time() {}, paint() {}, drive() {} };
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
    e.element.onHatch?.(e);
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
  // A late frame's join: the calls that overlapped a window, each charged its overlap.
  const now = performance.now(), ran = globalThis.exact.hatchPerf.window(now - 1000, now);
  expect(ran.hatches.find(h => h.hatch === 'element outer')).toMatchObject({ calls: 1 });
  expect(ran.hatches.find(h => h.hatch === 'element outer').ms).toBeGreaterThanOrEqual(3);
  expect(ran.coverage).toBeUndefined();
  expect(globalThis.exact.hatchPerf.window(now + 1000, now + 2000)).toBe(null);
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

test('a word this platform does not handle is shown and never called', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  globalThis.exact.hatchWords = ['meter'];
  try {
    const before = handles.length;
    await mount(hatches, node('elsewhere', 30));
    await mount(hatches, node('elsewhere', 30));
    expect(handles.length).toBe(before);
    expect(journal.filter(l => /element elsewhere: not handled on this platform/.test(l)).length).toBe(1);
    const state = globalThis.exact.hatchState();
    expect(state.platform).toEqual(['meter']);
    expect(state.unhandled).toEqual([{ word: 'elsewhere', reason: 'plan' }]);
    await mount(hatches, node('meter', 31));
    expect(handles.length).toBe(before + 1);
  } finally { delete globalThis.exact.hatchWords; }
});

// A fake text field: the DOM's value, `setRangeText` and events, enough for `input`.
globalThis.InputEvent ??= class { constructor(type, init) { this.type = type; Object.assign(this, init); } };
const field = (word, props = {}) => Object.assign(node(word), {
  localName: 'input', type: 'text', value: '', maxLength: -1, disabled: false, readOnly: false, events: [], clicks: 0,
  setRangeText(text, from, to) { if (this.type === 'email' || this.type === 'number') throw new Error('InvalidStateError'); this.value = this.value.slice(0, from) + text + this.value.slice(to); },
  dispatchEvent(ev) { this.events.push(`${ev.type}:${ev.inputType}:${this.value}`); return true; },
  click() { this.clicks++; }, focus() {}, blur() {},
}, props);
const settled = async () => { for (let i = 0; i < 20 && globalThis.exact.hatchActs.queued(); i++) await turn(); await turn(); };

test('input replaces an authored field\'s whole value and is journaled by length', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  const from = journal.length, refused = () => globalThis.exact.hatchState().refused, before = refused();
  const search = field('search', { value: 'old', onHatch: e => { e.input('Palo Alto'); expect(search.value).toBe('old'); } });
  await mount(hatches, search); await settled();
  expect(search.value).toBe('Palo Alto');
  expect(search.events).toEqual(['input:insertReplacementText:Palo Alto']);
  expect(journal.slice(from).some(l => /element search #\d+: input \(9 chars, delivery: hatch\)/.test(l))).toBe(true);
  expect(journal.slice(from).join('\n')).not.toContain('Palo');
  // The field's own limit; a type without a selection API; a password's length is not said.
  const short = field('short', { maxLength: 4, onHatch: e => e.input('abcdefgh') });
  const mail = field('mail', { type: 'email', onHatch: e => e.input('a@b.c') });
  const secret = field('secret', { type: 'password', onHatch: e => e.input('hunter2') });
  for (const f of [short, mail, secret]) await mount(hatches, f);
  await settled();
  expect([short.value, mail.value, secret.value]).toEqual(['abcd', 'a@b.c', 'hunter2']);
  expect(journal.slice(from).some(l => /element secret #\d+: input \(protected, delivery: hatch\)/.test(l))).toBe(true);
  // Refused by name, and counted: not a field, a contenteditable editor, disabled, readonly, over 64 KB.
  const cases = [
    [field('box', { localName: 'div' }), 'x', /not an editable text field/], [field('editor', { localName: 'div', isContentEditable: true }), 'x', /a contenteditable editor/],
    [field('off', { disabled: true }), 'x', /the field is disabled/], [field('fixed', { readOnly: true }), 'x', /the field is readonly/],
    [field('huge'), 'x'.repeat(70000), /70000 bytes is over 64 KB/],
  ];
  for (const [f, text] of cases) { f.onHatch = e => e.input(text); await mount(hatches, f); }
  await settled();
  for (const [f, , why] of cases) { expect(f.value).toBe(''); expect(journal.slice(from).some(l => why.test(l))).toBe(true); }
  expect(refused() - before).toBe(5);
});

test('acts are queued, bounded, drained 64 at a time, and never run inside the hatch', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  const drains = globalThis.exact.hatchActs.drains(), refused = globalThis.exact.hatchState().refused;
  let live = null;
  const button = field('button', { onHatch: e => { live = e; for (let i = 0; i < 300; i++) e.click(); expect(button.clicks).toBe(0); } });
  const end = await mount(hatches, button);
  // 256 are queued and the rest refused; a drain runs 64, so four drains run them.
  expect(globalThis.exact.hatchState().refused - refused).toBe(300 - 256);
  await settled();
  expect(button.clicks).toBe(256);
  expect(globalThis.exact.hatchActs.drains() - drains).toBe(4);
  expect(rt.inflight.n).toBe(0);
  // After its end a handle's act does nothing.
  end();
  live.click(); await settled();
  expect(button.clicks).toBe(256);
});

// The runtime's seek, as rt.js `advance` treats a hatch's entries: the
// earliest due first, a hatch's instant after any other timer due at the same
// time, its next time its own to set. A refusal ends the seek with its line.
const seek = (to) => {
  for (;;) {
    let next = null;
    for (const t of clock.timers) if (t.due <= to && (!next || t.due < next.due || t.due === next.due && next.hatch && !t.hatch)) next = t;
    if (!next) break;
    clock.now = next.due;
    if (!next.hatch) next.due = next.base + ++next.k * 1000 / 60; // the product first, as rt.js `vf`
    if (next.action() !== true) return journal.at(-1);
  }
  clock.now = Math.max(clock.now, to);
  return null;
};

test('a frame ticket ticks at the virtual display\'s instants, after the frame\'s tasks, and a seek is its steps', async () => {
  await import(resolve(dir, 'hatches.js'));
  const { frames, after } = globalThis.exact.hatches;
  const run = (steps) => {
    clock.now = 1000; clock.timers.length = 0; globalThis.exact.hatchActs.command();
    // A plan's frame task at the same instants: it counts frames.
    let count = 0;
    clock.timers.push({ base: 1000, k: 1, due: 1000 + 1000 / 60, action: () => { count++; return true; } });
    const log = [];
    let n = 0, stopped = null;
    const stop = frames((frame) => {
      n++;
      log.push(`tick ${n} sees ${count} at ${frame.now.toFixed(3)}`);
      if (n === 1) {
        after(0, () => { log.push(`after0 at ${clock.now.toFixed(3)} tick ${n}`); after(20, () => log.push(`after20 at ${clock.now.toFixed(3)}`)); });
        stopped = after(30, () => log.push('stopped ran'));
        stopped();
      }
    });
    for (const to of steps) expect(seek(to)).toBe(null);
    stop();
    expect(clock.timers.some(t => t.hatch)).toBe(false);
    return { log, n, count };
  };
  const whole = run([2000]), stepped = run(Array.from({ length: 10 }, (_, i) => 1100 + i * 100));
  expect(whole.n).toBe(60);
  // Tick k sees the count the frame task made at the same instant.
  for (let k = 1; k <= 60; k++) expect(whole.log.some(l => l.startsWith(`tick ${k} sees ${k} at`))).toBe(true);
  // after(0) in the first tick runs at that instant, before the second tick; its after(20) later in the seek; the stopped one never.
  expect(whole.log.indexOf(whole.log.find(l => l.startsWith('after0')))).toBe(1);
  expect(whole.log.some(l => l === `after20 at ${(1000 + 1000 / 60 + 20).toFixed(3)}`)).toBe(true);
  expect(whole.log).not.toContain('stopped ran');
  expect(stepped.log).toEqual(whole.log);
});

test('what a tick asks is drained at its instant, with the moments it causes, under the command\'s caps', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  const { frames } = globalThis.exact.hatches;
  clock.now = 5000; clock.timers.length = 0; globalThis.exact.hatchActs.command();
  // A button whose 65th click changes its words: its `changed` clicks once more.
  let handle = null;
  const button = field('clocked', { onHatch: e => { handle = e; if (!e.isNew && button.clicks === 65) e.click(); } });
  button.click = function () { this.clicks++; if (this.clicks === 65) this.$ht(); };
  await mount(hatches, button);
  let ticks = 0;
  const stop = frames(() => { if (++ticks === 3) for (let i = 0; i < 65; i++) handle.click(); });
  expect(seek(5000 + 3 * 1000 / 60)).toBe(null);
  // All 66 ran inside the seek, at the third tick's instant: none waited for a microtask.
  expect(button.clicks).toBe(66);
  expect(globalThis.exact.hatchActs.queued()).toBe(0);
  // 4,096 ticks are the most one command runs; the next names the limit.
  globalThis.exact.hatchActs.command();
  ticks = 100;
  expect(seek(clock.now + 4096 * 1000 / 60)).toBe(null);
  expect(seek(clock.now + 2 * 1000 / 60)).toMatch(/refused advance: HatchFireLimit/);
  stop();
  await settled();
});

test('an act that asks for another, forever, stops the command at HatchActLimit', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  const { frames } = globalThis.exact.hatches;
  clock.now = 20000; clock.timers.length = 0; globalThis.exact.hatchActs.command();
  let handle = null, asked = false;
  const button = field('looper', { onHatch: e => { handle = e; } });
  button.click = function () { this.clicks++; handle.click(); };
  await mount(hatches, button);
  const stop = frames(() => { if (!asked) { asked = true; handle.click(); } });
  // One tick's click asks for the next at the same instant: 4,096 run, then the seek is refused by name.
  expect(seek(clock.now + 1000 / 60)).toMatch(/refused advance: HatchActLimit/);
  expect(button.clicks).toBe(4096);
  button.click = function () { this.clicks++; };
  stop();
  await settled();
});

test('regions and parts are bounded, bound to their elements, and end as tombstones', async () => {
  const hatches = await import(resolve(dir, 'hatches.js'));
  clock.now = 9000;
  const el = (connected = true) => ({ isConnected: connected, localName: 'div', getBoundingClientRect: () => ({ x: 4, y: 4, width: 12, height: 12 }) });
  let handle = null;
  const n = node('owner');
  n.onHatch = e => { handle = e; };
  const end = await mount(hatches, n);
  const of = () => globalThis.exact.hatchRegions.of(n), refused = () => globalThis.exact.hatchState().regionsRefused ?? 0;
  const seal = el();
  expect(handle.owns(seal, 'seal: drawn by the hatch')).toBe(true);
  expect(of().owns).toEqual([{ by: 'element owner', kind: 'view', what: 'seal: drawn by the hatch', observed: { live: true, frame: { x: 4, y: 4, w: 12, h: 12 }, class: 'div' } }]);
  // The same element again replaces its region; a sentence over 120 bytes, or none, is refused.
  handle.owns(seal, 'seal, said again', { surface: true });
  expect(of().owns.length).toBe(1);
  expect(of().owns[0]).toMatchObject({ what: 'seal, said again', surface: true });
  expect(handle.owns(el(), 'x'.repeat(121))).toBe(false);
  expect(handle.owns(el(), '')).toBe(false);
  expect(refused()).toBe(2);
  // 32 regions a scope.
  for (let i = 0; i < 40; i++) handle.owns(el(), `region ${i}`);
  expect(of().owns.length).toBe(32);
  expect(refused()).toBe(2 + 9);
  // Parts: a list replaces the last; one that breaks a bound is refused whole and the last stays.
  handle.parts = [{ id: 'seal', element: seal, role: 'button', label: 'Verified' }];
  handle.parts = [{ id: 'a', element: seal }, { id: 'a', element: seal }];
  handle.parts = Array.from({ length: 33 }, (_, i) => ({ id: `p${i}`, element: seal }));
  handle.parts = [{ id: 'x'.repeat(65), element: seal }];
  handle.parts = [{ id: 'long', element: seal, label: 'l'.repeat(121) }];
  expect(of().parts).toEqual([{ id: 'seal', role: 'button', label: 'Verified', live: true, frame: { x: 4, y: 4, w: 12, h: 12 } }]);
  expect(globalThis.exact.hatchRegions.part(n, 'seal')).toBe(seal);
  expect(refused()).toBe(2 + 9 + 4);
  expect(journal.some(l => /hatch element owner: parts refused: `a` is listed twice/.test(l))).toBe(true);
  // The seal leaves the page: its region is a tombstone, the same at each read, for 5 s of session clock.
  seal.isConnected = false;
  const tomb = of().owns.find(o => o.what === 'seal, said again');
  expect(tomb.observed).toEqual({ live: false, ended: 9000 });
  expect(of().parts[0].live).toBe(false);
  expect(globalThis.exact.hatchRegions.part(n, 'seal')).toBe(null);
  clock.now = 9100;
  expect(of().owns.find(o => o.what === 'seal, said again')).toEqual(tomb);
  clock.now = 14000;
  expect(of().owns.some(o => o.what === 'seal, said again')).toBe(false);
  // The node's end takes what it registered.
  end();
  expect(globalThis.exact.hatchRegions.of(n)).toBe(null);
});
