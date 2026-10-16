// Press feedback in a real browser: the shell's CSS and the input glue,
// driven by CDP mouse and key events. A declared press (LLP 1061 D3) goes by
// UIKit's rule, not `:active`'s: only the innermost pressable shows it, only
// while the pointer is inside its box, including under reduced motion. A
// pressable that declares none dims while the browser holds it `:active`
// (LLP 1115.000 D6).
import { test, expect, beforeAll, afterAll } from 'bun:test';
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
// author's (a scale, `pointerdown`, an `opacity` of its own), a native
// button, a game's action, a disabled custom button, rows of a list (plain,
// virtualized, grouped, and a button inside a grouped row), an item of a
// sideways virtualized list, and a scrim over the window, shown on demand.
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
  createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  window.ready = true;
</script>`;

// One server and one browser for the file; each test opens a page of its own.
let server, profile, child, cdp;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    const html = { '/': page, '/unsaid': unsaid }[req.url];
    if (html) { res.writeHead(200, { 'content-type': 'text/html' }); res.end(html); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  profile = mkdtempSync(resolve(tmpdir(), 'exact-press-'));
  child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  cdp = new Cdp(child.stdio[3], child.stdio[4]);
});
afterAll(() => {
  child?.kill();
  server?.close();
  if (profile) rmSync(profile, { recursive: true, force: true });
});
/** A page at `path`, ready: its CDP `call` and `evaluate`, a node's `centre`, and the left button by `mouse`. */
async function open(path) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
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
    // A native button highlights with no row or handler of its own: the
    // look's wash of its ink over its face (LLP 1116 D1's dimmed the title
    // instead, as a UIButton does), nothing scales, its card is not pressed.
    // The unsaid dim is not laid over it.
    const face = () => evaluate(`getComputedStyle(document.getElementById('title')).opacity`);
    const wash = () => evaluate(`getComputedStyle(document.getElementById('native')).backgroundImage !== 'none'`);
    expect(await wash()).toBe(false);
    await mouse('mousePressed', await centre('native'));
    expect(await pressed()).toBe('native');
    expect([await wash(), await face()]).toEqual([true, '1']);
    expect(await opacity('native')).toBe('1');
    expect(await scale('native')).toBe(1);
    expect(await scale('card')).toBe(1);
    await mouse('mouseReleased', await centre('native'));
    expect(await pressed()).toBe('');
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 }); // off it: a pointer left over it is its hover, a wash too
    expect([await wash(), await face()]).toEqual([false, '1']);
    // The keyboard shows the same state (LLP 1115.000 D4.14): Space holds a
    // native button's highlight, and a button's `-exact-press-scale`, until
    // its keyup.
    const space = (type) => call('Input.dispatchKeyEvent', { type, key: ' ', code: 'Space', windowsVirtualKeyCode: 32, ...(type === 'keyDown' ? { text: ' ' } : {}) });
    await evaluate(`document.getElementById('native').focus()`);
    await space('keyDown');
    expect([await pressed(), await wash(), await opacity('native')]).toEqual(['native', true, '1']);
    await space('keyUp');
    expect([await pressed(), await wash()]).toEqual(['', false]);
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
    expect(await evaluate('clicks.join()')).toBe('custom,box,box,row');
    // Another key's release leaves the mark; it goes when the focus leaves with Space still down.
    await evaluate(`document.getElementById('box').focus()`);
    await key('keyDown', 'Space');
    await key('keyUp', 'Enter');
    expect(await marked('box', 'data-pressed')).toBe(true);
    await evaluate(`document.getElementById('custom').focus()`);
    expect([await marked('box', 'data-pressed'), await shown('box')]).toEqual([false, '1']);
    await key('keyUp', 'Space');
  } finally {
    await close();
  }
}, 60000);
