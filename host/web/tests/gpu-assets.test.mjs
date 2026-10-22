import {assetDelivery} from '../gpu-assets.js';
import {test, expect} from 'bun:test';
import {readFileSync} from 'node:fs';
const source = readFileSync(new URL('../gpu-glue.js', import.meta.url), 'utf8');
test('asset fetches have at most eight simultaneous requests', async () => {
  let active = 0, peak = 0;
  const release = [];
  const entry = {id: 1, view: 1};
  let names = Array.from({length: 32}, (_, i) => `${i}.model`);
  const gpu = {gpu_assets: () => JSON.stringify({requests:names.splice(0),retired:[]}), gpu_asset: () => true};
  const fetch = () => { peak = Math.max(peak, ++active); return new Promise(resolve => release.push(() => {
    active--; resolve(new Response(new Uint8Array(1)));
  })); };
  const api = assetDelivery({getModule:()=>gpu, live:()=>entry, fetch, baseURI:()=> 'http://fixture/'});
  api.assets(entry);
  const firstPeak = peak;
  while (api.assetFlights.size) { for (const finish of release.splice(0)) finish(); await new Promise(r=>setTimeout(r,0)); }
  expect(firstPeak).toBeLessThanOrEqual(8);
  expect(peak).toBeLessThanOrEqual(8);
});
test('a failed cosmetic never gets a first-frame stamp or repeated state serialization', () => {
  let reads = 0;
  const body = 'let recoveringDevice, recoveryTimer; const trapped = () => undefined; const retainTrap = error => { throw error; };\n' + source.slice(source.indexOf('function childFrames(entry)'), source.indexOf('// The live frame clock'));
  const gpu = {gpu_render:()=>0, gpu_agent:()=>{reads++; return JSON.stringify({world:{assets:[{name:'bad.model',state:'Failed'}]}})}};
  const render = new Function('gpu','hidden','exact','size','clockFor','messages','requestAnimationFrame',body+'return render;')(
    gpu, false, {}, ()=>({w:10,h:10,s:1}), x=>x, ()=>{}, ()=>{});
  const entry = {id:1,el:{}};
  render(entry,0); render(entry,1); render(entry,2);
  expect(entry.firstFrameSubmittedMs).toBeUndefined();
  expect(reads).toBe(1);
});

test('a final paused frame delivers submitted output once without another render', () => {
  const entry = {id:1,view:1,name:'world',el:{},host:{},wants:true};
  const surfaces = new Map([[entry.view,entry]]), delivered = [], records = [];
  let renders = 0, submits = 0, pending = false, output = [], record;
  const gpu = {
    gpu_render:()=>{ pending = true; return ++renders === 1 ? 1 : 0; },
    gpu_dirty:()=>false,
    gpu_flush:()=>{
      if (pending) {
        pending = false; submits++;
        if (submits === 2) { output.push('custom frame submitted'); record = '{"complete":true}'; }
      }
      return true;
    },
    // The first-frame diagnostic asks state, which settles the first render.
    gpu_agent:()=>{ gpu.gpu_flush(); return '{"world":{}}'; },
    gpu_published:()=>{ const next = record; record = undefined; return next; },
    gpu_messages:()=>output.length ? JSON.stringify(output.splice(0)) : undefined,
  };
  const body = `
    let recoveringDevice, recoveryTimer, terminalDevice = false, hidden = false;
    let raf = null, frameAt = null, frameRaw = 0, frameGeneration = 0, sentPeriod = 0, liveWindow = false;
    const trapped = () => false, retainTrap = error => { throw error; };
    const pace = Object.assign(now => now, {period_ms:0}), clockFor = now => now;
    const size = () => ({w:10,h:10,s:1}), supplyChildren = () => {}, placeChildren = () => {};
    const assets = () => {}, finishRestore = () => {}, reportRestore = () => {};
    const publishers = new Map([['world',entry]]), live = view => surfaces.get(view);
    const surfaceRecord = (name,json) => records.push([name,json]);
    const exact = {message:(host,text) => delivered.push([host,text])};
    const requestAnimationFrame = () => 1;
  ` + source.slice(source.indexOf('function flush('), source.indexOf('// The live frame clock'))
    + source.slice(source.indexOf('function frame(now)'), source.indexOf('// An editable element'))
    + source.slice(source.indexOf('function messages(entry'), source.indexOf('function listen(entry)'));
  const api = new Function('globalThis','gpu','surfaces','entry','delivered','records',body + 'return {frame,flush};')(
    {exact:{}},gpu,surfaces,entry,delivered,records);
  api.frame(0);
  expect(delivered).toEqual([]);
  api.frame(16);
  expect(entry.wants).toBe(false);
  expect(delivered).toEqual([[entry.host,'custom frame submitted']]);
  expect(records).toEqual([['world','{"complete":true}']]);
  api.flush(); api.frame(32);
  expect([renders,submits]).toEqual([2,2]);
  expect(delivered).toHaveLength(1);
  expect(records).toHaveLength(1);
});

