import { beforeAll, afterAll, test as bunTest, expect } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { cdpBrowser, chromium, DESKTOP_INPUT } from '../../scripts/agent-launch.mjs';
import { need } from '../../scripts/coverage.mjs';

const { unavailable } = chromium();
const test = need('chrome', unavailable, { test: bunTest });
let server, suite;
beforeAll(async () => {
  if (unavailable) return;
  server = Bun.serve({port:0, hostname:'127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/names.js') return new Response('export const sourceTypes = {};', {headers:{'content-type':'text/javascript'}});
    const file = existsSync(new URL(`.${path}`, import.meta.url)) ? `.${path}` : `../web${path}`;
    return new Response(path.endsWith('.js') ? readFileSync(new URL(file, import.meta.url)) : '<!doctype html><body><main id="exact-root"></main>', {
      headers:{'content-type':path.endsWith('.js') ? 'text/javascript' : 'text/html'},
    });
  }});
  suite = await cdpBrowser({args:[DESKTOP_INPUT]});
}, 60_000);
afterAll(async () => { await suite?.close(); server?.stop(true); }, 60_000);

for (const grouped of [false, true]) test(`a deferred drop keeps its own result through an unrelated refusal (grouped=${grouped})`, async () => {
  const {sessionId, targetId} = await suite.page();
  const call = (method, params) => suite.cdp.send(method, params, sessionId);
  await call('Page.navigate', {url:`http://127.0.0.1:${server.port}`});
  const evaluated = await call('Runtime.evaluate', {returnByValue:true, awaitPromise:true, expression:`(async () => {
    const rt = await import('/rt.js'), list = await import('/list.js'), {install} = await import('/reorder.js');
    rt.clock.agent = true;
    const grouped = ${grouped};
    const port = document.createElement('div'); port.id = 'arrange';
    if (grouped) port.dataset.reordergroup = 'cards';
    port.style.cssText = 'height:100px;width:320px;overflow:auto';
    const status = document.createElement('div');
    document.getElementById('exact-root').append(port, status);
    const rows = rt.sig(Array.from({length:30}, (_, i) => String(i))), sent = rt.sig(false);
    const reply = Promise.withResolvers();
    let response;
    port.$reorderdrop = rt.act((item, before) => {
      rt.W(sent, true);
      response = reply.promise.then(() => rt.commit(() => {
        const next = rows().filter(k => k !== item);
        next.splice(next.indexOf(before), 0, item);
        rt.W(rows, next);
      }, 'move reply'));
    }, ['s', '?s']);
    const label = text => parent => {
      const el = document.createElement('span'); el.textContent = text;
      el.dataset.sharedElement = 'status'; el.style.setProperty('--exact-layout-transition', '20 0 linear');
      parent.append(el); rt.pr(el);
    };
    const own = rt.scope(() => {
      list.vl(port, rows, i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = i(); row.style.height = '20px';
        row.dataset.reorderfor = 'arrange'; row.dataset.view = rt.viewId(row); w.append(row);
      }, {est:20, pw:[], ph:[100]});
      rt.when(status, sent, label('Waiting for move'), label('Ready'));
    });
    rt.settled(); await rt.pieces(); await exact.lists.pending(); await exact.lists.settle();
    const reorder = install(exact.lists, {}, rt.After);
    await new Promise(resolve => { port.addEventListener('scroll', resolve, {once:true}); port.scrollTop = 200; });
    await exact.lists.settle();
    const row = port.querySelector('[data-listitemkey="s:10"]').firstElementChild;
    row.dispatchEvent(new PointerEvent('pointerdown', {bubbles:true, pointerId:1}));
    await exact.lists.settle();
    const binding = reorder.binding(rt.viewId(row)), geometry = reorder.geometry(binding.list);
    const {token} = grouped ? reorder.beginGroup(binding, geometry, true) : reorder.begin(binding, geometry);
    reorder.preview(token, geometry, 300);
    const gate = Promise.withResolvers(), start = document.startViewTransition.bind(document);
    let transition;
    document.startViewTransition = update => transition = start(async () => { await gate.promise; return update(); });
    const snapshot = () => ({pending:port.$list.preview.dropped, phase:reorder.session()?.phase ?? null});
    const accepted = grouped ? rt.commit(() => reorder.dropGroup(token, geometry, 0)) : reorder.drop(token, geometry);
    const deferred = !!transition, before = snapshot();
    const refused = rt.commit(() => { throw new rt.Refusal('unrelated action'); });
    const during = snapshot();
    gate.resolve();
    if (transition) { await transition.updateCallbackDone; await transition.finished; }
    document.startViewTransition = start;
    const after = snapshot();
    reply.resolve(); await response;
    const offset = port.$list.geometry.offset;
    reorder.reconcileGroup(); reorder.finish(token);
    row.dispatchEvent(new PointerEvent('pointerup', {bubbles:true, pointerId:1}));
    rt.end(own); port.remove(); status.remove(); rt.settled();
    return {accepted, refused, deferred, before, during, after, offset};
  })()`});
  await suite.cdp.send('Target.closeTarget', {targetId});
  if (evaluated.exceptionDetails) throw Error(evaluated.exceptionDetails.exception?.description ?? evaluated.exceptionDetails.text);
  const held = {pending:true, phase:grouped ? 'holding' : null};
  expect(evaluated.result.value).toEqual({accepted:true, refused:false, deferred:true, before:held, during:held, after:held, offset:180});
}, 60_000);
