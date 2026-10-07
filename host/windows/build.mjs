#!/usr/bin/env bun
// The ordinary bake, packaged as a native executable, GPU DLL and baked assets.
import { copyFileSync, cpSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { bakeTarget, buildBake, copyShaders, developmentBuildEnv, resolveApp } from '../../scripts/app.mjs';

if (process.platform !== 'win32') throw new Error('build the Windows host on Windows with the MSVC Rust target');
const args = process.argv.slice(2);
const app = resolveApp(args.find(arg => !arg.startsWith('--')));
const target = bakeTarget('windows');
const profile = args.includes('--release') ? 'release' : 'gpu-dev';
const build = buildBake(app, 'windows', target, { profile, env: developmentBuildEnv(app) });
const output = resolve(app.dir, 'dist-windows');
mkdirSync(output, {recursive:true});
let executable;
for (const product of build.products) {
  if (!/\.(exe|dll)$/.test(product.path)) continue;
  const destination = resolve(output, basename(product.path));
  copyFileSync(product.path, destination);
  if (product.path.endsWith('.exe')) executable = destination;
}
if (!executable) throw new Error('Windows bake produced no executable');
for (const name of ['assets','deck']) {
  const source = resolve(app.dir, name);
  if (existsSync(source)) cpSync(source, resolve(output, name), {recursive:true});
}
copyShaders(app, resolve(output, 'shaders'), {replace:true});
writeFileSync(resolve(output, 'compat.json'), JSON.stringify(build.compat, null, 2) + '\n');
console.log(`Windows game: ${executable}`);
if (args.includes('--run')) {
  const result = spawnSync(executable, [], {cwd:output, env:process.env, stdio:'inherit'});
  process.exitCode = result.status ?? 1;
}
