import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, existsSync, readFileSync, rmSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {resolve} from 'node:path';
import {buildingServer, devPort} from './dev.mjs';
import {readStatus, startStatus, waitReady} from './status.mjs';

// Loaded only in subprocesses: keep the real launchers and substitute the two
// expensive startup commands with a build whose descendant ignores cancellation.
async function shutdownFixture(root, dir, stage, mode) {
  const {mock} = await import('bun:test');
  const {appendFileSync} = await import('node:fs');
  const cp = await import('node:child_process'), spawn = cp.spawn;
  const apps = await import(root + '/scripts/app.mjs');
  mock.module(root + '/scripts/app.mjs', () => ({...apps, resolveApp:() => ({
    dir, target:dir, workspace:dir, name:'shutdown', manifest:mode === 'game' ? {game:{}} : {}, crate:kind => 'shutdown-' + kind,
  })}));
  const record = (role, pid) => appendFileSync(dir + '/pids', role + ' ' + pid + '\n');
  const leaf = `const fs=require('node:fs');
    process.on('SIGTERM',()=>{}); process.on('SIGHUP',()=>{});
    fs.appendFileSync(${JSON.stringify(dir + '/pids')},'leaf '+process.pid+'\\n');
    fs.writeFileSync(${JSON.stringify(dir + '/ready')},'ready'); setInterval(()=>{},1000);`;
  const slow = `const fs=require('node:fs');
    for(const signal of ['SIGTERM','SIGHUP']) process.on(signal,()=>{fs.writeFileSync(${JSON.stringify(dir + '/cancelled')},signal); ${stage === 'shells-exit' ? 'process.exit(0);' : ''}});
    require('node:child_process').spawn(process.execPath,['-e',${JSON.stringify(leaf)}],{stdio:'inherit'});
    setInterval(()=>{},1000);`;
  mock.module('node:child_process', () => ({...cp, spawn(command, args, options) {
    let role;
    if (args[0] === root + '/host/web/dev.mjs') {
      role = 'resident'; args = ['--preload', dir + '/fixture.mjs', ...args];
    } else if (args[0] === root + '/game/app/shells.mjs' || args[0] === root + '/host/web/build.mjs') {
      role = args[0].endsWith('/shells.mjs') ? 'shells' : 'build';
      args = ['-e', role === stage || stage === 'shells-exit' && role === 'shells' ? slow : 'process.exit(0)'];
    } else throw new Error('unexpected startup command: ' + command + ' ' + args.join(' '));
    const child = spawn(command, args, options); record(role, child.pid); return child;
  }}));
}

function running(pid) {
  try { process.kill(pid, 0); } catch { return false; }
  // An orphan's zombie can await init in a container, but cannot keep building.
  const state = spawnSync('ps', ['-o', 'stat=', '-p', String(pid)], {encoding:'utf8'}).stdout.trim();
  return state !== '' && !state.startsWith('Z');
}

async function shutdownUntil(predicate, description, timeout = 5000) {
  const deadline = Date.now() + timeout;
  while (!predicate()) {
    if (Date.now() >= deadline) throw new Error(description);
    await Bun.sleep(50);
  }
}

const shutdownCases = [
  ...['shells', 'build'].flatMap(stage => ['SIGTERM', 'SIGHUP', 'repeated'].map(signal => ({stage, signal, mode:'game'}))),
  ...['wasm', 'producer'].map(mode => ({stage:'build', signal:'SIGTERM', mode})),
  {stage:'shells-exit', signal:'SIGTERM', mode:'game'},
];
for (const {stage, signal, mode} of shutdownCases) {
  test(`${mode} shutdown during ${stage} reaps the build tree after ${signal}`, async () => {
    const root = resolve(import.meta.dir, '..');
    mkdirSync(resolve(root, 'target'), {recursive:true});
    const dir = mkdtempSync(resolve(root, 'target/dev-shutdown-'));
    writeFileSync(resolve(dir, 'app.contract'), '');
    writeFileSync(resolve(dir, 'fixture.mjs'), `await (${shutdownFixture.toString()})(${JSON.stringify(root)},${JSON.stringify(dir)},${JSON.stringify(stage)},${JSON.stringify(mode)});`);
    const {port} = await devPort([], {start:29601});
    const entry = mode === 'game' ? [resolve(root, 'game/dev.mjs'), dir] : [resolve(root, 'host/web/dev.mjs'), ...(mode === 'wasm' ? ['--wasm'] : ['--serve-as', String(port + 1)])];
    const child = Bun.spawn([process.execPath, '--preload', resolve(dir, 'fixture.mjs'), ...entry, '--port', String(port)], {
      env:{...process.env, EXACT_WEB_DIST:resolve(dir, 'dist')}, stdout:'pipe', stderr:'pipe',
    });
    let exited = false;
    const exit = child.exited.then(code => { exited = true; return code; });
    let log = '';
    const drain = async stream => { for await (const bytes of stream) log += new TextDecoder().decode(bytes); };
    const readers = [drain(child.stdout), drain(child.stderr)];
    const pids = () => existsSync(resolve(dir, 'pids')) ? readFileSync(resolve(dir, 'pids'), 'utf8').trim().split('\n').map(line => Number(line.split(' ')[1])) : [];
    try {
      await shutdownUntil(() => existsSync(resolve(dir, 'ready')) || exited, 'build did not become ready');
      expect(exited, log).toBe(false);
      child.kill(signal === 'repeated' ? 'SIGTERM' : signal);
      if (signal === 'repeated') {
        await shutdownUntil(() => existsSync(resolve(dir, 'cancelled')) || exited, 'build did not receive cancellation');
        expect(exited, log).toBe(false);
        child.kill('SIGHUP');
        child.kill('SIGTERM');
      }
      await shutdownUntil(() => exited, 'dev launcher did not exit');
      await shutdownUntil(() => pids().every(pid => !running(pid)), 'startup descendants survived shutdown');
      expect(await exit, log).toBe(0);
      const roles = readFileSync(resolve(dir, 'pids'), 'utf8').trim().split('\n').map(line => line.split(' ')[0]).sort();
      expect(roles).toEqual(mode !== 'game' ? ['build', 'leaf'] : stage.startsWith('shells') ? ['leaf', 'resident', 'shells'] : ['build', 'leaf', 'resident', 'shells']);
    } finally {
      for (const pid of [child.pid, ...pids()]) if (running(pid)) { try { process.kill(pid, 'SIGKILL'); } catch {} }
      await shutdownUntil(() => exited && pids().every(pid => !running(pid)), 'fixture cleanup timed out');
      await Promise.all(readers);
      rmSync(dir, {recursive:true, force:true});
    }
  }, 15000);
}

