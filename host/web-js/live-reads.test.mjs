import { test, expect } from 'bun:test';
import { rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runTests } from '../../scripts/agent-test.mjs';
import { blueskyServer, FAILING_TESTS, liveReadsApp, TESTS } from '../../scripts/live-reads-fixture.mjs';
import { webCompiler } from './module.mjs';

// What the Bluesky clone's three live-read tests prove, on the JS target, against a server of this machine that
// answers only 300 ms into a clock step (scripts/live-reads-fixture.mjs): each `clock` step must wait for the fetch
// in flight. Then every read fails, and each test falls back rather than hanging.
test('the clone\'s live reads land within clock data, and fall back when they fail', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  // runTests names the app in EXACT_APP_DIR (scripts/app.mjs resolveApp); the next test file reads its own.
  const appDir = process.env.EXACT_APP_DIR;
  let server, dir, page;
  try {
    server = blueskyServer({ delay: 300 });
    dir = liveReadsApp(root, server.origin);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'live-reads', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    const run = async (name, text) => {
      const file = resolve(dir, name);
      writeFileSync(file, text);
      return runTests({ host: 'web', file, app: dir, url: page.url.href, atStep: (s) => server.watch(s) });
    };
    const answered = await run('app.test.contract', TESTS);
    expect(answered.results.map(r => [r.name, r.failures])).toEqual(answered.results.map(r => [r.name, []]));
    expect(answered.results.length).toBe(3);
    expect(server.seen).toContain('/xrpc/app.bsky.feed.getFeed.poll');
    server.state.failing = true;
    const failing = await run('failing.test.contract', FAILING_TESTS);
    expect(failing.results.map(r => [r.name, r.failures])).toEqual(failing.results.map(r => [r.name, []]));
    expect(failing.results.length).toBe(3);
  } finally {
    page?.stop(true);
    server?.stop();
    if (dir) rmSync(dir, { recursive: true, force: true });
    if (appDir === undefined) delete process.env.EXACT_APP_DIR;
    else process.env.EXACT_APP_DIR = appDir;
  }
}, 300_000);
