import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {buildingServer, devPort} from './dev.mjs';
import {readStatus, startStatus, waitReady} from './status.mjs';

test('HTTP answers during a wedged build; readiness belongs to the current invocation and generation', async () => {
  mkdirSync(resolve(import.meta.dir, '../target'), {recursive:true});
  const app = mkdtempSync(resolve(import.meta.dir, '../target/dev-test-'));
  const {port} = await devPort([], {start:29301});
  const gate = {check:() => ({allowed:true})};
  const early = await buildingServer({app, host:'127.0.0.1', port, origins:[{origin:`http://127.0.0.1:${port}`}], gate});
  try {
    early.status.set({phase:'build'});
    startStatus(app, 'prove');
    expect(readStatus(app).map(r => r.verb).sort()).toEqual(['prove','web']);
    expect(readStatus(app)[0].state).toBe('running');
    expect((await fetch(`http://127.0.0.1:${port}/`)).status).toBe(503);
    await expect(waitReady(app, {timeout:0.1})).rejects.toThrow('timed out');
    await expect(devPort(['--port', String(port)])).rejects.toThrow('in use');
    early.serve((req, res) => res.end('game'));
    expect((await waitReady(app, {timeout:1})).invocation).toBe(early.status.get().invocation);
    early.phase('contract','build');
    gate.check = () => { early.phase('contract','ready'); return {allowed:true}; };
    expect((await waitReady(app,{timeout:1})).phase).toBe('ready');
    gate.check = () => ({allowed:true});
    early.phase('contract','failed');
    await expect(waitReady(app, {timeout:1})).rejects.toThrow('build failed');
    early.status.set({state:'exited', exitCode:0});
    await expect(waitReady(app, {timeout:1, invocation:early.status.get().invocation})).rejects.toThrow('exited');
  } finally { early.server.closeAllConnections(); early.server.close(); /* exit handlers still write status */ }
});


test('dead lock owners recover and readiness ignores a previous exited invocation', async () => {
  const app = mkdtempSync(resolve(import.meta.dir, '../target/status-recovery-'));
  const lock = resolve(app,'.exact/status.lock');
  const child = Bun.spawn([process.execPath,'-e', `const fs=require('node:fs');fs.mkdirSync(${JSON.stringify(lock)},{recursive:true});fs.writeFileSync(${JSON.stringify(resolve(lock,'owner'))},String(process.pid));console.log('locked');setInterval(()=>{},1000);`], {stdout:'pipe'});
  const reader = child.stdout.getReader(); await reader.read(); child.kill('SIGKILL'); await child.exited;
  expect(existsSync(lock)).toBe(true);
  const prior = startStatus(app,'web'); prior.set({state:'exited'});
  expect(existsSync(lock)).toBe(false);
  const waiting = waitReady(app,{timeout:2});
  const {port} = await devPort([], {start:29401});
  const server = await buildingServer({app,host:'127.0.0.1',port,origins:[{origin:`http://127.0.0.1:${port}`}],gate:{check:()=>({allowed:true})}});
  try { server.serve((req,res)=>res.end('game')); expect((await waiting).invocation).toBe(server.status.get().invocation); }
  finally { server.server.closeAllConnections(); server.server.close(); }
});

test.skipIf(!process.env.EXACT_DEV_READINESS_APP)('Contract syntax failure and repair change web --ready through the real producer', async () => {
  const {readFileSync} = await import('node:fs');
  const app = resolve(process.env.EXACT_DEV_READINESS_APP), contract = resolve(app,'app.contract');
  const original = readFileSync(contract,'utf8'), {port} = await devPort([], {start:29501});
  const p = Bun.spawn([process.execPath,resolve(import.meta.dir,'dev.mjs'),app,'--port',String(port)], {stdout:'pipe',stderr:'pipe'});
  let log=''; const drain=async stream=>{for await(const bytes of stream)log+=new TextDecoder().decode(bytes);};
  const readers=[drain(p.stdout),drain(p.stderr)];
  const until=async predicate=>{const deadline=Date.now()+600000;while(!predicate()){if(p.exitCode!==null || Date.now()>deadline)throw new Error(log);await Bun.sleep(50);}};
  try {
    await until(()=>log.includes('Open in native:'));
    expect((await waitReady(app,{timeout:600})).phase).toBe('ready');
    writeFileSync(contract, original+'\nthis is not Contract (\n');
    await until(()=>readStatus(app).find(r=>r.pid===p.pid || r.verb==='web' && r.state==='running')?.phase==='failed');
    await expect(waitReady(app,{timeout:2})).rejects.toThrow('build failed');
    log=''; writeFileSync(contract,original);
    await until(()=>log.includes('plan ready') || log.includes('generation ready'));
    expect((await waitReady(app,{timeout:600})).phase).toBe('ready');
  } finally {
    writeFileSync(contract,original); p.kill('SIGTERM');await p.exited;await Promise.all(readers);
  }
},600000);


