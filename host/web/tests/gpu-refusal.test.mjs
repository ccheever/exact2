import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
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
  const {cdpBrowser} = await import('../../../scripts/agent-launch.mjs');
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
    Object.defineProperty(Navigator.prototype,'gpu',{get(){return {requestAdapter:async()=>({info:{}})};},configurable:true}); // the stub module's adapter
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
  const suite=await cdpBrowser(),{cdp}=suite, hostLines=[];
  let listener;
  try {
    const {sessionId}=await suite.page();
    const call=(method,params={})=>cdp.send(method,params,sessionId);
    cdp.listeners.push(listener = msg => { if(msg.sessionId===sessionId && msg.method==='Runtime.consoleAPICalled')hostLines.push(msg.params.args.map(a=>a.value??a.description).join(' ')); });
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
    if(listener)cdp.listeners.splice(cdp.listeners.indexOf(listener),1);
    await suite.close();server.stop(true);rmSync(profile,{recursive:true,force:true});
  }
},120000);


test('a Rust-edit candidate trap leaves the committed world driveable and skips candidate cleanup', async () => {
  const output=mkdtempSync('/tmp/gpu-candidate-');
  let bytes;
  try {
    const child=Bun.spawn(['rustc',resolve(import.meta.dir,'../../../game/engine/tests/fixtures/gpu-borrow-panic.rs'),'--crate-type','cdylib','--target','wasm32-unknown-unknown','-C','panic=abort','-o',resolve(output,'candidate.wasm')],{stderr:'pipe'});
    const [code,errors]=await Promise.all([child.exited,new Response(child.stderr).text()]);expect(code,errors).toBe(0);bytes=readFileSync(resolve(output,'candidate.wasm'));
  } finally {rmSync(output,{recursive:true,force:true});}
  const wasm=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{panic_log(){},world_borrowed(){}}});
  let candidateCalls=0;
  const f=await fixture({nextGpu:{gpu_render(){candidateCalls++;wasm.exports.trap_borrowed_world();},gpu_destroy(){candidateCalls++;},gpu_unload(){candidateCalls++;}}});
  f.create(1);await f.exact.gpu.settled();
  await expect(f.exact.gpu.swap(1)).rejects.toThrow(/unreachable/i);
  expect(candidateCalls).toBe(1);
  expect(f.exact.gpu.agent(1,{op:'state'}).world.tick).toBe(0);
  await f.exact.gpu.settled();
  expect(f.exact.gpu.agent(1,{op:'state'}).world.tick).toBe(0);
  expect(f.order.includes('old destroy') || f.order.includes('old unload') || f.order.includes('replace')).toBe(false);
});

// LLP 1139.003 D1 in a real browser with no WebGPU: `install` is how the page's install data answers.
async function noWebGPU(install, run) {
  const {cdpBrowser} = await import('../../../scripts/agent-launch.mjs');
  const web = resolve(import.meta.dir,'..');
  // On one game page (its web build names its world surfaces on the root): a world with an in-flow label and
  // an overlaid button, as game/games/placement-fixture has; and a decorative surface with overlaid controls.
  const html = `<button id="menu">Menu</button>
    <div id="host" style="width:300px;height:200px"><canvas data-surface></canvas><p id="label">Pull the lever</p><button id="hud" style="position:absolute;top:0">Play</button></div>
    <div id="sky" style="width:300px;height:200px;position:relative"><canvas data-surface></canvas><div id="content" style="position:absolute;inset:0"><button id="change">Change station</button></div></div><script type="module">
    Object.defineProperty(Navigator.prototype,'gpu',{get(){return undefined;},configurable:true});
    document.documentElement.setAttribute('data-exact-worlds','world arena');
    const host=document.getElementById('host');
    globalThis.exact={views:new Map([[1,host],[2,document.getElementById('sky')]]),root:document.body,devAssets:[],generation:0,pageFacts:{agent:false,read:()=>({}),tell(){},changed(){}},
      wasm:{exact_surface_record:()=>0},writeIn:x=>x,send(){}};
    await import('/gpu-glue.js'); exact.gpu.surface(1,'world',[]);
    const started=performance.now(); await exact.gpu.settled(); globalThis.settleMs=performance.now()-started;
    exact.gpu.surface(2,'glass',[]); await exact.gpu.settled();
    globalThis.ready=true;
  </script>`;
  const server=Bun.serve({port:0,hostname:'127.0.0.1',fetch(req){
    const path=new URL(req.url).pathname;
    if(path==='/')return new Response(html,{headers:{'content-type':'text/html'}});
    if(path==='/gpu.js'){globalThis.moduleFetched=true;return new Response('export default function(){}',{headers:{'content-type':'text/javascript'}});}
    if(path==='/.exact/install.json')return install();
    if(path==='/favicon.ico')return new Response('',{status:404});
    return new Response(Bun.file(resolve(web,path.slice(1))));
  }});
  const suite=await cdpBrowser(),{cdp}=suite, lines=[];
  let listener;
  try {
    const {sessionId}=await suite.page();
    const call=(method,params={})=>cdp.send(method,params,sessionId);
    cdp.listeners.push(listener = msg => { if(msg.sessionId===sessionId && msg.method==='Runtime.consoleAPICalled')lines.push(msg.params.args.map(a=>a.value??a.description).join(' ')); });
    await call('Runtime.enable');
    const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
    await call('Page.navigate',{url:`http://127.0.0.1:${server.port}/`});
    const deadline=Date.now()+30000;
    while(!await evaluate('globalThis.ready === true')) {if(Date.now()>deadline)throw new Error('fixture did not boot: '+lines.join('\n'));await Bun.sleep(50);}
    await run(evaluate, lines);
  } finally {
    if(listener)cdp.listeners.splice(cdp.listeners.indexOf(listener),1);
    await suite.close();server.stop(true);
  }
}

