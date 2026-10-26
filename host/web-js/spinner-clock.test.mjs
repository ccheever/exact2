// An indeterminate `progress` turns on the agent's clock (LLP 1069.001: "under the agent it follows the agent's
// clock"): two pictures of the same clock are the same bytes, however much wall time passes between them, from the
// first one at boot on. conform's boot screenshot once caught the ring at two angles (synthetic-controls, 0.24%).
import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

test('two boot pictures of a spinner on the agent clock are byte-identical', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/spinner-clock-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Spinner Clock Test', app: { id: 'test.spinner-clock', name: 'Spinner Clock Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `component SpinnerClock
  view
    column padding=16 gap=8
      progress testId="small"
      progress testId="large" width=80 height=80
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'spinner-clock-test', '--out', dist], {
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
    browser = await open({ host: 'web', url: server.url.href, size: [400, 300] });
    // Held from the first render's commit, before a frame shows it, at the boot's clock: never running on the
    // browser's clock (or turned on the compositor) while agent.js was still loading.
    const turns = await browser.carrier.evaluate(`JSON.stringify(document.getAnimations().map(a => [a.animationName, a.playState, a.currentTime]))`);
    expect(JSON.parse(turns)).toEqual([['exact-progress', 'paused', 0], ['exact-progress', 'paused', 0]]);
    // The first picture straight after boot, with no clock op or read before it; the second a third of a turn of
    // wall time later.
    const first = resolve(dir, 'first.png'), second = resolve(dir, 'second.png');
    await browser.screenshot(first, false, undefined, true);
    await new Promise(r => setTimeout(r, 330));
    await browser.screenshot(second, false, undefined, true);
    expect(readFileSync(second).equals(readFileSync(first))).toBe(true);
    // A spinner whose turn began after the last commit (here one added outside any commit, as a sheet arriving late
    // would start one) is the clock's in the next picture too.
    await browser.carrier.evaluate(`(() => { const p = document.querySelector('[data-testid="large"]'), c = p.cloneNode(true); c.removeAttribute('data-testid'); p.after(c); })()`);
    const third = resolve(dir, 'third.png'), fourth = resolve(dir, 'fourth.png');
    await browser.screenshot(third, false, undefined, true);
    await new Promise(r => setTimeout(r, 330));
    await browser.screenshot(fourth, false, undefined, true);
    expect(readFileSync(fourth).equals(readFileSync(third))).toBe(true);
    // A second boot's first picture is the first's, byte for byte.
    await browser.close?.();
    browser = await open({ host: 'web', url: server.url.href, size: [400, 300] });
    const again = resolve(dir, 'again.png');
    await browser.screenshot(again, false, undefined, true);
    expect(readFileSync(again).equals(readFileSync(first))).toBe(true);
  } finally {
    await browser?.close?.();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 120_000);
