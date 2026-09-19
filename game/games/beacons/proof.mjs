#!/usr/bin/env bun
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {proof} from '../../proof.mjs';

// Build products stay beside this game, including in callers with a shared target.
process.env.CARGO_TARGET_DIR = resolve(import.meta.dir, 'target');
await proof(import.meta, async ({open, check, equal, out, host, pin, pinSave, say}) => {
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const world = s => s.world('world');
  const snapshot = s => world(s).snapshot();
  const position = s => world(s).local_position('player');
  const moveTo = async (s, x, z) => {
    await world(s).settle();
    for (const [axis, target, plus, minus] of [[0,x,'KeyD','KeyA'], [2,z,'KeyS','KeyW']]) {
      const delta = target - (await position(s))[axis];
      if (Math.abs(delta) < 0.15) continue;
      // From rest, acceleration/braking lose 0.2667 m relative to 4 m/s × held time.
      const ms = Math.round((Math.abs(delta) + 0.2666667) / 4 * 60) * 1000 / 60;
      await world(s).hold(delta > 0 ? plus : minus, ms);
      check('walk settles after releasing key', await world(s).settle());
    }
    const p = await position(s);
    check(`walk reaches (${x}, ${z}) within 0.15 m`, Math.hypot(p[0]-x,p[2]-z) < 0.15, p);
  };
  let firstEnd, checkpointState, uninterrupted;
  const checkpoint = resolve(out,'checkpoint.world');
  const original = resolve(out,'original.world');
  for (let run = 0; run < 2; run++) {
    const s = await open();
    const title = await s.tree();
    check('title has focused, accessible Play', node(title,'play')?.focused === true && node(title,'play')?.accessibleName === 'Play');
    check('title has no world', !node(title,'world'));
    await s.tap('play');
    const initial = await snapshot(s);
    pin(0, initial);
    check('six seeded crates', initial.entities.filter(e => /^crate-/.test(e.name)).length === 6);
    check('capsule radius 0.4 height 1.8', equal(await world(s).get('player','Mesh'), {Capsule:{height:1.8,radius:0.4}}));
    await world(s).hold('KeyW',1500);
    const p = await position(s);
    check('W exactly 1.5 s: expected (0, 0.9, -5.3666644), tolerance 1 mm', Math.hypot(p[0],p[1]-0.9,p[2]+5.3666644) < 0.001,p);
    pin(90,await snapshot(s));
    await world(s).settle();
    await world(s).tap('KeyE'); await world(s).run(100);
    check('E outside range lights nothing', !(await world(s).get('beacon-1','Beacon')).lit);
    await moveTo(s,8,0);
    await world(s).tap('KeyE');
    await world(s).run(100);
    const partial = (await world(s).get('beacon-1','Material')).emissive[0] / 3;
    check('glow is partway at 0.1 s', partial > 0 && partial < 1, partial);
    check('beacon-1 lit', (await world(s).get('beacon-1','Beacon')).lit);
    const hud = node(await s.tree(),'hud-lit');
    check('HUD is Beacons 1 / 3 and a polite live region', hud?.props.text === 'Beacons 1 / 3' && hud?.props.accessibilityLive === 'polite');
    await world(s).run(900);
    check('glow fully up at 1 s', equal((await world(s).get('beacon-1','Material')).emissive,[3,3,3]));
    // Pause while moving and mid-jump, so freezing is not a stationary-world tautology.
    await world(s).key_down('KeyW'); await world(s).tap('Space'); await world(s).run(100);
    await s.tap('pause');
    const paused = await snapshot(s);
    await world(s).run(2000);
    check('Pause freezes every entity and the tick for 2 seconds', equal(paused,await snapshot(s)));
    check('Pause becomes accessible Resume', node(await s.tree(),'pause')?.accessibleName === 'Resume');
    await s.tap('pause');
    await world(s).key_up('KeyW'); await world(s).run(200);
    const end = await snapshot(s);
    if (run === 0) firstEnd = end;
    else check('two full input scripts end in identical state',equal(firstEnd,end));
    if (run === 0) {
      if (host === 'web' || host === 'ios') await s.screenshot(resolve(out,'beacons.png'));
      else say('SKIP 3D pixels on GPU-less Linux; run this proof on web for the screenshot.');
      checkpointState = await snapshot(s);
      await world(s).save(checkpoint);
      await world(s).hold('ArrowLeft',750);
      await world(s).run(1100);
      uninterrupted = await snapshot(s);
      await world(s).save(original);
      pinSave('continuation',original);
    }
    await s.close();
  }
  say('Original processes closed before fresh restore.');
  const restored = await open({world:checkpoint});
  await restored.tap('play');
  check('fresh process restores full mid-jump world', equal(checkpointState,await snapshot(restored)));
  check('restore explicitly acknowledged', (await restored.state()).world[0].restored === true);
  await world(restored).hold('ArrowLeft',750);
  await world(restored).run(1100);
  check('fresh process continues exactly',equal(uninterrupted,await snapshot(restored)));
  const restoredFile = resolve(out,'restored.world');
  await world(restored).save(restoredFile);
  check('continuation saves byte-identical, including input/time',readFileSync(original).equals(readFileSync(restoredFile)));
  await moveTo(restored,-6,7);
  await world(restored).tap('KeyE'); await world(restored).run(600);
  await moveTo(restored,3,-9);
  await world(restored).tap('KeyE'); await world(restored).run(600);
  const won = await restored.tree();
  check('all three light and win UI appears',node(won,'hud-lit')?.props.text === 'Beacons 3 / 3' && !!node(won,'victory'));
  check('Play again has focus and accessible name',node(won,'again')?.focused === true && node(won,'again')?.accessibleName === 'Play again');
  await restored.tap('again');
  check('restart clears count and resets player',node(await restored.tree(),'hud-lit')?.props.text === 'Beacons 0 / 3' && equal(await position(restored),[0,0.9,0]));
  const logs = await restored.logs();
  check('no host exceptions', !(logs.host ?? []).some(line => /^(exception:|console\.error:|error:)/.test(line)));
  await restored.close();
});
