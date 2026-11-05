import { afterEach, expect, jest, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { chmodSync, closeSync, constants, mkdtempSync, openSync, readFileSync, rmSync, writeFileSync, writeSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import process from 'node:process';
import { CLOAK_MS, logStream, opened, openUntil, TRANSITION_MS, until, watchCloak } from './cloak.mjs';

const line = (timestamp, eventMessage) => JSON.stringify({ timestamp, eventMessage });
const UP = 'ContinuousSnapshot: cloaked arriving panel .cover at preflight | colour UIExtendedGrayColorSpace 0 1';
const at = (s) => Date.parse(`2026-10-19T16:08:${s}-07:00`);
const stamp = (ms) => new Date(ms).toISOString().replace('T', ' ').replace('Z', '-0000');
const ID = 'CD4AC74F-B56D-4A56-9A18-CCB66A35C0AA';
const OTHER = '00000000-0000-0000-0000-000000000000';
// Match SpringBoard's main-transition messages on iOS 27.0
const phase = (name, id = ID) => `[Main (EmbeddedDisplay)] dispatch event:\n<SBTransitionSwitcherModifierEvent: 0x11d840600; type: MainTransition; transitionID: ${id}; phase: ${name}; animated: YES; fromAppLayout: 0x0; toEnvironmentMode: application>`;
const transition = (t, name, id = ID) => line(stamp(t), phase(name, id));

afterEach(() => { jest.useRealTimers(); });

test('a cloak covers the display from its logged time, including its offset', () => {
  expect(until(0, line('2026-10-19 16:08:52.060357-0700', UP))).toBe(at('52.060') + CLOAK_MS);
  expect(until(0, line('2026-10-19 23:08:52.060-0000', UP))).toBe(at('52.060') + CLOAK_MS);
  const first = until(0, line('2026-10-19 16:08:52.060-0700', UP));
  expect(until(first, line('2026-10-19 16:08:53.000-0700', UP))).toBe(at('53.000') + CLOAK_MS);
  expect(until(first, line('2026-10-19 16:08:51.000-0700', UP))).toBe(first);
});

test('headers, unrelated messages and invalid times leave covers unchanged', () => {
  const open = new Map([[ID, at('52.060')]]);
  for (const l of [
    'Filtering the log data',
    line('2026-10-19 16:08:54.030-0700', 'ContinuousSnapshot: dropping overlays at transition end'),
    line('2026-10-19 16:08:54.017-0700', 'ContinuousSnapshot: cloak reveal completed'),
    line('2026-10-19 16:08:54.017-0700', '[Main (EmbeddedDisplay)] handle response:\n<SBSwitcherModifierEventResponse: 0x11dba6d00>'),
    ...[UP, phase('Prepare'), phase('Complete')].flatMap((message) => [
      line('yesterday', message), line('2026-99-19 16:08:54.017-0700', message), JSON.stringify({ eventMessage: message }),
    ]),
  ]) {
    expect(until(7, l)).toBe(7);
    expect(opened(open, l)).toBe(open);
  }
});

test('main transitions remain open by ID until Complete, with a bound from their last phase', () => {
  const t = at('52.060');
  let open = opened(new Map(), transition(t, 'Prepare'));
  expect(openUntil(open)).toBe(t + TRANSITION_MS);
  open = opened(open, transition(t + 50, 'Animate'));
  expect(openUntil(open)).toBe(t + 50 + TRANSITION_MS);
  open = opened(open, transition(t, 'Prepare'));
  expect(openUntil(open)).toBe(t + 50 + TRANSITION_MS);
  open = opened(open, transition(t + 100, 'Animate', OTHER));
  open = opened(open, transition(t + 500, 'Complete', ID.toLowerCase()));
  expect(openUntil(open)).toBe(t + 100 + TRANSITION_MS);
  expect(openUntil(opened(open, transition(t + 600, 'Complete', OTHER)))).toBe(0);
  open = opened(open, transition(t + 100 + TRANSITION_MS, 'Prepare'));
  expect(open.has(OTHER)).toBe(false);
  expect(openUntil(open)).toBe(t + 100 + 2 * TRANSITION_MS);
});

function followed(foldable) {
  let emit, predicate;
  const w = watchCloak('udid', foldable, (udid, filter, read) => {
    expect(udid).toBe('udid');
    predicate = filter;
    emit = read;
    return { ready: async () => {}, close() {} };
  });
  return { ...w, emit: (l) => emit(l), predicate };
}

test('a cloak arriving during a main transition keeps the touch waiting after Complete', async () => {
  jest.useFakeTimers({ now: at('52.060') });
  const t = Date.now(), w = followed(true);
  expect(w.predicate).toContain('category == "AppSwitcher"');
  expect(w.predicate).toContain('category == "DisplayContentMode"');
  w.emit(transition(t, 'Prepare'));
  let cleared = false;
  const clearing = w.clear().then((r) => { cleared = true; return r; });
  await Promise.resolve();
  expect(cleared).toBe(false);
  jest.advanceTimersByTime(100);
  w.emit(line(stamp(Date.now()), UP));
  w.emit(transition(Date.now(), 'Complete'));
  await Promise.resolve();
  jest.advanceTimersByTime(CLOAK_MS - 1);
  await Promise.resolve();
  expect(cleared).toBe(false);
  jest.advanceTimersByTime(1);
  expect(await clearing).toEqual({ cloaked: CLOAK_MS, transitioned: 100 });
  expect(await w.clear()).toEqual({ cloaked: 0, transitioned: 0 });
});

test('every simulator waits for main transitions, and only a foldable one waits for cloaks', async () => {
  jest.useFakeTimers({ now: at('52.060') });
  for (const foldable of [false, undefined]) {
    const t = Date.now(), w = followed(foldable);
    expect(w.predicate).toContain('category == "AppSwitcher"');
    expect(w.predicate).not.toContain('DisplayContentMode');
    w.emit(line(stamp(t), UP));
    expect(await w.clear()).toEqual({ cloaked: 0, transitioned: 0 });
    w.emit(transition(t, 'Animate'));
    let cleared = false;
    const clearing = w.clear().then((r) => { cleared = true; return r; });
    await Promise.resolve();
    expect(cleared).toBe(false);
    jest.advanceTimersByTime(250);
    w.emit(transition(Date.now(), 'Complete'));
    expect(await clearing).toEqual({ cloaked: 0, transitioned: 250 });
  }
});

test('a missing Complete releases the touch at the last phase deadline', async () => {
  jest.useFakeTimers({ now: at('52.060') });
  const t = Date.now(), w = followed(false);
  w.emit(transition(t, 'Prepare'));
  const clearing = w.clear();
  jest.advanceTimersByTime(100);
  w.emit(transition(Date.now(), 'Animate'));
  await Promise.resolve();
  let cleared = false;
  clearing.then(() => { cleared = true; });
  jest.advanceTimersByTime(TRANSITION_MS - 1);
  await Promise.resolve();
  expect(cleared).toBe(false);
  jest.advanceTimersByTime(1);
  expect(await clearing).toEqual({ cloaked: 0, transitioned: 100 + TRANSITION_MS });
});

test('the log stream joins chunks and reaps its process on close', async () => {
  // Disable the readiness deadline so only the first partial chunk releases the writer
  jest.useFakeTimers();
  const dir = mkdtempSync(join(tmpdir(), 'exact-cloak-')), path = process.env.PATH;
  const fifo = join(dir, 'release');
  let release, stream;
  try {
    expect(spawnSync('mkfifo', [fifo]).status).toBe(0);
    release = openSync(fifo, constants.O_RDWR);
    const cloak = line(stamp(at('52.060')), UP), received = Promise.withResolvers(), lines = [];
    const quote = (value) => `'${value.replaceAll(/'/g, `'\\''`)}'`;
    writeFileSync(join(dir, 'xcrun'), `#!/bin/sh\necho $$ > ${quote(join(dir, 'pid'))}\nprintf '%s' ${quote(cloak.slice(0, 40))}\nread -r _ < ${quote(fifo)}\nprintf '%s\\n' ${quote(cloak.slice(40))}\nexec sleep 60\n`);
    chmodSync(join(dir, 'xcrun'), 0o755);
    process.env.PATH = `${dir}:${path ?? ''}`;
    stream = logStream('udid', 'predicate', (l) => {
      lines.push(l);
      if (l === cloak) { received.resolve(); }
    });
    await stream.ready();
    expect(lines).toEqual([]);
    writeSync(release, 'go\n');
    await received.promise;
    expect(lines).toEqual([cloak]);
    const pid = Number(readFileSync(join(dir, 'pid'), 'utf8'));
    expect(pid).toBeGreaterThan(0);
    await stream.close();
    let exitError;
    try { process.kill(pid, 0); } catch (e) { exitError = e; }
    expect(exitError?.code).toBe('ESRCH');
  } finally {
    await stream?.close();
    if (release !== undefined) { closeSync(release); }
    if (path === undefined) { delete process.env.PATH; }
    else { process.env.PATH = path; }
    rmSync(dir, { recursive: true, force: true });
  }
}, 30_000);
