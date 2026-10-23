// Press feedback in a real browser: the shell's CSS and the input glue,
// driven by CDP mouse and key events. A declared press (LLP 1061 D3) goes by
// UIKit's rule, not `:active`'s: only the innermost pressable shows it, only
// while the pointer is inside its box, including under reduced motion. A
// pressable that declares none dims while the browser holds it `:active`
// (LLP 1115.000 D6).
import { test, expect, beforeAll, afterAll } from 'bun:test';
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

// The stylesheet a default build's page has (look.mjs), with nodes as the host writes them: a card
// (0.9) holding a button (0.5), a plain pressable (no row) in the card, and
// a disabled button, and a native button with neither (its card's press is its own).
const sheet = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>[\s\S]*?<\/style>/)[0];
const page = sheet + `
<div id="exact-root" style="padding:20px">
  <div id="card" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.9;width:300px;height:300px;padding:20px">
    <button id="button" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;width:100px;height:100px">b</button>
    <div id="plain" data-exact-on="press" style="width:100px;height:50px;--exact-press-haptic:impact-medium">p</div>
    <button id="off" data-exact-on="press" disabled style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;--exact-press-haptic:selection;width:100px;height:50px">d</button>
    <button id="native" type="button" data-button-style="filled" style="width:100px;height:40px"><span id="title">n</span></button>
  </div>
  <button id="scaled" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;width:100px;height:60px;--exact-scale:1.5;transform-origin:0 0">s</button>
  <svg width="400" height="180"><rect id="svg" data-exact-on="press" x="0" y="0" width="100" height="60" transform="translate(100 40) rotate(20)" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;transform-origin:20px 10px;fill:red" /></svg>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root');
  window.vibrations = []; navigator.vibrate = ms => { vibrations.push(ms); return true; }; // headless Chrome has none
  createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  window.ready = true;
</script>`;

// LLP 1115.000 D6, nodes as both web targets write them: pressables that
// declare no feedback (a box, one that hears `hover`, a custom button, a
// link, a card holding a button and a field), those whose press is the
// author's (a scale, `pointerdown`, an `opacity` of its own), native
// buttons (filled, on a translucent accent too, plain, glass, tinted, and one
// with a pressed paint), a game's action, a disabled custom button, rows of a
// list (plain, virtualized, grouped, and a button inside a grouped row), an
// item of a sideways virtualized list, and a scrim over the window, shown on
// demand. `?agent` is the agent's clock.
const unsaid = sheet + `
<style>#exact-root > *, #exact-root [role] * { min-height: 24px; } #away { height: 60px; }</style>
<div id="exact-root" style="padding:20px;width:300px">
  <div id="box" data-exact-on="press" tabindex="0">box</div>
  <div id="hears" data-exact-on="press hover" tabindex="0">hears hover</div>
  <button id="custom" type="button" data-exact-on="press">custom</button>
  <a id="link" data-view="" href="#there">link</a>
  <div id="outer" data-exact-on="press" tabindex="0">outer <button id="inner" type="button" data-exact-on="press" style="display:inline-block">inner</button> <input id="field" data-native="" style="display:inline-block;width:60px"></div>
  <div id="scaled" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.9">scaled</div>
  <div id="drawn" data-exact-on="press pointerdown pointerup">draws its own</div>
  <div id="faded" data-exact-on="press" style="opacity:0.4">faded</div>
  <button id="native" type="button" data-button-style="filled" data-native="" data-exact-on="press"><span>native</span></button>
  <button id="text" type="button" data-button-style="plain" data-native="" data-exact-on="press"><span id="text-face">text</span></button>
  <button id="translucent" type="button" data-button-style="filled" data-native="" style="--exact-accent:var(--exact-quaternary-fill)"><span id="translucent-face">translucent</span></button>
  <button id="glass" type="button" data-button-style="glass" data-native="" data-exact-on="press"><span id="glass-face">glass</span></button>
  <button id="tinted" type="button" data-button-style="tinted" data-native=""><span id="tinted-face">tinted</span></button>
  <button id="bordered" type="button" data-button-style="bordered" data-native=""><span id="bordered-face">bordered</span></button>
  <button id="painting" type="button" data-button-style="filled" data-native="" style="--exact-press-opacity:0.5"><span id="painting-face">paints its own</span></button>
  <button id="action" type="button" data-action="jump">action</button>
  <button id="off" type="button" disabled data-exact-on="press">off</button>
  <button id="ownoff" type="button" disabled data-exact-on="press" data-disabledstyled="true">its own disabled look</button>
  <div role="list" data-listrows="true">
    <div id="row" data-exact-on="press" tabindex="0">row</div>
    <div id="swipe"><div><a id="content" data-view="" href="#book"><span>a row's content</span></a></div></div>
    <a id="rowlink" data-view="" href="#there"><span data-exact-text="">a row that is a link of text</span></a>
    <div id="drawnrow" data-exact-on="press pointerdown pointerup">a row that draws its own</div>
  </div>
  <div role="list" data-listrows="true" data-estimateditemheight="24">
    <div role="listitem" data-listitemkey="a"><div id="windowed" data-exact-on="press">windowed row</div></div>
  </div>
  <div role="list">
    <div id="own" data-exact-on="press" tabindex="0">a row of a list whose author said appearance="none"</div>
  </div>
  <div role="list" data-liststyle="inset-grouped">
    <section><header><div>header</div></header><div>
      <div id="cell" data-exact-on="press">cell</div>
      <div><button id="accessory" type="button" data-exact-on="press">accessory</button></div>
    </div></section>
  </div>
  <div role="list" data-estimateditemwidth="120">
    <div role="listitem" data-listitemkey="b"><div id="slide" data-exact-on="press">slide</div></div>
  </div>
  <div id="away"></div>
  <div id="scrim" data-exact-on="press" style="position:fixed;inset:0;display:none"></div>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root');
  window.clicks = []; root.addEventListener('click', e => clicks.push(e.target.id));
  createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {}, agentMode: location.search === '?agent' });
  window.ready = true;
</script>`;

