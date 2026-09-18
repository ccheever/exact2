#!/usr/bin/env bun
import { cpSync, existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const name = process.argv[2];
if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name ?? '') || /-(web|apple|linux|gpu)$/.test(name)) {
  throw new Error('Usage: bun game/new.mjs <lowercase-hyphenated-name> (no host suffix)');
}
const destination = resolve(import.meta.dir, 'games', name);
if (existsSync(destination)) throw new Error(`Game already exists: ${destination}`);
cpSync(resolve(import.meta.dir, 'new'), destination, {recursive:true});
const title = name.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
for (const file of readdirSync(destination, {recursive:true, withFileTypes:true})) {
  if (!file.isFile()) continue;
  const path = resolve(file.parentPath, file.name);
  writeFileSync(path, readFileSync(path, 'utf8').replaceAll('small-game', name)
    .replaceAll('small_game', name.replaceAll('-', '_'))
    .replaceAll('Small game', title));
}
// Register the copied packages without upgrading dependencies; bakes use --locked.
const lock = spawnSync('cargo', ['update', '--workspace', '--offline', '--quiet'], {cwd:import.meta.dir, stdio:'inherit'});
if (lock.status !== 0) throw new Error('Could not update the shared game lockfile');
console.log(`Created game/games/${name}\n  bun game/dev.mjs ${name}\n  bun game/games/${name}/proof.mjs web`);
