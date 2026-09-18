// Fixture-only instrumentation of the real Surface ABI; no production agent verbs.
export function residencyProbe(model, texture) {
  let receive, changedTexture = false;
  const popName = model === 'fox.model' ? 'new.model' : 'extra.model';
  const source = `
  document.addEventListener('keydown', async event => {
    if (!['KeyR','KeyC','KeyP'].includes(event.code)) return;
    event.stopImmediatePropagation(); event.preventDefault();
    const entry = [...surfaces.values()][0];
    const state = () => JSON.parse(gpu.gpu_agent(entry.id, '{"op":"state"}')).world;
    const draw = () => render(entry, performance.now());
    try {
      const before = state(), saved = gpu.gpu_carry(entry.id), original = saved.slice(), samples = [];
      if (event.code === 'KeyR') {
        for (let i=0; i<21; i++) {
          performance.mark('a3-restore-start');
          if (!gpu.gpu_restore(entry.id, saved, 0)) throw new Error(gpu.gpu_error());
          draw();
          const sample = performance.measure('a3-restore', 'a3-restore-start').duration;
          if (i) samples.push(sample);
        }
      } else {
        // Replace only the Mesh asset name in a valid save with an equal-length name.
        const from = new TextEncoder().encode('${model}'), to = new TextEncoder().encode('${popName}');
        let replaced = false;
        for (let i=0;i<=saved.length-from.length;i++) {
          if(from.every((b,j)=>saved[i+j]===b)) { saved.set(to,i); replaced=true; }
        }
        if(!replaced) throw new Error('saved mesh name absent');
        if(!gpu.gpu_restore(entry.id,saved,0)) throw new Error(gpu.gpu_error());
        if(event.code === 'KeyC') {
          // Drain retirement without fetching the temporary name, then carry the
          // original scene back. This exercises host-authorized redelivery on
          // the same device; a module swap would necessarily use a new renderer.
          gpu.gpu_assets(entry.id); gpu.gpu_retired(entry.id);
          if(!gpu.gpu_restore(entry.id,original,1)) throw new Error(gpu.gpu_error());
        }
        draw(); assets(entry); await settled(); draw();
      }
      const after = state();
      if(event.code === 'KeyP') {
        if(!gpu.gpu_restore(entry.id,original,0)) throw new Error(gpu.gpu_error());
        draw(); assets(entry); await settled(); draw();
      }
      fetch('/__residency', {method:'POST',body:JSON.stringify({before,after,samples})});
    } catch(error) { fetch('/__residency', {method:'POST',body:JSON.stringify({error:String(error)})}); }
  }, true);
`;
  return {
    source,
    assetPath: path => path === `/assets/${popName}` ? `/assets/${model}` : path,
    async textureResponse(path, file) {
      if(!changedTexture || path !== `/assets/${texture}`) return null;
      const bytes = new Uint8Array(await file.arrayBuffer());
      const marker=[109,105,112,115];
      let p=bytes.findIndex((_,i)=>marker.every((b,j)=>bytes[i+j]===b))+4;
      if(p<4 || bytes[p++]!==7) throw new Error('missing mip sequence');
      while(bytes[p++] & 128) {}
      if(bytes[p++]!==12) throw new Error('missing mip bytes');
      while(bytes[p++] & 128) {}
      bytes[p]^=127;
      return new Response(bytes);
    },
    async fetch(request) {
      if(new URL(request.url).pathname !== '/__residency') return null;
      receive?.(await request.json()); return new Response('',{status:204});
    },
    async run(session, key='KeyR') {
      let timer;
      if(key==='KeyC') changedTexture=true;
      const result = new Promise((ok,reject)=>{
        receive=ok; timer=setTimeout(()=>reject(new Error('residency probe timeout')),20000);
      });
      try { await session.type('world',{key}); return await result; }
      finally { clearTimeout(timer); receive=null; }
    },
  };
}
export async function checkResidency(probe, session, check, say) {
  const restored = await probe.run(session);
  if(restored.error) throw new Error(restored.error);
  const unchanged = (a,b) => JSON.stringify(a.gpu) === JSON.stringify(b.gpu);
  check('same-device save/restore: zero uploads, pipelines or reallocations', restored.before.ready && restored.after.ready && unchanged(restored.before,restored.after), restored.after.gpu);
  const samples = (restored.samples ?? []).sort((a,b)=>a-b);
  if (samples.length) say(`restore through first draw, browser performance trace: n=${samples.length}, p50=${samples[Math.floor(samples.length/2)].toFixed(3)} ms, p95=${samples[Math.ceil(samples.length*.95)-1].toFixed(3)} ms`);
  else say('restore timing samples: none recorded by this host');
  const delta = (a,b,key) => b.gpu.afterReady[key]-a.gpu.afterReady[key];
  const changed = await probe.run(session, 'KeyC');
  if(changed.error) throw new Error(changed.error);
  check('changed texture carry uploads exactly one texture', changed.after.ready && delta(changed.before,changed.after,'textureUploads') === 1, changed);
  check('changed texture carry reuses geometry, pipelines and model/skin capacity',
    ['meshUploads','pipelineCreations','modelSkinBufferReallocations'].every(key=>delta(changed.before,changed.after,key)===0), changed.after.gpu);
  const repeated = await probe.run(session, 'KeyC');
  if(repeated.error) throw new Error(repeated.error);
  check('same texture bytes redelivered reuse residency', repeated.after.ready && unchanged(repeated.before,repeated.after), repeated.after.gpu);
  const popped = await probe.run(session, 'KeyP');
  if(popped.error) throw new Error(popped.error);
  check('new model name uploads geometry after arrival', popped.after.ready && delta(popped.before,popped.after,'meshUploads') > 0, popped.after.gpu);
}

export function checkSteadyResidency(world, check, say) {
  if (world.device === false) {
    say('SKIP: no device — after-ready GPU residency');
    return;
  }
  check('after-ready ticks do no recorded GPU residency work',
    world.device === true && world.ready && Object.values(world.gpu.afterReady).every(n => n === 0), world.gpu);
}
