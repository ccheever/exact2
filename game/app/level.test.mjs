import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {gameDefaults, gameShells} from './shells.mjs';

test('a level-only game bakes its declared type without an art directory', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-level-shell-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true});
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game:{}}));
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    const game = gameDefaults(dir).game;
    gameShells(dir, game, resolve(import.meta.dir, '..'));
    const build = readFileSync(resolve(dir, '.shells/gpu/build.rs'), 'utf8');
    expect(build).toContain('bake_files::bake_game_level::<game_logic::Island>');
    expect(build).toContain('bake/src/files.rs');
    expect(build).not.toContain('bake_art(');
    expect(readFileSync(resolve(dir, '.shells/gpu/Cargo.toml'), 'utf8')).not.toContain('exact-game-bake.workspace = true');
    expect(readFileSync(resolve(dir, '.shells/web/Cargo.toml'), 'utf8')).not.toContain('exact-data-host');
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('a declared app data crate is linked into generated game hosts', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-data-shell-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true});
    mkdirSync(resolve(dir, 'data/src'), {recursive:true});
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game:{data:{crate:'island-data',type:'SaveData'}}}));
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    writeFileSync(resolve(dir, 'data/Cargo.toml'), '[package]\nname = "island-data"\nversion = "0.1.0"\nedition = "2021"\nworkspace = "../.shells"\n');
    const game = gameDefaults(dir).game;
    gameShells(dir, game, resolve(import.meta.dir, '..'));
    const workspace = readFileSync(resolve(dir, '.shells/Cargo.toml'), 'utf8');
    const web = readFileSync(resolve(dir, '.shells/web/Cargo.toml'), 'utf8');
    const linux = readFileSync(resolve(dir, '.shells/linux/Cargo.toml'), 'utf8');
    expect(workspace).toContain('"../data"');
    for (const manifest of [web, linux]) {
      expect(manifest).toContain('exact-data-host.workspace = true');
      expect(manifest.match(/app-data = /g)).toHaveLength(2);
    }
    for (const host of ['web', 'apple', 'linux']) {
      expect(readFileSync(resolve(dir, `.shells/${host}/build.rs`), 'utf8'))
        .toContain('bake_data_declaration::<app_data::SaveData>');
    }
    writeFileSync(resolve(dir, 'data/Cargo.toml'), '[package]\nname = "other-data"\nversion = "0.1.0"\nworkspace = "../.shells"\n');
    expect(() => gameShells(dir, game, resolve(import.meta.dir, '..')))
      .toThrow('game.data.crate island-data must name the package');
    writeFileSync(resolve(dir, 'data/Cargo.toml'), '[package]\nname = "island-data"\nversion = "0.1.0"\nworkspace = ".."\n');
    expect(() => gameShells(dir, game, resolve(import.meta.dir, '..')))
      .toThrow('set package.workspace = "../.shells"');
  } finally { rmSync(dir, {recursive:true, force:true}); }
});
