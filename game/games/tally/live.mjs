#!/usr/bin/env bun
// Real layout-observer race and human-clock Tally, using the baked ordinary page.
// Run after: EXACT_APP_DIR=game/games/tally EXACT_WEB_DIST=game/games/tally/dist bun host/web/build.mjs
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {Cdp} from '../../../scripts/agent.mjs';
const dist=resolve(import.meta.dir,'dist');
const profile=mkdtempSync(resolve(tmpdir(),'k8-tally-live-'));
const server=Bun.serve({port:0,fetch(request) {
  const path=new URL(request.url).pathname;
  if(path==='/gpu.js') return new Response(`export * from './real-gpu.js'; export {default} from './real-gpu.js'; import {gpu_advance as advance} from './real-gpu.js'; export function gpu_advance(id,at) { globalThis.race.advances.push(at); return advance(id,at); }`,{headers:{'content-type':'text/javascript'}});
  return new Response(Bun.file(resolve(dist,path==='/'?'index.html':path==='/real-gpu.js'?'gpu.js':path.slice(1))));
}});
let child,exited;
try {
  child=spawn(process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',[
    '--headless=new','--remote-debugging-pipe','--no-sandbox','--no-first-run',
    '--disable-background-networking',`--user-data-dir=${profile}`,'about:blank',
  ],{stdio:['ignore','ignore','ignore','pipe','pipe']});
  exited=new Promise(resolve=>{child.once('exit',resolve);child.once('error',resolve);});
  await new Promise((resolve,reject)=>{child.once('spawn',resolve);child.once('error',reject);});
  const cdp=new Cdp(child.stdio[3],child.stdio[4]);
  const {targetId}=await cdp.send('Target.createTarget',{url:'about:blank'});
  const {sessionId}=await cdp.send('Target.attachToTarget',{targetId,flatten:true});
  const call=(method,params={})=>cdp.send(method,params,sessionId);
  const evaluate=async expression=>{
    const result=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});
    if(result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  };
  // Without the Page domain enabled the script below is never injected, and the wrapped
  // gpu_advance then throws on every frame: the world looks frozen when it is the fixture that failed.
  await call('Page.enable');
  await call('Page.addScriptToEvaluateOnNewDocument',{source:`
    globalThis.race={advances:[],observerAdvances:null,done:false};
    const raf=globalThis.requestAnimationFrame.bind(globalThis), cancel=globalThis.cancelAnimationFrame.bind(globalThis);
    const pending=new Map();let next=-1;
    globalThis.requestAnimationFrame=callback=>{
      if(callback.name==='frame' && !race.done) {const id=next--;pending.set(id,callback);return id;}
      return raf(callback);
    };
    globalThis.cancelAnimationFrame=id=>{if(id<0)pending.delete(id);else cancel(id);};
    const RealObserver=globalThis.ResizeObserver;
    globalThis.ResizeObserver=class extends RealObserver {
      constructor(callback) {super((entries,observer)=>{
        const before=race.advances.length;callback(entries,observer);
        if(!race.done && entries.some(entry=>entry.target.tagName==='CANVAS')) {
          race.observerAdvances=race.advances.length-before;race.done=true;
          raf(at=>{for(const callback of pending.values())callback(at);pending.clear();});
        }
      });}
      observe(target,options) {super.observe(target,options);if(target.tagName==='CANVAS')target.style.width='calc(100% - 1px)';}
    };
  `});
  // No ?agent, no clock operation, and no handoff: the human owns time from boot.
  await call('Page.navigate',{url:`http://127.0.0.1:${server.port}/`});
  let ready=false;
  for(let i=0;i<500;i++) {
    // The first polls can still see about:blank, where the injected script never ran.
    ready=await evaluate('Boolean(globalThis.exact?.gpu && globalThis.race?.done)');
    if(ready)break;
    await Bun.sleep(20);
  }
  assert.ok(ready,'real ResizeObserver must deliver before the held first world frame');
  assert.equal(await evaluate('race.observerAdvances'),0,'negative control: resize must never call gpu_advance');
  // A page opened without ?agent has no agent carrier at all: that IS the human-owned clock. Read what a person
  // would see — the Contract text bound to the world's published tick count.
  assert.equal(await evaluate('typeof exact.agent'),'undefined');
  assert.equal(await evaluate('Boolean(exact.now)'),false);
  const ticks=async()=>Number(/Tick (\d+)/.exec(await evaluate('document.body.innerText'))?.[1]);
  const before=await ticks();
  await Bun.sleep(1000);
  const after=await ticks();
  assert.ok(Number.isFinite(before) && after>before,`negative control: live ticks must advance after the observer race (${before} → ${after})`);
  assert.ok(after-before>=30 && after-before<=90,`about 60 ticks per second expected, got ${after-before}`);
  console.log(JSON.stringify({before,after,elapsedMs:1000,observerAdvances:0,owner:'human'}));
} finally {
  if(child?.pid)child.kill('SIGKILL');
  if(exited)await exited;
  server.stop(true);rmSync(profile,{recursive:true,force:true});
}
