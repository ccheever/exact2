// LLP 1116.000 D4 in a real browser: the shell's stylesheet and the
// navigation glue's projection (navigation.js, which both web targets run)
// over nodes as the hosts write them. A pushed route's app bar draws Back as
// iOS draws UIKit's (a route above its stack's root, not a sheet, whose header
// does not hold the root's `navigationBack` control), and a block `link` of
// text is in the accent with the pointing hand while an authored colour, a
// row inside it, a `role` or a `buttonStyle` keeps the author's look.
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

// A root with two tabs. The first tab's stack: a root, a pushed route, one
// whose Back control is in its content, and a sheet. The second's: a root
// with a long title, a pushed route, and one whose header holds its control.
const page = readFileSync(resolve(WEB, 'index.html'), 'utf8').match(/<style>[\s\S]*?<\/style>/)[0] + `
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
          <div data-exact-text>An <a data-view href="/x" id="l-run" style="color:LinkText">inline run</a>.</div>
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

/** The page in headless Chrome: `test(evaluate, call)` with its `Runtime.evaluate` and its CDP session. */
async function inChrome(test) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'navigation.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-app-bar-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
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
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    await test(evaluate, call);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
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
  expect(await evaluate(`getComputedStyle(document.querySelector('#r2 > header').shadowRoot.querySelector('button')).color`)).toBe('rgb(0, 117, 255)');

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
}), 30000);

check('a block link of text is in the accent and points; the author\'s look wins', () => inChrome(async (evaluate) => {
  // A link of text (or a symbol and text) is in the accent, the app's when it names one; an authored colour, a row
  // inside it, a role or a buttonStyle keeps the author's look; an inline run keeps its LinkText row. Every link points.
  expect(await evaluate(`['l-text', 'l-symbol', 'l-authored', 'l-row', 'l-role', 'l-style', 'l-accent', 'l-run'].map(id => { const s = getComputedStyle(document.getElementById(id)); return id + ' ' + s.color + ' ' + s.cursor + ' ' + s.display + ' ' + s.textDecorationLine; })`)).toEqual([
    'l-text rgb(0, 117, 255) pointer block none',
    'l-symbol rgb(0, 117, 255) pointer block none',
    'l-authored rgb(204, 0, 0) pointer block none',
    'l-row rgb(1, 2, 3) pointer block none',
    'l-role rgb(1, 2, 3) pointer block none',
    'l-style rgb(1, 2, 3) pointer block none',
    'l-accent rgb(10, 20, 30) pointer block none',
    'l-run rgb(0, 0, 238) pointer inline none',
  ]);
}), 30000);
