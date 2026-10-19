// An import planned in a process of its own (scripts/tamber-isolate.mjs):
// answered as the serve answers, stopped by the watchdog past its memory,
// its time or its answer's size, or when it can't be watched; one at a
// time, a full queue refused; the design read when its turn comes, a file
// of the app.
import { expect, test } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { planAlone, stopImports } from './tamber-isolate.mjs';

const DESIGN = 'tamber t\n\ntheme\n  primary = #000000\n';
const withApp = async f => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-tamber-isolate-'));
  try { writeFileSync(join(dir, 'app.tamber'), DESIGN); return await f(join(dir, 'app.tamber'), dir); } finally { rmSync(dir, { recursive: true, force: true }); }
};
/** A stand-in planner: `script` run by sh, its stdin read first. */
const sh = script => () => spawn('sh', ['-c', `cat > /dev/null; ${script}`], { stdio: ['pipe', 'pipe', 'pipe'], detached: true });

test('a plan is answered as the serve answers it, from the design file read when its turn comes', async () => {
  // The planner is the compiler cargo built; build it if this checkout has not.
  const { spawnSync } = await import('node:child_process');
  expect(spawnSync('cargo', ['build', '-q', '-p', 'contract'], { cwd: new URL('..', import.meta.url).pathname, stdio: 'inherit' }).status).toBe(0);
  await withApp(async (design, app) => {
    const got = await planAlone({ kind: 'shadcn', source: ':root { --primary: #ff0000; }' }, design, {}, app);
    expect(got.ok?.ops?.length).toBeGreaterThan(0);
    expect(typeof got.ok.digest).toBe('string');
    const bad = await planAlone({ kind: 'nope', source: 'x' }, design, {}, app);
    expect(bad.error?.message).toMatch(/no importer|kind/);
    // Queued behind a slow one, the design changes: the new text is the one planned.
    const slow = planAlone({}, design, { spawnPlan: sh('sleep 0.3; echo \'{"ok":{}}\'') }, app);
    const next = planAlone({ kind: 'shadcn', source: ':root { --primary: #000000; }' }, design, {}, app);
    writeFileSync(design, DESIGN.replace('#000000', '#ff0000'));
    await slow;
    expect((await next).ok.ops.length).toBeGreaterThan(0);
  });
}, 600_000); // a cold checkout builds the compiler first

test('stopped past its time (503), its memory, really allocated (413), or its answer\'s size (413); unwatchable, stopped (503)', async () => {
  await withApp(async (design, app) => {
    const slow = await planAlone({}, design, { ms: 300, spawnPlan: sh('sleep 30') }, app);
    expect([slow.status, slow.error.message]).toEqual([503, 'the source is too costly to read: reading it took more than 0.3 s']);
    const hog = () => spawn(process.execPath, ['-e', 'const a = []; setInterval(() => a.push(Buffer.alloc(64 << 20, 1)), 20);'], { stdio: ['pipe', 'pipe', 'pipe'], detached: true });
    const big = await planAlone({}, design, { rssKb: 256 << 10, ms: 20_000, spawnPlan: hog }, app);
    expect([big.status, big.error.message]).toEqual([413, 'the source is too costly to read: reading it took more than 256 MiB']);
    const loud = await planAlone({}, design, { outBytes: 1000, spawnPlan: sh('head -c 100000 /dev/zero | tr "\\0" a; sleep 30') }, app);
    expect([loud.status, loud.error.message]).toEqual([413, 'the source is too costly to read: its review is over 1000 bytes']);
    const blind = await planAlone({}, design, { probe: async () => null, spawnPlan: sh('sleep 30') }, app);
    expect([blind.status, blind.error.message]).toEqual([503, 'the import could not be watched: try again']);
  });
}, 60_000);

test('one at a time; a full queue is refused; an answer split inside a character reads whole', async () => {
  await withApp(async (design, app) => {
    const order = [];
    const timed = (name, s) => () => { order.push(`start ${name}`); const c = sh(`sleep ${s}; echo '{"ok":{}}'`)(); c.on('close', () => order.push(`end ${name}`)); return c; };
    await Promise.all([planAlone({}, design, { spawnPlan: timed('a', 0.3) }, app), planAlone({}, design, { spawnPlan: timed('b', 0.01) }, app)]);
    expect(order).toEqual(['start a', 'end a', 'start b', 'end b']);
    await new Promise(ok => setTimeout(ok, 200)); // (b's planner reaped: its permit back)
    const all = Array.from({ length: 4 }, () => planAlone({}, design, { waiting: 2, spawnPlan: sh('sleep 0.2; echo \'{"ok":{}}\'') }, app));
    const replies = await Promise.all(all);
    expect(replies.map(r => r.status ?? 200)).toEqual([200, 200, 200, 503]);
    // "é" is two bytes; the planner writes the first, waits, then the second.
    const split = await planAlone({}, design, { spawnPlan: sh(`printf '{"ok":{"f":"\\303'; sleep 0.2; printf '\\251"}}'`) }, app);
    expect(split.ok.f).toBe('é');
  });
}, 60_000);

