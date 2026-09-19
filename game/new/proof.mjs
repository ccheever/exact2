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
// Intended pin change: bun game/prove.mjs small-game --repin
// Save/setup investigation: bun game/games/small-game/proof.mjs linux --paranoid
import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check, out, host, pin, pinSave}) => {
  const s = await open();
  if (process.argv.includes('--screenshot-only')) {
    check('screenshot uses web', host === 'web');
    await s.tap('play');
    await s.screenshot(resolve(out, 'game.png'));
    await s.close();
    return;
  }
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const title = await s.tree();
  check('Play is initially focused and named', node(title, 'play')?.focused === true && node(title, 'play')?.accessibleName === 'Play');
  check('state agrees with initial focus', (await s.state()).focus.logical === node(title, 'play')?.id);
  check('title and Play', !!node(title, 'play') && title.nodes.some(n => n.props?.text === 'Small game'));
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
  check('prompt appears in range', node(await s.tree(), 'near-prompt')?.props.text === 'Tap Light to light this beacon');
  await game.tap("KeyE");
  await game.run(100.0);
  const beacon = await game.get("beacon-1", "Beacon");
  check('movement and light', position[0] > 0.5 && beacon.lit);
  check('publication reaches HUD', node(await s.tree(), 'hud-lit')?.props?.text === 'Lit 1');
  check('lit beacon hides prompt', !node(await s.tree(), 'near-prompt'));
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
  if (host === 'web') await s.screenshot(resolve(out, 'game.png'));
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
});
