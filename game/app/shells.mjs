// Host adapters belong to the bake, never to a game author.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';

export function gameShells(dir, game, workspace) {
  const cargo = args => {
    const result = spawnSync('cargo', args, {cwd:workspace, encoding:'utf8', maxBuffer:32 * 1024 * 1024});
    if (result.error || result.status !== 0) throw new Error(`game shells: cargo ${args.join(' ')}: ${result.error?.message ?? result.stderr}`);
    return result.stdout;
  };
  const metadata = JSON.parse(cargo(['metadata', '--no-deps', '--offline', '--format-version', '1']));
  const logic = metadata.packages.find(pkg => pkg.name === game.crate);
  if (!logic || resolve(dirname(logic.manifest_path)) !== resolve(dir, 'logic')) {
    throw new Error(`game.crate ${game.crate} must name the package in ${dir}/logic`);
  }
  const name = game.crate.slice(0, -'-logic'.length);
  let changed = false;
  const write = (path, source) => {
    if (existsSync(path) && readFileSync(path, 'utf8') === source) return;
    mkdirSync(dirname(path), {recursive:true});
    writeFileSync(path, source);
    changed = true;
  };
  for (const kind of ['gpu', 'web', 'apple']) {
    const shell = resolve(workspace, '.shells', `${name}-${kind}`);
    const header = `[package]\nname = "${name}-${kind}"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false\n\n[lib]\ncrate-type = ["${kind === 'apple' ? 'staticlib' : 'cdylib'}", "rlib"]\n\n[dependencies]\n`;
    const dependencies = kind === 'gpu'
      ? `exact-game-render.workspace = true\ngame-logic = { package = "${game.crate}", path = ${JSON.stringify(relative(shell, dirname(logic.manifest_path)))} }\n\n[target.'cfg(target_arch = "wasm32")'.dependencies]\nwasm-bindgen.workspace = true\nwasm-bindgen-futures.workspace = true\nweb-sys.workspace = true\n`
      : `exact-runner.workspace = true\nexact-${kind}.workspace = true\n\n[build-dependencies]\nexact-game-app.workspace = true\n`;
    write(resolve(shell, 'Cargo.toml'), header + dependencies);
    write(resolve(shell, 'src/lib.rs'), kind === 'gpu'
      ? `exact_game_render::module!(game_logic::${game.type});\n`
      : 'include!(concat!(env!("OUT_DIR"), "/entry.rs"));\n');
    if (kind !== 'gpu') write(resolve(shell, 'build.rs'), `fn main() {\n    exact_game_app::bake("${kind}", ${JSON.stringify(relative(shell, dir))});\n}\n`);
  }
  // Adding members changes only local package entries; retain all dependency pins.
  if (changed) {
    const locked = spawnSync('cargo', ['metadata', '--locked', '--offline', '--format-version', '1'], {cwd:workspace, stdio:'ignore'});
    if (locked.status !== 0) cargo(['update', '-p', 'exact-game', '--offline', '--quiet']);
  }
  return name;
}
