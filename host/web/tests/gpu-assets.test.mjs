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
    active--; resolve({ok:true, arrayBuffer:async()=>new ArrayBuffer(1)});
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
  const body = source.slice(source.indexOf('function render(entry, now)'), source.indexOf('// The live frame clock'));
  const gpu = {gpu_render:()=>0, gpu_agent:()=>{reads++; return JSON.stringify({world:{assets:[{name:'bad.model',state:'Failed'}]}})}};
  const render = new Function('gpu','hidden','exact','size','clockFor','messages','requestAnimationFrame',body+'return render;')(
    gpu, false, {}, ()=>({w:10,h:10,s:1}), x=>x, ()=>{}, ()=>{});
  const entry = {id:1,el:{}};
  render(entry,0); render(entry,1); render(entry,2);
  expect(entry.firstFrameSubmittedMs).toBeUndefined();
  expect(reads).toBe(1);
});
