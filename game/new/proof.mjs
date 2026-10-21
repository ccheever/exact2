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
// The proof ends with the look: the world drawn offscreen (look(), and at each
// session's close), its tells (a near-black or clipped frame, particles that
// cover nothing, a shadowed sun with no caster) failing the look result, and a
// picture to open once before reporting, which the tells cannot replace.
import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import { proof, axNames, completedWorldFrame, worldPixels } from '../../proof.mjs';

// How a fresh session reaches the running world: change it with the title screen.
const play = async session => {
  await session.tap('play');
  return session.world('world');
};

// Constant acceleration, semi-implicit integration: v_k=min(k*a/h,v).
// Sum the accelerating ticks, then add the constant-speed tail.
const distance = (n, h, a=12, v=4) => {
  const m=Math.min(n,Math.floor(v*h/a));
  return a*m*(m+1)/(2*h*h)+(n-m)*v/h;
};
if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave, look, say}) => {
  if (process.argv.includes('--screenshot-only')) {
    check('screenshot uses web', host === 'web');
    const shot = await open();
    await play(shot);
    await shot.screenshot(resolve(out, 'game.png'));
    await shot.close();
    return;
  }

  // Engine facts: they hold for any game, so keep them as they are.
  const first = await open();
  const world = await play(first);
  pin(0, await world.snapshot());
  await world.run(500);
  // The GPU-less Linux host draws no world; every other host's renderer must report
  // itself ready and its canvas, outside the HUD, must not be one cleared colour.
  let shown;
  if (host !== 'linux') {
    const shot = await first.screenshot(resolve(out, 'game.png'));
    shown = worldPixels(readFileSync(resolve(out, 'game.png')), shot, await first.layout(), await first.tree());
    const frame = completedWorldFrame(shown, (await first.state()).world?.[0]);
    if (frame.fallback) say('NOTE the HUD covers the whole canvas: the frame fact rests on ready and the first-draw report');
    check('the GPU host presents a completed frame of the world', frame.ok, frame);
  }
  await world.save(resolve(out, 'checkpoint.world'));
  const checkpoint = await world.snapshot();
  await world.run(500);
  const continued = await world.snapshot();
  await world.save(resolve(out, 'continued.world'));
  pinSave('continuation', resolve(out, 'continued.world'));
  await first.close();
  const restored = await open({fresh:true, world:resolve(out, 'checkpoint.world')});
  const loaded = await play(restored);
  check('a fresh process restores the save', JSON.stringify(await loaded.snapshot()) === JSON.stringify(checkpoint));
  await loaded.run(500);
  check('a fresh process continues identically', JSON.stringify(await loaded.snapshot()) === JSON.stringify(continued));
  await loaded.save(resolve(out, 'restored.world'));
  check('continuation saves are byte-identical', readFileSync(resolve(out, 'continued.world')).equals(readFileSync(resolve(out, 'restored.world'))));
  await restored.close();

  // starter: replace these with your game's checks
  // The starter's scene fills most of its canvas; retune the band to your own scene.
  if (shown) check('the starter scene covers most of the canvas', shown.share > 0.1, shown);
  const movement = await open();
  const walking = await play(movement);
  await walking.hold('KeyW',1500);
  const walked = await walking.local_position('player');
  check('W for 1.5 s matches the closed-form acceleration series', Math.abs(walked[2]+distance(180,120)) < 0.001, walked);
  await movement.close();
  const s = await open();
  // Launch work lands first: an Apple host autofocuses the turn after activation.
  await s.clock('settle');
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const title = await s.tree();
  const titleAx = await axNames(s);
  check('Play is initially focused and named', node(title, 'play')?.focused === true && (titleAx.unavailable || titleAx.name('play') === 'Play'));
  check('state agrees with initial focus', (await s.state()).focus.logical === node(title, 'play')?.id);
  check('title, controls and Play', !!node(title, 'play') && ['Small game', 'Move with WASD or arrow keys', 'Space jumps · E lights', 'Walk to the critter: it drops coins · 1 feeds it · 2 buys shoes', 'On a touch screen, touch controls appear during play'].every(text => title.nodes.some(n => n.props?.text === text)));
  check('world loads after Play', !node(title, 'world'));
  await s.tap('play');
  check('HUD is a polite live region', node(await s.tree(), 'hud-lit')?.props.accessibilityLive === 'polite');
  check('initial HUD', node(await s.tree(), 'hud-lit')?.props?.text === 'Lit 0');
  // On-screen controls follow the primary input: a mouse shows none, a touch screen all three.
  // At launch it is the device class's: a phone's touch screen, a desktop's mouse.
  const touchScreen = ['ios', 'android'].includes(host), launched = await s.tree();
  check(`the ${touchScreen ? 'touch screen shows' : 'mouse shows no'} on-screen controls at launch`, ['move', 'jump', 'light'].every(id => !!node(launched, id) === touchScreen));
  await s.prefer({pointer:'coarse'});
  const touch = await s.tree();
  check('a touch screen shows the stick and buttons', ['move', 'jump', 'light'].every(id => !!node(touch, id)));
  await s.prefer({pointer:'fine'});
  const mouse = await s.tree();
  check('a mouse again hides them', ['move', 'jump', 'light'].every(id => !node(mouse, id)));
  const game = s.world('world');
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
  await game.save(resolve(out, 'lit.world'));
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
  await s.close();
  const relit = await open({fresh:true, world:resolve(out, 'lit.world')});
  const relitGame = await play(relit);
  check('a fresh process restores the lit beacon and its HUD', (await relitGame.get('beacon-1', 'Beacon')).lit && node(await relit.tree(), 'hud-lit')?.props.text === 'Lit 1');
  await relit.close();
  // The critter: asleep until the player comes, then it follows, drops coins
  // that drift to the player, eats from the shop and keeps away from the light.
  const pet = await open({fresh:true});
  await pet.tap('play');
  const petWorld = pet.world('world');
  check('the critter sleeps and its shop waits for it', !(await petWorld.get('critter', 'Critter')).awake && !node(await pet.tree(), 'shop'));
  await petWorld.hold('KeyA', 600);
  await petWorld.run(100);
  check('the critter wakes when the player comes near', (await petWorld.get('critter', 'Critter')).awake);
  await petWorld.run(2500);
  let shop = await pet.tree();
  check('its first coin drifts to the player and is counted', node(shop, 'hud-coins')?.props.text === 'Coins 1 · critter fed', node(shop, 'hud-coins')?.props);
  check('feeding is locked until it is half hungry', node(shop, 'buy-1')?.props.accessibilityLabel === 'Feed, Not hungry' && node(shop, 'buy-1')?.props.disabled === true);
  const [cx, , cz] = await petWorld.global_position('critter'), [px, , pz] = await petWorld.global_position('player');
  check('it follows the player and stops short of it', Math.abs(Math.hypot(cx - px, cz - pz) - 1.2) < 0.2, {critter: [cx, cz], player: [px, pz]});
  await petWorld.run(18500);
  shop = await pet.tree();
  check('half hungry after 20 s, with a coin every 2 s', node(shop, 'hud-coins')?.props.text === 'Coins 10 · critter hungry' && node(shop, 'buy-1')?.props.accessibilityLabel === 'Feed, 6 coins, key 1', node(shop, 'hud-coins')?.props);
  await petWorld.tap('Digit1');
  await petWorld.run(100);
  shop = await pet.tree();
  check('Digit1 feeds it once, for six coins', node(shop, 'hud-coins')?.props.text === 'Coins 4 · critter fed', node(shop, 'hud-coins')?.props);
  // Pins after the kit has run, so the first baseline compares its state across
  // hosts: the critter steered, a coin drifted, the shop sold. Ticks: 72 walking,
  // 12 + 300 + 2220 waiting, 12 after the purchase.
  await petWorld.save(resolve(out, 'critter.world'));
  pinSave('critter-fed', resolve(out, 'critter.world'));
  await petWorld.run(4000);
  const fedOnState = await petWorld.snapshot();
  pin(3096, fedOnState);
  const fedOn = JSON.stringify(fedOnState);
  await petWorld.hold('KeyD', 1100);
  await petWorld.tap('KeyE');
  await petWorld.run(3000);
  const [bx, , bz] = await petWorld.global_position('beacon-1'), [kx, , kz] = await petWorld.global_position('critter');
  const fromLight = Math.hypot(kx - bx, kz - bz);
  check('it keeps 3 m from a lit beacon, beside the player', (await petWorld.get('beacon-1', 'Beacon')).lit && fromLight > 2.9 && fromLight < 3.5, {critter: [kx, kz], fromLight});
  pin(3588, await petWorld.snapshot());
  await pet.close();
  // Its hunger, its next coin and the coins' serials travel in a save.
  const petAgain = await open({fresh:true, world:resolve(out, 'critter.world')});
  const petRestored = await play(petAgain);
  await petRestored.run(4000);
  check('a fresh process restores the awake critter and its coin schedule', JSON.stringify(await petRestored.snapshot()) === fedOn);
  await petAgain.close();
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
  // One HUD, laid out by the screen's shape: in portrait a top bar and a bottom strip,
  // the panel a sheet behind Menu, and nothing over the middle third of the world.
  const tall = await open({fresh:true});
  await tall.resize(420, 900);
  await play(tall);
  // The canvas's box, the HUD nodes crossing its middle third, and the named nodes not wholly inside it.
  const measure = async (ids = []) => {
    const tree = await tall.tree(), boxes = await tall.layout(), owner = node(tree, 'world');
    const box = boxes.nodes.find(n => n.id === owner.id), parents = new Map(tree.nodes.map(n => [n.id, n.parent]));
    const inside = id => { for (let p = parents.get(id); p != null; p = parents.get(p)) if (p === owner.id) return true; return false; };
    const third = {x: box.x + box.w / 3, y: box.y + box.h / 3, w: box.w / 3, h: box.h / 3};
    const crossing = boxes.nodes.filter(n => inside(n.id) && n.w > 0 && n.h > 0 && n.x < third.x + third.w && third.x < n.x + n.w && n.y < third.y + third.h && third.y < n.y + n.h)
      .map(n => tree.nodes.find(t => t.id === n.id)?.props?.testId ?? n.id);
    const off = ids.filter(id => { const b = boxes.nodes.find(n => n.id === node(tree, id)?.id);
      return !b || b.w <= 0 || b.h <= 0 || b.x < box.x || b.y < box.y || b.x + b.w > box.x + box.w || b.y + b.h > box.y + box.h; });
    return {crossing, off};
  };
  const upright = await tall.tree();
  check('portrait: the panel waits behind Menu', !node(upright, 'panel') && !!node(upright, 'panel-open'));
  check('portrait: no HUD node crosses the middle third of the world', (await measure()).crossing.length === 0, await measure());
  // The critter wakes and, 20 s on, is hungry with ten coins: its shop is in the sheet.
  const tallWorld = tall.world('world');
  await tallWorld.hold('KeyA', 600);
  await tallWorld.run(21100);
  await tall.tap('panel-open');
  const sheet = await measure(['panel', 'buy-1', 'buy-2', 'touch-controls', 'panel-close']);
  check('portrait: the open sheet, with the shop, is on screen and clear of the middle third', sheet.crossing.length === 0 && sheet.off.length === 0, sheet);
  await tall.tap('buy-1');
  await tallWorld.run(100);
  check('portrait: a tap on the sheet\'s shop feeds the critter for six coins', node(await tall.tree(), 'hud-coins')?.props.text === 'Coins 4 · critter fed', node(await tall.tree(), 'hud-coins')?.props);
  await tall.tap('touch-controls');
  await tall.tap('panel-close');
  const strip = await tall.tree(), controls = await measure(['move', 'jump', 'light']);
  check('portrait: the sheet\'s Touch controls show the bottom strip on screen', controls.off.length === 0 && !node(strip, 'panel'), controls);
  check('portrait: the strip leaves the middle third clear too', controls.crossing.length === 0, controls);
  await tall.close();
  // The scene from above as well as from the game's camera: what the camera never shows.
  const plan = await look(pointerSession, {top: 30}, 'plan').catch(error => ({unavailable: error.unavailable ?? error.message}));
  say(plan.picture ? `LOOK plan ${plan.picture}: open it once, with the frames below, before you report` : `look: ${plan.unavailable}`);
  await pointerSession.close();
});
