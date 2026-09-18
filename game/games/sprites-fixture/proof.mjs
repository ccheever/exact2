#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { checkSteadyResidency } from '../asset-fixture/residency.mjs';
import { decodePng } from '../../../scripts/png.mjs';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
await proof(import.meta,async ({open,check,equal,out,say,host})=>{
  const start=async world=>{const s=await open({size:[1280,720],...world?{world}:{}});await s.tap('play');return s;};
  const s=await start(),w=s.world('world');await w.hold('KeyD',2500);
  const mid=await w.snapshot(),save=resolve(out,'leaves-150.world');await w.save(save);
  await w.run(2500);const end=await w.snapshot();
  say(`PIN sprites tick ${end.tick} ${end.hash}`);
  check('pinned cross-host hash at tick 300',end.tick===300&&end.hash==='0x2e3d805eb6c89e55');
  const leaves=end.entities.find(e=>e.name==='leaves').components.Emitter;
  check('one emitter derives 200 live leaves',leaves.state.alive===200&&end.entities.filter(e=>e.components.Emitter).length===1);
  const player=end.entities.find(e=>e.name==='player').components;
  check('character walked and animated saved atlas frames',Math.abs(player.Transform.position[0]-100)<0.001&&player.SpriteAnimation.age===300);
  check('three parallax sprite layers',end.entities.filter(e=>e.name?.startsWith('parallax-')).length===3);
  if(host!=='linux') {
    const path=resolve(out,`sprites-${host}.png`);await s.screenshot(path);
    const image=decodePng(readFileSync(path));
    const samples=JSON.parse(readFileSync(resolve(import.meta.dir,'logic/tests/leaf-pixels.json'),'utf8'));
    const screen=(await s.layout('world:player')).entity.screen;
    // Sprite center is the camera target. Agent rectangles and screenshots share CSS
    // pixels on web; native scale follows the screenshot/viewport ratio.
    const scale=image.width/1280;
    for(const [x,y,z] of samples) {
      const px=Math.round((screen.x+screen.w/2+x*screen.w/24)*scale);
      const py=Math.round((screen.y+screen.h/2-y*screen.h/32)*scale);
      const at=(py*image.width+px)*4, rgb=Array.from(image.data.slice(at,at+3));
      check(`leaf ${z>0?'in front of':'behind'} character at ${px},${py}`,z>0?rgb[0]>rgb[2]:(rgb[2]>100&&rgb[0]<80),rgb);
    }
  }
  checkSteadyResidency((await s.state()).world[0],check,say,host);
  const endSave=resolve(out,'leaves-300.world');await w.save(endSave);await s.close();
  const r=await start(save),rw=r.world('world');check('restore mid-fall snapshot',equal(await rw.snapshot(),mid));
  await rw.run(2500);check('restore continues animation, camera and leaves',equal(await rw.snapshot(),end));
  const final=resolve(out,'leaves-300-restored.world');await rw.save(final);
  check('mid-fall continuation saves byte-identically',readFileSync(final).equals(readFileSync(endSave)));await r.close();
});
