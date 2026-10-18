// App-owned virtual-list restoration through the browser runtime and real DOM geometry.
import { beforeAll, afterAll, test as bunTest, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp } from '../../scripts/agent.mjs';
import { chromium, DESKTOP_INPUT } from '../../scripts/agent-launch.mjs';
import { need } from '../../scripts/coverage.mjs';

const { executable: chrome, unavailable } = chromium();
const test = need('chrome', unavailable, { test: bunTest });
let server, child, cdp, evaluate, dir, call;
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
  child = spawn(chrome, ['--headless=new', DESKTOP_INPUT, '--no-sandbox', '--remote-debugging-pipe', '--disable-background-networking', `--user-data-dir=${dir}`, 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  cdp = new Cdp(child.stdio[3], child.stdio[4]);
  child.on('exit', () => cdp.fail('Chrome closed'));
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  call = (method, params) => cdp.send(method, params, sessionId);
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

test('agent alignments are malformed with or without a view; authored refusals stay journaled', async () => {
  const result = await evaluate(`(async () => {
    const port = document.createElement('div'); port.id = 'alignment-feed';
    port.style.cssText = 'height:200px;width:200px;overflow:auto';
    document.getElementById('exact-root').append(port);
    const own = rt.scope(() => list.vl(port, () => [0], i => i(), (w, i) => {
      const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '50px'; w.append(row);
    }, {est:50, pw:[], ph:[200]}));
    await exact.lists.pending(); await exact.lists.settle();
    const errors = [];
    for (const view of [port.$list.view, null, undefined]) for (const [block, inline] of [['banana', 'nearest'], ['start', 'banana']]) {
      try { exact.lists.into(view, '0', block, inline); errors.push(null); }
      catch (e) { errors.push({message:e.message, malformed:e.malformed}); }
    }
    rt.Hosts.scrollIntoView('alignment-feed', 0, 'banana', 'nearest', 'instant', null, null);
    const authored = exact.lists.intoView().at(-1);
    exact.lists.into(-1, '0', 'start', 'nearest');
    const missing = exact.lists.intoView().at(-1);
    rt.end(own); port.remove(); rt.settled();
    return {errors, authored, missing};
  })()`);
  expect(result.errors).toEqual(Array.from({length:6}, () => ({ message:'no alignment banana: start, center, end or nearest', malformed:true })));
  expect(result.authored).toEqual({list:'alignment-feed', status:'refused: no alignment banana'});
  expect(result.missing).toEqual({list:'#-1', status:'refused: view -1 is not a mounted virtualized list'});
}, 60_000);

test('element jumps use Chrome scroll padding and margins and settle once', async () => {
  const result = await evaluate(`(async () => {
    const root = document.getElementById('exact-root');
    const port = document.createElement('div');
    port.style.cssText = 'width:200px;height:200px;overflow:auto;scroll-padding:24px 0 12px';
    port.innerHTML = '<div style="height:400px"></div><div id="scroll-target" style="height:50px;scroll-margin:8px 0 6px"></div><div style="height:550px"></div>';
    root.append(port);
    let ends = 0;
    rt.onScroll(port, 'scrollend', () => { ends++; }, (kind, f) => port.addEventListener(kind, f));
    const jump = async align => {
      const end = new Promise(resolve => port.addEventListener('scrollend', resolve, {once:true}));
      rt.Hosts.scrollIntoView('scroll-target', align, 'nearest', 'instant');
      await end; return port.scrollTop;
    };
    const nearest = await jump('nearest'), start = await jump('start');
    rt.Hosts.scrollIntoView('scroll-target', 'nearest', 'nearest', 'instant');
    const still = port.scrollTop;
    port.remove();
    return {nearest, start, still, ends};
  })()`);
  expect(result).toEqual({nearest:268, start:368, still:368, ends:2});
}, 60_000);

test('a keyed row jump includes the same scroll margin as Chrome element alignment', async () => {
  const result = await evaluate(`(async () => {
    const root = document.getElementById('exact-root');
    const port = document.createElement('div'); port.id = 'margin-feed';
    port.style.cssText = 'height:200px;width:200px;overflow:auto;scroll-padding:24px 0 12px';
    root.append(port);
    const own = rt.scope(() => list.vl(port, () => Array.from({length:100}, (_, i) => i), i => i(), (w, i) => {
      const row = document.createElement('div'); row.textContent = String(i());
      row.style.cssText = 'height:50px;scroll-margin:8px 0 6px'; w.append(row);
    }, {est:50, pw:[], ph:[200]}));
    await exact.lists.pending(); await exact.lists.settle();
    rt.Hosts.scrollIntoView('margin-feed', 50, 'start', 'nearest', 'instant', null, null);
    await exact.lists.settle();
    const start = port.scrollTop;
    rt.Hosts.scrollIntoView('margin-feed', 50, 'end', 'nearest', 'instant', null, null);
    await exact.lists.settle();
    const end = port.scrollTop;
    rt.end(own); port.remove(); rt.settled();
    return {start, end};
  })()`);
  expect(result).toEqual({start:2468, end:2368});
}, 60_000);

test('nested smooth element jumps align both ports at their destinations', async () => {
  const result = await evaluate(`(async () => {
    const port = document.createElement('div');
    port.style.cssText = 'height:200px;width:200px;overflow:auto';
    port.innerHTML = '<div style="height:300px"></div><div id="inner" style="height:200px;overflow:auto"><div style="height:400px"></div><div id="nested-target" style="height:50px"></div><div style="height:550px"></div></div><div style="height:700px"></div>';
    document.getElementById('exact-root').append(port);
    const inner = port.querySelector('#inner');
    const ends = [port, inner].map(p => new Promise(resolve => p.addEventListener('scrollend', resolve, {once:true})));
    rt.Hosts.scrollIntoView('nested-target', 'start', 'nearest', 'smooth');
    await Promise.all(ends);
    const result = [inner.scrollTop, port.scrollTop]; port.remove(); return result;
  })()`);
  expect(result).toEqual([400, 300]);
}, 60_000);

test('a visible instant replacement stops a smooth element jump', async () => {
  const result = await evaluate(`(async () => {
    const port = document.createElement('div');
    port.style.cssText = 'height:200px;width:200px;overflow:auto;position:relative';
    port.innerHTML = '<div id="visible-target" style="position:absolute;height:1px;width:100px"></div><div style="height:400px"></div><div id="far-target" style="height:50px"></div><div style="height:550px"></div>';
    document.getElementById('exact-root').append(port);
    let ends = 0; port.addEventListener('scrollend', () => ends++);
    const moved = new Promise(resolve => port.addEventListener('scroll', resolve, {once:true}));
    rt.Hosts.scrollIntoView('far-target', 'start', 'nearest', 'smooth'); await moved;
    const at = port.scrollTop;
    port.querySelector('#visible-target').style.top = (at + 50) + 'px';
    rt.Hosts.scrollIntoView('visible-target', 'nearest', 'nearest', 'instant');
    // Observe through a longer independent animation: a no-op replacement
    // cancels motion in Chrome without emitting scrollend.
    const probe = port.cloneNode(true); port.after(probe);
    const end = new Promise(resolve => probe.addEventListener('scrollend', resolve, {once:true}));
    probe.scrollTo({top:800, behavior:'smooth'}); await end; probe.remove();
    const result = {at, after:port.scrollTop, ends}; port.remove(); return result;
  })()`);
  expect(result.at).toBeGreaterThan(0);
  expect(result.at).toBeLessThan(400);
  expect(result.after).toBe(result.at);
  expect(result.ends).toBe(0);
}, 60_000);

test('bordered element scrollports resolve percentage padding inside the border', async () => {
  const result = await evaluate(`(() => {
    const port = document.createElement('div');
    port.style.cssText = 'height:200px;width:200px;overflow:auto;border-top:20px solid black;border-left:10px solid black;scroll-padding:25%';
    port.innerHTML = '<div style="height:400px;width:1000px"></div><div id="border-target" style="height:50px;width:50px;margin-left:400px"></div><div style="height:550px;width:1000px"></div>';
    document.getElementById('exact-root').append(port);
    const width = port.clientWidth, height = port.clientHeight;
    rt.Hosts.scrollIntoView('border-target', 'start', 'nearest', 'instant');
    const start = [port.scrollLeft, port.scrollTop];
    rt.Hosts.scrollIntoView('border-target', 'end', 'end', 'instant');
    const end = [port.scrollLeft, port.scrollTop]; port.remove(); return {start, end, width, height};
  })()`);
  // Percentage padding uses the scrollport, excluding borders and classic
  // scrollbars. Nearest aligns the target's right edge with the padded end.
  const {width, height} = result;
  const right = Math.round(450 - width * 0.75);
  expect(result.start).toEqual([right, Math.round(400 - height * 0.25)]);
  expect(result.end).toEqual([right, Math.round(450 - height * 0.75)]);
}, 60_000);

test('a horizontal wheel leaves a vertical Chrome jump running to one scrollend', async () => {
  const point = await evaluate(`(async () => {
    const port = document.createElement('div');
    port.style.cssText = 'position:fixed;top:0;left:0;height:200px;width:200px;overflow-y:scroll;overflow-x:hidden';
    port.innerHTML = '<div style="height:400px"></div><div id="wheel-target" style="height:50px"></div><div style="height:550px"></div>';
    document.body.append(port);
    globalThis.wheelJump = {port, ends:0};
    port.addEventListener('scrollend', () => wheelJump.ends++);
    const moved = new Promise(resolve => port.addEventListener('scroll', resolve, {once:true}));
    rt.Hosts.scrollIntoView('wheel-target', 'start', 'nearest', 'smooth'); await moved;
    return {x:100, y:100};
  })()`);
  await call('Input.dispatchMouseEvent', {type:'mouseWheel', ...point, deltaX:40, deltaY:0});
  const result = await evaluate(`(async () => {
    const {port} = wheelJump;
    const probe = port.cloneNode(true); probe.style.left = '250px'; document.body.append(probe);
    const end = new Promise(resolve => probe.addEventListener('scrollend', resolve, {once:true}));
    probe.scrollTo({top:800, behavior:'smooth'}); await end;
    const result = {top:port.scrollTop, left:port.scrollLeft, ends:wheelJump.ends};
    port.remove(); probe.remove(); delete globalThis.wheelJump; return result;
  })()`);
  expect(result.top).toBeGreaterThan(0);
  expect(result.top).toBe(400);
  expect(result.left).toBe(0);
  expect(result.ends).toBe(1);
}, 60_000);
