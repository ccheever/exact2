// Build exact-web-js once before running this test (docs/testing.md).
// LLP 1121 stage 11 B2, review round 1 (Astra 4): a real live edit on a
// page. A design that sets `space.md` and aliases it (`card.pad: length =
// space.md`) is built for development and shown in Chrome; the edit
// `space.md = 40px` to `48px` is compiled as the dev loop compiles it, sent
// as its batch through the page's own `exact.design`, and both the row that
// names `space.md` and the one that names the alias move from 40px to 48px,
// with no reload.
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../scripts/agent-launch.mjs';

const VIEW = `component App
  view
    column testId="direct" padding="space.md"
      column testId="alias" padding="card.pad"
        text "a"
`;
const design = px => `tamber t\n\ntheme\n  space.md = ${px}\n  card.pad: length = space.md\n`;

test('a live edit of a base name the app sets moves its row and its alias on the page', async () => {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  mkdirSync(target, { recursive: true });
  const app = mkdtempSync(resolve(target, 'base-live-app-'));
  const dir = mkdtempSync(resolve(target, 'base-live-out-'));
  const sheet = readFileSync(resolve(root, 'host/web/index.html'), 'utf8').match(/<style>([\s\S]*?)<\/style>/)[1];
  const bin = resolve(target, 'debug/exact-web-js');
  let server, browser;
  try {
    writeFileSync(resolve(app, 'app.contract'), VIEW);
    writeFileSync(resolve(app, 'app.tamber'), design('40px'));
    const built = spawnSync(bin, ['js', resolve(app, 'app.contract'), '-o', dir, '--dev-reload'], { encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    const before = JSON.parse(readFileSync(resolve(dir, 'dev-design.json'), 'utf8'));
    // The edit, compiled as the dev loop's design read compiles it.
    const edited = resolve(app, 'edited.tamber');
    writeFileSync(edited, design('48px'));
    const read = spawnSync(bin, ['design', resolve(app, 'app.contract'), edited], { encoding: 'utf8' });
    expect(read.status, read.stderr).toBe(0);
    const next = JSON.parse(read.stdout);
    // A value-only edit: the dev loop keeps the build and batches it.
    expect(next.structure).toBe(before.structure);
    expect(next.shape).toBe(before.shape);
    const changed = next.tokens.filter((t, i) => JSON.stringify(t) !== JSON.stringify(before.tokens[i]));
    expect(changed.map(t => t.name).sort()).toEqual(['card.pad', 'space.md']);
    const batch = { kind: 'tamber-batch', structure: next.shape, base: before.digest, design: next.digest, tokens: changed };

    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<style>${sheet}</style><link rel="stylesheet" href="/app.css">
        <div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
      { headers: { 'content-type': 'text/html' } });
      const file = [dir, import.meta.dir, resolve(root, 'host/web')]
        .map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage({ viewport: { width: 400, height: 300 } });
    await page.goto(server.url.href);
    const direct = page.getByTestId('direct'), alias = page.getByTestId('alias');
    await alias.waitFor({ state: 'attached' });
    const padding = e => e.evaluate(n => getComputedStyle(n).paddingTop);
    expect([await padding(direct), await padding(alias)]).toEqual(['40px', '40px']);
    const marker = await page.evaluate(() => (window.__kept = 1));
    const live = await page.evaluate(b => globalThis.exact.design(b).live, batch);
    expect(live).toBe('applied');
    expect([await padding(direct), await padding(alias)]).toEqual(['48px', '48px']);
    // The same page: no reload.
    expect(await page.evaluate(() => window.__kept)).toBe(marker);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(app, { recursive: true, force: true });
    rmSync(dir, { recursive: true, force: true });
  }
}, 60000);

// Review round 1 (Astra 2): an alias to a base name the design leaves unset
// switched live (`card.pad: length = space.md` to `space.lg`) would name a
// property the page's sheet never declared. Its structure moves, so the dev
// loop builds (`sameShape` in dev.mjs), and the build declares it: the page
// shows space.lg's length in this build's (neutral) column.
test('an alias switched to a base the sheet lacks rebuilds, and the build shows it', async () => {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  const app = mkdtempSync(resolve(target, 'base-switch-app-'));
  const dir = mkdtempSync(resolve(target, 'base-switch-out-'));
  const bin = resolve(target, 'debug/exact-web-js');
  const alias = base => `tamber t\n\ntheme\n  card.pad: length = ${base}\n`;
  let server, browser;
  try {
    writeFileSync(resolve(app, 'app.contract'), `component App\n  view\n    column testId="alias" padding="card.pad"\n      text "a"\n`);
    writeFileSync(resolve(app, 'app.tamber'), alias('space.md'));
    const build = () => {
      const built = spawnSync(bin, ['js', resolve(app, 'app.contract'), '-o', dir, '--dev-reload'], { encoding: 'utf8' });
      expect(built.status, built.stderr).toBe(0);
      return JSON.parse(readFileSync(resolve(dir, 'dev-design.json'), 'utf8'));
    };
    const before = build();
    expect(readFileSync(resolve(dir, 'app.css'), 'utf8')).not.toContain('--exact-space-lg');
    const edited = resolve(app, 'edited.tamber');
    writeFileSync(edited, alias('space.lg'));
    const read = spawnSync(bin, ['design', resolve(app, 'app.contract'), edited], { encoding: 'utf8' });
    expect(read.status, read.stderr).toBe(0);
    const next = JSON.parse(read.stdout);
    expect(next.shape).toBe(before.shape);
    expect(next.structure).not.toBe(before.structure);
    // The build the dev loop runs then.
    writeFileSync(resolve(app, 'app.tamber'), alias('space.lg'));
    expect(build().structure).toBe(next.structure);
    expect(readFileSync(resolve(dir, 'app.css'), 'utf8')).toContain('--exact-space-lg:24px');
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<link rel="stylesheet" href="/app.css"><div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
        { headers: { 'content-type': 'text/html' } });
      const file = [dir, import.meta.dir, resolve(root, 'host/web')]
        .map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage({ viewport: { width: 400, height: 300 } });
    await page.goto(server.url.href);
    const row = page.getByTestId('alias');
    await row.waitFor({ state: 'attached' });
    expect(await row.evaluate(n => getComputedStyle(n).paddingTop)).toBe('24px');
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(app, { recursive: true, force: true });
    rmSync(dir, { recursive: true, force: true });
  }
}, 60000);
