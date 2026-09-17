#!/usr/bin/env bun
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check, equal, out, host, say}) => {
const node = (tree, id) => tree?.nodes?.find(n => n.props?.testId === id);
const position = state => state?.entity?.components?.Transform?.position;
async function run(number) {
  const s = await open();
  const trace = [];
    const entity = async name => (await s.state(`world:${name}`)).entity.components;
    const pos = async () => (await entity('player')).Transform.position;
    async function snapshot(label) {
      const state = await s.state();
      const world = state.world[0];
      const entities = Object.fromEntries((await s.state('world:*')).entities.map(e => [e.name, e.components]));
      const snap = {label, hash:world.hash, tick:world.tick, entities};
      trace.push(snap);
      return snap;
    }
    const key = (code, phase) => s.type('world', {key:code, ...(phase ? {phase} : {})});
    async function walk(x, z) {
      for (const [axis, target, positive, negative] of [[0,x,'KeyD','KeyA'],[2,z,'KeyS','KeyW']]) {
        const p = await pos(), delta = target-p[axis];
        if (Math.abs(delta) > 0.04) {
          await s.type('world', {key:delta > 0 ? positive : negative, for:Math.round(Math.abs(delta)*15)*1000/60});
          await s.clock('settle');
        }
      }
      const p = await pos();
      check(`run ${number}: walked to (${x}, ${z})`, Math.hypot(p[0]-x,p[2]-z)<0.15, p);
    }
    let tree = await s.tree();
    check(`run ${number}: title + Play`, !!node(tree,'play') && tree.nodes.some(n=>n.props?.text==='Beacons'));
    await s.tap('play');
    tree = await s.tree();
    check(`run ${number}: initial HUD`, node(tree,'hud-beacons')?.props.text==='Beacons 0 / 3');
    const delivery = await key('KeyW','down');
    check(`run ${number}: real browser key path`, delivery.delivery===(host==='web'?'platform':'recognized'));
    await s.clock('+1500');
    const p = await pos();
    check(`run ${number}: exactly 1500 ms → (0, 0.9, -5.733332), tolerance 1 mm`, Math.hypot(p[0],p[1]-0.9,p[2]+5.733332)<0.001, p);
    const forward = await snapshot('W1500');
    check(`run ${number}: 90 fixed ticks`, forward.tick===90, forward.hash);
    check(`run ${number}: native/web simulation hash parity`, forward.hash==='0xf1bdfbe68b382647', forward.hash);
    check(`run ${number}: capsule dimensions`, equal(forward.entities.player.Mesh.Capsule,{radius:0.4,height:1.8}));
    check(`run ${number}: 40 m ground`, equal(forward.entities.ground.Mesh.Plane,{width:40,depth:40}));
    for (const [i,expected] of [[1,[8,1,0]],[2,[-6,1,7]],[3,[3,1,-9]]]) {
      const beacon = forward.entities[`beacon-${i}`];
      check(`run ${number}: beacon ${i} position and default 0.5 m radius`, equal(beacon.Transform.position,expected) && equal(beacon.Transform.scale,[1,1,1])
        && beacon.Mesh.Sphere.radius===0.5);
    }
    await key('KeyW','up'); await s.clock('settle');
    await walk(8,0);
    await key('KeyE'); await s.clock('+100');
    const beacon = (await entity('beacon-1')).Beacon;
    tree = await s.tree();
    check(`run ${number}: glow partway after 100 ms`, beacon.lit && beacon.glow>0 && beacon.glow<1, beacon);
    check(`run ${number}: lit count reaches real HUD`, node(tree,'hud-beacons')?.props.text==='Beacons 1 / 3');
    await s.clock('+900');
    check(`run ${number}: glow fully up at 1 second`, (await entity('beacon-1')).Beacon.glow===1);
    await key('KeyW','down'); await s.clock('+100');
    await s.tap('pause');
    const before = await snapshot('paused');
    await s.clock('+2000');
    const after = await snapshot('paused+2000');
    check(`run ${number}: pause freezes entire world with movement held`, before.hash===after.hash && before.tick===after.tick && equal(before.entities,after.entities));
    tree = await s.tree();
    check(`run ${number}: Resume label`, tree.nodes.some(n=>n.props?.text==='Resume'));
    await key('KeyW','up'); await s.tap('pause'); await s.clock('settle');
    if (number===1) await s.screenshot(resolve(out,'beacons-playing.png'));
    await walk(-6,7); await key('KeyE'); await s.clock('+1000');
    await walk(3,-9); await key('KeyE'); await s.clock('+1000');
    tree = await s.tree();
    check(`run ${number}: completion UI`, node(tree,'hud-beacons')?.props.text==='Beacons 3 / 3' && !!node(tree,'play-again')
      && tree.nodes.some(n=>n.props?.text==='All beacons lit'));
    await snapshot('complete');
    await s.tap('play-again');
    check(`run ${number}: Play again resets position`, equal(await pos(),[0,0.9,0]));
    tree = await s.tree();
    check(`run ${number}: restart resets HUD`, node(tree,'hud-beacons')?.props.text==='Beacons 0 / 3' && !node(tree,'victory'));
    await snapshot('restarted');
    const logs = await s.logs();
    check(`run ${number}: no page/GPU errors (Chrome profile diagnostics recorded)`, !logs.host?.some(l=>/^(exception:|console\.error:|error:)/.test(l)), logs.host);
    const state = await s.state();
    say(`TIMINGS run ${number} ${JSON.stringify(state.world[0].perf)}`);
    await s.close();
    return trace;

}
  const a = await run(1), b = await run(2);
  check('two fresh browser runs have identical full entity states, ticks and hashes', equal(a,b));
  writeFileSync(resolve(out,'runs.json'),JSON.stringify({a,b},null,2)+'\n');

  // D6: hold W, queue a jump without advancing a tick, then capture the whole sim.
  let session;
  const worldFile = resolve(out, 'checkpoint.world');
  const originalFile = resolve(out, 'original.world'), restoredFile = resolve(out, 'restored.world');
  session = await open();
  await session.tap('play'); await session.clock(0);
  await session.type('world', {key:'KeyW',phase:'down'}); await session.clock('+500');
  await session.type('world', {key:'Space',phase:'down'});
  const saved = await session.screenshot(worldFile, 'world', 'save');
  const continueWorld = async () => {
    await session.clock('+500');
    const jumped = await session.state('world:player');
    check('saved queued jump executes after capture', position(jumped)?.[1] > 0.9, position(jumped));
    await session.type('world', {key:'Space',phase:'up'});
    await session.clock('+1000');
    await session.type('world', {key:'KeyW',phase:'up'});
    return {world:(await session.state()).world[0], player:await session.state('world:player')};
  };
  const uninterrupted = await continueWorld();
  await session.screenshot(originalFile, 'world', 'save');
  await session.close(); session = null; say('CLOSED original browser/process before restoring');
  session = await open({world:worldFile});
  check('save waits behind Play', !node(await session.tree(), 'world'));
  await session.tap('play');
  const restored = (await session.state()).world[0];
  check('new session restores before first render, with the same tick and hash', restored.restored === true && restored.tick === saved?.tick && restored.hash === saved?.hash, restored);
  const continued = await continueWorld();
  await session.screenshot(restoredFile, 'world', 'save');
  check('D6 two sessions continue to the same state, position, tick and hash', continued.world.restored === false && continued.world.hash === uninterrupted.world.hash
    && continued.world.tick === uninterrupted.world.tick && equal(position(continued.player), position(uninterrupted.player)), {expected:uninterrupted.world.hash, actual:continued.world.hash});
  check('D6 entire Sim save is byte-identical, including held input, queue, clock, journal and publications', readFileSync(originalFile).equals(readFileSync(restoredFile)));


});
