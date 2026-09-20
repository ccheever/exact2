#!/usr/bin/env bun
// Diagnostic, not a gate. Bake Tally normally; use the existing agent carrier.
import {spawnSync} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {gzipSync} from 'node:zlib';
import {open} from '../../scripts/agent.mjs';
import {closeFilesystemReader} from '../../scripts/filesystem.mjs';
import {serveStatic} from '../../host/web/serve.mjs';
const root=resolve(import.meta.dir,'../..');
const scratch=resolve(process.env.K6_SCRATCH ?? process.env.K4_SCRATCH ?? process.env.K3_SCRATCH ?? `${process.env.HOME}/lanes/gamenext/scratch/k6`);
const app=resolve(root,'game/games/tally'), dist=resolve(app,'dist');
mkdirSync(scratch,{recursive:true});
const stats=values=>{const a=[...values].sort((a,b)=>a-b), mid=a.length/2;return {median:(a[Math.floor(mid)]+a[Math.ceil(mid)-1])/2,p95:a[Math.ceil(a.length*.95)-1]};};
const summarize=rows=>Object.fromEntries(Object.keys(rows[0]).map(key=>[key,stats(rows.map(row=>row[key]))]));
const save=(name,value)=>writeFileSync(resolve(scratch,`${name}.json`),JSON.stringify(value,null,2)+'\n');
if(process.argv.includes('--sizes')) {
  const result=Object.fromEntries(['app.wasm','gpu_bg.wasm','gpu.js'].map(name=>{
    const b=readFileSync(resolve(dist,name));return [name,{raw:b.length,gzip:gzipSync(b,{level:9}).length}];
  }));save('sizes',result);console.log(JSON.stringify(result,null,2));
}
if(process.argv.includes('--linux')) {
  const target=process.env.CARGO_TARGET_DIR ?? resolve(root,'game/target');
  const results={};
  for(const name of ['tally','caltrain']) {
    const binary=process.env[name==='tally'?'TALLY_LINUX':'CALTRAIN_LINUX'] ?? resolve(target,`x86_64-unknown-linux-gnu/gpu-dev/${name}-linux`);
    const rows=[];
    for(let i=0;i<20;i++) {
      const p=spawnSync(binary,[],{env:{...process.env,EXACT_AGENT:'1',EXACT_WORLD_TIMING:'1',EXACT_UPDATE_TRUST:'development',EXACT_ASSETS:resolve(root,name==='tally'?'game/games/tally':'apps/caltrain')},input:'',encoding:'utf8'});
      if(p.status!==0)throw Error(p.stderr || p.error?.message);
      const ready=JSON.parse(p.stdout.split('\n').find(line=>line.startsWith('{')) ?? 'null');
      if(!ready?.ready || ready.error || /not a loadable source/.test(p.stderr))throw Error('host did not boot completely: '+p.stdout+p.stderr);
      const row={boot_reported:ready.boot};let binding;
      for(const line of p.stderr.split('\n')) {
        if(line.startsWith('exact-world-startup: ')){const v=JSON.parse(line.slice(21));row[v.event]=v.ms;}
        if(line.startsWith('exact-world-binding: '))binding=JSON.parse(line.slice(21));
      }
      if(!row.first_frame)throw Error('missing first frame: '+p.stderr);
      if(name==='tally') {
        if(!binding || !row.first_publication)throw Error('missing world spans: '+p.stderr);
        row.bind_work=binding.bind_ms;row.tick_work=binding.first_tick_ms;
        // Binding takes the first tick. Its return is the observed completion,
        // including ABI parsing and trace writes; inner spans separate game work.
        row.first_tick=row.bind_done;
        row.world_work=row.bind_done-row.load_headless;
      }
      rows.push(row);
    }
    results[name]={rows,summary:summarize(rows)};
  }
  save('linux',results);console.log(JSON.stringify(Object.fromEntries(Object.entries(results).map(([k,v])=>[k,v.summary])),null,2));
}
if(process.argv.includes('--web')) {
  Object.assign(process.env,{EXACT_APP_DIR:app,EXACT_WEB_DIST:dist});
  const compare=process.argv.includes('--compare-delay'), preload=process.argv.includes('--compare-preload');
  const modes=preload?['preload','serial']:compare?['immediate','delayed']:['immediate'];
  let mode=modes[0];
  const index=readFileSync(resolve(dist,'index.html'),'utf8');
  if(!index.includes('data-device-free-surfaces'))throw Error('bake Tally with the device-free startup marker first');
  // Only the declared loading policy varies; every wasm and JS byte is identical.
  const server=createServer((req,res)=>{
    if(new URL(req.url,'http://local').pathname==='/') {
      res.writeHead(200,{'content-type':'text/html','cache-control':'no-store'});
      res.end(mode==='serial'?index.replace(/<link rel="(?:modulepreload|preload)"[^>]+href="(?:app\.wasm|gpu(?:_bg\.wasm|\.js))"[^>]*>\n/g,''):mode==='delayed'?index.replace(' data-device-free-surfaces',''):index);return;
    }
    serveStatic(dist,req,res);
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const url=`http://127.0.0.1:${server.address().port}/`, children=[],rows=[];
  const launch=()=>open({host:'web',url,onProcess:child=>{children.push(child);console.log('browser PID',child.pid);}});
  async function sample(session,cache,index) {
    // Passive grace: do not let a state request pull the lazy module ahead of paint.
    await new Promise(r=>setTimeout(r,200));
    const perf=(await session.state()).world?.[0]?.perf;
    if (perf?.resourceTransfers) perf.resources=perf.resources.map(row=>({...row,...perf.resourceTransfers.find(t=>t.name===row.name)}));
    for(const key of ['navigationToFirstContentfulPaintMs','moduleInstantiatedMs','boundMs','firstTickMs','firstPublicationMs']) {
      if(!Number.isFinite(perf?.[key]))throw Error('missing startup marker '+key);
    }
    for(const name of ['/app.wasm','/gpu.js','/gpu_bg.wasm']) {
      const resources=perf.resources.filter(r=>r.name===name);
      if(resources.length!==1 || !(resources[0].transferBytes>0))throw Error('expected ONE transfer for '+name+': '+JSON.stringify(resources));
    }
    rows.push({mode,cache,index,...perf});save('web-partial',rows);
    console.log(mode,cache,index,perf.navigationToFirstContentfulPaintMs,perf.firstPublicationMs);
  }
  try {
    for(let i=0;i<10;i++)for(mode of (i%2?modes:[...modes].reverse())) {
      const s=await launch();try{await sample(s,'cold',i);}finally{await s.close();}
    }
    for(mode of (preload?modes:['immediate'])) {
      const s=await launch();
      try {
        await sample(s,'prime',0);
        for(let i=0;i<10;i++){await s.carrier.reset({warm:true});await sample(s,'warm',i);}
      } finally {await s.close();}
    }
    const markers=['navigationToFirstContentfulPaintMs','moduleInstantiatedMs','boundMs','firstTickMs','firstPublicationMs'];
    const summary=[];
    for(const mode of modes)for(const cache of ['cold','warm']) {
      const samples=rows.filter(r=>r.mode===mode&&r.cache===cache);if(!samples.length)continue;
      summary.push({mode,cache,markers:summarize(samples.map(r=>Object.fromEntries(markers.map(k=>[k,r[k]])))),resources:Object.fromEntries(['/app.wasm','/gpu.js','/gpu_bg.wasm'].map(name=>[name,summarize(samples.map(r=>{const v=r.resources.find(x=>x.name===name);return {transferBytes:v.transferBytes,encodedBytes:v.encodedBytes,bytes:v.bytes,durationMs:v.durationMs};}))]))});
    }
    save('web',{summary,rows});console.log(JSON.stringify(summary,null,2));
  } finally {
    server.close();
    closeFilesystemReader();
    // Carrier close SIGKILLs its recorded group and awaits the browser's exit.
    const leaked=children.filter(c=>c.exitCode===null&&c.signalCode===null).map(c=>c.pid);
    save('web-processes',{recorded:children.map(c=>c.pid),leaked});
    if(leaked.length)throw Error('leaked browser PIDs: '+leaked);
    console.log('leaked browser PIDs: none');
  }
}
