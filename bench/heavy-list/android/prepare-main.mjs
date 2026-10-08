#!/usr/bin/env bun
// Install benchmark scaffolding only; leave the selected main runtime unchanged.
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url)), app = resolve(here, '../exact-heavylist');
if (process.argv.length !==3) throw new Error('Usage: bun bench/heavy-list/android/prepare-main.mjs /path/to/main-checkout');
const root = resolve(process.argv[2]), destination = resolve(root, 'bench/heavy-list/exact-heavylist');
if (!existsSync(resolve(root, 'host/linux/src/android.rs'))) throw new Error('Main checkout has no public Android Handle');
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
for (const file of ['app.contract', 'data/Cargo.toml', 'data/src/lib.rs']) {
    if (hash(resolve(app, file)) !==hash(resolve(destination, file))) throw new Error(`Original Heavy List input differs: ${file}`);
}
for (const file of ['android-bake.rs', 'android-main/Cargo.toml', 'android-main/build.rs', 'android-main/src/lib.rs']) {
    const output = resolve(destination, file);
    if (existsSync(output) && hash(output) !==hash(resolve(app, file))) throw new Error(`Existing adapter differs: ${file}`);
}
let cargo = readFileSync(resolve(destination, 'Cargo.toml'), 'utf8');
const members = cargo.match(/^members = \[(.*?)\]$/m);
if (!members) throw new Error('Expected the original nested Heavy List workspace');
if (!members[1].includes('"android-main"')) cargo = cargo.replace(members[0], `members = [${members[1]}, "android-main"]`);
const manifest = JSON.parse(readFileSync(resolve(destination, 'app.json'), 'utf8'));
manifest.host.android = { minSdk:29, targetSdk:36 };
manifest.deploy.store.android = '0';
mkdirSync(destination, { recursive:true });
cpSync(resolve(app, 'android-main'), resolve(destination, 'android-main'), { recursive:true });
cpSync(resolve(app, 'android-bake.rs'), resolve(destination, 'android-bake.rs'));
writeFileSync(resolve(destination, 'Cargo.toml'), cargo);
writeFileSync(resolve(destination, 'app.json'), JSON.stringify(manifest, null, 2)+'\n');
console.log(`Heavy List main adapter prepared: ${destination}`);
