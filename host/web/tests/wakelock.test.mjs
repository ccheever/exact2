// `wakelock="screen"` on the web (LLP 1125 D11), wakelock-glue.js in a real
// browser over a stand-in `navigator.wakeLock` that counts what it is asked:
// the holders are the displayed elements that say `screen` (not under a
// route or tab the navigation hides, nor an exit animation's ghost); the
// lock is asked once while wanted, given back when not, asked again when a
// visible page lost it, and a missing API or a refused request is journaled
// once. The JS target's wakelock.js journals a bad word as the runner does.
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
  <div navigationKey="r1" style="visibility:hidden"><div id="under-route" data-exact-wakelock="screen"></div></div>
  <div navigationKey="r2"><div id="under-sheet" data-exact-wakelock="screen"></div></div>
  <div role="tabpanel" style="visibility:hidden"><div id="in-tab" data-exact-wakelock="screen"></div></div>
  <div style="visibility:hidden"><div id="authored" data-exact-wakelock="screen"></div></div>
  <div data-exiting><div id="leaving" data-exact-wakelock="screen"></div></div>
</div>
<script type="module">
  const api = new URL(location.href).searchParams.get('api');
  window.asked = 0; window.released = 0; window.lines = []; window.vis = 'visible'; window.refuse = false;
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => window.vis });
  const sentinels = [];
  const wakeLock = { request(type) {
    window.asked++;
    if (api === 'refuses') return Promise.reject(new DOMException('no', 'NotAllowedError'));
    if (window.refuse) return new Promise((_, no) => setTimeout(() => no(new DOMException('hidden', 'NotAllowedError'))));
    const s = new EventTarget(); s.type = type; s.released = false;
    s.release = async () => { if (!s.released) { s.released = true; window.released++; s.dispatchEvent(new Event('release')); } };
    sentinels.push(s); return Promise.resolve(s);
  } };
  Object.defineProperty(navigator, 'wakeLock', { configurable: true, get: () => api === 'missing' ? undefined : wakeLock });
  // The page is hidden: the user agent releases the lock (Screen Wake Lock §"Handling document loss of visibility").
  window.hide = () => { window.vis = 'hidden'; for (const s of sentinels) if (!s.released) { s.released = true; s.dispatchEvent(new Event('release')); } document.dispatchEvent(new Event('visibilitychange')); };
  // Hidden with the release's event not yet run: the sentinel says released, nothing has heard it.
  window.hideQuietly = () => { window.vis = 'hidden'; for (const s of sentinels) s.released = true; document.dispatchEvent(new Event('visibilitychange')); };
  // The user agent releases a visible page's lock (a power policy), and its event runs.
  window.drop = () => { for (const s of [...sentinels]) if (!s.released) { s.released = true; s.dispatchEvent(new Event('release')); } };
  window.show = () => { window.vis = 'visible'; document.dispatchEvent(new Event('visibilitychange')); };
  const glue = await import('./wakelock-glue.js');
  window.glue = glue; window.log = line => lines.push(line);
  window.ready = true;
</script>`;

// The JS target's wakelock.js over a stand-in for the four names it takes from rt.js.
const RT = `export const After = [], journal = [], clock = { now: 0 };
const ids = new Map(); export const viewId = el => ids.get(el) ?? (ids.set(el, ids.size + 1), ids.size);`;
const jsPage = `<div id="exact-root"><div id="player" data-exact-wakelock="screen"></div></div>
<script type="module">
  import { After, journal } from './rt.js';
  import { install } from './wakelock.js';
  Object.defineProperty(navigator, 'wakeLock', { configurable: true, get: () => undefined });
  install();
  const el = document.getElementById('player');
  // A commit whose binding computed v, as the emitted code writes it (host/web-js/src/emit.rs).
  window.commit = v => { el.$wake = v; el.setAttribute('data-exact-wakelock', String(v)); for (const f of After) f(); };
  window.journal = journal;
  window.ready = true;