// LLP 1061 D7, the pressed paint as both web targets write it: a card holding
// a box that dims to half its own opacity and turns blue with red text (one
// span inheriting the colour, one writing its own), a box that only recolours
// its text, a button that also scales, and a row of a cell list. `?agent`
// is the agent's clock.
const paint = sheet + `
<div id="exact-root" style="padding:20px">
  <div id="paintcard" data-exact-on="press" style="--exact-press-opacity:0.5;width:320px;padding:20px">
    <div id="painted" data-exact-on="press" style="--exact-press-opacity:0.5;--exact-press-background-color:rgb(0, 0, 255);--exact-press-color:rgb(255, 0, 0);background-color:rgb(255, 255, 255);color:rgb(0, 0, 0);opacity:0.8;width:200px;height:60px"><span id="label">label</span> <span id="kept" style="color:rgb(0, 128, 0)">kept</span></div>
  </div>
  <div id="inked" data-exact-on="press" style="--exact-press-color:rgb(255, 0, 0);width:200px;height:40px">ink</div>
  <button id="both" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;--exact-press-opacity:0.5;width:100px;height:40px">both</button>
  <div role="list" data-listrows="true"><div id="paintrow" data-exact-on="press" style="--exact-press-background-color:rgb(0, 0, 255);height:40px">row</div></div>
  <div id="away" style="height:60px"></div>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root');
  createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {}, agentMode: location.search === '?agent' });
  window.ready = true;
</script>`;

