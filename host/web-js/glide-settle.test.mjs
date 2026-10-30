import { test, expect } from 'bun:test';
import { rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { driveGlide, expectGlide, glideApp, glideServer } from '../../scripts/glide-fixture.mjs';
import { webCompiler } from './module.mjs';

// `clock settle` on the JS target waits for a list a drag left gliding, so its last scroll is delivered before it
// answers; a held drag settles at once; a scroller the page keeps moving gives up at 4 s with reason `scroll`, as a
// native host does (scripts/glide-fixture.mjs).
test('the JS target settles after a drag\'s glide, at once after a held drag, and gives up on a scroller that never stops', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  let server, dir, page, s;
  try {
    server = glideServer();
    dir = glideApp(root, server.origin);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'glide-settle', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    s = await open({ host: 'web', url: page.url.href, webDist: resolve(dir, 'dist') });
    expectGlide(await driveGlide(s), expect);
  } finally {
    await s?.close?.();
    page?.stop(true);
    server?.stop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
}, 180_000);
