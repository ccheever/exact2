// Held states in a real browser (LLP 1122 D8, LLP 1121 stage 8 E): the input
// glue's `hold`, which the build channel's `hold` and the agent's `holdState`
// call. `active` holds the press of the node the press resolves to (its paint,
// as a real press shows it); `hover` and `focus-visible` hold that node's look
// (`data-exact-held`, which the looks rules take); a disabled node refuses one;
// real input on the node, its disabling and its going let it go, said why.
import { expect, beforeAll, afterAll } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { readyPage } from './page.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { executable: chrome, unavailable } = chromium();
const check = need('chrome', unavailable);

const sheet = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>[\s\S]*?<\/style>/)[0];
const on = ':not(:disabled,[disabled]:not([disabled="false"]))';
// As `looks::rules` writes a `:hover` look and a `:focus-visible` one (a yellow face; a green one).
const rules = `@media (hover:hover){[data-view="3"]:hover${on}{background-color:rgba(255,255,0,1)!important;--exact-hover-wash:none!important;}}[data-view="3"][data-exact-held~=hover]${on}{background-color:rgba(255,255,0,1)!important;--exact-hover-wash:none!important;}`
  + `[data-view="3"]:focus-visible${on},[data-view="3"][data-exact-held~=focus-visible]${on}{background-color:rgba(0,255,0,1)!important;}`;
const press = '--exact-press-background-color:rgba(0,0,255,1);';
const page = sheet + `<style id="exact-looks">${rules}</style>
<div id="exact-root" style="padding:20px">
  <div id="card" data-view="3" data-exact-on="press" tabindex="0" style="${press}width:100px;height:100px;background-color:#ffffff">
    <span id="label">c</span>
  </div>
  <div id="plain" style="width:50px;height:50px">p</div>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  createInputHandlers({ root: document.getElementById('exact-root'), views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  window.dropped = []; window.ready = true;
</script>`;

let server, profile, child, cdp;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  profile = mkdtempSync(resolve(tmpdir(), 'exact-hold-'));
  child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  cdp = new Cdp(child.stdio[3], child.stdio[4]);
});
afterAll(() => {
  child?.kill();
  server?.close();
  if (profile) rmSync(profile, { recursive: true, force: true });
});