test('simultaneous dead-lock recovery preserves every invocation and completed proof duration', async () => {
  const app = mkdtempSync(resolve(import.meta.dir,'../target/status-contenders-'));
  const lock = resolve(app,'.exact/status.lock');
  const owner = Bun.spawn([process.execPath,'-e',`require('node:fs').mkdirSync(${JSON.stringify(lock)},{recursive:true});require('node:fs').writeFileSync(${JSON.stringify(resolve(lock,'owner'))},String(process.pid));console.log('ready');setInterval(()=>{},1000)`],{stdout:'pipe'});
  await owner.stdout.getReader().read(); owner.kill('SIGKILL'); await owner.exited;
  const gate = resolve(app,'go'), module = resolve(import.meta.dir,'status.mjs');
  const children = Array.from({length:8},()=>Bun.spawn([process.execPath,'-e',`const fs=require('node:fs'), read=fs.readFileSync;
    fs.readFileSync=(path,...args)=>{const value=read(path,...args);if(path===${JSON.stringify(resolve(lock,'owner'))} && value===${JSON.stringify(String(owner.pid))}) { fs.writeFileSync(${JSON.stringify(app)}+'/seen-'+process.pid,''); const deadline=Date.now()+5000;while(fs.readdirSync(${JSON.stringify(app)}).filter(n=>n.startsWith('seen-')).length<8){if(Date.now()>deadline)throw new Error('lock barrier timed out');Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,5);} } return value;};
    require('node:module').syncBuiltinESMExports();const {startStatus}=await import(${JSON.stringify(module)});while(!fs.existsSync(${JSON.stringify(gate)})) await Bun.sleep(5); startStatus(${JSON.stringify(app)},'prove');`],{stdout:'pipe',stderr:'pipe'}));
  writeFileSync(gate,'go');
  for (const child of children) { const [code,err] = await Promise.all([child.exited,new Response(child.stderr).text()]); expect(code,err).toBe(0); }
  const rows = readStatus(app);
  expect(rows).toHaveLength(8);
  expect(new Set(rows.map(row=>row.invocation)).size).toBe(8);
  expect(rows.every(row=>row.state==='exited' && typeof row.lastDurationMs==='number' && row.lastDurationMs>=0)).toBe(true);
  expect(startStatus(app,'prove').get().lastDurationMs).toBe(rows.at(-1).lastDurationMs);
});

test('TypeScript producer callback publishes success, reports failure and accepts repair', async () => {
  const {readFileSync} = await import('node:fs');
  const source = readFileSync(resolve(import.meta.dir,'../host/web/dev.mjs'),'utf8');
  const start = source.indexOf("  child.stdout.on('data', chunk => {");
  const callback = source.slice(start,source.indexOf("  child.on('error'",start));
  const phases=[], errors=[], publications=[];
  const run = new Function('early','console','publications',`
    const child={stdout:{on(_,fn){this.reply=fn}}}, dev=child, scrubAsks=new Map();
    let buffered='', active, moduleRun=0, moduleStage, currentModule, moduleDesign, moduleScrubbed, moduleReleased, designSnapshot, designSource, seq=0;
    const pending=new Map(), clients=new Set(), MODULE_FILES={web:'app.js'}, app={id:'test'}, tamber=null;
    const readFileSync=()=>new Uint8Array([1]), resolve=(...p)=>p.join('/'), captureAssets=()=>[], moduleCards=()=>{}, designBytes=()=> 'built-hash', contractBytes=()=> 'built-hash';
    let contractStamp, contractTreeStamp; const contractInputsStamp=()=> 'tree'; ${source.match(/function contractReady[^\n]+/)[0]}
    const captureGeneration=()=>publications.push(seq), push=()=>{}, announcement=()=>({}), watchPackages=()=>{}, rmSync=()=>{};
    ${callback}
    return (ok, design='built-hash')=>{active={id:++moduleRun,design,contract:design,output:'out',stage:'stage',started:Date.now()}; currentModule=null; child.stdout.reply(JSON.stringify({id:moduleRun,ok,error:ok?undefined:'bad TypeScript'})+'\\n');return contractStamp;};
  `)({phase:(producer,phase)=>phases.push([producer,phase])},{log(){},error:message=>errors.push(message)},publications);
  expect(run(true)).toBe('built-hash');
  run(false); run(true);
  expect(publications).toEqual([1,2]);
  expect(errors).toEqual(['bad TypeScript']);
  expect(phases.filter(([p])=>p==='typescript').map(([,v])=>v)).toEqual(['ready','failed','ready']);
  run(true,'older-compiled-hash');
  expect(phases.at(-1)).toEqual(['contract','build']);
});
