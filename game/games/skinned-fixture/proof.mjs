#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

await proof(import.meta, async ({open, check, equal, out, say, host}) => {
  const start = async world => { const s = await open(world ? {world} : {}); await s.tap('play'); return s; };
  const s = await start();
  await s.world('world').run(750);
  const at45 = await s.world('world').snapshot();
  check('save is mid-transition at tick 45', at45.tick === 45 && at45.entities.find(e=>e.name==='fox').components.Animator.since > 0);
  const save = resolve(out,'fox-45.world'); await s.world('world').save(save);
  await s.world('world').run(250);
  const pose = await s.op({op:'state', ...await s.target('world:fox'), pose:true});
  say(`state world:fox pose\n${JSON.stringify(pose)}`);
  writeFileSync(resolve(out,`pose-60-${host}.json`), JSON.stringify(pose.pose));
  const pinned = JSON.parse(readFileSync(resolve(import.meta.dir,'logic/tests/tick60.json'),'utf8'));
  check('all 24 joint world transforms match the native tick-60 pin', pose.tick===60 && equal(pose.pose,pinned.pose));
  const layout = await s.layout('world:fox');
  check('animated bounds are available through layout', !!layout.entity?.bounds, layout.entity?.bounds);
  if(host !== 'linux') await s.screenshot(resolve(out,`fox-mid-stride-${host}.png`));
  await s.world('world').run(1000);
  const at120 = await s.world('world').snapshot();
  check('tick-120 cross-host hash', at120.tick===120 && at120.hash==='0xb05ce95a6c799acf',at120.hash);
  await s.close();
  const restored = await start(save);
  check('fresh process restores mid-transition exactly',equal(await restored.world('world').snapshot(),at45));
  await restored.world('world').run(1250);
  check('fresh-process continuation is identical at tick 120',equal(await restored.world('world').snapshot(),at120));
  const finalSave=resolve(out,`fox-120-${host}.world`);await restored.world('world').save(finalSave);
  await restored.close();
});
