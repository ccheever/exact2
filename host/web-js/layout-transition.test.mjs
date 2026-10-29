// A bound `-exact-layout-transition` on the JS runtime (GitHub #809): the shape reduced motion takes, `none` while calm
// and a curve otherwise. A row holding the curve slides when a row above it goes; one whose value is `none` jumps; one
// that gains the curve in the commit that moves it slides from where it was, as presence-glue.js plays a style op that
// gains the row. Build exact-web-js once before running this test (docs/testing.md).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';
import { withLook } from '../web/look.mjs';

const SOURCE = `component App
  state calm = false
  state shown = true
  action motion
    calm = not calm
  action drop
    shown = not shown
  action both
    calm = not calm
    shown = not shown
  view
    column
      button "Motion" press=motion testId="motion"
      button "Drop" press=drop testId="drop"
      button "Both" press=both testId="both"
      if shown
        text "First" testId="first" height=40
      text "Second" testId="second" height=40 -exact-layout-transition=(calm ? "none" : "2000ms linear")
`;

test('a bound layout transition slides while it holds a curve, jumps at none, and slides from where it was when gained', async () => {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  mkdirSync(target, { recursive: true });
  const dir = mkdtempSync(resolve(target, 'layout-transition-'));
  const style = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
  let server, browser;
  try {
    writeFileSync(resolve(dir, 'app.contract'), SOURCE);
    const compiled = spawnSync(resolve(target, 'debug/exact-web-js'), ['js', resolve(dir, 'app.contract'), '-o', dir], { encoding: 'utf8' });
    expect(compiled.status, compiled.stderr).toBe(0);
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<style>${style}</style><link rel="stylesheet" href="/app.css">
        <div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
      { headers: { 'content-type': 'text/html' } });
      // A module that re-exports the host's own reaches it through `../web/`.
      const bases = path.startsWith('web/') ? [resolve(root, 'host')] : [dir, import.meta.dir, resolve(root, 'host/web')];
      const file = bases.map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await suiteBrowser();
    const page = await (await browser.newContext({ viewport: { width: 400, height: 700 } })).newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => globalThis.exact?.presenceLive);
    // The ops presence hears, as wasm's batch would carry them: one only for a view gaining (before) or losing or gaining (after) the row.
    await page.evaluate(() => {
      const p = globalThis.exact.presenceLive, heard = globalThis.heard = [];
      for (const k of ['before', 'after']) { const f = p[k].bind(p); p[k] = (batch, views) => { heard.push([k, (batch.ops ?? []).length]); return f(batch, views); }; }
    });
    const heard = () => page.evaluate(() => globalThis.heard.splice(0));
    // Each press: the row's declaration, then where it shows now against where it lands, and whether it plays.
    const press = (id) => page.evaluate(async (id) => {
      const second = document.querySelector('[data-testid="second"]');
      document.querySelector(`[data-testid="${id}"]`).click();
      await new Promise((r) => setTimeout(r));
      const shown = second.getBoundingClientRect().top, playing = second.getAnimations().length > 0;
      await Promise.all(second.getAnimations().map((a) => (a.finish(), a.finished)));
      return { css: second.style.getPropertyValue('--exact-layout-transition'), moving: playing && Math.abs(shown - second.getBoundingClientRect().top) > 1 };
    }, id);
    expect(await press('drop')).toEqual({ css: '2000 0 linear', moving: true });
    expect(await heard()).toEqual([['before', 0], ['after', 0]]);
    expect(await press('motion')).toEqual({ css: '', moving: false });
    expect(await heard()).toEqual([['before', 0], ['after', 1]]);
    // Calm: nothing to measure (no op), and it jumps.
    expect(await press('drop')).toEqual({ css: '', moving: false });
    expect(await heard()).toEqual([['before', 0], ['after', 0]]);
    // Gained in the commit that moves it (First goes again).
    expect(await press('both')).toEqual({ css: '2000 0 linear', moving: true });
    expect(await heard()).toEqual([['before', 1], ['after', 1]]);
    // Lost in the commit that moves it: it jumps, and nothing plays it after.
    expect(await press('both')).toEqual({ css: '', moving: false });
    expect(await heard()).toEqual([['before', 0], ['after', 1]]);
    expect(await page.evaluate(() => globalThis.exact.presenceLive.tracking())).toBe(false);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 120000);
