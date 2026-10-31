import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// An answer's own timers on the JS target (LLP 1027.000 D1, amended 2026-10-28), ts-fetch.js and held.js as they
// are: each is an entry of the page clock's timers, owned by its answer, run with that answer current, never run
// once the answer has answered or been let go, and an answer with only timers out counts for no clock.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

// ts-fetch.js and held.js copied beside a fake admission whose fetch answers when the test lands it
// (`globalThis.landing`) and rejects when its signal aborts.
async function load() {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-answer-timers-'));
  for (const name of ['ts-fetch.js', 'held.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission.js'), `export { ownWithout, redirectOf } from ${JSON.stringify(pathToFileURL(resolve(ROOT, 'host/web-js/admission.js')).href)};
export class FetchError extends Error {} export const overlaying = { on: false }; export const inOverlay = what => new Error(what);
export const signalDeadline = { arm: null };
// As the real one: the deadline armed with the signal the request reads, ended when it is done
export const fetchWith = (grants, input, init) => { const armed = signalDeadline.arm?.(init?.signal);
  return new Promise((land, reject) => { globalThis.landing?.set(String(input), land); init?.signal?.addEventListener('abort', () => reject(init.signal.reason)); }).finally(() => armed?.cancel()); };\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  const tsf = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
  const held = await import(pathToFileURL(resolve(dir, 'held.js')).href);
  return { dir, tsf, held };
}

test("an answer's timer is a clock entry it owns, and only timers out count for no clock", async () => {
  const { dir, tsf, held } = await load();
  try {
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

// LLP 1146 §2.1: a fetch carrying an `AbortSignal.timeout`, or a signal composed of one, has the time the signal has
// left on the page's clock as its deadline, in real time, its answer over or not, as natively (js/tests/it/timers.rs).
test("a fetch carrying an AbortSignal.timeout has the signal's time left as its deadline", async () => {
  const { dir, tsf } = await load();
  globalThis.landing = new Map();
  // The browser timers the deadlines take, recorded and fired here by hand, the real time they read moved with them; a
  // 0 ms one (the end of an answer's turn) runs as it is
  const { setTimeout: realSet, clearTimeout: realClear } = globalThis, realNow = performance.now;
  const deadlines = new Map();
  let next = 1, elapsed = 0;
  performance.now = () => elapsed;
  globalThis.setTimeout = (run, ms, ...args) => {
    if (!ms) return realSet(run, ms, ...args);
    const id = `deadline ${next++}`;
    deadlines.set(id, { run, ms });
    return id;
  };
  globalThis.clearTimeout = id => { if (!deadlines.delete(id)) realClear(id); };
  const due = () => [...deadlines.values()].map(d => d.ms);
  // Fire the timers of `ms`, `late` ms after they were due
  const fire = (ms, late = 0) => {
    elapsed += ms + late;
    for (const [id, d] of [...deadlines]) if (d.ms === ms) { deadlines.delete(id); d.run(); }
  };
  try {
    const clock = { now: 0, timers: [], agent: true };
    tsf.answering.page = { clock, journal: [], drive() {} };
    const answer = () => { const owner = {}, ticket = { owner }; owner.ticket = ticket; return { ticket }; };
    const outcome = p => p.then(() => 'landed', e => e.name);

    // A POST its answer let go: the answer's timer went with it, and the request still aborts at the deadline, the
    // 20 ms its signal, made 4980 ms of the page's clock earlier, has left.
    const posting = answer();
    tsf.answering.call = posting;
    const signal = tsf.AbortSignal.timeout(5000);
    clock.now = 4980;
    const posted = outcome(tsf.fetch('https://x.test/post', { method: 'POST', signal }));
    tsf.answering.call = null;
    posting.ticket.owner.gone = true;
    tsf.dropTimers(posting, true);
    expect(clock.timers).toEqual([]);
    expect(due()).toEqual([20]);
    fire(20);
    expect(await posted).toBe('TimeoutError');
    expect(signal.reason.name).toBe('TimeoutError');

    // Composed with `AbortSignal.any`, the earliest timeout aborts it, whatever its place, one inside a composed argument
    // too, and another answer may carry the signal.
    const making = answer();
    tsf.answering.call = making;
    const early = tsf.AbortSignal.timeout(20), late = tsf.AbortSignal.timeout(5000);
    const composed = tsf.AbortSignal.any([late, tsf.AbortSignal.any(new Set([early])), new AbortController().signal]);
    tsf.answering.call = null;
    making.settled = true;
    tsf.dropTimers(making);
    tsf.answering.call = answer();
    const carried = outcome(tsf.fetch('https://x.test/any', { method: 'POST', signal: composed }));
    tsf.answering.call = null;
    expect(due()).toEqual([20]);
    fire(20);
    expect(await carried).toBe('TimeoutError');
    expect([early.aborted, late.aborted]).toEqual([true, false]);

    // In its answer, the deadline also removes the answer's timer; a request that lands first ends its deadline.
    const live = answer();
    tsf.answering.call = live;
    const quick = tsf.AbortSignal.timeout(20);
    const landed = tsf.fetch('https://x.test/lands', { signal: quick });
    const hung = outcome(tsf.fetch('https://x.test/hung', { signal: tsf.AbortSignal.timeout(20) }));
    tsf.answering.call = null;
    expect(due()).toEqual([20, 20]);
    globalThis.landing.get('https://x.test/lands')(new Response('ok'));
    await landed;
    expect(due()).toEqual([20]);
    fire(20);
    expect(await hung).toBe('TimeoutError');
    expect(quick.aborted).toBe(false);
    expect(clock.timers.length).toBe(1);
    expect(live.timers).toBe(1);

    // The signal aborts in the turn of the answer that fetched, so a listener of that answer's may arm a timer.
    const listening = answer();
    tsf.answering.call = listening;
    const heard = tsf.AbortSignal.timeout(20);
    heard.addEventListener('abort', () => tsf.setTimeout(() => {}, 5));
    const listened = outcome(tsf.fetch('https://x.test/listened', { signal: heard }));
    tsf.answering.call = null;
    fire(20);
    expect(await listened).toBe('TimeoutError');
    expect(clock.timers.filter(e => e.call === listening).map(e => e.due)).toEqual([clock.now + 5]);

    // A mutation's fetch the socket guard sends a turn later, once its answer is no longer current, still aborts in the
    // mutation's turn
    const mutating = { ...answer(), mutation: true }, sockets = tsf.answering.sockets;
    tsf.answering.call = mutating;
    tsf.answering.sockets = true;
    const guarded = tsf.AbortSignal.timeout(20);
    guarded.addEventListener('abort', () => tsf.setTimeout(() => {}, 5));
    const deferred = outcome(tsf.fetch('https://x.test/guarded', { method: 'POST', signal: guarded }));
    tsf.answering.call = null;
    await new Promise(done => realSet(done, 0));
    tsf.answering.sockets = sockets;
    expect(due()).toEqual([20]);
    fire(20);
    expect(await deferred).toBe('TimeoutError');
    expect(clock.timers.filter(e => e.call === mutating).map(e => e.due)).toEqual([clock.now + 5]);

    // Each request takes the time its signal has left when it is made: one made with 20 ms of the signal left aborts when
    // they pass, though an earlier one carrying it, made with all 5000 ms left, is still in flight.
    clock.now = 10_000;
    tsf.answering.call = answer();
    const shared = tsf.AbortSignal.timeout(5000);
    const first = outcome(tsf.fetch('https://x.test/first', { method: 'POST', signal: shared }));
    clock.now = 14_980;
    const second = outcome(tsf.fetch('https://x.test/second', { method: 'POST', signal: shared }));
    tsf.answering.call = null;
    expect(due()).toEqual([5000, 20]);
    fire(20);
    expect([await second, await first]).toEqual(['TimeoutError', 'TimeoutError']);
    expect(due()).toEqual([]);

    // A deadline past the browser timers' longest delay waits in steps of it, each step waiting only what is left by the
    // real clock, so one that came late ends it at once
    clock.now = 0;
    tsf.answering.call = answer();
    const longest = tsf.AbortSignal.timeout(2147483648), longer = tsf.AbortSignal.timeout(2147483647 + 60_000);
    const waited = outcome(tsf.fetch('https://x.test/long', { method: 'POST', signal: longest }));
    tsf.answering.call = null;
    expect(due()).toEqual([2147483647]);
    fire(2147483647);
    expect([due(), longest.aborted]).toEqual([[1], false]);
    fire(1);
    expect(await waited).toBe('TimeoutError');
    tsf.answering.call = answer();
    const overdue = outcome(tsf.fetch('https://x.test/late', { method: 'POST', signal: longer }));
    tsf.answering.call = null;
    fire(2147483647, 120_000);
    expect([await overdue, due()]).toEqual(['TimeoutError', []]);
  } finally {
    globalThis.setTimeout = realSet;
    globalThis.clearTimeout = realClear;
    performance.now = realNow;
    delete globalThis.landing;
    rmSync(dir, { recursive: true, force: true });
  }
});

// Its argument as Web IDL converts a sequence, as Chrome's own `AbortSignal.any` does: the iterator method, `next`
// and each `value` read once, each value converted as it is yielded.
test("AbortSignal.any converts its argument as the platform's does", async () => {
  const { dir, tsf } = await load();
  try {
    const signal = new AbortController().signal;
    const said = f => { try { f(); return 'made'; } catch (e) { return e.name; } };
    let reads = 0, nexts = 0, n = 0, gets = 0;
    const once = { get [Symbol.iterator]() { reads++; return reads === 1 ? function* () { yield signal; } : undefined; } };
    function* lateThrow() { yield {}; throw new Error('after the bad value'); }
    const badStep = { [Symbol.iterator]: () => ({ next: () => 1 }) };
    const nextOnce = { [Symbol.iterator]: () => ({ get next() { nexts++; return () => ({ done: n++ > 0, value: signal }); } }) };
    const valueOnce = { [Symbol.iterator]: () => { let k = 0; return { next: () => (k++ ? { done: true } : { done: false, get value() { gets++; return signal; } }) }; } };
    // A `next` with a `call` of its own, which the conversion does not use
    const ownCall = { [Symbol.iterator]: () => ({ next: Object.defineProperty(() => ({ done: true }), 'call', { value: () => ({ done: false, value: {} }) }) }) };
    // A value refused leaves its iterator open: Web IDL's conversion does not close it, nor does Chrome's (Chrome for
    // Testing 156, by hand)
    let closed = false;
    function* closing() { try { yield {}; } finally { closed = true; } }
    const any = tsf.AbortSignal.any;
    expect([said(() => any(once)), reads, said(() => any(lateThrow())), said(() => any(badStep)), said(() => any(nextOnce)), nexts,
      said(() => any(valueOnce)), gets, said(() => any(ownCall)), said(() => any('ab')), said(() => any(closing())), closed])
      .toEqual(['made', 1, 'TypeError', 'TypeError', 'made', 1, 'made', 1, 'made', 'TypeError', 'TypeError', false]);
    const controller = new AbortController(), composed = any(new Set([controller.signal]));
    controller.abort('why');
    expect([composed.aborted, composed.reason]).toEqual([true, 'why']);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
