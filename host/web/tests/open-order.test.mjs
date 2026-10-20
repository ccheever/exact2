// The order dialogs and popovers opened in (navigation.js stamps each `exactOpened`), which the runtime's structural
// reads (`modalLayers`) and the menu on top (input-glue.js) go by, holds when two copies of navigation.js run, as the JS
// target's agent loads its own (`agent-navigation.js`) after the page has opened things: one copy stamps each opening.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

const navigation = readFileSync(new URL('../navigation.js', import.meta.url), 'utf8');
const page = `<!doctype html><dialog id="first"><button>Close</button></dialog><dialog id="sheet"><button popovertarget="menu">More</button><div id="menu" popover="auto" role="menu"><button>Rename</button></div></dialog>`;

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: the open order holds with a second copy of navigation.js`, async () => {
  const server = Bun.serve({ port: 0, fetch: (req) => {
    const path = new URL(req.url).pathname;
    if (path === '/') return new Response(page, { headers: { 'content-type': 'text/html' } });
    if (['/navigation.js', '/agent-navigation.js'].includes(path)) return new Response(navigation, { headers: { 'content-type': 'text/javascript' } });
    return new Response('', { status: 404 });
  } });
  const browser = await suiteBrowser(engine);
  try {
    const tab = await (await browser.newContext()).newPage();
    await tab.goto(`http://127.0.0.1:${server.port}/`);
    // A dialog opened and closed, then the one the app stays in, before the second copy loads; then a menu in it.
    expect([engine, await tab.evaluate(async () => {
      const { modalLayers } = await import('/navigation.js');
      first.showModal(); first.close(); sheet.showModal();
      await import('/agent-navigation.js');
      menu.showPopover();
      return [modalLayers()?.map(el => el.id), menu.exactOpened > sheet.exactOpened];
    })]).toEqual([engine, [['sheet', 'menu'], true]]);
  } finally { await browser.close(); server.stop(true); }
}, 60000);
