#!/usr/bin/env node
// Build the macOS app: the app's static library (cargo, release), then the
// presenter (swift build) linked against it. Usage:
//   node host/apple/build.mjs [crate=caltrain-apple] [--run]
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const args = process.argv.slice(2);
const crate = args.find((a) => !a.startsWith('--')) ?? 'caltrain-apple';
const root = resolve(new URL('../..', import.meta.url).pathname);
const t0 = Date.now();
const cargo = spawnSync('cargo', ['build', '--release', '-p', crate], { cwd: root, stdio: 'inherit' });
if (cargo.status !== 0) process.exit(cargo.status ?? 1);
const t1 = Date.now();
const env = { ...process.env, EXACT_LIB_DIR: resolve(root, 'target/release'), EXACT_LIB: crate.replace(/-/g, '_') };
const swift = spawnSync('swift', ['build', '-c', 'release'], { cwd: resolve(root, 'host/apple/macos'), stdio: 'inherit', env });
if (swift.status !== 0) process.exit(swift.status ?? 1);
const t2 = Date.now();
const bin = resolve(root, 'host/apple/macos/.build/release/ExactMac');
console.log(`host/apple: ${bin.replace(root + '/', '')} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s)`);
if (args.includes('--run')) spawnSync(bin, [], { stdio: 'inherit', env });
