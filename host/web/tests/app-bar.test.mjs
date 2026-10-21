// LLP 1116.000 D4 in a real browser: a default build's stylesheet and the
// navigation glue's projection (navigation.js, which both web targets run)
// over nodes as the hosts write them. A pushed route's app bar draws Back,
// in the bar's own ink (a route above its stack's root, not a sheet, whose header
// does not hold the root's `navigationBack` control), and a block `link` of
// text is in the accent with the pointing hand while an authored colour, a
// row inside it, a `role` or a `buttonStyle` keeps the author's look. The
// browser's Back with no `navigate` handler is the platform's own whatever the
// routes are keyed by (LLP 1115 D5). A root whose key names no route keeps
// what is left of the stack it showed (LLP 1035.001 D1), so a closed sheet
// never leaves the route beneath it inert.
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

// A root with two tabs. The first tab's stack: a root, a pushed route, one
// whose Back control is in its content, and a sheet. The second's: a root
// with a long title, a pushed route, and one whose header holds its control.
// Under the stylesheet a default build's page has (look.mjs).
const page = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root">
  <main id="nav" navigationKey="2" navigationBack="back" style="display:block">
    <div role="tabpanel" id="p1">
      <div navigationKey="1" id="r1"><header><h1>People</h1></header></div>
      <div navigationKey="2" id="r2">
        <header style="display:flex;align-items:center"><h2 id="title2">Ada</h2></header>
        <div id="links" style="color:rgb(1, 2, 3)">
          <a data-view href="tel:555" id="l-text"><div data-exact-text>555</div></a>
          <a data-view href="mailto:a@b.c" id="l-symbol"><img alt=""><div data-exact-text>Mail</div></a>
          <a data-view href="tel:556" id="l-authored" style="color:rgb(204, 0, 0)"><div data-exact-text>Red</div></a>
          <a data-view href="mailto:r@b.c" id="l-row"><div><img alt=""><div data-exact-text>A row</div></div></a>
          <a data-view href="mailto:b@b.c" id="l-role" role="button"><div data-exact-text>Button</div></a>
          <a data-view href="mailto:s@b.c" id="l-style" data-button-style="plain"><div data-exact-text>Styled</div></a>
          <div style="--exact-accent:rgb(10, 20, 30)"><a data-view href="tel:557" id="l-accent"><div data-exact-text>Tinted</div></a></div>
          <div data-exact-text>An <a data-view href="/x" id="l-run" style="color:LinkText">inline run</a>, <a data-view id="l-plain" style="color:inherit">a run whose href is empty</a>.</div>
          <a data-view id="l-blank"><div data-exact-text>A block link whose href is empty</div></a>
          <span id="ink" style="color:var(--exact-label)"></span><span id="accent" style="color:var(--exact-accent, AccentColor)"></span>
        </div>
      </div>
      <div navigationKey="3" id="r3"><header><h2>More</h2></header><button id="back">Done</button></div>
      <div navigationKey="4" id="r4" navigationPresentation="modal"><header><button id="back">Cancel</button><h2>New</h2></header></div>
    </div>
    <div role="tabpanel" id="p2">
      <div navigationKey="10" id="t1"><header><h1>Favorites of everyone</h1></header></div>
      <div navigationKey="11" id="t2"><header><h2>Favorite</h2></header></div>
      <div navigationKey="12" id="t3"><header><button id="back">Mine</button><h2>Own</h2></header></div>
    </div>
    <div role="tablist"><button role="tab" aria-controls="p1">People</button><button role="tab" aria-controls="p2">Favorites</button></div>
  </main>
</div>
<script type="module">
  import { navigation } from './navigation.js';
  const root = document.getElementById('exact-root'), nav = document.getElementById('nav');
  window.calls = []; window.heard = false;
  navigation.connect(root, location => { calls.push('navigate ' + location); return heard; }, line => calls.push('log ' + line), id => calls.push('host back ' + id));
  for (const id of ['r3', 't3']) document.querySelector('#' + id + ' #back').addEventListener('click', () => calls.push('pressed ' + id));
  window.select = key => { nav.setAttribute('navigationKey', key); navigation.project(root, () => {}); };
  window.backs = () => [...document.querySelectorAll('header')].map(h => { const b = h.shadowRoot?.querySelector('button'); return b && !b.hidden ? h.parentElement.id + ':' + b.textContent + '|' + b.getAttribute('aria-label') : null; }).filter(Boolean);
  window.press = id => document.querySelector('#' + id + ' > header').shadowRoot.querySelector('button').click();
  select('2');
  window.ready = true;