</script>`;

// A counting stand-in for the API, for the navigation pages below.
const FAKE = `window.asked = 0;
  Object.defineProperty(navigator, 'wakeLock', { configurable: true, get: () => ({ request() {
    window.asked++; const s = new EventTarget(); s.released = false; s.release = async () => { s.released = true; s.dispatchEvent(new Event('release')); };
    return Promise.resolve(s);
  } }) });`;
// A stack whose root route is a split's list (an explicit column width) holding
// the lock, under its detail: hidden on a narrow window, shown beside the detail
// on a wide one (navigation.js `splitOf`, 900 px). Projected by navigation.js.
const navPage = `<div id="exact-root" style="position:relative;width:100vw;height:100vh">
  <main id="nav" navigationKey="detail" navigationBack="back" style="position:absolute;inset:0">
    <div navigationKey="list" navigationSplit="320" style="position:absolute;inset:0"><div id="player" data-exact-wakelock="screen">p</div></div>
    <div navigationKey="detail" style="position:absolute;inset:0">detail</div>
  </main>
</div>
<script type="module">
  ${FAKE}
  const { navigation } = await import('./navigation.js');
  const glue = await import('./wakelock-glue.js');
  const root = document.getElementById('exact-root');
  navigation.connect(root, () => false, () => {});
  navigation.project(root, () => {});
  glue.hold(glue.holders(root).length > 0, () => {});
  window.glue = glue; window.root = root;
  window.ready = true;
</script>`;
// The JS target's order: wakelock.js's commit callback comes before the
// navigation projection in rt.js's After (document.js pushes it later).
const navJsPage = `<div id="exact-root">
  <main id="nav" navigationKey="a" navigationBack="back">
    <div role="tablist"><button role="tab" aria-controls="p1">1</button><button role="tab" aria-controls="p2">2</button></div>
    <div role="tabpanel" id="p1"><div navigationKey="a">a</div></div>
    <div role="tabpanel" id="p2"><div navigationKey="b"><div id="player" data-exact-wakelock="screen">p</div></div></div>
  </main>
</div>
<script type="module">
  ${FAKE}
  import { After } from './rt.js';
  import { install } from './wakelock.js';
  import { navigation } from './navigation.js';
  import { granted } from './wakelock-glue.js';
  const root = document.getElementById('exact-root');
  install();
  navigation.connect(root, () => false, () => {});
  After.push(() => navigation.project(root, () => {}));
  window.commit = key => { document.getElementById('nav').setAttribute('navigationKey', key); for (const f of After) f(); };
  window.granted = granted;
  commit('a');
  window.ready = true;
