// The grouped reorder's ghost (LLP 1094 D6) in a real browser: a deep clone
// of the row in a manual popover (the top layer), carrying the row's look
// inline and none of the five identities a node of the app's has, so no
// glue adopts it, no handler finds it and no driver names it.
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

const page = `<!doctype html>
<style>#exact-root .card { display: flex; gap: 6px; background: rgb(1, 2, 3); }</style>
<div id="exact-root">
  <div data-listitemkey="s:c1" data-view="5" id="wrap" style="translate: 0px 40px; transition: translate 1s">
    <div class="card" id="card" data-view="6" data-exact-on="press" data-agent-view="6" data-testid="card-c1">
      <div id="grip" data-view="7" data-testid="grip-c1" style="width: 20px; height: 20px">g</div>
      <span data-testid="title">Draft</span>
    </div>
  </div>
</div>
<script type="module">
  import { ghostOf } from './group-glue.js';
  const g = ghostOf(document.getElementById('wrap'), false);
  const all = [g.el, ...g.el.querySelectorAll('*')];
  window.result = {
    open: g.el.matches(':popover-open'),
    left: ${'`${STRIP}`'}.split(',').filter(a => all.some(el => el.hasAttribute(a))),
    card: getComputedStyle(g.el.firstElementChild.firstElementChild).backgroundColor,
    translate: g.el.firstElementChild.style.translate,
    pointer: getComputedStyle(g.el).pointerEvents,
    shadow: g.el.style.boxShadow,
  };
</script>`.replace('${STRIP}', 'id,data-view,data-exact-on,data-agent-view,data-testid');

check('the ghost is a stripped clone in the top layer, with the row\'s look', async () => {
  const server = createServer((req, res) => {
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'group-glue.js')));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-group-'));
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,900', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => (await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result.value;
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    let r = null;
    for (let i = 0; !(r = await evaluate('window.result')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
    expect(r.open).toBe(true);
    expect(r.left).toEqual([]);
    expect(r.card).toBe('rgb(1, 2, 3)'); // the page's rule, inlined: the top layer is outside the root
    expect(r.translate).toBe('none'); // the row's own offset is not the ghost's
    expect(r.pointer).toBe('none');
    expect(r.shadow).toContain('24px');
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}, 30_000);
