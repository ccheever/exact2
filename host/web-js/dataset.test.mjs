import { test, expect } from 'bun:test';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../scripts/agent-launch.mjs';

test('structural selectors retain the front modal and later popovers, with modal inert escape', async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/dataset.js') return new Response(Bun.file(new URL('./dataset.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../web/navigation.js', import.meta.url)));
    return new Response(`<main id="exact-root">
      <div id="page" data-mood="accent"></div>
      <div id="ancestor">
        <dialog id="upper" data-mood="accent">
          <div id="upper-child" data-mood="accent"></div>
          <div inert><div id="inert-child" data-mood="accent"></div></div>
          <div id="inner-pop" popover="manual" data-mood="accent"></div>
          <div style="display:none"><div id="hidden" data-mood="accent"></div></div>
          <div data-exiting><div id="exiting" data-mood="accent"></div></div>
        </dialog>
      </div>
      <dialog id="lower" data-mood="accent"></dialog>
      <div id="outer-pop" popover="manual" data-mood="accent"></div>
      <div inert><dialog id="popover-dialog" popover="manual" data-mood="accent"></dialog></div>
    </main><script type="module">
      import { x_querySelectorAll } from '/dataset.js';
      window.read = () => x_querySelectorAll('[data-mood~="accent"]');
    </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => !!window.read);
    const snapshots = await page.evaluate(() => {
      const el = id => document.getElementById(id), snapshots = { page: read() };
      el('lower').showModal();
      snapshots.lower = read();
      el('outer-pop').showPopover();
      snapshots.aboveLower = read();
      // Opening order differs from tree order; reads stay in tree order.
      el('upper').showModal();
      snapshots.upper = read();
      el('inner-pop').showPopover();
      snapshots.aboveUpper = read();
      el('ancestor').inert = true;
      snapshots.escape = read();
      el('inner-pop').inert = true;
      snapshots.ownInert = read();
      el('inner-pop').inert = false;
      el('upper').inert = true;
      snapshots.inertModal = read();
      el('upper').inert = false;
      el('outer-pop').hidePopover();
      el('outer-pop').showPopover();
      snapshots.reopened = read();
      el('popover-dialog').showPopover();
      snapshots.popoverDoesNotEscape = read();
      el('upper').close();
      snapshots.closedUpper = read();
      el('lower').close();
      snapshots.closedAll = read();
      el('lower').addEventListener('beforetoggle', e => e.preventDefault(), { once: true });
      el('lower').showModal();
      snapshots.cancelled = read();
      el('outer-pop').remove();
      snapshots.removed = read();
      return snapshots;
    });
    expect(snapshots).toEqual({
      page: ['page'], lower: ['lower'], aboveLower: ['lower', 'outer-pop'],
      upper: ['upper', 'upper-child'], aboveUpper: ['upper', 'upper-child', 'inner-pop'],
      escape: ['upper', 'upper-child', 'inner-pop'], ownInert: ['upper', 'upper-child'], inertModal: [],
      reopened: ['upper', 'upper-child', 'inner-pop', 'outer-pop'],
      popoverDoesNotEscape: ['upper', 'upper-child', 'inner-pop', 'outer-pop'],
      closedUpper: ['lower', 'outer-pop'], closedAll: ['page', 'outer-pop'],
      cancelled: ['page', 'outer-pop'], removed: ['page'],
    });
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);

test('unobserved modals rank below observed openings and auto popovers replace their siblings', async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/dataset.js') return new Response(Bun.file(new URL('./dataset.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../web/navigation.js', import.meta.url)));
    return new Response(`<main id="exact-root">
      <div id="page" data-mood="accent"></div>
      <dialog id="d" data-mood="accent"></dialog>
      <dialog id="later" data-mood="accent"></dialog>
      <div id="p" popover="auto" data-mood="accent"></div>
      <div id="q" popover="auto" data-mood="accent"></div>
    </main>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    const results = {};
    for (const scenario of ['late-import', 'late-import-unread', 'auto', 'auto-reopened']) {
      await page.goto(server.url.href);
      results[scenario] = await page.evaluate(async scenario => {
        const el = id => document.getElementById(id), snapshots = [];
        if (scenario.startsWith('late-import')) el('d').showModal();
        const { x_querySelectorAll } = await import('/dataset.js');
        const read = () => x_querySelectorAll('[data-mood~="accent"]');
        if (scenario.startsWith('late-import')) {
          if (scenario === 'late-import') snapshots.push(read());
          el('later').showModal();
        } else {
          if (scenario === 'auto-reopened') { el('p').showPopover(); el('p').hidePopover(); }
          el('d').showModal();
          el('q').showPopover();
          el('p').showPopover();
        }
        snapshots.push(read());
        return snapshots;
      }, scenario);
    }
    expect(results).toEqual({
      'late-import': [['d'], ['later']],
      'late-import-unread': [['later']],
      auto: [['d', 'p']],
      'auto-reopened': [['d', 'p']],
    });
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);
