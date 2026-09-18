#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

await proof(import.meta, async ({open,check,out,say,host}) => {
  const web = host === 'web';
  let gate, server, fail = false;
  const next = () => {
    let release, observed, requested;
    gate = {delivery:new Promise(r=>release=r), probe:new Promise(r=>observed=r), request:new Promise(r=>requested=r), release, observed, requested};
    return gate;
  };
  if (web) server = Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request) {
    const path = decodeURIComponent(new URL(request.url).pathname);
    if(path === '/__asset-probe') { gate.observed(await request.json()); return new Response('',{status:204}); }
    if(path === '/assets/crate.model') { gate.requested(); await gate.delivery; if(fail) return new Response('',{status:503}); }
    if(path.startsWith('/__')) return new Response('',{status:204});
    if(path.includes('..')) return new Response('',{status:404});
    const file = Bun.file(resolve(import.meta.dir,'dist',path==='/'?'index.html':path.slice(1)));
    if(path === '/gpu-glue.js') {
      // Observe the actual surface before delivery, below the agent's mandatory
      // settlement barrier. This probe exists only in this proof's HTTP response.
      const source = (await file.text()).replace('function assets(entry) {', `function assets(entry) {
        if (entry.id && !entry.fixtureProbe) {
          entry.fixtureProbe = true;
          const world = JSON.parse(gpu.gpu_agent(entry.id, JSON.stringify({op:'state'})))?.world;
          fetch('/__asset-probe', {method:'POST',body:JSON.stringify({world,carry:gpu.gpu_carry(entry.id)?.length ?? null})});
        }`);
      return new Response(source,{headers:{'Content-Type':'text/javascript'}});
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
      check(world?'loading carry returns the pending restore':'loading carry refuses the pre-setup world', world ? observation.carry===readFileSync(world).length : observation.carry===null, observation.carry);
      let answered=false; const read=s.state().then(r=>{answered=true; return r;});
      await Promise.resolve();
      check('state waits at the same settlement barrier as clock and save', !answered);
      g.release(); await read;
    }
    await play; return s;
  };
  try {
    const s=await start();
    await s.clock(500);
    const at30=(await s.state()).world[0];
    check('setup and exactly 30 ticks after settlement', at30.tick===30 && at30.loading.length===0 && at30.assets.every(a=>a.state==='Loaded'),at30);
    await s.world('world').key_down('KeyW');
    const saved=resolve(out,'crate.world'); await s.screenshot(saved,'world','save');
    await s.close();
    const restored=await start(saved);
    const loaded=(await restored.state()).world[0];
    check('fresh process resumes saved tick/hash only after delivery', loaded.tick===30 && loaded.restored===true && loaded.loading.length===0 && loaded.hash===at30.hash,loaded);
    check('restored forwarded key owns its later keyup', loaded.input.forwarded.includes('KeyW'),loaded.input);
    await restored.world('world').key_up('KeyW');
    check('keyup clears restored forwarded input', !(await restored.state()).world[0].input.forwarded.includes('KeyW'));
    await restored.clock(500);
    const world=(await restored.state()).world[0];
    check('native and web simulation hash agrees at 60', world.tick===60 && world.hash==='0x8f6d518f39634478',world.hash);
    const layout=await restored.layout('world:crate');
    check('declared model supplies layout bounds',!!layout.entity?.bounds,layout);
    if(host!=='macos') await restored.screenshot(resolve(out,`crate-${host}.png`));
    else say('macOS pixel checks run in the shared surface GPU tests; desktop screencapture is unavailable in this session');
    say(`model bytes: ${readFileSync(resolve(import.meta.dir,'assets/crate.model')).length}; texture bytes: ${readFileSync(resolve(import.meta.dir,'assets/crate/0-srgb.tex')).length}`);
    await restored.close();
    if(web) {
      fail=true; const failed=await start();
      const state=(await failed.state()).world[0];
      check('terminal transport failures have names without poisoning the world', state.tick===0 && state.assets.some(a=>a.name==='crate.model' && a.state==='Failed' && a.reason.includes('503')) && !state.renderError,state.assets);
      const refused=await failed.clock(1000);
      check('refused clock retains the failed name and reason', JSON.stringify(refused).includes('crate.model') && JSON.stringify(refused).includes('503'),refused);
      await failed.close();
    }
  } finally { gate?.release(); server?.stop(true); }
});
