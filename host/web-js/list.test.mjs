// App-owned virtual-list restoration through the browser runtime and real DOM geometry.
import { beforeAll, afterAll, test as bunTest, expect } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { cdpBrowser, chromium, DESKTOP_INPUT } from '../../scripts/agent-launch.mjs';
import { need } from '../../scripts/coverage.mjs';

const { unavailable } = chromium();
const test = need('chrome', unavailable, { test: bunTest });
let server, suite, cdp, evaluate, call;
beforeAll(async () => {
  if (unavailable) return;
  server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/names.js') return new Response('export const sourceTypes = {};', {headers: {'content-type':'text/javascript'}});
    const file = existsSync(new URL(`.${path}`, import.meta.url)) ? `.${path}` : `../web${path}`;
    return new Response(path.endsWith('.js') ? readFileSync(new URL(file, import.meta.url)) : '<!doctype html><body><main id="exact-root"></main>', {
      headers: { 'content-type': path.endsWith('.js') ? 'text/javascript' : 'text/html' },
    });
  }});
  suite = await cdpBrowser({ args: [DESKTOP_INPUT] });
  ({ cdp } = suite);
  const { sessionId } = await suite.page();
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
  await suite?.close();
  server?.stop(true);
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

test('drop anchoring preserves visible rows and cancellation in the DOM', async () => {
  const {sessionId, targetId} = await suite.page();
  const call = (method, params) => cdp.send(method, params, sessionId);
  const evaluate = async expression => {
    const r = await call('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true});
    if (r.exceptionDetails) throw Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  await call('Page.navigate', {url: `http://127.0.0.1:${server.port}`});
  await evaluate(`Promise.all([import('/rt.js'), import('/list.js')]).then(([rt, list]) => { globalThis.rt = rt; globalThis.list = list; rt.clock.agent = true; })`);
  const result = await evaluate(`(async () => {
    const results = [];
    for (const [offset, item, y, cancel, grouped] of [[200,10,300,false,false], [207,10,300,false,false], [0,0,100,false,false], [500,25,600,false,false], [200,10,300,true,false], [200,10,300,false,true]]) {
      const port = document.createElement('div'); port.id = 'arrange';
      if (grouped) port.dataset.reordergroup = 'cards';
      port.style.cssText = 'height:100px;width:320px;overflow:auto';
      document.getElementById('exact-root').append(port);
      const rows = rt.sig(Array.from({length:30}, (_, i) => String(i)));
      port.$reorderdrop = (item, before) => {
        const next = rows().filter(k => k !== item);
        next.splice(before === null ? next.length : next.indexOf(before), 0, item);
        rt.W(rows, next);
      };
      const own = rt.scope(() => list.vl(port, rows, i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = i();
        row.dataset.reorderfor = 'arrange'; row.dataset.view = rt.viewId(row); row.style.height = '20px'; w.append(row);
      }, {est:20, pw:[], ph:[100], follow:() => true}));
      rt.settled();
      await exact.lists.pending(); await exact.lists.settle();
      const { install } = await import('/reorder.js');
      const reorder = install(exact.lists, {}, rt.After);
      if (offset) await new Promise(resolve => { port.addEventListener('scroll', resolve, {once:true}); port.scrollTop = offset; });
      await exact.lists.settle();
      const row = port.querySelector('[data-listitemkey="s:' + item + '"]').firstElementChild;
      row.dispatchEvent(new PointerEvent('pointerdown', {bubbles:true, pointerId:1}));
      await exact.lists.settle();
      const binding = reorder.binding(rt.viewId(row)), geometry = reorder.geometry(binding.list);
      const start = grouped ? reorder.beginGroup(binding, geometry, false) : reorder.begin(binding, geometry);
      if (!start) throw Error(JSON.stringify({binding, geometry, pin:port.$list.geometry.interaction_view, preview:port.$list.preview}));
      const token = start.token;
      const preview = reorder.preview(token, geometry, y);
      let dropped;
      rt.commit(() => {
        if (cancel) { reorder.cancel(token); port.$reorderdrop(String(item), '15'); dropped = false; }
        else dropped = grouped ? reorder.dropGroup(token, geometry, 0) : reorder.drop(token, geometry);
      });
      const corrected = port.$list.geometry.offset, moved = rows().indexOf(String(item));
      reorder.finish(token);
      row.dispatchEvent(new PointerEvent('pointerup', {bubbles:true, pointerId:1}));
      await exact.lists.settle();
      const successor = port.querySelector('[data-listitemkey="s:' + (item + 1) + '"]');
      const top = successor.getBoundingClientRect().top - port.getBoundingClientRect().top;
      const scrollTop = port.scrollTop;
      rt.end(own); port.remove(); rt.settled();
      results.push({preview, dropped, moved, offset:corrected, top, scrollTop});
    }
    return results;
  })()`);
  await cdp.send('Target.closeTarget', {targetId});
  expect(result).toEqual([
    {preview:'accepted', dropped:true, moved:14, offset:180, top:20, scrollTop:180},
    {preview:'accepted', dropped:true, moved:14, offset:187, top:13, scrollTop:187},
    {preview:'accepted', dropped:true, moved:4, offset:0, top:0, scrollTop:0},
    {preview:'accepted', dropped:true, moved:29, offset:480, top:20, scrollTop:480},
    {preview:'accepted', dropped:false, moved:14, offset:280, top:-80, scrollTop:280},
    {preview:'accepted', dropped:true, moved:14, offset:180, top:20, scrollTop:180},
  ]);
}, 60_000);

test('relative corrections and drop eligibility survive later commits', async () => {
  const {sessionId, targetId} = await suite.page();
  const call = (method, params) => cdp.send(method, params, sessionId);
  const evaluate = async expression => {
    const r = await call('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true});
    if (r.exceptionDetails) throw Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  await call('Page.navigate', {url: `http://127.0.0.1:${server.port}`});
  const results = await evaluate(`(async () => {
    const rt = await import('/rt.js'), list = await import('/list.js'), {install} = await import('/reorder.js');
    rt.clock.agent = true;
    const results = [];
    for (const mode of ['trim', 'same-slot', 'same-slot-group', 'refuse', 'refuse-cancel', 'refuse-write', 'refuse-group', 'refuse-write-group', 'pending-cancel', 'pending', 'pending-group', 'landed-group']) {
      const port = document.createElement('div'); port.id = 'arrange';
      const grouped = mode.endsWith('-group');
      if (grouped) port.dataset.reordergroup = 'cards';
      port.style.cssText = 'height:100px;width:320px;overflow:auto';
      document.getElementById('exact-root').append(port);
      const rows = rt.sig(Array.from({length:30}, (_, i) => String(i)));
      const move = (item, before) => {
        const next = rows().filter(k => k !== item);
        next.splice(before === null ? next.length : next.indexOf(before), 0, item);
        rt.W(rows, next);
      };
      const invalid = rt.sig(0, 'n');
      port.$reorderdrop = (item, before) => {
        if (mode.startsWith('refuse-write')) { rt.W(invalid, 'wrong'); return; }
        if (mode.startsWith('refuse')) throw new rt.Refusal('refused drop');
        if (!mode.startsWith('pending')) move(item, before);
      };
      const own = rt.scope(() => list.vl(port, rows, i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = i();
        row.dataset.reorderfor = 'arrange'; row.dataset.view = rt.viewId(row); row.style.height = '20px'; w.append(row);
      }, {est:20, pw:[], ph:[100]}));
      rt.settled(); await exact.lists.pending(); await exact.lists.settle();
      const reorder = install(exact.lists, {}, rt.After);
      await new Promise(resolve => { port.addEventListener('scroll', resolve, {once:true}); port.scrollTop = 200; });
      await exact.lists.settle();
      const c = port.$list;
      if (mode === 'trim') {
        const ix = new c.index.constructor(20); ix.replace(['s:a', 's:b', 's:c']); ix.leading = 20;
        const boundaries = [-21, -20, 0, 40, 41].map(offset => {
          const a = ix.anchorAt('s:a', offset);
          return [ix.restoreAnchor(a, 20), ix.keptRow(a, 20)];
        });
        results.push({mode:'boundaries', values:boundaries});
        port.scrollTop = 210; // Ten pixels of fling travel after the last geometry report.
        rt.commit(() => rt.W(rows, rows().slice(10)));
        const correction = {offset:c.geometry.offset, from:c.correction?.from};
        await exact.lists.settle();
        results.push({mode, ...correction, scroll:port.scrollTop});
      } else {
        const row = port.querySelector('[data-listitemkey="s:10"]').firstElementChild;
        row.dispatchEvent(new PointerEvent('pointerdown', {bubbles:true, pointerId:1}));
        await exact.lists.settle();
        const binding = reorder.binding(rt.viewId(row)), geometry = reorder.geometry(binding.list);
        const {token} = grouped ? reorder.beginGroup(binding, geometry, false) : reorder.begin(binding, geometry);
        reorder.preview(token, geometry, mode.startsWith('same-slot') ? 200 : 300);
        const accepted = rt.commit(() => grouped ? reorder.dropGroup(token, geometry, 0) : reorder.drop(token, geometry));
        if (grouped && mode.startsWith('refuse')) results.push({mode:mode + '-hold', phase:reorder.session().phase, ending:reorder.session().ending});
        if (mode === 'refuse-cancel' || mode === 'pending-cancel') reorder.cancel(token);
        if (mode === 'landed-group') {
          results.push({mode, offset:c.geometry.offset, pending:!!c.dropAnchor()});
        } else {
          rt.commit(() => move('10', mode.startsWith('refuse') || mode === 'pending-cancel' ? '15' : '12'));
          const first = c.geometry.offset;
          let landed;
          if (mode === 'pending') {
            await exact.lists.settle();
            rt.commit(() => move('10', '15'));
            landed = c.geometry.offset;
          }
          results.push({mode, accepted, first, ...(landed === undefined ? {} : {landed})});
        }
        if (grouped) reorder.reconcileGroup();
        reorder.finish(token);
        row.dispatchEvent(new PointerEvent('pointerup', {bubbles:true, pointerId:1}));
      }
      rt.end(own); port.remove(); rt.settled();
    }
    return results;
  })()`);
  await cdp.send('Target.closeTarget', {targetId});
  expect(results).toEqual([
    {mode:'boundaries', values:[[-20,false], [-20,true], [0,true], [40,true], [40,false]]},
    {mode:'trim', offset:0, from:200, scroll:10},
    {mode:'same-slot', accepted:true, first:220},
    {mode:'same-slot-group', accepted:true, first:220},
    {mode:'refuse', accepted:false, first:280},
    {mode:'refuse-cancel', accepted:false, first:280},
    {mode:'refuse-write', accepted:false, first:280},
    {mode:'refuse-group-hold', phase:'settling', ending:null},
    {mode:'refuse-group', accepted:false, first:280},
    {mode:'refuse-write-group-hold', phase:'settling', ending:null},
    {mode:'refuse-write-group', accepted:false, first:280},
    {mode:'pending-cancel', accepted:true, first:280},
    {mode:'pending', accepted:true, first:220, landed:200},
    {mode:'pending-group', accepted:true, first:220},
    {mode:'landed-group', offset:180, pending:false},
  ]);
}, 60_000);

// Rows of 100.3 px: a keyed jump to row 18 asks for 1805.4, which the
// browser rounds to the device pixel at 1805. The 0.4 px of row 17 above
// the port's start is painted nowhere (edges snap to device pixels), so the
// first visible row is 18, as the reader sees it.
test('a jump reports the row it put at the start, not a sliver above it', async () => {
  const result = await evaluate(`(async () => {
    const port = document.createElement('div'); port.id = 'sliver-feed';
    port.style.cssText = 'height:300px;width:200px;overflow:auto';
    document.getElementById('exact-root').append(port);
    let heard;
    rt.onScroll(port, 'scroll', (x, y, e) => { heard = e; }, (kind, f) => port.addEventListener(kind, f));
    const own = rt.scope(() => list.vl(port, () => Array.from({length:100}, (_, i) => i), i => i(), (w, i) => {
      const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '100.3px'; w.append(row);
    }, {est:100.3, pw:[], ph:[300]}));
    await exact.lists.pending(); await exact.lists.settle();
    rt.Hosts.scrollIntoView('sliver-feed', 18, 'start', 'nearest', 'instant', null, null);
    await exact.lists.settle();
    port.dispatchEvent(new Event('scroll'));
    const top = port.scrollTop, row = port.querySelector('[data-listitemkey="n:18"]');
    const drawn = row.getBoundingClientRect().top - port.getBoundingClientRect().top;
    rt.end(own); port.remove(); rt.settled();
    return {top, drawn, key:heard[6], offset:heard[7]};
  })()`);
  expect(result.top).toBe(1805);
  expect(result.key).toBe('18');
  expect(result.offset).toBe(0);
}, 60_000);

// Rows 150 px tall under an estimate of 100: lists opened at their ends
// are first placed at the ends their estimates give; each one's rows are
// measured, and the port moved to their end, before the next frame, whose
// scroll events therefore hear the ends (collection-glue.js `commit`).
// Three open at once, as one budget a frame would not reach them all.
test('lists opened at their ends report only their ends while their rows measure', async () => {
  const result = await evaluate(`(async () => {
    const lists = Array.from({length:3}, () => {
      const port = document.createElement('div');
      port.style.cssText = 'height:300px;width:200px;overflow:auto';
      document.getElementById('exact-root').append(port);
      const heard = [];
      rt.onScroll(port, 'scroll', (x, y, e) => { heard.push(Math.round(e[3] - e[1] - e[5])); }, (kind, f) => port.addEventListener(kind, f));
      const own = rt.scope(() => list.vl(port, () => Array.from({length:100}, (_, i) => i), i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '150px'; w.append(row);
      }, {est:100, pw:[], ph:[300], atEnd:true}));
      return {port, heard, own};
    });
    rt.settled();
    for (let f = 0; f < 10; f++) await new Promise(r => requestAnimationFrame(() => setTimeout(r, 0)));
    const out = lists.map(({port, heard}) => ({heard, gap: port.scrollHeight - port.scrollTop - port.clientHeight}));
    for (const {port, own} of lists) { rt.end(own); port.remove(); }
    rt.settled();
    return out;
  })()`);
  for (const {heard, gap} of result) {
    expect(gap).toBe(0);
    expect(heard.length).toBeGreaterThan(0);
    expect(heard).toEqual(heard.map(() => 0));
  }
}, 60_000);

test('measured virtual rows count construction and retirement without eager realization', async () => {
  const result = await evaluate(`(async () => {
    const perf = await import('/perf.js'), root = document.getElementById('exact-root');
    perf.install(root);
    const el = document.createElement('div'); el.dataset.site = '93';
    el.style.cssText = 'height:320px;width:400px;overflow:auto'; root.append(el);
    const own = rt.scope(() => list.vl(el, () => Array.from({length:3000}, (_, i) => i), i => i(), (w, i) => {
      const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '32px'; w.append(row);
    }, {est:32,pw:[],ph:[320]}));
    await exact.lists.pending(); await exact.lists.settle();
    const before = perf.reply(root, {}).sites.find(s => s.site === 93);
    exact.lists.into(el.$list.view, 500);
    await exact.lists.settle();
    const moved = perf.reply(root, {}).sites.find(s => s.site === 93);
    rt.end(own); el.remove(); rt.settled();
    const after = perf.reply(root, {}).sites.find(s => s.site === 93);
    return {before,moved,after};
  })()`);
  expect(result.before.rowConstructed).toBeGreaterThan(0);
  expect(result.before.rowConstructed).toBeLessThan(100);
  expect(result.moved.rowRetired).toBeGreaterThan(0);
  expect(result.after.rowRetired).toBe(result.after.rowConstructed);
}, 60_000);

// @ref LLP 1145 D4 — `reach-margin` sets how far ahead of the last row `reachend` fires, as the runner's reach.rs.
test('reach-margin moves reachend earlier or later; the default is one port', async () => {
  const result = await evaluate(`(async () => {
    const lists = [undefined, '300%', '0PX 960px', '0px'].map(reach => {
      const el = document.createElement('div');
      el.style.cssText = 'height:320px;width:400px;overflow:auto';
      document.getElementById('exact-root').append(el);
      const l = { el, ends: 0, at: null };
      l.own = rt.scope(() => list.vl(el, () => Array.from({length:200}, (_, i) => i), i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '32px'; w.append(row);
      }, {est:32, pw:[], ph:[320], end: () => { l.ends++; }, reach}));
      return l;
    });
    rt.settled();
    await exact.lists.pending(); await exact.lists.settle();
    // The last row starts at 6368; the port ends 320 past its offset.
    for (const top of [1000, 5128, 5768, 6080]) {
      await Promise.all(lists.map(({el}) => new Promise(resolve => { el.addEventListener('scroll', resolve, {once:true}); el.scrollTop = top; })));
      await exact.lists.settle();
      for (let f = 0; f < 3; f++) await new Promise(r => requestAnimationFrame(() => setTimeout(r, 0)));
      for (const l of lists) if (l.ends && l.at === null) l.at = top;
    }
    for (const {el, own} of lists) { rt.end(own); el.remove(); }
    rt.settled();
    return lists.map(l => l.at);
  })()`);
  expect(result).toEqual([5768, 5128, 5128, 6080]);
}, 60_000);

// @ref LLP 1027.004 D3 — the edge band leads a fling as the window does (reach.rs `Margin::band`). The
// agent's reports carry no velocity, so these are made here: one at rest, then one moving at -4,000 px/s.
test('a fling toward the start fires reachstart as far ahead as the window leads', async () => {
  const result = await evaluate(`(async () => {
    const lists = [undefined, '0px', '200%'].map(reach => {
      const el = document.createElement('div');
      el.style.cssText = 'height:320px;width:400px;overflow:auto';
      document.getElementById('exact-root').append(el);
      const l = { el, starts: 0 };
      l.own = rt.scope(() => list.vl(el, () => Array.from({length:200}, (_, i) => i), i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '32px'; w.append(row);
      }, {est:32, pw:[], ph:[320], start: () => { l.starts++; }, reach}));
      l.c = [...exact.lists.internals.Lists.values()].find(c => c.el === el);
      return l;
    });
    rt.settled();
    await exact.lists.pending(); await exact.lists.settle();
    await Promise.all(lists.map(({el}) => new Promise(resolve => { el.addEventListener('scroll', resolve, {once:true}); el.scrollTop = 1000; })));
    await exact.lists.settle();
    // Row 0 ends at 32; the band before a port at rest is the margin, and
    // moving toward the start a quarter second of travel more, up to two ports.
    const report = (c, offset, velocity) => {
      const f = { ...c.geometry, revision: c.revision, scroll_sequence: c.geometry.scroll_sequence + 1, offset, measurements: [] };
      const byView = c.prepare(f);
      return byView ? c.feedback(f, byView, { velocity, limit: null })[1]?.first ?? null : 'refused';
    };
    const out = lists.map(({c}) => [
      [960, 0], [960, -4000], [991, -4000], [992, -4000], [640, 0], [640, -4000], [671, 0], [1311, -4000], [1312, -4000],
    ].map(([offset, velocity]) => { const edge = report(c, offset, velocity); if (edge === 0) c.edgeArmed[0] = true; return edge; }));
    for (const {el, own} of lists) { rt.end(own); el.remove(); }
    rt.settled();
    return out;
  })()`);
  const N = null;
  expect(result).toEqual([
    [N, 0, 0, N, N, 0, N, N, N],
    [N, N, N, N, N, 0, N, N, N],
    [N, 0, 0, 0, 0, 0, 0, 0, N],
  ]);
}, 60_000);

// reach.rs `geometric_edges`: a page smaller than a fling's band re-arms its edge by the band at rest.
test('a page that lands within a fling\'s band re-arms reachstart', async () => {
  const result = await evaluate(`(async () => {
    const el = document.createElement('div');
    el.style.cssText = 'height:320px;width:400px;overflow:auto';
    document.getElementById('exact-root').append(el);
    let starts = 0;
    const rows = rt.sig(Array.from({length:200}, (_, i) => i));
    const own = rt.scope(() => list.vl(el, rows, i => i(), (w, i) => {
      const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '32px'; w.append(row);
    }, {est:32, pw:[], ph:[320], start: () => { starts++; }}));
    rt.settled();
    await exact.lists.pending(); await exact.lists.settle();
    await new Promise(resolve => { el.addEventListener('scroll', resolve, {once:true}); el.scrollTop = 1000; });
    await exact.lists.settle();
    const c = [...exact.lists.internals.Lists.values()].find(c => c.el === el);
    const report = (offset, velocity) => {
      const f = { ...c.geometry, revision: c.revision, scroll_sequence: c.geometry.scroll_sequence + 1, offset,
        measurements: c.mounted.map(m => ({ view: m.view, epoch: m.epoch, size: 32 })) };
      const byView = c.prepare(f);
      if (!byView) return 'refused';
      const edge = c.feedback(f, byView, { velocity, limit: null })[1];
      if (edge?.first === 0) starts++;
      return edge?.first ?? null;
    };
    const out = [report(960, -4000), report(900, -4000)];
    // A message at the end re-arms nothing: the first row is the one it fired at.
    rt.commit(() => rt.W(rows, [...rows(), ...Array.from({length:20}, (_, i) => 200 + i)]));
    rt.settled();
    out.push(report(900, -4000), report(900, -4000), report(100, -4000));
    rt.commit(() => rt.W(rows, [...Array.from({length:20}, (_, i) => i - 20), ...rows()]));
    rt.settled();
    out.push(c.correction?.offset, report(740, -4000));
    rt.end(own); el.remove(); rt.settled();
    return out;
  })()`);
  expect(result).toEqual([0, null, null, null, null, 740, 0]);
}, 60_000);

// @ref LLP 1010 §6.11 — `keep-mounted` keeps its rows mounted wherever the window is, as the runner's keep.rs.
test('keep-mounted keeps the first, last and playing rows mounted as the window moves', async () => {
  const result = await evaluate(`(async () => {
    const el = document.createElement('div');
    el.style.cssText = 'height:320px;width:400px;overflow:auto';
    document.getElementById('exact-root').append(el);
    const rows = rt.sig(Array.from({length:200}, (_, i) => i)), playing = rt.sig(100);
    const own = rt.scope(() => list.vl(el, rows, i => i(), (w, i) => {
      const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '32px'; w.append(row);
    }, {est:32, pw:[], ph:[320], keep: () => [0, rows().length - 1, playing(), null]}));
    rt.settled();
    await exact.lists.pending(); await exact.lists.settle();
    const c = [...exact.lists.internals.Lists.values()].find(c => c.el === el);
    const mounted = () => c.mounted.map(m => m.position).filter(p => p < 20 || p > 60).join(' ');
    const out = [mounted()];
    await new Promise(resolve => { el.addEventListener('scroll', resolve, {once:true}); el.scrollTop = 6080; });
    await exact.lists.settle();
    out.push(mounted());
    rt.commit(() => rt.W(playing, 150));
    rt.settled();
    out.push(mounted());
    // The kept last key and the data change in one commit: one pass, one revision.
    const before = c.revision;
    rt.commit(() => rt.W(rows, [...rows(), ...Array.from({length:20}, (_, i) => 200 + i)]));
    rt.settled();
    out.push(c.mounted.some(m => m.position === 219), c.mounted.some(m => m.position === 199) === c.mounted.some(m => m.position === 198), c.revision - before);
    rt.end(own); el.remove(); rt.settled();
    return out;
  })()`);
  const [start, end, played, ...grown] = result;
  // At the start, the window's first rows, the playing row and the last row.
  expect(start.split(' ').map(Number)).toEqual(expect.arrayContaining([0, 100, 199]));
  // At the end: the first row and the playing one are still there; the window's rows near the start went.
  expect(end.split(' ').map(Number)).toEqual(expect.arrayContaining([0, 100]));
  expect(end.split(' ').map(Number)).not.toContain(10);
  // A new playing row mounts and the old one goes.
  expect(played.split(' ').map(Number)).toEqual(expect.arrayContaining([0, 150]));
  expect(played.split(' ').map(Number)).not.toContain(100);
  // The new last row is kept; 199 is now an ordinary row of the window.
  expect(grown).toEqual([true, true, 1]);
}, 60_000);

// @ref LLP 1010 §6.11 — a kept row before a jump's target does not hold the jump open (into_view.rs `settle_target`).
test('a jump past a kept row settles', async () => {
  const result = await evaluate(`(async () => {
    const out = [];
    for (const keep of [undefined, () => [0]]) {
      const el = document.createElement('div'); el.id = 'kept-jump';
      el.style.cssText = 'height:320px;width:400px;overflow:auto';
      document.getElementById('exact-root').append(el);
      const own = rt.scope(() => list.vl(el, () => Array.from({length:200}, (_, i) => i), i => i(), (w, i) => {
        const row = document.createElement('div'); row.textContent = String(i()); row.style.height = '32px'; w.append(row);
      }, {est:32, pw:[], ph:[320], keep}));
      rt.settled();
      await exact.lists.pending(); await exact.lists.settle();
      rt.Hosts.scrollIntoView('kept-jump', 150, 'start', 'nearest', 'instant', null, null);
      for (let f = 0; f < 4; f++) { await exact.lists.settle(); await new Promise(r => requestAnimationFrame(() => setTimeout(r, 0))); }
      const c = [...exact.lists.internals.Lists.values()].find(c => c.el === el);
      out.push([exact.lists.intoView().find(e => e.list === c.view)?.status, el.scrollTop]);
      rt.end(own); el.remove(); rt.settled();
    }
    return out;
  })()`);
  expect(result).toEqual([['done', 4800], ['done', 4800]]);
}, 60_000);
