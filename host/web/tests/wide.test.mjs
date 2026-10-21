// LLP 1116.004 D1 in a real browser at a phone's width (390) and a desktop's (1280): the shell's stylesheet and the
// navigation glue's projection (navigation.js, which both web targets run) over nodes as the hosts write them. A root
// tab bar whose placement is unsaid is a sidebar on a wide window and a bar on a phone; an arrangement the app wrote
// stays its own. A stack whose root route is a list whose rows push shows that route beside its top on a wide window
// (the list live, its disclosure gone, the row whose press pushed the detail marked, no Back in the detail, and while
// nothing is selected the platform's placeholder in the pane or the author's own) and pushes on a phone;
// `navigationSplit` opts out or names the width; a sheet stays a sheet; a resize changes nothing else. At a
// desktop width `check` advises (D1.3) on a tab bar left along the bottom and on a list that pushes unsplit.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { cdpBrowser, chromium } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { readyPage } from './page.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { unavailable } = chromium();
const check = need('chrome', unavailable);

const style = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>[\s\S]*?<\/style>/)[0];
const chevron = '<img data-symbol-source="symbol:forward-chevron" alt="" style="width:17px;height:17px">';
const abs = 'position:absolute;inset:0;display:flex;flex-direction:column';
// A route of rows (`rows` true: each row writes the router and draws the disclosure), as the hosts write a `list` of `button`s; `inner` after it.
const list = (id, key, rows = true, extra = '', inner = '') => `<div navigationKey="${key}" id="${id}" style="${abs}" ${extra}><header><h1>People</h1></header>
  <div data-exact-list role="list" id="${id}-list">${['ada', 'bo'].map(n => `<button id="${id}-${n}" data-exact-on="press" data-exact-router-press="${rows}" style="display:flex"><span data-exact-text>${n}</span>${rows ? chevron : ''}</button>`).join('')}</div>${inner}</div>`;
const detail = (id, key, extra = '') => `<div navigationKey="${key}" id="${id}" style="${abs}" ${extra}><header><h2>${id}</h2></header><div>${id}</div></div>`;
const tabs = (bar) => `<main id="nav" navigationKey="2" navigationBack="back" style="width:100%;height:100%;display:flex;flex-direction:column;position:relative">
    <div id="panels" style="flex:1 1 0%;min-height:0;position:relative">
      <div role="tabpanel" id="p1" style="${abs}">${list('r1', 1)}${detail('r2', 2)}${detail('r3', 3, 'navigationPresentation="modal"')}</div>
      <div role="tabpanel" id="p2" style="${abs}">${list('t1', 10, false)}${detail('t2', 11)}</div>
    </div>
    ${bar}
  </main>`;