test('a browser without WebGPU shows the refusal page in a world\'s box and refuses world requests by type', () => noWebGPU(
  () => Response.json({platforms:{ios:{methods:[{kind:'app-store',url:'https://apps.example/game',label:'View in the App Store'}]},macos:{methods:[]}}}),
  async (evaluate, lines) => {
    const page=await evaluate(`(() => { const s=document.querySelector('#host > [role=status]'), r=s.getBoundingClientRect(), h=document.getElementById('host').getBoundingClientRect();
      return {text:s.textContent, box:[r.x,r.y,r.width,r.height], host:[h.x,h.y,h.width,h.height], label:document.getElementById('label').inert, hud:document.getElementById('hud').inert, menu:document.getElementById('menu').inert}; })()`);
    expect(page.text).toContain('needs WebGPU');
    expect(page.text).toContain('Chrome 121 or later on Android 12');
    expect(page.box).toEqual(page.host);
    expect([page.label, page.hud, page.menu]).toEqual([true, true, false]);
    const refused=await evaluate(`exact.gpu.handle({op:'state',id:1,world:true}, null, x => x)`);
    expect(refused.refused).toMatchObject({host:'web',subsystem:'gpu'});
    expect(refused.refused.message).toContain('no adapter');
    // The agent's settle waited for the install links, so `tree` names them.
    const tree=await evaluate(`exact.gpu.decorate({op:'tree'},{nodes:[{id:1},{id:2}]})`);
    expect(tree.nodes[0].status).toContain('View in the App Store');
    // A decorative surface on the same game page shows no page; its controls stay live over the empty canvas.
    expect(tree.nodes[1].status).toBeUndefined();
    expect(await evaluate(`[document.querySelector('#sky > [role=status]'), document.getElementById('content').inert]`)).toEqual([null, false]);
    // A surface replaced on the same element takes its page with it: one page, its links there and live.
    await evaluate(`exact.gpu.surface(1,'arena',[]); exact.gpu.settled()`);
    expect(await evaluate(`(() => { const a = document.querySelector('#host > [role=status] a');
      return [document.querySelectorAll('#host > [role=status]').length, !!a, !!a && !a.closest('[inert]'), document.getElementById('label').inert]; })()`)).toEqual([1, true, true, true]);
    // The app's own updates while the page stands, as glue.js applies them: clearing the label's inertness does
    // not re-enable it under the page; the button made inert and the host's position are the app's afterwards.
    expect(await evaluate(`(async () => { const label = document.getElementById('label'), hud = document.getElementById('hud');
      label.authoredInert = false; label.inert = false; hud.authoredInert = true; hud.inert = true;
      document.getElementById('host').style.position = 'absolute';
      await new Promise(r => setTimeout(r, 0)); return label.inert; })()`)).toBe(true);
    // Destroyed, the page goes, and each child has the app's inertness and the host its position.
    expect(await evaluate(`exact.gpu.destroy(1), [document.querySelectorAll('#host > [role=status]').length, document.getElementById('label').inert, document.getElementById('hud').inert, document.getElementById('host').style.position]`)).toEqual([0, false, true, 'absolute']);
    expect(globalThis.moduleFetched).toBeUndefined();
    expect(lines.join('\n')).not.toContain('No available adapters');
  }), 120000);

test('install data that never answers delays no agent reply past its bound', () => noWebGPU(
  () => new Promise(() => {}),
  async evaluate => {
    expect(await evaluate('settleMs')).toBeLessThan(3000);
    const tree=await evaluate(`exact.gpu.decorate({op:'tree'},{nodes:[{id:1}]})`);
    expect(tree.nodes[0].status).toContain('needs WebGPU');
    expect(tree.nodes[0].status).not.toContain('App Store');
  }), 120000);

// The shell's root carries a game's world surfaces beside the manifest's language (host/web/build.mjs writes
// the language, then the worlds after the GPU build, by attribute); an app's root carries none.
test('a game shell names its world surfaces without losing its language', async () => {
  const {markWorlds, worldSurfaces} = await import('../../../scripts/app.mjs');
  const shell = readFileSync(resolve(import.meta.dir, '../index.html'), 'utf8');
  const lang = html => html.replace('<html lang="en">', '<html lang="fr">');
  expect(markWorlds(lang(shell), ['world', 'arena'])).toContain('<html data-exact-worlds="world arena" lang="fr">');
  expect(markWorlds(lang(shell), [])).toBe(lang(shell));
  const dir = mkdtempSync('/tmp/exact-worlds-');
  try {
    expect(worldSurfaces(dir)).toEqual([]);
    mkdirSync(resolve(dir, '.shells'));
    writeFileSync(resolve(dir, '.shells/surfaces.json'), JSON.stringify({world: [{name: 'seed', default: 0}]}));
    expect(worldSurfaces(dir)).toEqual(['world']);
  } finally { rmSync(dir, {recursive: true, force: true}); }
});
