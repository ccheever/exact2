// A page's stream on the JS dev loop (host/web-js/dev.mjs /__dev/page) ends when the page goes, whichever loopback
// it came by: the design tool frames its canvas at localhost ([::1], host/web/serve.mjs alsoOnIPv6Loopback), where
// a page that closed stayed a connected build for as long as the loop ran (the tool listed ten on the clone, and
// pinned and inspected them all).
import { afterEach, expect, test } from 'bun:test';
import { spawnOwned, stopAll } from '../../scripts/test-children.mjs';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { connect } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
afterEach(() => stopAll());

test('a page that closes is no build of the loop any more, by 127.0.0.1 or by localhost', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-page-streams-')), dist = mkdtempSync(join(tmpdir(), 'exact-page-streams-dist-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Streams', app: { id: 'com.exact.streams', name: 'Streams' } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "streams"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber streams\n\ntheme\n  ink = #000000\n');
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '', browser;
  dev.stdout.on('data', d => { out += d; });
  dev.stderr.on('data', d => { out = (out + d).slice(-8000); });
  const until = async (pass, ms, what) => { const end = Date.now() + ms; while (!(await pass())) { if (Date.now() > end) throw new Error(`${what}: not seen in ${out.slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
  const base = `http://127.0.0.1:${port}`;
  const builds = async () => {
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const hello = await (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    return hello.ok.builds.length;
  };
  try {
    // Listening (its banner comes after both listeners), and its first build done.
    await until(() => out.includes(`127.0.0.1:${port}/`) && out.includes('plan ready'), 900_000, 'the first build');
    // localhost resolved to [::1] (as the design tool's canvas is reached first), not left to Chrome to choose.
    browser = await suiteBrowser('chromium', { args: ['--host-resolver-rules=MAP localhost [::1]'] });
    const six = await new Promise(ok => { const c = connect(port, '::1'); c.once('connect', () => { c.destroy(); ok(true); }); c.once('error', () => ok(false)); });
    if (!six) console.warn('no IPv6 loopback here: only 127.0.0.1 is checked');
    for (const host of six ? ['127.0.0.1', 'localhost'] : ['127.0.0.1']) {
      const context = await browser.newContext(), page = await context.newPage();
      await page.goto(`http://${host}:${port}/`);
      await until(async () => (await builds()) === 1, 60_000, `${host}: the page's build`);
      await context.close();
      // Gone with its page: within a few seconds, not for as long as the loop runs.
      await until(async () => (await builds()) === 0, 10_000, `${host}: the closed page's build gone`);
    }
    expect(await builds()).toBe(0);
  } finally {
    await browser?.close();
    await dev.stop(30_000);
    rmSync(dir, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}, 1_200_000);
