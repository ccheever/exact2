#!/usr/bin/env bun
import { cpSync, existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { gameDefaults, gameShells } from './app/shells.mjs';

export function createGame(name, directory = import.meta.dir, run = spawnSync, options = {}) {
  if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name ?? '') || /-(web|apple|linux|gpu)$/.test(name)) {
    throw new Error('Usage: bun game/new.mjs <lowercase-hyphenated-name> (no host suffix)');
  }
  const destination = resolve(directory, 'games', name);
  if (existsSync(destination)) throw new Error(`Game already exists: ${destination}`);
  cpSync(resolve(directory, 'new'), destination, {recursive:true});
  const title = name.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
  for (const file of readdirSync(destination, {recursive:true, withFileTypes:true})) {
    if (!file.isFile()) continue;
    const path = resolve(file.parentPath, file.name);
    writeFileSync(path, readFileSync(path, 'utf8').replaceAll('small-game', name)
      .replaceAll('small_game', name.replaceAll('-', '_'))
      .replaceAll('Small game', title));
  }
  // Synthesize this game's host shells and prune orphans first: an abandoned
  // shell (a deleted game's) would fail every cargo command below.
  const appJson = resolve(destination, 'app.json');
  const manifest = gameDefaults(destination) ?? {};
  if (options.assets === true && manifest.game) {
    manifest.game.assets = true;
    writeFileSync(appJson, JSON.stringify({game:{assets:true}}, null, 2) + "\n");
  }
  if (manifest.game) gameShells(destination, manifest.game, directory);
  // A locked resolution is read-only, including when these packages were already recorded.
  const check = run('cargo', ['metadata', '--locked', '--offline', '--format-version', '1'],
    {cwd:directory, stdio:['ignore','ignore','pipe'], encoding:'utf8'});
  if (check.error) throw check.error;
  let action;
  if (check.status === 0) {
    action = 'Shared game lockfile unchanged (packages already registered).';
  } else {
    const args = existsSync(resolve(directory, 'Cargo.lock'))
      // New members cannot be selected by -p until locked. The engine is an existing
      // path package: resolve the new members while retaining other dependency pins.
      ? ['update', '-p', 'exact-game', '--offline', '--quiet']
      : ['generate-lockfile', '--offline', '--quiet'];
    const lock = run('cargo', args, {cwd:directory, stdio:'inherit'});
    if (lock.error) throw lock.error;
    if (lock.status !== 0) throw new Error(`Could not register packages in the shared game lockfile: ${check.stderr ?? ''}`);
    action = `Shared game lockfile registered ${name} (${args.slice(0, args.indexOf('--offline')).join(' ')}).`;
  }
  return `${action}\nCreated game/games/${name}\n  bun game/dev.mjs ${name}\n  bun game/games/${name}/proof.mjs linux
  bun game/games/${name}/proof.mjs web --screenshot-only
  bun game/prove.mjs ${name} --repin`;
}

if (import.meta.main) console.log(createGame(process.argv[2], undefined, undefined, {assets:process.argv.includes("--assets")}));
