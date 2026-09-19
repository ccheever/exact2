#!/usr/bin/env bun
import {readFileSync, mkdirSync} from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';
import { installAdapter } from './inject-adapter.mjs';

await proof(import.meta, async ({open, check, equal, out, host, pin, pinSave, say}) => {
  if (host === 'web') installAdapter(resolve(import.meta.dirname, 'dist'));
  const node = (tree, id) => tree.nodes.find(item => item.props?.testId === id);
  const store = resolve(out,'checkpoints'); mkdirSync(store,{recursive:true});
  const launch = options => open({env:{EXACT_SURFACE_STORE:store}, ...options});
  const s = await launch();
  const title = await s.tree();
  check('accessible Play overlays a real title-paused world at tick zero',
    node(title, 'play')?.focused === true && node(title, 'world')?.world?.tick === 0);
  await s.tap('play');
  const playing = await s.tree();
  check('the world and distinct Light and Jump controls are mounted',
    !!node(playing, 'world') && !!node(playing, 'light') && !!node(playing, 'jump'));
  const foxMesh = await s.world('world').get('fox', 'Mesh');
  const foxAnimation = await s.world('world').get('fox', 'Animation');
  check('the consumer uses the real Fox asset', equal(foxMesh, {Asset:['fox.model']}), foxMesh);
  check('the Fox begins in the Survey clip', foxAnimation?.clip === 'Survey', foxAnimation);
  pin(0,(await s.state()).world[0]);
  await s.world('world').hold('KeyW', 1000);
  pin(60,(await s.state()).world[0]);
  const moved = await s.world('world').global_position('player');
  const run = await s.world('world').get('fox', 'Animation');
  check('one second of W moves 4.5 metres through real physics',
    Math.abs(moved[2] - 7.5) < 0.08, moved);
  check('movement selects and advances Run', run?.clip === 'Run' && run.time > 0, run);
  if (host !== 'linux') await s.screenshot(resolve(out, 'lanterns-fox.png'));
  else say('SKIP headless screenshot: declared Fox/Pose and gameplay are checked');
  const logs = await s.logs();
  check('the browser and GPU report no errors',
    !logs.host?.some(line => /^(exception:|console\.error:|error:)/.test(line)), logs.host);
  const checkpoint = resolve(out,'lanterns-60.world');
  await s.world('world').save(checkpoint); pinSave('60',checkpoint);
  const before = (await s.state()).world[0];
  await s.tap('save');
  check('Save confirms durable bytes through the canvas message',node(await s.tree(),'save-status')?.props.text === 'Saved');
  await s.world('world').run(2000);
  pin(180,(await s.state()).world[0]);
  const reference = resolve(out,'lanterns-180.world'); await s.world('world').save(reference); pinSave('180',reference);
  await s.tap('load-save');
  const loaded = (await s.state()).world[0];
  check('Load restores exact tick and world after commit',loaded.tick===before.tick && loaded.hash===before.hash,{before:{tick:before.tick,hash:before.hash},loaded:{tick:loaded.tick,hash:loaded.hash}});
  check('Load confirms through the canvas message',node(await s.tree(),'save-status')?.props.text === 'Loaded');
  await s.world('world').run(2000);
  const afterLoad=resolve(out,'lanterns-loaded-180.world');await s.world('world').save(afterLoad);
  check('HUD checkpoint continuation saves byte-identically',readFileSync(afterLoad).equals(readFileSync(reference)));
  await s.close();
  const titleLoad = await launch();
  await titleLoad.tap('title-load');
  const resumed = (await titleLoad.state()).world[0];
  check('title Load mounts the HUD at the saved tick without Play', !node(await titleLoad.tree(),'title') && resumed.tick===before.tick && resumed.hash===before.hash);
  await titleLoad.world('world').run(2000);
  const afterTitle=resolve(out,'lanterns-title-load-180.world');await titleLoad.world('world').save(afterTitle);
  check('fresh title Load continues byte-identically',readFileSync(afterTitle).equals(readFileSync(reference)));
  await titleLoad.close();
  const restored = await launch({world:checkpoint});
  await restored.tap('play');
  check('fresh process restores tick 60', (await restored.state()).world[0].tick===60);
  await restored.world('world').run(2000);
  const afterFresh=resolve(out,'lanterns-fresh-180.world');await restored.world('world').save(afterFresh);
  check('fresh-process continuation saves byte-identically',readFileSync(afterFresh).equals(readFileSync(reference)));
  await restored.close();
  const blocked = await launch();
  await blocked.tap('play');
  await blocked.world('world').run(0);
  let diagnostic = '';
  try { await blocked.world('world').moveTo('player', [4.8,8]); }
  catch (error) { diagnostic = error.message; }
  check('stalled direct crate route names sign-board, bounds and nearest clear side',
    ['sign-board','bounds','nearestClearSide','minX'].every(part => diagnostic.includes(part)), diagnostic);
  await blocked.close();
});