test('port retries keep one cancellation handler and never retry after cancellation', async () => {
  const root = resolve(import.meta.dir, '..');
  mkdirSync(resolve(root, 'target'), {recursive:true});
  const dir = mkdtempSync(resolve(root, 'target/dev-retry-'));
  writeFileSync(resolve(dir, 'app.contract'), '');
  const fixture = async (root, dir) => {
    const {mock} = await import('bun:test');
    const {appendFileSync} = await import('node:fs');
    const cp = await import('node:child_process'), spawn = cp.spawn;
    let attempt = 0;
    mock.module('node:child_process', () => ({...cp, spawn(command, args, options) {
      if (args[0] !== root + '/host/web/dev.mjs') throw new Error('unexpected spawn');
      const code = ++attempt < 3 ? 'process.exit(1)' : `
        const fs=require('node:fs'); let signals=0;
        process.on('SIGTERM',()=>{fs.writeFileSync(${JSON.stringify(dir + '/cancelled')},'yes'); if(++signals===2)process.exit(1);});
        fs.writeFileSync(${JSON.stringify(dir + '/ready')},'yes'); setInterval(()=>{},1000);`;
      const child = spawn(command, ['-e', `console.error('cannot listen on 127.0.0.1:8765: EADDRINUSE'); ${code}`], options);
      appendFileSync(dir + '/pids', child.pid + '\n'); return child;
    }}));
  };
  writeFileSync(resolve(dir, 'fixture.mjs'), `await (${fixture.toString()})(${JSON.stringify(root)},${JSON.stringify(dir)});`);
  const child = Bun.spawn([process.execPath, '--preload', resolve(dir, 'fixture.mjs'), resolve(root, 'game/dev.mjs'), dir], {stdout:'pipe', stderr:'pipe'});
  let exited = false;
  const exit = child.exited.then(code => { exited = true; return code; });
  const output = Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
  const pids = () => existsSync(resolve(dir, 'pids')) ? readFileSync(resolve(dir, 'pids'), 'utf8').trim().split('\n').map(Number) : [];
  try {
    await shutdownUntil(() => existsSync(resolve(dir, 'ready')) || exited, 'third port attempt did not start');
    expect(exited).toBe(false);
    child.kill('SIGTERM');
    await shutdownUntil(() => existsSync(resolve(dir, 'cancelled')) || exited, 'first signal was not forwarded');
    expect(exited).toBe(false);
    child.kill('SIGTERM');
    await shutdownUntil(() => exited && pids().every(pid => !running(pid)), 'port retry did not stop');
    expect(await exit, (await output).join('')).toBe(1);
    expect(pids()).toHaveLength(3);
  } finally {
    for (const pid of [child.pid, ...pids()]) if (running(pid)) { try { process.kill(pid, 'SIGKILL'); } catch {} }
    await shutdownUntil(() => exited && pids().every(pid => !running(pid)), 'port retry cleanup timed out');
    await output;
    rmSync(dir, {recursive:true, force:true});
  }
}, 15000);

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
  mkdirSync(resolve(import.meta.dir, '../target'), {recursive:true});
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
  mkdirSync(resolve(import.meta.dir, '../target'), {recursive:true});
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
    const captureGeneration=()=>publications.push(seq), push=()=>{}, announcement=()=>({}), watchPackages=()=>{}, rmSync=()=>{}, sayReady=line=>console.log(line);
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
