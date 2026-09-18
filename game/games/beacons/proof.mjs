#!/usr/bin/env bun
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check, equal, out, say}) => {
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const walk = async (s, x, z) => {
    for (const [axis, target, positive, negative] of [[0,x,'KeyD','KeyA'],[2,z,'KeyS','KeyW']]) {
      const gap = target - (await s.world('world').position('player'))[axis];
      if (Math.abs(gap) < 0.15) continue;
      // Acceleration and braking contribute a net 0.2667 m deficit per leg.
      const ms = Math.round((Math.abs(gap) + 0.2666667) / 4 * 60) * (1000 / 60);
      await s.world('world').hold(gap > 0 ? positive : negative, ms);
      check('walk comes to rest', await s.world('world').settle());
    }
    const p = await s.world('world').position('player');
    check(`walk reaches (${x}, ${z})`, Math.hypot(p[0]-x,p[2]-z) < 0.2, p);
  };
  const continuation = async s => {
    await s.world('world').run(900);
    await s.world('world').key_up('KeyW');
    await s.world('world').key_up('Space');
    check('glow fully up at one second after E', equal((await s.world('world').get('beacon-1', 'Material')).emissive,[0.7,2.5,3]));
    check('continuation settles', await s.world('world').settle());
    return s.world('world').snapshot();
  };
  let first, original, checkpoint;
  const checkpointFile = resolve(out,'checkpoint.world');
  for (let run=0; run<2; run++) {
    const s = await open();
    const title = await s.tree();
    check('title has focused accessible Play', node(title,'play')?.focused === true && node(title,'play')?.accessibleName === 'Play');
    check('world waits for Play', !node(title,'world'));
    await s.tap('play');
    await s.world('world').hold('KeyW', 1500);
    const p = await s.world('world').position('player');
    check('W exactly 1.5 s: [0, 0.9, -5.3666644] within 1 mm', p.every((v,i)=>Math.abs(v-[0,0.9,-5.3666644][i])<0.001), p);
    check('exactly 90 ticks', (await s.state()).world[0].tick === 90);
    const forward = await s.world('world').snapshot();
    check('W for 1500 ms equals native hash', forward.hash === '0x7379ac5210e92317', forward.hash);
    if (!run) first = forward;
    else check('two runs after W are identical', equal(first,forward));
    await s.world('world').tap('KeyE'); await s.world('world').run(100);
    check('E outside range lights nothing', node(await s.tree(),'hud-beacons')?.props.text === 'Beacons 0 / 3');
    check('brakes and camera settle', await s.world('world').settle());
    await walk(s,8,0);
    await s.world('world').tap('KeyE'); await s.world('world').run(100);
    const beacon = await s.world('world').get('beacon-1', 'Beacon');
    const material = await s.world('world').get('beacon-1', 'Material');
    check('first beacon lit', beacon.lit === true);
    check('glow partway up after 0.1 s', material.emissive[2] > 0 && material.emissive[2] < 3, material.emissive);
    const hud = node(await s.tree(),'hud-beacons');
    check('HUD reads Beacons 1 / 3, polite live region', hud?.props.text === 'Beacons 1 / 3' && hud?.props.accessibilityLive === 'polite');
    await s.world('world').key_down('KeyW');
    await s.world('world').key_down('Space');
    if (!run) {
      checkpoint = await s.world('world').snapshot();
      await s.world('world').save(checkpointFile);
    }
    const endpoint = await continuation(s);
    const savedPath = resolve(out,`run-${run}.world`);
    await s.world('world').save(savedPath);
    if (!run) original = endpoint;
    else {
      check('two full runs end in identical state', equal(original,endpoint));
      check('two full runs have byte-identical saves', readFileSync(resolve(out,'run-0.world')).equals(readFileSync(savedPath)));
    }
    await s.tap('pause');
    const before = await s.world('world').snapshot();
    await s.world('world').key_down('KeyW'); await s.world('world').run(2000);
    check('paused two seconds with W held: all entities and tick unchanged', equal(before,await s.world('world').snapshot()));
    await s.world('world').key_up('KeyW');
    check('pause button now accessible Resume', node(await s.tree(),'pause')?.accessibleName === 'Resume');
    if (!run) await s.screenshot(resolve(out,'beacons-playing.png'));
    await s.tap('pause');
    if (!run) {
      await walk(s,-6,7); await s.world('world').tap('KeyE'); await s.world('world').run(600);
      await walk(s,3,-9); await s.world('world').tap('KeyE'); await s.world('world').run(600);
      const won = await s.tree();
      check('three lit and centred victory UI', node(won,'hud-beacons')?.props.text === 'Beacons 3 / 3' && !!node(won,'victory') && node(won,'again')?.accessibleName === 'Play again');
      await s.tap('again');
      check('Play again resets player and count', equal(await s.world('world').position('player'),[0,0.9,0]) && node(await s.tree(),'hud-beacons')?.props.text === 'Beacons 0 / 3');
    }
    const logs = await s.logs();
    check('no browser or GPU errors', !logs.host?.some(l=>/^(exception:|console\.error:|error:)/.test(l)), logs.host);
    await s.close();
  }
  say('Original sessions closed; restoring into a fresh process.');
  const restored = await open({world:checkpointFile});
  await restored.tap('play');
  check('fresh process reports restored', (await restored.state()).world[0].restored === true);
  check('restored checkpoint has every saved entity', equal(checkpoint,await restored.world('world').snapshot()));
  check('restored continuation matches original', equal(original,await continuation(restored)));
  const restoredFile = resolve(out,'restored.world');
  await restored.world('world').save(restoredFile);
  check('complete continuation save is byte-identical', readFileSync(resolve(out,'run-0.world')).equals(readFileSync(restoredFile)));
  await restored.close();
});
