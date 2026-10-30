import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// The file-body module (ts-body-from.js) loads on an exactBodyFrom fetch's first
// use: the request is the call's, and its deadline and abort hold while it loads.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const realHelpers = `export { ownWithout, redirectOf, signalDeadline, within } from ${JSON.stringify(pathToFileURL(resolve(ROOT, 'host/web-js/admission.js')).href)}; `;

test('an exactBodyFrom fetch captures its options before loading the file-body module', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-options-'));
  for (const name of ['ts-fetch.js', 'ts-body-from.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission.js'), `${realHelpers}export class FetchError extends Error { constructor(kind, message) { super(message); this.kind = kind; } } export const coversPath = () => true; export const fetchWith = async (set, input, init, expires, redirect, read, origin, decided) => { globalThis.uploads.push({ method: init.method, path: await init.body.text(), timeout: init.exactTimeout, signal: init.signal, decided }); return new Response(); }; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n'); writeFileSync(resolve(dir, 'storage-fs.js'), 'export const requestBody = async (app, grants, path) => new Blob([path]); export const bodyTarget = async () => ({ close() {} });\n');
  globalThis.uploads = [];
  try {
    const { fetch: appFetch, AbortSignal: AppAbortSignal, answering } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    const options = Object.defineProperty({ method: 'POST', exactTimeout: 5000 }, 'exactBodyFrom', { value: 'app:/data/a', writable: true });
    const first = appFetch('https://x.test/up', options);
    options.method = 'PUT'; options.exactBodyFrom = 'app:/data/b'; options.exactTimeout = 1000;
    const second = appFetch('https://x.test/up', options);
    // Beside an `AbortSignal.timeout` as long, the `exactTimeout` gives way to the signal's deadline (LLP 1146 §2.1)
    answering.page = { clock: { now: 0, timers: [], agent: true }, journal: [], drive() {} };
    answering.call = {};
    const signal = AppAbortSignal.timeout(5000);
    answering.call = null;
    const third = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/c', exactTimeout: 5000, signal });
    // An inherited signal is the request's too
    const fourth = appFetch('https://x.test/up', Object.assign(Object.create({ signal }), { method: 'POST', exactBodyFrom: 'app:/data/d', exactTimeout: 5000 }));
    const fifth = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/e', exactBodyTo: 'app:/data/f' });
    await Promise.all([first, second, third, fourth, fifth]);
    const uploads = Object.fromEntries(globalThis.uploads.map(({ method, path, timeout, signal }) => [path, { method, timeout, signal }]));
    expect(uploads).toMatchObject({ 'app:/data/a': { method: 'POST' }, 'app:/data/b': { method: 'PUT' } });
    expect(uploads['app:/data/a'].timeout).toBeGreaterThan(uploads['app:/data/b'].timeout);
    expect(uploads['app:/data/c'].timeout).toBeUndefined();
    expect([uploads['app:/data/d'].timeout, uploads['app:/data/d'].signal === signal]).toEqual([undefined, true]);
    // Every upload's deadline was decided before its file work, so `fetchWith` does not weigh it again
    expect(globalThis.uploads.length === 5 && globalThis.uploads.every(u => u.decided === true)).toBe(true);
  } finally { delete globalThis.uploads; rmSync(dir, { recursive: true, force: true }); }
});

test('an exactBodyFrom fetch times out and aborts while its module is loading', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-load-'));
  cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), resolve(dir, 'ts-fetch.js'));
  writeFileSync(resolve(dir, 'ts-body-from.js'), 'await globalThis.bodyImportGate;\n' + readFileSync(resolve(ROOT, 'host/web-js/ts-body-from.js'), 'utf8'));
  writeFileSync(resolve(dir, 'admission.js'), `${realHelpers}export class FetchError extends Error { constructor(kind, message) { super(message); this.kind = kind; } } export const coversPath = () => true; export const fetchWith = async () => { globalThis.sentBodies++; return new Response(); }; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  let release, timed, aborted, signalled, tied, stopped, stoppedTimeout;
  globalThis.bodyImportGate = new Promise(resolveGate => { release = resolveGate; }); globalThis.sentBodies = 0;
  try {
    const { fetch: appFetch, AbortSignal: AppAbortSignal, answering } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    timed = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/a', exactTimeout: 10 }).catch(e => e);
    const controller = new AbortController(); aborted = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/b', signal: controller.signal }).catch(e => e);
    controller.abort(new Error('stopped'));
    // An `AbortSignal.timeout`'s deadline holds too, and beside an `exactTimeout` as long it is the request's (LLP 1146 §2.1)
    answering.page = { clock: { now: 0, timers: [], agent: true }, journal: [], drive() {} };
    answering.call = {};
    const signal = AppAbortSignal.timeout(10), tiedSignal = AppAbortSignal.timeout(10), stoppedSignal = AppAbortSignal.timeout(10);
    answering.call = null;
    signalled = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/c', signal }).catch(e => e);
    tied = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/d', signal: tiedSignal, exactTimeout: 10 }).catch(e => e);
    // An abort listener of the app's that stops the event, added first, ends neither an abort nor a deadline, as it
    // ends no browser's fetch
    const stop = event => event.stopImmediatePropagation();
    const halted = new AbortController();
    halted.signal.addEventListener('abort', stop);
    stoppedSignal.addEventListener('abort', stop);
    stopped = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/e', signal: halted.signal }).catch(e => e);
    stoppedTimeout = appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/f', signal: stoppedSignal }).catch(e => e);
    halted.abort(new Error('halted'));
    const guard = promise => Promise.race([promise, new Promise(resolveGuard => setTimeout(() => resolveGuard(new Error('still pending')), 100))]);
    const [timeout, abort, own, tie, halt, haltedTimeout] = await Promise.all([guard(timed), guard(aborted), guard(signalled), guard(tied), guard(stopped), guard(stoppedTimeout)]);
    expect([timeout.kind, timeout.message, abort.message, globalThis.sentBodies]).toEqual(['Timeout', 'the request timed out after 10 ms', 'stopped', 0]);
    expect([own === signal.reason, tie === tiedSignal.reason, signal.reason.name, tiedSignal.reason.name]).toEqual([true, true, 'TimeoutError', 'TimeoutError']);
    expect([halt.message, haltedTimeout === stoppedSignal.reason]).toEqual(['halted', true]);
  } finally {
    release(); await Promise.allSettled([timed, aborted, signalled, tied, stopped, stoppedTimeout].filter(Boolean));
    delete globalThis.bodyImportGate; delete globalThis.sentBodies; rmSync(dir, { recursive: true, force: true });
  }
});

