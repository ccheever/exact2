// Build exact-web-js before running these tests (docs/testing.md).
// Live token edits must update both direct rows and aliases without reloading.
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

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
      // A module that re-exports the host's own reaches it through `../web/`.
      const bases = path.startsWith('web/') ? [resolve(root, 'host')] : [dir, import.meta.dir, resolve(root, 'host/web')];
      const file = bases.map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await suiteBrowser();
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

// Once a plan uses the base scale, switching aliases keeps its structure.
// Introducing the scale changes the structure and requires a rebuild.
test('an alias switched between base names goes live; the first base name moves the structure', async () => {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  const app = mkdtempSync(resolve(target, 'base-switch-app-'));
  const dir = mkdtempSync(resolve(target, 'base-switch-out-'));
  const bin = resolve(target, 'debug/exact-web-js');
  const alias = base => `tamber t\n\ntheme\n  card.pad: length = ${base}\n`;
  let server, browser;
  try {
    writeFileSync(resolve(app, 'app.contract'), `component App\n  view\n    column testId="alias" padding="card.pad"\n      text "a"\n`);
    const build = (design) => {
      writeFileSync(resolve(app, 'app.tamber'), design);
      const built = spawnSync(bin, ['js', resolve(app, 'app.contract'), '-o', dir, '--dev-reload'], { encoding: 'utf8' });
      expect(built.status, built.stderr).toBe(0);
      return JSON.parse(readFileSync(resolve(dir, 'dev-design.json'), 'utf8'));
    };
    // A scrub's patch, as the dev loop asks `design-serve` for one.
    const patch = (before, values) => {
      const asked = spawnSync(bin, ['design-serve', resolve(app, 'app.contract')], {
        encoding: 'utf8', input: `${JSON.stringify({ id: 1, key: `${before.shape} ${before.digest}`, patch: values })}\n`,
      });
      expect(asked.status, asked.stderr).toBe(0);
      return JSON.parse(asked.stdout.trim().split('\n').at(-1));
    };
    // None to nine: the structure moves, in a build and in a scrub's patch.
    const plain = build(alias('12px'));
    expect(readFileSync(resolve(dir, 'app.css'), 'utf8')).not.toContain('--exact-space');
    expect(patch(plain, [['card.pad', ['space.md']]]).structure).not.toBe(plain.structure);
    const before = build(alias('space.md'));
    expect(before.structure).not.toBe(plain.structure);
    expect(readFileSync(resolve(dir, 'app.css'), 'utf8')).toContain('--exact-space-lg:24px');
    // Between two base names: the same structure, in a patch and a compile.
    expect(patch(before, [['card.pad', ['space.lg']]]).structure).toBe(before.structure);
    const edited = resolve(app, 'edited.tamber');
    writeFileSync(edited, alias('space.lg'));
    const read = spawnSync(bin, ['design', resolve(app, 'app.contract'), edited], { encoding: 'utf8' });
    expect(read.status, read.stderr).toBe(0);
    const next = JSON.parse(read.stdout);
    expect([next.shape, next.structure]).toEqual([before.shape, before.structure]);
    const changed = next.tokens.filter((t, i) => JSON.stringify(t) !== JSON.stringify(before.tokens[i]));
    expect(changed.map(t => t.name)).toEqual(['card.pad']);
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<link rel="stylesheet" href="/app.css"><div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
        { headers: { 'content-type': 'text/html' } });
      // A module that re-exports the host's own reaches it through `../web/`.
      const bases = path.startsWith('web/') ? [resolve(root, 'host')] : [dir, import.meta.dir, resolve(root, 'host/web')];
      const file = bases.map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await suiteBrowser();
    const page = await browser.newPage({ viewport: { width: 400, height: 300 } });
    await page.goto(server.url.href);
    const row = page.getByTestId('alias');
    await row.waitFor({ state: 'attached' });
    const padding = () => row.evaluate(n => getComputedStyle(n).paddingTop);
    expect(await padding()).toBe('16px');
    await page.evaluate(() => (window.__kept = 1));
    const batch = { kind: 'tamber-batch', structure: next.shape, base: before.digest, design: next.digest, tokens: changed };
    expect(await page.evaluate(b => globalThis.exact.design(b).live, batch)).toBe('applied');
    expect(await padding()).toBe('24px');
    expect(await page.evaluate(() => window.__kept)).toBe(1);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(app, { recursive: true, force: true });
    rmSync(dir, { recursive: true, force: true });
  }
}, 60000);

// Bound grid tracks resolve the scale in release and development builds.
test('a bound grid track naming a base name or an alias keeps its length', async () => {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  const app = mkdtempSync(resolve(target, 'base-grid-app-'));
  const bin = resolve(target, 'debug/exact-web-js');
  let server, browser;
  const dirs = [];
  try {
    writeFileSync(resolve(app, 'app.tamber'), 'tamber t\n\ntheme\n  card.pad: length = space.lg\n');
    writeFileSync(resolve(app, 'app.contract'), `component App\n  state direct = "space.md"\n  state alias = "card.pad"\n  view\n    column\n      column display="grid" grid-template-columns=direct testId="direct"\n        text "a"\n      column display="grid" grid-template-columns=alias testId="alias"\n        text "b"\n`);
    browser = await suiteBrowser();
    for (const dev of [false, true]) {
      const dir = mkdtempSync(resolve(target, 'base-grid-out-'));
      dirs.push(dir);
      const built = spawnSync(bin, ['js', resolve(app, 'app.contract'), '-o', dir, ...(dev ? ['--dev-reload'] : [])], { encoding: 'utf8' });
      expect(built.status, built.stderr).toBe(0);
      server?.stop(true);
      server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
        const path = new URL(request.url).pathname.slice(1);
        if (!path) return new Response(`<link rel="stylesheet" href="/app.css"><div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
          { headers: { 'content-type': 'text/html' } });
        // A module that re-exports the host's own reaches it through `../web/`.
        const bases = path.startsWith('web/') ? [resolve(root, 'host')] : [dir, import.meta.dir, resolve(root, 'host/web')];
        const file = bases.map(base => resolve(base, path)).find(existsSync);
        return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
      } });
      const page = await browser.newPage({ viewport: { width: 400, height: 300 } });
      await page.goto(server.url.href);
      await page.getByTestId('alias').waitFor({ state: 'attached' });
      const tracks = id => page.getByTestId(id).evaluate(n => [n.style.gridTemplateColumns, getComputedStyle(n).gridTemplateColumns]);
      expect(await tracks('direct'), `dev ${dev}`).toEqual(['var(--exact-space-md)', '16px']);
      // A development build's alias is its own property, whose value is the base's.
      expect(await tracks('alias'), `dev ${dev}`).toEqual([dev ? 'var(--x-card-pad)' : 'var(--exact-space-lg)', '24px']);
      await page.close();
    }
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(app, { recursive: true, force: true });
    for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
  }
}, 60000);