</script>`;

/** The page (`html`) in headless Chrome: `test(evaluate, call)` with its `Runtime.evaluate` and its CDP session. */
async function inChrome(test, html = page) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(html); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'navigation.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const suite = await cdpBrowser({ args: ['--window-size=600,900'] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
    const call = (method, params) => cdp.send(method, params, sessionId);
    // The light look these checks expect, in whatever appearance this machine has; no motion, as the helpers remove a
    // route by hand and do not pass over a sheet that is still sliding away (neutral-look.test.mjs holds the motion).
    await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }, { name: 'prefers-reduced-motion', value: 'reduce' }] });
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    await test(evaluate, call);
  } finally {
    await suite.close();
    server.close();
  }
}

check('a pushed route draws Back at its app bar\'s leading edge, and goes back as the browser\'s Back does', () => inChrome(async (evaluate, call) => {
  // Pushed routes only: neither stack's root nor the sheet; none where the header holds the control. The words are
  // the route beneath's title, "Back" for a long one, or the control's when it is elsewhere in the route.
  expect(await evaluate('backs()')).toEqual(['r2:People|Back to People', 'r3:Done|Done', 't2:Back|Back']);
  // A disabled control is no way back, as iOS hides its Back button; enabled again, it is.
  expect(await evaluate(`document.querySelector('#r3 #back').disabled = true; select('2'); backs()`)).toEqual(['r2:People|Back to People', 't2:Back|Back']);
  expect(await evaluate(`document.querySelector('#r3 #back').disabled = false; select('2'); backs().length`)).toBe(3);
  // Until the page is interactive the wasm glue disables every control (`aria-busy`), which is not the author's.
  expect(await evaluate(`const busy = document.getElementById('exact-root'); busy.setAttribute('aria-busy', 'true'); document.querySelector('#r3 #back').disabled = true; select('2'); const n = backs().length; busy.removeAttribute('aria-busy'); document.querySelector('#r3 #back').disabled = false; n`)).toBe(3);
  // The renderer's children stay its own: the button is the header's shadow root's, before its slot, at the leading edge.
  expect(await evaluate(`[document.querySelector('#r2 > header').children.length, document.querySelector('#r2 > header').shadowRoot.querySelector('button').getBoundingClientRect().right <= document.getElementById('title2').getBoundingClientRect().left]`)).toEqual([1, true]);
  // It is one of the bar's buttons, so in the bar's ink, the label's (LLP 1116 D1's look drew it in the accent, as UIKit does).
  expect(await evaluate(`[getComputedStyle(document.querySelector('#r2 > header').shadowRoot.querySelector('button')).color === getComputedStyle(document.getElementById('ink')).color, getComputedStyle(document.getElementById('ink')).color === getComputedStyle(document.getElementById('links')).color]`)).toEqual([true, false]);

  // Keyboard: the first Tab stop of the shown route; Enter goes back (the runner's own, with no entry beneath known).
  await evaluate('document.activeElement.blur()');
  await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
  await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 });
  expect(await evaluate(`[document.activeElement.parentElement.id, document.activeElement.shadowRoot?.activeElement?.textContent]`)).toEqual(['r2', 'People']);
  await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' });
  await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  expect(await evaluate('calls.splice(0)')).toEqual(['host back 2']);
  // With the entry beneath in the history, Back is the root's `navigate` with its location, as the browser's Back
  // (`popped`); unheard, the runner's own.
  expect(await evaluate(`(async () => { const { navigation } = await import('./navigation.js'); navigation.apply({ top: 1, url: '/', removed: [] }); navigation.apply({ top: 2, url: '/people/ada', removed: [] }); heard = true; press('r2'); heard = false; press('r2'); return calls.splice(0); })()`))
    .toEqual(['navigate /', 'navigate /', 'host back 2']);
  // A control elsewhere in the route is what Back presses; only the shown route's Back acts.
  expect(await evaluate(`select('3'); press('r3'); press('r2'); calls.splice(0)`)).toEqual(['pressed r3']);
}), 60000);

check('Escape closes a sheet as its Cancel does; with no Cancel the root names, as the app bar\'s Back does', () => inChrome(async (evaluate, call) => {
  const escape = async () => {
    await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
    await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
    return evaluate('calls.splice(0)');
  };
  await evaluate(`document.querySelector('#r4 #back').addEventListener('click', () => calls.push('pressed r4')); select('4'); calls.splice(0)`);
  expect(await escape()).toEqual(['pressed r4']);
  // A Cancel whose id the root does not name (LLP 1129's probe): the runner's own back, as iOS's swipe down takes it.
  await evaluate(`document.querySelector('#r4 #back').id = 'cancel'; select('4')`);
  expect(await escape()).toEqual(['host back 4']);
  // The innermost shown sheet is the one Escape closes, and one that closes only by its own buttons stays: a sheet
  // inside it that says `closedby="none"` is not passed over for this one.
  await evaluate(`document.getElementById('r4').insertAdjacentHTML('beforeend', '<main id="inner" navigationKey="31" navigationBack="back"><div navigationKey="30"></div><div navigationKey="31" navigationPresentation="modal" closedby="none"></div></main>'); select('4')`);
  expect(await escape()).toEqual([]);
  await evaluate(`document.getElementById('inner').remove(); select('4')`);
  // A sheet that is not shown is passed over for the one that is.
  await evaluate(`document.getElementById('r4').insertAdjacentHTML('beforeend', '<div id="hidden" style="visibility:hidden"><main navigationKey="51" navigationBack="back"><div navigationKey="50"></div><div navigationKey="51" navigationPresentation="modal"></div></main></div>'); select('4')`);
  expect(await escape()).toEqual(['host back 4']);
  await evaluate(`document.getElementById('hidden').remove(); select('4')`);
  // Nor an inert one.
  await evaluate(`nav.inert = true`);
  expect(await escape()).toEqual([]);
  await evaluate(`nav.inert = false; document.getElementById('r4').setAttribute('closedby', 'none'); select('4')`);
  expect(await escape()).toEqual([]);
  // A sheet with nothing beneath it is closed by its named control.
  await evaluate(`select('2'); document.getElementById('exact-root').insertAdjacentHTML('beforeend', '<main navigationKey="40" navigationBack="back"><div navigationKey="40" navigationPresentation="fullscreen"><button id="back">Close</button></div></main>'); document.querySelector('[navigationKey="40"] > #back').addEventListener('click', () => calls.push('pressed 40'))`);
  expect(await escape()).toEqual(['pressed 40']);
}), 60000);

check('a block link of text is in the accent and points; the author\'s look wins', () => inChrome(async (evaluate) => {
  // A link of text (or a symbol and text) is in the accent, the app's when it names one; an authored colour, a row
  // inside it, a role or a buttonStyle keeps the author's look; an inline run keeps its LinkText row. Every link points.
  // (The page's accent is the look's: the browser's own hue, fitted to read as text. Each engine words it its own way.)
  // An `href` that turns out empty is not written (LLP 1115 §3): that `<a>` is no link, so it does not point and is not
  // in the accent; the run's `inherit` is the compiler's row for an empty value.
  const accent = await evaluate(`getComputedStyle(document.getElementById('accent')).color`);
  expect(['rgb(1, 2, 3)', 'rgb(0, 0, 0)', 'rgba(0, 0, 0, 0)'].includes(accent)).toBe(false);
  expect(await evaluate(`['l-text', 'l-symbol', 'l-authored', 'l-row', 'l-role', 'l-style', 'l-accent', 'l-run', 'l-plain', 'l-blank'].map(id => { const s = getComputedStyle(document.getElementById(id)); return id + ' ' + s.color + ' ' + s.cursor + ' ' + s.display + ' ' + s.textDecorationLine; })`)).toEqual([
    `l-text ${accent} pointer block none`,
    `l-symbol ${accent} pointer block none`,
    'l-authored rgb(204, 0, 0) pointer block none',
    'l-row rgb(1, 2, 3) pointer block none',
    'l-role rgb(1, 2, 3) pointer block none',
    'l-style rgb(1, 2, 3) pointer block none',
    'l-accent rgb(10, 20, 30) pointer block none',
    'l-run rgb(0, 0, 238) pointer inline none',
    'l-plain rgb(1, 2, 3) auto inline none',
    'l-blank rgb(1, 2, 3) auto block none',
  ]);
}), 60000);

// A stack whose root route is keyed by a name, not its visit id, as the list-detail pattern keys its list
// (`navigationKey="items"`), over a stand-in runner: each visit a route, each change the router's op, the runner's own
// `back` (`host back`) popping the selected top, and a `navigate` handler, when heard, `go` to the location. With
// `byVisit` the root names the visit at the stack's root too (`${top(nav).id}`), which no route carries.
const keyed = '<title>Starter</title>' + page.match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root"><main id="nav" navigationKey="items" navigationBack="back" style="display:block">
  <div navigationKey="items"><header><h1>Items</h1></header></div>
</main></div>
<script type="module">
  import { navigation } from './navigation.js';
  const root = document.getElementById('exact-root'), nav = document.getElementById('nav');
  window.calls = []; window.heard = false; window.byVisit = false; window.stack = [[0, '/']];
  const show = window.show = removed => {
    const [top, url] = stack.at(-1);
    nav.setAttribute('navigationKey', stack.length === 1 && !byVisit ? 'items' : String(top));
    navigation.project(root, line => calls.push('log ' + line)); navigation.apply({ top, url, removed });
  };
  window.pop = () => { const [id] = stack.pop(); nav.lastElementChild.remove(); show([id]); };
  window.push = (id, url, sheet = false) => {
    const route = document.createElement('div');
    route.setAttribute('navigationKey', String(id));
    if (sheet) { route.setAttribute('navigationPresentation', 'modal'); route.innerHTML = '<header><button id="back">Cancel</button><h2>New</h2></header>'; }
    else route.innerHTML = '<header><h2>' + url + '</h2></header>';
    route.querySelector('#back')?.addEventListener('click', () => { calls.push('pressed ' + id); pop(); });
    nav.append(route); stack.push([id, url]); show([]);
  };
  navigation.connect(root, location => {
    calls.push('navigate ' + location);
    if (heard) while (stack.length > 1 && stack.at(-1)[1] !== location) pop();
    return heard;
  }, line => calls.push('log ' + line), id => { calls.push('host back ' + id); if (stack.length > 1 && stack.at(-1)[0] === id) pop(); },
  // The runner's own go: to a location in the stack, else a push; window.lost names a location no route matches.
  location => {
    calls.push('host go ' + location);
    if (location === window.lost) return;
    if (stack.some(([, url]) => url === location)) while (stack.at(-1)[1] !== location) pop(); else push(100 + stack.length, location);
  });
  show([]);
  // The browser's Back (Forward, or any number of entries), after the popstates it causes (a restored entry is a second).
  window.back = (pops = 1, by = -1) => new Promise(ok => {
    const heardPop = () => { if (--pops === 0) { removeEventListener('popstate', heardPop); ok([calls.splice(0), location.pathname, stack.length]); } };
    addEventListener('popstate', heardPop); history.go(by);
  });
  window.ready = true;
</script>`;

