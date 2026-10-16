// `wakelock="screen"` on the web (LLP 1125 D11), wakelock-glue.js in a real
// browser over a stand-in `navigator.wakeLock` that counts what it is asked:
// the holders are the displayed elements that say `screen`; the lock is
// asked once while wanted, given back when not, asked again when the page is
// visible again after a hidden page lost it, and a missing API or a refused
// request is journaled once.
import { test, expect, beforeAll, afterAll } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { executable: chrome, unavailable } = chromium();
const check = need('chrome', unavailable);

// `?api=missing` removes the API, `?api=refuses` rejects every request.
const page = `
<div id="exact-root">
  <div id="a" data-exact-wakelock="screen">a</div>
  <div id="panel" style="display:none"><div><div id="hidden" data-exact-wakelock="screen">hidden</div></div></div>
  <div id="self" data-exact-wakelock="screen" style="display:none">self</div>
  <div id="none" data-exact-wakelock="none">none</div>
  <div id="bad" data-exact-wakelock="dim">bad</div>
</div>
<script type="module">
  const api = new URL(location.href).searchParams.get('api');
  window.asked = 0; window.released = 0; window.lines = []; window.vis = 'visible';
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => window.vis });
  const sentinels = [];
  const wakeLock = { request(type) {
    window.asked++;
    if (api === 'refuses') return Promise.reject(new DOMException('no', 'NotAllowedError'));
    const s = new EventTarget(); s.type = type; s.released = false;
    s.release = async () => { if (!s.released) { s.released = true; window.released++; s.dispatchEvent(new Event('release')); } };
    sentinels.push(s); return Promise.resolve(s);
  } };
  Object.defineProperty(navigator, 'wakeLock', { configurable: true, get: () => api === 'missing' ? undefined : wakeLock });
  // The page is hidden: the user agent releases the lock (Screen Wake Lock §"Handling document loss of visibility").
  window.hide = () => { window.vis = 'hidden'; for (const s of sentinels) if (!s.released) { s.released = true; s.dispatchEvent(new Event('release')); } document.dispatchEvent(new Event('visibilitychange')); };
  window.show = () => { window.vis = 'visible'; document.dispatchEvent(new Event('visibilitychange')); };
  const glue = await import('./wakelock-glue.js');
  window.glue = glue; window.log = line => lines.push(line);
  window.ready = true;
</script>`;

let server, profile, child, cdp;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    if (req.url.startsWith('/wakelock-glue.js')) { res.writeHead(200, { 'content-type': 'text/javascript' }); res.end(readFileSync(resolve(WEB, 'wakelock-glue.js'))); return; }
    res.writeHead(200, { 'content-type': 'text/html' }); res.end(page);
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  profile = mkdtempSync(resolve(tmpdir(), 'exact-wakelock-'));
  child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  cdp = new Cdp(child.stdio[3], child.stdio[4]);
});
afterAll(() => {
  child?.kill();
  server?.close();
  if (profile) rmSync(profile, { recursive: true, force: true });
});

async function open(path) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  const evaluate = async expression => {
    const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}${path}` });
  for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 2000) throw new Error('page never ready'); await Bun.sleep(5); }
  return { evaluate, close: () => cdp.send('Target.closeTarget', { targetId }) };
}

check('the holders are the displayed elements that say screen', async () => {
  const { evaluate, close } = await open('/');
  try {
    const holders = () => evaluate(`glue.holders(document.getElementById('exact-root')).map(el => el.id).join()`);
    expect(await holders()).toBe('a');
    await evaluate(`document.getElementById('panel').style.display = 'block'`);
    expect(await holders()).toBe('a,hidden');
    await evaluate(`document.getElementById('a').remove()`);
    expect(await holders()).toBe('hidden');
    expect(await evaluate(`glue.holders(null).length`)).toBe(0);
  } finally { await close(); }
});

check('asked once while wanted, given back, and asked again when visible again', async () => {
  const { evaluate, close } = await open('/');
  try {
    const counts = () => evaluate(`[asked, released, glue.granted()]`);
    await evaluate(`glue.hold(true, log)`);
    expect(await counts()).toEqual([1, 0, true]);
    await evaluate(`glue.hold(true, log)`);
    expect(await counts()).toEqual([1, 0, true]);
    await evaluate(`glue.hold(false, log)`);
    expect(await counts()).toEqual([1, 1, false]);
    await evaluate(`glue.hold(true, log)`);
    expect(await counts()).toEqual([2, 1, true]);
    // A hidden page loses it, and nothing is asked while hidden.
    await evaluate(`hide()`);
    expect(await counts()).toEqual([2, 1, false]);
    await evaluate(`show(); new Promise(r => setTimeout(r))`);
    expect(await counts()).toEqual([3, 1, true]);
    // Not wanted when the page shows again: nothing is asked.
    await evaluate(`glue.hold(false, log); hide(); show(); new Promise(r => setTimeout(r))`);
    expect(await counts()).toEqual([3, 2, false]);
    expect(await evaluate(`lines`)).toEqual([]);
  } finally { await close(); }
});

check('a missing API or a refused request is journaled once', async () => {
  for (const [api, line] of [['missing', 'wakelock: unavailable: navigator.wakeLock is missing'], ['refuses', 'wakelock: unavailable: the request was refused (NotAllowedError)']]) {
    const { evaluate, close } = await open(`/?api=${api}`);
    try {
      await evaluate(`(async () => { for (const on of [true, false, true, false, true]) { glue.hold(on, log); await new Promise(r => setTimeout(r)); } })()`);
      expect(await evaluate(`lines`)).toEqual([line]);
      expect(await evaluate(`glue.granted()`)).toBe(false);
    } finally { await close(); }
  }
});
