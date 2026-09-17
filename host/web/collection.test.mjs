// Real DOM geometry and deterministic feedback admission; no app data/network.
import { beforeAll, afterAll, test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Cdp } from '../../scripts/agent.mjs';
import { collectionBytes, motionBytes } from './navigation.js';

let server, child, cdp, evaluate, protocol, dir;
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
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  protocol = call;
  evaluate = async expression => {
    const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  };
  await call('Page.navigate', { url: `http://127.0.0.1:${server.port}` });
  await evaluate(`import('/navigation.js').then(module => { globalThis.createController = module.collectionController; globalThis.createMotion = module.motionController; globalThis.createArrange = module.arrangeController; })`);
  await evaluate(`(${setup})()`);
});
afterAll(async () => {
  if (child && child.exitCode === null) { const exit = new Promise(r => child.once('exit', r)); child.kill(); await exit; }
  server?.stop(true);
  if (dir) rmSync(dir, { recursive: true, force: true });
});
function setup() {
  globalThis.fixture = (options = {}) => {
    globalThis.f?.controller?.dispose();
    globalThis.f?.motion?.reset();
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
  const apply=batch=>{ for(const op of batch?.ops??[]) {
    if(op.op==='animate') motion.animate(op);
    if(op.op==='height-drag') motion.heightBinding(op);
    if(op.op==='style') motion.style(op.id,op.value);
  } };
  const request=r=>{
    calls.push(r);
    if(r.op==='begin'||r.op==='height-begin') { const token=String(serial++); held.set(token,r); return {token,target:f.heightTarget??r.view,value:[r.x,r.y],batch:{ops:f.beginOps??[]}}; }
    const old=held.get(r.token); if(!old) return {accepted:false};
    if(r.op==='live') return {accepted:true};
    if(r.op==='move') { old.x=r.x; old.y=r.y; return {accepted:true,batch:{ops:[]}}; }
    if(r.op==='action'||r.op==='height-action') { f.onAction?.(r); return {accepted:true}; }
    held.delete(r.token);
    return {accepted:true,batch:{ops:f.releaseOps??[]}};
  };
  const motion=createMotion({views:f.views,now:()=>time,generation:()=>epoch,request,applyBatch:apply,inert:el=>el.closest('[inert]'),releaseInteraction:pointer=>{f.released?.push(pointer);f.controller.releaseInteraction(pointer);}});
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

test('height wire preserves the appended discriminator and exact token', () => {
  const bytes=motionBytes({op:'begin',view:2,property:'height',token:'9007199254740993',x:400,now:100});
  const d=new DataView(bytes.buffer);
  expect(d.getUint32(12,true)).toBe(4);
  expect(d.getBigUint64(16,true)).toBe(9007199254740993n);
  expect(d.getFloat64(24,true)).toBe(400);
});
test('height catch uses constrained CSS pixels and survives a latest-target commit',async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,n=f.node;
    const base='box-sizing:border-box;width:200px;padding:10px;border:2px solid;max-height:400px;';
    m.style(2,base+'height:640px;opacity:1;transition:none');
    const other=n.animate([{opacity:1},{opacity:.5}],{duration:10000});
    m.animate({id:2,property:'height',values:[180,640],delay:0,duration:1000});
    const a=n.getAnimations().find(a=>a.effect.getKeyframes().some(k=>k.height));a.pause();a.currentTime=600;
    const before=getComputedStyle(n).height,h=m.begin(2,'height');m.move(h,h.value);
    m.style(2,base+'height:240px;opacity:.8;transition:none');
    const held=getComputedStyle(n).height;m.end(h,[0,0],true);
    return {before,base:h.value[0],held,end:getComputedStyle(n).height,old:a.playState,other:other.playState};})()`);
  expect(result.before).toBe('400px');expect(result.base).toBe(400);expect(result.held).toBe('400px');
  expect(result.end).toBe('240px');expect(result.old).toBe('idle');expect(result.other).not.toBe('idle');
});
test('explicit height retirement clears held pixels and playback without cancelling translate',async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,n=f.node;
    n.innerHTML='<div style="height:70px"></div>';m.style(2,'height:200px;translate:0px;transition:none');
    const h=m.begin(2,'height'),t=m.begin(2,'translate');m.move(h,[250,0]);m.move(t,[30,0]);
    m.style(2,'height:auto;translate:80px;transition:none');m.retire(2,'height');
    const before=f.calls.length,late=m.move(h,[300,0]),end=m.end(h,[10,0]);
    const actual=getComputedStyle(n),height=actual.height,translate=actual.translate;
    m.end(t,[0,0],true);
    m.style(2,'height:200px;translate:80px;transition:none');m.animate({id:2,property:'height',values:[200,400],delay:0,duration:1000});
    const running=n.getAnimations().find(a=>a.effect.getKeyframes().some(k=>k.height));
    m.style(2,'height:auto;translate:80px;transition:none');m.retire(2,'height');
    return {height,translate,late,end,lateCalls:f.calls.length-before-1,running:running.playState,final:getComputedStyle(n).height};})()`);
  expect(result).toEqual({height:'70px',translate:'30px',late:false,end:false,lateCalls:0,running:'idle',final:'70px'});
});

