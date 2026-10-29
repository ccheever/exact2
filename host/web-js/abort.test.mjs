import { test, expect } from 'bun:test';
import { rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runTests } from '../../scripts/agent-test.mjs';
import { abortApp, abortServer, TESTS, within } from '../../scripts/abort-fixture.mjs';
import { webCompiler } from './module.mjs';

// A fetch the app aborts, on the JS target (scripts/abort-fixture.mjs): the browser cancels it, so the server sees
// its connection close; the aborted answer goes on, and the next request goes out at once.
test('an aborted fetch is cancelled on the wire, on the JS target', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  // runTests names the app in EXACT_APP_DIR (scripts/app.mjs resolveApp); the next test file reads its own.
  const appDir = process.env.EXACT_APP_DIR;
  let server, dir, page;
  try {
    server = abortServer();
    dir = abortApp(root, server.origin);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'abort-fixture', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    const file = resolve(dir, 'app.test.contract');
    writeFileSync(file, TESTS);
    const ran = await runTests({ host: 'web', file, app: dir, url: page.url.href });
    expect(ran.results.map(r => [r.name, r.failures])).toEqual(ran.results.map(r => [r.name, []]));
    expect(ran.results.length).toBe(1);
    expect(server.seen).toEqual(['/held', '/next']);
    expect(await within(2000, () => server.closed.includes('/held'))).toBe(true);
  } finally {
    page?.stop(true);
    server?.stop();
    if (dir) rmSync(dir, { recursive: true, force: true });
    if (appDir === undefined) delete process.env.EXACT_APP_DIR;
    else process.env.EXACT_APP_DIR = appDir;
  }
}, 300_000);
