// Host adapters belong to the bake, never to a game author.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';

export function gameShells(dir, game, workspace) {
  // Never ask Cargo to inspect generated members until they are complete. Even
  // --no-deps metadata rejects a missing target or a stale path dependency.
  const apps = new Map([[dir, {game, ...JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'))}]]);
  for (const group of ['games', 'bench']) {
    const parent = resolve(workspace, group);
    if (!existsSync(parent)) continue;
    for (const entry of readdirSync(parent, {withFileTypes:true})) {
      const appDir = resolve(parent, entry.name), manifest = resolve(appDir, 'app.json');
      if (!entry.isDirectory() || !existsSync(manifest)) continue;
      const app = JSON.parse(readFileSync(manifest, 'utf8'));
      if (app.game) apps.set(appDir, app);
    }
  }
  const root = resolve(workspace, '.shells'), desired = new Map();
  for (const [appDir, app] of apps) {
    const logicDir = resolve(appDir, 'logic'), {crate, type} = app.game;
    const manifest = resolve(logicDir, 'Cargo.toml');
    if (!existsSync(manifest) || Bun.TOML.parse(readFileSync(manifest, 'utf8')).package?.name !== crate) {
      throw new Error(`game.crate ${crate} must name the package in ${appDir}/logic`);
    }
    if (app.game.audio !== undefined && typeof app.game.audio !== "boolean") throw new Error("game.audio must be a boolean");
    if (app.game.assets !== undefined && typeof app.game.assets !== "boolean") throw new Error("game.assets must be a boolean");
    const name = crate.slice(0, -'-logic'.length);
    const key = createHash('sha256').update(app.app.id).digest('hex').slice(0, 24);
    for (const kind of ['gpu', 'web', 'apple', 'linux']) {
      const shellName = `${key}-${kind}`, shell = resolve(root, shellName);
      const header = `[package]\nname = "${name}-${kind}"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false${kind === 'linux' ? '' : `\n\n[lib]\ncrate-type = ["${kind === 'apple' ? 'staticlib' : 'cdylib'}", "rlib"]`}\n\n[dependencies]\n`;
      const dependencies = kind === 'gpu'
        ? `exact-game-render.workspace = true\n${app.game.audio === true ? "exact-game-audio.workspace = true\n" : ""}game-logic = { package = "${crate}", path = ${JSON.stringify(relative(shell, logicDir))} }\n\n[target.'cfg(target_arch = "wasm32")'.dependencies]\nwasm-bindgen.workspace = true\nwasm-bindgen-futures.workspace = true\nweb-sys.workspace = true\n\n[build-dependencies]\nexact-game-bake.workspace = true\n`
        : `exact-runner.workspace = true\nexact-${kind}.workspace = true\n\n[build-dependencies]\nexact-game-app.workspace = true\n`;
      desired.set(shellName, {
        'Cargo.toml': header + dependencies,
        [kind === 'linux' ? 'src/main.rs' : 'src/lib.rs']: kind === 'gpu' ? `exact_game_render::module!(game_logic::${type}${app.game.audio === true ? ", audio" : ""}${app.game.assets === true ? ", assets" : ""});\n` : 'include!(concat!(env!("OUT_DIR"), "/entry.rs"));\n',
        ...(kind === 'gpu' ? {'build.rs': `fn main() {\n    exact_game_bake::bake_art(${JSON.stringify(relative(shell, appDir))}).expect("bake art");\n}\n`} : {'build.rs': `fn main() {\n    exact_game_app::bake("${kind}", ${JSON.stringify(relative(shell, appDir))});\n}\n`}),
      });
    }
  }
  mkdirSync(root, {recursive:true});
  // The snapshot excludes this entire generated root, including its gitignore.
  if (!existsSync(resolve(root, '.gitignore'))) writeFileSync(resolve(root, '.gitignore'), '*\n!.gitignore\n');
  let changed = false;
  for (const [name, files] of desired) {
    const shell = resolve(root, name);
    if (Object.entries(files).every(([file, bytes]) => existsSync(resolve(shell, file)) && readFileSync(resolve(shell, file), 'utf8') === bytes)) continue;
    const stage = mkdtempSync(resolve(workspace, '.shell-stage-'));
    try {
      const replacement = resolve(stage, 'new'), backup = resolve(stage, 'old');
      for (const [file, bytes] of Object.entries(files)) {
        const path = resolve(replacement, file);
        mkdirSync(dirname(path), {recursive:true}); writeFileSync(path, bytes);
      }
      if (existsSync(shell)) renameSync(shell, backup);
      try { renameSync(replacement, shell); }
      catch (error) { if (existsSync(backup)) renameSync(backup, shell); throw error; }
      changed = true;
    } finally { rmSync(stage, {recursive:true, force:true}); }
  }
  // This directory contains only generated crates. Prune old names and deleted
  // games too, so one abandoned member cannot poison every surviving game.
  for (const entry of readdirSync(root, {withFileTypes:true})) {
    if (entry.isDirectory() && !desired.has(entry.name)) {
      rmSync(resolve(root, entry.name), {recursive:true}); changed = true;
    }
  }
  // Adding members changes only local package entries; retain dependency pins.
  if (changed) {
    const locked = spawnSync('cargo', ['metadata', '--locked', '--offline', '--format-version', '1'], {cwd:workspace, stdio:'ignore'});
    if (locked.status !== 0) {
      const updated = spawnSync('cargo', ['update', '-p', 'exact-game', '--offline', '--quiet'], {cwd:workspace, encoding:'utf8'});
      if (updated.error || updated.status !== 0) throw new Error(`game shells: ${updated.error?.message ?? updated.stderr}`);
    }
  }
  return game.crate.slice(0, -'-logic'.length);
}
