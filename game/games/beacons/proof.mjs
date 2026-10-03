#!/usr/bin/env bun
import {readFileSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {proof, captureWorld, diffWorlds, formatWorldDiff, equal, axNames} from '../../proof.mjs';
import {crop, decodePng, diff} from '../../../scripts/png.mjs';

export async function walkTo(world, check, x, z) {
  await world.settle();
  for (const [axis,target,plus,minus] of [[0,x,'KeyD','KeyA'],[2,z,'KeyS','KeyW']]) {
    for (let attempt=0;attempt<4;attempt++) {
      const delta=target-(await world.local_position('player'))[axis];
      if (Math.abs(delta)<0.1) break;
      // Rest-to-rest travel includes acceleration (12), speed (4), and braking
      // (20). Short corrections never reach full speed: d = 9.6 * hold².
      const distance=Math.abs(delta);
      const seconds=distance<16/15 ? Math.sqrt(distance/9.6) : (distance+4/15)/4;
      await world.hold(delta>0?plus:minus,Math.min(4500,Math.ceil(seconds*60)*1000/60));
      check('walk settles after releasing key',await world.settle());
    }
  }
  const p=await world.local_position('player');
  check(`walk reaches (${x}, ${z}) within 0.15 m`,Math.hypot(p[0]-x,p[2]-z)<0.15,p);
}

export function compare(rows, {root}) {
  const captures = rows.map(row => JSON.parse(readFileSync(resolve(root,
    `${row.host}-0-${row.repeat}`, 'tap-timing.json'), 'utf8')));
  if (captures.some(capture => !equal(capture, captures[0])))
    throw new Error('tap timing differs on the first affected tick across hosts');
  console.log('TAPS first affected tick and world hash agree across hosts');
}

// Build products stay beside this game, including in callers with a shared target.
if (import.meta.main) process.env.CARGO_TARGET_DIR = resolve(import.meta.dir, 'target');
if (import.meta.main) await proof(import.meta, async ({open, check, equal, out, host, pin, pinSave, say}) => {
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const world = s => s.world('world');
  const snapshot = s => world(s).snapshot();
  const position = s => world(s).local_position('player');
  const captureGlow = async (s, label) => {
    const screen = (await s.layout('world:beacon-1')).entity.screen;
    const path = resolve(out,`beacon-1-glow-${label}.png`);
    const shot = await s.screenshot(path);
    const image = decodePng(readFileSync(path));
    const scaleX = image.width / shot.w, scaleY = image.height / shot.h;
    const x = Math.max(0,Math.floor(screen.x*scaleX));
    const y = Math.max(0,Math.floor(screen.y*scaleY));
    const right = Math.min(image.width,Math.ceil((screen.x+screen.w)*scaleX));
    const bottom = Math.min(image.height,Math.ceil((screen.y+screen.h)*scaleY));
    const pixels = crop(image,x,y,right-x,bottom-y);
    let rgb = 0, min = 765, max = 0;
    for (let i=0;i<pixels.data.length;i+=4) {
      const value=pixels.data[i]+pixels.data[i+1]+pixels.data[i+2];
      rgb+=value;min=Math.min(min,value);max=Math.max(max,value);
    }
    return {pixels,screen,crop:{x,y,w:right-x,h:bottom-y},mean:rgb/(pixels.width*pixels.height*3),range:(max-min)/3};
  };
  let checkpointState, uninterrupted;
  const checkpoint = resolve(out,'checkpoint.world');
  const original = resolve(out,'original.world');
  {
    const s = await open();
    const title = await s.tree();
    const titleAx = await axNames(s);
    check('title has focused, accessible Play', node(title,'play')?.focused === true && (titleAx.unavailable || titleAx.name('play') === 'Play'));
    check('title has no world', !node(title,'world'));
    await s.tap('play');
    const initial = await snapshot(s);
    pin(0, initial);
    check('six seeded crates', initial.entities.filter(e => /^crate-/.test(e.name)).length === 6);
    check('capsule radius 0.4 height 1.8', equal(await world(s).get('player','Mesh'), {Capsule:{height:1.8,radius:0.4}}));
    const beforeMove = await captureWorld(s);
    await world(s).hold('KeyW',1500);
    const afterMove = await captureWorld(s);
    const movement = diffWorlds(beforeMove, afterMove);
    check('world diff identifies the player movement field', movement.changes.some(c =>
      c.path === 'entities["player"].Transform.position[2]' && c.before === 0 && c.after < -5));
    check('unchanged world capture compares equal', diffWorlds(afterMove, await captureWorld(s)).total === 0);
    writeFileSync(resolve(out, 'before-move.json'), JSON.stringify(beforeMove));
    writeFileSync(resolve(out, 'after-move.json'), JSON.stringify(afterMove));
    say(formatWorldDiff(movement));
    const p = await position(s);
    check('W exactly 1.5 s: expected (0, 0.9, -5.3666644), tolerance 1 mm', Math.hypot(p[0],p[1]-0.9,p[2]+5.3666644) < 0.001,p);
    pin(90,await snapshot(s));
    await world(s).settle();
    await world(s).tap('KeyE'); await world(s).run(100);
    check('E outside range lights nothing', !(await world(s).get('beacon-1','Beacon')).lit);
    await walkTo(world(s),check,8,0);
    const glowFrames = host === 'web' ? {unlit:await captureGlow(s,'unlit')} : null;
    await world(s).tap('KeyE');
    await world(s).run(100);
    const [glow] = await world(s).get('beacon-1','Glow');
    check('saved Glow declares a half-second transition from 0 to 1', glow.start_value === 0 && glow.target === 1 && glow.duration === 0.5, glow);
    const observed = await snapshot(s);
    check('0.1 s observation lies inside the saved Glow transition', observed.tick > glow.start_tick && observed.tick < glow.start_tick + 60*glow.duration,{tick:observed.tick,start_tick:glow.start_tick,duration:glow.duration});
    if (glowFrames) glowFrames.partial=await captureGlow(s,'100ms');
    check('beacon-1 lit', (await world(s).get('beacon-1','Beacon')).lit);
    const hud = node(await s.tree(),'hud-lit');
    check('HUD is Beacons 1 / 3 and a polite live region', hud?.props.text === 'Beacons 1 / 3' && hud?.props.accessibilityLive === 'polite');
    await world(s).run(500);
    if (glowFrames) glowFrames.full=await captureGlow(s,'600ms');
    await world(s).run(400);
    if (glowFrames) {
      glowFrames.later=await captureGlow(s,'1000ms');
      const frames=Object.values(glowFrames), base=glowFrames.unlit;
      const sameGeometry=frames.every(frame => ['x','y','w','h'].every(key => Math.abs(frame.screen[key]-base.screen[key])<0.01)
        && ['x','y','w','h'].every(key => frame.crop[key]===base.crop[key]));
      check('glow pixel crops keep the same geometry and non-uniform samples',sameGeometry && frames.every(frame=>frame.range>8),frames.map(({screen,crop,range})=>({screen,crop,range})));
      const partial=diff(base.pixels,glowFrames.partial.pixels);
      const full=diff(glowFrames.partial.pixels,glowFrames.full.pixels);
      const stable=diff(glowFrames.full.pixels,glowFrames.later.pixels);
      say(`glow pixel observations ${JSON.stringify({means:Object.fromEntries(Object.entries(glowFrames).map(([name,frame])=>[name,frame.mean])),diffs:{partial,full,stable}})}`);
      check('web beacon pixels visibly brighten at 0.1 s',partial.differing>0.05 && partial.mean>2 && glowFrames.partial.mean>base.mean+2,{...partial,from:base.mean,to:glowFrames.partial.mean});
      check('web beacon pixels brighten further by 0.6 s',full.differing>0.05 && full.mean>2 && glowFrames.full.mean>glowFrames.partial.mean+2,{...full,from:glowFrames.partial.mean,to:glowFrames.full.mean});
      check('web beacon pixels stay stable from 0.6 to 1 s',stable.differing<0.01 && stable.mean<0.5 && Math.abs(glowFrames.later.mean-glowFrames.full.mean)<0.5,{...stable,from:glowFrames.full.mean,to:glowFrames.later.mean});
    }
    check('clock passes the saved Glow half-second deadline', (await snapshot(s)).tick >= glow.start_tick + 60*glow.duration && glow.target === 1);
    check('saved material stays constant while renderer samples glow', equal((await world(s).get('beacon-1','Material')).emissive,[3,3,3]));
    // Pause while moving and mid-jump, so freezing is not a stationary-world tautology.
    await world(s).key_down('KeyW'); await world(s).tap('Space'); await world(s).run(100);
    await s.tap('pause');
    const paused = await snapshot(s);
    await world(s).run(2000);
    check('Pause freezes every entity and the tick for 2 seconds', equal(paused,await snapshot(s)));
    const pausedAx = await axNames(s);
    check('Pause becomes accessible Resume', node(await s.tree(),'pause')?.props && (pausedAx.unavailable || pausedAx.name('pause') === 'Resume'));
    await s.tap('pause');
    await world(s).key_up('KeyW'); await world(s).run(200);
    if (host !== 'linux') await s.screenshot(resolve(out,`beacons-${host}.png`));
    else say('SKIP 3D pixels on GPU-less Linux; run this proof on web for the screenshot.');
      checkpointState = await snapshot(s);
      await world(s).save(checkpoint);
      await world(s).hold('ArrowLeft',750);
      await world(s).run(1100);
      uninterrupted = await snapshot(s);
      await world(s).save(original);
      pinSave('continuation',original);
    await s.close();
  }
  const restored = await open({fresh:true, world:checkpoint});
  say('Original carrier closed before fresh restore.');
  await restored.tap('play');
  check('fresh process restores full mid-jump world', equal(checkpointState,await snapshot(restored)));
  check('restore explicitly acknowledged', (await restored.state()).world[0].restored === true);
  await world(restored).hold('ArrowLeft',750);
  await world(restored).run(1100);
  check('fresh process continues exactly',equal(uninterrupted,await snapshot(restored)));
  const restoredFile = resolve(out,'restored.world');
  await world(restored).save(restoredFile);
  check('continuation saves byte-identical, including input/time',readFileSync(original).equals(readFileSync(restoredFile)));
  if (host === 'web' || host === 'ios') {
    await world(restored).settle();
    const floor = (await position(restored))[1];
    const jump = await restored.tap('jump', {down:true});
    await world(restored).run(100);
    check(`${host} pointer Jump raises the player`, (await position(restored))[1] > floor && jump.delivery === (host === 'web' ? 'platform' : 'recognized'), jump);
    await restored.pointer('up');
    await world(restored).settle();
  }
  await walkTo(world(restored),check,-6,7);
  if (host === 'web' || host === 'ios') {
    const light = await restored.tap('light', {down:true});
    await world(restored).run(1000 / 60);
    await restored.pointer('up');
    check(`${host} pointer Light lights beacon-2`, (await world(restored).get('beacon-2','Beacon')).lit, light);
  } else await world(restored).tap('KeyE');
  await world(restored).run(600);
  await walkTo(world(restored),check,3,-9);
  await world(restored).tap('KeyE'); await world(restored).run(600);
  const won = await restored.tree();
  check('all three light and win UI appears',node(won,'hud-lit')?.props.text === 'Beacons 3 / 3' && !!node(won,'victory'));
  const wonAx = await axNames(restored);
  check('Play again has focus and accessible name',node(won,'again')?.focused === true && (wonAx.unavailable || wonAx.name('again') === 'Play again'));
  await restored.tap('again');
  check('restart clears count and resets player',node(await restored.tree(),'hud-lit')?.props.text === 'Beacons 0 / 3' && equal(await position(restored),[0,0.9,0]));
  const logs = await restored.logs();
  check('no host exceptions', !(logs.host ?? []).some(line => /^(exception:|console\.error:|error:)/.test(line)));
  // The reset world is already at the title probe's starting state. Reuse its
  // carrier for the focus check instead of launching a third native process.
  await restored.tap('pause'); await restored.tap('pause');
  check('pointer Resume releases focus', node(await restored.tree(),'pause')?.focused !== true);
  await restored.type('world',{key:'Space',phase:'down'});
  await world(restored).run(100);
  await restored.type('world',{key:'Space',phase:'up'});
  check('click Pause then Resume leaves Space to jump', (await position(restored))[1] > 0.9);
  // Keep world-changing taps in parity coverage, including fractional boundaries.
  await world(restored).settle();
  const taps = [];
  for (const offset of [0, 0.001, 16.665, 16.667, 0.333, 1500]) {
    await world(restored).run(offset);
    const before = await snapshot(restored);
    await world(restored).tap('Space');
    await world(restored).run(16.667);
    const after = await snapshot(restored);
    check(`tap at offset ${offset} changes the next tick`, after.tick === before.tick + 1
      && (await world(restored).get('player', 'Character')).airborne);
    taps.push({offset, before:{tick:before.tick,hash:before.hash}, after:{tick:after.tick,hash:after.hash}});
    await world(restored).run(1000);
  }
  writeFileSync(resolve(out, 'tap-timing.json'), JSON.stringify(taps));
  await restored.close();
});