const BAR = `<div role="tablist" id="bar" style="display:flex;flex-direction:row"><button role="tab" id="tab1" aria-controls="p1" aria-selected="true" data-button-style="bordered" data-native style="flex:1 1 0%"><img alt=""><span data-exact-text>People</span></button><button role="tab" id="tab2" aria-controls="p2" aria-selected="false" data-button-style="bordered" data-native style="flex:1 1 0%"><img alt=""><span data-exact-text>Saved</span></button></div>`;
const page = (body, routePaths = [], waitForActivation = false) => style + `<div id="exact-root">${body}</div><span id="secondary" style="position:absolute;color:var(--exact-secondary-label)"></span>
<script type="module">
  import { navigation, unselected } from './navigation.js';
  const root = document.getElementById('exact-root');
  window.calls = [];
  navigation.connect(root, () => false, line => calls.push(line), id => calls.push('host back ' + id), null, href => ${JSON.stringify(routePaths)}.includes(href), ${waitForActivation});
  window.activate = () => navigation.activate(root, line => calls.push(line));
  window.select = key => { document.getElementById('nav').setAttribute('navigationKey', key); navigation.project(root, () => {}); };
  const locations = new Map();
  window.push = (key, url, visits) => {
    locations.set(Number(key), url);
    navigation.apply({ top: Number(key), url, removed: [], visits: visits ?? [...locations] }); select(key);
  };
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
  const suite = await cdpBrowser({ args: ['--window-size=1400,900'] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
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
    const click = async (id) => {
      const [x, y] = await evaluate(`(e => { const b = e.getBoundingClientRect(); return [b.x + b.width / 2, b.y + b.height / 2]; })(document.getElementById(${JSON.stringify(id)}))`);
      await call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
      await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
    };
    await size(w);
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    await test(evaluate, size, click);
  } finally {
    await suite.close();
    server.close();
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
}), 60000);

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

check('appearance none keeps authored root tabs across selection and restores the sidebar with auto', () => inChrome(page(tabs(BAR.replace('id="bar"', 'id="bar" class="authored-tabs"'))) + '<style>.authored-tabs { appearance: none; }</style>', async (evaluate) => {
  await evaluate(`select(10)`);
  expect(await evaluate(`[document.getElementById('nav').hasAttribute('data-exact-sidebar'), box('bar')[2], shown('t1'), shown('r2')]`))
    .toEqual([false, 1280, true, false]);
  await evaluate(`document.getElementById('bar').style.appearance = 'auto'; select(2)`);
  expect(await evaluate(`[document.getElementById('nav').hasAttribute('data-exact-sidebar'), shown('r2')]`)).toEqual([true, true]);
  await evaluate(`document.getElementById('bar').style.appearance = 'none'; select(10)`);
  expect(await evaluate(`document.getElementById('nav').hasAttribute('data-exact-sidebar')`)).toBe(false);
}), 60000);

check('a projection with nothing to change writes nothing', () => inChrome(page(tabs(BAR)), async (evaluate) => {
  // What projections write, but for the selection and a style the test itself sets on the tab bar.
  await evaluate(`window.writes = (change) => new Promise(done => { const seen = [], watch = new MutationObserver(records => seen.push(...records.filter(r => r.attributeName !== 'navigationkey' && !(r.target.id === 'bar' && r.attributeName === 'style')).map(r => (r.target.id || r.target.localName) + ' ' + r.attributeName))); watch.observe(document.getElementById('exact-root'), { attributes: true, subtree: true }); change(); setTimeout(() => { watch.disconnect(); done(seen); }); })`);
  // A split showing the detail a row's press pushed, then a bound style that does not place the bar; then a sheet
  // over it, the tab bar inert beneath.
  await evaluate(`select('1'); { const row = document.getElementById('r1-bo'); row.addEventListener('click', () => push('2', '/people/bo'), {once:true}); row.click(); }`);
  expect(await evaluate(`writes(() => { select(2); document.getElementById('bar').style.backgroundColor = 'red'; select(2); }).then(seen => [document.getElementById('nav').hasAttribute('data-exact-sidebar'), document.getElementById('r1-bo').hasAttribute('data-exact-split-row'), seen])`)).toEqual([true, true, []]);
  await evaluate(`select(3)`);
  expect(await evaluate(`writes(() => { select(3); select(3); }).then(seen => [document.getElementById('bar').inert, seen])`)).toEqual([true, []]);
}), 60000);

check('retained root tabs yield to changed authored placement and restore without losing selection', () => inChrome(page(tabs(BAR)), async (evaluate, size) => {
  // Each change to what places the bar, then its undoing: by an inline style, a class or the bar's orientation.
  await evaluate(`document.head.append(Object.assign(document.createElement('style'), { textContent: '#exact-root .placed { position: absolute !important; }' }))`);
  const style = (id, property, value) => `document.getElementById('${id}').style.${property} = '${value}'`, bar = "document.getElementById('bar')";
  const samples = [];
  for (const change of [
    [style('nav', 'display', 'block'), style('nav', 'display', 'flex')],
    [style('bar', 'position', 'absolute'), style('bar', 'position', 'relative')],
    [style('nav', 'flexDirection', 'row'), style('nav', 'flexDirection', 'column')],
    [style('bar', 'flexDirection', 'column'), style('bar', 'flexDirection', 'row')],
    [`${bar}.classList.add('placed')`, `${bar}.classList.remove('placed')`],
    [`${bar}.setAttribute('aria-orientation', 'vertical')`, `${bar}.removeAttribute('aria-orientation')`],
  ]) {
    for (const step of change) samples.push(await evaluate(`${step}; select(2); [document.getElementById('nav').hasAttribute('data-exact-sidebar'), shown('r2'), shown('t1')]`));
  }
  expect(samples).toEqual(Array.from({ length: 6 }, () => [[false, true, false], [true, true, false]]).flat());
  await size(390);
  expect(await evaluate(`document.getElementById('nav').hasAttribute('data-exact-sidebar')`)).toBe(false);
  await size(1280);
  expect(await evaluate(`[document.getElementById('nav').hasAttribute('data-exact-sidebar'), box('bar'), shown('r2')]`)).toEqual([true, [0, 0, 240, 800], true]);
}), 60000);

check('a list whose rows push is beside its detail on a wide window, and pushes on a phone', () => inChrome(page(tabs(BAR)), async (evaluate, size) => {
  // Wide, nothing chosen: the list is a 320-wide column, its disclosure gone; the rest of the stack's width is the
  // platform's placeholder (LLP 1116.006 D2), one line in the secondary label's colour (the look's, which its sheet carries).
  await evaluate(`select('1')`);
  const secondary = await evaluate(`getComputedStyle(document.getElementById('secondary')).color`);
  expect(secondary).not.toBe(await evaluate(`getComputedStyle(document.body).color`));
  expect(await evaluate(`[document.getElementById('r1').getAttribute('data-exact-split'), box('r1'), getComputedStyle(document.querySelector('#r1 img')).display, shown('r1'), placeholder('p1')]`))
    .toEqual(['list', [240, 0, 320, 800], 'none', true, ['"No Selection"', 320, secondary]]);
  // A row's press pushes its detail: both show, the list live and the row marked; the detail has no Back.
  await evaluate(`{ const row = document.getElementById('r1-bo'); row.addEventListener('click', () => push('2', '/people/bo'), {once:true}); row.click(); }`);
  expect(await evaluate(`[shown('r1'), shown('r2'), box('r2'), document.getElementById('r2').getAttribute('data-exact-split'), document.getElementById('r1-bo').hasAttribute('data-exact-split-row'), back('r2'), off(), placeholder('p1')]`))
    .toEqual([true, true, [560, 0, 720, 800], 'detail', true, false, ['p2', 'r3'], null]);
  // A sheet stays a sheet over both, the list and the detail inert beneath it. In a wide window the look draws it as a
  // card (LLP 1115.000 D4.8): 560 wide, a sheet's height (45rem), in the window's middle once it has slid up.
  await evaluate(`push('3', '/people/bo/edit')`);
  const sheet = await evaluate(`document.getElementById('r3').getAnimations().forEach(a => a.finish()); box('r3')`);
  expect([sheet[0], sheet[2], sheet[3], Math.abs(sheet[1] * 2 + sheet[3] - 800) <= 1]).toEqual([360, 560, 720, true]);
  expect(await evaluate(`[document.getElementById('r3').hasAttribute('data-exact-split'), shown('r1'), shown('r2'), document.getElementById('r1').hasAttribute('data-exact-covered'), off()]`))
    .toEqual([false, false, false, true, ['p2', 'r1', 'r2']]);
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
  // The other tab's rows do not write the router: the stack stays unsplit, with no placeholder.
  expect(await evaluate(`[document.getElementById('t1').hasAttribute('data-exact-split'), placeholder('p2')]`)).toEqual([false, null]);
  // A row that is a link (LLP 1115.000 D4.4) is followed from the document's capture phase (input-glue.js, router.js),
  // so its commit lands within the click, before a listener nearer the row would hear it: its row is marked all the same.
  await evaluate(`select('1'); document.addEventListener('click', e => { if (e.target.closest('#r1-ada')) push('2', '/people/ada'); }, true); document.getElementById('r1-ada').click()`);
  expect(await evaluate(`[shown('r2'), document.getElementById('r1-ada').hasAttribute('data-exact-split-row'), document.getElementById('r1-bo').hasAttribute('data-exact-split-row')]`)).toEqual([true, true, false]);
}), 60000);

check('the author\'s placeholder in the root route takes the detail pane while nothing is selected, and shows nowhere else', () => {
  // As the lowerer writes it: `display: none` but where the split shows it (LLP 1116.006 D2).
  const own = '<div navigationSplit="placeholder" id="own" style="display:none;gap:8px"><span id="own-text">Pick a person</span><button id="own-add">Add</button></div>';
  return inChrome(page(`<main id="nav" navigationKey="1" navigationBack="back" style="width:100%;height:100%;position:relative">${list('r1', 1, true, '', own)}${detail('r2', 2)}</main>`), async (evaluate, size) => {
    const hit = `(e => document.elementFromPoint(box(e)[0] + box(e)[2] / 2, box(e)[1] + box(e)[3] / 2)?.id)`;
    // Wide, nothing chosen: the author's box is the pane beside the list, its content centred and pressable; no host line.
    // OWED (QUEUE.md, the neutral look): the pane starts at 319, not 320, and so ends a pixel short of the window. The
    // shell places it at the list column's `100%`, and the neutral look draws that column's divider as a 1 px border
    // where LLP 1116 D1's drew a shadow, so the column's own box is a pixel narrower. A detail is placed by the stack.
    expect(await evaluate(`[box('r1'), box('own'), shown('own'), Math.round(box('own-text')[0] + box('own-text')[2] / 2), ${hit}('own-add'), placeholder('nav'), document.getElementById('nav').hasAttribute('data-exact-split-empty')]`))
      .toEqual([[0, 0, 320, 800], [319, 0, 960, 800], true, 799, 'own-add', null, false]);
    // A narrower wide window: the pane follows.
    await size(1000);
    expect(await evaluate(`box('own')`)).toEqual([319, 0, 680, 800]);
    // A detail shown: the author's box is gone; back at the list, it returns.
    await evaluate(`{ const row = document.getElementById('r1-bo'); row.addEventListener('click', () => push('2', '/people/bo'), {once:true}); row.click(); }`);
    expect(await evaluate(`[box('own'), getComputedStyle(document.getElementById('own')).display, box('r2')]`)).toEqual([[0, 0, 0, 0], 'none', [320, 0, 680, 800]]);
    await evaluate(`push('1', '/')`);
    expect(await evaluate(`box('own')`)).toEqual([319, 0, 680, 800]);
    // A phone: the list alone, the author's box nowhere.
    await size(390);
    expect(await evaluate(`[box('r1'), getComputedStyle(document.getElementById('own')).display, placeholder('nav')]`)).toEqual([[0, 0, 390, 800], 'none', null]);
  });
}, 60000);

check('navigationSplit opts a stack out, or names the width of a list the rule does not find', async () => {
  const stack = (extra, rows) => `<main id="nav" navigationKey="2" navigationBack="back" style="width:100%;height:100%;position:relative">${list('r1', 1, rows, extra)}${detail('r2', 2)}</main>`;
  await inChrome(page(stack('navigationSplit="none"', true)), async (evaluate) => {
    expect(await evaluate(`[shown('r1'), document.getElementById('r1').hasAttribute('data-exact-split'), box('r2')]`)).toEqual([false, false, [0, 0, 1280, 800]]);
  });
  await inChrome(page(stack('navigationSplit="280"', false)), async (evaluate) => {
    expect(await evaluate(`[shown('r1'), box('r1'), box('r2'), back('r2')]`)).toEqual([true, [0, 0, 280, 800], [280, 0, 1000, 800], false]);
  });
}, 60000);

// Stage 6d's projection inputs. These DOM fixtures test navigation.js, not
// whether either compiler emits the facts; compiled-plan coverage is separate.
const ruleStack = body => `<main id="nav" navigationKey="2" navigationBack="back" style="width:100%;height:100%;position:relative">
  <div id="r1" navigationKey="1" style="${abs}"><header><h1>People</h1></header>${body}</div>${detail('r2', 2)}</main>`;

check('a served document stays unsplit across resize until its target adopts it', () => {
  const body = '<div data-exact-list><button data-exact-router-press="true">Open</button></div>';
  const stack = ruleStack(body).replace('id="r1"', 'id="r1" navigationSplit="280"');
  return inChrome(page(stack, [], true), async (evaluate, size) => {
    const state = `[document.getElementById('r1').getAttribute('data-exact-split'), document.getElementById('r2').getAttribute('data-exact-split'), document.getElementById('r1').style.getPropertyValue('--exact-split'), document.getElementById('r1').inert, document.getElementById('r2').inert]`;
    expect(await evaluate(state)).toEqual([null, null, '', false, false]);
    await size(390); await size(1280);
    expect(await evaluate(state)).toEqual([null, null, '', false, false]);
    await evaluate('activate()');
    expect(await evaluate(state)).toEqual(['list', 'detail', '280px', false, false]);
  });
}, 60000);

check('split admission reads list and outer press facts, not roles or decorative chevrons', () => {
  const link = '<a href="/item/a">A</a>';
  const cases = [
    ['link without a chevron', `<div data-exact-list role="presentation">${link}</div>`, true],
    ['link beside decoration', `<div data-exact-list>${link}<button data-exact-on="press" data-exact-router-press="false">Decorative${chevron}</button></div>`, true],
    ['decorative list', `<div data-exact-list><button data-exact-on="press" data-exact-router-press="false">Decorative${chevron}</button></div>`, false],
    ['press outside a list', `<button data-exact-on="press" data-exact-router-press="true">Open${chevron}</button>`, false],
    ['handler beats href', `<div data-exact-list><a href="/item/a" data-exact-on="press" data-exact-router-press="false">Pick${chevron}</a></div>`, false],
    ['outer press beats inner press', '<div data-exact-list><div data-exact-on="press" data-exact-router-press="false"><button data-exact-on="press" data-exact-router-press="true">Open</button></div></div>', false],
    ['another route before a list', `<div navigationKey="nested"><div data-exact-list>${link}</div></div>`, false],
    ['route key inside list rows', `<div data-exact-list><div navigationKey="nested">${link}</div></div>`, true],
    ['sideways outer list', `<div data-exact-list style="display:flex;flex-direction:row"><div data-exact-list>${link}</div></div>`, true],
  ];
  return inChrome(page(ruleStack(''), ['/item/a']), async evaluate => {
    const observed = [];
    for (const [name, body] of cases) {
      // A new root identity isolates each admission case from retained-list state.
      const route = `<div id="r1" navigationKey="1" style="${abs}">${body}</div>`;
      observed.push([name, await evaluate(`document.getElementById('r1').outerHTML = ${JSON.stringify(route)}; select(2); document.getElementById('r1').hasAttribute('data-exact-split')`)]);
    }
    expect(observed).toEqual(cases.map(([name, , splits]) => [name, splits]));
  });
}, 60000);

check('a later pushing list wins over a retained empty list, whose identity alone can retain the split', () => {
  const body = `<div data-exact-list id="first"><button data-exact-on="press" data-exact-router-press="true">Open${chevron}</button></div>
    <div data-exact-list id="later" style="display:none"><a id="later-row" href="/item/b">B</a></div>`;
  return inChrome(page(ruleStack(body), ['/item/b']), async evaluate => {
    expect(await evaluate(`document.getElementById('r1').hasAttribute('data-exact-split')`)).toBe(true);
    await evaluate(`document.getElementById('first').textContent = 'No Results'; document.getElementById('later').style.display = 'block'; push(2, '/item/b', [[1, '/'], [2, '/item/b']])`);
    expect(await evaluate(`document.getElementById('later-row').hasAttribute('data-exact-split-row')`)).toBe(true);
    await evaluate(`document.getElementById('later').textContent = 'No Results'; select(2)`);
    expect(await evaluate(`document.getElementById('r1').hasAttribute('data-exact-split')`)).toBe(true);
    // The first empty list was not the retained candidate after the later one pushed.
    await evaluate(`document.getElementById('later').remove(); select(2)`);
    expect(await evaluate(`[document.getElementById('r1').hasAttribute('data-exact-split'), back('r2')]`)).toEqual([false, true]);
  });
}, 60000);

check('a bare plan rechecks sideways admission on a resize that stays above the wide threshold', () => {
  const body = `<div data-exact-list id="resize-list"><button data-exact-on="press" data-exact-router-press="true">Open${chevron}</button></div>`;
  const html = page(ruleStack(body)) + '<style>@media (min-width:1100px) { #resize-list { display:flex;flex-direction:row } }</style>';
  // No exactViewport runtime and no authored or visible default placeholder.
  return inChrome(html, async (evaluate, size) => {
    expect(await evaluate(`[document.querySelector('[navigationSplit="placeholder"]') === null, document.querySelector('[data-exact-split-empty]') === null, document.getElementById('r1').hasAttribute('data-exact-split')]`)).toEqual([true, true, true]);
    await size(1200);
    expect(await evaluate(`[getComputedStyle(document.getElementById('resize-list')).flexDirection, document.getElementById('r1').hasAttribute('data-exact-split'), back('r2')]`)).toEqual(['row', false, true]);
    await size(1000);
    expect(await evaluate(`[document.getElementById('r1').hasAttribute('data-exact-split'), back('r2')]`)).toEqual([true, false]);
  }, 1000);
}, 60000);

check('turning a split off clears its selected mark and stale row ownership cannot hide deeper Back', () => {
  const body = `<div data-exact-list><button id="owned-row" data-exact-on="press" data-exact-router-press="true">Open${chevron}</button></div>`;
  const html = ruleStack(body).replace('</main>', `${detail('r3', 3)}</main>`);
  return inChrome(page(html), async evaluate => {
    await evaluate(`push(2, '/item/a'); { const row = document.getElementById('owned-row'); row.addEventListener('click', () => push(3, '/item/b'), {once:true}); row.click(); }`);
    expect(await evaluate(`[document.getElementById('owned-row').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([true, false]);
    await evaluate(`document.getElementById('r1').setAttribute('navigationSplit', 'none'); select(3)`);
    expect(await evaluate(`[document.getElementById('owned-row').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([false, true]);
    // Keep an explicit split width, but revoke the old row's router-writing fact.
    await evaluate(`document.getElementById('r1').setAttribute('navigationSplit', '280'); document.getElementById('owned-row').setAttribute('data-exact-router-press', 'false'); select(3)`);
    expect(await evaluate(`[box('r1')[2], document.getElementById('owned-row').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([280, false, true]);
  });
}, 60000);

