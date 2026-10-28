import { test, expect } from 'bun:test';
import { rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { driveHeld, heldApp, holdingServer } from '../../scripts/held-fixture.mjs';
import { webCompiler } from './module.mjs';

// A request the server holds (`exactHeld`, LLP 1016.000 D9) on the JS target: no clock waits on it, each
// clock reply names it, the answer's work after its reply is waited on again, and the same ask unmarked is
// waited on before a due timer (calendar F10).
test('the JS target leaves a held fetch out of every clock, names it, and counts the work after its reply', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  const server = holdingServer({ detailDelay: 1500, plainAfter: 3000 });
  const dir = heldApp(root, server.origin);
  let browser, page;
  try {
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'held-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      // (the agent asks a page's /exact.json, a dev envelope this build doesn't write)
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: page.url.href, webDist: resolve(dir, 'dist') });
    const seen = await driveHeld(browser, server);
    // 1. Across 20 timers with the held ask out: no wait, and it is named.
    expect(seen.jump.ms).toBeLessThan(5000);
    expect(seen.jump.r).toMatchObject({ clock: 1000, held: 1, heldTargets: ['item'] });
    expect(seen.jump.r.inflight).toBeUndefined();
    // 2. `clock data` settles beside it, naming it.
    expect(seen.data.r).toMatchObject({ settled: true, held: 1, heldTargets: ['item'] });
    expect(seen.settle.ms).toBeLessThan(5000);
    expect(seen.settle.r).toMatchObject({ settled: true, held: 1, heldTargets: ['item'] });
    expect(seen.state.pending).toEqual([{ name: 'item', ticket: expect.any(Number), held: true }]);
    // 3. The answer's fetch after the held reply (1.5 s) is waited on before the next timer, then lands.
    expect(seen.after.ms).toBeGreaterThan(1000);
    expect(seen.after.r.held).toBeUndefined();
    expect(seen.item).toMatch(/^r\d+\|detail$/);
    // 4. Unmarked, the same hold is waited on (the server answers it 3 s on).
    // `clock held`: waited ~1 s for the reply, no timer fired meanwhile, the work it started landed.
    expect(seen.held.ms).toBeGreaterThan(800);
    expect(seen.held.r).toMatchObject({ settled: true });
    expect(seen.held.r.held).toBeUndefined();
    expect(seen.nAfter).toBe(seen.nBefore);
    expect(seen.heldItem).toMatch(/^r\d+\|detail$/);
    // Past its bound: not settled, naming the target.
    expect(seen.timeout.ms).toBeGreaterThan(900);
    expect(seen.timeout.r).toMatchObject({ settled: false, reason: 'held', heldTargets: ['item'] });
    expect(seen.plain.ms).toBeGreaterThan(2000);
    expect(seen.plain.r.held).toBeUndefined();
  } finally {
    await browser?.close?.();
    page?.stop(true);
    server.stop();
    rmSync(dir, { recursive: true, force: true });
  }
}, 180_000);
