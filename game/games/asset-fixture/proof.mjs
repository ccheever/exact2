#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { residencyProbe, checkResidency } from './residency.mjs';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

await proof(import.meta, async ({open,check,out,say,host}) => {
  const web = host === 'web';
  const residency = residencyProbe('crate.model','crate/0-srgb-straight.tex');
  let gate, server, fail = false, lossDone;
  let textureRequests = 0;
  const next = () => {
    let release, observed, requested;
    gate = {delivery:new Promise(r=>release=r), probe:new Promise(r=>observed=r), request:new Promise(r=>requested=r), release, observed, requested};
    return gate;
  };
  if (web) server = Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request) {
    const reply = await residency.fetch(request); if(reply) return reply;
    const path = residency.assetPath(decodeURIComponent(new URL(request.url).pathname));
    if(path === '/__loss-probe') { lossDone(await request.json()); return new Response('',{status:204}); }
    if(path.endsWith('.tex')) textureRequests++;
    if(path === '/__asset-probe') { gate.observed(await request.json()); return new Response('',{status:204}); }
    if(path === '/assets/crate.model') { gate.requested(); await gate.delivery; if(fail) return new Response('',{status:503}); }
    if(path.startsWith('/__')) return new Response('',{status:204});
    if(path.includes('..')) return new Response('',{status:404});
    const file = Bun.file(resolve(import.meta.dir,'dist',path==='/'?'index.html':path.slice(1)));
    const textureReply = await residency.textureResponse(path,file); if(textureReply) return textureReply;
    if(path === '/gpu-glue.js') {
      // Observe the actual surface before delivery, below the agent's mandatory
      // settlement barrier. This probe exists only in this proof's HTTP response.
      const source = `let fixtureDevice; const fixtureErrors=[];
      const fixtureRequest = GPUAdapter.prototype.requestDevice;
      GPUAdapter.prototype.requestDevice = async function(...args) {
        fixtureDevice = await fixtureRequest.apply(this, args); fixtureDevice.addEventListener('uncapturederror',e=>fixtureErrors.push(e.error.message)); return fixtureDevice;
      };
      document.addEventListener('keydown', async event => {
        if(event.code !== 'KeyL') return;
        try {
          fixtureDevice.destroy(); await fixtureDevice.lost;
          await new Promise(resolve=>setTimeout(resolve, 0));
          for(const entry of surfaces.values()) render(entry, 0);
          await recoveringDevice; await settled();
          for(const entry of surfaces.values()) render(entry, 0);
          await fixtureDevice.queue.onSubmittedWorkDone();
          await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
          const entry = [...surfaces.values()][0];
          fetch('/__loss-probe', {method:'POST',body:JSON.stringify({errors:fixtureErrors,world:JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:'state'}))).world})});
        } catch(error) { fetch('/__loss-probe', {method:'POST',body:JSON.stringify({error:String(error)})}); }
      });
` + (await file.text()).replace('function assets(entry) {', `function assets(entry) {
        if (entry.id && !entry.fixtureProbe) {
          entry.fixtureProbe = true;
          const world = JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:'state'})))?.world;
          fetch('/__asset-probe', {method:'POST',body:JSON.stringify({world,carry:gpu.gpu_carry(entry.id)?.length ?? null})});
        }`);
      return new Response(source+residency.source,{headers:{'Content-Type':'text/javascript'}});
    }
    return await file.exists() ? new Response(file) : new Response('',{status:404});
  }});
  const options = server ? {url:`http://127.0.0.1:${server.port}/`} : {};
  const start = async (world) => {
    const g=next(), s=await open({...options,...(world?{world}: {})});
    const play=s.tap('play');
    if(web) {
      const observation=await g.probe; await g.request;
      check('before delivery: tick 0, declaration pending, not restored', observation.world?.tick===0 && observation.world?.restored===false && observation.world?.loading?.includes('crate.model'), observation);
      check('loading carry refuses with current named states, even during restore', observation.carry===null, observation);
      let answered=false; const read=s.state().then(r=>{answered=true; return r;});
      await Promise.resolve();
      check('state waits at the same settlement barrier as clock and save', !answered);
      g.release(); await read;
    }
    await play; return s;
  };
  try {
    const s=await start();
    const ready = (await s.state()).world[0];
    await s.clock(500);
    const at30=(await s.state()).world[0];
    check('setup and exactly 30 ticks after settlement', at30.tick===30 && at30.loading.length===0 && at30.assets.every(a=>a.state==='Loaded'),at30);
    await s.world('world').key_down('KeyW');
    const saved=resolve(out,'crate.world'); await s.screenshot(saved,'world','save');
    check('steady ticks including paranoid Save do no GPU residency work',JSON.stringify(ready.gpu)===JSON.stringify(at30.gpu),at30.gpu);
    await s.close();
    const restored=await start(saved);
    const loaded=(await restored.state()).world[0];
    check('fresh process resumes saved tick/hash only after delivery', loaded.tick===30 && loaded.restored===true && loaded.loading.length===0 && loaded.hash===at30.hash,loaded);
    check('restored forwarded key owns its later keyup', loaded.input.forwarded.includes('KeyW'),loaded.input);
    await restored.world('world').key_up('KeyW');
    check('keyup clears restored forwarded input', !(await restored.state()).world[0].input.forwarded.includes('KeyW'));
    const clock = await restored.clock(500);
    check('publication-only changes keep clock changing', JSON.stringify(clock).includes('published.tick'), clock);
    const world=(await restored.state()).world[0];
    check('native and web simulation hash agrees at 60', world.tick===60 && world.hash==='0x8f6d518f39634478',world.hash);
    if(web) await checkResidency(residency,restored,check,say);
    if(web) {
      const beforeLoss=resolve(out,'before-loss.png'), afterLoss=resolve(out,'after-loss.png');
      await restored.screenshot(beforeLoss);
      const requestsBefore = textureRequests;
      const recoveryHash = (await restored.state()).world[0].hash;
      const lost = new Promise(resolve=>lossDone=resolve);
      await restored.type('world', {key:'KeyL'});
      const recovered = await Promise.race([lost, new Promise((_,reject)=>setTimeout(()=>reject(new Error('device recovery timeout')),20000))]);
      check('destroyed GPUDevice recovers content and reuploads texture', !recovered.error && !recovered.errors?.length && recovered.world?.hash===recoveryHash && recovered.world?.assets.every(a=>a.state==='Loaded') && textureRequests>requestsBefore, recovered);
      await restored.screenshot(afterLoss);
      say('host recovery owed: web must recreate each canvas surface/context on the new device; native hosts need a recovery ABI preserving the surface table. Native module replacement-device pixel equality is asserted by render/tests/asset_lifecycle.rs.');
    }
    const layout=await restored.layout('world:crate');
    check('declared model supplies layout bounds',!!layout.entity?.bounds,layout);
    if(host!=='macos') await restored.screenshot(resolve(out,`crate-${host}.png`));
    else say('macOS pixel checks run in the shared surface GPU tests; desktop screencapture is unavailable in this session');
    say(`model bytes: ${readFileSync(resolve(import.meta.dir,'assets/crate.model')).length}; texture bytes: ${readFileSync(resolve(import.meta.dir,'assets/crate/0-srgb-straight.tex')).length}`);
    if(!web) say('Headless host: simulation restore/pins verified; GPU residency requires the web/device proof.');
    await restored.close();
    {
      const invalid = resolve(out, 'invalid.world'); writeFileSync(invalid, 'invalid deferred save');
      const g = next(), refused = await open({...options, world:invalid});
      const opening = refused.tap('play').then(()=>null, error=>String(error));
      if (web) {
        const observation = await g.probe; await g.request;
        check('invalid carrier waits for declared content before validation', observation.world?.restored===false && observation.world?.loading.includes('crate.model'));
        g.release();
      }
      const error = await opening;
      check('late restore refusal reaches the creating operation', error?.includes('restore refused'), error);
      const state = (await refused.state()).world[0];
      check('late restore refusal stays on the fresh surface', state.restored===false && state.restoreError?.includes('restore refused'), state.restoreError);
      await refused.world('world').run(100);
      check('fresh world runs after deferred refusal', (await refused.state()).world[0].tick===6);
      const lines = (await refused.logs()).world?.flatMap(w=>w.lines) ?? [];
      const again = (await refused.logs()).world?.flatMap(w=>w.lines) ?? [];
      check('late restore refusal is journalled exactly once', lines.filter(l=>l.includes('restore refused')).length===1 && !again.some(l=>l.includes('restore refused')));
      await refused.close();
    }
    {
      fail=true;
      const empty=resolve(out,'missing-assets'); mkdirSync(empty,{recursive:true});
      const failed=web ? await start() : await open({env:{EXACT_ASSETS:empty}});
      if(!web) await failed.tap('play');
      const state=(await failed.state()).world[0];
      check('terminal transport failures have names without poisoning the world', state.tick===0 && state.assets.some(a=>a.name==='crate.model' && a.state==='Failed' && a.reason.includes(web?'503':'missing')) && !state.renderError,state.assets);
      const refused=await failed.clock(1000);
      check('refused clock retains the failed name and reason', JSON.stringify(refused).includes('crate.model') && JSON.stringify(refused).includes(web?'503':'missing'),refused);
      let refusedSave;
      try { await failed.screenshot(resolve(out,'refused.world'),'world','save'); } catch(error) { refusedSave=String(error); }
      check('failed save reports the asset name', refusedSave?.includes('crate.model'), refusedSave);
      await failed.close();
    }
  } finally { gate?.release(); server?.stop(true); }
});
