import { test, expect } from 'bun:test';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

test('a data-word read includes literal and bound attributes in tree order', async () => {
  // The build stages these modules together; serve that same layout to a real DOM.
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/dataset.js') return new Response(Bun.file(new URL('../../web-js/dataset.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../navigation.js', import.meta.url)));
    return new Response(`<main id="exact-root">
      <button id="save" data-paint="accent">Save</button>
      <div id="row-0"></div><div id="row-1"></div><div id="row-2"></div>
    </main>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    const snapshots = await page.evaluate(async () => {
      const { ds, x_querySelectorAll } = await import('/dataset.js');
      const el = id => document.getElementById(id), read = () => x_querySelectorAll('[data-paint~="accent"]');
      ds(el('row-0'), '{"paint":"accent primary"}');
      ds(el('row-1'), '{"paint":"accented"}');
      ds(el('row-2'), '{"paint":"secondary accent"}');
      const initial = read();
      ds(el('row-0'), '{}');
      ds(el('row-1'), '{"paint":"accent"}');
      return { initial, updated: read(), invalid: x_querySelectorAll('button') };
    });
    expect(snapshots).toEqual({ initial: ['save', 'row-0', 'row-2'], updated: ['save', 'row-1', 'row-2'], invalid: [] });
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);
