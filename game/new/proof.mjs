#!/usr/bin/env bun
// Eight operations, each answers a different question:
// tree: find UI controls and named world entities.
// screenshot: inspect pixels, or capture a complete world save.
// tap: press a visible control through real hit testing.
// type: hold/release physical keys (world().hold is this operation).
// state: inspect components, pending assets, held input and busy reasons.
// layout: see world/placed boxes, camera visibility and line-of-sight picks.
// logs: read gameplay events and refusals, including reload/carry failures.
// clock: advance deterministic ticks; settle reports what keeps moving.
import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import { proof, axNames } from '../../proof.mjs';

// Constant acceleration, semi-implicit integration: v_k=min(k*a/h,v).
// Sum the accelerating ticks, then add the constant-speed tail.
const distance = (n, h, a=12, v=4) => {
  const m=Math.min(n,Math.floor(v*h/a));
  return a*m*(m+1)/(2*h*h)+(n-m)*v/h;
};
if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave}) => {
  const movement = await open();
  if (process.argv.includes('--screenshot-only')) {
    check('screenshot uses web', host === 'web');
    await movement.tap('play');
    await movement.screenshot(resolve(out, 'game.png'));
    await movement.close();
    return;
  }
  await movement.type('play',{key:'Enter'});
  await movement.world('world').hold('KeyW',1500);
  const walked = await movement.world('world').local_position('player');
  check('W for 1.5 s matches the closed-form acceleration series', Math.abs(walked[2]+distance(180,120)) < 0.001, walked);
  await movement.close();
  const s = await open();
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const title = await s.tree();
  const titleAx = await axNames(s);
  check('Play is initially focused and named', node(title, 'play')?.focused === true && (titleAx.unavailable || titleAx.name('play') === 'Play'));
  check('state agrees with initial focus', (await s.state()).focus.logical === node(title, 'play')?.id);
  check('title, controls and Play', !!node(title, 'play') && ['Small game', 'Move with WASD or arrow keys', 'Space jumps · E lights', 'Touch controls appear during play'].every(text => title.nodes.some(n => n.props?.text === text)));
  check('world loads after Play', !node(title, 'world'));
  await s.tap('play');
  check('HUD is a polite live region', node(await s.tree(), 'hud-lit')?.props.accessibilityLive === 'polite');
  check('initial HUD', node(await s.tree(), 'hud-lit')?.props?.text === 'Lit 0');
  const game = s.world('world');
  pin(0, await game.snapshot());
  check('no prompt outside range', !node(await s.tree(), 'near-prompt'));
  await game.tap('KeyE');
  await game.run(100);
  check('outside range lights nothing', !(await game.get('beacon-1', 'Beacon')).lit);
  await game.hold("KeyD", 500.0);
  await game.settle();
  const position = await game.global_position("player");
  check('prompt appears in range', node(await s.tree(), 'near-prompt')?.props.text === 'Press E or tap Light');
  await game.tap("KeyE");
  await game.run(100.0);
  const beacon = await game.get("beacon-1", "Beacon");
  check('movement and light', position[0] > 0.5 && beacon.lit);
  check('publication reaches HUD', node(await s.tree(), 'hud-lit')?.props?.text === 'Lit 1');
  check('lit beacon hides prompt', !node(await s.tree(), 'near-prompt'));
  await game.run(500);
  if (host === 'web') await s.screenshot(resolve(out, 'game.png'));
  await s.tap('pause');
  check('pointer Pause releases focus', node(await s.tree(),'pause')?.focused !== true);
  await s.type('pause',{key:'Space'});
  await game.run(100);
  check('focused Pause takes Space without a world jump', (await game.local_position('player'))[1] === 0.9);
  await s.type('pause',{key:'Enter'});
  await s.type('pause',{key:'Enter'});
  await game.run(100);
  check('focused Pause takes Enter without a world jump', (await game.local_position('player'))[1] === 0.9);
  await game.hold('KeyD', 1000);
  await game.settle();
  check('second beacon prompt appears', !!node(await s.tree(), 'near-prompt'));
  await game.hold('KeyW', 1000);
  await game.settle();
  check('prompt disappears outside range', !node(await s.tree(), 'near-prompt'));
  await game.save(resolve(out, 'final.world'));
  const checkpoint = await game.snapshot();
  await game.hold('KeyA', 250);
  await game.settle();
  const continued = await game.snapshot();
  await game.save(resolve(out, 'continued.world'));
  pinSave('continuation', resolve(out, 'continued.world'));
  await s.close();
  const restored = await open({fresh:true, world:resolve(out, 'final.world')});
  await restored.tap('play');
  const loaded = restored.world('world');
  check('fresh process restores snapshot', JSON.stringify(await loaded.snapshot()) === JSON.stringify(checkpoint));
  await loaded.hold('KeyA', 250);
  await loaded.settle();
  check('fresh process continues identically', JSON.stringify(await loaded.snapshot()) === JSON.stringify(continued));
  await loaded.save(resolve(out, 'restored.world'));
  check('continuation saves are byte-identical', readFileSync(resolve(out, 'continued.world')).equals(readFileSync(resolve(out, 'restored.world'))));
  await restored.close();
  const pointerSession = await open({fresh:true});
  await pointerSession.tap('play');
  await pointerSession.tap('pause'); await pointerSession.tap('pause');
  check('pointer Resume releases focus', node(await pointerSession.tree(),'pause')?.focused !== true);
  await pointerSession.type('world',{key:'Space',phase:'down'});
  await pointerSession.world('world').run(100);
  await pointerSession.type('world',{key:'Space',phase:'up'});
  check('click Pause then Resume leaves Space to jump', (await pointerSession.world('world').local_position('player'))[1] > 0.9);
  const pointerGame = pointerSession.world('world');
  const total = (await pointerGame.snapshot()).entities.filter(e => e.components?.Beacon).length;
  check('the scene supplies a nonempty beacon goal', total > 0);
  // The starter places the first beacon at x=2 and the rest four metres apart.
  for (let i = 0; i < total; i++) {
    await pointerGame.hold('KeyD', i === 0 ? 500 : 1000);
    await pointerGame.settle();
    await pointerGame.tap('KeyE');
    await pointerGame.run(100);
    const tree = await pointerSession.tree();
    check(`beacon ${i + 1} reaches the HUD`, node(tree, 'hud-lit')?.props.text === `Lit ${i + 1}`);
    check(`victory waits for every beacon (${i + 1}/${total})`, !!node(tree, 'again') === (i + 1 === total));
  }
  if (host === 'web') await pointerSession.screenshot(resolve(out, 'victory.png'));
  await pointerSession.tap('again');
  const restarted = await pointerSession.tree();
  check('Play again clears victory and the lit count', !node(restarted, 'again') && node(restarted, 'hud-lit')?.props.text === 'Lit 0');
  check('Play again returns keyboard focus to the game', node(restarted, 'world')?.focused === true);
  await pointerGame.hold('KeyD', 500);
  await pointerGame.settle();
  await pointerGame.tap('KeyE');
  await pointerGame.run(100);
  await pointerSession.tap('pause');
  const paused = await pointerSession.tree();
  const pausedAx = await axNames(pointerSession);
  check('a paused game offers Restart before victory', !!node(paused, 'restart') && (pausedAx.unavailable || pausedAx.name('restart') === 'Restart') && node(paused, 'hud-lit')?.props.text === 'Lit 1');
  if (host === 'web') await pointerSession.screenshot(resolve(out, 'paused.png'));
  await pointerSession.tap('restart');
  const reset = await pointerSession.tree();
  const resetAx = await axNames(pointerSession);
  check('Restart clears progress and resumes play', node(reset, 'hud-lit')?.props.text === 'Lit 0' && !node(reset, 'restart') && !!node(reset, 'pause') && (resetAx.unavailable || resetAx.name('pause') === 'Pause'));
  check('Restart returns keyboard focus to the game', node(reset, 'world')?.focused === true);
  await pointerGame.hold('KeyD', 500);
  check('Restart leaves movement running', (await pointerGame.local_position('player'))[0] > 0.5);
  await pointerSession.close();
});
