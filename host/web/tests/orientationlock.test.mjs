// `orientationlock` on the web (LLP 1133.029 D3, D7), orientationlock-glue.js
// in a real browser over a stand-in `screen.orientation` that records what it
// is asked: the counted holders follow the runner's three rules
// (runner/src/orientation_tests.rs) over the DOM navigation.js projects; the
// lock asks the set's type, `current` the orientation captured when the hold
// began, `natural` the API's own type; it is asked again on entering
// fullscreen and on becoming visible; a refusal is journaled once by its
// DOMException name. The JS target's orientationlock.js journals a bad word
// as the runner does.
import { test, expect, beforeAll, afterAll } from 'bun:test';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { cdpBrowser, chromium } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { unavailable } = chromium();
const check = need('chrome', unavailable);

// `?api=refuses` rejects every lock with NotSupportedError, as desktop Chrome does.
const STANDIN = `
  const api = new URL(location.href).searchParams.get('api');
  const o = { type: 'portrait-primary', angle: 0, asked: [], unlocked: 0,
    lock(type) { this.asked.push(type); return api === 'refuses' ? Promise.reject(new DOMException('no', 'NotSupportedError')) : Promise.resolve(); },
    unlock() { this.unlocked++; } };
  Object.defineProperty(screen, 'orientation', { configurable: true, get: () => o });
  window.o = o; window.vis = 'visible'; window.full = null; window.lines = [];
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => window.vis });
  Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => window.full });
  window.hide = () => { window.vis = 'hidden'; document.dispatchEvent(new Event('visibilitychange')); };
  window.show = () => { window.vis = 'visible'; document.dispatchEvent(new Event('visibilitychange')); };
  window.fullscreen = on => { window.full = on ? document.body : null; document.dispatchEvent(new Event('fullscreenchange')); };`;

// A page, two sheets over it, a dialog and a popover, projected by navigation.js.
const page = `<div id="exact-root">
  <main id="nav" navigationKey="home" navigationBack="back">
    <div navigationKey="home"><div id="root" data-exact-orientationlock="portrait"><div id="child" data-exact-orientationlock="none"></div></div><div id="side" data-exact-orientationlock="none"></div></div>
    <div navigationKey="a" navigationPresentation="modal"><div id="sa" data-exact-orientationlock="landscape"></div></div>
    <div navigationKey="b" navigationPresentation="modal"><div id="sb" data-exact-orientationlock="none"></div></div>
  </main>
  <dialog id="dialog"><div id="in-dialog" data-exact-orientationlock="landscape"></div></dialog>
  <div id="pop" popover><div id="in-popover" data-exact-orientationlock="landscape"></div></div>
</div>
<script type="module">
  ${STANDIN}
  const { navigation } = await import('./navigation.js');
  const glue = await import('./orientationlock-glue.js');
  const root = document.getElementById('exact-root');
  navigation.connect(root, () => false, () => {});
  window.select = key => { document.getElementById('nav').setAttribute('navigationKey', key); navigation.project(root, () => {}); };
  window.say = (id, word) => document.getElementById(id).setAttribute('data-exact-orientationlock', word);
  window.counted = () => { const s = glue.session(root); return [s.by.map(el => el.id).join(), s.types]; };
  window.apply = () => glue.hold(glue.session(root), line => lines.push(line));
  window.glue = glue; window.root = root;
  select('home');
  window.ready = true;
</script>`;

const RT = `export const After = [], journal = [], clock = { now: 0 };
const ids = new Map(); export const viewId = el => ids.get(el) ?? (ids.set(el, ids.size + 1), ids.size);`;
const jsPage = `<div id="exact-root"><div id="world" data-exact-orientationlock="current"></div></div>
<script type="module">
  ${STANDIN}
  import { After, journal } from './rt.js';
  import { install } from './orientationlock.js';
  install();
  const el = document.getElementById('world');
  // A commit whose binding computed v, as the emitted code writes it (host/web-js/src/emit.rs).
  window.commit = v => { el.$orient = v; el.setAttribute('data-exact-orientationlock', String(v)); for (const f of After) f(); };
  window.journal = journal;
  window.ready = true;
</script>`;

