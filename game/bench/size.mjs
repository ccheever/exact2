#!/usr/bin/env bun
// Shipped size plus pre-bindgen twiggy attribution. Diagnostic, never a gate.
// bun game/bench/size.mjs [label] [--no-build] [--app beacons|greybox|skinned-fixture]
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';

const root = resolve(import.meta.dir, '../..');
const args = process.argv.slice(2);
const appAt = args.indexOf('--app');
const name = appAt < 0 ? 'beacons' : args.splice(appAt, 2)[1];
if (!['beacons', 'greybox', 'skinned-fixture'].includes(name))
  throw new Error('Use --app beacons, greybox or skinned-fixture');
const app = resolve(root, 'game/games', name);
const output = resolve(app, 'target/d3-size');
const dist = resolve(output, 'dist');
const env = { ...process.env, DEVELOPER_DIR: '/Library/Developer/CommandLineTools',
  SDKROOT: '/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk',
  EXACT_UPDATE_TRUST: 'development', EXACT_IDENTITY: '-', EXACT_APP_DIR: app,
  EXACT_WEB_DIST: dist, CARGO_TARGET_DIR: resolve(app, 'target') };
const label = args.find(arg => !arg.startsWith('--')) ?? 'current';
if (!/^[a-zA-Z0-9_-]+$/.test(label)) throw new Error('Use a filename-safe label');
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, env, encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024, ...options });
  if (result.error || result.status !== 0)
    throw new Error(`${command}: ${result.error ?? result.stderr ?? result.status}`);
  return result.stdout;
}
mkdirSync(output, { recursive: true });
if (!args.includes('--no-build')) {
  run('bun', ['host/web/build.mjs'], { stdio: 'inherit' });
}
const preopt = resolve(app, `target/wasm32-unknown-unknown/web/${name.replaceAll('-', '_')}_gpu.wasm`);
const shipped = readFileSync(resolve(dist, 'gpu_bg.wasm'));
const rows = JSON.parse(run('twiggy', ['top', '-n', '20000', '-f', 'json', preopt]));
// Drop metadata only, never code/data. Trait impl names are attributed to their
// implementing type's crate (e.g. <alloc::vec::Vec<T> as core::...> -> alloc).
function crate(name) {
  if (name.startsWith('data segment')) return name.match(/^data segment "([^"]+)"/)?.[1] ?? 'data';
  if (name.startsWith('custom section') || name.includes(' subsection')) return 'metadata';
  const path = name.match(/([A-Za-z_][A-Za-z_0-9]*)\[[a-f0-9]+\]::/) ?? name.match(/^(?:<)?([A-Za-z_][A-Za-z_0-9]*)::/);
  return path?.[1] ?? 'other';
}
const totals = {};
const modules = {};
for (const row of rows) {
  const owner = crate(row.name);
  totals[owner] = (totals[owner] ?? 0) + row.shallow_size;
  const module = row.name.replace(/\[[a-f0-9]+\]/g, '').replace(/^<+/, '').match(/^(exact_game(?:_render)?)::([A-Za-z_0-9]+)::/);
  if (module) {
    const key = `${module[1]}::${module[2]}`;
    modules[key] = (modules[key] ?? 0) + row.shallow_size;
  }
}
const result = { label, raw: shipped.length, gzip: gzipSync(shipped, { level: 9 }).length,
  sha256: createHash('sha256').update(shipped).digest('hex'),
  preoptBytes: readFileSync(preopt).length, totals, modules };
writeFileSync(resolve(output, `${label}.json`), JSON.stringify(result, null, 2) + '\n');
writeFileSync(resolve(output, `${label}-twiggy.json`), JSON.stringify(rows) + '\n');
console.log('| build | shipped bytes | gzip bytes |\n|---|---:|---:|');
console.log(`| ${label} | ${result.raw.toLocaleString('en-US')} | ${result.gzip.toLocaleString('en-US')} |`);
console.log('\nCrate/module attribution is pre-bindgen and pre-`wasm-opt`; it is not a breakdown of the shipped bytes above.');
console.log('\n| pre-opt crate / section | bytes |\n|---|---:|');
for (const [name, bytes] of Object.entries(totals).filter(([name]) => name !== 'metadata').sort((a,b) => b[1]-a[1]))
  console.log(`| ${name} | ${bytes.toLocaleString('en-US')} |`);
console.log('\n| engine / render module | pre-opt bytes |\n|---|---:|');
for (const [name, bytes] of Object.entries(modules).sort((a,b) => b[1]-a[1]))
  console.log(`| ${name} | ${bytes.toLocaleString('en-US')} |`);
console.log(`\nFull attribution and measurements: ${resolve(output, `${label}.json`)}`);