test('the design must be a regular file of the app', async () => {
  await withApp(async (design, app) => {
    const outside = mkdtempSync(join(tmpdir(), 'exact-tamber-outside-'));
    try {
      writeFileSync(join(outside, 'app.tamber'), DESIGN);
      symlinkSync(join(outside, 'app.tamber'), join(app, 'linked.tamber'));
      const got = await planAlone({}, join(app, 'linked.tamber'), { spawnPlan: sh('echo \'{"ok":{}}\'') }, app);
      expect([got.status, got.error.message]).toEqual([400, 'the design is not a file of the app']);
      const gone = await planAlone({}, join(app, 'none.tamber'), { spawnPlan: sh('echo \'{"ok":{"none":true}}\'') }, app);
      expect(gone.ok.none).toBe(true); // no design yet: planned against none
    } finally { rmSync(outside, { recursive: true, force: true }); }
  });
}, 60_000);

test('a waiting import expires on its own clock; a FIFO in the design\'s place is refused, never waited on', async () => {
  await withApp(async (design, app) => {
    const first = planAlone({}, design, { spawnPlan: sh('sleep 0.5; echo \'{"ok":{}}\'') }, app);
    const second = await planAlone({}, design, { waitMs: 100, spawnPlan: sh('echo \'{"ok":{}}\'') }, app);
    expect(second.status).toBe(503);
    expect((await first).ok).toEqual({});
    // A FIFO in the design's place: refused at once, never waited on.
    const { execFileSync } = await import('node:child_process');
    rmSync(design);
    execFileSync('mkfifo', [design]);
    const fifo = await planAlone({}, design, { spawnPlan: sh('echo \'{"ok":{}}\'') }, app);
    expect([fifo.status, fifo.error.message]).toEqual([400, 'the design is not a regular file']);
  });
}, 60_000);

test('one client holds at most two imports; another is still taken', async () => {
  await withApp(async (design, app) => {
    const slow = sh('sleep 0.3; echo \'{"ok":{}}\'');
    const three = await Promise.all([1, 2, 3].map(() => planAlone({}, design, { spawnPlan: slow }, app, 'figma')).concat(planAlone({}, design, { spawnPlan: slow }, app, 'tool')));
    expect(three.map(r => r.status ?? 200)).toEqual([200, 200, 503, 200]);
  });
}, 60_000);

test('an import whose request or client goes leaves the queue at once, or is stopped while it runs; the server\'s close cancels them all', async () => {
  await withApp(async (design, app) => {
    const slow = sh('sleep 0.5; echo \'{"ok":{}}\'');
    // A planner that says when it started, and when it ended.
    const watched = script => { let child; const started = new Promise(ok => { watched.start = ok; }); return { started, ended: () => new Promise(ok => (child.exitCode !== null || child.signalCode !== null ? ok() : child.once('exit', ok))), spawn: () => { child = sh(script)(); watched.start(); return child; } }; };
    // Running: stopped (its planner's sleep would take 30 s), its process ended.
    const run = new AbortController(), t0 = Date.now(), p1 = watched('sleep 30');
    const running = planAlone({}, design, { signal: run.signal, spawnPlan: p1.spawn }, app);
    await p1.started;
    run.abort();
    expect([(await running).status, (await running).error.message, Date.now() - t0 < 5000]).toEqual([503, 'the import was cancelled', true]);
    await p1.ended();
    await new Promise(ok => setTimeout(ok, 200)); // (its planner reaped: its permit back)
    // Waiting: answered at once, its place given back (a queue of one takes another).
    const first = planAlone({}, design, { spawnPlan: slow }, app);
    const wait = new AbortController();
    const second = planAlone({}, design, { waiting: 1, signal: wait.signal, spawnPlan: slow }, app);
    wait.abort();
    expect((await second).error.message).toBe('the import was cancelled');
    const third = planAlone({}, design, { waiting: 1, spawnPlan: slow }, app);
    expect((await first).ok).toEqual({});
    expect((await third).ok).toEqual({});
    // Already gone: never queued.
    expect((await planAlone({}, design, { signal: AbortSignal.abort(), spawnPlan: slow }, app)).error.message).toBe('the import was cancelled');
    // The server's close: the running one stopped, the waiting one cancelled.
    const p2 = watched('sleep 30');
    const a = planAlone({}, design, { spawnPlan: p2.spawn }, app), b = planAlone({}, design, { spawnPlan: slow }, app);
    await p2.started;
    stopImports();
    const [ra, rb] = await Promise.all([a, b]);
    expect([ra.error.message, rb.error.message]).toEqual(['the import was cancelled', 'the import was cancelled']);
    await p2.ended();
  });
}, 60_000);
