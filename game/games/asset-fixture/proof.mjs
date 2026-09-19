#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { decodePng } from '../../../scripts/png.mjs';
import { residencyProbe, checkResidency, checkSteadyResidency } from './residency.mjs';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

await proof(import.meta, async ({pin, pinSave, open,check,equal,out,say,host}) => {
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
    if(path === '/gpu-assets.js') {
      // The delivery gate follows the asset module; the renderer probe stays
      // in gpu-glue. Neither probe changes the shipped source.
      const source = (await file.text()).replace('  const names = JSON.parse(module.gpu_assets(id));', `
        if (!entry.fixtureProbe) {
          entry.fixtureProbe = true;
          const world = JSON.parse(module.gpu_agent(id, JSON.stringify({op:'state'})))?.world;
          let carry, carryError;
          try { carry = module.gpu_carry(id)?.length ?? null; } catch (error) { carryError = String(error); }
          fetch('/__asset-probe', {method:'POST',body:JSON.stringify({world,carry,carryError})});
        }
        const names = JSON.parse(module.gpu_assets(id));`);
      return new Response(source,{headers:{'Content-Type':'text/javascript'}});
    }
    if(path === '/gpu-glue.js') {
      // Observe the actual surface before delivery, below the agent's mandatory
      // settlement barrier. This probe exists only in this proof's HTTP response.
      const source = `let fixtureDevice; const fixtureErrors=[];
      let fixtureFailAdapter = false, fixtureAdapterFailures = 0;
      const fixtureAdapter = navigator.gpu.requestAdapter.bind(navigator.gpu);
      navigator.gpu.requestAdapter = async (...args) => {
        if (fixtureFailAdapter) { fixtureFailAdapter = false; fixtureAdapterFailures++; return null; }
        return fixtureAdapter(...args);
      };
      const fixtureRequest = GPUAdapter.prototype.requestDevice;
      GPUAdapter.prototype.requestDevice = async function(...args) {
        fixtureDevice = await fixtureRequest.apply(this, args); fixtureDevice.addEventListener('uncapturederror',e=>fixtureErrors.push(e.error.message)); return fixtureDevice;
      };
      document.addEventListener('keydown', async event => {
        if(event.code !== 'KeyL') return;
        try {
          const originalCanvases = [...surfaces.values()].map(e=>e.el);
          await recoverDevice();
          const healthyNoCutover = exact.gpu.recovery?.status === 'healthy' && [...surfaces.values()].every((e,i)=>e.el===originalCanvases[i]);
          if (!healthyNoCutover) throw new Error('healthy recovery replaced a canvas');
          fixtureFailAdapter = true;
          fixtureDevice.destroy(); await fixtureDevice.lost;
          await new Promise(resolve=>setTimeout(resolve, 0));
          for(const entry of surfaces.values()) render(entry, 0);
          await recoveringDevice;
          for(let retry=0; exact.gpu.recovery?.status !== 'recovered' && retry<200; retry++) await new Promise(r=>setTimeout(r,10));
          if(exact.gpu.recovery?.status !== 'recovered' || fixtureAdapterFailures !== 1) throw new Error('fail-once recovery did not retry successfully');
          await settled();
          for(const entry of surfaces.values()) render(entry, 0);
          await fixtureDevice.queue.onSubmittedWorkDone();
          await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
          const entry = [...surfaces.values()][0];
          fetch('/__loss-probe', {method:'POST',body:JSON.stringify({healthyNoCutover,errors:fixtureErrors,world:JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:'state'}))).world})});
        } catch(error) { fetch('/__loss-probe', {method:'POST',body:JSON.stringify({error:String(error)})}); }
      });
` + await file.text();
      return new Response(source+residency.source,{headers:{'Content-Type':'text/javascript'}});
    }
    return await file.exists() ? new Response(file) : new Response('',{status:404});
  }});
  const options = server ? {url:`http://127.0.0.1:${server.port}/`} : {};
  const start = async (world) => {
    const g=next(), s=await open({...options,...(world?{world}: {})});
    const play=s.tap('play');
    if(web) {
      const [observation]=await Promise.race([Promise.all([g.probe,g.request]), play.then(()=>{throw new Error('Play completed before the asset delivery gate');})]);
      check('before delivery: tick 0, declaration pending, not restored', observation.world?.tick===0 && observation.world?.restored===false && observation.world?.loading?.includes('crate.model'), observation);
      check('loading carry refuses with current named states, even during restore', observation.carry===undefined && /assets are not ready/.test(observation.carryError ?? '') && observation.carryError.includes('crate.model'), observation);
      let answered=false; const read=s.state().then(r=>{answered=true; return r;});
      await Promise.resolve();
      check('state waits at the same settlement barrier as clock and save', !answered);
      g.release(); await read;
    }
    await play; return s;
  };
  try {
    const s=await start();
    // Establish the device's first draw before measuring after-ready ticks.
    if (host !== 'linux') await s.screenshot(resolve(out, 'crate-ready.png'));
    await s.clock(500);
    const at30=(await s.state()).world[0];
    check('setup and exactly 30 ticks after settlement', at30.tick===30 && at30.loading.length===0 && at30.assets.every(a=>a.state==='Loaded'),at30);
    await s.world('world').key_down('KeyW');
    const saved=resolve(out,'crate.world'); await s.screenshot(saved,'world','save');
    checkSteadyResidency(at30, check, say, host);
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
    pin(60, world);
    const pinnedSave=resolve(out,"continuation.world"); await restored.world("world").save(pinnedSave); pinSave("continuation",pinnedSave);
    if(web) await checkResidency(residency,restored,check,say);
    if(web) {
      const beforeLoss=resolve(out,'before-loss.png'), afterLoss=resolve(out,'after-loss.png');
      await restored.screenshot(beforeLoss);
      const requestsBefore = textureRequests;
      const recoveryHash = (await restored.state()).world[0].hash;
      const lost = new Promise(resolve=>lossDone=resolve);
      await restored.type('world', {key:'KeyL'});
      const recovered = await Promise.race([lost, new Promise((_,reject)=>setTimeout(()=>reject(new Error('device recovery timeout')),20000))]);
      check('healthy device recovery attaches no replacement canvases',recovered.healthyNoCutover===true,recovered);
      check('destroyed GPUDevice recovers content and reuploads texture', !recovered.error && !recovered.errors?.length && recovered.world?.hash===recoveryHash && recovered.world?.assets.every(a=>a.state==='Loaded') && textureRequests>requestsBefore, recovered);
      await restored.screenshot(afterLoss);
      const before = decodePng(readFileSync(beforeLoss)), after = decodePng(readFileSync(afterLoss));
      check('device recovery presents identical pixels', before.width===after.width && before.height===after.height && Buffer.from(before.data).equals(Buffer.from(after.data)));
      check('recovery has no readiness reasons', recovered.world?.ready && recovered.world.readyReasons.length===0, recovered.world?.readyReasons);
      check('replacement device uploads only retained assets and re-prepares pipelines', equal(recovered.world?.gpu.beforeReady,at30.gpu.beforeReady) && Object.values(recovered.world.gpu.afterReady).every(n=>n===0), recovered.world?.gpu);
      check('one retained texture is fetched exactly once', textureRequests===requestsBefore+1, {before:requestsBefore,after:textureRequests});
    }
    const layout=await restored.layout('world:crate');
    check('declared model supplies layout bounds',!!layout.entity?.bounds,layout);
    await restored.screenshot(resolve(out,`crate-${host}.png`));
    if(host==='macos' || host==='ios') say('SKIP physical Metal removal: this integrated device cannot be removed; native recovery ABI and replacement-device pixels run in the GPU tests.');
    say(`model bytes: ${readFileSync(resolve(import.meta.dir,'assets/crate.model')).length}; texture bytes: ${readFileSync(resolve(import.meta.dir,'assets/crate/0-srgb-straight.tex')).length}`);
    if(host==='ios') say('SKIP browser transport gates and residency reload probe: simulator uses bundled files; asset readiness, failure, restore, GPU counters and pins still run.');
    if(host==='linux') say('Headless host: simulation restore/pins verified; GPU residency requires the web/device proof.');
    await restored.close();
    {
      const invalid = resolve(out, 'invalid.world'); writeFileSync(invalid, 'invalid deferred save');
      const g = next(), refused = await open({...options, world:invalid});
      const opening = refused.tap('play').then(()=>null, error=>String(error));
      if (web) {
        const [observation] = await Promise.race([Promise.all([g.probe,g.request]), opening.then(error=>{throw new Error(`restore completed before the asset delivery gate: ${error}`);})]);
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
