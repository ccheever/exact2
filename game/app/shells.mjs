// Host adapters belong to the bake, never to a game author.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';

// Resolve authored keys in memory; only the bake writes the resolved manifest.
const merge = (base, overrides) => {
  const result = {...base};
  for (const [key, value] of Object.entries(overrides)) {
    result[key] = value && typeof value === "object" && !Array.isArray(value) && base[key] && typeof base[key] === "object" && !Array.isArray(base[key]) ? merge(base[key], value) : value;
  }
  return result;
};
const gameRoot = resolve(import.meta.dir, '..');
const hasArt = dir => existsSync(resolve(dir, 'art')) || existsSync(resolve(dir, '.baked-assets.json'));
const writeChanged = (path, text, authored = true) => {
  if (!existsSync(path) || readFileSync(path, "utf8") !== text) {
    if (authored && process.env.EXACT_PROOF_REPIN === '1') throw new Error(`repin refused: manifest normalization would write ${path}; run the ordinary bake first`);
    writeFileSync(path, text);
  }
};
// Only the literal Game declaration used by the template is inferred; unusual
// Rust exports/IDs use app.json. Rust remains responsible for checking the type.
export function gameDefaults(dir, workspace = gameRoot) {
  const source = resolve(dir, 'logic/src/lib.rs'), path = resolve(dir, 'app.json');
  if (!existsSync(source)) return null;
  const authored = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : {};
  const overrides = authored;
  const rust = readFileSync(source, 'utf8');
  const declaration = /impl\s+(?:exact_game::)?Game\s+for\s+(\w+)\s*\{[^{}]*?\bconst\s+ID\s*:\s*&'static\s+str\s*=\s*"([a-z][a-z0-9-]*)"/.exec(rust);
  if (!declaration && !(overrides.game?.crate && overrides.game?.type)) return null;
  if (!declaration && (!(overrides.id ?? overrides.app?.id) || !(overrides.name ?? overrides.app?.name)))
    throw new Error(`${path}: explicit id and name are required when the Game declaration cannot be inferred`);
  const [, type, gameId] = declaration ?? [], name = basename(dir);
  const cargoPath = resolve(dir, 'logic/Cargo.toml');
  const cargo = existsSync(cargoPath) ? readFileSync(cargoPath, 'utf8') : null;
  const crate = overrides.game?.crate ?? (cargo ? Bun.TOML.parse(cargo).package.name : `${name}-logic`);
  const title = overrides.name ?? overrides.app?.name ?? name.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
  const id = overrides.id ?? overrides.app?.id ?? `com.exact.${gameId}`;
  const app = merge({
    name:title, short_name:title, id, start_url:'/', display:'standalone',
    theme_color:'#202731', background_color:'#202731',
    app:{id, name:title},
    host:{macos:{minimumOS:'14.0', window:{width:1280,height:720}}, ios:{minimumOS:'17.0',deviceFamily:['iphone','ipad']},web:{}},
    game:{crate, type}, rust:false,
    deploy:{store:{web:'0',macos:'0',ios:'0',linux:'0'}},
  }, overrides);
  const manifest = resolve(dir, 'logic/Cargo.toml');
  if (app.game && !existsSync(manifest)) writeChanged(manifest,
    `# Author-owned game logic manifest; the bake never rewrites this file.\n[package]\nname = "${app.game.crate}"\nversion = "0.1.0"\nedition = "2021"\nlicense = "MIT"\nworkspace = "../.shells"\n\n[dependencies]\nexact-game = { path = ${JSON.stringify(relative(resolve(dir, 'logic'), resolve(workspace, 'engine')))} }\n`);
  return app;
}

export function gameShells(dir, game, workspace) {
  // Only this app is materialized. Each bake owns a generated Cargo workspace.
  const app = {game, ...gameDefaults(dir, workspace)};
  const {crate, type, data} = app.game, name = crate.slice(0, -'-logic'.length);
  const root = resolve(dir, '.shells');
  const source = existsSync(resolve(workspace, 'Cargo.toml')) ? workspace : gameRoot;
  const cargo = Bun.TOML.parse(readFileSync(resolve(source, 'Cargo.toml'), 'utf8'));
  cargo.workspace.members = ['gpu','web','apple','linux','../logic', ...(data ? ['../data'] : [])];
  delete cargo.workspace.exclude;
  // Engine crates are dependencies here: the wildcard already optimizes them.
  // Retain member overrides and any settings distinct from that wildcard.
  const members = new Set([crate, data?.crate, ...['gpu','web','apple','linux'].map(kind => `${name}-${kind}`)].filter(Boolean));
  for (const profile of Object.values(cargo.profile ?? {})) {
    const defaults = profile.package?.['*'];
    if (defaults) for (const [name, settings] of Object.entries(profile.package)) {
      if (/^[\w-]+$/.test(name) && !members.has(name) && Object.entries(settings).every(([key,value]) => value === defaults[key])) delete profile.package[name];
    }
  }
  for (const deps of [cargo.workspace.dependencies, ...Object.values(cargo.patch ?? {})]) {
    for (const dep of Object.values(deps ?? {})) if (dep.path) dep.path = relative(root, resolve(source, dep.path));
  }
  const toml = value => value && typeof value === 'object' && !Array.isArray(value)
    ? `{ ${Object.entries(value).map(([key,item])=>`${JSON.stringify(key)} = ${toml(item)}`).join(', ')} }`
    : Array.isArray(value) ? `[${value.map(toml).join(', ')}]` : JSON.stringify(value);
  mkdirSync(root,{recursive:true});
  mkdirSync(resolve(root,'.cargo'),{recursive:true});
  // Cargo already inherits the SDK's config when this workspace is inside it.
  const inherited = realpathSync(root).startsWith(realpathSync(gameRoot) + '/');
  const config = (inherited ? '[build]\n' : readFileSync(resolve(gameRoot,'.cargo/config.toml'),'utf8'))
    .replace('[build]', `[build]\nbuild-dir = ${JSON.stringify(resolve(source,'target'))}`);
  writeChanged(resolve(root,'.cargo/config.toml'), config, false);
  writeChanged(resolve(root, 'app.json'), JSON.stringify(app, null, 2) + '\n', false);
  writeChanged(resolve(root,'Cargo.toml'), '# Generated by the game bake.\n' + Object.entries(cargo)
    .map(([key,value])=>`[${key}]\n${Object.entries(value).map(([key,value])=>`${JSON.stringify(key)} = ${toml(value)}\n`).join('')}`).join('\n'), false);
  const lock = resolve(dir, 'Cargo.lock');
  if (existsSync(lock)) writeChanged(resolve(root, 'Cargo.lock'), readFileSync(lock, 'utf8'), false);
  const appDir = dir;
  const logicDir = resolve(appDir, 'logic');
  const manifest = resolve(logicDir, 'Cargo.toml');
  if (!existsSync(manifest) || Bun.TOML.parse(readFileSync(manifest, 'utf8')).package?.name !== crate) {
    throw new Error(`game.crate ${crate} must name the package in ${appDir}/logic`);
  }
  if (Bun.TOML.parse(readFileSync(manifest, 'utf8')).package.workspace !== '../.shells') {
    throw new Error(`${manifest}: set package.workspace = "../.shells" so the app owns its logic`);
  }
  let dataDir;
  if (data !== undefined) {
    if (!data || typeof data !== 'object' || Array.isArray(data)
        || Object.keys(data).some(key => !['crate', 'type'].includes(key))
        || typeof data.crate !== 'string' || !/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*-data$/.test(data.crate)
        || typeof data.type !== 'string' || !/^[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*$/.test(data.type)) {
      throw new Error('game.data must contain only a <name>-data crate and Rust type');
    }
    dataDir = resolve(appDir, 'data');
    const dataManifest = resolve(dataDir, 'Cargo.toml');
    if (!existsSync(dataManifest)) throw new Error(`game.data.crate ${data.crate} requires ${dataManifest}`);
    const declared = Bun.TOML.parse(readFileSync(dataManifest, 'utf8')).package;
    if (declared?.name !== data.crate) {
      throw new Error(`game.data.crate ${data.crate} must name the package in ${dataDir}`);
    }
    if (declared.workspace !== '../.shells') {
      throw new Error(`${dataManifest}: set package.workspace = "../.shells" so the app owns its data`);
    }
  }
  if (app.game.audio !== undefined && typeof app.game.audio !== "boolean") throw new Error("game.audio must be a boolean");
  if (app.game.assets !== undefined && typeof app.game.assets !== "boolean") throw new Error("game.assets must be a boolean");
  // Resolve validates manifest syntax; Rust checks this path's exported type
  // when compiling the generated GPU shell, including macro/cfg exports.
  // An absent art directory must not be a Cargo watch: missing paths are always
  // dirty. Keep the baker only for art or a final generated-output cleanup.
  const bakeArt = hasArt(appDir);
  // The snapshot excludes this entire generated root, including its gitignore.
  if (!existsSync(resolve(root, '.gitignore'))) writeFileSync(resolve(root, '.gitignore'), '*\n!.gitignore\n');
  for (const kind of ['gpu', 'web', 'apple', 'linux']) {
    const shell = resolve(root, kind);
    const levelBake = relative(shell, resolve(source, 'bake/src/files.rs'));
    // These adapters contain entry points only; tests live in authored crates.
    const target = kind === 'linux'
      ? `[[bin]]\nname = "${name}-linux"\npath = "src/main.rs"\ntest = false`
      : `[lib]\ncrate-type = ["${kind === 'apple' ? 'staticlib' : 'cdylib'}"]\ntest = false\ndoctest = false`;
    const header = `[package]\nname = "${name}-${kind}"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false\n\n${target}\n\n[dependencies]\n`;
    const dataDependency = data ? `exact-data-host.workspace = true\napp-data = { package = "${data.crate}", path = ${JSON.stringify(relative(shell, dataDir))} }\n` : '';
    const dataBuildDependency = data ? `app-data = { package = "${data.crate}", path = ${JSON.stringify(relative(shell, dataDir))} }\n` : '';
    const dependencies = kind === 'gpu'
      ? `exact-game-render.workspace = true\n${app.game.audio === true ? "exact-game-audio.workspace = true\n" : ""}game-logic = { package = "${crate}", path = ${JSON.stringify(relative(shell, logicDir))} }\n\n[target.'cfg(target_arch = "wasm32")'.dependencies]\nwasm-bindgen.workspace = true\nwasm-bindgen-futures.workspace = true\nweb-sys.workspace = true\n\n[build-dependencies]\nexact-game.workspace = true\nserde_json = "1"\ngame-logic = { package = "${crate}", path = ${JSON.stringify(relative(shell, logicDir))} }\n${bakeArt ? 'exact-game-bake.workspace = true\n' : ''}`
      : `exact-runner.workspace = true\nexact-${kind}.workspace = true\n${dataDependency}\n[build-dependencies]\nexact-game-app.workspace = true\n${dataBuildDependency}`;
    const files = {
      'Cargo.toml': header + dependencies,
      [kind === 'linux' ? 'src/main.rs' : 'src/lib.rs']: kind === 'gpu' ? `exact_game_render::module!(game_logic::${type}${app.game.audio === true ? ", audio" : ""}${app.game.assets === true ? ", assets" : ""});\n` : 'include!(concat!(env!("OUT_DIR"), "/entry.rs"));\n',
      'build.rs': kind === 'gpu'
        ? `use exact_game::{Args, Game, Value};
use std::{env, fs, path::PathBuf};
#[path = ${JSON.stringify(levelBake)}]
mod bake_files;
fn level_bake_path() -> &'static str {
    ${JSON.stringify(levelBake)}
}
fn main() {
    type Options = <game_logic::${type} as Game>::Args;
    let arguments: Vec<_> = Options::FIELDS
        .iter()
        .zip(Options::default().values())
        .map(|((name, _), value)| {
            let value = match value {
                Value::Number(n) => serde_json::json!(n),
                Value::Bool(b) => serde_json::json!(b),
                Value::Str(s) => serde_json::json!(s.as_ref()),
                _ => panic!("unsupported surface argument default: {name}"),
            };
            serde_json::json!({"name": name, "default": value})
        })
        .collect();
    let name = <game_logic::${type} as Game>::NAME;
    let text = serde_json::to_string_pretty(&serde_json::json!({name: arguments})).unwrap() + "\\n";
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../surfaces.json");
    if fs::read_to_string(&path).ok().as_deref() != Some(&text) {
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&temporary, text).expect("write game surface declaration");
        fs::rename(temporary, &path).expect("publish complete game surface declaration");
    }
${bakeArt ? `    exact_game_bake::bake_art(${JSON.stringify(relative(shell, appDir))}).expect("bake art");\n` : ''}    bake_files::bake_game_level::<game_logic::${type}>(${JSON.stringify(relative(shell, appDir))}).expect("bake level");
    println!("cargo:rerun-if-changed={}", level_bake_path());
    println!("cargo:rerun-if-changed=build.rs");
}
`
        : data
          ? `fn main() {\n    exact_game_app::bake_data_declaration::<app_data::${data.type}>("${kind}", ${JSON.stringify(relative(shell, appDir))}, "app_data::${data.type}");\n}\n`
          : `fn main() {\n    exact_game_app::bake_declaration("${kind}", ${JSON.stringify(relative(shell, appDir))});\n}\n`,
    };
    if (Object.entries(files).every(([file, bytes]) => existsSync(resolve(shell, file)) && readFileSync(resolve(shell, file), 'utf8') === bytes)) continue;
    const stage = mkdtempSync(resolve(root, '.shell-stage-'));
    try {
      const replacement = resolve(stage, 'new'), backup = resolve(stage, 'old');
      for (const [file, bytes] of Object.entries(files)) {
        const path = resolve(replacement, file);
        mkdirSync(dirname(path), {recursive:true}); writeFileSync(path, bytes);
      }
      if (existsSync(shell)) renameSync(shell, backup);
      try { renameSync(replacement, shell); }
      catch (error) { if (existsSync(backup)) renameSync(backup, shell); throw error; }
    } finally { rmSync(stage, {recursive:true, force:true}); }
  }
  return game.crate.slice(0, -'-logic'.length);
}

// The template supplies the source lock. Every ordinary bake is locked;
// dependency edits require an explicit update, never a publisher's cache choice.
export function prepareGame(dir, game, source = gameRoot, {updateLock = false, target, env = process.env} = {}) {
  const root = resolve(dir, '.shells'), lock = resolve(dir, 'Cargo.lock');
  const captured = existsSync(lock), cached = resolve(root, 'Cargo.lock');
  if (!captured && existsSync(cached)) {
    rmSync(cached);
    throw new Error(`${cached}: stale shell lock without a captured source Cargo.lock; removed it. Restore the source lock or explicitly --update-lock`);
  }
  gameShells(dir, game, source);
  if (!captured && !updateLock) {
    const seed = resolve(source, 'new/Cargo.lock');
    if (!existsSync(seed)) throw new Error(`${lock}: missing captured lock and template seed; create the game with game/new.mjs or explicitly --update-lock`);
    const name = game.crate.replace(/-logic$/, '');
    writeChanged(cached, readFileSync(seed, 'utf8').replaceAll('small-game', name), false);
  }
  const result = spawnSync('cargo', ['metadata', '--offline', ...(!updateLock ? ['--locked'] : []), '--format-version', '1', ...(target ? ['--filter-platform', target] : [])], {
    cwd:root, env, encoding:'utf8', maxBuffer:64 * 1024 * 1024,
  });
  if (result.status !== 0) throw new Error(`game Cargo graph: ${result.stderr || result.error?.message}\nOffline resolution requires a populated Cargo cache: cargo fetch --locked --manifest-path ${JSON.stringify(resolve(root, "Cargo.toml"))}\nTo capture dependency changes: bun game/app/shells.mjs ${JSON.stringify(dir)} --update-lock`);
  if (!captured || updateLock) writeChanged(lock, readFileSync(resolve(root, 'Cargo.lock'), 'utf8'), captured);
  return JSON.parse(result.stdout);
}

if (import.meta.main) {
  if (process.argv.includes('--test')) {
    let failed=false;
    const requested = process.argv.slice(2).find(arg => !arg.startsWith('--'));
    const dirs = requested ? [resolve(requested)] : ['games','bench'].flatMap(group =>
      readdirSync(resolve(gameRoot, group)).map(name => resolve(gameRoot, group, name)))
      .filter(dir => existsSync(resolve(dir, 'logic/src/lib.rs')));
    for (const dir of dirs) {
      try {
        prepareGame(dir, gameDefaults(dir).game);
        const env = {...process.env, CARGO_TARGET_DIR:process.env.CARGO_TARGET_DIR ?? resolve(gameRoot,'target')};
        if (hasArt(dir)) {
          const result=spawnSync('cargo',['run','--manifest-path',resolve(gameRoot,'Cargo.toml'),'-p','exact-game-bake','--locked','--offline','--','--art',dir], {
            cwd:gameRoot,env,stdio:'inherit',
          });
          if (result.error || result.status !== 0) throw new Error(`game art bake: ${result.error?.message ?? `cargo exited ${result.status ?? result.signal}`}`);
        }
        const result=spawnSync('cargo',['test','--workspace','--locked','--offline','--no-fail-fast'], {
          cwd:resolve(dir,'.shells'),env,stdio:'inherit',
        });
        failed ||= result.status !== 0;
      } catch(error) {console.error(error);failed=true;}
    }
    process.exitCode=failed?1:0;
  } else {
    const dir = resolve(process.argv[2]);
    const game = gameDefaults(dir).game;
    if (process.argv.includes('--update-lock')) prepareGame(dir, game, gameRoot, {updateLock:true});
    else gameShells(dir, game, gameRoot);
  }
}
