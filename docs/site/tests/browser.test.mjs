import { afterAll, beforeAll, expect, test } from 'bun:test';
import { existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { serveBuildTree } from '../../../host/web/serve.mjs';

const dist = resolve(import.meta.dir, '../dist');
let browser, server, origin;

beforeAll(async () => {
  if (!existsSync(resolve(dist, 'llms.txt'))) throw new Error('run bun run build in docs/site before the browser tests');
  server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  origin = `http://127.0.0.1:${server.address().port}`;
  browser = await suiteBrowser();
});

afterAll(async () => {
  await browser?.close();
  server?.closeAllConnections();
  await new Promise(resolve => server ? server.close(resolve) : resolve());
});

async function visit(page, path, interactive = true) {
  await page.goto(origin + path);
  if (interactive) await page.waitForSelector('#exact-root[data-boot-ms]');
  await page.evaluate(() => document.fonts.ready);
}

async function withPage(options, run) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, ...options });
  context.setDefaultTimeout(5000);
  try { await run(await context.newPage()); } finally { await context.close(); }
}

async function headingInView(page) {
  await page.waitForFunction(() => {
    const box = document.querySelector('[data-testid="h-a-deep-link"]').getBoundingClientRect();
    return box.top >= 0 && box.bottom <= innerHeight;
  }, null, { timeout: 5000 });
}

test('a table-of-contents link scrolls the article to its heading', () => withPage({}, async page => {
  await visit(page, '/recipes/website');
  const before = await page.getByTestId('h-a-deep-link').boundingBox();
  expect(before.y).toBeGreaterThan(900);
  await page.locator('[data-testid="toc"] a[href="#a-deep-link"]').click();
  await headingInView(page);
  expect(new URL(page.url()).hash).toBe('#a-deep-link');
}));

test('a fragment survives initial page activation', () => withPage({}, async page => {
  await page.clock.install();
  await visit(page, '/recipes/website#a-deep-link');
  await page.clock.runFor(350);
  await headingInView(page);
}));

test('the sidebar can be shown after a full reload with it hidden', () => withPage({}, async page => {
  await visit(page, '/start');
  await page.getByTestId('sidebar-hide').click();
  await page.waitForFunction(() => localStorage.getItem('exact.state.sidebar') === 'false');
  await page.reload();
  await page.waitForSelector('#exact-root[data-boot-ms]');
  await page.getByTestId('sidebar-show').click();
  await page.waitForFunction(() => {
    const sidebar = document.querySelector('[data-testid="sidebar-shown"]');
    return sidebar && getComputedStyle(sidebar).visibility === 'visible' && sidebar.getBoundingClientRect().width > 200;
  }, null, { timeout: 5000 });
  await page.getByTestId('nav-recipeWebsite').click();
  await page.getByTestId('h-a-deep-link').waitFor();
  expect(new URL(page.url()).pathname).toBe('/recipes/website');
}));

for (const interactive of [false, true]) {
  for (const width of [959, 960, 961, 1279, 1280, 1281]) {
    test(`navigation and page actions are visible at ${width}px ${interactive ? 'after activation' : 'before JavaScript'}`, () =>
      withPage({ viewport: { width, height: 900 }, javaScriptEnabled: interactive }, async page => {
        await visit(page, '/recipes/website', interactive);
        const nav = page.getByTestId(width >= 960 ? 'sidebar-shown' : 'menu');
        const actions = page.getByTestId(width >= 1280 ? 'toc' : 'page-actions');
        for (const control of [nav, actions]) {
          const box = await control.boundingBox();
          expect(box?.width).toBeGreaterThan(30);
          expect(box?.height).toBeGreaterThan(20);
          if (interactive) expect(await control.evaluate(e => !!e.closest('[inert]'))).toBe(false);
        }
        if (interactive && width < 960) {
          await nav.click();
          await page.getByTestId('menu-nav-startHere').click();
          expect(new URL(page.url()).pathname).toBe('/start');
        }
      }));
  }
}

test('copying delivers the onboarding prompt and the current article after navigation', () => withPage({}, async page => {
  await page.addInitScript(() => {
    window.clipboardWrites = [];
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: text => { window.clipboardWrites.push(text); return Promise.resolve(); } },
    });
  });
  await visit(page, '/start');
  await page.getByTestId('agent-prompt-copy').click();
  await page.waitForFunction(() => window.clipboardWrites.length === 1);
  const prompt = await page.evaluate(() => window.clipboardWrites[0]);
  expect(prompt).toContain('Exact');
  expect(prompt).toContain('setup --check');
  expect(prompt).toContain('feedback note');

  await page.getByTestId('copy-markdown').click();
  await page.waitForFunction(() => window.clipboardWrites.length === 2);
  expect(await page.evaluate(() => window.clipboardWrites[1])).toStartWith('# Start here: building an app with Exact');
  await page.getByTestId('nav-recipeWebsite').click();
  await page.getByTestId('h-a-deep-link').waitFor();
  await page.getByTestId('copy-markdown').click();
  await page.waitForFunction(() => window.clipboardWrites.length === 3);
  const markdown = await page.evaluate(() => window.clipboardWrites[2]);
  expect(markdown).toStartWith('# Recipe: a website');
  expect(markdown).toContain('## A deep link');
  expect(markdown).not.toContain('# Start here: building an app with Exact');
}));
