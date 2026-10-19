import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {buildingServer, devPort} from './dev.mjs';
import {readStatus, startStatus, waitReady} from './status.mjs';

test('HTTP answers during a wedged build; readiness belongs to the current invocation and generation', async () => {
  mkdirSync(resolve(import.meta.dir, '../target'), {recursive:true});
  const app = mkdtempSync(resolve(import.meta.dir, '../target/dev-test-'));
  const {port} = await devPort([], {start:29301});
  const early = await buildingServer({app, host:'127.0.0.1', port, origins:[{origin:`http://127.0.0.1:${port}`}], gate:{check:() => ({allowed:true})}});
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
},1500000);
