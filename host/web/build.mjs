#!/usr/bin/env node
// Build the web app: the wasm under the `web` profile (size-tuned), `wasm-opt -Oz`
// when binaryen is on PATH, then `dist/` = index.html + glue.js + app.wasm.
// Usage: node host/web/build.mjs [crate=caltrain-web]
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { gzipSync } from 'node:zlib';

const crate = process.argv[2] ?? 'caltrain-web';
const root = resolve(new URL('../..', import.meta.url).pathname);
const r = spawnSync('cargo', ['build', '-p', crate, '--lib', '--profile', 'web', '--target', 'wasm32-unknown-unknown'], { cwd: root, stdio: 'inherit' });
if (r.status !== 0) process.exit(r.status ?? 1);
const built = resolve(root, 'target/wasm32-unknown-unknown/web', crate.replace(/-/g, '_') + '.wasm');
const dist = resolve(root, 'host/web/dist');
mkdirSync(dist, { recursive: true });
const out = resolve(dist, 'app.wasm');

// Post-link optimization. The feature flags match what rustc's wasm32 target emits.
const opt = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', out, built], { stdio: 'inherit' });
let optNote;
if (opt.error?.code === 'ENOENT') { copyFileSync(built, out); optNote = 'wasm-opt not on PATH (brew install binaryen): shipped unoptimized'; }
else if (opt.status !== 0) process.exit(opt.status ?? 1);
else optNote = 'wasm-opt -Oz';

copyFileSync(resolve(root, 'host/web/index.html'), resolve(dist, 'index.html'));
copyFileSync(resolve(root, 'host/web/glue.js'), resolve(dist, 'glue.js'));
const wasm = readFileSync(out);
const kib = (n) => `${(n / 1024).toFixed(0)} KiB`;
console.log(`host/web/dist: app.wasm ${kib(wasm.length)} (${kib(gzipSync(wasm, { level: 9 }).length)} gzip; ${optNote}), index.html, glue.js`);