test('height presentation clamps the negative spring lobe and hold without changing translate',async()=>{
  const result=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,n=f.node;
    n.innerHTML='';m.style(2,'height:20px;min-height:0;padding:0;border:0;translate:0px;transition:none');
    const h=m.begin(2,'height'),t=m.begin(2,'translate');m.move(h,[-49.7162387436,0]);m.move(t,[-40,0]);
    const held=getComputedStyle(n).height,raw=h.value[0],caught=m.begin(2,'height');
    const before=f.calls.length,stale=m.move(h,[80,0]),lateCalls=f.calls.length-before;
    f.releaseOps=[{op:'animate',id:2,property:'height',values:[20,-49.7162387436,-20,20],delay:100,duration:300}];
    m.end(caught,[-2000,0]);const a=n.getAnimations().find(a=>a.effect.getKeyframes().some(k=>k.height));a.pause();
    const frames=a.effect.getKeyframes().map(k=>k.height);a.currentTime=250;
    const lobe=getComputedStyle(n).height;a.currentTime=400;
    return {held,raw,caught:caught.value[0],stale,lateCalls,frames,lobe,end:getComputedStyle(n).height,translate:getComputedStyle(n).translate};})()`);
  expect(result).toEqual({held:'0px',raw:-49.7162387436,caught:0,stale:false,lateCalls:0,frames:['20px','0px','0px','20px'],lobe:'0px',end:'20px',translate:'-40px'});
});


function heightFixture(make,native=false) {
  const f=make(), m=f.motion, panel=f.node, header=document.createElement('div');
  f.heightTarget=2; header.dataset.view='7'; header.textContent='Drag'; panel.append(header); f.views.set(7,header);
  panel.style.cssText='box-sizing:border-box;width:200px;height:640px;min-height:0;max-height:400px;padding:0;border:0;transition:none';
  const events={}, released=[];
  if(!native) { header.setPointerCapture=()=>{}; header.hasPointerCapture=()=>false; }
  m.heightBinding({id:7,target:2,handleKey:'9007199254741007',targetKey:'9007199254741002'});
  m.attachHeightDrag(header,7,(name,fn)=>{events[name]=fn;if(native)header.addEventListener(name,fn);});
  const event=(type,y,x=0,target=header)=>({type,isPrimary:true,button:0,pointerId:9,clientX:x,clientY:y,target,preventDefault(){},stopPropagation(){}});
  Object.assign(f,{panel,header,events,event,released}); return f;
}
test('height header catches current constrained presentation and commits the final displayed sample while held',async()=>{
  const result=await evaluate(`(() => {const f=(${heightFixture})((${motionFixture})),m=f.motion,n=f.panel,e=f.events;
    m.animate({id:2,property:'height',values:[180,640],delay:0,duration:1000});const a=n.getAnimations().find(a=>a.effect.getKeyframes().some(k=>k.height));a.pause();a.currentTime=200;
    e.pointerdown(f.event('pointerdown',100));a.currentTime=800;e.pointermove(f.event('pointermove',90));const caught=getComputedStyle(n).height;
    f.advance(20);e.pointermove(f.event('pointermove',110));const moved=getComputedStyle(n).height;
    m.style(2,'box-sizing:border-box;width:200px;height:180px;max-height:400px;transition:none');const held=getComputedStyle(n).height;
    let during;f.onAction=r=>{during={height:r.x,velocity:r.y,held:f.held.has(r.token),lastMove:f.calls.findLast(c=>c.op==='move').x};m.style(2,'box-sizing:border-box;width:200px;height:360px;max-height:400px;transition:none');};
    f.advance(20);e.pointerup(f.event('pointerup',120));const release=f.calls.findLast(c=>c.op==='release');
    const calls=f.calls.length;e.pointerup(f.event('pointerup',130));
    return {caught,moved,held,during,end:getComputedStyle(n).height,release:release.x,old:a.playState,lateCalls:f.calls.length-calls,order:f.calls.filter(c=>['height-action','release'].includes(c.op)).map(c=>c.op)};})()`);
  expect(result.caught).toBe('400px');expect(result.moved).toBe('380px');expect(result.held).toBe('380px');
  expect(result.during.height).toBe(370);expect(result.during.lastMove).toBe(370);expect(result.during.held).toBe(true);
  expect(result.during.velocity).toBeLessThan(0);expect(result.release).toBe(result.during.velocity);
  expect(result.end).toBe('360px');expect(result.old).toBe('idle');expect(result.lateCalls).toBe(0);expect(result.order).toEqual(['height-action','release']);
});
test('height header reports constrained velocity and keeps negative drag positions out of the host',async()=>{
  const result=await evaluate(`(() => {const run=lower=>{const f=(${heightFixture})((${motionFixture})),m=f.motion,e=f.events;
    m.style(2,lower?'box-sizing:border-box;height:200px;min-height:180px;transition:none':'box-sizing:border-box;height:640px;max-height:400px;transition:none');
    e.pointerdown(f.event('pointerdown',100));e.pointermove(f.event('pointermove',90));f.advance(20);e.pointermove(f.event('pointermove',lower?1000:40));f.advance(20);e.pointerup(f.event('pointerup',lower?1200:0));
    const action=f.calls.find(c=>c.op==='height-action');return {height:action.x,velocity:action.y,minInput:Math.min(...f.calls.filter(c=>c.op==='move').map(c=>c.x))};};return {upper:run(false),lower:run(true)};})()`);
  expect(result.upper.height).toBe(400);expect(result.upper.velocity).toBe(0);
  expect(result.lower.height).toBe(180);expect(result.lower.minInput).toBeGreaterThanOrEqual(0);
});
test('height binding invalidation cancels once and refuses stale release after restoration',async()=>{
  const result=await evaluate(`(() => {return ['disabled','rebind','destroy','reset'].map(kind=>{const f=(${heightFixture})((${motionFixture})),m=f.motion,e=f.events;
    e.pointerdown(f.event('pointerdown',100));e.pointermove(f.event('pointermove',90));f.advance(20);e.pointermove(f.event('pointermove',110));
    if(kind==='disabled'){f.header.setAttribute('disabled','');m.commit();f.header.removeAttribute('disabled');}
    if(kind==='rebind'){m.heightBinding({id:7,target:null,handleKey:'9007199254741007',targetKey:null});m.heightBinding({id:7,target:2,handleKey:'9007199254741007',targetKey:'9007199254741002'});}
    if(kind==='destroy'){m.destroy(7);f.header.remove();f.views.delete(7);}
    if(kind==='reset')f.reload();
    const before=f.calls.length;e.pointerup(f.event('pointerup',130));return {kind,late:f.calls.length-before,actions:f.calls.filter(c=>c.op==='height-action').length,cancels:f.calls.filter(c=>c.op==='cancel').length};});})()`);
  // A reset changes runtime incarnation first: never send cancellation to the
  // replacement runtime using a token owned by the destroyed one.
  expect(result).toEqual(['disabled','rebind','destroy','reset'].map(kind=>({kind,late:0,actions:0,cancels:kind==='reset'?0:1})));
});
test('height header leaves horizontal intent and interactive descendants alone',async()=>{
  const result=await evaluate(`(() => {const f=(${heightFixture})((${motionFixture})),e=f.events;
    e.pointerdown(f.event('pointerdown',100));e.pointermove(f.event('pointermove',102,40));e.pointerup(f.event('pointerup',150,50));
    const button=document.createElement('button');f.header.append(button);e.pointerdown(f.event('pointerdown',100,0,button));e.pointermove(f.event('pointermove',80,0,button));e.pointerup(f.event('pointerup',60,0,button));
    return f.calls.filter(c=>c.op==='height-begin'||c.op==='height-action').length;})()`);
  expect(result).toBe(0);
});

test('browser mouse capture carries a header drag outside its bounds and releases once',async()=>{
  const rect=await evaluate(`(() => {const f=(${heightFixture})((${motionFixture}),true);f.onAction=()=>f.motion.style(2,'box-sizing:border-box;height:360px;max-height:400px;transition:none');const r=f.header.getBoundingClientRect();return {x:r.x+20,y:r.y+r.height/2};})()`);
  const mouse=(type,y,buttons)=>protocol('Input.dispatchMouseEvent',{type,x:rect.x,y,button:'left',buttons,clickCount:1});
  await mouse('mousePressed',rect.y,1);
  await mouse('mouseMoved',rect.y-10,1);
  expect(await evaluate(`f.header.hasPointerCapture(f.calls.find(c=>c.op==='height-begin')?1:-1)`)).toBe(true);
  await evaluate(`f.advance(20)`);await mouse('mouseMoved',rect.y+30,1);
  await evaluate(`f.advance(20)`);await mouse('mouseReleased',rect.y+40,0);
  const result=await evaluate(`({actions:f.calls.filter(c=>c.op==='height-action').length,releases:f.calls.filter(c=>c.op==='release').length,height:f.calls.find(c=>c.op==='height-action')?.x,end:getComputedStyle(f.panel).height,captured:f.header.hasPointerCapture(1)})`);
  expect(result).toEqual({actions:1,releases:1,height:350,end:'360px',captured:false});
});

test('Rust-first handle cancellation clears only its local overlay while a second handle survives',async()=>{
  const result=await evaluate(`(() => {const f=(${heightFixture})((${motionFixture})),m=f.motion,n=f.panel,e=f.events;
    const second=document.createElement('div');second.textContent='second';n.append(second);f.views.set(8,second);m.heightBinding({id:8,target:2,handleKey:'9007199254741008',targetKey:'9007199254741002'});
    e.pointerdown(f.event('pointerdown',100));e.pointermove(f.event('pointermove',90));f.advance(20);e.pointermove(f.event('pointermove',120));
    m.style(2,'height:180px;max-height:400px;transition:none');const held=f.calls.find(c=>c.op==='move').token;
    f.held.delete(held);m.heightBinding({id:7,target:null,handleKey:'9007199254741007',targetKey:null});
    const cleared=getComputedStyle(n).height,before=f.calls.length;e.pointerup(f.event('pointerup',160));
    m.animate({id:2,property:'height',values:[180,360],delay:0,duration:1000});
    return {cleared,late:f.calls.length-before,actions:f.calls.filter(c=>c.op==='height-action').length,animations:n.getAnimations().length,second:second.isConnected};})()`);
  expect(result).toEqual({cleared:'180px',late:0,actions:0,animations:1,second:true});
});

test('begin reply binding retirement owns the adopted hold before synchronous batch delivery',async()=>{
  const result=await evaluate(`(() => {const f=(${heightFixture})((${motionFixture})),m=f.motion,n=f.panel,e=f.events;
    f.beginOps=[{op:'style',id:2,value:'height:180px;max-height:400px;transition:none'},
      {op:'height-drag',id:7,target:null,handleKey:'9007199254741007',targetKey:null}];
    e.pointerdown(f.event('pointerdown',100));e.pointermove(f.event('pointermove',90));
    const cleared=getComputedStyle(n).height,before=f.calls.length;e.pointerup(f.event('pointerup',160));
    m.animate({id:2,property:'height',values:[180,360],delay:0,duration:1000});
    return {cleared,held:f.held.size,cancels:f.calls.filter(c=>c.op==='cancel').length,
      late:f.calls.length-before,actions:f.calls.filter(c=>c.op==='height-action').length,
      animations:n.getAnimations().length,pins:f.released};})()`);
  expect(result).toEqual({cleared:'180px',held:0,cancels:1,late:0,actions:0,animations:1,pins:[9]});
});

test('photo pair packet preserves all seven u64 stamps and the complete terminal payload',()=>{
  const fields={op:'transform-action',runtime:'9007199254741001',handleKey:'9007199254741003',targetKey:'9007199254741005',clipKey:'9007199254741007',geometrySequence:'9007199254741009',translateToken:'9007199254741011',scaleToken:'9007199254741013',values:[13.25,-7.5,1.375,30,-10,0],now:110};
  const bytes=motionBytes(fields),d=new DataView(bytes.buffer);
  expect(bytes.length).toBe(120);expect(d.getUint32(0,true)).toBe(2);expect(d.getUint32(4,true)).toBe(13);
  for(const [i,name] of ['runtime','handleKey','targetKey','clipKey','geometrySequence','translateToken','scaleToken'].entries()) expect(String(d.getBigUint64(8+i*8,true))).toBe(fields[name]);
  expect(Array.from({length:6},(_,i)=>d.getFloat64(64+i*8,true))).toEqual(fields.values);
  expect(d.getFloat64(112,true)).toBe(110);
  expect(()=>motionBytes({...fields,geometrySequence:Number.MAX_SAFE_INTEGER+1})).toThrow();
});

function photoFixture(native=false) {
  globalThis.f?.motion?.reset();globalThis.f?.controller?.dispose();
  const root=document.getElementById('root');
  root.innerHTML='<div data-view="8" style="width:400px;height:300px;overflow:auto"><div data-view="1" style="width:320.25px;height:200.5px;box-sizing:border-box;overflow:hidden;padding:0;border:0"><div data-view="2" style="width:100%;height:100%;box-sizing:border-box;padding:0;border:0;translate:0px 0px;scale:1;transition:none"><div data-view="3" style="width:100%;height:100%">Photo</div></div></div><div style="height:800px"></div></div>';
  const views=new Map([...root.querySelectorAll('[data-view]')].map(el=>[+el.dataset.view,el]));
  const target=views.get(2),clip=views.get(1),handle=views.get(3),outer=views.get(8),events={},calls=[],held=new Map();
  let serial=9007199254741100n,time=100,generation=1,lastDimensions=null;
  const binding={op:'transform-drag',id:3,runtime:'9007199254741099',handleKey:'9007199254741003',target:2,targetKey:'9007199254741002',clip:1,clipKey:'9007199254741001'};
  const f={views,target,clip,handle,outer,events,calls,held,binding,geometryActions:0,actions:0,released:[]};
  const apply=batch=>{
    for(const op of batch?.ops??[]) {
      if(op.op==='transform-drag') motion.transformBinding(op);
      if(op.op==='retire-motion') motion.retire(op.id,op.property,op.token,op.runtime);
      if(op.op==='animate') motion.animate(op);
      if(op.op==='style') motion.style(op.id,op.css);
    }
    motion.commit();
  };
  const start=(property,value)=>{
    for(const [token,h] of held) if(h.property===property) held.delete(token);
    const token=String(serial++);held.set(token,{property,value});return token;
  };
  const request=r=>{
    calls.push({...r,values:r.values&&[...r.values]});
    if(r.op==='transform-geometry') {
      const dimensions=JSON.stringify(r.values.slice(0,4));
      if(dimensions!==lastDimensions) {lastDimensions=dimensions;f.geometryActions++;}
      return {accepted:true,batch:{ops:[]}};
    }
    if(r.op==='transform-invalidate') {return {accepted:true,batch:{ops:[]}};}
    if(r.op==='transform-begin') {
      const [x,y,s]=r.values;
      const translateToken=start('translate',[x,y]),scaleToken=start('scale',[s,0]);
      return {accepted:true,runtime:r.runtime,geometrySequence:r.geometrySequence,translateToken,scaleToken,value:[x,y,s],batch:{ops:f.beginOps??[]}};
    }
    if(r.op==='transform-move'||r.op==='transform-action') {
      if(!held.has(r.translateToken)||!held.has(r.scaleToken))return {accepted:false};
      const [x,y,s]=r.values;held.get(r.translateToken).value=[x,y];held.get(r.scaleToken).value=[s,0];
      if(r.op==='transform-action') {f.actions++;f.onAction?.(r);}
      return {accepted:true,dispatched:r.op==='transform-action',committed:true,batch:{ops:[]}};
    }
    if(r.op==='begin') {const token=start(r.property,[r.x,r.y]);return {token,value:[r.x,r.y],batch:{ops:[]}};}
    if(!held.has(r.token))return {accepted:false};
    if(r.op==='live')return {accepted:true};
    if(r.op==='move'){held.get(r.token).value=[r.x,r.y];return {accepted:true,batch:{ops:[]}};}
    held.delete(r.token);return {accepted:true,batch:{ops:[]}};
  };
  const motion=createMotion({views,now:()=>time,generation:()=>generation,request,applyBatch:apply,inert:el=>el.closest('[inert]'),releaseInteraction:id=>f.released.push(id)});
  if(!native){handle.setPointerCapture=()=>{};handle.hasPointerCapture=()=>false;}
  motion.transformBinding(binding);
  motion.attachTransformDrag(handle,3,(name,fn)=>{events[name]=fn;if(native)handle.addEventListener(name,fn);});
  const event=(type,x,y=100,target=handle)=>({type,isPrimary:true,button:0,pointerId:1,clientX:x,clientY:y,target,preventDefault(){},stopPropagation(){}});
  Object.assign(f,{motion,event,advance:dt=>time+=dt,reload:()=>{generation++;motion.reset();},settle:()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))});
  globalThis.f=f;motion.commit();return f;
}

test('photo geometry reports fractional untransformed dimensions and ignores own motion',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();const first=f.calls.find(c=>c.op==='transform-geometry');const n=f.geometryActions;
    f.motion.style(2,'width:100%;height:100%;box-sizing:border-box;padding:0;border:0;translate:21px -13px;scale:1.75;transition:none');f.motion.commit();await f.settle();
    return {dimensions:first.values.slice(0,4),integerWidth:f.target.offsetWidth,after:f.geometryActions,before:n};})()`);
  expect(result.dimensions).toEqual([320.25,200.5,320.25,200.5]);expect(result.integerWidth).toBe(320);expect(result.after).toBe(result.before);
});

