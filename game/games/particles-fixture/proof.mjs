#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { checkSteadyResidency } from '../../render/tests/residency.mjs';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
if (import.meta.main) await proof(import.meta, async ({pin, pinSave, open,check,equal,out,say,host}) => {
  const start=async world=>{const s=await open({size:[1280,720],...world?{world}:{}});await s.tap('play');return s;};
  const s=await start(),w=s.world('world');
  await w.run(2500);const mid=await w.snapshot(),save=resolve(out,'sparks-150.world');await w.save(save);
  await w.run(2500);const end=await w.snapshot();
  const emitters=end.entities.filter(e=>e.components.Emitter);
  check('twenty emitters produce 20,000 sparks at tick 300 without particle entities',end.tick===300&&end.entities.length===21&&emitters.reduce((n,e)=>n+e.components.Emitter.state.alive,0)===20000);
  check('each emitter has saved seed, age and RNG stream',emitters.every(e=>e.components.Emitter.state.age===300&&e.components.Emitter.state.stream===8000));
  say(`PIN particles tick ${end.tick} ${end.hash}`);
  pin(300, end);
  check('layout uses the authored bound',!!(await s.layout('world:sparks-0')).entity?.bounds);
  const state=await s.state('world:sparks-0');
  check('state exposes the emitter, never individual particles',state.entity.components.Emitter.state.alive===1000&&!('particles' in state.entity.components));
  if(host!=='linux') await s.screenshot(resolve(out,`particles-${host}.png`));
  checkSteadyResidency((await s.state()).world[0],check,say,host);
  const endSave=resolve(out,'sparks-300.world');await w.save(endSave);pinSave('continuation',endSave);await s.close();
  const restored=await start(save),rw=restored.world('world');
  check('fresh host restores the exact mid-flight snapshot',equal(await rw.snapshot(),mid));
  await rw.run(2500);check('continued derivation and hash match',equal(await rw.snapshot(),end));
  const restoredSave=resolve(out,'sparks-300-restored.world');await rw.save(restoredSave);
  check('save bytes match after continuation',readFileSync(endSave).equals(readFileSync(restoredSave)));
  await restored.close();
});
