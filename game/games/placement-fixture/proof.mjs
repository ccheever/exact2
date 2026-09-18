#!/usr/bin/env bun
import {proof} from '../../proof.mjs';
import {resolve} from 'node:path';
await proof(import.meta,async ({open,check,equal,out,host,say})=>{
  if(process.argv.includes('--capture40')) {
    const s=await open({size:[1280,720]});await s.tap('crowd');
    await s.clock('+0');await s.screenshot(resolve(out,`forty-${host}.png`));
    const logs=await s.logs();say(JSON.stringify(logs));
    check('40 placed children',(await s.world('world').snapshot()).entities.filter(e=>e.components.Placed).length===40);
    await s.close();return;
  }
  const start=async world=>{const s=await open({size:[1280,720],...world?{world}:{}});await s.tap('play');return s;};
  const s=await start(),w=s.world('world');
  const node=(tree,id)=>tree.nodes.find(n=>n.props?.testId===id);
  const box=async id=>(await s.layout(id)).nodes.find(n=>n.testId===id);
  const hud=await box('hud');
  const first=await box('sign');
  check('three saved placements', (await w.snapshot()).entities.filter(e=>e.components.Placed).length===3);
  if(host!=='linux') {
    check('sign has explicit visible placement', (await s.state('world:sign')).entity.placed.hidden===false);
    const tap=await s.tap('pull');check('Pull taps through its placed box',!tap.error,tap);
  } else {
    // Linux currently composites the Contract overlay and drives the same bind.
    await s.tap('pull');
  }
  await w.run(1000);
  check('button lights the world lamp', (await s.state()).world[0].resources.Lamp.lit===true);
  check('lamp publication reaches HUD',node(await s.tree(),'hud').props.text==='Lamp true');
  if(host!=='linux') {
    const after=await box('sign');check('orbit moves sign placed box',Math.abs(after.x-first.x)>1,after);
    check('HUD stays at kernel frame',equal(await box('hud'),hud));
    if(host==='macos') {
      const sign=node(await s.tree(),'sign');
      check('accessible sign text uses the placed frame',sign?.props.text==='The lantern path' && Math.abs(sign.accessibilityFrame[2]-after.w)<0.1 && Math.abs(sign.accessibilityFrame[3]-after.h)<0.1 && !sign.accessibilityHidden,sign);
    }
  }
  const mid=await w.snapshot(),save=resolve(out,'placed.world');await w.save(save);
  await w.run(4500);
  if(host!=='linux') {
    check('fixed sign explicitly hidden from behind',(await s.state('world:sign')).entity.placed.hidden===true);
    check('layout reports hidden sign',(await s.layout('sign')).node.visible.hidden===true);
    let refusal;try {const r=await s.tap('sign');refusal=r.error;}catch(e){refusal=e.message;}
    check('hidden sign rejects tap with reason',/hidden|inert|not hit|off screen|covered/.test(refusal??''),refusal);
    await s.screenshot(resolve(out,`placed-${host}.png`));
  }
  const end=await w.snapshot();say(`PIN placement tick ${end.tick} ${end.hash}`);
  check('tick-330 cross-host pin',end.tick===330 && end.hash==='0x61007363bd681d3c');
  await s.close();
  const r=await start(save),rw=r.world('world');
  check('restore carries every component, no outcomes',equal(await rw.snapshot(),mid));
  await rw.run(4500);check('restored orbit keeps same hash',equal(await rw.snapshot(),end));
  await r.close();
});
