import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

// The agent's `layout` reports each scroller's range beside its offset (LLP 1012 `rx`/`ry`), in `nodes` and in
// `layout <target>`'s scroll chain: a scroller that hides `overflow-x` keeps the programmatic range the browser gives
// it, and the viewport's entry is there while the page has a range, though it is not scrolled.
test('a scroller\'s range and the page\'s are in layout and in the scroll chain', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/scroll-range-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Scroll Range Test', app: { id: 'test.scroll-range', name: 'Scroll Range Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `component ScrollRange
  view
    column testId="page"
      scroll testId="wide" width=400 height=200 overflow-x="hidden"
        box width=484 height=600 testId="wide-content"
          text "Wide" testId="wide-text"
      box height=2000 testId="filler"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'scroll-range-test', '--out', dist, '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      if (path.startsWith('/shared/')) return new Response(Bun.file(resolve(root, 'host/web', path.slice('/shared/'.length))));
      const file = resolve(dist, path === '/' ? 'index.html' : path.slice(1));
      return existsSync(file) ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
    const wide = (await browser.layout()).nodes.find((n) => n.testId === 'wide');
    expect(wide).toMatchObject({ sx: 0, sy: 0, rx: 84, ry: 400 });
    const chain = (await browser.layout('wide-text')).node.scroll;
    expect(chain[0]).toMatchObject({ viewport: true, sx: 0, sy: 0, rx: 0 });
    expect(chain[0].ry).toBeGreaterThan(1000);
    expect(chain[1]).toMatchObject({ id: wide.id, sx: 0, sy: 0, rx: 84, ry: 400 });
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 120_000);
