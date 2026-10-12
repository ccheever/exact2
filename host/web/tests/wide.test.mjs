// LLP 1116.004 D1 in a real browser at a phone's width (390) and a desktop's (1280): the shell's stylesheet and the
// navigation glue's projection (navigation.js, which both web targets run) over nodes as the hosts write them. A root
// tab bar whose placement is unsaid is a sidebar on a wide window and a bar on a phone; an arrangement the app wrote
// stays its own. A stack whose root route is a list whose rows push shows that route beside its top on a wide window
// (the list live, its disclosure gone, the row whose press pushed the detail marked, no Back in the detail, and while
// nothing is selected the platform's placeholder in the pane or the author's own) and pushes on a phone;
// `navigationSplit` opts out or names the width; a sheet stays a sheet; a resize changes nothing else. At a
// desktop width `check` advises (D1.3) on a tab bar left along the bottom and on a list that pushes unsplit.
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
// A route of rows (`rows` true: each row draws the disclosure), as the hosts write a `list` of `button`s; `inner` after it.
const list = (id, key, rows = true, extra = '', inner = '') => `<div navigationKey="${key}" id="${id}" style="${abs}" ${extra}><header><h1>People</h1></header>
  <div role="list" id="${id}-list">${['ada', 'bo'].map(n => `<button id="${id}-${n}" data-exact-on="press" style="display:flex"><span data-exact-text>${n}</span>${rows ? chevron : ''}</button>`).join('')}</div>${inner}</div>`;
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
  // The platform's placeholder in a stack's detail pane: its words, its left edge and colour (null where none shows).
  window.placeholder = id => { const e = document.getElementById(id), p = getComputedStyle(e, '::before'); return e.hasAttribute('data-exact-split-empty') && p.content !== 'none' ? [p.content, Math.round(parseFloat(p.left)), p.color] : null; };
  select(document.getElementById('nav').getAttribute('navigationKey'));
  window.ready = true;
