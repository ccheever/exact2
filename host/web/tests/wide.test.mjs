// LLP 1116.004 D1 in a real browser at a phone's width (390) and a desktop's (1280): the shell's stylesheet and the
// navigation glue's projection (navigation.js, which both web targets run) over nodes as the hosts write them. A root
// tab bar whose placement is unsaid is a sidebar on a wide window and a bar on a phone; an arrangement the app wrote
// stays its own; a resize changes nothing else.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;

const style = readFileSync(resolve(WEB, 'index.html'), 'utf8').match(/<style>[\s\S]*?<\/style>/)[0];
const chevron = '<img data-symbol-source="symbol:forward-chevron" alt="" style="width:17px;height:17px">';
const abs = 'position:absolute;inset:0;display:flex;flex-direction:column';
// A route of rows (`rows` true: each row draws the disclosure), as the hosts write a `list` of `button`s.
const list = (id, key, rows = true, extra = '') => `<div navigationKey="${key}" id="${id}" style="${abs}" ${extra}><header><h1>People</h1></header>
  <div role="list" id="${id}-list">${['ada', 'bo'].map(n => `<button id="${id}-${n}" data-exact-on="press" style="display:flex"><span data-exact-text>${n}</span>${rows ? chevron : ''}</button>`).join('')}</div></div>`;
const detail = (id, key, extra = '') => `<div navigationKey="${key}" id="${id}" style="${abs}" ${extra}><header><h2>${id}</h2></header><div>${id}</div></div>`;
const tabs = (bar) => `<main id="nav" navigationKey="2" navigationBack="back" style="width:100%;height:100%;display:flex;flex-direction:column;position:relative">
    <div id="panels" style="flex:1 1 0%;min-height:0;position:relative">
      <div role="tabpanel" id="p1" style="${abs}">${list('r1', 1)}${detail('r2', 2)}${detail('r3', 3, 'navigationPresentation="modal"')}</div>
      <div role="tabpanel" id="p2" style="${abs}">${list('t1', 10, false)}${detail('t2', 11)}</div>
    </div>
    ${bar}
  </main>`;
const BAR = `<div role="tablist" id="bar" style="display:flex;flex-direction:row"><button role="tab" id="tab1" aria-controls="p1" aria-selected="true" data-button-style="bordered" data-native style="flex:1 1 0%"><img alt=""><span data-exact-text>People</span></button><button role="tab" id="tab2" aria-controls="p2" aria-selected="false" data-button-style="bordered" data-native style="flex:1 1 0%"><img alt=""><span data-exact-text>Saved</span></button></div>`;
const page = (body) => style + `<div id="exact-root">${body}</div>
<script type="module">
  import { navigation, unselected } from './navigation.js';
  const root = document.getElementById('exact-root');
  window.calls = [];
  navigation.connect(root, () => false, line => calls.push(line), id => calls.push('host back ' + id));
  window.select = key => { document.getElementById('nav').setAttribute('navigationKey', key); navigation.project(root, () => {}); };
  window.push = (key, url) => { navigation.apply({ top: Number(key), url, removed: [] }); select(key); };
  window.box = id => { const b = document.getElementById(id).getBoundingClientRect(); return [b.x, b.y, b.width, b.height].map(Math.round); };
  window.shown = id => { const e = document.getElementById(id); return getComputedStyle(e).visibility === 'visible' && !e.closest('[inert]'); };
  window.off = () => unselected(document.getElementById('nav')).map(e => e.id);
  window.back = id => { const b = document.querySelector('#' + id + ' > header')?.shadowRoot?.querySelector('button'); return !!b && !b.hidden; };
  select(document.getElementById('nav').getAttribute('navigationKey'));
  window.ready = true;
</script>`;

/** The page (`html`) in headless Chrome at `w`×800: `test(evaluate, size)`, `size(w)` resizing the viewport. */
async function inChrome(html, test, w = 1280) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(html); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'navigation.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-wide-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=1400,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    const size = async (width) => {
      await call('Emulation.setDeviceMetricsOverride', { width, height: 800, deviceScaleFactor: 1, mobile: false });
      await evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    };
    await size(w);
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    await test(evaluate, size);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}

check('a root tab bar left unsaid is a sidebar on a wide window and a bar on a phone, across a resize', () => inChrome(page(tabs(BAR)), async (evaluate, size) => {
  // Wide: the root's column turns; the bar is a 240-wide column at the leading edge, the panels beside it, the tabs rows.
  expect(await evaluate(`[document.getElementById('nav').hasAttribute('data-exact-sidebar'), box('bar'), box('panels'), box('tab1')[3] < 60, box('tab2')[1] > box('tab1')[1]]`))
    .toEqual([true, [0, 0, 240, 800], [240, 0, 1040, 800], true, true]);
  // The selected tab is in the accent's tint; the tablist and its tabs keep their roles.
  expect(await evaluate(`[getComputedStyle(document.getElementById('tab1')).backgroundColor !== getComputedStyle(document.getElementById('tab2')).backgroundColor, document.getElementById('bar').getAttribute('role')]`)).toEqual([true, 'tablist']);
  await size(390);
  expect(await evaluate(`[document.getElementById('nav').hasAttribute('data-exact-sidebar'), box('bar')[0], box('bar')[2], box('bar')[1] + box('bar')[3], box('panels')[2]]`)).toEqual([false, 0, 390, 800, 390]);
  await size(1280);
  expect(await evaluate(`[document.getElementById('nav').hasAttribute('data-exact-sidebar'), box('bar')]`)).toEqual([true, [0, 0, 240, 800]]);
}), 30000);

check('an arrangement the app wrote stays its own on a wide window', async () => {
  // A vertical tablist (a rail), a bar it positions itself, and a root that is no flex column: none is turned.
  const vertical = BAR.replace('role="tablist"', 'role="tablist" aria-orientation="vertical"');
  const positioned = BAR.replace('style="display:flex;flex-direction:row"', 'style="display:flex;flex-direction:row;position:absolute;left:0;right:0;bottom:0"');
  for (const html of [tabs(vertical), tabs(positioned), tabs(BAR).replace('display:flex;flex-direction:column;position:relative', 'display:block;position:relative')]) {
    await inChrome(page(html), async (evaluate) => {
      expect(await evaluate(`document.getElementById('nav').hasAttribute('data-exact-sidebar')`)).toBe(false);
    });
  }
}, 60000);
