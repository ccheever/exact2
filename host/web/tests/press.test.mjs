// Press feedback in a real browser (LLP 1061 D3): the shell's CSS and the
// input glue, driven by CDP mouse events. UIKit's rule, not `:active`'s:
// only the innermost pressable shows the press, only while the pointer is
// inside its box, including under reduced motion.
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

// The shell's own stylesheet, with nodes as the host writes them: a card
// (0.9) holding a button (0.5), a plain pressable (no row) in the card, and
// a disabled button.
const page = readFileSync(resolve(WEB, 'index.html'), 'utf8').match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root" style="padding:20px">
  <div id="card" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.9;width:300px;height:300px;padding:20px">
    <button id="button" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;width:100px;height:100px">b</button>
    <div id="plain" data-exact-on="press" style="width:100px;height:50px">p</div>
    <button id="off" data-exact-on="press" disabled style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;width:100px;height:50px">d</button>
  </div>
  <button id="scaled" data-exact-on="press" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;width:100px;height:60px;--exact-scale:1.5;transform-origin:0 0">s</button>
  <svg width="400" height="180"><rect id="svg" data-exact-on="press" x="0" y="0" width="100" height="60" transform="translate(100 40) rotate(20)" style="scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.5;transform-origin:20px 10px;fill:red" /></svg>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  const root = document.getElementById('exact-root');
  createInputHandlers({ root, views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  window.ready = true;
</script>`;

check('only the innermost pressable shows the press, and only while inside', async () => {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-press-'));
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
    const centre = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    const mouse = (type, [x, y]) => call('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1 });
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

    // The innermost pressable has no row: nothing shows, not even its card.
    await mouse('mousePressed', await centre('plain'));
    expect(await pressed()).toBe('');
    await mouse('mouseReleased', await centre('plain'));
    // A disabled button gives nothing either; its card is not pressed through it.
    await mouse('mousePressed', await centre('off'));
    expect(await pressed()).toBe('');
    await mouse('mouseReleased', await centre('off'));

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
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 60000);