function harness({names, fetch, exact = {}}) {
  const entry = {id: 1, view: 1}, delivered = [], failed = [];
  let retired = [];
  const gpu = {gpu_assets: () => JSON.stringify({requests:names.splice(0),retired:retired.splice(0)}),
    gpu_asset: (_, name, bytes) => { delivered.push([name,bytes]); return true; },
    gpu_asset_failed: (_, name, reason) => { failed.push([name,reason]); return true; }};
  const api = assetDelivery({getModule:()=>gpu, live:()=>entry, fetch, devAssets:()=>exact.devAssets, baseURI:()=> 'http://fixture/'});
  return {...api, entry, delivered, failed, retire: n => retired.push(n)};
}
test('stream limit aborts before joining an oversized chunked response', async () => {
  let cancelled = false, read = 0;
  const api = harness({names:['large.tex'], fetch: async () => new Response(new ReadableStream({
    pull(c) { read++; if (read > 4) c.close(); else c.enqueue(new Uint8Array(32 * 1024 * 1024)); },
    cancel() { cancelled = true; },
  }))});
  api.assets(api.entry);
  await Promise.all([...api.assetFlights].map(f=>f.promise));
  expect(api.delivered).toEqual([]);
  expect(api.failed[0][1]).toContain('64 MiB');
  expect(cancelled).toBe(true);
  expect(read).toBeLessThanOrEqual(4);
});
test('dev assets obey the same pre-copy byte limit', async () => {
  const api = harness({names:['large.tex'], exact:{devAssets:new Map([['assets/large.tex',{bytes:{length:64*1024*1024+1}}]])}});
  api.assets(api.entry);
  await Promise.all([...api.assetFlights].map(f=>f.promise));
  expect(api.delivered).toEqual([]);
  expect(api.failed[0][1]).toContain('64 MiB');
});
test('retirement cancels queued and active names beyond 256 assets', async () => {
  const names = Array.from({length:300},(_,i)=>`${i}.model`);
  const api = harness({names, fetch: (_, {signal}) => new Promise((_, reject) => signal.addEventListener('abort',()=>reject(new Error('aborted'))))});
  api.assets(api.entry);
  const count = api.assetFlights.size;
  api.retire('0.model'); api.retire('20.model'); api.assets(api.entry);
  await new Promise(r=>setTimeout(r,0));
  const remaining = [...api.assetFlights].map(f=>f.name);
  api.cancelAssets(api.entry);
  await Promise.all([...api.assetFlights].map(f=>f.promise));
  expect(count).toBe(300);
  expect(api.failed).toEqual([]);
  expect(remaining).not.toContain('0.model');
  expect(remaining).not.toContain('20.model');
});

