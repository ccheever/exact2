// State looks in a real browser (LLP 1121 D8, stage 8 B): the rules the host
// writes into the page's looks sheet (src/host/looks.rs), over a node's inline
// base, with the shell's CSS and the input glue. `:active` is the press glue's
// `data-pressed`; the unsaid 65% stands aside; rows a look doesn't name show the
// base, also a base that moves while pressed; a disabled node takes none.
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
// As `looks::rules` writes them for an `:active` (a red face, a green top border) and a `:hover` (a blue face).
const ease = '120ms cubic-bezier(.16,1,.3,1)';
const rules = '@media (hover:hover){[data-view="3"]:hover:not(:disabled,[disabled="true"]){background-color:rgba(0,0,255,1)!important;}}'
  + '[data-view="3"][data-pressed]:not(:disabled,[disabled="true"]){background-color:rgba(255,0,0,1)!important;border-top-color:rgba(0,255,0,1)!important;}'
  + `[data-view="3"][data-exact-easing]{transition:background-color ${ease},border-top-color ${ease}!important}`;
const page = sheet + `<style id="exact-looks">${rules}</style>
<div id="exact-root" style="padding:20px">
  <div id="card" data-view="3" data-exact-on="press" data-exact-look="active hover" tabindex="0" style="width:100px;height:100px;background-color:#ffffff;border:2px solid #000000">c</div>
  <div id="away" style="height:100px"></div>
</div>
<script type="module">
  import { createInputHandlers } from './input-glue.js';
  createInputHandlers({ root: document.getElementById('exact-root'), views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  window.ready = true;
</script>`;

let server, profile, child, cdp;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'input-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  profile = mkdtempSync(resolve(tmpdir(), 'exact-looks-'));
  child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  cdp = new Cdp(child.stdio[3], child.stdio[4]);
});
afterAll(() => {
  child?.kill();
  server?.close();
  if (profile) rmSync(profile, { recursive: true, force: true });
});

check('a look composes over the base on the host\'s own press and hover', async () => {
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
    const at = (id) => evaluate(`(() => { const r = document.getElementById('${id}').getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    const mouse = (type, [x, y], buttons = 0) => call('Input.dispatchMouseEvent', { type, x, y, button: type === 'mouseMoved' ? 'none' : 'left', buttons, clickCount: 1 });
    const settle = () => evaluate(`(document.getAnimations().forEach(a => a.finish()), new Promise(r => requestAnimationFrame(() => r())))`); // and their finish events
    const look = async () => {
      await settle();
      return evaluate(`(() => { const s = getComputedStyle(document.getElementById('card')); return [s.backgroundColor, s.borderTopColor, s.borderRightColor, s.opacity]; })()`);
    };
    const easing = () => evaluate(`document.getElementById('card').hasAttribute('data-exact-easing')`);
    const card = await at('card'), away = await at('away');

    await mouse('mouseMoved', card);
    expect(await look()).toEqual(['rgb(0, 0, 255)', 'rgb(0, 0, 0)', 'rgb(0, 0, 0)', '1']); // :hover
    await mouse('mousePressed', card, 1);
    expect(await easing()).toBe(true); // its rows ease on the press curve
    // Only the authored overlay: red over the hover, the top border green, no 65%.
    expect(await look()).toEqual(['rgb(255, 0, 0)', 'rgb(0, 255, 0)', 'rgb(0, 0, 0)', '1']);
    expect(await easing()).toBe(false);
    // The base moves while pressed: rows the look names stay its, the rest show the new base.
    await evaluate(`Object.assign(document.getElementById('card').style, { backgroundColor: '#eeeeee', borderRightColor: '#123456' })`);
    expect(await look()).toEqual(['rgb(255, 0, 0)', 'rgb(0, 255, 0)', 'rgb(18, 52, 86)', '1']);
    await mouse('mouseReleased', card);
    expect(await easing()).toBe(true); // and back
    await mouse('mouseMoved', away);
    expect(await look()).toEqual(['rgb(238, 238, 238)', 'rgb(0, 0, 0)', 'rgb(18, 52, 86)', '1']);

    // Reduce Motion: the colours snap.
    await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await mouse('mousePressed', card, 1);
    expect(await easing()).toBe(false);
    expect(await look()).toEqual(['rgb(255, 0, 0)', 'rgb(0, 255, 0)', 'rgb(18, 52, 86)', '1']);
    await mouse('mouseReleased', card);
    await call('Emulation.setEmulatedMedia', { features: [] });

    // A disabled node takes no look: no hover, no press.
    await evaluate(`document.getElementById('card').setAttribute('disabled', 'true')`);
    await mouse('mouseMoved', card);
    await mouse('mousePressed', card, 1);
    expect(await look()).toEqual(['rgb(238, 238, 238)', 'rgb(0, 0, 0)', 'rgb(18, 52, 86)', '1']);
    await mouse('mouseReleased', card);
  } finally {
    await cdp.send('Target.closeTarget', { targetId });
  }
}, 60000);
