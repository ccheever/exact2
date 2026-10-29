// State looks in a real browser (LLP 1121 D8, stage 8 B): a `:hover` look as the
// host writes it into the page's looks sheet (src/host/looks.rs), over a node's
// inline base, at once; `:active` lowered to the press paint (LLP 1061 D7), whose
// opacity is an absolute target eased from what shows (the hover's included) and
// whose paint goes over the hover; a hover look replaces the unsaid wash; rows neither names show the base, also a base
// that moves while pressed; a disabled node takes no look.
import { expect, beforeAll, afterAll } from 'bun:test';
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

const sheet = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>[\s\S]*?<\/style>/)[0];
// As `looks::rules` writes a `:hover` look (a yellow face at 0.8); `:active` is the node's press paint as the
// host writes it (LLP 1061 D7: a blue face, an absolute opacity of 0.6).
const on = ':not(:disabled,[disabled]:not([disabled="false"]))';
const hover = at => `${at}:hover${on}{background-color:rgba(255,255,0,1)!important;opacity:0.8!important;--exact-hover-wash:none!important;}`;
const rules = `@media (hover:hover){${['3', '6', '7', '9', '11'].map(id => hover(`[data-view="${id}"]`)).join('')}${hover('[data-exact-looks="1"]')}}[data-exact-looks="1"][data-exact-held~=hover]${on}{background-color:rgba(255,255,0,1)!important;opacity:0.8!important;}[data-exact-looks="2"]:focus-visible${on},[data-exact-looks="2"][data-exact-held~=focus-visible]${on}{background-color:rgba(0,255,255,1)!important;}`;
// A node with its own transition on what its look sets: the wasm host's (a view the looks sheet has rules for) and
// the JS target's (`data-exact-looks`).
const eased = 'width:100px;height:40px;background-color:#ffffff;opacity:1;transition:background-color 400ms linear,opacity 400ms linear';
const press = 'scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.95;--exact-press-background-color:rgba(0,0,255,1);--exact-press-opacity-to:0.6;';
const page = sheet + `<style id="exact-looks">${rules}</style>
<div id="exact-root" style="padding:20px">
  <div id="card" data-view="3" data-exact-on="press" tabindex="0" style="${press}width:100px;height:100px;background-color:#ffffff;border:2px solid #000000">c</div>
  <div id="away" style="height:100px"></div>
  <div id="bare" data-view="5" data-exact-on="press" style="width:100px;height:40px">b</div>
  <style>#exact-root#exact-root .drawn { background-image: linear-gradient(red, blue); }</style>
  <div id="image" class="drawn" data-view="6" data-exact-on="press" style="width:100px;height:40px">i</div>
  <div id="outer" data-view="7" data-exact-on="press" style="padding:10px"><div id="inner" data-view="8" data-exact-on="press" style="width:100px;height:40px">n</div></div>
  <div data-listrows><div><div id="in-row" data-view="9" data-exact-on="press" style="width:100%;height:40px">r</div></div></div>
  <div id="eased" data-view="11" style="${eased}">e</div>
  <div id="eased-js" data-exact-looks="1" style="${eased}"><span id="eased-js-in">j</span></div>
  <div id="focused" data-exact-looks="2" tabindex="0" style="${eased}">f</div>
  <div id="delayed" data-exact-looks="1" style="width:100px;height:40px;background-color:#ffffff;transition:background-color 0s linear 300ms">d</div>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  createInputHandlers({ root: document.getElementById('exact-root'), views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  // As the wasm glue keeps the looks sheet's rules, by view.
  document.getElementById('exact-looks').exactRules = new Map([[11, '[data-view="11"]:hover{}']]);
  window.ready = true;
</script>`;

let server, suite, cdp;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  suite = await cdpBrowser({ args: ['--window-size=600,900'] });
  ({ cdp } = suite);
});
afterAll(async () => {
  await suite?.close();
  server?.close();
});