test('photo catch adopts current Translate and Scale at zero displacement and preserves latest controls',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();const m=f.motion,e=f.events;
    const t=f.target.animate([{translate:'0px 0px'},{translate:'80px 0px'}],{duration:1000,fill:'both'}),s=f.target.animate([{scale:1},{scale:2}],{duration:1000,fill:'both'});
    t.pause();s.pause();t.currentTime=250;s.currentTime=250;e.pointerdown(f.event('pointerdown',100));t.currentTime=500;s.currentTime=500;e.pointermove(f.event('pointermove',110));
    const began=f.calls.find(c=>c.op==='transform-begin'),caught={t:getComputedStyle(f.target).translate,s:getComputedStyle(f.target).scale};
    f.advance(20);e.pointermove(f.event('pointermove',130,112));const moved={t:getComputedStyle(f.target).translate,s:getComputedStyle(f.target).scale};
    m.style(2,'width:100%;height:100%;box-sizing:border-box;padding:0;border:0;translate:100px 20px;scale:2;opacity:.7;transition:none');
    const held={t:getComputedStyle(f.target).translate,s:getComputedStyle(f.target).scale};f.advance(20);e.pointerup(f.event('pointerup',130,112));
    const action=f.calls.find(c=>c.op==='transform-action');return {began:began.values.slice(0,3),caught,moved,held,action:action.values.slice(0,3),end:{t:getComputedStyle(f.target).translate,s:getComputedStyle(f.target).scale,opacity:getComputedStyle(f.target).opacity},tokens:f.held.size,actions:f.actions};})()`);
  expect(result.began).toEqual([40,0,1.5]);expect(result.caught).toEqual({t:'40px',s:'1.5'});
  expect(result.moved).toEqual({t:'60px 12px',s:'1.5'});expect(result.held).toEqual(result.moved);
  expect(result.action).toEqual([60,12,1.5]);expect(result.end).toEqual({t:'100px 20px',s:'2',opacity:'0.7'});expect(result.tokens).toBe(0);expect(result.actions).toBe(1);
});

test('photo eligibility refuses 3D, nonuniform, offcenter, perspective and pending ancestor curves',async()=>{
  const result=await evaluate(`(async()=>{const results=[];for(const kind of ['3d','nonuniform','origin','perspective','matrix','ancestor-curve','mixed-target-curve']){
    const f=(${photoFixture})();await f.settle();
    if(kind==='3d')f.target.style.scale='2 2 3';if(kind==='nonuniform')f.target.style.scale='2 3';if(kind==='origin')f.target.style.transformOrigin='0px 0px';
    if(kind==='perspective')f.outer.style.perspective='500px';if(kind==='matrix')f.target.style.transform='translateX(10px)';
    if(kind==='ancestor-curve')f.outer.animate([{translate:'0px 0px'},{translate:'20px 0px'}],{duration:1000,delay:10000});
    if(kind==='mixed-target-curve')f.target.animate([{translate:'0px 0px',opacity:1},{translate:'20px 0px',opacity:.5}],{duration:1000,delay:10000});
    f.motion.commit();f.events.pointerdown(f.event('pointerdown',100));f.events.pointermove(f.event('pointermove',120));f.events.pointerup(f.event('pointerup',130));
    results.push({kind,begins:f.calls.filter(c=>c.op==='transform-begin').length,holds:f.held.size,actions:f.actions});f.motion.reset();}return results;})()`);
  expect(result).toEqual(['3d','nonuniform','origin','perspective','matrix','ancestor-curve','mixed-target-curve'].map(kind=>({kind,begins:0,holds:0,actions:0})));
});

test('photo begin batch retirement sees both adopted holds before reentry and releases capture once',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();f.beginOps=[{...f.binding,target:null,targetKey:null,clip:null,clipKey:null}];
    f.events.pointerdown(f.event('pointerdown',100));f.events.pointermove(f.event('pointermove',110));const before=f.calls.length;f.events.pointerup(f.event('pointerup',130));
    return {tokens:f.held.size,cancels:f.calls.filter(c=>c.op==='cancel').length,late:f.calls.length-before,actions:f.actions,released:f.released};})()`);
  expect(result).toEqual({tokens:0,cancels:2,late:0,actions:0,released:[1]});
});

