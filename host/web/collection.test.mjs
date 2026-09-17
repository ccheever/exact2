// Real DOM geometry and deterministic feedback admission; no app data/network.
import { beforeAll, afterAll, test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp } from '../../scripts/agent.mjs';
import { collectionBytes, motionBytes } from './navigation.js';

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
  await evaluate(`import('/navigation.js').then(module => { globalThis.createController = module.collectionController; globalThis.createMotion = module.motionController; })`);
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
  const result = await evaluate(`(async () => { const f=fixture(); f.controller.commit([f.snapshot()]); f.flush(); await new Promise(r=>setTimeout(r,50)); f.flush(); f.views.get(2).style.height='97px'; f.port.style.width='400px'; for(let i=0;i<40;i++){ await new Promise(r=>setTimeout(r,25)); f.flush(); if(f.reports.at(-1)?.rows[0].height===97 && f.reports.at(-1)?.rowWidth===f.port.clientWidth-24) break; } const last=f.reports.at(-1), n=f.reports.length; await new Promise(r=>setTimeout(r,50)); f.flush(); return {last,inner:f.port.clientWidth,idle:f.reports.length===n}; })()`);
  expect(result.last.rowWidth).toBe(result.inner - 24);
  expect(result.last.rows[0].height).toBe(97);
  expect(result.idle).toBe(true);
});
test('pre-paint port feedback shares four reports with rAF and defers excess without a resize loop', async () => {
  const result = await evaluate(`(() => {
    const NativeObserver=ResizeObserver; let notify;
    globalThis.ResizeObserver=class { constructor(fn){notify=fn;} observe(){} unobserve(){} disconnect(){} };
    let f;
    try { f=fixture(); f.controller.commit([f.snapshot()]); } finally { globalThis.ResizeObserver=NativeObserver; }
    f.flush();
    for(let i=0;i<20;i++) { f.port.style.height=(200+i)+'px'; notify([{target:f.port}]); }
    const before={reports:f.reports.length,pending:f.frames.size,height:f.reports.at(-1).height};
    f.flush(); const after={reports:f.reports.length,height:f.reports.at(-1).height};
    // Identical delivered entries cannot replenish budget or schedule work.
    for(let i=0;i<20;i++) notify([{target:f.port}]);
    return {before,after,pending:f.frames.size};
  })()`);
  expect(result.before).toEqual({reports:4,pending:1,height:202});
  expect(result.after).toEqual({reports:5,height:219});
  expect(result.pending).toBe(0);
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


function motionFixture() {
  const f=fixture(), node=f.views.get(2); f.motion?.reset();
  let serial=9007199254740993n, time=100, epoch=1;
  const held=new Map(), calls=[];
  const apply=batch=>{ for(const op of batch?.ops??[]) if(op.op==='animate') motion.animate(op); };
  const request=r=>{
    calls.push(r);
    if(r.op==='begin') { const token=String(serial++); held.set(token,r); return {token,value:[r.x,r.y],batch:{ops:[]}}; }
    const old=held.get(r.token); if(!old) return {accepted:false};
    if(r.op==='live') return {accepted:true};
    if(r.op==='move') { old.x=r.x; old.y=r.y; return {accepted:true,batch:{ops:[]}}; }
    if(r.op==='action') { f.onAction?.(); return {accepted:true}; }
    held.delete(r.token);
    return {accepted:true,batch:{ops:f.releaseOps??[]}};
  };
  const motion=createMotion({views:f.views,now:()=>time,generation:()=>epoch,request,applyBatch:apply,inert:el=>el.closest('[inert]'),releaseInteraction:pointer=>f.controller.releaseInteraction(pointer)});
  Object.assign(f,{motion,node,held,calls,advance:ms=>time+=ms,reload:()=>{epoch++;motion.reset();}});
  return f;
}
test('WAAPI catch at crossing/overshoot preserves only its property across latest style commits', async () => {
  const result=await evaluate(`(() => { const f=(${motionFixture})(); const m=f.motion,n=f.node;
    m.style(2,'translate:0px;opacity:1;transition:opacity 1s linear');
    const opacity=n.animate([{opacity:1},{opacity:0.2}],{duration:10000});
    m.animate({id:2,property:'translate',values:[[0,0],[100,0],[0,0]],delay:0,duration:1000});
    const animation=n.getAnimations().find(a=>a.effect.getKeyframes().some(k=>k.translate)); animation.pause(); animation.currentTime=500;
    const before=getComputedStyle(n).translate, h=m.begin(2,'translate');
    const caught=getComputedStyle(n).translate;
    m.style(2,'translate:24px;opacity:0.4;background-color:blue;transition:none');
    const held=getComputedStyle(n).translate, color=getComputedStyle(n).backgroundColor;
    m.end(h,[0,0],true);
    return {before,caught,held,color,end:getComputedStyle(n).translate,old:animation.playState,other:opacity.playState,token:h.token}; })()`);
  expect(result.before).toBe('100px'); expect(result.caught).toBe(result.before); expect(result.held).toBe(result.before);
  expect(result.color).toBe('rgb(0, 0, 255)'); expect(result.end).toBe('24px'); expect(result.old).toBe('idle'); expect(result.other).not.toBe('idle'); expect(result.token).toBe('9007199254740993');
});
test('same-clock rebegin rejects stale completion and delayed release holds its first frame', async () => {
  const result=await evaluate(`(() => { const f=(${motionFixture})(); const m=f.motion,n=f.node;
    m.style(2,'translate:0px;transition:none'); const old=m.begin(2,'translate'); m.move(old,[84,0]); const h=m.begin(2,'translate');
    const stale=m.finish(old,[85,0],[0,0],true); const actions=f.calls.filter(c=>c.op==='action').length;
    f.releaseOps=[{op:'animate',id:2,property:'translate',values:[[84,0],[0,0]],delay:100,duration:200}]; m.end(h,[0,0]);
    const animation=n.getAnimations().find(a=>a.effect.getKeyframes().some(k=>k.translate)); animation.pause(); animation.currentTime=0;
    const delay=getComputedStyle(n).translate; f.reload(); const late=m.move(h,[90,0]);
    return {stale,actions,delay,fill:animation.effect.getTiming().fill,late,animations:n.getAnimations().length}; })()`);
  expect(result).toEqual({stale:false,actions:0,delay:'84px',fill:'backwards',late:false,animations:0});
});

test('motion destroy/remount/reset cancel only owned animations', async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,n=f.node; m.animate({id:2,property:'scale',values:[1,2],delay:0,duration:10000}); const old=n.getAnimations()[0];m.destroy(2);m.animate({id:2,property:'scale',values:[1,2],delay:0,duration:10000});const next=n.getAnimations()[0];m.reset();return {old:old.playState,next:next.playState,remaining:n.getAnimations().length};})()`);
  expect(result).toEqual({old:'idle',next:'idle',remaining:0});
});

test('target-crossing catches cancel playback even at the authored target',async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,n=f.node;m.style(2,'translate:80px');m.animate({id:2,property:'translate',values:[[0,0],[80,0],[120,0],[80,0]],delay:0,duration:900});const a=n.getAnimations()[0];a.pause();a.currentTime=300;const before=getComputedStyle(n).translate,h=m.begin(2,'translate');return {before,after:getComputedStyle(n).translate,old:a.playState,value:h.value};})()`);
  expect(result).toEqual({before:'80px',after:'80px',old:'idle',value:[80,0]});
});
test('swipe recognition catches current overshoot unchanged then reverses in displayed units',async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,n=f.node;const events={};n.setPointerCapture=()=>{};n.hasPointerCapture=()=>false;
    m.style(2,'translate:0px');m.animate({id:2,property:'translate',values:[[0,0],[100,0],[0,0]],delay:0,duration:1000});const a=n.getAnimations()[0];a.pause();a.currentTime=250;
    m.attachSwipe(n,2,(name,fn)=>events[name]=fn);
    const event=(type,x)=>({type,isPrimary:true,button:0,pointerId:7,clientX:x,clientY:0,target:n,preventDefault(){},stopPropagation(){}});
    events.pointerdown(event('pointerdown',0));a.currentTime=500;events.pointermove(event('pointermove',10));const caught=getComputedStyle(n).translate;
    f.advance(20);events.pointermove(event('pointermove',30));const forward=getComputedStyle(n).translate;f.advance(20);events.pointermove(event('pointermove',20));const reverse=getComputedStyle(n).translate;
    f.advance(20);events.pointerup(event('pointerup',20));const release=f.calls.findLast(c=>c.op==='release');
    return {caught,forward,reverse,velocity:release.x,actions:f.calls.filter(c=>c.op==='action').length};})()`);
  expect(result.caught).toBe('100px'); expect(result.forward).toBe('104px'); expect(result.reverse).toBe('102px'); expect(result.actions).toBe(1);
  expect(result.velocity).toBeLessThan(0); expect(Math.abs(result.velocity)).toBeLessThan(250);
});
test('accepted final sample precedes action and deletion makes end harmless',async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,h=m.begin(2,'translate');f.onAction=()=>{m.destroy(2);f.node.remove();f.views.delete(2);f.held.delete(h.token);};const accepted=m.finish(h,[70,0],[20,0],true);return {accepted,ops:f.calls.map(c=>c.op),late:m.end(h,[0,0])};})()`);
  expect(result).toEqual({accepted:true,ops:['begin','live','move','action'],late:false});
});

