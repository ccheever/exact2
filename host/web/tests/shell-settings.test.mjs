// LLP 1115.000 D4.11, D4.12 and D4.14 in a real browser: what the shell's stylesheet (index.html, both web
// targets) and the navigation glue do whatever the look, because none of it is taste. A material the host
// paints honours the person's settings: with Reduce Transparency the bar is opaque and does not blur, and
// under forced colours each drawn control is the browser's own again; an effect the author wrote is not
// touched. The shell carries the resets a careful web app ships (text inflation, the double-tap zoom on a
// control, a scroller's end pulling the page). A symbol, and an image in something pressable, is not
// dragged out as a picture; the select's chevron and the switch's thumb follow the page's direction; and a
// plain `dialog` has no padding of the user agent's. The nodes are as the hosts write them.
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

const page = readFileSync(resolve(WEB, 'index.html'), 'utf8').match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root">
  <main id="nav" navigationKey="1" navigationBack="back" style="display:flex;flex-direction:column;height:300px">
    <div role="tabpanel" id="p1">
      <div navigationKey="1" id="r1"><header id="bar"><h1>Inbox</h1></header></div>
      <div navigationKey="2" id="r2"><header id="bar-said" style="backdrop-filter:blur(4px);background-color:rgba(1, 2, 3, 0.5)"><h2>Said</h2></header></div>
    </div>
    <div role="tablist" id="tabs"><button type="button" role="tab" aria-controls="p1" aria-selected="true" data-native data-button-style="bordered" id="tab">Inbox</button></div>
  </main>
  <div role="radiogroup" id="group"><button type="button" role="radio" aria-checked="true" data-native data-button-style="bordered" id="chosen">Day</button><button type="button" role="radio" aria-checked="false" data-native data-button-style="bordered" id="other">Week</button></div>
  <button type="button" data-native data-button-style="bordered" id="button" style="display:grid"><img alt="" data-symbol-source="symbol:add" id="symbol"><span data-exact-text>Add</span></button>
  <button type="button" id="bare"><img alt="" id="in-button"></button>
  <a data-view href="/x" id="link"><img alt="" id="in-link"></a>
  <div data-exact-on="press" id="pressable"><img alt="" id="in-pressable"></div>
  <div id="plain"><img alt="" id="content"></div>
  <div data-exact-on="press" id="panned" style="touch-action:none"></div>
  <input data-native id="field"><textarea data-native id="area"></textarea>
  <select id="select"><option>One</option></select>
  <input type="checkbox" switch data-exact-switch id="switch"><input type="checkbox" switch data-exact-switch checked id="switch-on">
  <progress value="40" max="100" id="progress"></progress><div data-exact-progress id="spinner" style="display:grid;width:20px;height:20px"></div>
  <div dir="rtl"><select id="rtl-select"><option>One</option></select><input type="checkbox" switch data-exact-switch id="rtl-switch"><input type="checkbox" switch data-exact-switch checked id="rtl-switch-on"></div>
  <dialog id="dialog" open><div data-exact-text>Its own box</div></dialog>
  <dialog id="alert" role="alertdialog" aria-label="Sure?" open><div data-exact-text>It cannot be undone.</div></dialog>
</div>
<script type="module">
  import { navigation } from './navigation.js';
  const root = document.getElementById('exact-root');
  navigation.connect(root, () => false, () => {});
  navigation.project(root, () => {});
  window.drags = [];
  addEventListener('dragstart', event => drags.push(event.target.id + ':' + event.defaultPrevented));
  window.drag = id => { document.getElementById(id).dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true })); return drags.splice(0); };
  window.css = (id, property, pseudo) => getComputedStyle(document.getElementById(id), pseudo)[property];
  window.ready = true;
