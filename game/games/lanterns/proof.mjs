#!/usr/bin/env bun
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';
import { installAdapter } from './inject-adapter.mjs';

await proof(import.meta, async ({open, check, equal, out}) => {
  installAdapter(resolve(import.meta.dirname, 'dist'));
  const node = (tree, id) => tree.nodes.find(item => item.props?.testId === id);
  const s = await open();
  const title = await s.tree();
  check('accessible Play overlays a real title-paused world at tick zero',
    node(title, 'play')?.focused === true && node(title, 'world')?.world?.tick === 0);
  await s.tap('play');
  const playing = await s.tree();
  check('the world and distinct Light and Jump controls are mounted',
    !!node(playing, 'world') && !!node(playing, 'light') && !!node(playing, 'jump'));
  const foxMesh = await s.world('world').get('fox', 'Mesh');
  const foxAnimation = await s.world('world').get('fox', 'Animation');
  check('the consumer uses the real Fox asset', equal(foxMesh, {Asset:['Fox.glb']}), foxMesh);
  check('the Fox begins in the Survey clip', foxAnimation?.clip === 'Survey', foxAnimation);
  await s.world('world').hold('KeyW', 1000);
  const moved = await s.world('world').position('player');
  const run = await s.world('world').get('fox', 'Animation');
  check('one second of W moves 4.5 metres through real physics',
    Math.abs(moved[2] - 7.5) < 0.08, moved);
  check('movement selects and advances Run', run?.clip === 'Run' && run.seconds > 0, run);
  await s.screenshot(resolve(out, 'lanterns-fox.png'));
  const logs = await s.logs();
  check('the browser and GPU report no errors',
    !logs.host?.some(line => /^(exception:|console\.error:|error:)/.test(line)), logs.host);
  await s.close();
});
