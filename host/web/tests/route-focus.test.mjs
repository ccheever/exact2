// LLP 1115.000 D4 part A in a real browser: a route is a page, so the focus
// follows it. The shell's stylesheet, the navigation glue's projection
// (navigation.js) and the input piece (input-glue.js), which both web targets
// run, over nodes as the hosts write them and a stand-in runner: each visit a
// route, each change projected and then `arrive`d, as each target does after
// a commit's tree, its focus commands and its `autofocus`. Presses are real
// ones (CDP mouse and key events), since a pointer's press and a key's differ
// in what the browser focuses and whether it draws the ring.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { cdpBrowser, chromium, suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { readyPage } from './page.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { unavailable } = chromium();
const check = need('chrome', unavailable);

// A root with two tabs. The first tab's root route is a list in a scroller:
// rows that push a detail (a button, a link, each drawing the disclosure, so
// a wide window splits), a button that opens a sheet, and one that pushes a
// route with no header. `window.then` is what the pressed control's action
// does besides navigating, before the commit ends (a `focus()` command, an
// `autofocus` field).
const page = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root" style="height:100%">
  <main id="nav" navigationKey="list" navigationBack="back" style="display:flex;flex-direction:column;height:100%">
    <div role="tabpanel" id="p1" style="flex:1;position:relative">
      <div navigationKey="list" id="list" style="position:absolute;inset:0;display:flex;flex-direction:column">
        <header><h1 id="list-title">Books</h1><button id="add">Add</button><input id="search" data-native></header>
        <div id="scroller" data-exact-list style="flex:1;min-height:0;overflow-y:auto">
          <button id="row-1" data-exact-router-press="true" style="height:60px"><img data-symbol-source="symbol:forward-chevron" alt="">One</button>
          <a id="row-2" data-view href="/item/2" style="height:60px"><div><img data-symbol-source="symbol:forward-chevron" alt="">Two</div></a>
          <button id="bare" data-exact-router-press="true" style="height:60px">Bare</button>
          <div style="height:2000px"></div>
          <button id="row-far" data-exact-router-press="true" style="height:60px">Far</button>
        </div>
        <div id="menu" role="menu" popover="auto"><button id="m-1">Open</button><button id="m-2" disabled>Share</button><a id="m-3" href="#x">Copy</a><button id="m-4">Remove</button></div>
        <div id="chooser" role="alertdialog" aria-label="Open in" popover="auto"><button id="c-1" data-exact-on="press">Maps</button><button id="c-2" data-exact-on="press">Google Maps</button><button id="c-cancel">Cancel</button></div>
        <div id="bar-menu" role="menu" popover="auto"><button id="b-1">Bump</button><button id="b-open-in" popovertarget="chooser" popovertargetaction="show">Open in…</button></div>
        <button id="chooser-open" popovertarget="chooser">Open in</button>
      </div>
    </div>
    <div role="tabpanel" id="p2" style="flex:1;position:relative">
      <div navigationKey="other" id="other" style="position:absolute;inset:0"><header><h1>Stats</h1></header><button id="in-other">Goal</button></div>
    </div>
    <div role="tablist" id="tabs"><button role="tab" id="tab-1" aria-controls="p1" aria-selected="true">Books</button><button role="tab" id="tab-2" aria-controls="p2">Stats</button></div>
  </main>