test('ancestor scroll changes mapping without fake dimensions and cancels the old photo gesture',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();f.events.pointerdown(f.event('pointerdown',100));f.events.pointermove(f.event('pointermove',110));
    const before=f.geometryActions;f.outer.scrollTop=40;f.outer.dispatchEvent(new Event('scroll'));await f.settle();const count=f.calls.length;f.events.pointerup(f.event('pointerup',130));
    return {before,after:f.geometryActions,actions:f.actions,holds:f.held.size,late:f.calls.length-count};})()`);
  expect(result.after).toBe(result.before);expect(result.actions).toBe(0);expect(result.holds).toBe(0);expect(result.late).toBe(0);
});

test('transient mapping change still releases the pair when coalesced geometry returns to its original facts',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();f.events.pointerdown(f.event('pointerdown',100));f.events.pointermove(f.event('pointermove',110));
    const before=f.geometryActions;f.outer.scrollTop=40;f.outer.dispatchEvent(new Event('scroll'));f.outer.scrollTop=0;f.outer.dispatchEvent(new Event('scroll'));
    await f.settle();const count=f.calls.length;f.events.pointerup(f.event('pointerup',130));return {before,after:f.geometryActions,actions:f.actions,holds:f.held.size,cancels:f.calls.filter(c=>c.op==='cancel').length,late:f.calls.length-count};})()`);
  expect(result.after).toBe(result.before);expect(result.actions).toBe(0);expect(result.holds).toBe(0);expect(result.cancels).toBe(2);expect(result.late).toBe(0);
});