check('a look snaps through an authored transition, entering and leaving; a base change still eases', async () => {
  const { targetId, sessionId } = await suite.page();
  const call = (method, params) => cdp.send(method, params, sessionId);
  const evaluate = async (expression) => {
    const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  try {
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    const at = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    const mouse = (type, [x, y]) => call('Input.dispatchMouseEvent', { type, x, y, button: 'none', buttons: 0, clickCount: 1 });
    // What shows right after the edge, and the transitions that started on the node since the last look (none: it
    // snapped; a transition started and cancelled still counts, by its transitionrun).
    await evaluate(`window.runs = new Map(); document.addEventListener('transitionrun', e => runs.set(e.target.id, (runs.get(e.target.id) ?? 0) + 1), true)`);
    const shows = (id) => evaluate(`new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => { const e = document.getElementById('${id}'), s = getComputedStyle(e), n = runs.get('${id}') ?? 0; runs.delete('${id}'); r([s.backgroundColor, Number(s.opacity).toFixed(2), n]); })))`).then(([bg, o, n]) => n ? [bg, o, `${n} transitions`] : [bg, o]);
    const away = await at('away');
    for (const id of ['eased', 'eased-js']) {
      await mouse('mouseMoved', await at(id));
      expect(await shows(id)).toEqual(['rgb(255, 255, 0)', '0.80']); // into the look, at once
      await mouse('mouseMoved', away);
      expect(await shows(id)).toEqual(['rgb(255, 255, 255)', '1.00']); // and out of it, at once
    }
    // A pointer crossing is one still change, its mouse events' twins taken as the same edge.
    await evaluate(`window.pulses = 0; new MutationObserver(r => { pulses += r.filter(m => m.oldValue === null).length; }).observe(document.getElementById('eased-js'), { attributes: true, attributeOldValue: true, attributeFilter: ['data-exact-snap'] })`);
    await mouse('mouseMoved', await at('eased-js'));
    await mouse('mouseMoved', away);
    expect(await evaluate('pulses')).toBe(2);
    // Moving within the node (onto a child) is no edge: nothing is touched.
    await mouse('mouseMoved', await at('eased-js'));
    await evaluate(`window.snaps = 0; new MutationObserver(r => { window.snaps += r.length; }).observe(document.getElementById('eased-js'), { attributes: true, attributeFilter: ['data-exact-snap'] })`);
    await mouse('mouseMoved', await at('eased-js-in'));
    expect(await shows('eased-js')).toEqual(['rgb(255, 255, 0)', '0.80']);
    expect(await evaluate('window.snaps')).toBe(0);
    await mouse('mouseMoved', away);
    // A delay alone would hold the look back: it snaps too.
    await mouse('mouseMoved', await at('delayed'));
    expect((await shows('delayed'))[0]).toBe('rgb(255, 255, 0)');
    await mouse('mouseMoved', away);
    expect((await shows('delayed'))[0]).toBe('rgb(255, 255, 255)');
    // A click, then a key: the key turns the focused node's focus look on, at once.
    await call('Input.dispatchMouseEvent', { type: 'mousePressed', x: (await at('focused'))[0], y: (await at('focused'))[1], button: 'left', buttons: 1, clickCount: 1 });
    await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x: (await at('focused'))[0], y: (await at('focused'))[1], button: 'left', buttons: 0, clickCount: 1 });
    await mouse('mouseMoved', away);
    expect(await evaluate(`[document.activeElement.id, document.activeElement.matches(':focus-visible')]`)).toEqual(['focused', false]);
    await evaluate(`document.getAnimations().forEach(a => a.finish())`);
    await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 });
    await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 });
    expect(await evaluate(`document.getElementById('focused').matches(':focus-visible')`)).toBe(true);
    expect(await evaluate(`new Promise(r => requestAnimationFrame(() => r((e => [getComputedStyle(e).backgroundColor, e.getAnimations().filter(a => a.transitionProperty).length])(document.getElementById('focused')))))`)).toEqual(['rgb(0, 255, 255)', 0]);
    await evaluate(`document.getElementById('focused').blur()`);
    // A pointer's focus shows no focus look: nothing is held still (a base transition there runs on), nor is hovering
    // a node whose only look is its focus.
    await evaluate(`window.snapped = 0; new MutationObserver(r => { window.snapped += r.length; }).observe(document.getElementById('focused'), { attributes: true, attributeFilter: ['data-exact-snap'] })`);
    await mouse('mouseMoved', await at('focused'));
    await call('Input.dispatchMouseEvent', { type: 'mousePressed', x: (await at('focused'))[0], y: (await at('focused'))[1], button: 'left', buttons: 1, clickCount: 1 });
    await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x: (await at('focused'))[0], y: (await at('focused'))[1], button: 'left', buttons: 0, clickCount: 1 });
    await evaluate(`document.getElementById('focused').blur()`);
    await mouse('mouseMoved', away);
    expect(await evaluate('window.snapped')).toBe(0);
    // A focus look: focus coming and going, at once. (Focusing reads style before its focus events come, so a
    // transition can start there and be ended at once: what shows, not whether one ran, is the check.)
    await evaluate(`document.getElementById('focused').focus({ focusVisible: true })`);
    expect(await evaluate(`document.getElementById('focused').matches(':focus-visible')`)).toBe(true);
    expect((await shows('focused')).slice(0, 2)).toEqual(['rgb(0, 255, 255)', '1.00']);
    await evaluate(`document.getElementById('focused').blur()`);
    expect((await shows('focused')).slice(0, 2)).toEqual(['rgb(255, 255, 255)', '1.00']);
    // A held state's edges (stage 8 E's hold) are held still the same way: hover, and a keyboard's focus look.
    await evaluate(`exact.hold(document.getElementById('eased-js'), 'hover', () => {})`);
    expect(await shows('eased-js')).toEqual(['rgb(255, 255, 0)', '0.80']);
    await evaluate(`exact.hold(null, null)`);
    expect(await shows('eased-js')).toEqual(['rgb(255, 255, 255)', '1.00']);
    await evaluate(`exact.hold(document.getElementById('focused'), 'focus-visible', () => {})`);
    expect(await shows('focused')).toEqual(['rgb(0, 255, 255)', '1.00']);
    await evaluate(`exact.hold(null, null)`);
    expect(await shows('focused')).toEqual(['rgb(255, 255, 255)', '1.00']);
    // A base change still eases as authored.
    expect(await evaluate(`(e => { getComputedStyle(e).backgroundColor; e.style.backgroundColor = '#000000'; getComputedStyle(e).backgroundColor; return e.getAnimations().filter(a => a.transitionProperty === 'background-color').length; })(document.getElementById('eased'))`)).toBe(1);
  } finally {
    await cdp.send('Target.closeTarget', { targetId });
  }
}, 30_000);

