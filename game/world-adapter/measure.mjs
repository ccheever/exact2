#!/usr/bin/env bun
// Diagnostic, not a gate. Run after the ordinary Tally production web bake.
import {spawn,spawnSync} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {gzipSync} from 'node:zlib';
import {Cdp} from '../../scripts/agent.mjs';
import {serveStatic} from '../../host/web/serve.mjs';
const root=resolve(import.meta.dir,'../..'), scratch=resolve(process.env.K2_SCRATCH ?? '/home/ccheever/lanes/gamenext/scratch/k2');
const dist=resolve(root,'game/games/tally/dist');
const median=a=>a.sort((a,b)=>a-b)[Math.floor(a.length/2)];
const sizes=dir=>Object.fromEntries(['app.wasm','gpu_bg.wasm','gpu.js'].map(name=>{const b=readFileSync(resolve(dir,name));return [name,{raw:b.length,gzip:gzipSync(b,{level:9}).length}];}));
if(process.argv.includes('--sizes')) console.log(JSON.stringify({full:sizes(resolve(scratch,'full')),deviceFree:sizes(dist)},null,2));
if(process.argv.includes('--linux')) {
  const rows=[];
  for(let i=0;i<20;i++) {
    const p=spawnSync(resolve(root,'game/target/x86_64-unknown-linux-gnu/gpu-dev/tally-linux'),[],{env:{...process.env,EXACT_AGENT:'1',EXACT_WORLD_TIMING:'1',EXACT_UPDATE_TRUST:'development'},input:'',encoding:'utf8'});
    if(p.status!==0)throw Error(p.stderr);
    const row={}; for(const line of p.stderr.split('\n'))if(line.startsWith('exact-world-startup: ')){const v=JSON.parse(line.slice(21));row[v.event]=v.ms;}
    if(!row.first_tick||!row.first_publication)throw Error(p.stderr);rows.push(row);
  }
  const result={rows,summary:Object.fromEntries(['first_tick','first_publication'].map(key=>{const a=rows.map(r=>r[key]).sort((a,b)=>a-b);return [key,{median:median([...a]),p95:a[18]}];}))};
  writeFileSync(resolve(scratch,'linux.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result.summary));
}
if(process.argv.includes('--web')) {
  let mode='after';
  const baseline=spawnSync('git',['show','f418821:host/web/gpu-glue.js'],{cwd:root,encoding:'utf8'}).stdout.replace('gpu = await loadModule(version);','gpu = await loadModule(version); exact.root.dataset.worldModuleMs = performance.now();');
  const server=createServer((req,res)=>{
    const path=new URL(req.url,'http://local').pathname;
    if(mode==='before' && path==='/gpu-glue.js'){res.writeHead(200,{'content-type':'text/javascript'});res.end(baseline);return;}
    if(mode==='before' && ['/gpu.js','/gpu_bg.wasm'].includes(path)){res.writeHead(200,{'content-type':path.endsWith('.wasm')?'application/wasm':'text/javascript'});res.end(readFileSync(resolve(scratch,'full',path.slice(1))));return;}
    serveStatic(dist,req,res);
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const url=`http://127.0.0.1:${server.address().port}/`;
  const records=[];
  async function browser(cache,loads) {
    const profile=mkdtempSync(resolve(scratch,'chromium-'));
    const p=spawn(process.env.CHROME ?? resolve(process.env.HOME,'.cache/ms-playwright/chromium-1234/chrome-linux64/chrome'),['--headless=new','--remote-debugging-pipe','--no-sandbox','--disable-gpu','--disable-background-networking',`--user-data-dir=${profile}`,'about:blank'],{detached:true,stdio:['ignore','ignore','pipe','pipe','pipe']});
    let stderr='';p.stderr.on('data',b=>stderr+=b);
    const exited=new Promise(r=>p.once('exit',(code,signal)=>{if(code)console.error('browser exit',code,signal,stderr.slice(-2000));r();}));
    const cdp=new Cdp(p.stdio[3],p.stdio[4]);
    try {
      const target=(await cdp.send('Target.getTargets')).targetInfos.find(t=>t.type==='page') ?? await cdp.send('Target.createTarget',{url:'about:blank'});
      const {sessionId}=await cdp.send('Target.attachToTarget',{targetId:target.targetId,flatten:true});
      const call=(method,params)=>cdp.send(method,params,sessionId);
      const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
      await call('Page.enable');await call('Page.bringToFront');await call('Runtime.enable');await call('Network.enable');await call('Network.setCacheDisabled',{cacheDisabled:!cache});
      await call('Page.addScriptToEvaluateOnNewDocument',{source:`
        Object.defineProperty(navigator,'gpu',{get:()=>undefined});
        new MutationObserver(()=>{if(!globalThis.tallyVisible && /Tick [1-9][0-9]*/.test(document.body?.textContent??''))globalThis.tallyVisible=performance.now();}).observe(document,{subtree:true,childList:true,characterData:true});
      `});
      for(let i=0;i<loads+(cache?1:0);i++) {
        await call('Page.navigate',{url});
        await new Promise(r=>setTimeout(r,30));
        console.log('load',mode,cache,i,p.pid);
        const until=Date.now()+3000;
        while(Date.now()<until) {
          const done=await evaluate(`location.href===${JSON.stringify(url)} && Boolean(globalThis.tallyVisible || (globalThis.exact?.root?.dataset.worldModuleMs && ${JSON.stringify(mode)}==='before'))`).catch(()=>false);
          if(done)break;
          await new Promise(r=>setTimeout(r,25));
        }
        const row=await evaluate(`(()=>{const w=exact.gpu?.decorate({op:'state'}, {})?.world?.[0];return {fcp:performance.getEntriesByName('first-contentful-paint')[0]?.startTime??null,module:Number(exact.root.dataset.worldModuleMs)||null,bound:w?.perf?.boundMs??null,tick:w?.perf?.firstTickMs??null,published:globalThis.tallyVisible??null};})()`);
        if(!cache||i>0) { records.push({mode,cache:cache?'warm':'cold',...row}); writeFileSync(resolve(scratch,'web-partial.json'),JSON.stringify(records,null,2)); }
        await call('Page.navigate',{url:'about:blank'});
      }
    } finally {try{process.kill(-p.pid,'SIGKILL');}catch{}await exited;rmSync(profile,{recursive:true,force:true});}
  }
  try {
    for(mode of ['before','after']) {for(let i=0;i<10;i++)await browser(false,1);await browser(true,10);console.log(`measured ${mode}`);}
    const summary=[];
    for(const mode of ['before','after'])for(const cache of ['cold','warm']){const rows=records.filter(r=>r.mode===mode&&r.cache===cache);summary.push({mode,cache,...Object.fromEntries(['fcp','module','bound','tick','published'].map(k=>[k,rows.every(r=>r[k]!=null)?median(rows.map(r=>r[k])):null]))});}
    writeFileSync(resolve(scratch,'web.json'),JSON.stringify({summary,records},null,2));console.log(JSON.stringify(summary,null,2));
  } finally {server.close();}
}
