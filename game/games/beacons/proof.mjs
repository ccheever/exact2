#!/usr/bin/env bun
import {readFileSync, writeFileSync} from 'node:fs';
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
    if (host !== 'linux') {
      const limits = (await s.state()).world[0].gpu;
      check('renderer storage needs fit this host device', limits.requiredStorageBindings <= limits.storageBindings, limits);
    }
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
    if (host !== 'linux') await s.op({op:'state', ...await s.target('world'), world:true, perf_reset:true});
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
    if (index === 1 && host !== 'linux') {
      const measured = (await s.state()).world[0];
      writeFileSync(resolve(out, `perf-${host}.json`), JSON.stringify({unsampled:true, perf:measured.perf, gpu:measured.gpu}, null, 2)+'\n');
      say(`PERF ${host} seekable counters (timing rings are unsampled, not zero-cost frames): ${JSON.stringify(measured.perf)}`);
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
  const controls = await open();
  await controls.tap('play');
  const cg = controls.world('world'), ct = await controls.tree();
  check('Contract controls are named and discoverable', ['move','jump','light'].every(id => node(ct,id)?.props.action === id) && node(ct,'jump')?.accessibleName === 'Jump' && node(ct,'light')?.accessibleName === 'Light');
  await controls.tap('jump', {down:true});
  check('held control is visible before its tick', (await controls.state()).world[0].input.forwardedControls.includes('jump'));
  await cg.run(100);
  check('Jump control changes player height through gpu_input', (await cg.global_position('player'))[1] > 1);
  const pointerSave=resolve(out,'pointer-control.world'); await cg.save(pointerSave);
  await controls.pointer('cancel'); await cg.settle();
  // A keyboard-activated Control has the same ID/local origin on every host;
  // physical touch IDs and font-dependent button centers are host observations.
  await controls.type('jump',{key:'Space',phase:'down'}); await cg.run(100);
  const heldSave = resolve(out,'held-control.world'); await cg.save(heldSave);
  check('held contact is visible after ticks', (await controls.state()).world[0].input.controls.includes('jump'));
  await controls.type('jump',{key:'Space',phase:'up'}); await cg.settle();
  await controls.tap('move', {down:true}); await controls.pointer('move',{dx:60,dy:0}); await cg.run(500); await controls.pointer('up');
  check('local-origin stick moves right', (await cg.global_position('player'))[0] > 1);
  await cg.settle(); await walk(cg,8,0); await controls.tap('light'); await cg.run(100);
  check('Light control lights a beacon through gpu_input', (await cg.get('beacon-1','Beacon')).lit);
  await controls.type('jump',{key:'Space',phase:'down'}); await cg.run(100); await controls.type('jump',{key:'Space',phase:'up'});
  check('focused control keyboard activation feeds Jump', (await cg.global_position('player'))[1] > 1);
  await controls.close();
  const restoredPointer=await open({world:pointerSave}); await restoredPointer.tap('play');
  await restoredPointer.pointer('cancel');
  check('fresh host releases saved pointer ownership',(await restoredPointer.state()).world[0].input.forwardedControls.length===0);
  await restoredPointer.close();
  const held = await open({world:heldSave}); await held.tap('play');
  check('fresh host restores held control', (await held.state()).world[0].input.controls.includes('jump'));
  await held.type('jump',{key:'Space',phase:'up'});
  check('restored Space release clears the original contact',(await held.state()).world[0].input.forwardedControls.length===0);
  await held.world('world').run(100);
  check('restored controls empty after tick',(await held.state()).world[0].input.controls.length===0);
  await held.close();
  if (host !== 'linux' && process.env.EXACT_PROOF_COMPARE !== '1') {
    const pointer = await open(); await pointer.tap('play');
    const pg = pointer.world('world');
    const delivered = await pointer.tap('world',{down:true,at:[640,360]});
    await pg.run(100);
    check('raw pointer changes gameplay via input.pointer()', (await pg.global_position('player'))[1] > 1, {delivery:delivered.delivery});
    await pointer.pointer('up'); await pointer.close();
  }
  // Live/host-only observations have no deterministic cross-host save identity.
  if (host === 'ios' && process.env.EXACT_PROOF_COMPARE !== '1') {
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
    check('live simulator produces timing samples', measured.perf?.frameMs?.count >= 10, measured.perf);
    const presentation = {...measured.presentation, renders:measured.presentation.sessionRenders-started.sessionRenders, captures:measured.presentation.sessionCaptures-started.sessionCaptures};
    check('live presentation reports HUD and placement counts', presentation.renders > 0 && presentation.hudChildren > 0, presentation);
    const receipt = {label:'iOS simulator CPU/presentation timings; no real GPU timing', perf:measured.perf, gpu:measured.gpu, presentation};
    writeFileSync(resolve(out, 'perf-ios-live.json'), JSON.stringify(receipt,null,2)+'\n');
    say(`LIVE SIMULATOR ${JSON.stringify(receipt)}`);
    await live.close();
  }

});
