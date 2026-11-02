// SpringBoard's fold cloak as the touch driver reads it from the log (cloak.mjs).
import { expect, test } from 'bun:test';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import process from 'node:process';
import { CLOAK_MS, until, watchCloak } from './cloak.mjs';

const line = (timestamp, eventMessage) => JSON.stringify({ timestamp, eventMessage, subsystem: 'com.apple.SpringBoard', category: 'DisplayContentMode' });
const UP = 'ContinuousSnapshot: cloaked arriving panel .cover at preflight | colour UIExtendedGrayColorSpace 0 1';
const at = (s) => Date.parse(`2026-10-19T16:08:${s}-07:00`);

test('a cloak going up covers a display for CLOAK_MS from its logged time', () => {
  expect(until(0, line('2026-10-19 16:08:52.060357-0700', UP))).toBe(at('52.060') + CLOAK_MS);
  expect(until(0, line('2026-10-19 23:08:52.060-0000', UP))).toBe(at('52.060') + CLOAK_MS);
});

test('a later cloak extends the cover and an earlier one never shortens it', () => {
  const first = until(0, line('2026-10-19 16:08:52.060-0700', UP));
  expect(until(first, line('2026-10-19 16:08:53.000-0700', UP))).toBe(at('53.000') + CLOAK_MS);
  expect(until(first, line('2026-10-19 16:08:51.000-0700', UP))).toBe(first);
});

test('the stream\'s header, other lines and unreadable times change nothing', () => {
  for (const l of [
    'Filtering the log data using "process == "SpringBoard""',
    line('2026-10-19 16:08:54.030-0700', 'ContinuousSnapshot: dropping overlays at transition end'),
    line('2026-10-19 16:08:54.017-0700', 'ContinuousSnapshot: cloak reveal completed'),
    line('yesterday', UP),
    JSON.stringify({ eventMessage: UP }),
  ]) expect(until(7, l)).toBe(7);
});

test('the watcher waits out a cloak it reads in chunks, and ends its stream on close', async () => {
  // A stand-in for `xcrun simctl spawn … log stream`: the header, a cloak's line in two chunks, then nothing
  const dir = mkdtempSync(join(tmpdir(), 'exact-cloak-')), path = process.env.PATH;
  const deadline = Date.now() + 1500, cloak = line(new Date(deadline - CLOAK_MS).toISOString().replace('T', ' ').replace('Z', '-0000'), UP);
  writeFileSync(join(dir, 'xcrun'), `#!/bin/sh\necho $$ > '${dir}/pid'\necho 'Filtering the log data'\nprintf '%s' '${cloak.slice(0, 40)}'\nsleep 0.1\nprintf '%s\\n' '${cloak.slice(40)}'\nexec sleep 60\n`);
  chmodSync(join(dir, 'xcrun'), 0o755);
  process.env.PATH = `${dir}:${path}`;
  try {
    const w = watchCloak('udid');
    await w.ready();
    await new Promise((r) => setTimeout(r, 400));
    expect(await w.clear()).toBeGreaterThan(500);
    expect(Date.now()).toBeGreaterThanOrEqual(deadline);
    const pid = Number(readFileSync(join(dir, 'pid'), 'utf8'));
    w.close();
    const alive = () => { try { process.kill(pid, 0); return true; } catch { return false; } };
    for (const end = Date.now() + 3000; alive() && Date.now() < end;) await new Promise((r) => setTimeout(r, 20));
    expect(alive()).toBe(false);
  } finally {
    process.env.PATH = path;
    rmSync(dir, { recursive: true, force: true });
  }
});
