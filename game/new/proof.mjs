#!/usr/bin/env bun
import {resolve} from 'node:path';
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check, out, host}) => {
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
  check('no prompt outside range', !node(await s.tree(), 'near-prompt'));
  await game.tap('KeyE');
  await game.run(100);
  check('outside range lights nothing', !(await game.get('beacon-1', 'Beacon')).lit);
  await game.hold("KeyD", 500.0);
  await game.settle();
  const position = await game.position("player");
  check('prompt appears in range', node(await s.tree(), 'near-prompt')?.props.text === 'Press E to light this beacon');
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
  await s.close();
});
