#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { residencyProbe, checkResidency } from '../asset-fixture/residency.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawn } from 'node:child_process';

await proof(import.meta, async ({open, check, equal, out, say, host}) => {
  const probe = residencyProbe('fox.model','fox/0-srgb-straight.tex');
  const server = host === 'web' ? Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request) {
    const reply = await probe.fetch(request); if(reply) return reply;
    const path = probe.assetPath(new URL(request.url).pathname);
    const file = Bun.file(resolve(import.meta.dir,'dist',path==='/'?'index.html':path.slice(1)));
    const textureReply = await probe.textureResponse(path,file); if(textureReply) return textureReply;
    if(path==='/gpu-glue.js') return new Response(await file.text()+probe.source,{headers:{'Content-Type':'text/javascript'}});
    return new Response(file);
  }}) : null;
  try {
  const start = async world => { const s = await open({...server && {url:`http://127.0.0.1:${server.port}/`},...world && {world}}); await s.tap('play'); return s; };
  const s = await start();
  const ready = (await s.state()).world[0];
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
  check('tick-60 cross-host hash', (await s.world('world').snapshot()).hash==='0xb863e854ca85b74e');
  const layout = await s.layout('world:fox');
  check('animated bounds are available through layout', !!layout.entity?.bounds, layout.entity?.bounds);
  if(host !== 'linux') await s.screenshot(resolve(out,`fox-mid-stride-${host}.png`));
  await s.world('world').run(1000);
  const at120 = await s.world('world').snapshot();
  check('tick-120 cross-host hash', at120.tick===120 && at120.hash==='0x409341e24939d7c2',at120.hash);
  const referenceSave=resolve(out,`fox-120-reference-${host}.world`);await s.world('world').save(referenceSave);
  const log=await s.logs();
  check('fixture consumes step markers', JSON.stringify(log).includes('fox footstep'));
  check('clip root motion advances the fox', at120.entities.find(e=>e.name==='fox').components.Transform.position[2] > -1.8);
  const afterTicks = (await s.state()).world[0];
  check('steady ticks including paranoid Save do no GPU residency work', JSON.stringify(ready.gpu)===JSON.stringify(afterTicks.gpu),afterTicks.gpu);
  if(host==='web') await checkResidency(probe,s,check,say);
  else say('Headless host: simulation restore/pins verified; GPU residency requires the web/device proof.');
  await s.close();
  const restored = await start(save);
  check('fresh process restores mid-transition exactly',equal(await restored.world('world').snapshot(),at45));
  await restored.world('world').run(1250);
  check('fresh-process continuation is identical at tick 120',equal(await restored.world('world').snapshot(),at120));
  const finalSave=resolve(out,`fox-120-${host}.world`);await restored.world('world').save(finalSave);
  check('fresh continuation saves byte-identically',readFileSync(finalSave).equals(readFileSync(referenceSave)));
  await restored.close();
  const cli = spawn('bun',[resolve(import.meta.dir,'../../../scripts/agent.mjs'),host,'--json','tap play','state world:fox pose'],{env:process.env,stdio:['ignore','pipe','pipe']});
  let stdout='',stderr=''; cli.stdout.on('data',b=>stdout+=b); cli.stderr.on('data',b=>stderr+=b);
  const code=await new Promise((ok,reject)=>{cli.on('exit',ok);cli.on('error',reject);});
  const replies=stdout.trim().split('\n').filter(Boolean).map(line=>{try{return JSON.parse(line);}catch{return null;}});
  check('literal CLI state world:fox pose forwards pose:true',code===0 && replies.some(r=>Array.isArray(r?.pose) && r.pose.length===24),stderr || stdout.slice(-300));
  } finally { server?.stop(true); }
});
