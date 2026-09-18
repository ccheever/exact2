#!/usr/bin/env bun
import { proof } from '../../proof.mjs';

await proof(import.meta, async ({open, check}) => {
  const s = await open();
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const title = await s.tree();
  check('title and Play', !!node(title, 'play') && title.nodes.some(n => n.props?.text === 'Small game'));
  check('world loads after Play', !node(title, 'world'));
  await s.tap('play');
  check('initial HUD', node(await s.tree(), 'hud-lit')?.props?.text === 'Lit 0');
  await s.type('world', {key:'KeyW', for:250});
  check('movement', (await s.state('world:player')).entity.components.Transform.position[2] < 0);
  await s.type('world', {key:'KeyE'});
  check('settles', (await s.clock('settle')).settled === true);
  check('publication reaches HUD', node(await s.tree(), 'hud-lit')?.props?.text === 'Lit 1');
});
