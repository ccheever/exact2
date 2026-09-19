#!/usr/bin/env bun
import {proof} from '../../proof.mjs';
import {decodePng} from '../../../scripts/png.mjs';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
await proof(import.meta,async ({pin, pinSave, open,check,equal,out,host,say})=>{
  if(process.argv.includes('--capture40')) {
    const s=await open({size:[1280,720]});await s.tap('crowd');
    await s.clock('+0');await s.screenshot(resolve(out,`forty-${host}.png`));
    const logs=await s.logs();say(JSON.stringify(logs));
    say('40-child capture is visual evidence; no CPU-cost claim is made.');
    check('40 placed children',(await s.world('world').snapshot()).entities.filter(e=>e.components.Placed).length===40);
    await s.close();return;
  }
  const start=async world=>{const s=await open({size:[1280,720],...world?{world}:{}});await s.tap('play');return s;};
  const s=await start(),w=s.world('world');
  const node=(tree,id)=>tree.nodes.find(n=>n.props?.testId===id);
  const box=async id=>(await s.layout(id)).nodes.find(n=>n.testId===id);
  const covered=await box('hud'), pull=await box('pull');
  const overlap=[pull.x+pull.w/2,pull.y+pull.h-5];
  check('HUD overlaps Pull',overlap[0]>covered.x && overlap[0]<covered.x+covered.w && overlap[1]>covered.y && overlap[1]<covered.y+covered.h,{covered,pull});
  const shot=resolve(out,`hud-over-pull-${host}.png`);await s.screenshot(shot);
  const image=decodePng(readFileSync(shot)), scale=image.width/1280;
  const pixel=(Math.floor(overlap[1]*scale)*image.width+Math.floor(overlap[0]*scale))*4;
  const rgb=Array.from(image.data.slice(pixel,pixel+3));
  check('HUD paints over Pull',rgb.every((v,i)=>Math.abs(v-[32,39,49][i])<=3),rgb);
  const hudTap=await s.tap('hud');
  check('HUD wins overlapping hit',!hudTap.error && (await box('hud')).x===24,hudTap);
  check('HUD tap did not pull',!(await s.state()).world[0].resources.Lamp.lit);
  const hud=await box('hud');
  const first=await box('sign');
  const tuples={initial:Object.fromEntries(['x','y','w','h'].map(k=>[k,first[k]]))};
  const firstAX=node(await s.tree(),'sign')?.accessibilityFrame;
  const oracle={x:430.91,y:299.76,w:140.23,h:30.55};
  check('placed sign matches Placed::project within 0.5 px at 1280x720',Object.entries(oracle).every(([k,v])=>Math.abs(first[k]-v)<=0.5),first);
  await s.tap('hud');await s.clock('+0');
  check('zero-sized child is explicitly hidden',(await s.state('world:sign')).entity.placed.hidden===true);
  const signId=node(await s.tree(),'sign')?.id ?? first.id;
  const hiddenLayout=await s.op({op:'layout',id:signId});
  const hiddenBox=hiddenLayout.nodes.find(n=>n.id===signId) ?? hiddenLayout.node?.space?.viewport;
  check('hidden placement reports a zero box',!!hiddenBox && ['x','y','w','h'].every(k=>hiddenBox[k]===0),hiddenLayout);
  await s.tap('hud');await s.clock('+0');
  check('restored child has a fresh visible capture',(await s.state('world:sign')).entity.placed.hidden===false);
  check('three saved placements', (await w.snapshot()).entities.filter(e=>e.components.Placed).length===3);
  {
    check('sign has explicit visible placement', (await s.state('world:sign')).entity.placed.hidden===false);
    const tap=await s.tap('pull');check('Pull taps through its placed box',!tap.error,tap);
  }
  await w.run(1000);
  check('button lights the world lamp', (await s.state()).world[0].resources.Lamp.lit===true);
  check('lamp publication reaches HUD',node(await s.tree(),'hud-label').props.text==='Lamp true');
  {
    const after=await box('sign');
    tuples.moving=Object.fromEntries(['x','y','w','h'].map(k=>[k,after[k]]));
    writeFileSync(resolve(out,`placement-${host}.json`),JSON.stringify(tuples,null,2)+'\n');
    // Paranoid reconstruction primes current/current history every tick. Ordinary
    // playback displays tick 59 at alpha zero; reconstruction displays tick 60.
    // Both receipts come from logic/tests/sim.rs and keep the same 0.5 px limit.
    const projected=['1','fresh-game'].includes(process.env.EXACT_GAME_PARANOID)?{x:494.20934,y:284.9197,w:92.1972,h:38.18921}:{x:492.88443,y:285.10403,w:93.112885,h:38.104492};
    check('moving displayed sign matches sampled Placed::project within 0.5 px',Object.entries(projected).every(([k,v])=>Math.abs(after[k]-v)<=0.5),after);
    check('orbit moves sign placed box',Math.abs(after.x-first.x)>1,after);
    check('HUD stays at kernel frame',equal(await box('hud'),hud));
    if(host==='macos') {
      const sign=node(await s.tree(),'sign');
      check('accessible sign text uses the placed frame',sign?.props.text==='The lantern path' && Math.abs(sign.accessibilityFrame[2]-after.w)<0.1 && Math.abs(sign.accessibilityFrame[3]-after.h)<0.1 && Math.abs((sign.accessibilityFrame[0]-firstAX[0])-(after.x-first.x))<0.1 && Math.abs((sign.accessibilityFrame[1]-firstAX[1])+(after.y+after.h-first.y-first.h))<0.1 && !sign.accessibilityHidden,sign);
    }
  }
  const mid=await w.snapshot(),save=resolve(out,'placed.world');await w.save(save);
  await w.run(4500);
  const pinnedSave=resolve(out,"continuation.world"); await w.save(pinnedSave); pinSave("continuation",pinnedSave);
  {
    check('fixed sign explicitly hidden from behind',(await s.state('world:sign')).entity.placed.hidden===true);
    const backLayout=await s.op({op:'layout',id:first.id});
    const backBox=backLayout.nodes.find(n=>n.id===first.id) ?? backLayout.node?.space?.viewport;
    check('back-face hidden child also reports a zero box',!!backBox && ['x','y','w','h'].every(k=>backBox[k]===0),backBox);
    if(host==='linux') check('hidden sign leaves the tree', !node(await s.tree(),'sign'));
    else check('layout reports hidden sign',(await s.layout('sign')).node.visible.hidden===true);
    let refusal;try {const r=await s.tap('sign');refusal=r.error;}catch(e){refusal=e.message;}
    check('hidden sign rejects tap with reason',/hidden|inert|not hit|off screen|covered|no .*sign|not found|matched 0/.test(refusal??''),refusal);
    await s.screenshot(resolve(out,`placed-${host}.png`));
  }
  const end=await w.snapshot();say(`PIN placement tick ${end.tick} ${end.hash}`);
  pin(330, end);
  await s.close();
  const r=await start(save),rw=r.world('world');
  check('restore carries every component, no outcomes',equal(await rw.snapshot(),mid));
  await rw.run(4500);check('restored orbit keeps same hash',equal(await rw.snapshot(),end));
  await r.close();
});