const script = body => [200, { 'content-type': 'text/javascript' }, body];
let server, suite, cdp;
beforeAll(async () => {
  if (unavailable) return;
  server = createServer((req, res) => {
    const path = req.url.split('?')[0];
    const [status, headers, body] = ['/orientationlock-glue.js', '/wakelock-glue.js', '/navigation.js', '/grant-admission.js'].includes(path) ? script(readFileSync(resolve(WEB, path.slice(1))))
      : path === '/orientationlock.js' ? script(readFileSync(resolve(WEB, '../web-js/orientationlock.js')))
      : path === '/rt.js' ? script(RT)
      : [200, { 'content-type': 'text/html' }, path === '/js' ? jsPage : page];
    res.writeHead(status, headers); res.end(body);
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  suite = await cdpBrowser();
  ({ cdp } = suite);
});
afterAll(async () => {
  await suite?.close();
  server?.close();
});

async function open(path) {
  const { targetId, sessionId } = await suite.page();
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
const tick = 'new Promise(r => setTimeout(r))';

check('the counted holders follow the three rules, and a top-layer box holds nothing', async () => {
  const { evaluate, close } = await open('/');
  try {
    expect(await evaluate('counted()')).toEqual(['root', 3]);
    // The nearest holder wins: a landscape child replaces its portrait ancestor; `any` frees it.
    await evaluate(`say('child', 'landscape')`);
    expect(await evaluate('counted()')).toEqual(['child', 12]);
    await evaluate(`say('child', 'any')`);
    expect(await evaluate('counted()')).toEqual(['child', 15]);
    // Unrelated holders intersect, and share nothing in a conflict.
    await evaluate(`say('child', 'none'); say('side', 'portrait-primary')`);
    expect(await evaluate('counted()')).toEqual(['root,side', 1]);
    await evaluate(`say('side', 'landscape')`);
    expect(await evaluate('counted()')).toEqual(['root,side', 0]);
    await evaluate(`say('side', 'none')`);
    // The topmost sheet that holds wins over the page it covers.
    await evaluate(`select('a')`);
    expect(await evaluate('counted()')).toEqual(['sa', 12]);
    // A sheet that holds nothing leaves the lock of the sheet beneath it.
    await evaluate(`select('b')`);
    expect(await evaluate('counted()')).toEqual(['sa', 12]);
    await evaluate(`say('sb', 'portrait')`);
    expect(await evaluate('counted()')).toEqual(['sb', 3]);
    await evaluate(`select('home')`);
    expect(await evaluate('counted()')).toEqual(['root', 3]);
    // An open dialog or popover holds nothing.
    await evaluate(`document.getElementById('dialog').showModal(); document.getElementById('pop').showPopover()`);
    expect(await evaluate('counted()')).toEqual(['root', 3]);
  } finally { await close(); }
});

check('the lock asks the type, captures current once a hold, and is let go', async () => {
  const { evaluate, close } = await open('/');
  try {
    await evaluate(`apply(); ${tick}`);
    expect(await evaluate('[o.asked, glue.state(root).applied.held]')).toEqual([['portrait'], true]);
    await evaluate(`apply(); ${tick}`);
    expect(await evaluate('o.asked')).toEqual(['portrait']);
    // `current` is the orientation when the lock began, kept while it holds.
    await evaluate(`o.type = 'landscape-secondary'; o.angle = 270; say('root', 'current'); apply(); ${tick}`);
    await evaluate(`o.type = 'landscape-primary'; o.angle = 90; apply(); ${tick}`);
    expect(await evaluate('o.asked')).toEqual(['portrait', 'landscape-secondary']);
    // `natural` alone is the API's own type.
    await evaluate(`say('root', 'natural'); apply(); ${tick}`);
    expect(await evaluate('o.asked.at(-1)')).toBe('natural');
    // A conflict holds none, journaled; no holder lets the lock go.
    await evaluate(`say('root', 'portrait'); say('side', 'landscape'); apply(); ${tick}`);
    expect(await evaluate('[lines, glue.state(root).held, glue.state(root).conflict.length]')).toEqual([['orientationlock: conflict: view root "portrait", view side "landscape"; none held'], false, 2]);
    await evaluate(`say('root', 'none'); say('side', 'none'); apply(); ${tick}`);
    expect(await evaluate('[o.unlocked, glue.state(root).held]')).toEqual([1, false]);
  } finally { await close(); }
});

check('a refusal is journaled once, and the lock is asked again in fullscreen and when visible', async () => {
  const { evaluate, close } = await open('/?api=refuses');
  try {
    await evaluate(`apply(); ${tick}`);
    expect(await evaluate('[lines, glue.state(root).applied.held, glue.state(root).applied.why]')).toEqual([['orientationlock: unavailable: NotSupportedError'], false, 'NotSupportedError']);
    await evaluate(`fullscreen(true); ${tick}`);
    await evaluate(`hide(); show(); ${tick}`);
    expect(await evaluate('[o.asked.length, lines.length]')).toEqual([3, 1]);
    // Nothing is asked while hidden.
    await evaluate(`hide(); fullscreen(true); ${tick}`);
    expect(await evaluate('o.asked.length')).toBe(3);
  } finally { await close(); }
});

// The runner's side is runner/src/orientation_tests.rs.
check('the JS target journals a bad word as the runner does, and reads the state', async () => {
  const { evaluate, close } = await open('/js');
  try {
    await evaluate(`for (const v of ['current', 'sideways', 'current', 'sideways', 'current']) commit(v); ${tick}`);
    expect(await evaluate(`journal.map(l => l.replace(/^t=\\d+ /, ''))`)).toEqual([
      'view 1: invalid orientationlock value "sideways"; unset',
      'view 1: invalid orientationlock value "sideways"; unset',
    ]);
    const state = await evaluate('exact.orientationlock()');
    expect([state.held, state.by, state.words, state.lock.current, state.applied.asked]).toEqual([true, [1], ['current'], true, 'portrait-primary']);
  } finally { await close(); }
});
