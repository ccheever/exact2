// A sheet the JS runtime removes slides down first (navigation.js `sheetLeaves`, LLP 1115.000 D4.8): a keyed row that
// is a sheet the look draws stays, leaving, until its animation ends, whether the loop keeps a row beside it or
// replaces all it holds (the runtime's fast path empties such a parent at once). It keeps its ids while it leaves, and
// a focus the same commit asks for by an id it shares lands on the live route's. Build exact-web-js once before
// running this test (docs/testing.md).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';
import { withLook } from '../web/look.mjs';

const SOURCE = `component App
  state stack = ["a", "sheet"]
  action replace
    stack = ["b"]
  action pop
    stack = ["a"]
  action save
    stack = ["b"]
    focus("name")
  view
    column
      button "Replace" press=replace testId="replace"
      button "Pop" press=pop testId="pop"
      button "Save" press=save testId="save"
      column position="relative" height=300
        for r in stack key=r
          column navigationPresentation=(r == "sheet" ? "modal" : "") testId=\`route-\${r}\` position="absolute" top=0 right=0 bottom=0 left=0
            text r
            if r != "a"
              input id="name" testId=\`name-\${r}\` aria-label="Name"
`;

// A field's decorations are its runtime's to clear as the commit ends the field's scope, before the sheet is asked to
// leave: a sheet holding one the browser paints closes at once.
const DECORATED = `decoration mention color="red"

component App
  state stack = ["a", "sheet"]
  derive ranges = TextDecorations(value="hi @al", spans=[TextDecorationSpan(name="mention", start=3, end=6)])
  action pop
    stack = ["a"]
  view
    column
      button "Pop" press=pop testId="pop"
      column position="relative" height=300
        for r in stack key=r
          column navigationPresentation=(r == "sheet" ? "modal" : "") testId=\`route-\${r}\` position="absolute" top=0 right=0 bottom=0 left=0
            text r
            if r != "a"
              textarea testId="field" aria-label="Field" value="hi @al" decorations=ranges width=200 height=60
`;

/** `source` compiled for the JS target and served with the shell's style: `run(url, browser)`. */
async function served(source, run) {
  const root = resolve(import.meta.dir, '../..');
  const target = resolve(root, 'target');
  mkdirSync(target, { recursive: true });
  const dir = mkdtempSync(resolve(target, 'sheet-leave-'));
  const style = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
  let server, browser;
  try {
    writeFileSync(resolve(dir, 'app.contract'), source);
    const compiled = spawnSync(resolve(target, 'debug/exact-web-js'), ['js', resolve(dir, 'app.contract'), '-o', dir], { encoding: 'utf8' });
    expect(compiled.status, compiled.stderr).toBe(0);
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname.slice(1);
      if (!path) return new Response(`<style>${style}</style><link rel="stylesheet" href="/app.css">
        <div id="exact-root"></div><script type="module">import app from '/app.js';app();</script>`,
      { headers: { 'content-type': 'text/html' } });
      const file = [dir, import.meta.dir, resolve(root, 'host/web')].map(base => resolve(base, path)).find(existsSync);
      return file ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
    } });
    browser = await suiteBrowser();
    await run(server.url.href, browser);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}

test('a sheet the JS runtime removes slides down, beside a kept row and when the loop replaces every row', () => served(SOURCE, async (url, browser) => {
  for (const [press, kept] of [['pop', 'route-a'], ['replace', 'route-b'], ['save', 'route-b']]) for (const reducedMotion of ['no-preference', 'reduce']) {
    const context = await browser.newContext({ viewport: { width: 400, height: 700 }, reducedMotion });
    const page = await context.newPage();
    await page.goto(url);
    await page.getByTestId('route-sheet').waitFor({ state: 'attached' });
    // (The sheet covers the page, so the press is the button's own click.)
    await page.evaluate((press) => document.querySelector(`[data-testid="${press}"]`).click(), press);
    await page.getByTestId(kept).waitFor({ state: 'attached' });
    const leaving = await page.evaluate(() => { const s = document.querySelector('[data-testid="route-sheet"]'); return s && [s.hasAttribute('data-exiting'), s.getAnimations().length > 0]; });
    expect([press, reducedMotion, leaving]).toEqual([press, reducedMotion, reducedMotion === 'reduce' ? null : [true, true]]);
    if (press === 'save') expect([reducedMotion, await page.evaluate(() => document.activeElement?.dataset.testid)]).toEqual([reducedMotion, 'name-b']);
    const gone = await page.evaluate(async () => {
      await Promise.all(document.querySelector('[data-testid="route-sheet"]')?.getAnimations().map((a) => (a.finish(), a.finished)) ?? []);
      await new Promise((r) => setTimeout(r));
      return document.querySelector('[data-testid="route-sheet"]') === null;
    });
    expect([press, reducedMotion, gone]).toEqual([press, reducedMotion, true]);
    await context.close();
  }
}), 120000);

test('a sheet the JS runtime removes, holding a field whose decorations the browser paints, closes at once', () => served(DECORATED, async (url, browser) => {
  const page = await (await browser.newContext({ viewport: { width: 400, height: 700 }, reducedMotion: 'no-preference' })).newPage();
  await page.goto(url);
  await page.waitForFunction(() => document.querySelector('[data-testid="field"]')?.exactDecorations?.applied);
  const painted = await page.evaluate(() => document.querySelector('[data-testid="field"]').exactDecorations.painted);
  await page.evaluate(() => document.querySelector('[data-testid="pop"]').click());
  await page.getByTestId('route-a').waitFor({ state: 'attached' });
  expect([painted, await page.evaluate(() => document.querySelector('[data-testid="route-sheet"]') === null)]).toEqual([true, true]);
}), 120000);
