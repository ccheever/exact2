// An import planned in a process of its own (scripts/tamber-isolate.mjs):
// answered as the serve answers, and stopped by the watchdog past its
// memory or its time, one at a time.
import { expect, test } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { planAlone } from './tamber-isolate.mjs';

const DESIGN = 'tamber t\n\ntheme\n  primary = #000000\n';
const withDesign = async f => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-tamber-isolate-'));
  try { writeFileSync(join(dir, 'app.tamber'), DESIGN); return await f(join(dir, 'app.tamber')); } finally { rmSync(dir, { recursive: true, force: true }); }
};

test('a plan is answered as the serve answers it, from the design file read when its turn comes', async () => {
  await withDesign(async design => {
    const got = await planAlone({ kind: 'shadcn', source: ':root { --primary: #ff0000; }' }, design);
    expect(got.ok?.ops?.length).toBeGreaterThan(0);
    expect(typeof got.ok.digest).toBe('string');
    const bad = await planAlone({ kind: 'nope', source: 'x' }, design);
    expect(bad.error?.message).toMatch(/no importer|kind/);
  });
}, 600_000); // a cold checkout builds the compiler first

test('past its time it is stopped: 503; past its memory: 413', async () => {
  await withDesign(async design => {
    const sleeper = () => spawn('sh', ['-c', 'cat > /dev/null; sleep 30'], { stdio: ['pipe', 'pipe', 'pipe'], detached: true });
    const slow = await planAlone({ kind: 'shadcn', source: '' }, design, { ms: 300, spawnPlan: sleeper });
    expect([slow.status, slow.error.message]).toEqual([503, 'the source is too costly to read: reading it took more than 0.3 s']);
    const hog = () => spawn('sh', ['-c', 'cat > /dev/null; sleep 30'], { stdio: ['pipe', 'pipe', 'pipe'], detached: true });
    const big = await planAlone({ kind: 'shadcn', source: '' }, design, { rssKb: 1, ms: 5000, spawnPlan: hog });
    expect(big.status).toBe(413);
    expect(big.error.message).toMatch(/too costly to read/);
  });
}, 60_000);

test('one at a time: a second waits for the first', async () => {
  await withDesign(async design => {
    const order = [];
    const timed = (name, ms) => () => { order.push(`start ${name}`); const c = spawn('sh', ['-c', `cat > /dev/null; sleep ${ms / 1000}; echo '{"ok":{}}'`], { stdio: ['pipe', 'pipe', 'pipe'], detached: true }); c.on('close', () => order.push(`end ${name}`)); return c; };
    await Promise.all([planAlone({}, design, { spawnPlan: timed('a', 300) }), planAlone({}, design, { spawnPlan: timed('b', 10) })]);
    expect(order).toEqual(['start a', 'end a', 'start b', 'end b']);
  });
}, 60_000);