check('a hover look composes over the base, under the press paint', async () => {
  const { targetId, sessionId } = await suite.page();
  const call = (method, params) => cdp.send(method, params, sessionId);
  const evaluate = async (expression) => {
    const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  try {
    await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}/`);
    const at = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    const mouse = (type, [x, y], buttons = 0) => call('Input.dispatchMouseEvent', { type, x, y, button: type === 'mouseMoved' ? 'none' : 'left', buttons, clickCount: 1 });
    const settle = () => evaluate(`(document.getAnimations().forEach(a => a.finish()), new Promise(r => requestAnimationFrame(() => r())))`); // and their finish events
    // A colour as rgb(), whichever form it computes to (the press paint's is its look beneath's).
    const now = () => evaluate(`(() => { const s = getComputedStyle(document.getElementById('card')), n = c => c.replace(/color\\(srgb ([\\d.]+) ([\\d.]+) ([\\d.]+)\\)/, (_, ...v) => 'rgb(' + v.slice(0, 3).map(x => Math.round(x * 255)).join(', ') + ')'); return [n(s.backgroundColor), n(s.borderRightColor), Number(s.opacity).toFixed(2)]; })()`);
    const look = async () => { await settle(); return now(); };
    const card = await at('card'), away = await at('away');

    await mouse('mouseMoved', card);
    expect(await now()).toEqual(['rgb(255, 255, 0)', 'rgb(0, 0, 0)', '0.80']); // :hover, at once
    await mouse('mousePressed', card, 1);
    // The press eases from what shows, the hover's 0.8, to its absolute 0.6 (not 0.6 of 0.8).
    const opacity = Number((await now())[2]);
    expect(opacity).toBeGreaterThanOrEqual(0.6);
    expect(opacity).toBeLessThanOrEqual(0.8);
    expect(await look()).toEqual(['rgb(0, 0, 255)', 'rgb(0, 0, 0)', '0.60']); // the press paint over the hover
    // The base moves while pressed: a row the press doesn't paint shows the new base.
    await evaluate(`Object.assign(document.getElementById('card').style, { backgroundColor: '#eeeeee', borderRightColor: '#123456' })`);
    expect(await look()).toEqual(['rgb(0, 0, 255)', 'rgb(18, 52, 86)', '0.60']);
    // A live edit of the look beneath (its sheet, not the node's style) is what the press eases over now.
    // (The base the node's paint rule holds: its background's channels at 0.)
    const beneath = () => evaluate(`(() => { const key = document.getElementById('card').getAttribute('data-exact-painting'), rule = document.adoptedStyleSheets.flatMap(s => [...s.cssRules]).find(r => r.selectorText.includes('"' + key + '"')); const m = /background-color: rgb\\(calc\\(([\\d.]+) [^)]*\\)\\) calc\\(([\\d.]+) [^)]*\\)\\) calc\\(([\\d.]+)/.exec(rule?.cssText ?? ''); return m && 'rgb(' + m.slice(1, 4).join(', ') + ')'; })()`);
    expect(await beneath()).toBe('rgb(255, 255, 0)');
    await evaluate(`(s => { s.textContent = s.textContent.replace('255,255,0', '0,255,255'); })(document.getElementById('exact-looks'))`);
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    expect(await beneath()).toBe('rgb(0, 255, 255)');
    // A CSSOM write (the JS target's tokens, modes.js) no observer sees: modes.js calls the glue's hook.
    await evaluate(`document.getElementById('exact-looks').sheet.cssRules[0].cssRules[0].style.setProperty('background-color', 'rgb(255, 0, 255)', 'important')`);
    await evaluate('exact.repaintPress()');
    expect(await beneath()).toBe('rgb(255, 0, 255)');
    await evaluate(`(s => { s.textContent = s.textContent.replace('0,255,255', '255,255,0'); })(document.getElementById('exact-looks'))`);
    await mouse('mouseReleased', card);
    expect(await look()).toEqual(['rgb(255, 255, 0)', 'rgb(18, 52, 86)', '0.80']); // the hover again
    await mouse('mouseMoved', away);
    expect(await now()).toEqual(['rgb(238, 238, 238)', 'rgb(18, 52, 86)', '1.00']); // and the new base, at once

    // The look is the whole hover: it replaces the unsaid wash (LLP 1129), which a node with no look keeps, and leaves
    // an image of the author's (the JS target's in a class); one inside the node keeps its own wash; the stronger
    // wash of a row's press still shows.
    const image = (id) => evaluate(`getComputedStyle(document.getElementById('${id}')).backgroundImage`);
    await mouse('mouseMoved', card);
    expect(await image('card')).toBe('none');
    await mouse('mouseMoved', await at('bare'));
    expect(await image('bare')).toStartWith('linear-gradient(');
    await mouse('mouseMoved', await at('image'));
    expect([await image('image'), (await evaluate(`getComputedStyle(document.getElementById('image')).backgroundColor`))]).toEqual(['linear-gradient(rgb(255, 0, 0), rgb(0, 0, 255))', 'rgb(255, 255, 0)']);
    await mouse('mouseMoved', await at('inner'));
    expect([await image('outer'), await image('inner')]).toEqual(['none', expect.stringMatching(/^linear-gradient\(/)]);
    await mouse('mouseMoved', await at('in-row'));
    expect(await image('in-row')).toBe('none');
    await mouse('mousePressed', await at('in-row'), 1);
    expect(await image('in-row')).toStartWith('linear-gradient(');
    await mouse('mouseReleased', await at('in-row'));
    await mouse('mouseMoved', away);

    // Reduce Motion: the press's colours still ease (the press is feedback); a hover still snaps.
    await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await mouse('mouseMoved', card);
    expect(await now()).toEqual(['rgb(255, 255, 0)', 'rgb(18, 52, 86)', '0.80']);
    await mouse('mousePressed', card, 1);
    expect(await evaluate(`document.getElementById('card').hasAttribute('data-exact-painting')`)).toBe(true);
    expect((await now())[0]).not.toBe('rgb(0, 0, 255)'); // eased in, not snapped
    await mouse('mouseReleased', card);
    await settle();
    await call('Emulation.setEmulatedMedia', { features: [] });

    // A disabled node takes no look (a box says `disabled` as the hosts write it).
    await mouse('mouseMoved', away);
    await evaluate(`document.getElementById('card').setAttribute('disabled', '')`);
    await mouse('mouseMoved', card);
    expect(await look()).toEqual(['rgb(238, 238, 238)', 'rgb(18, 52, 86)', '1.00']);
    // Nor a press: no paint, no scale.
    await mouse('mousePressed', card, 1);
    expect(await evaluate(`['data-pressed', 'data-exact-painting'].map(a => document.getElementById('card').hasAttribute(a))`)).toEqual([false, false]);
    expect(await look()).toEqual(['rgb(238, 238, 238)', 'rgb(18, 52, 86)', '1.00']);
    await mouse('mouseReleased', card);
    // Nor by key (Space on the focused box).
    await evaluate(`document.getElementById('card').focus()`);
    await call('Input.dispatchKeyEvent', { type: 'keyDown', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
    expect(await evaluate(`['data-pressed', 'data-exact-painting'].map(a => document.getElementById('card').hasAttribute(a))`)).toEqual([false, false]);
    await call('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
  } finally {
    await cdp.send('Target.closeTarget', { targetId });
  }
}, 60000);