check('check advises on a bottom tab bar and an unsplit pushing list at a desktop width, and on neither at a phone\'s', () => {
  const positioned = BAR.replace('style="display:flex;flex-direction:row"', 'style="display:flex;flex-direction:row;position:absolute;left:0;right:0;bottom:0"');
  const html = page(tabs(positioned).replaceAll(chevron, '').replace('id="r1" style="position:absolute', 'id="r1" style="position:relative')) + `<script type="module">
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
}, 60000);

check('the split and sidebar leave check nothing to advise', () => inChrome(page(tabs(BAR)) + `<script type="module">
    import { observe } from './check.js';
    window.advice = () => { const views = new Map([...document.querySelectorAll('#exact-root *')].map(e => [e.id || e.localName, e])); return observe(views).advice ?? []; };
  </script>`, async (evaluate) => {
  expect(await evaluate(`select('2'); advice()`)).toEqual([]);
}), 60000);

// LLP 1116.005 D3: each route keeps its own scroll (LLP 1038 §7: mounted rows keep theirs), on a phone and through a
// split. A route of `n` tall rows over its own scroller; `rows` writes the router, so its stack splits.
const tall = (id, key, n, rows = false) => `<div navigationKey="${key}" id="${id}" style="${abs}"><header><h2>${id}</h2></header><div id="${id}-scroll" ${rows ? 'data-exact-list' : ''} style="flex:1 1 0%;min-height:0;overflow:auto">${
  Array.from({ length: n }, (_, i) => rows ? `<button data-exact-on="press" data-exact-router-press="true" style="display:flex;height:50px"><span data-exact-text>${i}</span>${chevron}</button>` : `<div style="height:50px">${i}</div>`).join('')}</div></div>`;

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
}), 60000);

check('retained visit URLs restore href selection after adoption, replace and same-top reset', () => {
  const body = `<div data-exact-list role="presentation" id="rows"><div data-listitemkey="a"><a id="row-a" href="/item/a">A<button data-exact-router-press="false">Star</button></a></div><a id="row-b" href="/item/b">B</a></div>`;
  const html = ruleStack(body).replace('</main>', `${detail('r3', 3)}</main>`);
  return inChrome(page(html, ['/item/a', '/item/b']), async evaluate => {
    const marks = `[...document.querySelectorAll('[data-exact-split-row]')].map(e => e.id)`;
    // A restored deeper route inherits its underlying row's selection, but keeps Back.
    await evaluate(`push(3, '/edit', [[1, '/'], [2, '/item/a'], [3, '/edit']])`);
    expect(await evaluate(`[${marks}, back('r2'), back('r3')]`)).toEqual([['row-a'], false, true]);
    await evaluate(`push(3, '/edit', [[1, '/'], [2, '/item/b'], [3, '/edit']])`);
    expect(await evaluate(marks)).toEqual(['row-b']);
    // A same-top reset must replace the snapshot before its history fast path returns.
    await evaluate(`(async () => { const {navigation} = await import('/navigation.js'); navigation.reset({top:3,url:'/edit',visits:[[1,'/'],[2,'/unknown'],[3,'/edit']]}); select(3); })()`);
    expect(await evaluate(`[${marks}, back('r3')]`)).toEqual([[], true]);
    await evaluate(`push(3, '/edit', [[1, '/'], [2, '/item/a'], [3, '/edit']]); document.getElementById('row-a').setAttribute('data-exact-router-press', 'false'); select(3)`);
    expect(await evaluate(marks)).toEqual([]);
    await evaluate(`document.getElementById('row-a').removeAttribute('data-exact-router-press'); select(3)`);
    expect(await evaluate(marks)).toEqual(['row-a']);
    await evaluate(`document.getElementById('rows').style.display = 'none'; select(3)`);
    expect(await evaluate(`[${marks}, document.getElementById('r1').hasAttribute('data-exact-split')]`)).toEqual([[], false]);
    await evaluate(`document.getElementById('rows').style.display = ''; select(3)`);
    expect(await evaluate(marks)).toEqual(['row-a']);
  });
}, 60000);

check('only an ordinary outer press associates its row with navigation', () => {
  const body = '<div data-exact-list><div id="owner" data-exact-router-press="true"><button id="star" data-exact-router-press="false">Star</button></div><a id="action-link" href="/item/b" data-exact-router-press="true">Linked action</a></div>';
  const html = ruleStack(body).replace('</main>', `${detail('r3', 3)}</main>`);
  return inChrome(page(html), async (evaluate, _size, click) => {
    // A refused action owns no later timer/resource/keyboard navigation after this click dispatch ends.
    // The first detail has no Back in a split whoever owns it, so ownership shows on a push inside it.
    await evaluate(`push(2, '/item/a'); document.getElementById('owner').click(); push(3, '/item/b')`);
    expect(await evaluate(`[document.getElementById('owner').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([false, true]);
    // The host leaves these link clicks to the browser, so a later router change owns no row.
    for (const modifier of ['ctrlKey', 'metaKey', 'shiftKey', 'altKey']) {
      await evaluate(`push(2, '/item/a'); { const link = document.getElementById('action-link'); link.addEventListener('click', e => e.preventDefault(), {once:true}); link.dispatchEvent(new MouseEvent('click', {bubbles:true,cancelable:true,${modifier}:true})); } push(3, '/edit')`);
      expect(await evaluate(`[document.getElementById('action-link').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([false, true]);
    }
    await evaluate(`push(2, '/item/a'); document.getElementById('star').click(); push(3, '/edit')`);
    expect(await evaluate(`[document.getElementById('owner').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([false, true]);
    await evaluate(`document.getElementById('owner').addEventListener('click', () => push(3, '/item/b'), {once:true})`);
    await click('owner');
    expect(await evaluate(`[document.getElementById('owner').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([true, false]);
    await evaluate(`(async () => { const {navigation} = await import('/navigation.js'); navigation.reset({top:3,url:'/item/b',visits:[[3,'/item/b']]}); select(3); })()`);
    expect(await evaluate(`[document.getElementById('owner').hasAttribute('data-exact-split-row'), back('r3')]`)).toEqual([false, true]);
  });
}, 60000);
