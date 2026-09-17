// Real DOM geometry and deterministic feedback admission; no app data/network.
import { beforeAll, afterAll, test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp } from '../../scripts/agent.mjs';
import { collectionBytes } from './navigation.js';

let server, child, cdp, evaluate, dir;
beforeAll(async () => {
  const chrome = process.env.CHROME ?? (process.platform === 'darwin' ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : '/usr/bin/chromium');
  if (!existsSync(chrome)) throw Error('Set CHROME to a Chromium executable');
  dir = mkdtempSync(join(tmpdir(), 'exact-collection-'));
  server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    return new Response(new URL(req.url).pathname === '/navigation.js' ? readFileSync(new URL('./navigation.js', import.meta.url)) : '<!doctype html><body><main id="root"></main>', {
      headers: { 'content-type': new URL(req.url).pathname.endsWith('.js') ? 'text/javascript' : 'text/html' },
    });
  }});
  child = spawn(chrome, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--disable-background-networking', `--user-data-dir=${dir}`, 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  cdp = new Cdp(child.stdio[3], child.stdio[4]);
  child.on('exit', () => cdp.fail('Chrome closed'));
  const { targetInfos } = await cdp.send('Target.getTargets');
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: targetInfos.find(t => t.type === 'page').targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  evaluate = async expression => {
    const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  };
  await call('Page.navigate', { url: `http://127.0.0.1:${server.port}` });
  await evaluate(`import('/navigation.js').then(module => { globalThis.createController = module.collectionController; })`);
  await evaluate(`(${setup})()`);
});
afterAll(async () => {
  if (child && child.exitCode === null) { const exit = new Promise(r => child.once('exit', r)); child.kill(); await exit; }
  server?.stop(true);
  if (dir) rmSync(dir, { recursive: true, force: true });
});
function setup() {
  globalThis.fixture = (options = {}) => {
    globalThis.f?.controller.dispose();
    const root = document.getElementById('root');
    root.innerHTML = `<div id="port" style="height:180px;width:320px;overflow:auto;border:3px solid;padding:0"><div style="height:50px"></div><div data-view="1" style="padding:10px 12px;overflow-anchor:auto"><div data-view="2" style="height:40px;display:flow-root"><input data-view="3"></div><div data-view="4" style="height:60px;display:flow-root"><button data-view="5">row</button></div><div style="height:1800px"></div></div></div>`;
    const views = new Map([...root.querySelectorAll('[data-view]')].map(el => [+el.dataset.view, el]));
    const frames = new Map(), reports = [];
    let serial = 0;
    const controller = createController({ root, views, report(bytes) {
      const d = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      const read = { view: d.getUint32(4, true), revision: String(d.getBigUint64(8, true)), sequence: String(d.getBigUint64(16, true)), top: d.getFloat64(24, true), width: d.getFloat64(32, true), height: d.getFloat64(40, true), rowWidth: d.getFloat64(48, true), focus: d.getUint32(56, true), interaction: d.getUint32(60, true), rows: [] };
      for (let n = 0; n < d.getUint32(64, true); n++) read.rows.push({ view: d.getUint32(68 + n * 20, true), epoch: String(d.getBigUint64(72 + n * 20, true)), height: d.getFloat64(80 + n * 20, true) });
      reports.push(read); return globalThis.f.onReport?.(read);
    }, requestFrame(fn) { frames.set(++serial, fn); return serial; }, cancelFrame(id) { frames.delete(id); }, ...options });
    const snapshot = (revision = '1', extra = {}) => ({ view: 1, revision, scrollSequence: '0', totalExtent: 1900, count: 100, rows: [{ view: 2, root: 3, index: 0, top: 0, height: 40, epoch: '9007199254740993' }, { view: 4, root: 5, index: 99, top: 1840, height: 60, epoch: '2' }], correction: null, ...extra });
    const flush = () => { const pending = [...frames.values()]; frames.clear(); for (const fn of pending) fn(); };
    globalThis.f = { root, views, controller, snapshot, reports, frames, flush, port: document.getElementById('port') };
    return f;
  };
}