</script>`;

/** The page (`html`) in headless Chrome at `w`×800: `test(evaluate, size)`, `size(w)` resizing the viewport. */
async function inChrome(html, test, w = 1280) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(html); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(req.url === '/check.js' ? resolve(WEB, '../web-js/check.js') : resolve(WEB, 'navigation.js')));
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

check('a list whose rows push is beside its detail on a wide window, and pushes on a phone', () => inChrome(page(tabs(BAR)), async (evaluate, size) => {
  // Wide, nothing chosen: the list is a 320-wide column, its disclosure gone; the rest of the stack's width is the
  // platform's placeholder (LLP 1116.006 D2), one line in the secondary label's colour.
  await evaluate(`select('1')`);
  expect(await evaluate(`[document.getElementById('r1').getAttribute('data-exact-split'), box('r1'), getComputedStyle(document.querySelector('#r1 img')).display, shown('r1'), placeholder('p1')]`))
    .toEqual(['list', [240, 0, 320, 800], 'none', true, ['"No Selection"', 320, 'rgba(60, 60, 67, 0.6)']]);
  // A row's press pushes its detail: both show, the list live and the row marked; the detail has no Back.
  await evaluate(`document.getElementById('r1-bo').click(); push('2', '/people/bo')`);
  expect(await evaluate(`[shown('r1'), shown('r2'), box('r2'), document.getElementById('r2').getAttribute('data-exact-split'), document.getElementById('r1-bo').hasAttribute('data-exact-split-row'), back('r2'), off(), placeholder('p1')]`))
    .toEqual([true, true, [560, 0, 720, 800], 'detail', true, false, ['p2', 'r3'], null]);
  // A sheet stays a sheet over both: the whole stack's width, the list and the detail inert beneath it.
  await evaluate(`push('3', '/people/bo/edit')`);
  expect(await evaluate(`[box('r3'), document.getElementById('r3').hasAttribute('data-exact-split'), shown('r1'), shown('r2'), document.getElementById('r1').hasAttribute('data-exact-covered'), off()]`))
    .toEqual([[240, 0, 1040, 800], false, false, false, true, ['p2', 'r1', 'r2']]);
  // A phone: the pushed stack, the list hidden and inert, the detail the whole width with Back; then wide again.
  await evaluate(`select('2')`);
  await size(390);
  expect(await evaluate(`[shown('r1'), shown('r2'), box('r2')[2], box('r2')[3] + box('bar')[3], document.getElementById('r1').hasAttribute('data-exact-split'), back('r2'), getComputedStyle(document.querySelector('#r1 img')).display, off()]`))
    .toEqual([false, true, 390, 800, false, true, 'block', ['p2', 'r1', 'r3']]);
  // A phone's list, nothing chosen, is the whole width: no placeholder.
  await evaluate(`select('1')`);
  expect(await evaluate(`[shown('r1'), box('r1')[2], placeholder('p1')]`)).toEqual([true, 390, null]);
  await evaluate(`select('2')`);
  await size(1280);
  expect(await evaluate(`[shown('r1'), shown('r2'), box('r1')[2], document.getElementById('r1-bo').hasAttribute('data-exact-split-row')]`)).toEqual([true, true, 320, true]);
  // The other tab's list draws no disclosure: it pushes, unsplit, with no placeholder.
  expect(await evaluate(`[document.getElementById('t1').hasAttribute('data-exact-split'), placeholder('p2')]`)).toEqual([false, null]);
  // A row that is a link (LLP 1115.000 D4.4) is followed from the document's capture phase (input-glue.js, router.js),
  // so its commit lands within the click, before a listener nearer the row would hear it: its row is marked all the same.
  await evaluate(`select('1'); document.addEventListener('click', e => { if (e.target.closest('#r1-ada')) push('2', '/people/ada'); }, true); document.getElementById('r1-ada').click()`);
  expect(await evaluate(`[shown('r2'), document.getElementById('r1-ada').hasAttribute('data-exact-split-row'), document.getElementById('r1-bo').hasAttribute('data-exact-split-row')]`)).toEqual([true, true, false]);
}), 30000);

check('the author\'s placeholder in the root route takes the detail pane while nothing is selected, and shows nowhere else', () => {
  // As the lowerer writes it: `display: none` but where the split shows it (LLP 1116.006 D2).
  const own = '<div navigationSplit="placeholder" id="own" style="display:none;gap:8px"><span id="own-text">Pick a person</span><button id="own-add">Add</button></div>';
  return inChrome(page(`<main id="nav" navigationKey="1" navigationBack="back" style="width:100%;height:100%;position:relative">${list('r1', 1, true, '', own)}${detail('r2', 2)}</main>`), async (evaluate, size) => {
    const hit = `(e => document.elementFromPoint(box(e)[0] + box(e)[2] / 2, box(e)[1] + box(e)[3] / 2)?.id)`;
    // Wide, nothing chosen: the author's box is the pane beside the list, its content centred and pressable; no host line.
    expect(await evaluate(`[box('r1'), box('own'), shown('own'), Math.round(box('own-text')[0] + box('own-text')[2] / 2), ${hit}('own-add'), placeholder('nav'), document.getElementById('nav').hasAttribute('data-exact-split-empty')]`))
      .toEqual([[0, 0, 320, 800], [320, 0, 960, 800], true, 800, 'own-add', null, false]);
    // A narrower wide window: the pane follows.
    await size(1000);
    expect(await evaluate(`box('own')`)).toEqual([320, 0, 680, 800]);
    // A detail shown: the author's box is gone; back at the list, it returns.
    await evaluate(`document.getElementById('r1-bo').click(); push('2', '/people/bo')`);
    expect(await evaluate(`[box('own'), getComputedStyle(document.getElementById('own')).display, box('r2')]`)).toEqual([[0, 0, 0, 0], 'none', [320, 0, 680, 800]]);
    await evaluate(`push('1', '/')`);
    expect(await evaluate(`box('own')`)).toEqual([320, 0, 680, 800]);
    // A phone: the list alone, the author's box nowhere.
    await size(390);
    expect(await evaluate(`[box('r1'), getComputedStyle(document.getElementById('own')).display, placeholder('nav')]`)).toEqual([[0, 0, 390, 800], 'none', null]);
  });
}, 30000);

check('navigationSplit opts a stack out, or names the width of a list the rule does not find', async () => {
  const stack = (extra, rows) => `<main id="nav" navigationKey="2" navigationBack="back" style="width:100%;height:100%;position:relative">${list('r1', 1, rows, extra)}${detail('r2', 2)}</main>`;
  await inChrome(page(stack('navigationSplit="none"', true)), async (evaluate) => {
    expect(await evaluate(`[shown('r1'), document.getElementById('r1').hasAttribute('data-exact-split'), box('r2')]`)).toEqual([false, false, [0, 0, 1280, 800]]);
  });
  await inChrome(page(stack('navigationSplit="280"', false)), async (evaluate) => {
    expect(await evaluate(`[shown('r1'), box('r1'), box('r2'), back('r2')]`)).toEqual([true, [0, 0, 280, 800], [280, 0, 1000, 800], false]);
  });
}, 60000);

check('check advises on a bottom tab bar and an unsplit pushing list at a desktop width, and on neither at a phone\'s', () => {
  const positioned = BAR.replace('style="display:flex;flex-direction:row"', 'style="display:flex;flex-direction:row;position:absolute;left:0;right:0;bottom:0"');
  const html = page(tabs(positioned).replaceAll(chevron, '')) + `<script type="module">
    import { observe } from './check.js';
    window.advice = () => { const views = new Map([...document.querySelectorAll('#exact-root *')].map(e => [e.id || e.localName, e])); return (observe(views).advice ?? []).map(a => a.kind + ' ' + a.id); };
  </script>`;
  return inChrome(html, async (evaluate, size) => {
    await evaluate(`select('1')`);
    expect(await evaluate('advice()')).toEqual(['tabbar bar']);
    await evaluate(`select('2')`);
    expect(await evaluate('advice()')).toEqual(['tabbar bar', 'push r2']);
    await size(390);
    expect(await evaluate('advice()')).toEqual([]);
  });
}, 30000);

check('the split and sidebar leave check nothing to advise', () => inChrome(page(tabs(BAR)) + `<script type="module">
    import { observe } from './check.js';
    window.advice = () => { const views = new Map([...document.querySelectorAll('#exact-root *')].map(e => [e.id || e.localName, e])); return observe(views).advice ?? []; };
  </script>`, async (evaluate) => {
  expect(await evaluate(`select('2'); advice()`)).toEqual([]);
}), 30000);

// LLP 1116.005 D3: each route keeps its own scroll (LLP 1038 §7: mounted rows keep theirs), on a phone and through a
// split. A route of `n` tall rows over its own scroller; `rows` draws the disclosure, so a stack it roots splits.
const tall = (id, key, n, rows = false) => `<div navigationKey="${key}" id="${id}" style="${abs}"><header><h2>${id}</h2></header><div id="${id}-scroll" style="flex:1 1 0%;min-height:0;overflow:auto">${
  Array.from({ length: n }, (_, i) => rows ? `<button data-exact-on="press" style="display:flex;height:50px"><span data-exact-text>${i}</span>${chevron}</button>` : `<div style="height:50px">${i}</div>`).join('')}</div></div>`;

check('a pushed route opens at its top, Back finds the list where it was, and a detail another row replaces starts at its top', () => inChrome(page(`<main id="nav" navigationKey="1" navigationBack="back" style="width:100%;height:100%;position:relative">${tall('r1', 1, 60, true)}</main>`), async (evaluate, size) => {
  // The renderer's rows: a push adds the route's row, a pop removes it, a `replace` keeps it (its key is the visit's id).
  const enter = (key, url) => `document.getElementById('nav').insertAdjacentHTML('beforeend', ${JSON.stringify(tall(`r${key}`, key, 40))}); push('${key}', '${url}')`;
  const pop = `document.getElementById('r2').remove(); push('1', '/')`;
  const tops = `[...document.querySelectorAll('[id$="-scroll"]')].map(e => Math.round(e.scrollTop))`;
  await size(390);
  // A phone: the list scrolled, a detail pushed over it opens at its top; Back shows the list at its offset.
  await evaluate(`document.getElementById('r1-scroll').scrollTop = 1500`);
  await evaluate(enter(2, '/item/30'));
  expect(await evaluate(`[shown('r1'), shown('r2'), ${tops}]`)).toEqual([false, true, [1500, 0]]);
  await evaluate(`document.getElementById('r2-scroll').scrollTop = 600; ${pop}`);
  expect(await evaluate(`[shown('r1'), ${tops}]`)).toEqual([true, [1500]]);
  // Wide: the list column keeps its offset beside a detail that opens at its top.
  await size(1280);
  await evaluate(enter(2, '/item/30'));
  expect(await evaluate(`[shown('r1'), shown('r2'), document.getElementById('r1').getAttribute('data-exact-split'), ${tops}]`)).toEqual([true, true, 'list', [1500, 0]]);
  // Another row's detail `replace`d in (the recipe's swap: the same visit, so the same node) starts at its top...
  await evaluate(`document.getElementById('r2-scroll').scrollTop = 600; push('2', '/item/31')`);
  expect(await evaluate(tops)).toEqual([1500, 0]);
  // ...and a new query on the same path (a search mirrored into the URL) keeps its place.
  await evaluate(`document.getElementById('r2-scroll').scrollTop = 300; push('2', '/item/31?q=salt')`);
  expect(await evaluate(tops)).toEqual([1500, 300]);
  await evaluate(pop);
  expect(await evaluate(`[shown('r1'), ${tops}]`)).toEqual([true, [1500]]);
}), 30000);
