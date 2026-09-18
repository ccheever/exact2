#!/usr/bin/env bun
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check, equal, out, say}) => {
  const node = (t, id) => t.nodes.find(n => n.props?.testId === id);
  const pos = s => s.entity.components.Transform.position;
  const components = s => s.entity.components;
  const near = (a, b, epsilon = 0.001) => a.length === b.length && a.every((v, i) => Math.abs(v - b[i]) <= epsilon);
  const go = async (s, x, z) => {
    await s.clock('settle');
    // At 4 m/s, accel 12 and brake 20 lose four travel ticks per stop.
    for (const [axis, target, positive, negative] of [[0,x,'KeyD','KeyA'],[2,z,'KeyS','KeyW']]) {
      const delta = target - pos(await s.world('world').state('player'))[axis];
      if (Math.abs(delta) > 0.05) await s.world('world').hold(delta > 0 ? positive : negative, Math.round(Math.abs(delta) / 4 * 60 + 4) * 1000 / 60);
      await s.clock('settle');
    }
    check(`walked near (${x}, ${z})`, near(pos(await s.world('world').state('player')), [x,0.9,z], 0.08));
  };
  const firstRun = async (index) => {
    const s = await open();
    check('title exposes Play and no world', !!node(await s.tree(), 'play') && !node(await s.tree(), 'world'));
    await s.tap('play');
    check('initial HUD is Beacons 0 / 3', node(await s.tree(), 'hud-beacons')?.props.text === 'Beacons 0 / 3');
    const initial = await s.world('world').snapshot();
    const input = await s.world('world').key('KeyW', {phase:'down'});
    check('movement uses browser input', input.delivery === 'platform');
    await s.clock('+1500');
    check('W exactly 1.5s: [0, 0.9, -5.3666644] within 1 mm', near(pos(await s.world('world').state('player')), [0,0.9,-5.3666644]));
    check('W for 1500 ms equals native pinned hash', (await s.world('world').snapshot()).hash === '0x7b36bcac0b3dfb2e');
    check('1.5s is exactly 90 ticks', (await s.state()).world[0].tick === 90);
    await s.world('world').key('KeyW', {phase:'up'});
    await go(s, 8, 0);
    await s.world('world').key('KeyE');
    await s.clock('+16.666667'); // Deliver E on its tick; glow starts at zero.
    await s.clock('+100');
    const beacon = components(await s.world('world').state('beacon-1'));
    check('first beacon lit, glow exactly 0.104 at 0.1s', beacon.Beacon.lit && beacon.Material.emissive[0] / 2 === 0.104, beacon.Material.emissive);
    check('lit count is 1 in published HUD', node(await s.tree(), 'hud-beacons')?.props.text === 'Beacons 1 / 3');
    await s.clock('+900');
    check('glow fully up at 1s', equal(components(await s.world('world').state('beacon-1')).Material.emissive, [2,2.5,2]));
    await s.tap('pause');
    check('Pause now reads Resume', (await s.tree()).nodes.some(n => n.props?.text === 'Resume'));
    const frozen = await s.world('world').snapshot();
    await s.world('world').key('KeyW', {phase:'down'});
    await s.clock('+2000');
    check('paused 2 seconds: every entity component and tick unchanged', equal(frozen, await s.world('world').snapshot()));
    await s.world('world').key('KeyW', {phase:'up'});
    await s.tap('pause');
    const state = await s.world('world').snapshot();
    const file = resolve(out, `repeat-${index}.world`);
    await s.world('world').save(file);
    if (index === 1) {
      await s.screenshot(resolve(out, 'beacons-playing.png'));
      check('player projects into the viewport', !!(await s.layout('world:player')).entity.screen);
    }
    await s.close();
    return {initial, state, bytes:readFileSync(file)};
  };
  const a = await firstRun(1), b = await firstRun(2);
  check('two identical scripts: same initial scene and final entity state', equal(a.initial,b.initial) && equal(a.state,b.state));
  check('two identical scripts: byte-identical complete simulation saves', a.bytes.equals(b.bytes));

  let s = await open();
  await s.tap('play');
  await go(s, 8, 0);
  await s.world('world').key('KeyE');
  await s.clock('+100');
  await s.world('world').key('KeyW', {phase:'down'});
  await s.world('world').key('Space', {phase:'down'});
  const checkpoint = resolve(out, 'checkpoint.world');
  const checkpointState = await s.world('world').snapshot();
  const saved = await s.world('world').save(checkpoint);
  const continueGame = async () => {
    await s.clock('+500');
    check('queued saved jump executes', pos(await s.world('world').state('player'))[1] > 2.08);
    await s.world('world').key('Space', {phase:'up'});
    await s.clock('+1000');
    await s.world('world').key('KeyW', {phase:'up'});
    return await s.world('world').snapshot();
  };
  const original = await continueGame();
  await s.world('world').save(resolve(out, 'continued-original.world'));
  await s.close();
  say('Original process closed; restoring checkpoint into a fresh browser process.');
  s = await open({world:checkpoint});
  await s.tap('play');
  const restored = (await s.state()).world[0];
  check('fresh process restores the exact checkpoint', restored.restored && restored.tick === saved.tick && restored.hash === saved.hash);
  check('restore preserves the simulation snapshot', equal(checkpointState, await s.world('world').snapshot()));
  check('saved publication restores HUD', node(await s.tree(), 'hud-beacons')?.props.text === 'Beacons 1 / 3');
  const continued = await continueGame();
  await s.world('world').save(resolve(out, 'continued-restored.world'));
  check('restored continuation has identical full entity state', equal(original,continued));
  check('restored continuation save is byte-identical', readFileSync(resolve(out, 'continued-original.world')).equals(readFileSync(resolve(out, 'continued-restored.world'))));
  await go(s, -6, 7);
  await s.world('world').key('KeyE'); await s.clock('+1000');
  await go(s, 3, -9);
  await s.world('world').key('KeyE'); await s.clock('+1000');
  check('all three lit: win text and Play again', (await s.tree()).nodes.some(n => n.props?.text === 'All beacons lit') && !!node(await s.tree(), 'play-again'));
  await s.tap('play-again');
  check('Play again resets position and count', near(pos(await s.world('world').state('player')), [0,0.9,0]) && node(await s.tree(), 'hud-beacons')?.props.text === 'Beacons 0 / 3');
  // Real keyboard activation of a focused button, not synthetic world input.
  await s.type('pause', {key:'Space'});
  check('focused Pause button is keyboard operable', (await s.state()).world[0].paused);
  const logs = await s.logs();
  check('no browser/GPU errors', !logs.host?.some(line => /^(exception:|console\.error:|error:)/.test(line)), logs.host);
  await s.close();
});