test('deferred open retains its carrier and a late refusal journals once', async () => {
  for (const refused of [true, false]) {
    const entry = {id:1,view:1,name:'world'}, exact = {worldCarry:new Uint8Array([1])}, journal = [];
    let names = ['crate.model'], restored = false;
    const gpu = {gpu_carry:()=>undefined, gpu_agent:()=>JSON.stringify({world:{restored}}), gpu_restore:()=>true,
      gpu_assets:()=>JSON.stringify({requests:names.splice(0),retired:[]}), gpu_error:()=>"invalid save",
      gpu_asset:()=>{ restored = !refused; return !refused; }};
    const restoreBody = source.slice(source.indexOf('function reportRestore('), source.indexOf('async function settled('))
      + source.slice(source.indexOf('function restorePending('), source.indexOf('function ensure('));
    const api = new Function('assetDelivery','gpu','exact','live','fetch','messages','schedule','restoreJournal','worldSize',
      restoreBody + `
const delivery = assetDelivery({getModule:()=>gpu, live, fetch, baseURI:()=> 'http://fixture/', delivered:finishRestore});
      return {...delivery, restorePending};`)(
      assetDelivery, gpu, exact, ()=>entry, async()=>new Response(new Uint8Array([1])), ()=>{}, ()=>{}, journal, x=>x);
    const bytes = exact.worldCarry;
    api.restorePending(entry);
    expect(exact.worldCarry).toBe(bytes);
    expect(entry.restoredCarry).toBeUndefined();
    api.assets(entry);
    await Promise.all([...api.assetFlights].map(f=>f.promise));
    api.assets(entry);
    if (refused) {
      expect(entry.restoreError).toContain('invalid save');
      expect(journal.length).toBe(1);
      expect(exact.worldCarry).toBe(bytes);
    } else {
      expect(entry.restoreError).toBeUndefined();
      expect(exact.worldCarry).toBeUndefined();
      expect(entry.restoredCarry).toBe(true);
    }
  }
});

test('retirement makes room for a full replacement batch before aborts settle', async () => {
  const names = Array.from({length:256},(_,i)=>`${i}.tex`), batch = [...names];
  const api = harness({names, fetch:(_, {signal})=>new Promise((_,reject)=>signal.addEventListener('abort',()=>reject(new Error('aborted'))))});
  api.assets(api.entry);
  for (const name of batch) api.retire(name);
  names.push(...batch); api.assets(api.entry);
  const failures = [...api.failed];
  api.cancelAssets(api.entry);
  await new Promise(r=>setTimeout(r,0));
  expect(failures).toEqual([]);
});

test('surface asset delivery keeps the live instance and commits only accepted bytes', async () => {
  const start = source.indexOf('  async reloadAssets('), end = source.indexOf('\n  // Dev only;', start);
  const entry = {id:7,view:3}, old = {bytes:new Uint8Array([1])}, next = {bytes:new Uint8Array([2])};
  const exact = {mutate:fn=>fn(),devAssets:new Map([['assets/tuning.json',old]])}, calls = [];
  let accepted = true;
  const gpu = {gpu_asset:(id,name,bytes)=>{calls.push({id,name,bytes});return accepted;},gpu_error:()=> 'invalid tuning field'};
  const api = new Function('ready','exact','surfaces','gpu','cancelAssets','messages','schedule',`return {${source.slice(start,end)}}`)(
    Promise.resolve(),exact,new Map([[3,entry]]),gpu,()=>{},()=>{},()=>{});
  const cards = new Map([['assets/tuning.json',next]]);
  expect(await api.reloadAssets(cards,['assets/tuning.json'],()=>false)).toBe(false);
  expect(calls).toHaveLength(0);
  expect(await api.reloadAssets(cards,['assets/tuning.json'])).toBe(true);
  expect(calls).toEqual([{id:7,name:'tuning.json',bytes:next.bytes}]);
  expect(entry.id).toBe(7);
  expect(exact.devAssets.get('assets/tuning.json')).toBe(next);
  accepted = false;
  await expect(api.reloadAssets(new Map([['assets/tuning.json',old]]),['assets/tuning.json'])).rejects.toThrow('invalid tuning field');
  expect(exact.devAssets.get('assets/tuning.json')).toBe(next);
});
