#!/usr/bin/env bun
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {proof} from '../../proof.mjs';

await proof(import.meta, async ({pin, pinSave, open, check, equal, out, host, say}) => {
  const node = (t,id) => t.nodes.find(n => n.props?.testId === id);
  const bytes = name => readFileSync(resolve(out,name));
  const walk = async (g,x,z) => {
    // From rest, acceleration and braking lose about 0.267m vs constant speed.
    for (const [axis,target,negative,positive] of [[0,x,'KeyA','KeyD'],[2,z,'KeyW','KeyS']]) {
      const delta = target - (await g.global_position('player'))[axis];
      if (Math.abs(delta) > 0.15) await g.hold(delta < 0 ? negative : positive, Math.round((Math.abs(delta)+0.267)/4*1000));
      check('walking settles', await g.settle());
    }
  };
  const continuation = async s => {
    const g = s.world('world');
    await g.run(900);
    await g.key_up('Space'); await g.key_up('KeyW');
    check('glow complete at +1s', equal((await g.get('beacon-1','Material')).emissive,[3,3,3]));
    check('continuation settles', await g.settle());
    return g.snapshot();
  };
  const run = async index => {
    const s = await open(), title = await s.tree();
    check('title, accessible Play and keyboard focus', title.nodes.some(n=>n.props?.text==='Beacons') && node(title,'play')?.accessibleName==='Play' && node(title,'play')?.focused===true);
    check('world deferred until Play', !node(title,'world'));
    await s.tap('play');
    const g = s.world('world');
    await g.hold('KeyW',1500);
    const p = await g.global_position('player');
    check('W exactly 1.5s: [0, 0.9, -5.3666644] within 1mm', p.every((v,i)=>Math.abs(v-[0,0.9,-5.3666644][i])<=0.001),p);
    check('exactly 90 ticks', (await s.state()).world[0].tick===90);
    check('capsule dimensions', equal(await g.get('player','Mesh'),{Capsule:{height:1.8,radius:0.4}}));
    await g.tap('KeyE'); await g.run(100);
    check('outside radius cannot light', !(await g.get('beacon-1','Beacon')).lit);
    await g.settle(); await walk(g,8,0);
    check('proximity prompt', !!node(await s.tree(),'near-prompt'));
    await g.tap('KeyE'); await g.run(100);
    check('first beacon is lit', (await g.get('beacon-1','Beacon')).lit);
    const hud = node(await s.tree(),'hud-lit');
    check('HUD Beacons 1 / 3 is real live text', hud?.props.text==='Beacons 1 / 3' && hud.props.accessibilityLive==='polite');
    const glow = (await g.get('beacon-1','Material')).emissive;
    check('glow partial +100ms', glow.every(v=>v>0 && v<3),glow);
    await g.key_down('KeyW'); await g.key_down('Space');
    const checkpoint = await g.snapshot();
    await g.save(resolve(out,`checkpoint-${index}.world`));
    const final = await continuation(s);
    await g.save(resolve(out,`final-${index}.world`));
    await s.tap('pause');
    check('Pause becomes accessible Resume', node(await s.tree(),'pause')?.accessibleName==='Resume');
    const paused = await g.snapshot();
    await g.key_down('KeyW'); await g.run(2000);
    check('pause freezes entire simulation for 2s despite held W',equal(paused,await g.snapshot()));
    await g.key_up('KeyW');
    if (index===1 && host !== 'linux') {
      await s.screenshot(resolve(out,'beacons.png'));
      const layout = await s.layout('world:player');
      check('rendered player has screen bounds',layout.entity?.screen?.w>0 && layout.entity.screen.h>0);
    }
    await s.tap('pause');
    const logs=await s.logs();
    check('no browser or GPU errors',!logs.host?.some(l=>/^(exception:|console\.error:|error:)/.test(l)),logs.host);
    await s.close();
    return {checkpoint,final};
  };
  const first=await run(1), second=await run(2);
  pin(907, first.final);
  pinSave("continuation", resolve(out, "final-1.world"));
  check('two full input runs produce identical state',equal(first.final,second.final));
  check('two full runs produce identical save bytes',bytes('final-1.world').equals(bytes('final-2.world')));
  say('Original host processes closed; restoring mid-glow with W held and jump queued in a fresh process.');
  const s=await open({world:resolve(out,'checkpoint-1.world')});
  await s.tap('play');
  const g=s.world('world');
  check('fresh process reports restored',(await s.state()).world[0].restored===true);
  check('restored checkpoint exact',equal(first.checkpoint,await g.snapshot()));
  check('fresh-process continuation exact',equal(first.final,await continuation(s)));
  await g.save(resolve(out,'restored.world'));
  check('restored complete save byte-identical',bytes('final-1.world').equals(bytes('restored.world')));
  await walk(g,-6,7); await g.tap('KeyE'); await g.run(1000);
  await walk(g,3,-9); await g.tap('KeyE'); await g.run(1000);
  const win=await s.tree();
  check('all three light and victory appears',node(win,'hud-lit')?.props.text==='Beacons 3 / 3' && win.nodes.some(n=>n.props?.text==='All beacons lit') && node(win,'again')?.accessibleName==='Play again');
  check('victory control takes logical focus', node(win,'again')?.focused === true && (await s.state()).focus.logical === node(win,'again')?.id);
  await s.tap('again');
  check('restart is visible in state.world', (await s.state()).world[0].restarted === 1);
  check('Play again resets position',equal(await g.global_position('player'),[0,0.9,0]));
  check('Play again resets HUD',node(await s.tree(),'hud-lit')?.props.text==='Beacons 0 / 3');
  await s.close();
});
