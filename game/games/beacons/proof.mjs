#!/usr/bin/env bun
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check, equal, out, say}) => {
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const entity = async (s, name) => (await s.state(`world:${name}`)).entity.components;
  const position = async s => (await entity(s, 'player')).Transform.position;
  // Snapshot reply also has carrier clock/epoch; compare every simulation component.
  const snapshot = async s => {
    const r = await s.state('world:*');
    check('entity snapshot is complete', !r.truncated);
    return {tick:r.tick, entities:r.entities};
  };
  const walk = async (s, x, z) => {
    for (const [axis, target, positive, negative] of [[0,x,'KeyD','KeyA'],[2,z,'KeyS','KeyW']]) {
      const gap = target - (await position(s))[axis];
      if (Math.abs(gap) < 0.15) continue;
      // Acceleration and braking contribute a net 0.2667 m deficit per leg.
      const ms = Math.round((Math.abs(gap) + 0.2666667) / 4 * 60) * (1000 / 60);
      await s.type('world', {key:gap > 0 ? positive : negative, for:ms});
      check('walk comes to rest', (await s.clock('settle')).settled === true);
    }
    const p = await position(s);
    check(`walk reaches (${x}, ${z})`, Math.hypot(p[0]-x,p[2]-z) < 0.2, p);
  };
  const continuation = async s => {
    await s.clock('+900');
    await s.type('world', {key:'KeyW',phase:'up'});
    await s.type('world', {key:'Space',phase:'up'});
    check('glow fully up at one second after E', equal((await entity(s,'beacon-1')).Material.emissive,[0.7,2.5,3]));
    check('continuation settles', (await s.clock('settle')).settled === true);
    return snapshot(s);
  };
  let first, original, checkpoint;
  const checkpointFile = resolve(out,'checkpoint.world');
  for (let run=0; run<2; run++) {
    const s = await open();
    const title = await s.tree();
    check('title has focused accessible Play', node(title,'play')?.focused === true && node(title,'play')?.accessibleName === 'Play');
    check('world waits for Play', !node(title,'world'));
    await s.tap('play');
    await s.type('world',{key:'KeyW',for:1500});
    const p = await position(s);
    check('W exactly 1.5 s: [0, 0.9, -5.3666644] within 1 mm', p.every((v,i)=>Math.abs(v-[0,0.9,-5.3666644][i])<0.001), p);
    check('exactly 90 ticks', (await s.state()).world[0].tick === 90);
    const forward = await snapshot(s);
    if (!run) first = forward;
    else check('two runs after W are identical', equal(first,forward));
    await s.type('world',{key:'KeyE'}); await s.clock('+100');
    check('E outside range lights nothing', node(await s.tree(),'hud-beacons')?.props.text === 'Beacons 0 / 3');
    check('brakes and camera settle', (await s.clock('settle')).settled === true);
    await walk(s,8,0);
    await s.type('world',{key:'KeyE'}); await s.clock('+100');
    const beacon = await entity(s,'beacon-1');
    check('first beacon lit', beacon.Beacon.lit === true);
    check('glow partway up after 0.1 s', beacon.Material.emissive[2] > 0 && beacon.Material.emissive[2] < 3, beacon.Material.emissive);
    const hud = node(await s.tree(),'hud-beacons');
    check('HUD reads Beacons 1 / 3, polite live region', hud?.props.text === 'Beacons 1 / 3' && hud?.props.accessibilityLive === 'polite');
    await s.type('world',{key:'KeyW',phase:'down'});
    await s.type('world',{key:'Space',phase:'down'});
    if (!run) {
      checkpoint = await snapshot(s);
      await s.screenshot(checkpointFile,'world','save');
    }
    const endpoint = await continuation(s);
    const savedPath = resolve(out,`run-${run}.world`);
    await s.screenshot(savedPath,'world','save');
    if (!run) original = endpoint;
    else {
      check('two full runs end in identical state', equal(original,endpoint));
      check('two full runs have byte-identical saves', readFileSync(resolve(out,'run-0.world')).equals(readFileSync(savedPath)));
    }
    await s.tap('pause');
    const before = await snapshot(s);
    await s.type('world',{key:'KeyW',phase:'down'}); await s.clock('+2000');
    check('paused two seconds with W held: all entities and tick unchanged', equal(before,await snapshot(s)));
    await s.type('world',{key:'KeyW',phase:'up'});
    check('pause button now accessible Resume', node(await s.tree(),'pause')?.accessibleName === 'Resume');
    if (!run) await s.screenshot(resolve(out,'beacons-playing.png'));
    await s.tap('pause');
    if (!run) {
      await walk(s,-6,7); await s.type('world',{key:'KeyE'}); await s.clock('+600');
      await walk(s,3,-9); await s.type('world',{key:'KeyE'}); await s.clock('+600');
      const won = await s.tree();
      check('three lit and centred victory UI', node(won,'hud-beacons')?.props.text === 'Beacons 3 / 3' && !!node(won,'victory') && node(won,'again')?.accessibleName === 'Play again');
      await s.tap('again');
      check('Play again resets player and count', equal(await position(s),[0,0.9,0]) && node(await s.tree(),'hud-beacons')?.props.text === 'Beacons 0 / 3');
    }
    const logs = await s.logs();
    check('no browser or GPU errors', !logs.host?.some(l=>/^(exception:|console\.error:|error:)/.test(l)), logs.host);
    await s.close();
  }
  say('Original browsers closed; restoring into a fresh browser process.');
  const restored = await open({world:checkpointFile});
  await restored.tap('play');
  check('fresh process reports restored', (await restored.state()).world[0].restored === true);
  check('restored checkpoint has every saved entity', equal(checkpoint,await snapshot(restored)));
  check('restored continuation matches original', equal(original,await continuation(restored)));
  const restoredFile = resolve(out,'restored.world');
  await restored.screenshot(restoredFile,'world','save');
  check('complete continuation save is byte-identical', readFileSync(resolve(out,'run-0.world')).equals(readFileSync(restoredFile)));
  await restored.close();
});