</script>`;

const script = body => [200, { 'content-type': 'text/javascript' }, body];
let server, profile, child, cdp;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    const [status, headers, body] = req.url.startsWith('/wakelock-glue.js') ? script(readFileSync(resolve(WEB, 'wakelock-glue.js')))
      : req.url === '/wakelock.js' ? script(readFileSync(resolve(WEB, '../web-js/wakelock.js')))
      : req.url === '/rt.js' ? script(RT)
      : req.url === '/navigation.js' || req.url === '/grant-admission.js' ? script(readFileSync(resolve(WEB, req.url.slice(1)))) // navigation.js re-exports the grant admission
      : [200, { 'content-type': 'text/html' }, { '/js': jsPage, '/nav': navPage, '/navjs': navJsPage }[req.url] ?? page];
    res.writeHead(status, headers); res.end(body);
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
  return { call, evaluate, close: () => cdp.send('Target.closeTarget', { targetId }) };
}

check('the holders are the displayed elements that say screen', async () => {
  const { evaluate, close } = await open('/');
  try {
    const holders = () => evaluate(`glue.holders(document.getElementById('exact-root')).map(el => el.id).join()`);
    // The route a sheet covers is shown, and an authored `visibility` is no
    // rule of the runner's (runner/src/wakelock.rs), so both hold.
    expect(await holders()).toBe('a,under-sheet,authored');
    await evaluate(`document.getElementById('panel').style.display = 'block'`);
    expect(await holders()).toBe('a,hidden,under-sheet,authored');
    await evaluate(`document.getElementById('a').remove()`);
    expect(await holders()).toBe('hidden,under-sheet,authored');
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

check('a release not yet heard, or one the user agent makes, is asked for again', async () => {
  const { evaluate, close } = await open('/');
  try {
    const counts = () => evaluate(`[asked, glue.granted()]`);
    await evaluate(`glue.hold(true, log)`);
    expect(await counts()).toEqual([1, true]);
    // Shown again before the release's event ran: the released sentinel is not kept.
    await evaluate(`hideQuietly(); show(); new Promise(r => setTimeout(r))`);
    expect(await counts()).toEqual([2, true]);
    // Released while visible and still wanted: asked again at once.
    await evaluate(`drop(); new Promise(r => setTimeout(r))`);
    expect(await counts()).toEqual([3, true]);
    expect(await evaluate(`lines`)).toEqual([]);
  } finally { await close(); }
});

check('a request refused while the page is hidden is asked again and not journaled', async () => {
  const { evaluate, close } = await open('/');
  try {
    await evaluate(`refuse = true; glue.hold(true, log); hide(); new Promise(r => setTimeout(r, 10))`);
    expect(await evaluate(`[asked, glue.granted(), lines]`)).toEqual([1, false, []]);
    await evaluate(`refuse = false; show(); new Promise(r => setTimeout(r))`);
    expect(await evaluate(`[asked, glue.granted(), lines]`)).toEqual([2, true, []]);
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

// The runner's side of the same sequence is runner/src/wakelock_tests.rs
// `a_bad_word_is_journaled_each_time_it_comes_back`.
check('the JS target journals a bad word as the runner does: raw, each time it comes back', async () => {
  const { evaluate, close } = await open('/js');
  try {
    await evaluate(`for (const v of ['screen', 'dim', 'screen', 'dim', 'dim', 1]) commit(v)`);
    expect(await evaluate(`journal.map(l => l.replace(/^t=\\d+ /, ''))`)).toEqual([
      'wakelock: unavailable: navigator.wakeLock is missing',
      'view 1: invalid wakelock value "dim"; unset',
      'view 1: invalid wakelock value "dim"; unset',
      'view 1: invalid wakelock value 1; unset',
    ]);
  } finally { await close(); }
});

check('the lock follows a window crossing the split width, with no commit', async () => {
  const { call, evaluate, close } = await open('/nav');
  const width = async w => {
    await call('Emulation.setDeviceMetricsOverride', { width: w, height: 700, deviceScaleFactor: 1, mobile: false });
    for (let i = 0; i < 200 && (await evaluate('innerWidth')) !== w; i++) await Bun.sleep(5);
    await evaluate('new Promise(r => setTimeout(r, 20))');
  };
  try {
    await width(600);
    expect(await evaluate('[glue.holders(root).length, glue.granted()]')).toEqual([0, false]);
    await width(1000);
    expect(await evaluate('[glue.holders(root).length, glue.granted()]')).toEqual([1, true]);
    await width(600);
    expect(await evaluate('[glue.holders(root).length, glue.granted()]')).toEqual([0, false]);
  } finally { await close(); }
});

check('on the JS target a commit that switches tabs moves the lock in that commit', async () => {
  const { evaluate, close } = await open('/navjs');
  const settle = () => evaluate('new Promise(r => setTimeout(r))');
  try {
    await settle();
    expect(await evaluate('[exact.wakelock().held, granted()]')).toEqual([false, false]);
    await evaluate(`commit('b')`); await settle();
    expect(await evaluate('[exact.wakelock().held, granted()]')).toEqual([true, true]);
    await evaluate(`commit('a')`); await settle();
    expect(await evaluate('[exact.wakelock().held, granted()]')).toEqual([false, false]);
  } finally { await close(); }
});
