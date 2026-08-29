#!/usr/bin/env node
// Build the web app: the wasm under the `web` profile (size-tuned), `wasm-opt -Oz`
// when binaryen is on PATH, then `dist/` = index.html + glue.js + app.wasm.
// Usage: node host/web/build.mjs [crate=caltrain-web]
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { gzipSync } from 'node:zlib';

const crate = process.argv[2] ?? 'caltrain-web';
const kib = (n) => `${(n / 1024).toFixed(0)} KiB`;
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

// The app's GPU module (LLP 1009 D2): a second wasm the page fetches on
// demand, built with wasm-bindgen's glue (its exports are the module's ABI on
// the web) and wasm-opt. Only when the app has a GPU crate.
const gpuCrate = crate.replace(/-web$/, '-gpu');
let gpuNote = 'no GPU crate';
if (existsSync(resolve(root, 'apps', crate.replace(/-web$/, ''), 'gpu', 'Cargo.toml'))) {
  const g = spawnSync('cargo', ['build', '-p', gpuCrate, '--lib', '--profile', 'web', '--target', 'wasm32-unknown-unknown', '--config', 'profile.web.strip=false'], { cwd: root, stdio: 'inherit' });
  if (g.status !== 0) process.exit(g.status ?? 1);
  const gpuWasm = resolve(root, 'target/wasm32-unknown-unknown/web', gpuCrate.replace(/-/g, '_') + '.wasm');
  const wb = spawnSync('wasm-bindgen', ['--target', 'web', '--no-typescript', '--out-dir', dist, '--out-name', 'gpu', gpuWasm], { stdio: 'inherit' });
  if (wb.error?.code === 'ENOENT') { gpuNote = 'wasm-bindgen not on PATH (cargo install wasm-bindgen-cli): GPU module not built'; }
  else if (wb.status !== 0) process.exit(wb.status ?? 1);
  else {
    const bg = resolve(dist, 'gpu_bg.wasm');
    const o = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', bg, bg], { stdio: 'inherit' });
    copyFileSync(resolve(root, 'host/web/gpu-glue.js'), resolve(dist, 'gpu-glue.js'));
    const gw = readFileSync(bg);
    gpuNote = `gpu_bg.wasm ${kib(gw.length)} (${kib(gzipSync(gw, { level: 9 }).length)} gzip${o.status === 0 ? ', wasm-opt' : ''}), gpu.js ${kib(readFileSync(resolve(dist, 'gpu.js')).length)}, on demand`;
  }
}
const wasm = readFileSync(out);
console.log(`host/web/dist: app.wasm ${kib(wasm.length)} (${kib(gzipSync(wasm, { level: 9 }).length)} gzip; ${optNote}), index.html, glue.js; GPU: ${gpuNote}`);