test('LE feedback preserves u64 identity and rejects invalid geometry before admission', () => {
  const feedback = { view: 1, revision: '18446744073709551615', scroll_sequence: '9007199254740993', scroll_top: 25, port_width: 320, port_height: 180, row_width: 296, focus_view: null, interaction_view: 9, measurements: [{ view: 2, epoch: '9007199254740995', height: 40.5 }] };
  const bytes = collectionBytes(feedback), view = new DataView(bytes.buffer);
  expect(bytes.length).toBe(88);
  expect(view.getBigUint64(8, true)).toBe(18446744073709551615n);
  expect(view.getBigUint64(72, true)).toBe(9007199254740995n);
  expect(view.getFloat64(80, true)).toBe(40.5);
  expect(() => collectionBytes({ ...feedback, port_width: NaN })).toThrow();
  expect(() => collectionBytes({ ...feedback, revision: Number.MAX_SAFE_INTEGER + 1 })).toThrow();
  expect(() => collectionBytes({ ...feedback, measurements: [feedback.measurements[0], feedback.measurements[0]] })).toThrow();
});
test('actual nested scrollport, content padding, mounted rows, and coalesced scroll without authored handler', async () => {
  const result = await evaluate(`(() => { const f=fixture(); f.controller.commit([f.snapshot()]); f.port.scrollTop=110; for(let i=0;i<100;i++) f.port.dispatchEvent(new Event('scroll')); const queued=f.frames.size; f.flush(); return {queued,last:f.reports.at(-1),inner:f.port.clientWidth,anchor:getComputedStyle(f.views.get(1)).overflowAnchor}; })()`);
  expect(result.queued).toBe(1);
  expect(result.last.top).toBe(50);
  expect(result.last.height).toBe(180);
  expect(result.last.width).toBe(result.inner);
  expect(result.last.rowWidth).toBe(result.inner - 24);
  expect(result.last.rows.map(row => row.height)).toEqual([40, 60]);
  expect(result.last.rows[0].epoch).toBe('9007199254740993');
  expect(result.anchor).toBe('none');
});
test('correction consumes once, never overwrites a newer DOM scroll even before its event', async () => {
  const result = await evaluate(`(() => { const f=fixture(); f.controller.commit([f.snapshot()]); f.port.scrollTop=160; f.port.dispatchEvent(new Event('scroll')); f.flush(); const seq=f.reports.at(-1).sequence; f.controller.commit([f.snapshot('2',{correction:{scrollSequence:seq,scrollTop:140}})]); const corrected=f.port.scrollTop; f.port.scrollTop=260; f.controller.commit([f.snapshot('3',{correction:{scrollSequence:seq,scrollTop:180}})]); const newer=f.port.scrollTop; f.controller.commit([f.snapshot('2',{correction:{scrollSequence:seq,scrollTop:0}})]); return {corrected,newer,old:f.port.scrollTop}; })()`);
  expect(result).toEqual({ corrected: 200, newer: 260, old: 260 });
});
test('focus and one active interaction pin use live descendants, unmount restores policy and cancels work', async () => {
  const result = await evaluate(`(() => { const f=fixture(); f.controller.commit([f.snapshot()]); f.views.get(3).focus({preventScroll:true}); f.views.get(5).dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:7})); f.flush(); const pins=f.reports.at(-1); f.root.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:7})); f.flush(); const released=f.reports.at(-1).interaction; f.controller.commit([]); f.views.get(1).dispatchEvent(new Event('scroll')); f.flush(); return {focus:pins.focus,interaction:pins.interaction,rows:pins.rows.length,released,anchor:f.views.get(1).style.overflowAnchor,pending:f.frames.size}; })()`);
  expect(result).toEqual({ focus: 3, interaction: 5, rows: 2, released: 0, anchor: 'auto', pending: 0 });
});
test('rejected duplicate facts and feedback-caused revisions cannot spin an unbounded frame loop', async () => {
  const result = await evaluate(`(() => { const f=fixture(); let revision=1; f.onReport=()=>{ f.views.get(2).style.height=(40+revision)+'px'; f.controller.commit([f.snapshot(String(++revision))]); }; f.controller.commit([f.snapshot()]); for(let n=0;n<20;n++) f.flush(); return {reports:f.reports.length,pending:f.frames.size}; })()`);
  expect(result.reports).toBe(2);
  expect(result.pending).toBe(0);
});
test('ResizeObserver updates real row heights and width, then becomes idle', async () => {
  const result = await evaluate(`(async () => { const f=fixture(); f.controller.commit([f.snapshot()]); f.flush(); await new Promise(r=>setTimeout(r,50)); f.flush(); f.views.get(2).style.height='97px'; f.port.style.width='400px'; await new Promise(r=>setTimeout(r,50)); f.flush(); const last=f.reports.at(-1), n=f.reports.length; await new Promise(r=>setTimeout(r,50)); f.flush(); return {last,inner:f.port.clientWidth,idle:f.reports.length===n}; })()`);
  expect(result.last.rowWidth).toBe(result.inner - 24);
  expect(result.last.rows[0].height).toBe(97);
  expect(result.idle).toBe(true);
});
test('a partial DOM never reports guessed geometry and resumes on a valid commit', async () => {
  const result = await evaluate(`(() => { const f=fixture(), row=f.views.get(2); f.views.delete(2); f.controller.commit([f.snapshot()]); f.views.get(3).focus({preventScroll:true}); f.flush(); const partial=f.reports.length; f.views.set(2,row); f.controller.commit([f.snapshot()]); f.flush(); return {partial,valid:f.reports.length}; })()`);
  expect(result).toEqual({ partial: 0, valid: 1 });
});