// One server for the file, and one Chrome over CDP for its Chrome-only tests; each test opens a page of its own.
let server, suite, cdp;
beforeAll(async () => {
  server = createServer((req, res) => {
    const html = { '/': page, '/unsaid': unsaid, '/unsaid?agent': unsaid, '/paint': paint, '/paint?agent': paint }[req.url];
    if (html) { res.writeHead(200, { 'content-type': 'text/html' }); res.end(html); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  if (unavailable) return;
  suite = await cdpBrowser({ args: ['--window-size=600,900'] });
  ({ cdp } = suite);
});
afterAll(async () => {
  await suite?.close();
  server?.close();
});
/** A page at `path`, ready: its CDP `call` and `evaluate`, a node's `centre`, and the left button by `mouse`. */
async function open(path) {
  const { targetId, sessionId } = await suite.page();
  const call = (method, params) => cdp.send(method, params, sessionId);
  const evaluate = async (expression) => {
    const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  await readyPage(cdp, sessionId, `http://127.0.0.1:${server.address().port}${path}`);
  const centre = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  const mouse = (type, [x, y]) => call('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1 });
  return { call, evaluate, centre, mouse, close: () => cdp.send('Target.closeTarget', { targetId }) };
}

check('only the innermost pressable shows the press, and only while inside', async () => {
  const { call, evaluate, centre, mouse, close } = await open('/');
  try {
    // Host glass remains stacking even against an authored `none`. The button rules are scoped to
    // #exact-root, and a disabled native button dims by its accent fill, not a filter (55fcdee61).
    expect(await evaluate(`(() => {
      const out = [];
      for (const style of ['glass', 'prominent-glass', 'clear-glass', 'prominent-clear-glass']) {
        const el = document.createElement('button');
        el.dataset.buttonStyle = style;
        el.style.backdropFilter = 'none'; el.disabled = true;
        document.getElementById('exact-root').append(el);
        out.push(getComputedStyle(el).backdropFilter !== 'none');
        el.remove();
      }
      return out;
    })()`)).toEqual([true, true, true, true]);
    const pressed = () => evaluate(`[...document.querySelectorAll('[data-pressed]')].map(el => el.id).join()`);
    const settle = () => evaluate(`document.getAnimations().forEach(a => { if (Number.isFinite(a.effect.getComputedTiming().endTime)) a.finish(); })`);
    const scale = async (id) => {
      await settle();
      return evaluate(`parseFloat(getComputedStyle(document.getElementById('${id}')).scale) || 1`);
    };
    const rect = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })()`);

    const button = await centre('button');
    await mouse('mousePressed', button);
    expect(await pressed()).toBe('button'); // the card, which `:active` would match too, is not
    expect(await scale('button')).toBe(0.5);
    expect(await scale('card')).toBe(1);
    await mouse('mouseMoved', [button[0] + 70, button[1]]); // off the unpressed box's right edge
    expect(await pressed()).toBe('');
    await mouse('mouseMoved', [button[0] + 40, button[1]]); // outside the pressed box, inside the unpressed one
    expect(await pressed()).toBe('button');
    await mouse('mouseReleased', button);
    expect(await pressed()).toBe('');

    // The innermost pressable has no row: it dims as the browser holds it
    // `:active` (LLP 1115.000 D6; the test below), which this piece does not
    // mark, and its card, pressed through it, shows nothing.
    // Its `-exact-press-haptic` plays at the press, as `haptic()`'s length (LLP 1077 D14).
    const vibrations = () => evaluate('vibrations.join()');
    const opacity = (id) => evaluate(`getComputedStyle(document.getElementById('${id}')).opacity`);
    expect(await vibrations()).toBe('');
    await mouse('mousePressed', await centre('plain'));
    expect(await pressed()).toBe('');
    expect(await opacity('plain')).toBe('0.65');
    expect([await opacity('card'), await scale('card')]).toEqual(['1', 1]);
    expect(await vibrations()).toBe('12');
    await mouse('mouseReleased', await centre('plain'));
    expect(await opacity('plain')).toBe('1');
    // A disabled button gives nothing either; its card is not pressed through it, and its haptic is silent.
    await mouse('mousePressed', await centre('off'));
    expect(await pressed()).toBe('');
    await mouse('mouseReleased', await centre('off'));
    expect(await vibrations()).toBe('12');
    // A native button highlights with no row or handler of its own, as a
    // UIButton does: its face fades over a lighter fill (neutral.css), nothing
    // scales, its card is not pressed. The unsaid dim is not laid over it.
    const face = () => evaluate(`getComputedStyle(document.getElementById('title')).opacity`);
    const fill = () => evaluate(`getComputedStyle(document.getElementById('native')).backgroundColor`);
    const rest = await fill();
    await mouse('mousePressed', await centre('native'));
    expect(await pressed()).toBe('native');
    expect([await fill() !== rest, await face()]).toEqual([true, '0.75']);
    expect(await opacity('native')).toBe('1');
    expect(await scale('native')).toBe(1);
    expect(await scale('card')).toBe(1);
    await mouse('mouseReleased', await centre('native'));
    expect(await pressed()).toBe('');
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 }); // off it: a pointer left over it is its hover
    await settle(); // it eases out of the highlight (the test below)
    expect([await fill(), await face()]).toEqual([rest, '1']);
    // The keyboard shows the same state (LLP 1115.000 D4.14): Space holds a
    // native button's highlight, and a button's `-exact-press-scale`, until
    // its keyup.
    const space = (type) => call('Input.dispatchKeyEvent', { type, key: ' ', code: 'Space', windowsVirtualKeyCode: 32, ...(type === 'keyDown' ? { text: ' ' } : {}) });
    await evaluate(`document.getElementById('native').focus()`);
    await space('keyDown');
    expect([await pressed(), await face(), await opacity('native')]).toEqual(['native', '0.75', '1']);
    await space('keyUp');
    await settle();
    expect([await pressed(), await face()]).toEqual(['', '1']);
    await evaluate(`document.getElementById('button').focus()`);
    await space('keyDown');
    expect([await pressed(), await scale('button'), await scale('card')]).toEqual(['button', 0.5, 1]);
    await space('keyUp');
    expect([await pressed(), await scale('button')]).toEqual(['', 1]);
    await evaluate(`document.activeElement.blur()`);
    // Mobile WebKit's grey tap highlight never paints over any of it.
    expect(await evaluate(`getComputedStyle(document.documentElement).getPropertyValue('-webkit-tap-highlight-color')`)).toBe('rgba(0, 0, 0, 0)');

    // Reduced motion keeps the host feedback, as a native button keeps its highlight.
    await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await mouse('mousePressed', button);
    expect(await pressed()).toBe('button');
    expect(await scale('button')).toBe(0.5);
    await mouse('mouseReleased', button);
    await settle();

    const before = await rect('scaled');
    await mouse('mousePressed', await centre('scaled'));
    expect(await scale('scaled')).toBe(0.75);
    const held = await rect('scaled');
    expect(held[0]).toBeCloseTo(before[0], 4);
    expect(held[1]).toBeCloseTo(before[1], 4);
    expect(held[2]).toBeCloseTo(before[2] / 2, 4);
    await mouse('mouseReleased', await centre('scaled'));
    // Re-press during release. Its original right edge is still inside;
    // undo only the feedback, about top-left, keeping the authored 1.5.
    await evaluate(`document.getAnimations().forEach(a => { a.pause(); a.currentTime = 30; })`);
    await mouse('mousePressed', await centre('scaled'));
    await mouse('mouseMoved', [before[0] + before[2] - 2, before[1] + 20]);
    expect(await pressed()).toBe('scaled');
    await mouse('mouseMoved', [before[0] + before[2] + 2, before[1] + 20]);
    expect(await pressed()).toBe('');
    await mouse('mouseReleased', await centre('scaled'));
    await settle();

    const transform = () => evaluate(`getComputedStyle(document.getElementById('svg')).transform`);
    const authored = await transform(), svg = await rect('svg');
    await mouse('mousePressed', await centre('svg'));
    expect(await scale('svg')).toBe(0.5);
    expect(await transform()).toBe(authored);
    const shrunk = await rect('svg');
    expect(shrunk[2]).toBeCloseTo(svg[2] / 2, 4);
    expect(shrunk[3]).toBeCloseTo(svg[3] / 2, 4);
    await mouse('mouseReleased', await centre('svg'));
    await settle();
    expect(await rect('svg')).toEqual(svg);

    // A scale animation and transition keep their own timing while held.
    await evaluate(`{ const style = document.createElement('style'); style.textContent = '@keyframes grow { from { scale:1; --exact-scale:1; } to { scale:2; --exact-scale:2; } }'; document.head.append(style); const el = document.getElementById('scaled'); el.style.animation = 'grow 1s linear both'; window.row = el.getAnimations().find(a => a instanceof CSSAnimation); row.pause(); row.currentTime = 500; }`);
    await mouse('mousePressed', await centre('scaled'));
    await evaluate(`document.getElementById('scaled').getAnimations().filter(a => a !== row).forEach(a => a.finish());`);
    expect(await evaluate(`parseFloat(getComputedStyle(document.getElementById('scaled')).scale)`)).toBe(0.75);
    await evaluate('row.currentTime = 1000');
    expect(await evaluate(`parseFloat(getComputedStyle(document.getElementById('scaled')).scale)`)).toBe(1);
    await mouse('mouseReleased', await centre('scaled'));
    await settle();
    await evaluate(`{ row.cancel(); document.getElementById('scaled').style.animation = ''; const el = document.getElementById('scaled').cloneNode(true); el.id = 'transitioned'; el.style.cssText += ';position:absolute;left:360px;top:30px;transition:--exact-scale 1s linear'; document.getElementById('exact-root').append(el); getComputedStyle(el).scale; }`);
    await evaluate('new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)))');
    await evaluate(`{ const el = document.getElementById('transitioned'); el.style.setProperty('--exact-scale', '2.5'); getComputedStyle(el).scale; el.getAnimations().forEach(a => { a.pause(); a.currentTime = 500; }); }`);
    expect(await evaluate(`document.getElementById('transitioned').getAnimations().filter(a => a instanceof CSSTransition).length`)).toBe(1);
    await mouse('mousePressed', [380, 50]);
    expect(await pressed()).toBe('transitioned');
    await evaluate(`document.getElementById('transitioned').getAnimations().filter(a => !(a instanceof CSSTransition)).forEach(a => a.finish());`);
    expect(await evaluate(`parseFloat(getComputedStyle(document.getElementById('transitioned')).scale)`)).toBe(1);
    await mouse('mouseReleased', [380, 50]);
  } finally {
    await close();
  }
}, 60000);

check('a pressable that declares no feedback dims while pressed; a row, a native control and an authored press do not', async () => {
  const { call, evaluate, centre, mouse, close } = await open('/unsaid');
  try {
    const shown = (...ids) => evaluate(`${JSON.stringify(ids)}.map(id => getComputedStyle(document.getElementById(id)).opacity).join()`);
    const marked = (id, name) => evaluate(`document.getElementById('${id}').hasAttribute('${name}')`);
    // The left button down on a node (or at a point), what the named nodes show, then the button up where it presses nothing.
    // Moving a held link away drags it, so each press below waits for the drag to end: a press during it is lost
    await evaluate(`addEventListener('dragstart', () => { window.dragging = new Promise(ended => addEventListener('dragend', () => ended(), { once: true })); }, true)`);
    const held = async (at, ...ids) => {
      await mouse('mousePressed', typeof at === 'string' ? await centre(at) : at);
      const seen = await shown(...ids);
      const away = await centre('away');
      await mouse('mouseMoved', away);
      await mouse('mouseReleased', away);
      await evaluate('window.dragging');
      return seen;
    };
    expect(await shown('box', 'custom', 'link', 'off')).toBe('1,1,1,0.5'); // at rest only a disabled custom button is dimmed
    expect(await shown('ownoff')).toBe('1'); // not one whose author styles its own disabled state (Contract's mark, LLP 1115.000 D6)

    // The browser's `:active`: this piece marks nothing.
    expect(await held('box', 'box')).toBe('0.65');
    expect(await marked('box', 'data-pressed')).toBe(false);
    expect(await shown('box')).toBe('1');
    expect(await held('hears', 'hears')).toBe('0.65'); // a `hover` handler observes; it replaces nothing
    expect(await held('custom', 'custom')).toBe('0.65');
    expect(await held('link', 'link')).toBe('0.65');
    // Only the innermost: a button or a field in a card takes the press, and the card's own area dims the card.
    expect(await held('inner', 'inner', 'outer')).toBe('0.65,1');
    expect(await held('field', 'outer')).toBe('1');
    const outer = await evaluate(`(() => { const r = document.getElementById('outer').getBoundingClientRect(); return [r.x + 4, r.y + r.height / 2]; })()`);
    expect(await held(outer, 'outer', 'inner')).toBe('0.65,1');

    // The author's own press replaces it: a scale, a press it draws from `pointerdown`, an `opacity` of its own.
    await mouse('mousePressed', await centre('scaled'));
    await evaluate(`document.getAnimations().forEach(a => a.finish())`);
    expect(await evaluate(`(() => { const s = getComputedStyle(document.getElementById('scaled')); return s.opacity + ' ' + parseFloat(s.scale); })()`)).toBe('1 0.9');
    await mouse('mouseReleased', await centre('scaled'));
    expect(await held('drawn', 'drawn')).toBe('1');
    expect(await held('faded', 'faded')).toBe('0.4');
    // A native control keeps its own feedback, a game's action is held, and a disabled custom button takes no press.
    expect(await held('native', 'native')).toBe('1');
    // A plain button, a filled one on a translucent accent and a glass one show UIKit's highlight (neutral.css): the
    // button, and so its focus ring, keeps its opacity, and its face fades. A pressed paint the author wrote adds to
    // it, as on iOS, where UIKit highlights a native button whatever press is written (NativeButtonsIOS.swift).
    // Released, it eases out of the highlight (the test below).
    const eased = () => evaluate(`document.getAnimations().forEach(a => a instanceof CSSTransition && a.finish())`);
    for (const id of ['text', 'translucent', 'glass']) {
      const down = await held(id, id, `${id}-face`);
      await eased();
      expect([id, down, await shown(`${id}-face`)]).toEqual([id, '1,0.75', '1']);
    }
    await mouse('mousePressed', await centre('painting'));
    await evaluate(`document.getAnimations().forEach(a => a.finish())`);
    expect(await shown('painting', 'painting-face')).toBe('0.5,0.75');
    await mouse('mouseMoved', await centre('away'));
    await mouse('mouseReleased', await centre('away'));
    // A tinted button held under the pointer shows its pressed wash, not its hover's, as it does under a finger (below).
    const tinted = await centre('tinted'), wash = () => evaluate(`getComputedStyle(document.getElementById('tinted')).backgroundImage`);
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: tinted[0], y: tinted[1], button: 'none', buttons: 0 });
    const hovered = await wash();
    await mouse('mousePressed', tinted);
    const pressedWash = await wash();
    await mouse('mouseMoved', await centre('away'));
    await mouse('mouseReleased', await centre('away'));
    expect(pressedWash).not.toBe(hovered);
    expect(await held('action', 'action')).toBe('1');
    expect(await held('off', 'off')).toBe('0.5');

    // A row of a list whose rows are cells (LLP 1115.000 D16: `data-listrows`, Contract's mark) never dims: what
    // presses in it shows the look's pressed wash over its own ground, wherever in the row it is (the list's child, a
    // windowed list's row in its wrapper, the content a swipe container holds, a link of text), and an authored press
    // still replaces it. A grouped list's row in its section keeps its own. A list that is no cell list (the author's
    // `appearance="none"`, a sideways windowed list, which is a carousel) holds ordinary pressables, which dim.
    // Its opacity while held, and whether it then shows a wash that is not the hover's (the pointer is over it too).
    const washed = async (at, id) => {
      const image = `getComputedStyle(document.getElementById('${id}')).backgroundImage`;
      const [x, y] = await centre(at);
      await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none', buttons: 0 });
      const hovered = await evaluate(image);
      await mouse('mousePressed', await centre(at));
      const seen = await evaluate(`(() => { const s = getComputedStyle(document.getElementById('${id}')); return [s.opacity, s.backgroundImage.startsWith('linear-gradient') && s.backgroundImage !== ${JSON.stringify(hovered)}].join(); })()`);
      const away = await centre('away');
      await mouse('mouseMoved', away);
      await mouse('mouseReleased', away);
      await evaluate('window.dragging');
      return seen;
    };
    expect(await washed('row', 'row')).toBe('1,true');
    expect(await washed('windowed', 'windowed')).toBe('1,true');
    expect(await washed('content', 'content')).toBe('1,true');
    expect(await washed('rowlink', 'rowlink')).toBe('1,true');
    expect(await washed('drawnrow', 'drawnrow')).toBe('1,false');
    expect(await evaluate(`getComputedStyle(document.getElementById('row')).backgroundImage`)).toBe('none'); // and nothing at rest
    expect(await held('cell', 'cell')).toBe('1');
    expect(await held('accessory', 'accessory')).toBe('0.65');
    expect(await washed('own', 'own')).toBe('0.65,false');
    expect(await washed('slide', 'slide')).toBe('0.65,false');

    // A pressable covering the window is a surface: measured at the press, marked, never dimmed; smaller, it dims again.
    await evaluate(`document.getElementById('scrim').style.display = 'block'`);
    expect(await held([300, 450], 'scrim')).toBe('1');
    expect(await marked('scrim', 'data-exact-surface')).toBe(true);
    await evaluate(`document.getElementById('scrim').style.inset = '0 0 50% 0'`);
    expect(await held([300, 200], 'scrim')).toBe('0.65');
    expect(await marked('scrim', 'data-exact-surface')).toBe(false);
    await evaluate(`document.getElementById('scrim').style.display = 'none'`);

    // The keyboard shows the same state. Space holds a `<button>` `:active`; a box this piece presses by key is
    // marked while Space is down, pressed once at its keydown. Enter presses and holds nothing, as on a button.
    const key = (type, name) => call('Input.dispatchKeyEvent', { type, key: name === 'Space' ? ' ' : name, code: name, windowsVirtualKeyCode: name === 'Space' ? 32 : 13, ...(type === 'keyDown' ? { text: name === 'Space' ? ' ' : '\r' } : {}) });
    const keyed = async (id, name, ...ids) => {
      await evaluate(`document.getElementById('${id}').focus()`);
      await key('keyDown', name);
      const seen = await shown(...ids);
      await key('keyUp', name);
      return seen;
    };
    await evaluate('clicks.length = 0');
    expect(await keyed('custom', 'Space', 'custom')).toBe('0.65');
    expect(await shown('custom')).toBe('1');
    expect(await keyed('box', 'Space', 'box')).toBe('0.65');
    expect([await shown('box'), await marked('box', 'data-pressed')]).toEqual(['1', false]);
    expect(await keyed('box', 'Enter', 'box')).toBe('1');
    expect(await keyed('row', 'Space', 'row')).toBe('1'); // a row from the keyboard is still its cell
    expect(await keyed('text', 'Space', 'text', 'text-face')).toBe('1,0.75');
    expect(await evaluate('clicks.join()')).toBe('custom,box,box,row,text');
    // Another key's release leaves the mark; it goes when the focus leaves with Space still down.
    await evaluate(`document.getElementById('box').focus()`);
    await key('keyDown', 'Space');
    await key('keyUp', 'Enter');
    expect(await marked('box', 'data-pressed')).toBe(true);
    await evaluate(`document.getElementById('custom').focus()`);
    expect([await marked('box', 'data-pressed'), await shown('box')]).toEqual([false, '1']);
    await key('keyUp', 'Space');

    // A finger: a touch's `:active` is the engine's, which a synthesized touch never gets, so this piece marks the
    // innermost pressable while the finger is down, and lets it go at the lift (the tap the lift makes then holds it
    // `:active` a moment, the browser's own).
    await call('Emulation.setTouchEmulationEnabled', { enabled: true });
    const finger = (type, at) => call('Input.dispatchTouchEvent', { type, touchPoints: at ? [{ x: at[0], y: at[1] }] : [] });
    await finger('touchStart', await centre('inner'));
    expect([await shown('inner', 'outer'), await marked('inner', 'data-pressed')]).toEqual(['0.65,1', true]);
    await finger('touchEnd');
    expect(await marked('inner', 'data-pressed')).toBe(false);
    for (const id of ['text', 'translucent', 'glass']) {
      await finger('touchStart', await centre(id));
      const down = await shown(id, `${id}-face`);
      await finger('touchEnd');
      await eased();
      expect([id, down, await shown(id, `${id}-face`)]).toEqual([id, '1,0.75', '1,1']);
    }
    await finger('touchStart', await centre('tinted'));
    expect(await wash()).toBe(pressedWash);
    await finger('touchEnd');
    // The window losing focus lets it go, and so does a gesture claiming the contact (motion-glue's swipe): a later
    // move inside the box does not press it again.
    await finger('touchStart', await centre('inner'));
    await evaluate(`dispatchEvent(new Event('blur'))`);
    expect(await marked('inner', 'data-pressed')).toBe(false);
    await finger('touchEnd');
    await evaluate(`addEventListener('pointerdown', e => { window.lastPointer = e.pointerId; }, true)`);
    const [x, y] = await centre('inner');
    await finger('touchStart', [x, y]);
    // As motion-glue claims a swipe: the mark goes at the claim, and no later move inside the box brings it back.
    await evaluate(`(globalThis.exact.contacts ??= new Map()).set(window.lastPointer, 'claimed'); globalThis.exact.pressClaimed(window.lastPointer)`);
    expect(await marked('inner', 'data-pressed')).toBe(false);
    await finger('touchMove', [x + 1, y]);
    expect(await marked('inner', 'data-pressed')).toBe(false);
    await finger('touchEnd');
  } finally {
    await close();
  }
}, 60000);

// A native button eases out of its press as the press ends (neutral.css; UIKit's highlight on iOS 27, a quarter second,
// ease-in-out), so even a quick tap shows, under a finger or the mouse, in each engine: a plain, a filled and a glass
// button's face, the filled one's fill, and a glass, tinted or bordered button's wash. The press itself comes at once,
// and nothing else about the button eases: a filled button enabled again takes its fill at once, and an authored
// transition running in it is its own. A release on the agent's clock lands at once, and a pressable that is no native
// button is never marked.
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a native button eases out of its press as the press ends`, async () => {
  const browser = await suiteBrowser(engine);
  const load = async (path, hasTouch = false) => {
    const page = await browser.newPage({ viewport: { width: 600, height: 900 }, hasTouch });
    await page.goto(`http://127.0.0.1:${server.address().port}${path}`);
    await page.waitForFunction(() => window.ready);
    return page;
  };
  // What each button's release eases: its face's opacity, its fill, its wash.
  const EASES = { text: ['opacity'], translucent: ['background-color', 'opacity'], glass: ['--exact-wash', 'opacity'], tinted: ['--exact-wash'], bordered: ['--exact-wash'] };
  try {
    for (const finger of [true, false]) {
      const page = await load('/unsaid', finger);
      for (const [id, eased] of Object.entries(EASES)) {
        const at = [engine, finger ? 'finger' : 'mouse', id];
        // The button's transitions (WebKit runs one on an `::after` that draws nothing too), and its face's opacity, its
        // fill and its wash's alpha where they start and half way.
        const seen = () => page.evaluate((id) => {
          const b = document.getElementById(id), face = document.getElementById(`${id}-face`), ink = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
          const alpha = (c) => { ink.clearRect(0, 0, 1, 1); ink.fillStyle = c; ink.fillRect(0, 0, 1, 1); return ink.getImageData(0, 0, 1, 1).data[3]; };
          const eases = b.getAnimations({ subtree: true }).filter((a) => !a.effect.pseudoElement || getComputedStyle(b, a.effect.pseudoElement).content !== 'none');
          const when = (t) => { for (const a of eases) { a.pause(); a.currentTime = t; } return [getComputedStyle(face).opacity, getComputedStyle(b).backgroundColor, alpha(getComputedStyle(b).getPropertyValue('--exact-wash'))]; };
          return [eases.map((a) => [a.transitionProperty, a.effect.getTiming().duration, a.effect.getTiming().easing]).sort(), when(0), when(125), b.hasAttribute('data-exact-released')];
        }, id);
        const rest = await page.evaluate((id) => getComputedStyle(document.getElementById(id)).backgroundColor, id);
        if (finger) await page.tap(`#${id}`);
        else {
          await page.hover(`#${id}`);
          await page.mouse.down();
          expect([at, (await seen())[0]]).toEqual([at, []]);
          await page.mouse.up();
        }
        const [eases, start, half, marked] = await seen();
        expect([at, eases, start[0], half[0], marked]).toEqual([at, eased.map((property) => [property, 250, 'ease-in-out']), ...(eased.includes('opacity') ? ['0.75', '0.875'] : ['1', '1']), true]);
        if (eased.includes('background-color')) expect([at, start[1] !== rest, half[1] !== rest && half[1] !== start[1]]).toEqual([at, true, true]);
        if (eased.includes('--exact-wash')) expect([at, start[2] > 0, half[2] > 0 && half[2] < start[2]]).toEqual([at, true, true]);
        // Once it ends the mark goes, and a change of state is the button's own again.
        await page.evaluate(() => document.getAnimations().forEach((a) => a.finish()));
        await page.waitForFunction((id) => !document.getElementById(id).hasAttribute('data-exact-released'), id);
        expect([at, await page.evaluate((id) => { const b = document.getElementById(id); b.disabled = true; getComputedStyle(b).backgroundColor; b.disabled = false; return [getComputedStyle(b).backgroundColor, b.getAnimations({ subtree: true }).length]; }, id)]).toEqual([at, [rest, 0]]);
      }
      if (finger) {
        // A pressable that is no native button is pressed under a finger but never marked released.
        await page.evaluate(() => { window.marks = []; new MutationObserver((records) => marks.push(...records.map((r) => r.attributeName))).observe(document.getElementById('box'), { attributeFilter: ['data-pressed', 'data-exact-released'] }); });
        await page.tap('#box');
        expect([engine, 'box', await page.evaluate(() => marks)]).toEqual([engine, 'box', ['data-pressed', 'data-pressed']]);
      } else {
        // Existing author transitions stay independent, including properties the platform also animates
        for (const [id, property, value] of [['translucent-face', 'color', 'rgb(255, 0, 0)'], ['translucent', 'opacity', '0.5'], ['translucent-face', 'background-color', 'rgb(255, 0, 0)']]) {
          await page.evaluate(([id, property, value]) => {
            const node = document.getElementById(id);
            node.style.transition = `${property} 3s linear`;
            getComputedStyle(node).getPropertyValue(property);
            node.style.setProperty(property, value);
          }, [id, property, value]);
          await page.click('#translucent');
          await page.evaluate(() => document.getAnimations().forEach((a) => a.effect.getTiming().duration < 3000 && a.finish()));
          await page.waitForFunction(() => !document.getElementById('translucent').hasAttribute('data-exact-released'), undefined, { timeout: 1000 });
          expect([engine, id, property, await page.evaluate((id) => document.getElementById(id).getAnimations().map((a) => a.transitionProperty), id)]).toEqual([engine, id, property, [property]]);
          await page.evaluate((id) => { document.getAnimations().forEach((a) => a.finish()); document.getElementById(id).removeAttribute('style'); }, id);
        }
      }
      await page.close();
    }
    // On the agent's clock the release lands at once.
    const page = await load('/unsaid?agent');
    await page.click('#text');
    expect([engine, 'agent', await page.evaluate(() => [getComputedStyle(document.getElementById('text-face')).opacity, document.getElementById('text').getAnimations({ subtree: true }).length, document.getElementById('text').hasAttribute('data-exact-released')])]).toEqual([engine, 'agent', ['1', 0, false]]);
  } finally { await browser.close(); }
}, 60000);

check('the pressed paint shows by the press rule, eased, and replaces the platform default', async () => {
  const { call, evaluate, centre, mouse, close } = await open('/paint');
  try {
    // A mixed colour computes as `color(srgb …)`; the rest as `rgb()`.
    await evaluate(`window.rgb = c => c.replace(/color\\(srgb ([\\d.]+) ([\\d.]+) ([\\d.]+)\\)/g, (_, ...v) => \`rgb(\${v.slice(0, 3).map(x => Math.round(x * 255)).join(', ')})\`)`);
    const look = (id) => evaluate(`(() => { const s = getComputedStyle(document.getElementById('${id}')); return rgb([s.opacity, s.backgroundColor, s.color].join(' ')); })()`);
    const ink = (id) => evaluate(`rgb(getComputedStyle(document.getElementById('${id}')).color)`);
    const settle = () => evaluate(`document.getAnimations().forEach(a => { if (Number.isFinite(a.effect.getComputedTiming().endTime)) a.finish(); })`);
    const rest = '0.8 rgb(255, 255, 255) rgb(0, 0, 0)';
    expect(await look('painted')).toBe(rest);

    // Held: half its own opacity, its background and text colours; a span
    // writing its own colour keeps it, and the card it is in shows nothing.
    const painted = await centre('painted');
    await mouse('mousePressed', painted);
    // Eased in on the press's curve: part way at 20 ms.
    await evaluate(`document.getElementById('painted').getAnimations().forEach(a => { a.pause(); a.currentTime = 20; })`);
    const part = parseFloat(await evaluate(`getComputedStyle(document.getElementById('painted')).opacity`));
    expect(part).toBeLessThan(0.8);
    expect(part).toBeGreaterThan(0.4);
    await settle();
    const held = await look('painted');
    expect(held).toBe('0.4 rgb(0, 0, 255) rgb(255, 0, 0)');
    expect([await ink('label'), await ink('kept')]).toEqual(['rgb(255, 0, 0)', 'rgb(0, 128, 0)']);
    expect(await evaluate(`getComputedStyle(document.getElementById('paintcard')).opacity`)).toBe('1');
    // Off the box it goes, and back it comes, as the scale does.
    await mouse('mouseMoved', [painted[0] + 150, painted[1]]);
    await settle();
    expect(await look('painted')).toBe(rest);
    await mouse('mouseMoved', painted);
    await settle();
    expect(await look('painted')).toBe('0.4 rgb(0, 0, 255) rgb(255, 0, 0)');
    await mouse('mouseReleased', painted);
    await settle();
    expect(await look('painted')).toBe(rest);
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    expect(await evaluate(`document.getElementById('painted').hasAttribute('data-exact-painting')`)).toBe(false);
    // An authored transition of the same properties running at the press is
    // left to run, above the paint (a transition is above every declaration
    // in the cascade); the paint shows as it ends, and released, the node
    // shows its own look again.
    await evaluate(`{ const el = document.getElementById('painted'); el.style.transition = 'background-color 1s linear, opacity 1s linear'; el.style.backgroundColor = 'rgb(255, 255, 0)'; el.style.opacity = '0.6'; getComputedStyle(el).opacity; }`);
    expect(await evaluate(`document.getElementById('painted').getAnimations().filter(a => a instanceof CSSTransition).length`)).toBe(2);
    await mouse('mousePressed', painted);
    await evaluate(`document.getAnimations().filter(a => !(a instanceof CSSTransition)).forEach(a => a.finish())`);
    expect(await evaluate(`document.getElementById('painted').getAnimations().filter(a => a instanceof CSSTransition).length`)).toBe(2);
    await settle();
    const faded = (await look('painted')).split(' ');
    expect(faded.slice(1).join(' ')).toBe('rgb(0, 0, 255) rgb(255, 0, 0)');
    expect(parseFloat(faded[0])).toBeLessThan(0.45);
    await mouse('mouseReleased', painted);
    await settle();
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    await settle();
    expect(await look('painted')).toBe('0.6 rgb(255, 255, 0) rgb(0, 0, 0)');
    await evaluate(`{ const el = document.getElementById('painted'); el.style.transition = ''; el.style.backgroundColor = 'rgb(255, 255, 255)'; el.style.opacity = '0.8'; }`);
    expect(await ink('label')).toBe('rgb(0, 0, 0)');
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    expect(await evaluate(`document.getElementById('painted').getAnimations().length`)).toBe(0);

    // A colour alone is the author's press too: no unsaid dim under it.
    const inked = await centre('inked');
    await mouse('mousePressed', inked);
    await settle();
    expect([await evaluate(`getComputedStyle(document.getElementById('inked')).opacity`), await ink('inked')]).toEqual(['1', 'rgb(255, 0, 0)']);
    await mouse('mouseReleased', inked);
    // It composes with the scale.
    const both = await centre('both');
    await mouse('mousePressed', both);
    await settle();
    expect(await evaluate(`(() => { const s = getComputedStyle(document.getElementById('both')); return s.opacity + ' ' + parseFloat(s.scale); })()`)).toBe('0.5 0.5');
    await mouse('mouseReleased', both);
    await settle();
    // A row's own press replaces the cell's pressed wash (what the pointer
    // over it shows is its hover's).
    const [x, y] = await centre('paintrow');
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none', buttons: 0 });
    const hovered = await evaluate(`getComputedStyle(document.getElementById('paintrow')).backgroundImage`);
    await mouse('mousePressed', [x, y]);
    await settle();
    expect(await evaluate(`(() => { const s = getComputedStyle(document.getElementById('paintrow')); return rgb(s.backgroundColor) + ' ' + s.backgroundImage; })()`)).toBe(`rgb(0, 0, 255) ${hovered}`);
    await mouse('mouseReleased', await centre('paintrow'));
    await settle();
    // A pressed opacity cleared while held stops showing at once: the press
    // is then the unsaid dim alone, never both.
    await mouse('mousePressed', inked);
    await evaluate(`document.getElementById('inked').style.setProperty('--exact-press-opacity', '0.5')`);
    await settle();
    expect(await evaluate(`getComputedStyle(document.getElementById('inked')).opacity`)).toBe('0.5');
    await evaluate(`{ const el = document.getElementById('inked'); el.style.removeProperty('--exact-press-opacity'); el.style.removeProperty('--exact-press-color'); }`);
    expect(await evaluate(`getComputedStyle(document.getElementById('inked')).opacity`)).toBe('0.65');
    await mouse('mouseReleased', inked);
    await settle();
    await evaluate(`document.getElementById('inked').style.setProperty('--exact-press-color', 'rgb(255, 0, 0)')`);
    // A press whose up never reaches the page (the window lost, the button
    // up outside it) lets go.
    for (const end of [`dispatchEvent(new Event('blur'))`, `document.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1, isPrimary: true, buttons: 0 }))`]) {
      await mouse('mousePressed', painted);
      expect(await evaluate(`document.getElementById('painted').hasAttribute('data-pressed')`)).toBe(true);
      await evaluate(end);
      await settle();
      expect([await evaluate(`document.getElementById('painted').hasAttribute('data-pressed')`), await look('painted')]).toEqual([false, rest]);
      await mouse('mouseReleased', painted);
      await settle();
    }
    // So does a press Space holds.
    await evaluate(`document.getElementById('both').focus()`);
    await call('Input.dispatchKeyEvent', { type: 'keyDown', key: ' ', code: 'Space', windowsVirtualKeyCode: 32, text: ' ' });
    expect(await evaluate(`document.getElementById('both').hasAttribute('data-pressed')`)).toBe(true);
    await evaluate(`dispatchEvent(new Event('blur'))`);
    expect(await evaluate(`document.getElementById('both').hasAttribute('data-pressed')`)).toBe(false);
    await call('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
    await evaluate(`document.activeElement.blur()`);
    await settle();

    // The node's own look changing while held is what the paint lies over,
    // and what the release eases back to; the node's transition list stays
    // its own, and a property painted from mid-press shows over a
    // transition of it.
    await mouse('mousePressed', painted);
    await settle();
    const own = await evaluate(`getComputedStyle(document.getElementById('painted')).transition`);
    await evaluate(`{ const el = document.getElementById('painted'); el.style.opacity = '0.6'; el.style.transition = 'color 2s linear, transform 1s'; }`);
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    await settle();
    expect(await evaluate(`getComputedStyle(document.getElementById('painted')).opacity`)).toBe('0.3');
    expect(await evaluate(`getComputedStyle(document.getElementById('painted')).transition`)).not.toBe(own);
    await mouse('mouseReleased', painted);
    await evaluate(`document.getAnimations().forEach(a => { a.pause(); a.currentTime = 60; })`);
    const easing = parseFloat(await evaluate(`getComputedStyle(document.getElementById('painted')).opacity`));
    expect(easing).toBeGreaterThan(0.3);
    expect(easing).toBeLessThan(0.6);
    await settle();
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    expect(await look('painted')).toBe('0.6 rgb(255, 255, 255) rgb(0, 0, 0)');
    await evaluate(`{ const el = document.getElementById('painted'); el.style.opacity = '0.8'; el.style.transition = ''; }`);
    await evaluate(`{ const el = document.getElementById('inked'); el.style.transition = 'background-color 1s linear'; el.style.backgroundColor = 'rgb(0, 255, 0)'; }`);
    await mouse('mousePressed', inked);
    await evaluate(`{ const el = document.getElementById('inked'); el.style.setProperty('--exact-press-background-color', 'rgb(0, 0, 255)'); }`);
    await evaluate('new Promise(ok => setTimeout(ok, 0))');
    await evaluate(`document.getAnimations().filter(a => !(a instanceof CSSTransition)).forEach(a => a.finish())`);
    expect(await evaluate(`rgb(getComputedStyle(document.getElementById('inked')).backgroundColor)`)).toBe('rgb(0, 0, 255)');
    await mouse('mouseReleased', inked);
    await settle();
  } finally {
    await close();
  }
  // Under the agent's clock it lands at once, both ways.
  const agent = await open('/paint?agent');
  try {
    const at = await agent.centre('painted');
    await agent.mouse('mousePressed', at);
    expect(await agent.evaluate(`getComputedStyle(document.getElementById('painted')).opacity`)).toBe('0.4');
    await agent.mouse('mouseReleased', at);
    expect(await agent.evaluate(`getComputedStyle(document.getElementById('painted')).opacity`)).toBe('0.8');
  } finally {
    await agent.close();
  }
}, 60000);
