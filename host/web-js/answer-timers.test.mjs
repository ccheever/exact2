import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// An answer's own timers on the JS target (LLP 1027.000 D1, amended 2026-10-28), ts-fetch.js and held.js as they
// are: each is an entry of the page clock's timers, owned by its answer, run with that answer current, never run
// once the answer has answered or been let go, and an answer with only timers out counts for no clock.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test("an answer's timer is a clock entry it owns, and only timers out count for no clock", async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-answer-timers-'));
  for (const name of ['ts-fetch.js', 'held.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission.js'), `export { ownWithout, redirectOf } from ${JSON.stringify(pathToFileURL(resolve(ROOT, 'host/web-js/admission.js')).href)};
export class FetchError extends Error {} export const overlaying = { on: false }; export const inOverlay = what => new Error(what);
export const fetchWith = (grants, input, init) => new Promise((_, reject) => init?.signal?.addEventListener('abort', () => reject(init.signal.reason)));\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  try {
    const tsf = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    const held = await import(pathToFileURL(resolve(dir, 'held.js')).href);
    const clock = { now: 10, timers: [], agent: true }, journal = [];
    tsf.answering.page = { clock, journal, drive() {} };
    const inflight = { n: 0 }, owner = {}, call = {}, t = { id: 1, name: 'slept', owner, call, waiting: true };
    owner.ticket = t;
    held.counted(t, inflight);
    expect(inflight.n).toBe(1);

    // Armed in the answer's turn: due on the page's clock, no I/O.
    tsf.answering.call = call;
    let ranAs = null;
    tsf.setTimeout((a, b) => { ranAs = [tsf.answering.call, a, b]; }, 100, 'x', 'y');
    const cleared = tsf.setTimeout(() => { throw new Error('a cleared timer ran'); }, 50);
    tsf.answering.call = null;
    expect(clock.timers.map(e => [e.due, e.data, e.once])).toEqual([[110, true, true], [60, true, true]]);
    expect(inflight.n).toBe(0);
    tsf.clearTimeout(cleared);
    expect(clock.timers.length).toBe(1);

    // It runs as its answer, in flight until the page's next task, and the answer counts again for what follows.
    tsf.answering.call = call;
    tsf.setTimeout(() => {}, 300);
    tsf.answering.call = null;
    clock.now = 110;
    const entry = clock.timers.find(e => e.due === 110);
    clock.timers.splice(clock.timers.indexOf(entry), 1);
    expect(entry.action()).toBe(true);
    expect(ranAs).toEqual([call, 'x', 'y']);
    expect(tsf.answering.call).toBe(null);
    expect(inflight.n).toBe(1);
    await new Promise(r => setTimeout(r));
    expect(inflight.n).toBe(0);
    // Its last timer run, it counts again.
    clock.timers.splice(0)[0].action();
    await new Promise(r => setTimeout(r));
    expect(inflight.n).toBe(1);

    // `AbortSignal.timeout` is one too.
    tsf.answering.call = call;
    const signal = tsf.AbortSignal.timeout(40);
    tsf.answering.call = null;
    expect(clock.timers[0].due).toBe(150);
    clock.timers.splice(0)[0].action();
    expect(signal.aborted && signal.reason.name).toBe('TimeoutError');
    expect(new AbortController().signal instanceof tsf.AbortSignal).toBe(true);

    // A let-go answer's timer never runs, and its bookkeeping ends.
    let abandoned = 0;
    call.abandon = () => { abandoned++; };
    tsf.answering.call = call;
    tsf.setTimeout(() => { throw new Error("a let-go answer's timer ran"); }, 5);
    tsf.answering.call = null;
    owner.ticket = { id: 2 };
    expect(clock.timers.splice(0)[0].action()).toBe(true);
    expect(abandoned).toBe(1);
    // Nor can its chain, which still runs, arm one from then on.
    tsf.answering.call = call;
    expect(() => tsf.setTimeout(() => {}, 1)).toThrow('setTimeout() is unavailable here');
    tsf.answering.call = null;

    // The page races them for real, where natively an answer waits on one thing at a time and its timer runs
    // after the fetch lands (js/tests/it/timers.rs): a timer wins over a hung fetch, and one that aborts it ends it.
    const racing = {};
    tsf.answering.call = racing;
    const raced = Promise.race([tsf.fetch('https://x.test/hung').then(() => 'fetched'), new Promise(r => tsf.setTimeout(() => r('slept'), 100))]);
    const ctrl = new AbortController();
    const aborted = tsf.fetch('https://x.test/hung', { signal: ctrl.signal }).then(() => 'fetched', e => e.name);
    tsf.setTimeout(() => ctrl.abort(), 200);
    tsf.answering.call = null;
    for (const e of clock.timers.splice(0).sort((a, b) => a.due - b.due)) e.action();
    expect(await raced).toBe('slept');
    expect(await aborted).toBe('AbortError');

    // A settle goes only to a live answer's timer.
    tsf.answering.call = racing;
    tsf.setTimeout(() => {}, 1);
    tsf.answering.call = null;
    expect(clock.timers.at(-1).live()).toBe(true);
    racing.settled = true;
    expect(clock.timers.at(-1).live()).toBe(false);
    clock.timers.splice(0);

    // Outside an answer, after it has answered, and every other timer refuse.
    expect(() => tsf.setTimeout(() => {}, 1)).toThrow('setTimeout() is unavailable here');
    tsf.answering.call = { settled: true };
    expect(() => tsf.setTimeout(() => {}, 1)).toThrow('setTimeout() is unavailable here');
    tsf.answering.call = null;
    expect(() => tsf.setInterval(() => {}, 1)).toThrow('setInterval() is unavailable in data sources');
    expect(journal).toEqual([]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