check('a held state is the real one, until real input, its disabling or its going lets it go', async () => {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  const evaluate = async (expression) => {
    const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  try {
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    const settle = () => evaluate(`(document.getAnimations().forEach(a => a.finish()), new Promise(r => requestAnimationFrame(() => r())))`);
    const face = async () => { await settle(); return evaluate(`getComputedStyle(document.getElementById('card')).backgroundColor`); };
    const hold = (id, state) => evaluate(`(r => ({ held: r.held ?? null, reason: r.reason ?? null, el: r.el?.id ?? null }))(exact.hold(document.getElementById('${id}'), ${JSON.stringify(state)}, reason => dropped.push(reason)))`);
    const dropped = () => evaluate('dropped.splice(0)');

    // `active`, named on the label inside: the press of the node it resolves to, its paint shown.
    expect(await hold('label', 'active')).toEqual({ held: 'active', reason: null, el: 'card' });
    expect(await evaluate(`['data-pressed', 'data-exact-painting'].map(a => document.getElementById('card').hasAttribute(a))`)).toEqual([true, true]);
    expect(await face()).toBe('color(srgb 0 0 1)');
    expect((await evaluate('exact.heldOf(document.getElementById("card"))')).held).toEqual({ state: 'active', rows: [{ name: 'press_background_color', value: 'rgba(0,0,255,1)' }] });
    // The element it was named on answers too, with the press's rows.
    expect((await evaluate('exact.heldOf(document.getElementById("label"))')).held.rows).toEqual([{ name: 'press_background_color', value: 'rgba(0,0,255,1)' }]);
    // A node no press reaches holds none, said why.
    expect((await hold('plain', 'active')).reason).toBe('gone');
    expect(await dropped()).toEqual(['replaced']);
    // `hover`: the look, by its held rule, the press's paint gone.
    expect(await hold('card', 'hover')).toEqual({ held: 'hover', reason: null, el: 'card' });
    expect(await face()).toBe('rgb(255, 255, 0)');
    expect((await evaluate('exact.heldOf(document.getElementById("card"))')).held.rows).toEqual([{ name: 'background_color', value: 'rgb(255, 255, 0)' }]);
    // Real input on the node lets it go.
    await evaluate(`document.getElementById('card').dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0, isPrimary: true }))`);
    expect(await dropped()).toEqual(['input']);
    expect(await evaluate(`document.getElementById('card').hasAttribute('data-exact-held')`)).toBe(false);
    await evaluate(`document.getElementById('card').dispatchEvent(new PointerEvent('pointerup', { bubbles: true, button: 0, isPrimary: true }))`);
    // `focus-visible`: its look; disabling the node lets it go; a disabled node refuses one.
    expect((await hold('card', 'focus-visible')).held).toBe('focus-visible');
    expect(await face()).toBe('rgb(0, 255, 0)');
    await evaluate(`document.getElementById('card').setAttribute('disabled', '')`);
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    expect(await dropped()).toEqual(['disabled']);
    expect((await hold('card', 'hover')).reason).toBe('disabled');
    await evaluate(`document.getElementById('card').removeAttribute('disabled')`);
    // A look held snaps, an authored transition or not, on and off.
    const now = () => evaluate(`getComputedStyle(document.getElementById('card')).backgroundColor`);
    await evaluate(`document.getElementById('card').style.transition = 'background-color 10s linear'`);
    expect((await hold('card', 'hover')).held).toBe('hover');
    expect(await now()).toBe('rgb(255, 255, 0)');
    // The pointer leaving the node lets a hover go; moving within it does not.
    const cross = (type, from, to) => evaluate(`document.getElementById('${from}').dispatchEvent(new PointerEvent('${type}', { bubbles: true, relatedTarget: document.getElementById('${to}') }))`);
    await cross('pointerout', 'label', 'card');
    expect(await dropped()).toEqual([]);
    await cross('pointerout', 'card', 'plain');
    expect(await dropped()).toEqual(['input']);
    expect(await now()).toBe('rgb(255, 255, 255)');
    await evaluate(`document.getElementById('card').style.transition = ''`);
    // A real press on the node as it is held ends there: its release leaves the held paint.
    await evaluate(`document.getElementById('card').dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0, isPrimary: true, pointerId: 7 }))`);
    expect((await hold('card', 'active')).held).toBe('active');
    await evaluate(`document.getElementById('card').dispatchEvent(new PointerEvent('pointerup', { bubbles: true, button: 0, isPrimary: true, pointerId: 7 }))`);
    expect(await evaluate(`document.getElementById('card').hasAttribute('data-pressed')`)).toBe(true);
    expect((await evaluate('exact.heldOf(document.getElementById("card"))')).held.state).toBe('active');
    // A dev loop's new generation lets it go unsaid (dev.js holds it again).
    await evaluate('exact.holdQuiet()');
    expect(await dropped()).toEqual([]);
    expect(await evaluate(`document.getElementById('card').hasAttribute('data-pressed')`)).toBe(false);
    // Its leaving (an exit animation's `data-exiting`) lets it go, and a leaving node holds none.
    expect((await hold('card', 'hover')).held).toBe('hover');
    await evaluate(`document.getElementById('card').setAttribute('data-exiting', '')`);
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    expect(await dropped()).toEqual(['gone']);
    expect((await hold('card', 'hover')).reason).toBe('gone');
    await evaluate(`document.getElementById('card').removeAttribute('data-exiting')`);
    // Its going lets it go.
    expect((await hold('card', 'active')).held).toBe('active');
    await evaluate(`document.getElementById('card').remove()`);
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    expect(await dropped()).toEqual(['gone']);
    // A state that is none of the three is refused.
    expect(await evaluate(`exact.hold(null, 'pressed').malformed`)).toBe(true);
  } finally {
    await cdp.send('Target.closeTarget', { targetId });
  }
}, 60000);
