#!/usr/bin/env bun
import { resolve } from 'node:path';
import { existsSync } from 'node:fs';

const name = process.argv[2];
if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name ?? '')) throw new Error('Usage: bun game/dev.mjs <name> [dev options]');
const app = resolve(import.meta.dir, 'games', name);
if (!existsSync(resolve(app, 'app.contract'))) throw new Error(`No game: ${name}`);
Object.assign(process.env, {EXACT_APP_DIR:app, EXACT_UPDATE_TRUST:'development', EXACT_LOOPBACK:'1', EXACT_WEB_DIST:resolve(app, 'dist')});
if (process.argv.includes('--scene-only')) {
  const {bakeGameScene} = await import('./app/scenes.mjs');
  const result = bakeGameScene(app, {development:true, build:!process.argv.includes('--reuse-baker')});
  if (!result) throw new Error(`No scene.json at ${app}`);
  console.log(result.output);
  console.log(`Rust targets compiled: ${result.compiled.length} (${result.compiled.join(', ') || 'all fresh / reused baker'})`);
  process.exit(0);
}
process.argv = [process.argv[0], resolve(import.meta.dir, '../host/web/dev.mjs'), ...process.argv.slice(3)];
await import('../host/web/dev.mjs');
