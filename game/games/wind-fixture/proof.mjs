#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { decodePng } from '../../../scripts/png.mjs';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
if (import.meta.main) await proof(import.meta, async ({pin, pinSave, open, check, equal, out, say, host}) => {
  const start=async world=>{const s=await open({size:[1280,720],...world?{world}:{}});await s.tap('play');return s;};
  const s=await start(),w=s.world('world');
  await w.run(2500);const mid=await w.snapshot(),save=resolve(out,'wind-150.world');await w.save(save);
  const shot=async name=>{const path=resolve(out,`${name}-${host}.png`);await s.screenshot(path);return decodePng(readFileSync(path));};
  const early=host!=='linux' ? await shot('wind-150') : null;
  await w.run(2500);const end=await w.snapshot();
  say(`PIN wind tick ${end.tick} ${end.hash}`);
  pin(300, end);
  check('the simulation holds 144 reeds and the ground, all still',end.entities.filter(e=>e.components.Mesh).length===145&&equal(mid.entities.map(e=>e.components.Transform),end.entities.map(e=>e.components.Transform)));
  // Presentation state is inspectable, yet the pinned hash above never includes it.
  check('the agent sees the drawn gust',typeof end.entities.find(e=>e.name==='wind')?.components.Gust==='object');
  if(host!=='linux') {
    const late=await shot('wind-300');
    const pixel=(image,x,y)=>Array.from(image.data.slice((y*image.width+x)*4,(y*image.width+x)*4+3));
    const top=pixel(late,Math.floor(late.width/2),4);
    check(`the dusk sky pack draws over the engine's sky (${top})`,top[0]>top[2]);
    check('the reeds sway between two ticks of a still simulation',!Buffer.from(early.data).equals(Buffer.from(late.data)));
    const hooks=(await s.state()).world[0].renderHooks;
    check(`the generated GPU module carries the game's render hooks (${JSON.stringify(hooks?.stages)})`,!!hooks);
  }
  const endSave=resolve(out,'wind-300.world');await w.save(endSave);pinSave('continuation',endSave);await s.close();
  const restored=await start(save),rw=restored.world('world');
  check('fresh host restores the exact snapshot',equal(await rw.snapshot(),mid));
  await rw.run(2500);check('continued derivation and hash match',equal(await rw.snapshot(),end));
  await restored.close();
});
