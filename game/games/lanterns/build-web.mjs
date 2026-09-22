#!/usr/bin/env bun
import {spawnSync} from 'node:child_process';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {installAdapter} from './inject-adapter.mjs';

const app = fileURLToPath(new URL('.', import.meta.url));
const root = resolve(app, '../../..');
const dist = resolve(app, 'dist');
const env = {...process.env, EXACT_APP_DIR: app, EXACT_WEB_DIST: dist,
  EXACT_UPDATE_TRUST: 'development', CARGO_TARGET_DIR: resolve(app, 'target')};
const result = spawnSync('bun', [resolve(root, 'host/web/build.mjs')], {cwd: root, env, stdio: 'inherit'});
if (result.status !== 0) process.exit(result.status ?? 1);
installAdapter(dist);
console.log(`Lanterns Exact web artifact with ?agent=1 adapter: ${dist}`);
