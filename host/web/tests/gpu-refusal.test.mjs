import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, readFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';
import {fixture} from './surface.mjs';
import {open} from '../../../scripts/agent.mjs';
import {launchFacts} from '../../../scripts/agent-launch.mjs';

test('a wasm trap while the world is borrowed leaves host diagnostics readable without reentering it', async () => {
  const f = await fixture(); f.create(1); await f.exact.gpu.settled();
  const target = resolve(import.meta.dir,'../../../target');
  mkdirSync(target,{recursive:true});
  const output = mkdtempSync(resolve(target,'borrow-panic-'));
  let bytes;
  try {
    const child = Bun.spawn(['rustc',resolve(import.meta.dir,'../../../game/engine/tests/fixtures/gpu-borrow-panic.rs'),
      '--crate-type','cdylib','--target','wasm32-unknown-unknown','-C','panic=abort',
      '-o',resolve(output,'fixture.wasm')], {stdout:'pipe',stderr:'pipe'});
    const [code, diagnostic] = await Promise.all([child.exited,new Response(child.stderr).text()]);
    expect(code,diagnostic).toBe(0);
    bytes = readFileSync(resolve(output,'fixture.wasm'));
  } finally { rmSync(output,{recursive:true,force:true}); }
  let calls = 0, borrowed = false;
  const wasm = new WebAssembly.Instance(new WebAssembly.Module(bytes), {env:{
    panic_log(pointer,length) { f.diagnostics.push(new TextDecoder().decode(new Uint8Array(wasm.exports.memory.buffer,pointer,length))); },
    world_borrowed() { borrowed = true; },
  }});
  f.gpu.gpu_agent = () => { calls++; wasm.exports.trap_borrowed_world(); };
  const refused = f.exact.gpu.handle({op:'state',id:1,world:true}, null, x => x);
  expect(refused.refused.host).toBe('web');
  expect(refused.refused.subsystem).toBe('gpu');
  expect(borrowed).toBe(true);
  const logs = f.exact.gpu.decorate({op:'logs',all:true},{lines:[],from:0,next:0});
  expect(logs.world[0].lines.join(' ')).toMatch(/unreachable/i);
  expect(calls).toBe(1);
  const carrier = {host:'web',browser:'chrome',launchFacts:launchFacts({}),reset(){},hostLines:f.diagnostics,
    ask:async () => { throw new WebAssembly.RuntimeError('unreachable'); }};
  const session = await open({reuse:carrier});
  expect((await session.logs()).host.join('\n')).toContain('borrowed world fixture');
  expect((await session.logs({all:true})).host.join('\n')).toContain('borrowed world fixture');
  expect((await session.logs()).host).toEqual([]);
}, 60000);