</div>
<script type="module">
  import { navigation, inertAncestor, sheetLeaves } from './navigation.js';
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root'), nav = document.getElementById('nav'), panel = document.getElementById('p1');
  const stack = window.stack = [['list', '/']];
  let input = null;
  window.then = null;
  // One commit: the tree's attributes, the projection, what the action focused, then the focus follows the route.
  const show = window.show = (key = stack.at(-1)[0], removed = []) => {
    nav.setAttribute('navigationKey', key);
    if (key !== 'other') navigation.apply({ top: stack.length - 1, url: stack.at(-1)[1], removed, visits: stack });
    navigation.project(root, () => {});
    const after = window.then; window.then = null; after?.();
    input?.arrive();
  };
  window.push = (key, url, { sheet = false, heading = 'Detail ' + key, field = false, role = null } = {}) => {
    const route = document.createElement('div');
    route.setAttribute('navigationKey', key); route.id = 'route-' + key; route.style.cssText = 'position:absolute;inset:0';
    if (sheet) route.setAttribute('navigationPresentation', 'modal');
    if (role) route.setAttribute('role', role);
    route.innerHTML = (heading ? '<header>' + (sheet ? '<button id="back">Cancel</button>' : '') + '<h2 id="title-' + key + '">' + heading + '</h2></header>' : '') + '<button id="in-' + key + '">Inside</button>' + (field ? '<input id="field-' + key + '" data-native>' : '');
    route.querySelector('#back')?.addEventListener('click', () => pop());
    panel.append(route); stack.push([key, url]); show();
  };
  window.pop = () => { const [key] = stack.pop(), route = document.getElementById('route-' + key); if (!sheetLeaves(route)) route.remove(); show(undefined, [stack.length]); };
  window.replace = (key, url) => { const [old] = stack.pop(); document.getElementById('route-' + old).remove(); stack.push(['x', url]); stack.pop(); push(key, url); };
  navigation.connect(root, () => false, () => {}, () => pop(), () => {}, href => href.startsWith("/item/"));
  document.getElementById('row-1').addEventListener('click', () => push('1', '/item/1'));
  // A link to a route is followed from the document's capture phase, as both targets follow one (input-glue.js,
  // router.js): its commit lands before any listener nearer the link has heard the click.
  document.addEventListener('click', e => { if (e.target.closest?.('#row-2')) { e.preventDefault(); push('2', '/item/2'); } }, true);
  document.getElementById('row-far').addEventListener('click', () => push('far', '/item/far'));
  document.getElementById('bare').addEventListener('click', () => push('bare', '/bare', { heading: '' }));
  document.getElementById('add').addEventListener('click', () => push('add', '/add', { sheet: true, heading: 'New Book', field: window.withField }));
  document.getElementById('tab-2').addEventListener('click', () => show('other'));
  document.getElementById('tab-1').addEventListener('click', () => show());
  show();
  input = createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor, dispatch() {} });
  input.arrive(); // the stack as it stands when the piece arrives, as each target calls it
  // Where the focus is: the element's id, whether the browser draws its ring, and the page's and the list's scroll.
  window.focus = () => { const a = document.activeElement; return [a === document.body ? 'body' : a.id, a.matches(':focus-visible'), document.scrollingElement.scrollTop, document.getElementById('scroller').scrollTop]; };
  window.ready = true;