check('the browser\'s Back with no navigate handler is the runner\'s own back, however the routes are keyed', () => inChrome(async (evaluate) => {
  // The route beneath is keyed "items", not visit 0: unheard, Back one entry down is the runner's own `back` from the
  // visit the entry left (LLP 1115 D5), accepted where it landed.
  expect(await evaluate(`push(1, '/item/1'); back()`)).toEqual([['navigate /', 'host back 1'], '/', 1]);
  // A `navigate` handler still wins: it is the Back, and the runner does nothing of its own.
  expect(await evaluate(`heard = true; push(2, '/item/2'); back().finally(() => { heard = false; })`)).toEqual([['navigate /'], '/', 1]);
  // A sheet's Back control is what Back presses, as on iOS; a disabled one refuses, and the entry is restored.
  expect(await evaluate(`push(3, '/add', true); back()`)).toEqual([['navigate /', 'pressed 3'], '/', 1]);
  expect(await evaluate(`push(4, '/add', true); document.querySelector('[navigationKey="4"] #back').disabled = true; back(2)`))
    .toEqual([['navigate /', 'log history: Back refused: its id="back" control is disabled, inert or not shown; restoring the entry'], '/add', 2]);
}, keyed), 60000);

check('the browser\'s Forward with no navigate handler is the runner\'s own go', () => inChrome(async (evaluate) => {
  // LLP 1115.000 D4.3. Back pops two pushed routes, one entry at a time; Forward then re-pushes each: unheard, the
  // runner's own `go` to the entry's location, accepted where it landed (no restoring traversal, nothing refused).
  await evaluate(`push(1, '/item/1'); push(2, '/item/1/more'); back()`);
  expect(await evaluate('back()')).toEqual([['navigate /', 'host back 1'], '/', 1]);
  expect(await evaluate('back(1, 1)')).toEqual([['navigate /item/1', 'host go /item/1'], '/item/1', 2]);
  expect(await evaluate('back(1, 1)')).toEqual([['navigate /item/1/more', 'host go /item/1/more'], '/item/1/more', 3]);
  // Two entries back at once is not "the route beneath": the runner's `go` to that location, which pops to it.
  expect(await evaluate('back(1, -2)')).toEqual([['navigate /', 'host go /'], '/', 1]);
}, keyed), 60000);

