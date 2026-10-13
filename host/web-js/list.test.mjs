// App-owned virtual-list restoration through the browser runtime and real DOM geometry.
import { beforeAll, afterAll, test as bunTest, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp } from '../../scripts/agent.mjs';
import { chromium } from '../../scripts/agent-launch.mjs';

const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const test = unavailable ? (name, ...args) => bunTest.skip(`${name} — ${unavailable}`, ...args) : bunTest;
let server, child, cdp, evaluate, dir;
beforeAll(async () => {
  if (unavailable) return;
  dir = mkdtempSync(join(tmpdir(), 'exact-collection-'));
  server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/names.js') return new Response('export const sourceTypes = {};', {headers: {'content-type':'text/javascript'}});
    const file = existsSync(new URL(`.${path}`, import.meta.url)) ? `.${path}` : `../web${path}`;
    return new Response(path.endsWith('.js') ? readFileSync(new URL(file, import.meta.url)) : '<!doctype html><body><main id="exact-root"></main>', {
      headers: { 'content-type': path.endsWith('.js') ? 'text/javascript' : 'text/html' },
    });
  }});
  child = spawn(chrome, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--disable-background-networking', `--user-data-dir=${dir}`, 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  cdp = new Cdp(child.stdio[3], child.stdio[4]);
  child.on('exit', () => cdp.fail('Chrome closed'));
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  evaluate = async expression => {
    const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  };
  await call('Page.navigate', { url: `http://127.0.0.1:${server.port}` });
  await evaluate(`Promise.all([import('/rt.js'), import('/list.js')]).then(([rt, list]) => { globalThis.rt = rt; globalThis.list = list; rt.clock.agent = true; })`);
}, 60_000); // hang bounds: Chrome's launch and exit are awaited as events, and take longer than 5 s under load
afterAll(async () => {
  if (child && child.exitCode === null) { const exit = new Promise(r => child.once('exit', r)); child.kill(); await exit; }
  server?.stop(true);
  if (dir) rmSync(dir, { recursive: true, force: true });
}, 60_000);

test('a scroll event saves a row and an explicit restore survives remount and new measurements', async () => {
  const result = await evaluate(`(async () => {
    const root = document.getElementById('exact-root');
    const mount = changed => {
      const el = document.createElement('div');
      el.id = 'feed'; el.style.cssText = 'height:600px;width:400px;overflow:auto';
      root.append(el);
      let heard;
      rt.onScroll(el, 'scroll', (x, y, e) => { heard = e; }, (kind, f) => el.addEventListener(kind, f));
      const own = rt.scope(() => list.vl(el, () => Array.from({length:1000}, (_, i) => i), i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = String(i());
        row.style.height = (changed ? 80 + i() % 3 * 20 : 100) + 'px'; w.append(row);
      }, {est:100, pw:[], ph:[600]}));
      return {el, own, read:() => heard};
    };
    const first = mount(false);
    await exact.lists.pending(); await exact.lists.settle();
    await new Promise(resolve => { first.el.addEventListener('scroll', resolve, {once:true}); first.el.scrollTop = 1861; });
    await exact.lists.settle();
    first.el.dispatchEvent(new Event('scroll'));
    const saved = first.read();
    rt.end(first.own); first.el.remove(); rt.settled();
    const second = mount(true);
    rt.Hosts.scrollIntoView('feed', saved[6], null, null, null, null, saved[7]);
    await exact.lists.settle();
    const row = second.el.querySelector('[data-listitemkey="n:18"]');
    const within = row ? second.el.getBoundingClientRect().top - row.getBoundingClientRect().top : null;
    const status = exact.lists.intoView().at(-1)?.status;
    rt.end(second.own); second.el.remove(); rt.settled();
    return {key:saved[6], offset:saved[7], within, status};
  })()`);
  expect(result.key).toBe('18');
  expect(result.offset).toBe(61);
  expect(result.within).toBeCloseTo(61, 0);
  expect(result.status).toBe('done');
}, 60_000);