test('CSS easing takeover samples browser presentation and releases toward newest easing target',async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,n=f.node;
    m.style(2,'translate:0px;transition:translate 1s linear');n.getBoundingClientRect();m.style(2,'translate:100px;transition:translate 1s linear');
    const old=n.getAnimations().find(a=>a.transitionProperty==='translate');old.pause();old.currentTime=500;const h=m.begin(2,'translate');
    m.style(2,'translate:200px;transition:translate 1s linear');const held=getComputedStyle(n).translate;m.end(h,[0,0],true);
    const next=n.getAnimations().find(a=>a.transitionProperty==='translate');next.pause();next.currentTime=500;return {held,old:old.playState,mid:getComputedStyle(n).translate};})()`);
  expect(result).toEqual({held:'50px',old:'idle',mid:'125px'});
});

test('immediate left catch uses displayed release threshold and cancellation never commits',async()=>{
  const result=await evaluate(`(() => {const run=(end,type='pointerup')=>{const f=(${motionFixture})();const m=f.motion,n=f.node,events={};n.setPointerCapture=()=>{};n.hasPointerCapture=()=>false;
    m.style(2,'translate:0px');m.animate({id:2,property:'translate',values:[[0,0],[100,0],[0,0]],delay:0,duration:1000});const a=n.getAnimations()[0];a.pause();a.currentTime=250;
    m.attachSwipe(n,2,(name,fn)=>events[name]=fn);const event=(type,x)=>({type,isPrimary:true,button:0,pointerId:7,clientX:x,clientY:0,target:n,preventDefault(){},stopPropagation(){}});
    events.pointerdown(event('pointerdown',0));a.currentTime=500;events.pointermove(event('pointermove',-10));const caught=getComputedStyle(n).translate;
    f.advance(20);events.pointermove(event('pointermove',end));const final=getComputedStyle(n).translate;events[type](event(type,end));
    return {caught,final,actions:f.calls.filter(c=>c.op==='action').length,cancels:f.calls.filter(c=>c.op==='cancel').length};};return [run(-10),run(-220),run(-10,'pointercancel')];})()`);
  expect(result).toEqual([{caught:'100px',final:'100px',actions:1,cancels:0},{caught:'100px',final:'34px',actions:0,cancels:0},{caught:'100px',final:'100px',actions:0,cancels:1}]);
});

test('motion wire keeps the complete u64 serial and millisecond clock',()=>{
  const bytes=motionBytes({op:'release',token:'18446744073709551615',x:17,y:-4,now:1234.5}),d=new DataView(bytes.buffer);
  expect(bytes.length).toBe(48);expect(d.getBigUint64(16,true)).toBe(18446744073709551615n);
  expect(d.getFloat64(24,true)).toBe(17);expect(d.getFloat64(40,true)).toBe(1234.5);
  expect(()=>motionBytes({op:'live',token:Number.MAX_SAFE_INTEGER+1})).toThrow();
});

test('completed batches cancel ineligible held rows and pins before any late pointerup',async()=>{
  const result=await evaluate(`(() => {const run=kind=>{const f=(${motionFixture})();const m=f.motion,n=f.node,events={};n.setPointerCapture=()=>{};n.hasPointerCapture=()=>false;
    f.controller.commit([f.snapshot()]);m.attachSwipe(n,2,(name,fn)=>events[name]=fn);const event=(type,x)=>({type,isPrimary:true,button:0,pointerId:7,clientX:x,clientY:0,target:n,preventDefault(){},stopPropagation(){}});
    n.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:7}));events.pointerdown(event('pointerdown',0));events.pointermove(event('pointermove',10));events.pointermove(event('pointermove',100));f.flush();const pinned=f.reports.at(-1).interaction;
    if(kind==='disabled')n.setAttribute('disabled','');else if(kind==='inert')f.views.get(1).inert=true;else f.views.get(1).style.display='none';
    m.commit();f.flush();const held=f.held.size,pin=f.reports.at(-1).interaction;
    n.removeAttribute('disabled');f.views.get(1).inert=false;f.views.get(1).style.display='';events.pointerup(event('pointerup',100));
    let clickSuppressed=false;events.click({preventDefault(){clickSuppressed=true;},stopPropagation(){}});
    return {kind,pinned,held,pin,clickSuppressed,actions:f.calls.filter(c=>c.op==='action').length,cancels:f.calls.filter(c=>c.op==='cancel').length};};return ['disabled','inert','hidden'].map(run);})()`);
  expect(result).toEqual(['disabled','inert','hidden'].map(kind=>({kind,pinned:2,held:0,pin:0,clickSuppressed:true,actions:0,cancels:1})));
});

test('signed swipe resistance and caught indicators stay continuous across both boundaries',async()=>{
  const result=await evaluate(`(() => {const run=(base,indicator,offsets)=>{const f=(${motionFixture})();const m=f.motion,n=f.node,events={};n.setPointerCapture=()=>{};n.hasPointerCapture=()=>false;
    const companion=document.createElement('div');companion.dataset.view='6';companion.setAttribute('swipeIndicator','true');n.append(companion);f.views.set(6,companion);
    m.style(2,'translate:'+base+'px');m.style(6,'height:10px;width:10px;opacity:'+indicator+';scale:'+indicator);
    m.attachSwipe(n,2,(name,fn)=>events[name]=fn);const event=(type,x)=>({type,isPrimary:true,button:0,pointerId:7,clientX:x,clientY:0,target:n,preventDefault(){},stopPropagation(){}});
    events.pointerdown(event('pointerdown',0));events.pointermove(event('pointermove',10));
    const samples=offsets.map(offset=>{f.advance(20);events.pointermove(event('pointermove',10+offset));return f.calls.filter(c=>c.op==='move').slice(-3).map(c=>c.x);});
    events.pointercancel(event('pointercancel',10));return samples;};
    return {positive:run(100,1,[0,-180,-212,-244,-308,-318,-308]),negative:run(-100,0.5,[0,20,180,244]),middle:run(32,0.6,[0,-16,-32,16,32]),zero:run(0,0.5,[0,32,64])};})()`);
  expect(result.positive).toEqual([[100,1,1],[64,1,1],[32,0.5,0.5],[0,0,0],[-64,0,0],[-66,0,0],[-64,0,0]]);
  expect(result.negative).toEqual([[-100,0.5,0.5],[-96,0.5,0.5],[-64,0.5,0.5],[0,0.5,0.5]]);
  expect(result.middle).toEqual([[32,0.6,0.6],[16,0.3,0.3],[0,0,0],[48,0.8,0.8],[64,1,1]]);
  expect(result.zero).toEqual([[0,0.5,0.5],[32,0.75,0.75],[64,1,1]]);
});
