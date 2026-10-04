#!/usr/bin/env bun
import {checkSteadyResidency} from '../../render/tests/residency.mjs';
import { proof } from '../../proof.mjs';
import { decodePng } from '../../../scripts/png.mjs';
import {residencyProbe, checkResidency, checkTextureFamily, bakedTexture} from '../../render/tests/residency.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawn } from 'node:child_process';

export function foxPixels(image, screen, viewport) {
  // This proof's requested viewport is 16:9; native screenshots may scale it.
  if (!screen || Math.abs(image.width/image.height - 16/9) > .02 || Math.abs(viewport[0]/viewport[1] - 16/9) > .02) return {ok:false,reason:'expected 16:9 viewport'};
  const sx=image.width/viewport[0], sy=image.height/viewport[1];
  const x0=Math.max(0,Math.floor(screen.x*sx)), y0=Math.max(0,Math.floor(screen.y*sy));
  const x1=Math.min(image.width,Math.ceil((screen.x+screen.w)*sx)), y1=Math.min(image.height,Math.ceil((screen.y+screen.h)*sy));
  let orange=0, nonwhite=0, cropPixels=0; const shades=new Set();
  for(let y=y0;y<y1;y++) for(let x=x0;x<x1;x++) {
    const i=(y*image.width+x)*4, [r,g,b]=image.data.subarray(i,i+3); cropPixels++;
    if(Math.min(r,g,b)<235) nonwhite++;
    if(r>g*1.35 && g>b*1.2 && r>70 && g>20) {
      orange++; shades.add(`${r>>3},${g>>3},${b>>3}`);
    }
  }
  const fraction=orange/cropPixels, nonwhiteFraction=nonwhite/cropPixels;
  return {ok:cropPixels>0 && fraction>.001 && nonwhiteFraction>.1 && shades.size>=8, orange,cropPixels,fraction,nonwhiteFraction,shades:shades.size,crop:[x0,y0,x1,y1]};
}

export function foxScreenshotPixels(image, screen, reply) {
  const scale = reply.scale ?? image.width / reply.w;
  if (!(reply.w > 0 && reply.h > 0 && scale > 0) || Math.abs(image.width - reply.w * scale) > 1 || Math.abs(image.height - reply.h * scale) > 1) return {ok:false, reason:'screenshot dimensions disagree with logical size and scale'};
  return foxPixels(image, screen, [reply.w, reply.h]);
}