test("an exactBodyTo fetch's signal deadline holds while its file opens, past an app's listener that stops the event", async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-open-'));
  for (const name of ['ts-fetch.js', 'ts-body-from.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission.js'), `${realHelpers}export class FetchError extends Error { constructor(kind, message) { super(message); this.kind = kind; } } export const coversPath = () => true; export const fetchWith = async () => { globalThis.sentBodies++; return new Response(); }; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  // The target says when it starts to open, opens only when the test says, and says when it is closed
  writeFileSync(resolve(dir, 'storage-fs.js'), 'export const requestBody = async () => new Blob([]); export const bodyTarget = async () => { globalThis.entered(); await globalThis.openGate; return { close() { globalThis.closed(); } }; };\n');
  let release, opening;
  const entering = new Promise(done => { globalThis.entered = done; }), closing = new Promise(done => { globalThis.closed = done; });
  globalThis.openGate = new Promise(resolveGate => { release = resolveGate; }); globalThis.sentBodies = 0;
  // The deadline's browser timer, held until the target is opening and then fired by hand
  const realSet = globalThis.setTimeout;
  let deadline;
  globalThis.setTimeout = (run, ms, ...args) => { if (ms === 10 && !deadline) { deadline = run; return 0; } return realSet(run, ms, ...args); };
  try {
    const { fetch: appFetch, AbortSignal: AppAbortSignal, answering } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    answering.page = { clock: { now: 0, timers: [], agent: true }, journal: [], drive() {} };
    answering.call = {};
    const signal = AppAbortSignal.timeout(10);
    answering.call = null;
    // An abort listener of the app's that stops the event does not keep it from ending here either
    signal.addEventListener('abort', event => event.stopImmediatePropagation());
    opening = appFetch('https://x.test/down', { exactBodyTo: 'app:/data/down', signal }).catch(e => e);
    await entering;
    deadline();
    const ended = await Promise.race([opening, new Promise(resolveGuard => realSet(() => resolveGuard(new Error('still pending')), 100))]);
    expect([ended === signal.reason, signal.reason.name, globalThis.sentBodies]).toEqual([true, 'TimeoutError', 0]);
    // The target that opens after the deadline is closed, never written
    release();
    await closing;
  } finally {
    globalThis.setTimeout = realSet;
    release(); await opening;
    delete globalThis.openGate; delete globalThis.sentBodies; delete globalThis.entered; delete globalThis.closed; rmSync(dir, { recursive: true, force: true });
  }
});

test("an exactBodyTo fetch's write is refused once its deadline has passed by the clock, its timer fired or not, and a write before it clears the signal deadline's timer", async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-publish-'));
  for (const name of ['ts-fetch.js', 'ts-body-from.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  // The reply's body arrives at once; the write asks the request whether it has ended just before it replaces the file
  writeFileSync(resolve(dir, 'admission.js'), `${realHelpers}export class FetchError extends Error { constructor(kind, message) { super(message); this.kind = kind; } } export const coversPath = () => true; export const fetchWith = async (set, input, init, expires, redirect, read) => { await read(new Response('new')); return new Response(null, { status: 200 }); }; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n`);
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  // A body read the caller's abort ends rejects with its reason (`bodyFails`); a write held for its lock (`held`) runs
  // its refusal only once let go, as a queued write does
  writeFileSync(resolve(dir, 'storage-fs.js'), 'export const requestBody = async () => new Blob([]); export const bodyTarget = async () => ({ async put(reply, limit, ended, refusal) { globalThis.publishing?.(); if (globalThis.bodyFails) throw globalThis.bodyFails(); if (globalThis.held) { globalThis.held(refusal); return new Promise((_, failed) => { const end = () => failed(refusal()); if (ended.aborted) end(); else ended.addEventListener("abort", end, { once: true }); }); } const refused = refusal(); if (refused) throw refused; return 3; }, close() {} });\n');
  const clock = globalThis.performance, realNow = clock.now, realSet = globalThis.setTimeout, realClear = globalThis.clearTimeout;
  let skew = 0;
  const cleared = [];
  clock.now = () => realNow.call(clock) + skew;
  try {
    const { fetch: appFetch, AbortSignal: AppAbortSignal, answering } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    // The clock passes the deadline before the file would be replaced, while the deadline's signal has not aborted
    globalThis.publishing = () => { skew = 10_000; };
    const late = await appFetch('https://x.test/down', { exactBodyTo: 'app:/data/down', exactTimeout: 5000 }).catch(e => e);
    expect([late.kind, late.message]).toEqual(['Timeout', 'the request timed out after 5000 ms']);
    // In time, the file is written; the signal deadline's real-time timer is cleared when the request is done
    globalThis.publishing = undefined;
    globalThis.setTimeout = (run, ms, ...args) => (ms === 5000 ? 4242 : realSet(run, ms, ...args));
    globalThis.clearTimeout = id => { cleared.push(id); realClear(id); };
    answering.page = { clock: { now: 0, timers: [], agent: true }, journal: [], drive() {} };
    answering.call = {};
    const signal = AppAbortSignal.timeout(5000);
    answering.call = null;
    const written = await appFetch('https://x.test/down', { exactBodyTo: 'app:/data/down', signal });
    expect([written.status, written.exactBodyTo, cleared.includes(4242)]).toEqual([200, { path: 'app:/data/down', bytes: 3 }, true]);
    // A signal's deadline due by the clock as the file would be replaced, its timer not yet fired, aborts the signal
    // then and refuses the write
    answering.call = {};
    const due = AppAbortSignal.timeout(5000);
    answering.call = null;
    globalThis.publishing = () => { skew += 10_000; };
    const overdue = await appFetch('https://x.test/down', { exactBodyTo: 'app:/data/down', signal: due }).catch(e => e);
    expect([overdue === due.reason, due.reason?.name]).toEqual([true, 'TimeoutError']);
    // A caller's abort refuses it with the caller's own reason, whatever that reason says
    const controller = new AbortController(), reason = Object.assign(new Error('stop'), { kind: 'Unavailable' });
    globalThis.publishing = () => controller.abort(reason);
    const stopped = await appFetch('https://x.test/down', { exactBodyTo: 'app:/data/down', signal: controller.signal }).catch(e => e);
    expect(stopped).toBe(reason);
    // and so does a body read that abort ends
    const reading = new AbortController(), readReason = Object.assign(new Error('stop reading'), { kind: 'Unavailable' });
    globalThis.publishing = () => reading.abort(readReason);
    globalThis.bodyFails = () => reading.signal.reason;
    const unread = await appFetch('https://x.test/down', { exactBodyTo: 'app:/data/down', signal: reading.signal }).catch(e => e);
    expect(unread).toBe(readReason);
    delete globalThis.bodyFails;
    // A write still queued when the caller's abort ended the request, run once its deadline has passed, aborts no
    // signal: the request's deadline ended with it
    globalThis.publishing = undefined;
    answering.call = {};
    const timeout = AppAbortSignal.timeout(5000);
    answering.call = null;
    const caller = new AbortController();
    let queued;
    globalThis.held = refusal => { queued = refusal; caller.abort(new Error('let go')); };
    const ended = await appFetch('https://x.test/down', { exactBodyTo: 'app:/data/down', signal: AppAbortSignal.any([caller.signal, timeout]) }).catch(e => e);
    delete globalThis.held;
    skew += 10_000;
    const refusedLate = queued();
    expect([ended.message, refusedLate?.message, timeout.aborted]).toEqual(['let go', 'let go', false]);
  } finally {
    clock.now = realNow; globalThis.setTimeout = realSet; globalThis.clearTimeout = realClear;
    delete globalThis.publishing; delete globalThis.bodyFails; delete globalThis.held; rmSync(dir, { recursive: true, force: true });
  }
});
