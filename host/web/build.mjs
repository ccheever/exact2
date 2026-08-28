#!/usr/bin/env node
// Build the web app: the wasm in release, then `dist/` = index.html + glue.js + app.wasm.
// Usage: node host/web/build.mjs [crate=caltrain-web]
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, statSync } from 'node:fs';
import { resolve } from 'node:path';

const crate = process.argv[2] ?? 'caltrain-web';
const root = resolve(new URL('../..', import.meta.url).pathname);
const r = spawnSync('cargo', ['build', '-p', crate, '--release', '--target', 'wasm32-unknown-unknown'], { cwd: root, stdio: 'inherit' });
if (r.status !== 0) process.exit(r.status ?? 1);
const wasm = resolve(root, 'target/wasm32-unknown-unknown/release', crate.replace(/-/g, '_') + '.wasm');
const dist = resolve(root, 'host/web/dist');
mkdirSync(dist, { recursive: true });
copyFileSync(wasm, resolve(dist, 'app.wasm'));
copyFileSync(resolve(root, 'host/web/index.html'), resolve(dist, 'index.html'));
copyFileSync(resolve(root, 'host/web/glue.js'), resolve(dist, 'glue.js'));
const size = statSync(resolve(dist, 'app.wasm')).size;
console.log(`host/web/dist: app.wasm ${size} bytes (${(size / 1024).toFixed(0)} KiB), index.html, glue.js`);
