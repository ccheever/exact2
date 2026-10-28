import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runTests } from '../../scripts/agent-test.mjs';
import { blueskyServer, FAILING_TESTS, TESTS } from '../../scripts/live-reads-fixture.mjs';

// What the Bluesky clone's three live-read tests prove, on the wasm target, through the wasm module's `fetch`, the
// runner and glue.js, against a server of this machine that answers only 300 ms into a clock step
// (scripts/live-reads-fixture.mjs): each `clock` step must wait for the fetch in flight; then every read fails, and
// each test falls back rather than hanging. Against a built dist: `bun scripts/live-reads-fixture.mjs native <dir>
// [port]`, then its wasm build (the line that prints), then EXACT_LIVE_READS_WASM_DIST=<dist>
// EXACT_LIVE_READS_PORT=<port> (47712; one run at a time per port).
test.skipIf(!process.env.EXACT_LIVE_READS_WASM_DIST)('the clone\'s live reads land within clock data on the wasm target, and fall back when they fail', async () => {
  const dist = resolve(process.env.EXACT_LIVE_READS_WASM_DIST);
  let server, page, dir;
  try {
    server = blueskyServer({ delay: 300, port: Number(process.env.EXACT_LIVE_READS_PORT ?? 47712) });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    dir = mkdtempSync(resolve(import.meta.dir, '../../target/live-reads-wasm-'));
    const run = async (name, text) => {
      const file = resolve(dir, name);
      writeFileSync(file, text);
      return runTests({ host: 'web', file, url: page.url.href, webDist: dist, atStep: (s) => server.watch(s) });
    };
    const answered = await run('app.test.contract', TESTS);
    expect(answered.results.map(r => [r.name, r.failures])).toEqual(answered.results.map(r => [r.name, []]));
    expect(answered.results.length).toBe(3);
    server.state.failing = true;
    const failing = await run('failing.test.contract', FAILING_TESTS);
    expect(failing.results.map(r => [r.name, r.failures])).toEqual(failing.results.map(r => [r.name, []]));
    expect(failing.results.length).toBe(3);
  } finally {
    page?.stop(true);
    server?.stop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
}, 300_000);