check('Forward to a location the runner\'s own go refuses restores the entry', () => inChrome(async (evaluate) => {
  expect(await evaluate(`push(1, '/item/1'); back()`)).toEqual([['navigate /', 'host back 1'], '/', 1]);
  expect(await evaluate(`lost = '/item/1'; back(2, 1)`)).toEqual([['navigate /item/1', 'host go /item/1',
    'log history: navigate "/item/1" refused: the navigation root has no navigate handler (navigate=…), and the runner\'s own go there committed no router change; restoring the entry'], '/', 1]);
}, keyed), 60000);

check('the page\'s title follows the route: the head\'s, else the route header\'s heading, else the app\'s name', () => inChrome(async (evaluate) => {
  // LLP 1115.000 D4.1. The selected route's header's heading, at the root, on a pushed route and on a sheet; back again.
  expect(await evaluate('document.title')).toBe('Items');
  expect(await evaluate(`push(1, '/item/1'); document.title`)).toBe('/item/1');
  expect(await evaluate(`push(2, '/add', true); document.title`)).toBe('New');
  expect(await evaluate(`pop(); pop(); document.title`)).toBe('Items');
  // A route whose header has no heading is the app's name: the page's own title, before any route's.
  expect(await evaluate(`push(3, '/bare'); document.querySelector('[navigationKey="3"] h2').remove(); show([]); document.title`)).toBe('Starter');
  // An authored head's title wins on every route, as each target's head writer passes it; without it the route's is back.
  expect(await evaluate(`(async () => { const { pageTitle } = await import('./navigation.js'); pageTitle('From the head'); const said = [document.title]; pop(); said.push(document.title); pageTitle(null); return [...said, document.title]; })()`))
    .toEqual(['From the head', 'From the head', 'Items']);
}, keyed), 60000);

