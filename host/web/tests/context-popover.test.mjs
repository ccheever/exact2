// A context menu's popover (LLP 1021 §5.1; LLP 1115.000 D4.7) in each engine: the navigation glue's
// `contextAt`, which both web runtimes open one through (rt.js `cp`, glue.js `contextpopover`).
//
// On a Mac the `contextmenu` event comes with the button still down. A popover shown then is closed by the
// release that follows in Firefox and WebKit: it is a pointer released outside a popover, their light
// dismiss. So a right click opened no menu there. The glue opens it once that press is over, and leaves
// where the press was on the popover for a look's sheet to place it by (the neutral one opens it there,
// which neutral-look.test.mjs holds; LLP 1116 D1's reads neither, and its menu stays where the user agent
// puts a popover). The page's stylesheet is a default build's (look.mjs).
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';

const read = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');
const page = `<style>${withLook(read('../index.html')).match(/<style>([\s\S]*?)<\/style>/)[1]}</style>
<div id="exact-root"><div id="row" style="height:60px">A row</div><div id="gone" style="height:60px">A row whose menu is gone</div>
  <div popover="auto" role="menu" id="menu"><button type="button" id="item">Rename</button></div></div>
<script type="module">${read('../navigation.js')}
  const menu = document.getElementById('menu');
  window.toggles = [];
  menu.addEventListener('toggle', (event) => toggles.push(event.newState));
  document.getElementById('row').addEventListener('contextmenu', (event) => { event.preventDefault(); contextAt(event, () => [menu, event.target]); });
  // What the runtimes answer when the node left, was disabled or names no popover by then: nothing to open.
  document.getElementById('gone').addEventListener('contextmenu', (event) => { event.preventDefault(); contextAt(event, () => undefined); });
  window.ready = true;
</script>`;

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a right click opens the context menu's popover, once the press is over, and says where it was`, async () => {
  const playwright = await import('playwright-core');
  const browser = await suiteBrowser(engine);
  try {
    const tab = await browser.newPage({ viewport: { width: 600, height: 400 } });
    await tab.setContent(page);
    await tab.waitForFunction('window.ready === true');
    const open = () => tab.evaluate(() => document.getElementById('menu').matches(':popover-open'));
    // The button goes down and the event comes: nothing shows while it is held.
    await tab.mouse.move(120, 30);
    await tab.mouse.down({ button: 'right' });
    if (await tab.evaluate(() => navigator.platform.startsWith('Mac'))) expect([engine, await open()]).toEqual([engine, false]);
    await tab.mouse.up({ button: 'right' });
    await tab.waitForFunction(() => document.getElementById('menu').matches(':popover-open'));
    // Open once, after the release, which therefore dismissed nothing. The press's point is on the popover.
    expect([engine, await open(), await tab.evaluate(() => [toggles.splice(0), document.getElementById('menu').style.getPropertyValue('--exact-context-x'), document.getElementById('menu').style.getPropertyValue('--exact-context-y')])])
      .toEqual([engine, true, [['open'], '120px', '30px']]);
    // Closed, the point is gone, so a button that opens the same popover finds none.
    await tab.keyboard.press('Escape');
    await tab.waitForFunction(() => toggles.includes('closed'));
    expect([engine, await tab.evaluate(() => document.getElementById('menu').getAttribute('style') ?? '')]).toEqual([engine, '']);
    // Nothing to open: nothing opens, and nothing is left listening for a release.
    await tab.mouse.click(120, 90, { button: 'right' });
    await tab.evaluate(() => new Promise((done) => setTimeout(done))); // a task after the one the release queued
    expect([engine, await open()]).toEqual([engine, false]);
  } finally { await browser.close(); }
}, 30000);

// A context menu asked for inside an open popover nests in it (HTML, "topmost popover ancestor": the
// source's nearest open popover), so the first stays open under it, and closing the menu leaves it open.
const nested = `<style>${withLook(read('../index.html')).match(/<style>([\s\S]*?)<\/style>/)[1]}</style>
<div id="exact-root"><button id="open" popovertarget="list">Open</button>
  <div popover="auto" id="list"><div id="inner" style="height:60px;width:200px">A row in a popover</div></div>
  <div popover="auto" role="menu" id="menu"><button type="button" id="item">Rename</button></div></div>
<script type="module">${read('../navigation.js')}
  const menu = document.getElementById('menu');
  document.getElementById('inner').addEventListener('contextmenu', (event) => { event.preventDefault(); contextAt(event, () => [menu, event.currentTarget ?? document.getElementById('inner')]); });
  window.ready = true;
</script>`;

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a context menu inside an open popover leaves that popover open`, async () => {
  const browser = await suiteBrowser(engine);
  try {
    const tab = await browser.newPage({ viewport: { width: 600, height: 400 } });
    await tab.setContent(nested);
    await tab.waitForFunction('window.ready === true');
    const open = () => tab.evaluate(() => ['list', 'menu'].map((id) => document.getElementById(id).matches(':popover-open')));
    await tab.click('#open');
    expect([engine, await open()]).toEqual([engine, [true, false]]);
    const box = await tab.locator('#inner').boundingBox();
    await tab.mouse.click(box.x + 20, box.y + 20, { button: 'right' });
    await tab.waitForFunction(() => document.getElementById('menu').matches(':popover-open'));
    expect([engine, await open()]).toEqual([engine, [true, true]]);
    await tab.keyboard.press('Escape');
    await tab.waitForFunction(() => !document.getElementById('menu').matches(':popover-open'));
    expect([engine, await open()]).toEqual([engine, [true, false]]);
  } finally { await browser.close(); }
}, 30000);