test('photo resize publishes latest fractional dimensions once and becomes idle after cancellation',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();f.events.pointerdown(f.event('pointerdown',100));f.events.pointermove(f.event('pointermove',110));
    for(let i=0;i<40;i++)f.clip.style.width=(330+i/4)+'px';await f.settle();const last=f.calls.findLast(c=>c.op==='transform-geometry'),n=f.calls.length;await f.settle();
    return {dims:last.values.slice(0,4),actions:f.actions,holds:f.held.size,geometryActions:f.geometryActions,idle:f.calls.length===n};})()`);
  expect(result.dims).toEqual([339.75,200.5,339.75,200.5]);expect(result.geometryActions).toBe(2);expect(result.holds).toBe(0);expect(result.actions).toBe(0);expect(result.idle).toBe(true);
});

test('photo token-qualified retirement cannot erase a replacement or unrelated animation',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();f.events.pointerdown(f.event('pointerdown',100));f.events.pointermove(f.event('pointermove',110));
    const began=f.calls.find(c=>c.op==='transform-begin');const old=[...f.held.keys()];const replacement=f.motion.begin(2,'scale');f.motion.move(replacement,[1.8,0]);
    const opacity=f.target.animate([{opacity:1},{opacity:.5}],{duration:10000});f.motion.retire(2,'scale',old[1],began.runtime);f.motion.commit();
    return {scale:getComputedStyle(f.target).scale,newHeld:f.held.has(replacement.token),oldHeld:f.held.has(old[0]),opacity:opacity.playState,actions:f.actions};})()`);
  expect(result.scale).toBe('1.8');expect(result.newHeld).toBe(true);expect(result.oldHeld).toBe(false);expect(result.opacity).not.toBe('idle');expect(result.actions).toBe(0);
});

test('photo runtime reset drops queued geometry and late pointer callbacks without cancelling new runtime',async()=>{
  const result=await evaluate(`(async()=>{const f=(${photoFixture})();await f.settle();f.events.pointerdown(f.event('pointerdown',100));f.events.pointermove(f.event('pointermove',110));
    f.clip.style.width='351.75px';f.reload();const n=f.calls.length;f.events.pointerup(f.event('pointerup',130));await f.settle();return {late:f.calls.length-n,actions:f.actions,cancels:f.calls.filter(c=>c.op==='cancel').length};})()`);
  expect(result).toEqual({late:0,actions:0,cancels:0});
});

test('physical primary mouse pan captures outside the photo and commits once',async()=>{
  const rect=await evaluate(`(async()=>{const f=(${photoFixture})(true);await f.settle();const r=f.handle.getBoundingClientRect();return {x:r.x+40,y:r.y+40,right:r.right};})()`);
  const mouse=(type,x,y,buttons)=>protocol('Input.dispatchMouseEvent',{type,x,y,button:'left',buttons,clickCount:1});
  await mouse('mousePressed',rect.x,rect.y,1);await mouse('mouseMoved',rect.x+10,rect.y,1);
  await evaluate('f.advance(20)');await mouse('mouseMoved',rect.right+30,rect.y+20,1);
  await evaluate('f.advance(20)');await mouse('mouseReleased',rect.right+30,rect.y+20,0);
  const result=await evaluate(`({actions:f.actions,held:f.held.size,captured:f.handle.hasPointerCapture(1),final:f.calls.find(c=>c.op==='transform-action')?.values})`);
  expect(result.actions).toBe(1);expect(result.held).toBe(0);expect(result.captured).toBe(false);expect(result.final[0]).toBeGreaterThan(250);expect(result.final[2]).toBe(1);
});

test('Arrange source reservation survives pointerup while focus plus source remain the only two pins', async () => {
  const result = await evaluate(`(() => {
    const f=fixture(); f.controller.commit([f.snapshot()]);
    f.views.get(3).focus({preventScroll:true});
    f.views.get(5).dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:7}));
    f.flush(); const before=f.reports.at(-1);
    const lease=f.controller.retainInteraction(f.views.get(5),7);
    f.root.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:7}));
    f.flush(); const terminal=f.reports.at(-1);
    f.controller.releaseRetainedInteraction(lease); f.flush(); const ended=f.reports.at(-1);
    return {before:[before.focus,before.interaction],terminal:[terminal.focus,terminal.interaction],
      ended:[ended.focus,ended.interaction],pending:f.frames.size};
  })()`);
  expect(result).toEqual({before:[3,5],terminal:[3,5],ended:[3,0],pending:0});
});
test('Arrange old completion cannot release replacement reservation with a reused pointer id', async () => {
  const result = await evaluate(`(() => {
    const f=fixture(); f.controller.commit([f.snapshot()]);
    const down=id=>f.views.get(id).dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:7}));
    down(5); f.flush(); const old=f.controller.retainInteraction(f.views.get(5),7);
    f.controller.releaseRetainedInteraction(old); f.flush();
    down(3); f.flush(); const replacement=f.controller.retainInteraction(f.views.get(3),7);
    f.controller.releaseRetainedInteraction(old); f.flush(); const stale=f.reports.at(-1).interaction;
    f.controller.releaseRetainedInteraction(replacement); f.flush();
    return {stale,ended:f.reports.at(-1).interaction,pending:f.frames.size};
  })()`);
  expect(result).toEqual({stale:3,ended:0,pending:0});
});
test('Arrange terminal reservation survives grip replacement until explicit finish', async () => {
  const result = await evaluate(`(() => {
    const f=fixture(); f.controller.commit([f.snapshot()]);
    const old=f.views.get(5),wrapper=f.views.get(4);
    old.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:7}));f.flush();
    const lease=f.controller.retainInteraction(old,7);
    // Common has entered no-action Terminal and retains this source KEY. The
    // authored grip/root can disappear while its flow wrapper still survives.
    old.remove();f.views.delete(5);
    const replacement=document.createElement('button');replacement.dataset.view='6';replacement.textContent='new grip';
    wrapper.append(replacement);f.views.set(6,replacement);
    f.controller.commit([f.snapshot('2',{rows:[
      {view:2,root:3,index:0,top:0,height:40,epoch:'9007199254740993'},
      {view:4,root:6,index:99,top:1840,height:60,epoch:'3'}]})]);
    f.root.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:7}));f.flush();
    const terminal=f.reports.at(-1).interaction;
    f.controller.releaseRetainedInteraction(lease);f.flush();
    return {terminal,ended:f.reports.at(-1).interaction,wrapperLive:wrapper.isConnected,pending:f.frames.size};
  })()`);
  expect(result).toEqual({terminal:5,ended:0,wrapperLive:true,pending:0});
});

