// State looks in a real browser (LLP 1121 D8, stage 8 B): a `:hover` look as the
// host writes it into the page's looks sheet (src/host/looks.rs), over a node's
// inline base, at once; `:active` lowered to the press paint (LLP 1061 D7), whose
// opacity is an absolute target eased from what shows (the hover's included) and
// whose paint goes over the hover; rows neither names show the base, also a base
// that moves while pressed; a disabled node takes no look.
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
// As `looks::rules` writes a `:hover` look (a yellow face at 0.8); `:active` is the node's press paint as the
// host writes it (LLP 1061 D7: a blue face, an absolute opacity of 0.6).
const on = ':not(:disabled,[disabled]:not([disabled="false"]))';
const rules = `@media (hover:hover){[data-view="3"]:hover${on}{background-color:rgba(255,255,0,1)!important;opacity:0.8!important;}}`;
const press = 'scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.95;--exact-press-background-color:rgba(0,0,255,1);--exact-press-opacity-to:0.6;';
const page = sheet + `<style id="exact-looks">${rules}</style>
<div id="exact-root" style="padding:20px">
  <div id="card" data-view="3" data-exact-on="press" tabindex="0" style="${press}width:100px;height:100px;background-color:#ffffff;border:2px solid #000000">c</div>
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

check('a hover look composes over the base, under the press paint', async () => {
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
    const now = () => evaluate(`(() => { const s = getComputedStyle(document.getElementById('card')); return [s.backgroundColor, s.borderRightColor, Number(s.opacity).toFixed(2)]; })()`);
    const look = async () => { await settle(); return now(); };
    const card = await at('card'), away = await at('away');

    await mouse('mouseMoved', card);
    expect(await now()).toEqual(['rgb(255, 255, 0)', 'rgb(0, 0, 0)', '0.80']); // :hover, at once
    await mouse('mousePressed', card, 1);
    // The press eases from what shows, the hover's 0.8, to its absolute 0.6 (not 0.6 of 0.8).
    const opacity = Number((await now())[2]);
    expect(opacity).toBeGreaterThanOrEqual(0.6);
    expect(opacity).toBeLessThanOrEqual(0.8);
    expect(await look()).toEqual(['color(srgb 0 0 1)', 'rgb(0, 0, 0)', '0.60']); // the press paint over the hover
    // The base moves while pressed: a row the press doesn't paint shows the new base.
    await evaluate(`Object.assign(document.getElementById('card').style, { backgroundColor: '#eeeeee', borderRightColor: '#123456' })`);
    expect(await look()).toEqual(['color(srgb 0 0 1)', 'rgb(18, 52, 86)', '0.60']);
    await mouse('mouseReleased', card);
    expect(await look()).toEqual(['rgb(255, 255, 0)', 'rgb(18, 52, 86)', '0.80']); // the hover again
    await mouse('mouseMoved', away);
    expect(await now()).toEqual(['rgb(238, 238, 238)', 'rgb(18, 52, 86)', '1.00']); // and the new base, at once

    // Reduce Motion: the press's colours still ease (the press is feedback); a hover still snaps.
    await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await mouse('mouseMoved', card);
    expect(await now()).toEqual(['rgb(255, 255, 0)', 'rgb(18, 52, 86)', '0.80']);
    await mouse('mousePressed', card, 1);
    expect(await evaluate(`document.getElementById('card').hasAttribute('data-exact-painting')`)).toBe(true);
    expect((await now())[0]).not.toBe('color(srgb 0 0 1)'); // eased in, not snapped
    await mouse('mouseReleased', card);
    await settle();
    await call('Emulation.setEmulatedMedia', { features: [] });

    // A disabled node takes no look (a box says `disabled` as the hosts write it).
    await mouse('mouseMoved', away);
    await evaluate(`document.getElementById('card').setAttribute('disabled', '')`);
    await mouse('mouseMoved', card);
    expect(await look()).toEqual(['rgb(238, 238, 238)', 'rgb(18, 52, 86)', '1.00']);
  } finally {
    await cdp.send('Target.closeTarget', { targetId });
  }
}, 60000);