check('a closed sheet or a popped screen leaves the route beneath it live when the root names no route there', () => inChrome(async (evaluate) => {
  // The root names visit 0, which no route carries (chat-rooms in bake-off round 6): nothing is projected at first.
  const beneath = `(r => [r.inert, r.style.visibility, r.hasAttribute('data-exact-covered')])(document.querySelector('[navigationKey="items"]'))`;
  expect(await evaluate(`byVisit = true; show([]); calls.splice(0); ${beneath}`)).toEqual([false, '', false]);
  // A sheet covers it, shown and inert; Cancel closes the sheet and the route beneath is live again, not left inert.
  expect(await evaluate(`push(1, '/add', true); ${beneath}`)).toEqual([true, '', true]);
  expect(await evaluate(`document.querySelector('[navigationKey="1"] #back').click(); ${beneath}`)).toEqual([false, '', false]);
  // A pushed screen hides it; popped, it shows again.
  expect(await evaluate(`push(2, '/item/2'); ${beneath}`)).toEqual([true, 'hidden', false]);
  expect(await evaluate(`pop(); ${beneath}`)).toEqual([false, '', false]);
  // The key that names no route is still a refusal, journaled each time the root comes back to it.
  const refusal = 'log navigationKey "0" matches no route among the root\'s children or those of the tabpanels its tablist names; the stack is unchanged';
  expect(await evaluate(`calls.filter(c => c.startsWith('log '))`)).toEqual([refusal, refusal]);
}, keyed), 60000);

check('the bar says its own word, "Back", only on a page in English; elsewhere the chevron alone, named by the title beneath', () => inChrome(async (evaluate) => {
  // LLP 1115.000 D4.14. No `lang` reads as English, as every app here is until it says otherwise.
  const english = ['r2:People|Back to People', 'r3:Done|Done', 't2:Back|Back'];
  expect(await evaluate('backs()')).toEqual(english);
  // In another language the app's own words stay (a short title beneath, the Back control's) and are the name; the
  // bar's word is not said, so a long title beneath leaves the chevron alone, named by that title.
  expect(await evaluate(`document.documentElement.lang = 'de'; select('2'); backs()`)).toEqual(['r2:People|People', 'r3:Done|Done', 't2:|Favorites of everyone']);
  expect(await evaluate(`(b => [b.lastChild.hidden, b.hidden])(document.querySelector('#t2 > header').shadowRoot.querySelector('button'))`)).toEqual([true, false]);
  for (const lang of ['en-GB', 'EN', '']) expect([lang, await evaluate(`document.documentElement.lang = ${JSON.stringify(lang)}; select('2'); backs()`)]).toEqual([lang, english]);
}), 60000);

