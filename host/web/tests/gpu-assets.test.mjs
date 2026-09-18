import {test, expect} from 'bun:test';
import {readFileSync} from 'node:fs';
const source = readFileSync(new URL('../gpu-glue.js', import.meta.url), 'utf8');
test('asset fetches have at most eight simultaneous requests', async () => {
  let active = 0, peak = 0;
  const release = [];
  const entry = {id: 1, view: 1};
  let names = Array.from({length: 32}, (_, i) => `${i}.model`);
  const gpu = {gpu_assets: () => JSON.stringify(names.splice(0)), gpu_asset: () => true};
  const fetch = () => { peak = Math.max(peak, ++active); return new Promise(resolve => release.push(() => {
    active--; resolve(new Response(new Uint8Array(1)));
  })); };
  const body = source.slice(source.indexOf('const ASSET_DEADLINE_MS'), source.indexOf('function size('));
  const api = new Function('gpu','exact','live','fetch','document','messages','schedule', body + '\nreturn {assets, assetFlights};')(
    gpu, {}, () => entry, fetch, {baseURI:'http://fixture/'}, () => {}, () => {});
  api.assets(entry);
  const firstPeak = peak;
  while (api.assetFlights.size) { for (const finish of release.splice(0)) finish(); await new Promise(r=>setTimeout(r,0)); }
  expect(firstPeak).toBeLessThanOrEqual(8);
  expect(peak).toBeLessThanOrEqual(8);
});
test('a failed cosmetic never gets a first-frame stamp or repeated state serialization', () => {
  let reads = 0;
  const body = 'let recoveringDevice;\n' + source.slice(source.indexOf('function childFrames(entry)'), source.indexOf('// The live frame clock'));
  const gpu = {gpu_render:()=>0, gpu_agent:()=>{reads++; return JSON.stringify({world:{assets:[{name:'bad.model',state:'Failed'}]}})}};
  const render = new Function('gpu','hidden','exact','size','clockFor','messages','requestAnimationFrame',body+'return render;')(
    gpu, false, {}, ()=>({w:10,h:10,s:1}), x=>x, ()=>{}, ()=>{});
  const entry = {id:1,el:{}};
  render(entry,0); render(entry,1); render(entry,2);
  expect(entry.firstFrameSubmittedMs).toBeUndefined();
  expect(reads).toBe(1);
});

function harness({names, fetch, exact = {}}) {
  const entry = {id: 1, view: 1}, delivered = [], failed = [];
  let retired = [];
  const gpu = {gpu_assets: () => JSON.stringify(names.splice(0)), gpu_retired: () => JSON.stringify(retired.splice(0)),
    gpu_asset: (_, name, bytes) => { delivered.push([name,bytes]); return true; },
    gpu_asset_failed: (_, name, reason) => { failed.push([name,reason]); return true; }};
  const body = source.slice(source.indexOf('const ASSET_DEADLINE_MS'), source.indexOf('function size('));
  const api = new Function('gpu','exact','live','fetch','document','messages','schedule', body + '\nreturn {assets, assetFlights, cancelAssets};')(
    gpu, exact, () => entry, fetch, {baseURI:'http://fixture/'}, () => {}, () => {});
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
test('retirement cancels queued and active names and the whole queue is bounded', async () => {
  const names = Array.from({length:300},(_,i)=>`${i}.model`);
  const api = harness({names, fetch: (_, {signal}) => new Promise((_, reject) => signal.addEventListener('abort',()=>reject(new Error('aborted'))))});
  api.assets(api.entry);
  const count = api.assetFlights.size;
  api.retire('0.model'); api.retire('20.model'); api.assets(api.entry);
  await new Promise(r=>setTimeout(r,0));
  const remaining = [...api.assetFlights].map(f=>f.name);
  api.cancelAssets(api.entry);
  await Promise.all([...api.assetFlights].map(f=>f.promise));
  expect(count).toBeLessThanOrEqual(256);
  expect(remaining).not.toContain('0.model');
  expect(remaining).not.toContain('20.model');
});

test('deferred open retains its carrier and a late refusal journals once', async () => {
  for (const refused of [true, false]) {
    const entry = {id:1,view:1,name:'world'}, exact = {worldCarry:new Uint8Array([1])}, journal = [];
    let names = ['crate.model'], restored = false;
    const gpu = {gpu_carry:()=>undefined, gpu_agent:()=>JSON.stringify({world:{restored}}), gpu_restore:()=>true,
      gpu_assets:()=>JSON.stringify(names.splice(0)), gpu_retired:()=>"[]", gpu_error:()=>"invalid save",
      gpu_asset:()=>{ restored = !refused; return !refused; }};
    const assetBody = source.slice(source.indexOf('const ASSET_DEADLINE_MS'), source.indexOf('function size('));
    const restoreBody = source.slice(source.indexOf('function restorePending('), source.indexOf('function ensure('));
    const api = new Function('gpu','exact','live','fetch','document','messages','schedule','restoreJournal','worldSize',
      assetBody + restoreBody + '\nreturn {assets, assetFlights, restorePending};')(
      gpu, exact, ()=>entry, async()=>new Response(new Uint8Array([1])), {baseURI:'http://fixture/'}, ()=>{}, ()=>{}, journal, x=>x);
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
