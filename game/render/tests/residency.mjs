// Renderer residency verification shared by the asset-backed game proofs.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
export function checkSteadyResidency(world, check, say, host) {
  if (host === 'linux' && world.device !== true) {
    say('SKIP: no device — after-ready GPU residency');
    return;
  }
  check('after-ready ticks do no recorded GPU residency work',
    world.device === true && world.ready && Object.values(world.gpu.afterReady).every(n => n === 0), world.gpu);
}

// A device fetches textures in its own block family (BC, else ASTC, else RGBA8);
// a GPU-less host fetches the RGBA8 fallback. A family file may hold RGBA8 where
// blocks cannot represent the texture (a sprite's exact palette).
export function checkTextureFamily(world, check, say) {
  const gpu = world.gpu ?? {};
  say(`texture family ${gpu.textureFamily}: ${JSON.stringify(gpu.textures)}`);
  if (world.device !== true) {
    check('GPU-less host fetches the RGBA8 fallback', gpu.textureFamily === 'Rgba8', gpu);
    return;
  }
  const family = {Bc: /^Bc\d$/, Astc: /^Astc/, Rgba8: /^Rgba8$/}[gpu.textureFamily];
  const formats = Object.keys(gpu.textures ?? {}).filter(key => key !== 'bytes');
  check(`device textures are resident in the ${gpu.textureFamily} family it fetched`,
    !!family && formats.length > 0 && formats.every(f => family.test(f) || f === 'Rgba8'), gpu);
}

// Fixture-only instrumentation of the real Surface ABI; no production agent verbs.
/** A game's one baked model texture (`textures/<digest>.tex`), read from its
 * bake manifest once the bake has run: textures are named by content. */
export function bakedTexture(dir) {
  const names = Object.keys(JSON.parse(readFileSync(resolve(dir, '.baked-assets.json'), 'utf8')))
    .filter(name => /^textures\/[0-9a-f]{16}\.tex$/.test(name));
  if (names.length !== 1) throw new Error(`${dir}: expected one baked model texture, found ${JSON.stringify(names)}`);
  return names[0];
}

/** `texture` is a name, or a function returning it once the bake has run. */
export function residencyProbe(model, texture, popName) {
  let receive, changedTexture = false;
  if (typeof popName !== 'string' || Buffer.byteLength(popName) !== Buffer.byteLength(model) || popName === model)
    throw new Error('replacement model name must differ and have the same byte length');
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
          gpu.gpu_assets(entry.id);
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
      // The device fetches one family's file for the authored name (x.tex,
      // x.bc.tex or x.astc.tex); change whichever it asked for.
      const stem = (typeof texture === 'function' ? texture() : texture).replace(/\.tex$/, '');
      if(!changedTexture || !['.tex','.bc.tex','.astc.tex'].some(suffix => path === `/assets/${stem}${suffix}`)) return null;
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
      // The changed bytes belong to this probe alone: a later fetch of the
      // texture (a device loss's re-upload) is served the true file.
      try { await session.type('world',{key}); return await result; }
      finally { clearTimeout(timer); receive=null; changedTexture=false; }
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
  // Declared assets (Game::ASSETS) stay resident while unshown, so swapping the
  // scene away and back retires nothing and nothing is refetched: the changed
  // texture bytes are never requested, and no texture uploads.
  check('a declared model and its texture stay resident across a scene swap: no texture uploads', changed.after.ready && delta(changed.before,changed.after,'textureUploads') === 0, changed);
  check('changed texture carry reuses geometry, pipelines and model/skin capacity',
    ['meshUploads','pipelineCreations','modelSkinBufferReallocations'].every(key=>delta(changed.before,changed.after,key)===0), changed.after.gpu);
  const repeated = await probe.run(session, 'KeyC');
  if(repeated.error) throw new Error(repeated.error);
  check('same texture bytes redelivered reuse residency', repeated.after.ready && unchanged(repeated.before,repeated.after), repeated.after.gpu);
  const popped = await probe.run(session, 'KeyP');
  if(popped.error) throw new Error(popped.error);
  check('new model name reuses textures and pipelines',
    ['textureUploads','pipelineCreations'].every(key=>delta(popped.before,popped.after,key)===0), popped.after.gpu);
  check('new model name uploads geometry after arrival', popped.after.ready && delta(popped.before,popped.after,'meshUploads') > 0, popped.after.gpu);
}
