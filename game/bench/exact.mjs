// Exact bench bakes and live carriers. No browser dependency or virtual clock.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
import { appleArtifacts } from '../../host/apple/build.mjs';
import { resolveApp } from '../../scripts/app.mjs';

export const root = resolve(import.meta.dir, '../..');
export const appDir = resolve(import.meta.dir, 'cubes');
export const dist = resolve(root, 'host/web/dist');
export const sleep = ms => new Promise(r => setTimeout(r, ms));
const headless = process.env.BENCH_HEADLESS === '1';
const env = {...process.env, DEVELOPER_DIR:'/Library/Developer/CommandLineTools',
  EXACT_UPDATE_TRUST:'development', EXACT_APP_DIR:appDir, EXACT_IDENTITY:'-'};
const run = (bin, args, options = {}) => spawnSync(bin, args, {cwd:root, env, encoding:'utf8', ...options});
export function displayReady() {
  if (process.platform !== 'darwin') return;
  const r = run('ioreg', ['-n','Root','-d1']);
  if (r.status !== 0 || !r.stdout.includes('"kCGSessionLoginDoneKey"=Yes')) throw new Error('Cannot establish display lock state');
  if (/"CGSSessionScreenIsLocked"=Yes/.test(r.stdout)) throw new Error('Display is locked; build or BENCH_HEADLESS=1 sanity only. No benchmark numbers.');
}
export function focus(pid, activate = false) {
  if (headless || process.platform !== 'darwin') return;
  displayReady();
  const code = `ObjC.import('AppKit'); ${activate ? `$.NSRunningApplication.runningApplicationWithProcessIdentifier(${pid}).activateWithOptions(3);` : ''} $.NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;`;
  const r = run('osascript', ['-l','JavaScript','-e',code], {timeout:10000});
  if (!activate && Number(r.stdout.trim()) !== pid) throw new Error(`Measured PID ${pid} lost focus (${r.stdout.trim() || r.stderr})`);
}
const owned = new WeakMap();
const inventory = () => {
  const r=run('ps',['-axo','pid=,ppid=,stat=,lstart=,command=']);
  if(r.status!==0) throw new Error('Cannot inventory child processes');
  return r.stdout.trim().split('\n').map(line=>{
    const m=line.trim().match(/^(\d+)\s+(\d+)\s+(\S+)\s+(.{24})\s+(.*)$/);
    return m && {pid:Number(m[1]),parent:Number(m[2]),state:m[3],stamp:m[4],command:m[5]};
  }).filter(Boolean);
};
export function track(child) {
  const pids=new Map();
  const sample=()=>{
    const rows=inventory(), parents=new Set([child.pid,...pids.keys()]);
    for(let changed=true;changed;){changed=false;for(const row of rows){
      if((row.pid===child.pid || parents.has(row.parent)) && !pids.has(row.pid)) {
        pids.set(row.pid,row.stamp);parents.add(row.pid);changed=true;
      }
    }}
  };
  sample();const timer=setInterval(sample,1000);
  owned.set(child,{pids,sample,timer});return child;
}
export async function stop(child) {
  if(!child) return;
  const record=owned.get(child);record?.sample();if(record)clearInterval(record.timer);
  if(child.exitCode===null && child.signalCode===null) {
    const exited=new Promise(r=>child.once('exit',r));child.kill('SIGKILL');await exited;
  }
  // Recheck identities before touching recorded descendants; never kill by name.
  if(record) {
    for(const row of inventory()) if(record.pids.get(row.pid)===row.stamp && !row.state.startsWith('Z')) {
      try{process.kill(row.pid,'SIGKILL');}catch(e){if(e.code!=='ESRCH')throw e;}
    }
    const deadline=Date.now()+5000;
    while(Date.now()<deadline) {
      const left=inventory().filter(row=>record.pids.get(row.pid)===row.stamp && !row.state.startsWith('Z'));
      if(!left.length)return;
      await sleep(100);
    }
    throw new Error('A recorded child did not exit after SIGKILL');
  }
}
// Read-only input fingerprint; output hashes prevent reusing another app's dist.
function digest() {
  const h = createHash('sha256');
  const files = run('git',['ls-files','--cached','--others','--exclude-standard']);
  if (files.status) throw new Error('Cannot enumerate build inputs');
  for (const f of [...new Set(files.stdout.trim().split('\n'))].sort()) {
    if (!/\.(rs|toml|lock|contract|json|mjs|js|swift|h|c|wgsl|html|css)$/.test(f)
      || /^(llp|apps|game\/twins|game\/games)\//.test(f)
      || /(^|\/)(target|dist|artifacts|results|node_modules)\//.test(f)
      || (f.startsWith('game/bench/') && !f.startsWith('game/bench/cubes/'))
      || !existsSync(resolve(root,f))) continue;
    h.update(f).update(readFileSync(resolve(root,f)));
  }
  return h.digest('hex');
}
function artifact(host, paths) {
  const h = createHash('sha256');
  try {
    const files = host === 'web' ? ['.exact-build.json','app.wasm','gpu_bg.wasm','gpu.js','glue.js','gpu-glue.js','app.plan'].map(f=>resolve(dist,f)) : [paths.binary, resolve(paths.products,'libexact_gpu.dylib'), resolve(paths.products,'libexact_web.dylib'), resolve(paths.products,'ExactMac-Info.plist')];
    for (const f of files) h.update(readFileSync(f));
    return h.digest('hex');
  } catch { return null; }
}
export async function build(host, n) {
  Object.assign(process.env,env);
  const app = resolveApp('bench-cubes'), paths = appleArtifacts(app);
  const cache = resolve(appDir,'artifacts'); mkdirSync(cache,{recursive:true});
  const stamp = resolve(cache,`bench-${host}.json`);
  const input = digest(), output = artifact(host, paths);
  const expected = {input,n,output};
  if (output && existsSync(stamp) && readFileSync(stamp,'utf8') === JSON.stringify(expected)) return paths;
  const contract = resolve(appDir,'app.contract'), source = readFileSync(contract,'utf8');
  const wrappers = host === 'macos' ? mkdtempSync(join(tmpdir(),'exact2-bench-swift-')) : null;
  let child;
  const interrupted=()=>{void stop(child);};
  process.once('SIGINT',interrupted);process.once('SIGTERM',interrupted);
  try {
    // searchParam returns text; the Contract stdlib has no numeric parse. Bake N.
    writeFileSync(contract,source.replace('state n = 100000',`state n = ${n}`));
    const buildEnv = {...env, BENCH_N:String(n)};
    if (wrappers) {
      writeFileSync(join(wrappers,'swift'), '#!/bin/sh\ncase "$1" in build|test) exec /usr/bin/swift "$@" --build-system native;; *) exec /usr/bin/swift "$@";; esac\n',{mode:0o755});
      buildEnv.PATH = wrappers + ':' + process.env.PATH;
    }
    console.error(`BUILD cubes ${host} BENCH_N=${n}`);
    child = track(spawn(process.execPath,[resolve(root,host === 'web' ? 'host/web/build.mjs' : 'host/apple/build.mjs'), `bench-cubes-${host === 'web' ? 'web' : 'apple'}`], {cwd:root,env:buildEnv,stdio:['ignore',2,2]}));
    const code = await new Promise((ok,no)=>{child.once('exit',ok);child.once('error',no);});
    if (code !== 0) throw new Error(`cubes ${host} build exited ${code}`);
  } finally {
    await stop(child);
    process.removeListener('SIGINT',interrupted);process.removeListener('SIGTERM',interrupted);
    writeFileSync(contract, source);
    if (wrappers) rmSync(wrappers,{recursive:true,force:true});
  }
  const built = artifact(host,paths);
  if (built) writeFileSync(stamp,JSON.stringify({input,n,output:built}));
  return paths;
}
export async function cdp(url) {
  const ws = new WebSocket(url), pending = new Map(); let id=0;
  await new Promise((ok,no)=>{ws.onopen=ok;ws.onerror=no;});
  ws.onmessage = event => { const r=JSON.parse(event.data), p=pending.get(r.id); if (p) {pending.delete(r.id);clearTimeout(p.timer);r.error?p.no(new Error(r.error.message)):p.ok(r.result);} };
  ws.onclose = () => {for(const p of pending.values()){clearTimeout(p.timer);p.no(new Error('CDP closed'));}pending.clear();};
  const call = (method,params={}) => new Promise((ok,no)=>{
    const key=++id, timer=setTimeout(()=>{pending.delete(key);no(new Error(`CDP timeout: ${method}`));},30000);
    pending.set(key,{ok,no,timer});ws.send(JSON.stringify({id:key,method,params}));
  });
  return {call,close:()=>ws.close(),eval:async expression=>{
    const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});
    if(r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result?.value;
  }};
}
export async function browserWorld(chrome, profile, seconds) {
  const deadline=Date.now()+90000; let client;
  while(Date.now()<deadline) {
    if(chrome.exitCode !== null || chrome.signalCode !== null) throw new Error('Chrome exited before CDP');
    const portFile=join(profile,'DevToolsActivePort');
    if(existsSync(portFile)) {
      const port=readFileSync(portFile,'utf8').split('\n')[0];
      try {const pages=await (await fetch(`http://localhost:${port}/json/list`)).json(); const page=pages.find(p=>p.type==='page');if(page){client=await cdp(page.webSocketDebuggerUrl);break;}}catch{}
    }
    await sleep(100);
  }
  if(!client) throw new Error('Chrome CDP startup timed out');
  try {
    await client.call('Emulation.setDeviceMetricsOverride',{width:1280,height:720,deviceScaleFactor:2,mobile:false});
    await client.call('Runtime.enable');
    await client.call('Page.bringToFront'); focus(chrome.pid,true);
    const expr = `(reset=false)=>{const el=document.querySelector('[data-testid="world"],[testId="world"]');const id=Number(el?.dataset.view);return globalThis.exact?.gpu?.agent(id,{op:'state',...(reset?{perf_reset:true}:{})});}`;
    let state;
    while(Date.now()<deadline) {
      try {state=await client.eval(`(${expr})()`);} catch(error) {
        if(!/execution context/i.test(error.message)) throw error;
        await sleep(100);continue;
      }
      if(state?.renderError || state?.error) throw new Error(JSON.stringify(state));
      if(state?.world?.perf?.frameMs?.count>0) break;
      await sleep(100);
    }
    if(!state?.world?.perf?.frameMs?.count) throw new Error('World never produced live frames');
    await sleep(2000); focus(chrome.pid);
    await client.eval(`(${expr})(true)`);
    await sleep(seconds*1000);
    focus(chrome.pid);
    const result=await client.eval(`(${expr})()`);
    const pixels=await client.eval(`(()=>{const c=document.querySelector('canvas');return [c.width,c.height]})()`);
    return {perf:result.world.perf,pixels,tick:result.world.tick,entities:result.world.entities};
  } finally {client.close();}
}
export async function macWorld(paths, seconds) {
  if(headless) throw new Error('macOS has no headless presentation sanity mode');
  displayReady();
  const child=track(spawn(paths.binary,[],{cwd:root,env:{...env,EXACT_AGENT:'live',EXACT_ASSETS:appDir,EXACT_WINDOW_WIDTH:'1280',EXACT_WINDOW_HEIGHT:'720'},stdio:['pipe','pipe','pipe']}));
  const interrupted=()=>{void stop(child);};
  process.once('SIGINT',interrupted);process.once('SIGTERM',interrupted);
  console.error(`LAUNCH exact-macos pid=${child.pid}`);
  let buffer='', waiters=[], lines=[], viewId;
  child.stdout.on('data',data=>{buffer+=data;let end;while((end=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,end);buffer=buffer.slice(end+1);try{const v=JSON.parse(line);const next=waiters.shift();next?next(v):lines.push(v);}catch{console.error(line);}}});
  child.stderr.on('data',data=>process.stderr.write(data));
  const exited=new Promise((_,no)=>{child.once('error',no);child.once('exit',()=>no(new Error('macOS exited before reply')));});
  exited.catch(()=>{});
  const next=()=>Promise.race([new Promise(ok=>lines.length?ok(lines.shift()):waiters.push(ok)),exited]);
  const timed=async(p,ms,label)=>{let timer;try{return await Promise.race([p,new Promise((_,no)=>{timer=setTimeout(()=>no(new Error(label)),ms);})]);}finally{clearTimeout(timer);}};
  const read=async(reset=false)=>{child.stdin.write(JSON.stringify({op:'state',world:true,id:viewId,...(reset?{perf_reset:true}:{})})+'\n');return timed(next(),30000,'macOS state timed out');};
  try {
    // Exec-policy failures: two three-minute windows, then a clear refusal.
    const ready=await timed(next(),360000,'macOS launch silent for six minutes (exec-policy daemon?)');
    if(!ready.ready || ready.error) throw new Error(JSON.stringify(ready));
    child.stdin.write(JSON.stringify({op:'tree'})+'\n');
    const tree=await timed(next(),30000,'macOS tree timed out');
    viewId=tree.nodes?.find(n=>n.props?.testId==='world')?.id;
    if(viewId===undefined) throw new Error(JSON.stringify(tree));
    focus(child.pid,true);await sleep(2000);focus(child.pid);
    await read(true); await sleep(seconds*1000); focus(child.pid);
    const result=await read(); if(!result.world?.perf) throw new Error(JSON.stringify(result));
    return {perf:result.world.perf,pixels:result.world.perf.pixels,tick:result.world.tick,entities:result.world.entities};
  } finally {process.removeListener('SIGINT',interrupted);process.removeListener('SIGTERM',interrupted);await stop(child);}
}
export function summarize({perf,pixels,tick,entities},engine,n) {
  const f=perf.frameMs;
  if(!f.count || !(f.mean>0) || !perf.tickMs.count) throw new Error('No live perf samples');
  if(pixels[0]!==2560 || pixels[1]!==1440 || perf.instances!==Number(n)) throw new Error(`Wrong scene: ${JSON.stringify({pixels,instances:perf.instances,n})}`);
  if(f.count>=16384) throw new Error('Perf window exceeded ring capacity; shorten BENCH_SECONDS');
  const r=v=>Math.round(v*100)/100;
  return {engine,scene:'cubes',n:Number(n),mode:'entities',pixels,frames:f.count,
    fps_avg:Math.round(10000/f.mean)/10,ms_p50:r(f.p50),ms_p95:r(f.p95),ms_p99:r(f.p99),ms_max:r(f.max),
    script_ms_avg:r(perf.tickMs.mean),tick_ms:r(perf.tickMs.mean),feed_ms:r(perf.feedMs.mean),encode_ms:r(perf.encodeMs.mean),
    draws:perf.draws,instances:perf.instances,tick,entities,perf,headless,measurement:headless?'sanity-only':'live'};
}