</script>`;

/** The page and the glue it imports, served on a local port. */
async function serve() {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    const file = ['navigation.js', 'input-glue.js', 'prompt-glue.js', 'alert-message.js'].find(name => req.url === '/' + name);
    if (!file) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, file)));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  return server;
}

/** The page in headless Chrome at `width`: `test(evaluate, press, key)` with a real click on a node's middle and a real key. */
async function inChrome(test, width = 600, motion = false) {
  const server = await serve();
  const suite = await cdpBrowser({ args: [`--window-size=${width},900`] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await call('Emulation.setDeviceMetricsOverride', { width, height: 900, deviceScaleFactor: 1, mobile: false });
    // No motion unless a check asks for it: a press lands where a sheet's control is at once (neutral-look.test.mjs
    // holds the motion).
    await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: motion ? 'no-preference' : 'reduce' }] });
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    const press = async (id) => {
      const at = await evaluate(`(() => { const r = document.getElementById(${JSON.stringify(id)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
      for (const type of ['mousePressed', 'mouseReleased']) await call('Input.dispatchMouseEvent', { type, button: 'left', clickCount: 1, ...at });
    };
    const key = async (name, code = name, vk = 0, text) => {
      await call('Input.dispatchKeyEvent', { type: 'keyDown', key: name, code, windowsVirtualKeyCode: vk, ...(text ? { text } : {}) });
      await call('Input.dispatchKeyEvent', { type: 'keyUp', key: name, code, windowsVirtualKeyCode: vk });
    };
    await test(evaluate, press, key);
  } finally {
    await suite.close();
    server.close();
  }
}
const enter = (key) => key('Enter', 'Enter', 13, '\r');

check('on a push the focus moves to the new route\'s heading, and on Back to the row that was pressed', () => inChrome(async (evaluate, press, key) => {
  // A pointer's press: the heading takes the focus (so the next Tab starts in the new route, and a screen reader reads
  // its title) with no ring, since the person is not using the keyboard; nothing scrolled.
  await press('row-1');
  expect(await evaluate('focus()')).toEqual(['title-1', false, 0, 0]);
  expect(await evaluate(`[document.getElementById('title-1').getAttribute('tabindex'), document.getElementById('list').inert]`)).toEqual(['-1', true]);
  // Back (the app's own, the app bar's or the browser's: each a commit that pops) returns it to the row.
  expect(await evaluate('pop(); focus()')).toEqual(['row-1', false, 0, 0]);
  // The heading was only made focusable for that arrival.
  await press('row-1');
  expect(await evaluate(`pop(); push('again', '/again'); document.getElementById('title-1')`)).toBe(null);
  expect(await evaluate(`document.activeElement.id + ' ' + document.querySelectorAll('[tabindex="-1"]').length`)).toBe('title-again 1');
  await evaluate('pop()');

  // A key's press on a link row: the same move, and the browser draws the ring, as a keyboard user expects.
  await evaluate(`document.getElementById('row-2').focus()`);
  await enter(key);
  expect(await evaluate('focus()')).toEqual(['title-2', true, 0, 0]);
  expect(await evaluate('pop(); focus()')).toEqual(['row-2', true, 0, 0]);
  // And a pointer's press on it: no ring, and Back returns to the link. (Clicked while it still showed the key's
  // ring, the heading would show one too: the browser hands a script's focus the ring its last focus had.)
  await evaluate('document.activeElement.blur()');
  await press('row-2');
  expect(await evaluate('focus()')).toEqual(['title-2', false, 0, 0]);
  expect(await evaluate('pop(); focus()')).toEqual(['row-2', false, 0, 0]);

  // A route with no heading takes the focus itself.
  await press('bare');
  expect(await evaluate('focus().slice(0, 2)')).toEqual(['route-bare', false]);
  await evaluate('pop()');

  // Back to a row that has since scrolled out of sight does not scroll the list to it: the page is as it was left.
  await evaluate(`document.getElementById('scroller').scrollTop = 5000`);
  await press('row-far');
  expect(await evaluate(`document.getElementById('scroller').scrollTop = 0; pop(); focus()`)).toEqual(['row-far', false, 0, 0]);
}), 60000);

check('a focus the commit set is left alone, and another tab is no arrival', () => inChrome(async (evaluate, press) => {
  // The pressed control's action focuses a field in the new route (an `autofocus`, a `focus()` command): it stays.
  await evaluate(`withField = true; then = () => document.getElementById('field-add').focus()`);
  await press('add');
  expect(await evaluate('focus()[0]')).toBe('field-add');
  // Closing the sheet from that field returns the focus to the button that opened it.
  expect(await evaluate('pop(); focus()[0]')).toBe('add');
  // An action that focuses something outside the new route is left alone too.
  await evaluate(`then = () => document.getElementById('tab-2').focus()`);
  await press('row-1');
  expect(await evaluate('focus()[0]')).toBe('tab-2');
  // And on Back, a focus the commit set wins over the row.
  expect(await evaluate(`then = () => document.getElementById('search').focus(); pop(); focus()[0]`)).toBe('search');

  // A replace arrives as a push does, and keeps where its route came from.
  await press('row-1');
  expect(await evaluate(`replace('9', '/item/9'); focus()[0]`)).toBe('title-9');
  expect(await evaluate('pop(); focus()[0]')).toBe('row-1');

  // Another tab's stack is not a push: the focus stays on the tab that was pressed, and coming back is the same.
  await press('tab-2');
  expect(await evaluate('focus()[0]')).toBe('tab-2');
  await press('tab-1');
  expect(await evaluate('focus()[0]')).toBe('tab-1');

  // A route that arrives with no press (a timer's push, the browser's Forward) takes the focus only when nothing has
  // it, never from where the person put it. (The push and Back first spend the tab's press.)
  expect(await evaluate(`push('z', '/z'); pop(); focus()[0]`)).toBe('tab-1');
  expect(await evaluate(`push('t', '/timer'); focus()[0]`)).toBe('tab-1');
  expect(await evaluate(`pop(); document.activeElement.blur(); push('t2', '/timer'); focus()[0]`)).toBe('title-t2');
}), 60000);

check('a modal route is a dialog to assistive technology: the focus moves in, and returns on close', () => inChrome(async (evaluate, press, key) => {
  // LLP 1115.000 D4.5. The sheet is `role="dialog"` and `aria-modal`, named by its header's heading; the focus is on
  // that heading, inside it, and the route it covers is inert.
  const sheet = id => `(r => [r.getAttribute('role'), r.getAttribute('aria-modal'), r.getAttribute('aria-labelledby'), r.getAttribute('data-exact-dialog')])(document.getElementById('route-${id}'))`;
  await press('add');
  expect(await evaluate(sheet('add'))).toEqual(['dialog', 'true', 'title-add', 'named']);
  expect(await evaluate(`[...focus().slice(0, 2), document.getElementById('route-add').contains(document.activeElement), document.getElementById('list').inert]`)).toEqual(['title-add', false, true, true]);
  // It is modal: the root's tab bar is inert beneath it too, and live again when it closes.
  expect(await evaluate(`document.getElementById('tabs').inert`)).toBe(true);
  // A heading with no id of its own is given one to be named by.
  expect(await evaluate(`document.getElementById('title-add').removeAttribute('id'); show(); ${sheet('add')}`)).toEqual(['dialog', 'true', 'exact-title-add', 'named']);
  expect(await evaluate(`document.getElementById('exact-title-add').textContent`)).toBe('New Book');
  // Its Cancel closes it (so does Escape, navigation.js's), and the focus returns to the button that opened it.
  await press('back');
  expect(await evaluate(`[focus()[0], stack.length, document.getElementById('tabs').inert]`)).toEqual(['add', 1, false]);
  await evaluate(`document.getElementById('add').focus()`);
  await enter(key);
  expect(await evaluate('focus().slice(0, 2)')).toEqual(['title-add', true]);
  await key('Escape', 'Escape', 27);
  expect(await evaluate('[...focus().slice(0, 2), stack.length]')).toEqual(['add', true, 1]);

  // What the author wrote wins: a role of their own, or a label, is left as written; a sheet with no heading has no name.
  expect(await evaluate(`push('own', '/own', { sheet: true, role: 'region' }); ${sheet('own')}`)).toEqual(['region', null, null, null]);
  expect(await evaluate(`pop(); push('said', '/said', { sheet: true }); document.getElementById('route-said').setAttribute('aria-label', 'Said'); show(); ${sheet('said')}`)).toEqual(['dialog', 'true', null, '']);
  expect(await evaluate(`pop(); push('bare', '/bare', { sheet: true, heading: '' }); ${sheet('bare')}`)).toEqual(['dialog', 'true', null, '']);
  // A route that is no longer modal is no longer a dialog, and a pushed route never was.
  expect(await evaluate(`document.getElementById('route-bare').removeAttribute('navigationPresentation'); show(); ${sheet('bare')}`)).toEqual([null, null, null, null]);
  expect(await evaluate(`pop(); push('1', '/item/1'); ${sheet('1')}`)).toEqual([null, null, null, null]);
}), 60000);

check('a tablist\'s tabs and a menu\'s items move by the arrow keys', () => inChrome(async (evaluate, press, key) => {
  const arrow = name => key(name, name, { ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40, Home: 36, End: 35 }[name]);
  const after = async (name) => { await arrow(name); return evaluate('document.activeElement.id + " " + nav.getAttribute("navigationKey")'); };
  // Left and Right along a tablist, wrapping, with Home and End: the focus moves and nothing is pressed (the selected
  // route is still the list's), so Enter or Space chooses.
  await evaluate(`document.getElementById('tab-1').focus()`);
  expect(await after('ArrowRight')).toBe('tab-2 list');
  expect(await after('ArrowRight')).toBe('tab-1 list');
  expect(await after('ArrowLeft')).toBe('tab-2 list');
  expect(await after('Home')).toBe('tab-1 list');
  expect(await after('End')).toBe('tab-2 list');
  expect(await after('ArrowDown')).toBe('tab-2 list'); // not this bar's axis: the page's own key
  await enter(key);
  expect(await evaluate('document.activeElement.id + " " + nav.getAttribute("navigationKey")')).toBe('tab-2 other');
  // A tablist is one Tab stop: the tab the focus is on, else its selected tab.
  await key('Tab', 'Tab', 9);
  expect(await evaluate(`[document.getElementById('tab-1').tabIndex, document.getElementById('tab-2').tabIndex]`)).toEqual([-1, 0]);
  await press('tab-1');
  await evaluate(`document.getElementById('tab-1').setAttribute('aria-selected', 'true'); document.getElementById('add').focus()`);
  await key('Tab', 'Tab', 9);
  expect(await evaluate(`[document.getElementById('tab-1').tabIndex, document.getElementById('tab-2').tabIndex]`)).toEqual([0, -1]);

  // Up and Down in a menu, past a disabled item, wrapping, with Home and End; and into an open menu from outside it.
  await evaluate(`document.getElementById('menu').showPopover(); document.getElementById('m-1').focus()`);
  expect(await after('ArrowDown')).toBe('m-3 list');
  expect(await after('ArrowDown')).toBe('m-4 list');
  expect(await after('ArrowDown')).toBe('m-1 list');
  expect(await after('ArrowUp')).toBe('m-4 list');
  expect(await after('Home')).toBe('m-1 list');
  expect(await after('End')).toBe('m-4 list');
  expect(await after('ArrowRight')).toBe('m-4 list'); // not a menu's axis
  await evaluate(`document.getElementById('add').focus()`);
  expect(await after('ArrowDown')).toBe('m-1 list');
  await evaluate(`document.getElementById('add').focus()`);
  expect(await after('ArrowUp')).toBe('m-4 list');
  // With the menu closed, and in a field, the arrows are the page's own.
  await evaluate(`document.getElementById('menu').hidePopover(); document.getElementById('search').focus()`);
  expect(await after('ArrowDown')).toBe('search list');
  // A chooser its button opened is drawn as a menu and moves as one, from its button into it, its hidden cancel no stop.
  await evaluate(`document.getElementById('chooser-open').focus()`);
  await enter(key);
  expect(await evaluate(`document.getElementById('chooser').matches(':popover-open[data-exact-chooser]')`)).toBe(true);
  expect(await after('ArrowDown')).toBe('c-1 list');
  expect(await after('ArrowDown')).toBe('c-2 list');
  expect(await after('ArrowDown')).toBe('c-1 list');
  expect(await after('End')).toBe('c-2 list');
  expect(await after('Home')).toBe('c-1 list');
  await evaluate(`document.getElementById('chooser').hidePopover()`);
  // One a menu's row opens (an app bar's "Open in…") stands over the menu, which stays open beneath it, the row being
  // its invoker. The keys go to the chooser on top, the last to open, whichever of the two is written first: Down, Up,
  // Home and End from the row, and Down from the page, where WebKit leaves the focus after a click.
  for (const written of ['first', 'last']) {
    if (written === 'last') await evaluate(`document.getElementById('bar-menu').after(document.getElementById('chooser'))`);
    await evaluate(`document.getElementById('bar-menu').showPopover(); document.getElementById('b-open-in').focus()`);
    await enter(key);
    const seen = [await evaluate(`[...document.querySelectorAll(':popover-open')].map(p => p.id + (p.hasAttribute('data-exact-chooser') ? ' chooser' : '')).sort()`)];
    for (const name of ['ArrowDown', 'ArrowUp', 'Home', 'End']) { await evaluate(`document.getElementById('b-open-in').focus()`); seen.push(await after(name)); }
    await evaluate(`document.activeElement.blur()`);
    seen.push(await after('ArrowDown'));
    expect([written, ...seen]).toEqual([written, ['bar-menu', 'chooser chooser'], 'c-1 list', 'c-2 list', 'c-1 list', 'c-2 list', 'c-1 list']);
    await evaluate(`document.getElementById('chooser').hidePopover(); document.getElementById('bar-menu').hidePopover()`);
  }
}), 60000);

check('a root tab bar drawn as a sidebar moves by Up and Down', () => inChrome(async (evaluate, press, key) => {
  expect(await evaluate(`nav.hasAttribute('data-exact-sidebar')`)).toBe(true);
  await evaluate(`document.getElementById('tab-1').focus()`);
  await key('ArrowRight', 'ArrowRight', 39);
  expect(await evaluate('document.activeElement.id')).toBe('tab-1');
  await key('ArrowDown', 'ArrowDown', 40);
  expect(await evaluate('document.activeElement.id')).toBe('tab-2');
  await key('ArrowUp', 'ArrowUp', 38);
  expect(await evaluate('document.activeElement.id')).toBe('tab-1');
}, 1200), 60000);

check('in a wide window\'s split the focus follows the detail and comes back to its row', () => inChrome(async (evaluate, press) => {
  // The list stays live beside its detail (LLP 1116.004 D1), so the pressed row still has the focus when the detail
  // arrives; it moves to the detail's heading all the same, and Back returns it.
  await press('row-1');
  expect(await evaluate(`[document.getElementById('list').getAttribute('data-exact-split'), document.getElementById('list').inert, ...focus()]`)).toEqual(['list', false, 'title-1', false, 0, 0]);
  expect(await evaluate('pop(); focus()[0]')).toBe('row-1');
  // A link row is the same: the click focuses the link, and the detail's heading takes the focus from it.
  await press('row-2');
  expect(await evaluate('focus().slice(0, 2)')).toEqual(['title-2', false]);
  expect(await evaluate('pop(); focus()[0]')).toBe('row-2');
  // The list's own focus is left alone when the detail leaves while the person is in the list.
  await press('row-1');
  expect(await evaluate(`document.getElementById('search').focus(); pop(); focus()[0]`)).toBe('search');
}, 1200), 60000);

check('the agent\'s reveal leaves what a tap refuses where it is', () => inChrome(async (evaluate, press) => {
  // Before a tap the driver scrolls its target into view (input-glue.js `reveal`), as a person would. Under a sheet the
  // list is inert: nobody can scroll or press it, the tap is refused, and a refusal leaves the page as it was. (Shelf in
  // conformance: a refused tap on a covered row's swipe action left the row scrolled to it, on the page that was asked.)
  const reveal = `import('./input-glue.js').then(m => [m.reveal(document.getElementById('row-far'), 1).scrolled, document.getElementById('scroller').scrollTop > 0])`;
  await press('add');
  expect(await evaluate(`document.getElementById('list').inert`)).toBe(true);
  expect(await evaluate(reveal)).toEqual([false, false]);
  // Nor one that is hidden, which a tap refuses the same way.
  expect(await evaluate(`pop(); document.getElementById('row-far').style.visibility = 'hidden'; ${reveal}`)).toEqual([false, false]);
  // A row a person could reach is scrolled to, as before.
  expect(await evaluate(`document.getElementById('row-far').style.visibility = ''; ${reveal}`)).toEqual([true, true]);
}), 60000);

check('Back returns focus inside a modal beneath an inert ancestor', () => inChrome(async (evaluate, press) => {
  await evaluate(`
    const dialog = document.createElement('dialog');
    dialog.style.cssText = 'width:500px;height:700px';
    nav.before(dialog); dialog.append(nav); dialog.showModal();
    document.getElementById('exact-root').inert = true;
  `);
  await press('row-1');
  expect(await evaluate('focus()[0]')).toBe('title-1');
  expect(await evaluate('pop(); focus()[0]')).toBe('row-1');
  await press('row-1');
  expect(await evaluate(`document.getElementById('row-1').inert = true; pop(); focus()[0]`)).toBe('list-title');
}), 60000);

// A sheet sliding away (navigation.js `sheetLeaves`) is no route: one that opens from another button while it leaves,
// and closes, gives the focus back to that button.
check('a sheet sliding away is no route to the focus', () => inChrome(async (evaluate, press) => {
  const risen = (id) => evaluate(`document.getElementById('route-${id}').getAnimations().forEach(a => a.finish())`);
  await press('add');
  await risen('add');
  await press('back');
  // It is still leaving (held), when another button opens a sheet.
  expect(await evaluate(`(() => { const g = document.querySelector('[data-exiting]'); g.getAnimations().forEach(a => a.pause()); return g.getAttribute('navigationPresentation'); })()`)).toBe('modal');
  await evaluate(`{ const b = Object.assign(document.createElement('button'), { id: 'add-2', textContent: 'Another' }); b.addEventListener('click', () => push('add-2', '/add-2', { sheet: true, heading: 'Another' })); document.querySelector('#list header').append(b); }`);
  await press('add-2');
  await risen('add-2');
  await press('back');
  expect(await evaluate('focus()[0]')).toBe('add-2');
}, 600, true), 60000);

// A sheet closed with the focus inside it and no press there (the browser's Back, the app's own close) gives the focus
// back to the button that opened it as it slides away, as it does when it is gone at once.
check('a sheet that slides away with the focus in it gives the focus back', () => inChrome(async (evaluate, press) => {
  await evaluate('window.withField = true');
  await press('add');
  await evaluate("document.getElementById('field-add').focus()");
  expect(await evaluate(`pop(); [focus()[0], document.getElementById('route-add')?.hasAttribute('data-exiting')]`)).toEqual(['add', true]);
}, 600, true), 60000);

// A focus the opener's own focus handler moves on as the focus comes back (an app's `focus` action, whose commands run
// as it is dispatched) is left where the handler put it.
check('a focus the opener moves on as a sheet closes stays where it moved', () => inChrome(async (evaluate, press) => {
  await evaluate("document.getElementById('add').addEventListener('focus', () => { if (window.armed) { window.armed = false; document.getElementById('search').focus(); } })");
  await press('add');
  await evaluate("document.getElementById('route-add').getAnimations().forEach(a => a.finish()); window.armed = true");
  await press('back');
  expect(await evaluate('[focus()[0], window.armed]')).toEqual(['search', false]);
}, 600, true), 60000);

// The focus in a frame or a shadow root inside the sheet, which a blur does not take from a frame in Firefox, comes
// back to the button too, in every engine.
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a sheet that slides away with the focus in a frame or a shadow root gives the focus back`, async () => {
  const server = await serve(), browser = await suiteBrowser(engine);
  try {
    const seen = [];
    for (const kind of ['frame', 'shadow field', 'shadow frame']) {
      const context = await browser.newContext({ viewport: { width: 600, height: 900 }, reducedMotion: 'no-preference' }), tab = await context.newPage();
      await tab.goto(`http://127.0.0.1:${server.address().port}/`);
      await tab.waitForFunction('window.ready === true');
      await tab.click('#add');
      await tab.evaluate(async (kind) => {
        const sheet = document.getElementById('route-add'), inner = document.createElement(kind.endsWith('frame') ? 'iframe' : 'input');
        sheet.getAnimations().forEach(a => a.finish());
        if (inner.localName === 'iframe') inner.srcdoc = '<input id="in-frame">';
        if (kind.startsWith('shadow')) { const view = document.createElement('photo-editor'); view.attachShadow({ mode: 'open' }).append(inner); sheet.append(view); } else sheet.append(inner);
        if (inner.localName === 'iframe') { await new Promise(r => { inner.onload = r; }); inner.focus(); inner.contentDocument.getElementById('in-frame').focus(); } else inner.focus();
      }, kind);
      const now = await tab.evaluate(() => { pop(); return focus()[0]; });
      await tab.waitForTimeout(500);
      seen.push([kind, now, await tab.evaluate(() => focus()[0])]);
      await context.close();
    }
    expect([engine, seen]).toEqual([engine, [['frame', 'add', 'add'], ['shadow field', 'add', 'add'], ['shadow frame', 'add', 'add']]]);
  } finally { await browser.close(); server.close(); }
}, 90000);
