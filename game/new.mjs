#!/usr/bin/env bun
import { cpSync, existsSync, readFileSync, readdirSync, realpathSync, writeFileSync } from 'node:fs';
import { basename, relative, resolve } from 'node:path';
import { gameDefaults } from './app/shells.mjs';

export function createGame(destination, directory = import.meta.dir, options = {}) {
  const local = destination === '.' || destination?.includes('/');
  const name = local ? basename(resolve(destination)) : destination;
  if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name ?? '') || /-(web|apple|linux|gpu)$/.test(name)) {
    throw new Error('Usage: bun game/new.mjs <name|path> [--assets] (lowercase-hyphenated name, no host suffix)');
  }
  destination = local ? resolve(destination) : resolve(directory, 'games', name);
  if (existsSync(destination) && readdirSync(destination).length) throw new Error(`Game already exists: ${destination}`);
  cpSync(resolve(directory, 'new'), destination, {recursive:true});
  destination = realpathSync(destination);
  directory = realpathSync(directory);
  const title = name.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
  for (const file of readdirSync(destination, {recursive:true, withFileTypes:true})) {
    if (!file.isFile()) continue;
    const path = resolve(file.parentPath, file.name);
    writeFileSync(path, readFileSync(path, 'utf8').replaceAll('small-game', name)
      .replaceAll('small_game', name.replaceAll('-', '_'))
      .replaceAll('Small game', title));
  }
  writeFileSync(resolve(destination, '.gitignore'), '/target/\n/dist/\n/dist.previous/\n/artifacts/\n/.shells/\n/app.contract.d.ts\n');
  const proofPath = resolve(destination,'proof.mjs');
  if (existsSync(proofPath)) writeFileSync(proofPath,readFileSync(proofPath,'utf8').replace("'../../proof.mjs'",JSON.stringify(relative(destination,resolve(directory,'proof.mjs')))));
  const manifest = gameDefaults(destination, directory);
  writeFileSync(resolve(destination, 'app.json'), JSON.stringify({
    app:{id:manifest.app.id, name:manifest.app.name},
    game:{...manifest.game, ...(options.assets === true ? {assets:true} : {})},
  }, null, 2) + "\n");

  const quote = path => `'${path.replaceAll("'", "'\\''")}'`;
  const argument = local ? quote(destination) : name;
  const proof = local ? quote(resolve(destination, 'proof.mjs')) : `game/games/${name}/proof.mjs`;
  return `The template supplies Cargo.lock beside app.json; first bake resolves it offline and locked in .shells/.\nCreated ${local ? destination : `game/games/${name}`}\n  bun game/dev.mjs ${argument}\n  bun game/prove.mjs ${argument}
  bun ${proof} web`;
}

if (import.meta.main) console.log(createGame(process.argv[2], undefined, {assets:process.argv.includes("--assets")}));
