#!/usr/bin/env bun
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';
import { installAdapter } from './inject-adapter.mjs';

await proof(import.meta, async ({open, check, equal, out, host}) => {
  if (host === 'web') installAdapter(resolve(import.meta.dirname, 'dist'));
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
  const blocked = await open();
  await blocked.tap('play');
  await blocked.world('world').run(0);
  let diagnostic = '';
  try { await blocked.world('world').moveTo('player', [4.8,8]); }
  catch (error) { diagnostic = error.message; }
  check('stalled direct crate route names sign-board, bounds and nearest clear side',
    ['sign-board','bounds','nearestClearSide','minX'].every(part => diagnostic.includes(part)), diagnostic);
  await blocked.close();
});