</script>`;

/** The page in headless Chrome: `test(evaluate, call)` with its `Runtime.evaluate` and its CDP session. */
async function inChrome(test) {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'navigation.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-shell-settings-'));
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
const css = (evaluate, id, property, pseudo) => evaluate(`css(${JSON.stringify(id)}, ${JSON.stringify(property)}${pseudo ? `, ${JSON.stringify(pseudo)}` : ''})`);
const prefer = (call, features) => call('Emulation.setEmulatedMedia', { features: Object.entries(features).map(([name, value]) => ({ name, value })) });
const opaque = (color) => /^rgb\(/.test(color);

check('a material the host paints honours the person\'s settings: opaque with Reduce Transparency, the browser\'s own controls under forced colours', () => inChrome(async (evaluate, call) => {
  // As a person leaves it: a material that blurs what scrolls under it.
  expect(await css(evaluate, 'bar', 'backdropFilter')).toContain('blur(20px)');
  expect(opaque(await css(evaluate, 'bar', 'backgroundColor'))).toBe(false);
  expect(opaque(await css(evaluate, 'tabs', 'backgroundColor'))).toBe(false);
  await prefer(call, { 'prefers-reduced-transparency': 'reduce' });
  expect(await css(evaluate, 'bar', 'backdropFilter')).toBe('none');
  expect(await css(evaluate, 'bar', 'backgroundColor')).toBe('rgb(249, 249, 249)'); // the material's own colour, whole
  expect(await css(evaluate, 'tabs', 'backgroundColor')).toBe('rgb(249, 249, 249)');
  // The author's own effect on a header is the author's.
  expect(await css(evaluate, 'bar-said', 'backdropFilter')).toBe('blur(4px)');
  expect(await css(evaluate, 'bar-said', 'backgroundColor')).toBe('rgba(1, 2, 3, 0.5)');
  await prefer(call, {});

  // Forced colours. Drawn, as a person leaves it: no border on a button, the switch and the select the page's own boxes.
  expect(await css(evaluate, 'button', 'borderTopStyle')).toBe('none');
  expect([await css(evaluate, 'switch', 'appearance'), await css(evaluate, 'switch', 'content', '::before')]).toEqual(['none', '""']);
  expect(await css(evaluate, 'select', 'appearance')).toBe('none');
  expect(await css(evaluate, 'dialog', 'borderTopStyle')).toBe('none');
  await prefer(call, { 'forced-colors': 'active' });
  // A button and a dialog have the user agent's border, so an edge the theme can paint.
  for (const id of ['button', 'dialog']) expect([id, await css(evaluate, id, 'borderTopStyle') !== 'none', parseFloat(await css(evaluate, id, 'borderTopWidth')) > 0]).toEqual([id, true, true]);
  // The switch, the select and the progress bar are the browser's controls.
  expect([await css(evaluate, 'switch', 'appearance'), await css(evaluate, 'switch', 'content', '::before')]).toEqual(['auto', 'none']);
  expect(await css(evaluate, 'select', 'appearance')).toBe('auto');
  expect(await css(evaluate, 'select', 'backgroundImage')).toBe('none');
  expect(await css(evaluate, 'progress', 'appearance')).toBe('auto');
  // The chosen segment and the selected tab are the system's selection, which their neighbours are not.
  for (const id of ['chosen', 'tab']) expect([id, await css(evaluate, id, 'forcedColorAdjust')]).toEqual([id, 'none']);
  expect(await css(evaluate, 'chosen', 'backgroundColor')).not.toBe(await css(evaluate, 'other', 'backgroundColor'));
  // The spinner, which no browser draws, is a ring of borders: a gradient would be gone.
  expect(await css(evaluate, 'spinner', 'backgroundImage', '::before')).toBe('none');
  expect(parseFloat(await css(evaluate, 'spinner', 'borderLeftWidth', '::before'))).toBeGreaterThan(0);
}));

check('the shell carries the resets a careful web app ships', () => inChrome(async (evaluate) => {
  // A phone does not inflate the text in landscape.
  expect(await evaluate('getComputedStyle(document.documentElement).textSizeAdjust')).toBe('100%');
  // A control answers a tap at once; content does not change, and a node that said its own keeps it.
  for (const id of ['button', 'bare', 'link', 'pressable', 'field', 'area', 'select', 'switch']) expect([id, await css(evaluate, id, 'touchAction')]).toEqual([id, 'manipulation']);
  expect(await css(evaluate, 'plain', 'touchAction')).toBe('auto');
  expect(await css(evaluate, 'panned', 'touchAction')).toBe('none');
  // A shell that scrolls inside itself keeps a scroller's end from the page; a `scroll document` page is the browser's.
  const ends = 'JSON.stringify([getComputedStyle(document.documentElement).overscrollBehaviorY, getComputedStyle(document.body).overscrollBehaviorY])';
  expect(await evaluate(ends)).toBe('["contain","contain"]');
  expect(await evaluate(`document.documentElement.setAttribute('data-scrolldocument', ''); ${ends}`)).toBe('["auto","auto"]');
}));

check('the small ones: a control\'s image is not dragged out, the chevron and the thumb follow the page\'s direction, a plain dialog has no padding', () => inChrome(async (evaluate) => {
  for (const id of ['symbol', 'in-button', 'in-link', 'in-pressable']) {
    expect([id, await css(evaluate, id, 'webkitUserDrag')]).toEqual([id, 'none']);
    // Where an engine has no such property, the drag that starts is cancelled.
    expect(await evaluate(`drag(${JSON.stringify(id)})`)).toEqual([`${id}:true`]);
  }
  // An image that is content is the browser's, as on any page.
  expect(await css(evaluate, 'content', 'webkitUserDrag')).toBe('auto');
  expect(await evaluate(`drag('content')`)).toEqual(['content:false']);

  // Left to right: room for the select's chevron at the right, where it is drawn; the switch's thumb starts at the left.
  expect([await css(evaluate, 'select', 'paddingLeft'), await css(evaluate, 'select', 'paddingRight')]).toEqual(['12px', '32px']);
  expect(await css(evaluate, 'select', 'backgroundPosition')).toMatch(/^calc\(100% - 16px\) 50%, calc\(100% - 12px\) 50%/);
  expect(await css(evaluate, 'switch', 'backgroundPosition', '::before')).toBe('0px 0px, 0px 1px');
  expect(await css(evaluate, 'switch-on', 'backgroundPosition', '::before')).toBe('16px 0px, 16px 1px');
  // Right to left: both mirrored.
  expect([await css(evaluate, 'rtl-select', 'paddingLeft'), await css(evaluate, 'rtl-select', 'paddingRight')]).toEqual(['32px', '12px']);
  expect(await css(evaluate, 'rtl-select', 'backgroundPosition')).toMatch(/^12px 50%, 16px 50%/);
  expect(await css(evaluate, 'rtl-switch', 'backgroundPosition', '::before')).toBe('16px 0px, 16px 1px');
  expect(await css(evaluate, 'rtl-switch-on', 'backgroundPosition', '::before')).toBe('0px 0px, 0px 1px');
  // A plain dialog's box is its author's; an alert keeps the padding the page gives it.
  expect(await css(evaluate, 'dialog', 'padding')).toBe('0px');
  expect(await css(evaluate, 'alert', 'padding')).toBe('20px 16px 16px');
}));