if (import.meta.main) await proof(import.meta, async ({pin, pinSave, open, check, equal, out, say, host}) => {
  const probe = residencyProbe('fox.model',() => bakedTexture(import.meta.dir),'new.model');
  const server = host === 'web' ? Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request) {
    const reply = await probe.fetch(request); if(reply) return reply;
    const path = probe.assetPath(new URL(request.url).pathname);
    const file = Bun.file(resolve(process.env.EXACT_WEB_DIST ?? resolve(import.meta.dir,'dist'),path==='/'?'index.html':path.slice(1)));
    const textureReply = await probe.textureResponse(path,file); if(textureReply) return textureReply;
    // The probe names gpu-glue's own functions; the web build minifies them (af5dfdf4).
    if(path==='/gpu-glue.js') return new Response(await Bun.file(resolve(import.meta.dir,'../../../host/web/gpu-glue.js')).text()+probe.source,{headers:{'Content-Type':'text/javascript'}});
    return new Response(file);
  }}) : null;
  try {
  const start = async world => { const s = await open({...server && {url:`http://127.0.0.1:${server.port}/`},...world && {world}}); await s.tap('play'); return s; };
  const s = await start();
  const ready = (await s.state()).world[0];
    if (host !== 'linux') {
      const limits = (await s.state()).world[0].gpu;
      check('renderer storage needs fit this host device', limits.requiredStorageBindings <= limits.storageBindings, limits);
    }
  say(`module asset states: ${JSON.stringify(ready.assets)}`);
  if (host !== 'linux') await s.op({op:'state', ...await s.target('world'), world:true, perf_reset:true});
  await s.world('world').run(750);
  const at45 = await s.world('world').snapshot();
  check('save is mid-transition at tick 45', at45.tick === 45 && at45.entities.find(e=>e.name==='fox').components.Animator.since > 0);
  const save = resolve(out,'fox-45.world'); await s.world('world').save(save);
  await s.world('world').run(250);
  const pose = await s.state('world:fox', undefined, true);
  say(`state world:fox pose\n${JSON.stringify(pose)}`);
  writeFileSync(resolve(out,`pose-60-${host}.json`), JSON.stringify(pose.pose));
  const pinned = JSON.parse(readFileSync(resolve(import.meta.dir,'logic/tests/tick60.json'),'utf8'));
  check('all 24 joint world transforms match the native tick-60 pin', pose.tick===60 && equal(pose.pose,pinned.pose));
  pin(60, await s.world('world').snapshot());
  const layout = await s.layout('world:fox');
  check('animated bounds are available through layout', !!layout.entity?.bounds, layout.entity?.bounds);
  if(host !== 'linux') {
    const path = resolve(out,`fox-mid-stride-${host}.png`);
    const screenshot = await s.screenshot(path);
    const image = decodePng(readFileSync(path));
    const pixels=foxScreenshotPixels(image, layout.entity.screen, screenshot);
    check('Fox screenshot has textured orange fur',pixels.ok,pixels);
    const white={...image,data:new Uint8Array(image.data).fill(255)};
    for(let i=0;i<image.data.length;i+=80) white.data.set(image.data.subarray(i,i+4),i);
    check('95 percent white screenshot is rejected',!foxScreenshotPixels(white,layout.entity.screen,screenshot).ok);
  }
  await s.world('world').run(1000);
  const at120 = await s.world('world').snapshot();
  pin(120, at120);
  const referenceSave=resolve(out,'fox-120-reference.world');await s.world('world').save(referenceSave); pinSave('continuation',referenceSave);
  const log=await s.logs();
  check('fixture consumes step markers', JSON.stringify(log).includes('fox footstep'));
  check('clip root motion advances the fox', at120.entities.find(e=>e.name==='fox').components.Transform.position[2] > -1.8);
  const afterTicks = (await s.state()).world[0];
  checkSteadyResidency(afterTicks, check, say, host);
  checkTextureFamily(afterTicks, check, say);
  if (host !== 'linux') {
    writeFileSync(resolve(out, `perf-${host}.json`), JSON.stringify({unsampled:true, perf:afterTicks.perf, gpu:afterTicks.gpu}, null, 2)+'\n');
    say(`PERF ${host} seekable counters (timing rings are unsampled, not zero-cost frames): ${JSON.stringify(afterTicks.perf)}`);
  }
  if(host==='web') await checkResidency(probe,s,check,say);
  else if(host==='ios') say('SKIP browser residency reload probe: simulator uses bundled files; textured pixels, pose, GPU counters, restore and pins still run.');
  else if(host==='linux') say('Headless host: simulation restore/pins verified; GPU residency requires the web/device proof.');
  await s.close();
  const restored = await start(save);
  check('fresh process restores mid-transition exactly',equal(await restored.world('world').snapshot(),at45));
  await restored.world('world').run(1250);
  check('fresh-process continuation is identical at tick 120',equal(await restored.world('world').snapshot(),at120));
  const finalSave=resolve(out,'fox-120.world');await restored.world('world').save(finalSave);
  check('fresh continuation saves byte-identically',readFileSync(finalSave).equals(readFileSync(referenceSave)));
  await restored.close();
  // Setup is a presentable bind pose, also after a pre-first-step save.
  let attached = await start();
  const setupLayout = await attached.layout('world:charm');
  check('first present uses the bind socket before a step', (await attached.state()).world[0].tick===0 && setupLayout.entity.world.position.some(v=>Math.abs(v)>0.1), setupLayout);
  const setupSave=resolve(out,'fox-setup.world'); await attached.world('world').save(setupSave);
  await attached.close(); attached=await start(setupSave);
  check('pre-first-step restore keeps the bind socket', equal((await attached.layout('world:charm')).entity.world.position, setupLayout.entity.world.position));
  await attached.world('world').key_down('KeyM'); await attached.world('world').run(1000/60+.001); await attached.world('world').key_up('KeyM');
  const charm = await attached.state('world:charm');
  check('mirrored charm has a negative offset determinant', charm.entity.components.SocketFollow.offset.scale[0]<0, charm);
  const last=(await attached.layout('world:charm')).entity.world.position;
  await attached.world('world').key_down('KeyK'); await attached.world('world').run(1000/60+.001); await attached.world('world').key_up('KeyK');
  check('skipped animation tick holds the composed charm', equal((await attached.layout('world:charm')).entity.world.position,last));
  await attached.screenshot(resolve(out,`fox-mirrored-skipped-${host}.png`));
  await attached.close();
  const cli = spawn('bun',[resolve(import.meta.dir,'../../../scripts/agent.mjs'),host,'--json','tap play','state world:fox pose'],{env:process.env,stdio:['ignore','pipe','pipe']});
  let stdout='',stderr=''; cli.stdout.on('data',b=>stdout+=b); cli.stderr.on('data',b=>stderr+=b);
  const code=await new Promise((ok,reject)=>{cli.on('exit',ok);cli.on('error',reject);});
  const replies=stdout.trim().split('\n').filter(Boolean).map(line=>{try{return JSON.parse(line);}catch{return null;}});
  check('literal CLI state world:fox pose forwards pose:true',code===0 && replies.some(r=>Array.isArray(r?.pose) && r.pose.length===24),stderr || stdout.slice(-300));
  // Live/host-only observations have no deterministic cross-host save identity.
  if (['ios','macos'].includes(host) && process.env.EXACT_PROOF_COMPARE !== '1') {
    const live = await open({timing:'platform'});
    await live.tap('play');
    await live.op({op:'state', ...await live.target('world'), world:true, perf_reset:true});
    const started = (await live.state()).world[0].presentation;
    let measured;
    for (let sample=0; sample<40; sample++) {
      await new Promise(resolve => setTimeout(resolve, 100));
      measured = (await live.state()).world[0];
      if (measured.perf?.frameMs?.count >= 60) break;
    }
    check('platform display link produces nonzero timing samples', measured.perf?.wallClock === true && measured.perf?.frameMs?.count >= 10 && measured.perf.frameMs.p50 > 0, measured.perf);
    const presentation = {...measured.presentation, renders:measured.presentation.sessionRenders-started.sessionRenders, captures:measured.presentation.sessionCaptures-started.sessionCaptures};
    check('live presentation reports HUD and placement counts', presentation.renders > 0 && presentation.hudChildren > 0, presentation);
    const receipt = {label:`${host} CPU/presentation timings; no GPU duration measurement`, perf:measured.perf, gpu:measured.gpu, presentation};
    writeFileSync(resolve(out, `perf-${host}-live.json`), JSON.stringify(receipt,null,2)+'\n');
    say(`LIVE ${host} ${JSON.stringify(receipt)}`);
    await live.close();
  }

  } finally { server?.stop(true); }
});