// Real DOM mapping, C0 rebasing and stacking; Common authority is tested in Rust.
test('Arrange mapping uses signed list origin inside a bordered nested port', async()=>{
  const r=await evaluate(`(() => {const f=fixture();f.controller.commit([f.snapshot()]);f.flush();
    const y=f.port.getBoundingClientRect().top+f.port.clientTop+20;
    const m=f.controller.reorderMapping(1,y);
    return {y:m.contentY,expected:y-(f.views.get(1).getBoundingClientRect().top+10)};})()`);
  expect(r.y).toBe(r.expected);
  expect(r.y).toBe(-40);
});
test('Arrange terminal rebases actual wrapper pixels before release and preserves source elevation hit',async()=>{
  const r=await evaluate(`(() => {const f=(${motionFixture})();const m=f.motion,a=f.views.get(2),b=f.views.get(4),list=f.views.get(1);
    a.style.background='red';b.style.background='blue';
    const ha=m.begin(2,'translate'),hb=m.begin(4,'translate');m.move(ha,[0,50]);m.move(hb,[0,-30]);
    const old=[a,b].map(el=>el.getBoundingClientRect().top);
    m.raiseReorder(2,true);
    const hit=a.contains(document.elementFromPoint(a.getBoundingClientRect().left+10,a.getBoundingClientRect().top+20));
    list.insertBefore(b,a);
    m.rebaseReorder([{view:2,token:ha.token,visual:[a.getBoundingClientRect().left,old[0]]},{view:4,token:hb.token,visual:[b.getBoundingClientRect().left,old[1]]}]);
    const next=[a,b].map(el=>el.getBoundingClientRect().top);
    m.style(2,'height:40px;background:green;translate:0px');
    const elevated=getComputedStyle(a).zIndex;
    m.raiseReorder(2,false);
    return {old,next,hit,elevated,cleared:getComputedStyle(a).zIndex};})()`);
  expect(r.next).toEqual(r.old);expect(r.hit).toBe(true);expect(r.elevated).toBe('2147483647');expect(r.cleared).toBe('auto');
});
function arrangeFixture(realFrames=false,clock=()=>performance.now()) {
  const f=fixture({settled:()=>globalThis.f?.arrange?.commit(),...(realFrames?{requestFrame:fn=>requestAnimationFrame(fn),cancelFrame:id=>cancelAnimationFrame(id)}:{})}),calls=[],list=f.views.get(1),source=f.views.get(4),grip=f.views.get(5);
  let terminal=false,token='0',revision=1;
  const initial=[{view:2,key:'9007199254740993',rootKey:'9007199254740994',top:0,offset:0,hold:'0'},
    {view:4,key:'9007199254740995',rootKey:'9007199254740996',top:40,offset:0,hold:'0'}];
  const binding={id:5,runtime:'9007199254740999',handleKey:'9007199254740996',list:1,listKey:'9007199254740998',wrapper:4,
    wrapperKey:'9007199254740995',rootKey:'9007199254740996',rowEpoch:'2'};
  let motion,arrange;
  const apply=batch=>{for(const op of batch?.ops??[]){
    if(op.op==='style')motion.style(op.id,op.css);
    if(op.op==='animate')motion.animate(op);
    if(op.op==='swap'){list.insertBefore(source,f.views.get(2)); f.swapped=true;
      f.controller.commit([f.snapshot(String(++revision),{rows:[{view:4,root:5,index:0,top:0,height:60,epoch:'2'},{view:2,root:3,index:1,top:60,height:40,epoch:'9007199254740993'}]})]);}
  }motion.commit();arrange?.commit();};
  const request=r=>{
    calls.push(r);
    const frame=initial.map(row=>({...row,hold:row.view===4?'99':terminal?'101':'0'}));
    if(r.op==='reorder-begin'){if(f.requireGripPin&&f.reports.at(-1).interaction!==5)return {accepted:false};token='77';return {accepted:true,token,frame,batch:{ops:[]}};}
    if(r.op==='reorder-preview')return {accepted:true,token,frame,certified:true,batch:{ops:[]}};
    if(r.op==='reorder-terminal'||r.op==='reorder-cancel'){
      terminal=true;f.beforeSwap=motion.captureReorder(frame);
      return {accepted:true,token,terminal:true,frame:frame.map(row=>({...row,hold:row.view===4?'99':'101'})),batch:{ops:r.op==='reorder-terminal'?[{op:'swap'}]:[]}};
    }
    if(r.op==='reorder-rebase'){
      f.afterRebase=motion.captureReorder(frame);
      return {accepted:true,token,terminal:true,released:true,frame,batch:{ops:r.rows.map(row=>({op:'animate',id:row.view,property:'translate',values:[row.value,[0,0]],delay:0,duration:200}))}};
    }
    if(r.op==='reorder-finish'){f.finished=(f.finished??0)+1;return {accepted:true,batch:{ops:[]}};}
    return {accepted:false};
  };
  motion=createMotion({views:f.views,now:clock,generation:()=>1,request,applyBatch:apply,inert:el=>el.closest('[inert]')});
  arrange=createArrange({views:f.views,collections:f.controller,motion,request,applyBatch:apply,now:clock,generation:()=>1,inert:el=>el.closest('[inert]')});
  f.controller.commit([f.snapshot()]);f.flush();arrange.binding(binding);
  Object.assign(f,{arrange,motion,calls,source,grip,binding});return f;
}
test('Arrange real pointer catch, one drop, terminal C0 and retained pin until animation settlement',async()=>{
  await evaluate(`(() => {const f=(${arrangeFixture})();f.source.style.background='blue';f.motion.animate({id:4,property:'translate',values:[[0,0],[0,30]],delay:0,duration:1000});
    const a=f.source.getAnimations()[0];a.pause();a.currentTime=500;f.beforeCatch=f.source.getBoundingClientRect().top;})()`);
  const at=await evaluate(`(()=>{const r=f.grip.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  await protocol('Input.dispatchMouseEvent',{type:'mousePressed',x:at.x,y:at.y,button:'left',clickCount:1});
  await protocol('Input.dispatchMouseEvent',{type:'mouseMoved',x:at.x,y:at.y-10,button:'left',buttons:1});
  const caught=await evaluate(`({before:f.beforeCatch,after:f.source.getBoundingClientRect().top,begin:f.calls.filter(r=>r.op==='reorder-begin').length})`);
  expect(caught.begin).toBe(1);expect(caught.after).toBe(caught.before);
  await protocol('Input.dispatchMouseEvent',{type:'mouseMoved',x:at.x,y:at.y-30,button:'left',buttons:1});
  await protocol('Input.dispatchMouseEvent',{type:'mouseReleased',x:at.x,y:at.y-30,button:'left',clickCount:1});
  const terminal=await evaluate(`(()=>{f.flush();for(const a of f.source.getAnimations())a.pause();return {
    drops:f.calls.filter(r=>r.op==='reorder-terminal').length,rebases:f.calls.filter(r=>r.op==='reorder-rebase').length,
    old:f.beforeSwap?.map(r=>r.visual),next:f.afterRebase?.map(r=>r.visual),pin:f.reports.at(-1).interaction,finished:f.finished??0,swapped:f.swapped};})()`);
  expect(terminal.drops).toBe(1);expect(terminal.rebases).toBe(1);expect(terminal.next).toEqual(terminal.old);
  expect(terminal.pin).toBe(5);expect(terminal.finished).toBe(0);expect(terminal.swapped).toBe(true);
  const settled=await evaluate(`(async()=>{for(const el of [f.source,f.views.get(2)])for(const a of el.getAnimations())a.finish();await new Promise(r=>setTimeout(r,0));f.flush();return {finished:f.finished,pin:f.reports.at(-1).interaction,z:getComputedStyle(f.source).zIndex};})()`);
  expect(settled).toEqual({finished:1,pin:0,z:'auto'});
  await evaluate('f.arrange.reset()');
});
test('Arrange packet v3 preserves every u64 and every final wrapper sample',()=>{
  const facts={op:'reorder-terminal',runtime:'18446744073709551615',handleKey:'9007199254740993',listKey:'9007199254740994',wrapperKey:'9007199254740995',rootKey:'9007199254740996',rowEpoch:'9007199254740997',token:'9007199254740998',revision:'9007199254740999',scrollSequence:'9007199254741000',scrollTop:1.25,portWidth:320,portHeight:200,rowWidth:300,totalExtent:1234.5,contentY:800,x:3,y:42,vx:-12,vy:99,now:150,rows:[{key:'9007199254740995',hold:'9007199254741001',value:[3,42]}]};
  const bytes=motionBytes(facts),d=new DataView(bytes.buffer);
  expect(bytes.length).toBe(208);expect(d.getUint32(0,true)).toBe(3);expect(d.getUint32(4,true)).toBe(17);
  expect(d.getBigUint64(8,true)).toBe(18446744073709551615n);expect(d.getBigUint64(72,true)).toBe(9007199254741000n);
  expect(d.getFloat64(128,true)).toBe(800);expect(d.getFloat64(168,true)).toBe(150);
  expect(d.getBigUint64(184,true)).toBe(9007199254741001n);expect(d.getFloat64(200,true)).toBe(42);
  expect(()=>motionBytes({...facts,handleKey:9007199254740992})).toThrow();
  expect(()=>motionBytes({...facts,rows:Array(4097).fill(facts.rows[0])})).toThrow();
});
test('Arrange settle follows a retargeted return and paused virtual-clock completion without idle rAF',async()=>{
  const r=await evaluate(`(async()=>{const f=(${motionFixture})();let ended=0;f.motion.animate({id:2,property:'translate',values:[[0,50],[0,0]],delay:0,duration:200});
    f.motion.settleReorder(2,()=>ended++);f.motion.animate({id:2,property:'translate',values:[[0,30],[0,0]],delay:0,duration:1000});
    const replacement=f.node.getAnimations()[0];replacement.pause();replacement.currentTime=500;await Promise.resolve();await Promise.resolve();
    const middle={ended,settled:f.motion.reorderSettled(2)};replacement.currentTime=1000;
    const virtual=f.motion.reorderSettled(2);replacement.finish();await new Promise(r=>setTimeout(r,0));return {middle,virtual,ended};})()`);
  expect(r).toEqual({middle:{ended:0,settled:false},virtual:true,ended:1});
});
test('Arrange edge scroll reads actual movement and becomes idle at the clamp',async()=>{
  await evaluate(`(async()=>{const f=(${arrangeFixture})(true);f.views.get(1).lastElementChild.style.height='120px';f.controller.commit([f.snapshot('2',{totalExtent:220,count:3})]);await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));})()`);
  const at=await evaluate(`(()=>{const r=f.grip.getBoundingClientRect(),p=f.port.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,bottom:p.y+f.port.clientTop+f.port.clientHeight-2};})()`);
  await protocol('Input.dispatchMouseEvent',{type:'mousePressed',x:at.x,y:at.y,button:'left',clickCount:1});
  await protocol('Input.dispatchMouseEvent',{type:'mouseMoved',x:at.x,y:at.y+10,button:'left',buttons:1});
  await protocol('Input.dispatchMouseEvent',{type:'mouseMoved',x:at.x,y:at.bottom,button:'left',buttons:1});
  const r=await evaluate(`(async()=>{for(let n=0;n<20;n++)await new Promise(requestAnimationFrame);const g=f.controller.reorderMapping(1,0);const a={top:f.port.scrollTop,max:Math.min(f.port.scrollHeight-f.port.clientHeight,f.port.scrollTop+g.totalExtent-g.portHeight-g.raw),calls:f.calls.length};for(let n=0;n<4;n++)await new Promise(requestAnimationFrame);return {a,b:{top:f.port.scrollTop,calls:f.calls.length},previews:f.calls.filter(r=>r.op==='reorder-preview').length,cancels:f.calls.filter(r=>r.op==='reorder-cancel').length};})()`);
  if(r.a.top!==r.a.max) console.log(JSON.stringify({r,trace:await evaluate(`({calls:f.calls.map(r=>({op:r.op,raw:r.raw,contentY:r.contentY,y:r.y,sequence:r.scrollSequence})),reports:f.reports,css:[f.grip,f.source,f.views.get(1),f.port].map(el=>({tag:el.tagName,translate:getComputedStyle(el).translate,scale:getComputedStyle(el).scale,rect:el.getBoundingClientRect().toJSON()}))})`)}));
  expect(r.a.top).toBe(r.a.max);expect(r.a.top).toBeGreaterThan(0);expect(r.b).toEqual({top:r.a.top,calls:r.a.calls});expect(r.previews).toBeGreaterThan(3);expect(r.cancels).toBe(0);
  await protocol('Input.dispatchMouseEvent',{type:'mouseReleased',x:at.x,y:at.bottom,button:'left',clickCount:1});
  await evaluate('f.arrange.reset()');
});
test('Arrange pointer-up during unaccepted geometry cancels once without losing terminal cleanup',async()=>{
  await evaluate(`(() => {const f=(${arrangeFixture})();})()`);
  const at=await evaluate(`(()=>{const r=f.grip.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  await protocol('Input.dispatchMouseEvent',{type:'mousePressed',x:at.x,y:at.y,button:'left',clickCount:1});
  await protocol('Input.dispatchMouseEvent',{type:'mouseMoved',x:at.x,y:at.y+10,button:'left',buttons:1});
  // A real accepted feedback turn is not available for the final physical
  // sample. Never use the previously certified gap or leave a contact alive.
  await evaluate(`(()=>{f.savedMapping=f.controller.reorderMapping;f.controller.reorderMapping=()=>undefined;})()`);
  await protocol('Input.dispatchMouseEvent',{type:'mouseReleased',x:at.x,y:at.y+25,button:'left',clickCount:1});
  const r=await evaluate(`(async()=>{f.controller.reorderMapping=f.savedMapping;for(const el of [f.source,f.views.get(2)])for(const a of el.getAnimations())a.finish();await new Promise(r=>setTimeout(r,0));f.flush();return {cancel:f.calls.filter(r=>r.op==='reorder-cancel').length,drop:f.calls.filter(r=>r.op==='reorder-terminal').length,rebase:f.calls.filter(r=>r.op==='reorder-rebase').length,finish:f.finished??0,pin:f.reports.at(-1).interaction};})()`);
  await evaluate('f.arrange.reset()');
  expect(r).toEqual({cancel:1,drop:0,rebase:1,finish:1,pin:0});
});
test('Arrange destruction retires its binding listener and touch policy without waiting for reset',async()=>{
  const r=await evaluate(`(()=>{const f=(${arrangeFixture})();f.arrange.reset();f.grip.style.touchAction='pan-y';f.arrange.binding(f.binding);
    const attached=f.grip.style.touchAction;
    // Destruction removes the binding before the element leaves the DOM.
    let contacts=0;f.controller.reorderContact=()=>contacts++;
    f.arrange.destroy(5);f.grip.dispatchEvent(new PointerEvent('pointerdown',{isPrimary:true,button:0,pointerId:7}));f.grip.remove();f.views.delete(5);
    const retired=f.grip.style.touchAction;f.arrange.reset();return {attached,retired,contacts};})()`);
  expect(r).toEqual({attached:'none',retired:'pan-y',contacts:0});
});
test('Arrange edge speed follows elapsed time across 60 and 120Hz, bounds stalls and resets idle time',async()=>{
  const r=await evaluate(`(()=>{
    const request=requestAnimationFrame,cancel=cancelAnimationFrame;
    const run=hz=>{let time=0,serial=0;const frames=new Map();
      window.requestAnimationFrame=fn=>{frames.set(++serial,fn);return serial;};window.cancelAnimationFrame=id=>frames.delete(id);
      const f=(${arrangeFixture})(false,()=>time);f.grip.setPointerCapture=()=>{};f.grip.hasPointerCapture=()=>false;
      const row=f.grip.getBoundingClientRect(),port=f.port.getBoundingClientRect(),x=row.x+10,y=row.y+row.height/2,bottom=port.y+f.port.clientTop+f.port.clientHeight-2;
      const event=(target,type,at)=>target.dispatchEvent(new PointerEvent(type,{bubbles:true,isPrimary:true,pointerId:7,button:0,buttons:1,clientX:x,clientY:at}));
      event(f.grip,'pointerdown',y);event(document,'pointermove',y+10);event(document,'pointermove',bottom);
      const tick=at=>{time=at;f.flush();const callbacks=[...frames.values()];frames.clear();for(const fn of callbacks)fn(at);f.flush();};
      tick(0);const initial=f.port.scrollTop;for(let n=1;n<=hz;n++)tick(n*1000/hz);
      const distance=f.port.scrollTop-initial,preStall=f.port.scrollTop;tick(11000);const stall=f.port.scrollTop-preStall;
      event(document,'pointermove',port.y+f.port.clientTop+f.port.clientHeight/2);const idle=frames.size;
      time=21000;event(document,'pointermove',bottom);const preResume=f.port.scrollTop;tick(21000);const resumed=f.port.scrollTop-preResume;
      for(let n=1;n<=200;n++)tick(21000+n*1000/hz);const clamped={top:f.port.scrollTop,queued:frames.size};tick(99999);const still=f.port.scrollTop;
      f.arrange.reset();return {distance,stall,idle,resumed,clamped,still};
    };
    try{return {hz60:run(60),hz120:run(120)};}finally{window.requestAnimationFrame=request;window.cancelAnimationFrame=cancel;}
  })()`);
  expect(r.hz60.distance).toBe(720);expect(r.hz120.distance).toBe(720);
  for(const arm of [r.hz60,r.hz120]){
    expect(arm.stall).toBeLessThanOrEqual(24);expect(arm.idle).toBe(0);expect(arm.resumed).toBe(0);
    expect(arm.clamped.queued).toBe(0);expect(arm.still).toBe(arm.clamped.top);
  }
});
test('Arrange nested grip label pins the authored handle before Common admission',async()=>{
  await evaluate(`(()=>{const f=(${arrangeFixture})();f.requireGripPin=true;const label=document.createElement('span');label.dataset.view='6';label.textContent='nested label';f.grip.replaceChildren(label);f.views.set(6,label);})()`);
  const at=await evaluate(`(()=>{const r=f.views.get(6).getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  await protocol('Input.dispatchMouseEvent',{type:'mousePressed',x:at.x,y:at.y,button:'left',clickCount:1});
  await protocol('Input.dispatchMouseEvent',{type:'mouseMoved',x:at.x,y:at.y+10,button:'left',buttons:1});
  const r=await evaluate(`(()=>{f.flush();return {pin:f.reports.at(-1).interaction,source:getComputedStyle(f.source).zIndex,begin:f.calls.filter(r=>r.op==='reorder-begin').length};})()`);
  await protocol('Input.dispatchMouseEvent',{type:'mouseReleased',x:at.x,y:at.y+10,button:'left',clickCount:1});
  await evaluate('f.arrange.reset()');
  expect(r).toEqual({pin:5,source:'2147483647',begin:1});
});
test('Arrange same-row handoff never reports null or steals focus, and rejects stale or different-row transfers',async()=>{
  const r=await evaluate(`(()=>{const f=fixture();f.controller.commit([f.snapshot()]);f.views.get(3).focus();f.flush();
    f.views.get(5).dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:7}));f.flush();
    const old=f.controller.retainInteraction(f.views.get(5),7),start=f.reports.length;
    const refused=f.controller.transferRetainedInteraction(old,f.views.get(3),8);
    const wrapper=f.controller.transferRetainedInteraction(old,f.views.get(4),8);
    const stale=f.controller.releaseRetainedInteraction(old);
    const grip=f.controller.transferRetainedInteraction(wrapper,f.views.get(5),8);
    const handoff=f.reports.slice(start).map(r=>({focus:r.focus,interaction:r.interaction}));
    f.controller.releaseRetainedInteraction(grip);f.flush();return {refused:refused===null,stale,handoff,ended:f.reports.at(-1).interaction};})()`);
  expect(r).toEqual({refused:true,stale:false,handoff:[{focus:3,interaction:4},{focus:3,interaction:5}],ended:0});
});
for(const abort of ['pointerup','pointercancel','horizontal'])test(`Arrange returning source keeps its original owner through pre-recognition ${abort}`,async()=>{
  await evaluate(`(()=>{const f=(${arrangeFixture})();})()`);
  const point=()=>evaluate(`(()=>{const r=f.grip.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  const mouse=(type,p)=>protocol('Input.dispatchMouseEvent',{type,...p,button:'left',...(type==='mouseMoved'?{buttons:1}:{clickCount:1})});
  const p=await point();await mouse('mousePressed',p);await mouse('mouseMoved',{x:p.x,y:p.y-10});await mouse('mouseMoved',{x:p.x,y:p.y-30});await mouse('mouseReleased',{x:p.x,y:p.y-30});
  await evaluate('for(const a of f.source.getAnimations()){a.pause();a.currentTime=50;}');
  const again=await point();await mouse('mousePressed',again);
  if(abort==='horizontal')await mouse('mouseMoved',{x:again.x+20,y:again.y});
  if(abort==='pointercancel')await evaluate("document.dispatchEvent(new PointerEvent('pointercancel',{pointerId:1,bubbles:true}))");
  await mouse('mouseReleased',again);
  const held=await evaluate(`(()=>{f.flush();return {finished:f.finished??0,begins:f.calls.filter(r=>r.op==='reorder-begin').length,pin:f.reports.at(-1).interaction};})()`);
  const settled=await evaluate(`(async()=>{for(const el of [f.source,f.views.get(2)])for(const a of el.getAnimations())a.finish();await new Promise(r=>setTimeout(r,0));f.flush();return {finished:f.finished??0,pin:f.reports.at(-1).interaction};})()`);
  await evaluate('f.arrange.reset()');
  expect(held).toEqual({finished:0,begins:1,pin:5});expect(settled).toEqual({finished:1,pin:0});
});
