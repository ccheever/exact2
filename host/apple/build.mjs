#!/usr/bin/env node
// Build the macOS app: the app's static library (cargo, release), then the
// presenter (swift build) linked against it. Usage:
//   node host/apple/build.mjs [crate=caltrain-apple] [--run]
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { copyFileSync, existsSync, rmSync } from 'node:fs';

const args = process.argv.slice(2);
const crate = args.find((a) => !a.startsWith('--')) ?? 'caltrain-apple';
const root = resolve(new URL('../..', import.meta.url).pathname);
const t0 = Date.now();
const cargo = spawnSync('cargo', ['build', '--release', '-p', crate], { cwd: root, stdio: 'inherit' });
if (cargo.status !== 0) process.exit(cargo.status ?? 1);
// The app's GPU module (LLP 1009 D2): a dylib beside the executable, loaded
// on demand by the presenter. Only when the app has a GPU crate.
const gpuCrate = crate.replace(/-apple$/, '-gpu');
let gpuNote = 'no GPU crate';
if (existsSync(resolve(root, 'apps', crate.replace(/-apple$/, ''), 'gpu', 'Cargo.toml'))) {
  const g = spawnSync('cargo', ['build', '--release', '-p', gpuCrate], { cwd: root, stdio: 'inherit' });
  if (g.status !== 0) process.exit(g.status ?? 1);
  gpuNote = `lib${gpuCrate.replace(/-/g, '_')}.dylib`;
}
const t1 = Date.now();
const env = { ...process.env, EXACT_LIB_DIR: resolve(root, 'target/release'), EXACT_LIB: crate.replace(/-/g, '_') };
// swift build does not see the Rust archive change; drop the executable so
// it relinks against the archive cargo just built (a relink is ~0.4 s).
rmSync(resolve(root, 'host/apple/macos/.build/release/ExactMac'), { force: true });
const swift = spawnSync('swift', ['build', '-c', 'release'], { cwd: resolve(root, 'host/apple/macos'), stdio: 'inherit', env });
if (swift.status !== 0) process.exit(swift.status ?? 1);
const t2 = Date.now();
const bin = resolve(root, 'host/apple/macos/.build/release/ExactMac');
if (gpuNote.endsWith('.dylib')) copyFileSync(resolve(root, 'target/release', gpuNote), resolve(root, 'host/apple/macos/.build/release', gpuNote));
console.log(`host/apple: ${bin.replace(root + '/', '')} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s); GPU: ${gpuNote}`);
// --run: the app, with the dev loop's plan watched when host/web/dev.mjs is
// running (it writes host/web/dist/app.plan on every save).
if (args.includes('--run')) spawnSync(bin, [], { stdio: 'inherit', env: { ...env, EXACT_DEV_PLAN: resolve(root, 'host/web/dist/app.plan') } });