test('browser agent settling retains render traps and quarantines all later module entries', async () => {
  const {spawn} = await import('node:child_process');
  const {chromium} = await import('../../../scripts/agent-launch.mjs');
  const {Cdp} = await import('../../../scripts/agent.mjs');
  const {glueDeclarations} = await import('./glue-fixture.mjs');
  const web = resolve(import.meta.dir,'..'), profile = mkdtempSync('/tmp/gpu-refusal-browser-');
  const compiled = Bun.spawn(['rustc',resolve(web,'../../game/engine/tests/fixtures/gpu-borrow-panic.rs'),'--crate-type','cdylib','--target','wasm32-unknown-unknown','-C','panic=abort','-o',resolve(profile,'panic.wasm')],{stderr:'pipe'});
  const [code,errors] = await Promise.all([compiled.exited,new Response(compiled.stderr).text()]);
  expect(code,errors).toBe(0);
  const adapter = glueDeclarations(readFileSync(resolve(web,'glue.js'),'utf8'), ['agent','agentNow','agentReply','settleGpu','gpuPendingReply']);
  const moduleSource = `
    const wasm = await WebAssembly.instantiate(await (await fetch('/panic.wasm')).arrayBuffer(), {env:{panic_log(p,n){console.error(new TextDecoder().decode(new Uint8Array(wasm.instance.exports.memory.buffer,p,n)));},world_borrowed(){globalThis.borrowed=true;}}});
    const enter = (render = false) => { globalThis.entries++; if(render && globalThis.armed) wasm.instance.exports.trap_borrowed_world(); };
    export default function() {}
    export function gpu_load() {} export function gpu_unload() {enter();}
    export function gpu_child_view() {} export function gpu_seekable() {} export function gpu_period() {}
    export function gpu_shader_names(){return '[]';} export function gpu_shaders_clear() {}
    export function gpu_create(){return 1;} export function gpu_bind_at(){return true;}
    export function gpu_wants_input(){return false;} export function gpu_dirty(){return globalThis.armed;}
    export function gpu_render(){enter(true);return 0;} export function gpu_flush(){enter();return true;}
    export function gpu_published() {} export function gpu_messages() {} export function gpu_lifecycle(){enter();}
    export function gpu_assets(){enter();return '{"requests":[],"retired":[]}';}
    export function gpu_agent(){enter();return globalThis.badReply ? '{"error":"unknown entity"}' : '{"world":{"tick":0},"lines":[],"from":0,"next":0}';}
  `;
  const html = `<div id="host"><canvas data-surface></canvas></div><script type="module">
    globalThis.entries=0; globalThis.armed=false;
    const host=document.getElementById('host');
    globalThis.exact={views:new Map([[1,host]]),root:document.body,now:()=>0,devAssets:[],generation:0};
    await import('/gpu-glue.js'); exact.gpu.surface(1,'world',[]); await exact.gpu.settled();
    const stageLoaded=()=>true, viewBox=()=>{}, agentMode=true, gpuInPlay=()=>true, loadGpuIfNeeded=()=>{}, gpuLoading=null;
    const wasm={}, ask=()=>({lines:[],from:0,next:0}), tagged=x=>x;
    ${adapter}
    globalThis.askAgent=agent; globalThis.ready=true;
  </script>`;
  const server=Bun.serve({port:0,hostname:'127.0.0.1',fetch(req){
    const path=new URL(req.url).pathname;
    if(path==='/favicon.ico')return new Response('',{status:404});
    if(path==='/')return new Response(html,{headers:{'content-type':'text/html'}});
    if(path==='/gpu.js')return new Response(moduleSource,{headers:{'content-type':'text/javascript'}});
    return new Response(Bun.file(path==='/panic.wasm'?resolve(profile,'panic.wasm'):path==='/gpu-glue.js' && process.env.E2B_GPU_SOURCE ? process.env.E2B_GPU_SOURCE : resolve(web,path.slice(1))));
  }});
  const child=spawn(chromium().executable,['--headless=new','--remote-debugging-pipe','--no-sandbox','--no-first-run',`--user-data-dir=${profile}`,'about:blank'],{stdio:['ignore','ignore','ignore','pipe','pipe']});
  const exited=new Promise(ok=>child.once('exit',ok));
  const cdp=new Cdp(child.stdio[3],child.stdio[4]), hostLines=[];
  try {
    const {targetId}=await cdp.send('Target.createTarget',{url:'about:blank'});
    const {sessionId}=await cdp.send('Target.attachToTarget',{targetId,flatten:true});
    const call=(method,params={})=>cdp.send(method,params,sessionId);
    cdp.listeners.push(msg => { if(msg.method==='Runtime.consoleAPICalled')hostLines.push(msg.params.args.map(a=>a.value??a.description).join(' ')); });
    await call('Runtime.enable');
    const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
    await call('Page.navigate',{url:`http://127.0.0.1:${server.port}/`});
    const deadline=Date.now()+30000;
    while(!await evaluate('globalThis.ready === true')) {if(Date.now()>deadline)throw new Error('browser fixture did not boot: '+hostLines.join('\n'));await Bun.sleep(50);}
    await evaluate('globalThis.badReply=true');
    expect(await evaluate('askAgent({op:"state",id:1,world:true})')).toEqual({error:'unknown entity'});
    await evaluate('globalThis.badReply=false; globalThis.armed=true');
    const refused=await evaluate('askAgent({op:"state",id:1,world:true})');
    expect(refused.refused).toMatchObject({host:'web',subsystem:'gpu'});
    expect(await evaluate('borrowed')).toBe(true);
    const entries=await evaluate('entries');
    const logs=await evaluate('askAgent({op:"logs",all:true})');
    expect(logs.world[0].lines.join(' ')).toContain('unreachable');
    await evaluate('document.dispatchEvent(new Event("visibilitychange")); exact.gpu.schedule()');
    await evaluate('askAgent({op:"state",id:1,world:true})');
    expect(await evaluate('entries')).toBe(entries);
    expect(hostLines.join('\n')).toContain('borrowed world fixture');
  } finally {
    await cdp.send('Browser.close',{},undefined,2000).catch(()=>{});
    if(child.exitCode===null && child.signalCode===null)child.kill('SIGKILL');
    await exited;server.stop(true);rmSync(profile,{recursive:true,force:true});
  }
},120000);