// Two nested collection owners plus their containing outer row. The accepted
// map models the runner retaining each collection's last pins until feedback.
function nestedFixture() {
  const f = fixture();
  f.root.innerHTML = `<div data-view="1" style="height:200px;overflow:auto"><div data-view="2"><div data-view="10" style="height:80px;overflow:auto"><div data-view="11"><input data-view="12"></div></div><div data-view="20" style="height:80px;overflow:auto"><div data-view="21"><input data-view="22"></div></div></div></div>`;
  f.views.clear();
  for (const el of f.root.querySelectorAll('[data-view]')) f.views.set(+el.dataset.view, el);
  f.snapshots = [ [1,2], [10,11], [20,21] ].map(([view,row]) =>
    f.snapshot('1', {view, rows:[{view:row, root:row+1, index:0, top:0, height:40, epoch:'1'}]}));
  f.accepted = new Map(); f.peak = 0; f.reject = false;
  f.onReport = r => {
    if (f.reject && r.view === 20) return false;
    f.accepted.set(r.view, r);
    f.peak = Math.max(f.peak, [...f.accepted.values()].reduce((n,r)=>n+!!r.focus+!!r.interaction,0));
    return true;
  };
  f.contact = (focus, interaction) => {
    f.views.get(focus).focus({preventScroll:true});
    f.views.get(interaction).dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:9}));
  };
  f.settle = () => { for(let i=0;i<8;i++) f.flush(); };
  f.controller.commit(f.snapshots);
  return f;
}
test('nested descendants pin only their nearest collection', async () => {
  const result = await evaluate(`(() => { const f=(${nestedFixture})(); f.contact(12,22); f.settle(); return {pins:[...f.accepted.values()].map(r=>[r.view,r.focus,r.interaction]),peak:f.peak}; })()`);
  expect(result.pins).toEqual([[1,0,0],[10,12,0],[20,0,22]]);
  expect(result.peak).toBe(2);
});
test('swapping collection owners retires both old pins before acquiring replacements', async () => {
  const result = await evaluate(`(() => { const f=(${nestedFixture})(); f.contact(22,12); f.settle(); f.contact(12,22); f.settle(); return {pins:[...f.accepted.values()].map(r=>[r.view,r.focus,r.interaction]),peak:f.peak,pending:f.frames.size}; })()`);
  expect(result.pins).toEqual([[1,0,0],[10,12,0],[20,0,22]]);
  expect(result.peak).toBe(2);
  expect(result.pending).toBe(0);
});
test('hidden old owner releases using prior geometry; rejected release never grants another pin', async () => {
  const result = await evaluate(`(() => { const f=(${nestedFixture})(); f.contact(22,22); f.settle(); f.views.get(20).style.display='none'; f.reject=true; f.contact(12,12); f.settle(); const blocked=[...f.accepted.values()].find(r=>r.view===10); const idle=f.frames.size; f.reject=false; f.controller.commit(f.snapshots); f.settle(); return {blocked:[blocked.focus,blocked.interaction],pins:[...f.accepted.values()].map(r=>[r.view,r.focus,r.interaction]),peak:f.peak,idle}; })()`);
  expect(result.blocked).toEqual([0,0]);
  expect(result.pins).toEqual([[1,0,0],[10,12,12],[20,0,0]]);
  expect(result.peak).toBe(2);
  expect(result.idle).toBe(0);
});

test('destroy cancels owned WAAPI springs, remount and reset retain no prior animation', async () => {
  const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
  const animate = source.slice(source.indexOf('case "animate":'), source.indexOf('case "surface":'));
  const destroy = source.slice(source.indexOf('case "destroy":'), source.indexOf('case "roots":'));
  const reset = source.slice(source.indexOf('for (const a of animations.values()) a.cancel();'), source.indexOf('animations.clear();') + 'animations.clear();'.length);
  const result = await evaluate(`(async () => {
    const f=fixture(), views=f.views, animations=new Map(), retiredViews=new WeakSet(), messageFrames=new Set();
    const viewFor=(_,id)=>views.get(id), followScroll=()=>{};
    globalThis.exact={gpu:{destroy(){}}};
    const apply=op=>{switch(op.op){${animate}${destroy}}};
    const start=id=>apply({op:'animate',id,property:'scale',values:[1,1.5],delay:0,duration:60000});
    let released=true, peak=0;
    start(5);
    const survivor=animations.get('5/scale');
    for(let i=0;i<16;i++) {
      start(3); const old=animations.get('3/scale');
      apply({op:'destroy',id:3});
      released &&= old.playState==='idle' && !animations.has('3/scale');
      const replacement=document.createElement('input'); replacement.dataset.view='3';
      f.views.get(2).append(replacement); views.set(3,replacement); start(3);
      const next=animations.get('3/scale'); await Promise.resolve();
      released &&= animations.get('3/scale')===next && animations.get('5/scale')===survivor;
      peak=Math.max(peak,animations.size);
      apply({op:'destroy',id:3});
      views.set(3,replacement); f.views.get(2).append(replacement);
    }
    start(3); const last=[...animations.values()]; ${reset}
    await Promise.resolve();
    return {released,peak,remaining:animations.size,resetIdle:last.every(a=>a.playState==='idle')};
  })()`);
  expect(result).toEqual({released:true,peak:2,remaining:0,resetIdle:true});
});
