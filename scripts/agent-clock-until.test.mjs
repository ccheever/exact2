// `clock +N real until <path>` (theme2 2026-10-24, a test harness's signal): `clock +N real`, ended early once the
// file at <path> exists, the clock moved by exactly the real time gone; met at once when it is there already;
// refused for a URL, a glob or a directory that does not exist. The driver's actual method, on a stand-in host whose
// wall clock the test moves (as caps.test.mjs's `clock +N real` test does).
import { expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { MalformedOperation } from './agent-keys.mjs';

const source = readFileSync(new URL('./agent.mjs', import.meta.url), 'utf8');
const start = source.indexOf("    async clock(spec = 'settle'"), end = source.indexOf('\n    /** The window', start);

/** The method on a web host at clock 1000, its wall clock moving 100 ms a look; `at(ms)` runs as the wall passes ms. */
function harness(events = {}) {
  let wall = 0;
  const sent = [], s = { now: 1000, async op(req) { sent.push(req.to); return { clock: req.to }; } };
  const performance = { now: () => { const t = wall; wall += 100; for (const [ms, f] of Object.entries(events)) if (t >= Number(ms)) { f(); delete events[ms]; } return t; } };
  const clock = new Function('s', 'carrier', 'performance', 'setTimeout', 'REAL_STEP_MS', 'statSync', 'resolve', 'dirname', 'MalformedOperation',
    `return ({${source.slice(start, end)}}).clock;`)(s, { host: 'web' }, performance, (done) => done(), 50, statSync, resolve, dirname, MalformedOperation);
  return { clock, sent, s };
}

test('met: the wait ends once the file appears, the clock moved by the real time gone, not the bound', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-clock-until-')), file = join(dir, 'go');
  try {
    // The harness writes the file 1 s into a 6 s wait.
    const { clock, sent, s } = harness({ 1000: () => writeFileSync(file, '') });
    const reply = await clock(`+6000 real until ${file}`);
    expect(reply.until).toBe('met');
    // The clock went as far as the real time did (a step at a time, as `+M real` with M that time), never to 7000.
    expect(reply.clock).toBeGreaterThanOrEqual(2000);
    expect(reply.clock).toBeLessThan(2500);
    expect(s.now).toBe(reply.clock);
    expect(sent.every((to, i) => i === 0 || to >= sent[i - 1])).toBe(true);
    // A timer due 500 ms in fired on the way (the clock passed it); one due 3 s in did not (the clock stopped short).
    expect(sent.some(to => to >= 1500)).toBe(true);
    expect(sent.some(to => to >= 4000)).toBe(false);
    expect(reply.real).toBeGreaterThanOrEqual(reply.clock - 1000);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('bound: no file, and it is `clock +N real` to the end, saying so', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-clock-until-'));
  try {
    const { clock, sent } = harness();
    const reply = await clock(`+2000 real until ${join(dir, 'never')}`);
    expect([reply.until, reply.clock, sent.at(-1)]).toEqual(['bound', 3000, 3000]);
    // Without `until`, the reply is as ever: no `until` field.
    expect('until' in await harness().clock('+200 real')).toBe(false);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('a file there already: met at once, 0 ms, the clock unmoved', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-clock-until-')), file = join(dir, 'go');
  try {
    writeFileSync(file, '');
    const { clock, sent } = harness();
    expect(await clock(`+6000 real until ${file}`)).toEqual({ clock: 1000, real: 0, until: 'met' });
    expect(sent).toEqual([]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('refused: a directory that does not exist (or is a file), a directory for the file, a URL, a glob', async () => {
  const { clock, sent } = harness();
  await expect(clock(`+6000 real until ${join(tmpdir(), 'exact-clock-until-none', 'go')}`)).rejects.toThrow(/its directory .*exact-clock-until-none does not exist/);
  const dir = mkdtempSync(join(tmpdir(), 'exact-clock-until-'));
  try {
    writeFileSync(join(dir, 'file'), '');
    await expect(clock(`+6000 real until ${join(dir, 'file', 'go')}`)).rejects.toThrow(/its directory .*file does not exist/);
    mkdirSync(join(dir, 'sub'));
    await expect(clock(`+6000 real until ${join(dir, 'sub')}`)).rejects.toThrow(/a directory, not the file a harness writes/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
  await expect(clock('+6000 real until file:///tmp/go')).rejects.toThrow(/a file path on this machine, not a URL or a glob/);
  await expect(clock('+6000 real until https://example.test/go')).rejects.toThrow(/not a URL or a glob/);
  await expect(clock(`+6000 real until ${tmpdir()}/*.go`)).rejects.toThrow(/not a URL or a glob/);
  expect(sent).toEqual([]);
  // A Windows drive path is a path (its one-letter "scheme" is a drive), never refused as a URL: it waits its bound, or
  // (where C:\\signals is missing, on Windows) is refused for its directory.
  const drive = await harness().clock('+200 real until C:\\signals\\go').then(r => r.until, e => e.message);
  expect(drive).not.toMatch(/not a URL or a glob/);
  expect(drive).toMatch(/^bound$|its directory .* does not exist/);
});