// LLP 1116.009 D4 (the neutral look's): a sheet's bar centres its title whatever flanks it, as UIKit's does; a page's stays leading. The
// author's flow wins: a header with its own `justify-content` (the host names it, `data-exact-said`) and a title with a
// margin of its own are left where they were written. Buttons 60 px wide, so what is centred is exact.
const bars = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>[\s\S]*?<\/style>/)[0] + `<div id="exact-root">
  ${[
    ['s1', 'modal', '', '', '<button>Cancel</button>', '', '<button>Save</button>'],
    ['s2', 'modal', '', '', '<button>Back</button>', '', ''],
    ['s3', 'modal', '', '', '', '', ''],
    ['p1', null, '', '', '<button>Back</button>', '', ''],
    ['s4', 'modal', 'data-exact-said="justify-content"', 'justify-content:space-between', '<button>Back</button>', '', ''],
    ['s5', 'modal', '', '', '<button>Back</button>', 'style="margin:0"', ''],
  ].map(([id, how, mark, flow, before, title, after]) => `<div navigationKey="${id}" id="${id}"${how ? ` navigationPresentation="${how}"` : ''}>
    <header style="display:flex;align-items:center;${flow}" ${mark}>${before.replace('<button>', '<button style="flex:none;width:60px">')}<h2 id="${id}-title" ${title}>Create an event</h2>${after.replace('<button>', '<button style="flex:none;width:60px">')}</header></div>`).join('\n  ')}
</div>
<script>
  window.edges = id => { const h = document.querySelector('#' + id + ' > header').getBoundingClientRect(), t = document.getElementById(id + '-title').getBoundingClientRect(); return { left: Math.round(t.left - h.left), right: Math.round(h.right - t.right), centre: Math.round((t.left + t.right) / 2 - (h.left + h.right) / 2), width: Math.round(t.width) }; };
  window.ready = true;
</script>`;

check('a sheet\'s bar centres its title, a page\'s stays leading, and the author\'s flow wins', () => inChrome(async (evaluate) => {
  // The padding is 16 px a side. Cancel and Save the same width: the title is on the bar's centre.
  const s1 = await evaluate(`edges('s1')`);
  expect(Math.abs(s1.centre)).toBeLessThanOrEqual(1);
  // A lone Back: centred between it and the bar's end, nowhere near the end (the free space is the same either side).
  const s2 = await evaluate(`edges('s2')`);
  expect(Math.abs((s2.left - 16 - 60) - (s2.right - 16))).toBeLessThanOrEqual(1);
  expect(s2.right).toBeGreaterThan(100);
  // Nothing flanking it.
  expect(Math.abs((await evaluate(`edges('s3')`)).centre)).toBeLessThanOrEqual(1);
  // A page's title stays leading: right after the Back, with its end free.
  const p1 = await evaluate(`edges('p1')`);
  expect(p1.left).toBe(16 + 60);
  // A header flowing its own items (space-between, the title last) and a title with its own margin are the author's.
  expect((await evaluate(`edges('s4')`)).right).toBe(16);
  expect((await evaluate(`edges('s5')`)).left).toBe(16 + 60);
  // Centred in its own text where it fills the space (a title the author grew).
  expect(await evaluate(`document.getElementById('s3-title').style.flexGrow = '1'; getComputedStyle(document.getElementById('s3-title')).textAlign`)).toBe('center');
}, bars), 60000);

check('an app bar Back control remains live inside a modal beneath an inert ancestor', () => inChrome(async evaluate => {
  await evaluate(`
    const nav = document.getElementById('nav'), dialog = document.createElement('dialog');
    nav.before(dialog); dialog.append(nav); dialog.showModal();
    document.getElementById('exact-root').inert = true;
    select('3');
  `);
  expect(await evaluate(`press('r3'); calls.splice(0)`)).toEqual(['pressed r3']);
  expect(await evaluate(`document.querySelector('#r3 #back').inert = true; press('r3'); calls.splice(0)`))
    .toEqual(['log app bar: Back refused: its id="back" control is disabled, inert or not shown']);
  expect(await evaluate(`document.querySelector('#r3 #back').inert = false; document.querySelector('dialog').inert = true; press('r3'); calls.splice(0)`))
    .toEqual(['log app bar: Back refused: its id="back" control is disabled, inert or not shown']);
}), 60000);
