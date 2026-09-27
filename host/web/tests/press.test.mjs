// Press feedback in a real browser (LLP 1061 D3): the shell's rule and the
// input glue, driven by CDP mouse events. UIKit's rule, not `:active`'s:
// only the innermost pressable shows the press, only while the pointer is
// inside its box, and reduced motion shows none.
import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const check = existsSync(chrome) ? test : test.skip;

// The shell's own stylesheet, with nodes as the host writes them: a card
// (0.9) holding a button (0.5), a plain pressable (no row) in the card, and
// a disabled button.
const page = readFileSync(resolve(WEB, 'index.html'), 'utf8').match(/<style>[\s\S]*?<\/style>/)[0] + `
<div id="exact-root" style="padding:20px">
  <div id="card" data-exact-on="press" style="--exact-press:0.9;width:300px;height:300px;padding:20px">
    <button id="button" data-exact-on="press" style="--exact-press:0.5;width:100px;height:100px">b</button>
    <div id="plain" data-exact-on="press" style="width:100px;height:50px">p</div>
    <button id="off" data-exact-on="press" disabled style="--exact-press:0.5;width:100px;height:50px">d</button>
  </div>
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
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=500,500', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => (await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result.value;
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    const centre = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    const mouse = (type, [x, y]) => call('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1 });
    const pressed = () => evaluate(`[...document.querySelectorAll('[data-pressed]')].map(el => el.id).join()`);
    const scale = (id) => evaluate(`new DOMMatrix(getComputedStyle(document.getElementById('${id}')).transform).a`);

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

    // Reduced motion: the node is still the pressed one, but nothing scales.
    await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await mouse('mousePressed', button);
    expect(await pressed()).toBe('button');
    expect(await scale('button')).toBe(1);
    await mouse('mouseReleased', button);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 60000);
