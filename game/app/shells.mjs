// Host adapters belong to the bake, never to a game author.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { shaderFiles } from '../../scripts/app.mjs';

// Resolve authored keys in memory; only the bake writes the resolved manifest.
const merge = (base, overrides) => {
  const result = {...base};
  for (const [key, value] of Object.entries(overrides)) {
    result[key] = value && typeof value === "object" && !Array.isArray(value) && base[key] && typeof base[key] === "object" && !Array.isArray(base[key]) ? merge(base[key], value) : value;
  }
  return result;
};
// Not Bun's import.meta.dir: apps outside this repo import this under Node.
const gameRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const hasArt = dir => existsSync(resolve(dir, 'art')) || existsSync(resolve(dir, '.baked-assets.json'));
const writeChanged = (path, text, authored = true) => {
  if (!existsSync(path) || readFileSync(path, "utf8") !== text) {
    if (authored && process.env.EXACT_PROOF_REPIN === '1') throw new Error(`repin refused: manifest normalization would write ${path}; run the ordinary bake first`);
    writeFileSync(path, text);
  }
};
// Only the literal Game declaration used by the template is inferred; unusual
// Rust exports/IDs use app.json. Rust remains responsible for checking the type.
export function gameDefaults(dir) {
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
  // The crate follows Game::ID, not the directory: a clone under another name still builds.
  const crate = overrides.game?.crate ?? (cargo ? Bun.TOML.parse(cargo).package.name : `${gameId ?? name}-logic`);
  const title = overrides.name ?? overrides.app?.name ?? name.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
  const id = overrides.id ?? overrides.app?.id ?? `com.exact.${gameId}`;
  const app = merge({
    name:title, short_name:title, id, start_url:'/', display:'standalone',
    theme_color:'#202731', background_color:'#202731',
    app:{id, name:title},
    host:{macos:{minimumOS:'14.0', window:{width:1280,height:720}}, ios:{minimumOS:'17.0',deviceFamily:['iphone','ipad']},web:{}},
    game:{crate, type}, rust:false,
    deploy:{store:{web:'0',macos:'0',ios:'0',linux:'0',windows:'0'}},
  }, overrides);
  return app;
}

// A game without logic/Cargo.toml gets this generated member: its sources stay
// where the author wrote them. @ref llp/1046.003-game-engine-as-built.explainer.md#one-sdk-lock-2026-09-23
function logicManifest(dir, crate) {
  const logic = resolve(dir, 'logic'), targets = [];
  for (const [kind, table] of [['tests', 'test'], ['examples', 'example'], ['benches', 'bench']]) {
    const folder = resolve(logic, kind);
    if (!existsSync(folder)) continue;
    for (const entry of readdirSync(folder, {withFileTypes:true}).sort((a, b) => a.name < b.name ? -1 : 1)) {
      const file = entry.isFile() && entry.name.endsWith('.rs') ? entry.name
        : entry.isDirectory() && existsSync(resolve(folder, entry.name, 'main.rs')) ? `${entry.name}/main.rs` : null;
      if (file) targets.push(`\n[[${table}]]\nname = ${JSON.stringify(file.replace(/(\.rs|\/main\.rs)$/, ''))}\npath = ${JSON.stringify(`../../logic/${kind}/${file}`)}\n`);
    }
  }
  return `# Generated from ../../logic by the game bake. A game that adds dependencies\n# writes logic/Cargo.toml instead (game/README.md).\n[package]\nname = "${crate}"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false\nautobins = false\nautoexamples = false\nautotests = false\nautobenches = false\n\n[lib]\npath = "../../logic/src/lib.rs"\n${targets.join('')}\n[dependencies]\nexact-game.workspace = true\n`;
}

// @ref LLP 1046.008#b-authored-presentation-crate — GPU-only authored hooks (renamed
// game.render by the 2026-10-04 amendment).
function renderDeclaration(dir, render) {
  if (render === undefined) return;
  const rustPath = value => typeof value === 'string' && /^[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*$/.test(value);
  if (!render || typeof render !== 'object' || Array.isArray(render)
      || Object.keys(render).some(key => !['crate', 'hooks', 'shaders'].includes(key))
      || typeof render.crate !== 'string' || !/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*-render$/.test(render.crate)
      || !rustPath(render.hooks) || ('shaders' in render && !rustPath(render.shaders))) {
    throw new Error('game.render must contain only a <name>-render crate, its Hooks type path and an optional shaders registry path');
  }
  const folder = resolve(dir, 'render'), manifest = resolve(folder, 'Cargo.toml');
  if (!existsSync(manifest)) throw new Error(`game.render.crate ${render.crate} requires ${manifest}`);
  const declared = Bun.TOML.parse(readFileSync(manifest, 'utf8')).package;
  if (declared?.name !== render.crate) throw new Error(`game.render.crate ${render.crate} must name the package in ${folder}`);
  if (declared.workspace !== '../.shells') throw new Error(`${manifest}: set package.workspace = "../.shells" so the app owns its render hooks`);
  return folder;
}

/** Check actual unfiltered Cargo edges: aliases and target/build dependencies
 * must not smuggle the GPU render crate into simulation or host metadata. */
export function renderGraph(metadata, game) {
  if (!game.render) return;
  const packages = new Map(metadata.packages.map(pkg => [pkg.id, pkg]));
  const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
  const members = new Set(metadata.workspace_members);
  const find = name => metadata.packages.find(pkg => pkg.name === name && members.has(pkg.id))?.id;
  const forbidden = find(game.render.crate), name = game.crate.slice(0, -'-logic'.length);
  if (!forbidden) throw new Error(`game.render package ${game.render.crate} is absent from Cargo metadata`);
  const roots = [game.crate, game.data?.crate, ...['web','apple','linux','windows'].map(kind => `${name}-${kind}`)].filter(Boolean).map(find);
  const gpu = nodes.get(find(`${name}-gpu`));
  if (!gpu) throw new Error('game.render isolation requires complete Cargo GPU metadata');
  for (const dependency of gpu.deps) if (dependency.dep_kinds.some(kind => kind.kind === 'build')) roots.push(dependency.pkg);
  for (const root of roots) {
    if (!root) throw new Error('game.render isolation requires complete Cargo metadata');
    const pending = [[root]], seen = new Set();
    while (pending.length) {
      const path = pending.pop(), id = path.at(-1);
      if (id === forbidden) throw new Error(`game.render must remain GPU-only: ${path.map(id => packages.get(id)?.name ?? id).join(' -> ')}`);
      if (seen.has(id)) continue;
      seen.add(id);
      const node = nodes.get(id);
      if (!node) throw new Error(`game.render isolation requires complete Cargo metadata for ${id}`);
      for (const dep of node.deps) if (dep.dep_kinds.some(kind => kind.kind !== 'dev')) pending.push([...path, dep.pkg]);
    }
  }
}

export function gameShells(dir, game, workspace) {
  // Relative paths are written from the real directory: /tmp is a symlink on macOS.
  dir = realpathSync(dir);
  // Only this app is materialized. Each bake owns a generated Cargo workspace.
  const app = {game, ...gameDefaults(dir)};
  const {crate, type, data, render} = app.game, name = crate.slice(0, -'-logic'.length);
  // Reject authored declaration errors before touching even an old generated workspace.
  if (app.game.presentation !== undefined) {
    throw new Error('game.presentation is now game.render: rename the key, move presentation/ to render/, name the crate <name>-render and rename its "type" to "hooks" (LLP 1046.008, amendment of 2026-10-04)');
  }
  const renderDir = renderDeclaration(dir, render);
  const root = resolve(dir, '.shells');
  const source = existsSync(resolve(workspace, 'Cargo.toml')) ? workspace : gameRoot;
  const cargo = Bun.TOML.parse(readFileSync(resolve(source, 'Cargo.toml'), 'utf8'));
  if (render) {
    // Reflection emits ::exact_gpu paths. Expose the existing SDK crates to
    // an authored render crate without changing a hookless workspace declaration.
    cargo.workspace.dependencies['exact-gpu'] ??= {path:relative(source, resolve(gameRoot, '../gpu'))};
    cargo.workspace.dependencies['exact-gpu-reflect'] ??= {path:relative(source, resolve(gameRoot, '../gpu/reflect'))};
  }
  const authoredLogic = existsSync(resolve(dir, 'logic/Cargo.toml'));
  cargo.workspace.members = ['gpu','web','apple','linux','windows', authoredLogic ? '../logic' : 'logic', ...(data ? ['../data'] : []), ...(render ? ['../render'] : [])];
  delete cargo.workspace.exclude;
  // Engine crates are dependencies here: the wildcard already optimizes them.
  // Retain member overrides and any settings distinct from that wildcard.
  const members = shellMembers(app.game, name);
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
  const child = relative(realpathSync(gameRoot), realpathSync(root));
  const inherited = child === '' || (!isAbsolute(child) && child !== '..' && !child.startsWith('..' + sep));
  let config = (inherited ? '[build]\n' : readFileSync(resolve(gameRoot,'.cargo/config.toml'),'utf8'))
    .replace('[build]', `[build]\nbuild-dir = ${JSON.stringify(resolve(dir,'target'))}`);
  // Clippy reads the determinism lints from here, for authored and generated logic alike.
  const lints = [resolve(source, 'app/determinism'), resolve(gameRoot, 'app/determinism')].find(path => existsSync(resolve(path, 'clippy.toml')));
  const clippy = lints ? `CLIPPY_CONF_DIR = ${JSON.stringify(lints)}\n` : '';
  // The render crate reflects the shader inventory exactly as the bake ships it
  // (each shader after its gpu.shaderPreludes): one join, written here.
  const shaders = render ? `EXACT_GAME_SHADERS = ${JSON.stringify(resolve(root, 'shaders'))}\n` : '';
  config = config.includes('[env]\n') ? config.replace('[env]\n', `[env]\n${clippy}${shaders}`) : `${config}\n[env]\n${clippy}${shaders}`;
  writeChanged(resolve(root,'.cargo/config.toml'), config, false);
  if (render) {
    const inventory = shaderFiles({dir, manifest: app}), folder = resolve(root, 'shaders');
    mkdirSync(folder, {recursive:true});
    for (const name of readdirSync(folder)) if (!inventory.has(name)) rmSync(resolve(folder, name));
    for (const [name, bytes] of inventory) writeChanged(resolve(folder, name), bytes.toString('utf8'), false);
  }
  // An external game cannot inherit the SDK's compiler through its ancestors.
  writeChanged(resolve(root,'rust-toolchain.toml'), readFileSync(resolve(gameRoot,'../rust-toolchain.toml'),'utf8'), false);
  writeChanged(resolve(root, 'app.json'), JSON.stringify(app, null, 2) + '\n', false);
  writeChanged(resolve(root,'Cargo.toml'), '# Generated by the game bake.\n' + Object.entries(cargo)
    .map(([key,value])=>`[${key}]\n${Object.entries(value).map(([key,value])=>`${JSON.stringify(key)} = ${toml(value)}\n`).join('')}`).join('\n'), false);
  // A game's own lock is captured only when it adds dependencies; otherwise
  // the SDK lock seeds the shell and prepareGame derives the game's subset.
  const lock = resolve(dir, 'Cargo.lock'), shellLock = resolve(root, 'Cargo.lock');
  if (existsSync(lock)) writeChanged(shellLock, readFileSync(lock, 'utf8'), false);
  else if (!existsSync(shellLock) && sdkLockFile(source)) writeFileSync(shellLock, readFileSync(sdkLockFile(source), 'utf8'));
  const appDir = dir;
  if (!existsSync(resolve(appDir, 'logic/src/lib.rs'))) throw new Error(`${appDir}/logic/src/lib.rs: a game's logic is required`);
  const logicDir = authoredLogic ? resolve(appDir, 'logic') : resolve(root, 'logic');
  const manifest = resolve(logicDir, 'Cargo.toml');
  if (authoredLogic) {
    const declared = Bun.TOML.parse(readFileSync(manifest, 'utf8')).package;
    if (declared?.name !== crate) throw new Error(`game.crate ${crate} must name the package in ${appDir}/logic`);
    if (declared.workspace !== '../.shells') throw new Error(`${manifest}: set package.workspace = "../.shells" so the app owns its logic`);
  } else {
    mkdirSync(logicDir, {recursive:true});
    writeChanged(manifest, logicManifest(appDir, crate), false);
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
  for (const kind of ['gpu', 'web', 'apple', 'linux', 'windows']) {
    const shell = resolve(root, kind);
    const levelBake = relative(shell, resolve(source, 'bake/src/files.rs'));
    // These adapters contain entry points only; tests live in authored crates.
    const target = ['linux', 'windows'].includes(kind)
      ? `[[bin]]\nname = "${name}-${kind}"\npath = "src/main.rs"\ntest = false`
      : `[lib]\ncrate-type = ["${kind === 'apple' ? 'staticlib' : 'cdylib'}"]\ntest = false\ndoctest = false`;
    const header = `[package]\nname = "${name}-${kind}"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false\n\n${target}\n\n[dependencies]\n`;
    const dataDependency = data ? `exact-data-host.workspace = true\napp-data = { package = "${data.crate}", path = ${JSON.stringify(relative(shell, dataDir))} }\n` : '';
    const dataBuildDependency = data ? `app-data = { package = "${data.crate}", path = ${JSON.stringify(relative(shell, dataDir))} }\n` : '';
    const renderDependency = render ? `game-render = { package = "${render.crate}", path = ${JSON.stringify(relative(shell, renderDir))} }\n` : '';
    const dependencies = kind === 'gpu'
      ? `exact-game-render.workspace = true\n${app.game.audio === true ? "exact-game-audio.workspace = true\n" : ""}${renderDependency}game-logic = { package = "${crate}", path = ${JSON.stringify(relative(shell, logicDir))} }\n\n[target.'cfg(target_arch = "wasm32")'.dependencies]\nwasm-bindgen.workspace = true\nwasm-bindgen-futures.workspace = true\nweb-sys.workspace = true\n\n[build-dependencies]\nexact-game.workspace = true\nserde_json = "1"\ngame-logic = { package = "${crate}", path = ${JSON.stringify(relative(shell, logicDir))} }\n${bakeArt ? 'exact-game-bake.workspace = true\n' : ''}`
      : `exact-runner.workspace = true\nexact-${kind}.workspace = true\n${kind === 'web' ? 'exact-web-capabilities.workspace = true\n' : ''}${dataDependency}\n[build-dependencies]\nexact-game-app.workspace = true\n${dataBuildDependency}`;
    const files = {
      'Cargo.toml': header + dependencies,
      [['linux', 'windows'].includes(kind) ? 'src/main.rs' : 'src/lib.rs']: kind === 'gpu' ? `exact_game_render::module!(game_logic::${type}${app.game.audio === true ? ", audio" : ""}${app.game.assets === true ? ", assets" : ""}${render ? `, hooks = game_render::${render.hooks}${render.shaders ? `, shaders = game_render::${render.shaders}` : ''}` : ''});\n` : (kind === 'windows' ? '#![cfg_attr(\n    all(target_os = "windows", not(debug_assertions)),\n    windows_subsystem = "windows"\n)]\n' : '') + 'include!(concat!(env!("OUT_DIR"), "/entry.rs"));\n',
      'build.rs': kind === 'gpu'
        ? `use exact_game::{Args, Game, Value};
use std::{env, fs, path::PathBuf};
#[path = ${JSON.stringify(levelBake)}]
mod bake_files;
fn level_bake_path() -> &'static str {
    ${JSON.stringify(levelBake)}
}
fn main() {
    // A short name, so no line's width (rustfmt's 100) depends on the game's.
    type Logic = game_logic::${type};
    type Options = <Logic as Game>::Args;
    let arguments: Vec<_> = Options::FIELDS
        .iter()
        .zip(Options::default().values())
        .map(|((name, _), value)| {
            let value = match value {
                Value::Number(n) => serde_json::json!(n),
                Value::Bool(b) => serde_json::json!(b),
                s if s.is_str() => serde_json::json!(s.text()),
                _ => panic!("unsupported surface argument default: {name}"),
            };
            serde_json::json!({"name": name, "default": value})
        })
        .collect();
    let name = <Logic as Game>::NAME;
    let text = serde_json::to_string_pretty(&serde_json::json!({name: arguments})).unwrap() + "\\n";
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../surfaces.json");
    if fs::read_to_string(&path).ok().as_deref() != Some(&text) {
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&temporary, text).expect("write game surface declaration");
        fs::rename(temporary, &path).expect("publish complete game surface declaration");
    }
${bakeArt ? `    exact_game_bake::bake_art(${JSON.stringify(relative(shell, appDir))}).expect("bake art");\n` : ''}    bake_files::bake_game_level::<Logic>(${JSON.stringify(relative(shell, appDir))}).expect("bake level");
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

// One SDK lock (@ref llp/1046.003-game-engine-as-built.explainer.md#one-sdk-lock-2026-09-23):
// every generated game workspace whose code adds no packages resolves against
// game/app/shells.lock, the lock of the union of every shell's dependencies.
// A game that adds packages captures its own Cargo.lock with --update-lock.
const sdkLockFile = source => [resolve(source, 'app/shells.lock'), resolve(gameRoot, 'app/shells.lock')].find(path => existsSync(path));
// Root crates reach every shell by path, so a root commit that adds a registry
// dependency to one (15856ff7: cssparser in kernel) moves every shell's graph
// while updating only the root Cargo.lock. Those packages are as decided as the
// SDK lock's: the root lock's registry packages whose names the SDK lock lacks
// seed a game's resolution and are admitted by version and checksum, so a new
// game resolves offline before anyone refreshes the SDK lock. A new root crate
// (00d37ef9f: exact-svg-filter under the kernel) is one too: the root lock's
// path packages, which carry no source or checksum, seed it the same way.
export function withRootPins(sdk, root = existsSync(resolve(gameRoot, '../Cargo.lock')) ? readFileSync(resolve(gameRoot, '../Cargo.lock'), 'utf8') : '') {
  // By name and semver series (Cargo's compatibility: 1.x, 0.37.x, 0.0.3): a root's
  // new major of a package the SDK lock holds is a pin too; a compatible one is not.
  const series = version => { const [major, minor, patch] = String(version).split('.'); return major !== '0' ? major : minor !== '0' ? `0.${minor}` : `0.0.${patch}`; };
  const held = new Set((Bun.TOML.parse(sdk).package ?? []).map(pkg => `${pkg.name} ${series(pkg.version)}`));
  const id = block => `${/^name = "([^"]+)"/m.exec(block)?.[1]} ${series(/^version = "([^"]+)"/m.exec(block)?.[1])}`;
  const pins = root.split(/\n(?=\[\[package\]\]\n)/).slice(1).map(block => block.trimEnd())
    .filter(block => (/^source = "registry\+/m.test(block) || !/^source = /m.test(block)) && !held.has(id(block)));
  return pins.length ? `${sdk.trimEnd()}\n\n${pins.join('\n\n')}\n` : sdk;
}
const cargoMetadata = (cwd, flags, env) => spawnSync('cargo', ['metadata', ...flags, '--format-version', '1'], {cwd, env, encoding:'utf8', maxBuffer:64 * 1024 * 1024});
// A lock's packages by identity. Dependency edges follow from the versions
// and the activated features, so a game's subset keeps every version but may
// drop edges; the versions and checksums are what the SDK lock decides.
const lockIds = text => new Map((Bun.TOML.parse(text).package ?? []).map(pkg => [`${pkg.name} ${pkg.version} ${pkg.source ?? ''}`.trim(), pkg.checksum ?? null]));
/** Packages of `derived` (members excepted) whose version the SDK lock does not hold.
 * A root path crate new since the SDK lock is held through `withRootPins`. */
export function outsideSdkLock(derived, sdk, members) {
  const known = lockIds(sdk);
  return [...lockIds(derived)].filter(([id, checksum]) => !members.has(id.split(' ')[0]) && (!known.has(id) || known.get(id) !== checksum)).map(([id]) => id);
}
// Every gpu-dev bake re-resolves the same graph. Reuse the last locked
// metadata while every manifest, lock and config it read is unchanged, keyed by
// stat as the proof's inputs are; production trust always asks Cargo.
const statKey = path => { try { const s = statSync(path); return `${s.ino}:${s.size}:${s.mtimeMs}`; } catch { return null; } };
function lockedMetadata(root, flags, env) {
  const cache = resolve(root, 'metadata.json'), fixed = ['Cargo.toml', 'Cargo.lock', '.cargo/config.toml', 'rust-toolchain.toml'].map(file => resolve(root, file));
  const key = metadata => JSON.stringify([flags, ...['CARGO_TARGET_DIR', 'CARGO_BUILD_BUILD_DIR', 'CARGO_HOME', 'RUSTUP_TOOLCHAIN', 'PATH'].map(name => env[name] ?? null),
    ...[...fixed, resolve(gameRoot, '.cargo/config.toml'), resolve(gameRoot, '../rust-toolchain.toml'),
      // A package directory's own stat changes when a target file (build.rs, src/bin) is added.
      ...metadata.packages.filter(pkg => !pkg.source).flatMap(pkg => [pkg.manifest_path, dirname(pkg.manifest_path), resolve(dirname(pkg.manifest_path), 'src')])]
      .map(path => [path, statKey(path)])]);
  if (env.EXACT_UPDATE_TRUST !== 'production') try {
    const stored = JSON.parse(readFileSync(cache, 'utf8'));
    if (stored.key === key(JSON.parse(stored.text))) return {status:0, stdout:stored.text};
  } catch { /* No reusable resolution. */ }
  const result = cargoMetadata(root, flags, env);
  if (result.status === 0) writeFileSync(cache, JSON.stringify({key:key(JSON.parse(result.stdout)), text:result.stdout}));
  return result;
}
const shellMembers = (game, name) => new Set([game.crate, game.data?.crate, game.render?.crate, ...['gpu','web','apple','linux','windows'].map(kind => `${name}-${kind}`)].filter(Boolean));

// Every ordinary bake is locked; dependency edits require an explicit update,
// never a publisher's cache choice.
export function prepareGame(dir, game, source = gameRoot, {updateLock = false, target, env = process.env} = {}) {
  // Match the declaration gameShells resolves now, even if a dev caller retained
  // an app object from before app.json gained game.render.
  game = gameDefaults(dir)?.game ?? game;
  const root = resolve(dir, '.shells'), own = resolve(dir, 'Cargo.lock'), shell = resolve(root, 'Cargo.lock');
  const name = gameShells(dir, game, source), members = shellMembers(game, name);
  const metadata = locked => (locked ? lockedMetadata : cargoMetadata)(root, ['--offline', ...(locked ? ['--locked'] : []), ...(target ? ['--filter-platform', target] : [])], env);
  const refused = result => new Error(`game Cargo graph: ${result.stderr || result.error?.message || ''}${result.status === null ? ` (cargo metadata ended by ${result.signal})` : ''}\nOffline resolution requires a populated Cargo cache: cargo fetch --manifest-path ${JSON.stringify(resolve(root, 'Cargo.toml'))}\nTo capture this game's own dependencies: bun game/app/shells.mjs ${JSON.stringify(dir)} --update-lock`);
  const checked = result => {
    const graph = JSON.parse(result.stdout);
    if (game.render) {
      // A platform-filtered graph omits inactive target edges. Check every
      // production edge before accepting any build or publishing an own lock.
      const all = target ? lockedMetadata(root, ['--offline', '--locked'], env) : result;
      if (all.status !== 0) throw refused(all);
      renderGraph(JSON.parse(all.stdout), game);
    }
    return graph;
  };
  if (updateLock || existsSync(own)) {
    const result = metadata(!updateLock);
    if (result.status !== 0) throw refused(result);
    const graph = checked(result);
    if (updateLock) writeChanged(own, readFileSync(shell, 'utf8'), existsSync(own));
    return graph;
  }
  const lockFile = sdkLockFile(source);
  if (!lockFile) throw new Error(`${own}: no captured lock and no SDK lock (game/app/shells.lock)`);
  const sdk = withRootPins(readFileSync(lockFile, 'utf8'));
  // The shell's derived lock stands while it is the SDK lock's subset and exact.
  const derived = existsSync(shell) ? readFileSync(shell, 'utf8') : sdk;
  let result = derived !== sdk && !outsideSdkLock(derived, sdk, members).length ? metadata(true) : null;
  if (result?.status !== 0) {
    writeFileSync(shell, sdk);
    result = metadata(false);
    const outside = result.status === 0 ? outsideSdkLock(readFileSync(shell, 'utf8'), sdk, members) : [];
    if (result.status !== 0 || outside.length) writeFileSync(shell, sdk);
    if (result.status !== 0) throw refused(result);
    if (outside.length) throw new Error(`${dir}: resolves packages outside the SDK lock ${lockFile}: ${outside.slice(0, 8).join(', ')}${outside.length > 8 ? ', …' : ''}.
A game that adds dependencies captures its own lock: bun game/app/shells.mjs ${JSON.stringify(dir)} --update-lock
A changed SDK refreshes the SDK lock: bun game/app/shells.mjs --update-lock
A partial offline Cargo cache: cargo fetch --manifest-path ${JSON.stringify(resolve(root, 'Cargo.toml'))}`);
  }
  return checked(result);
}

/** The determinism lints (app/determinism/clippy.toml) on the game's logic
 * library; tests may time themselves. @ref llp/1046.003-game-engine-as-built.explainer.md#determinism-lints-2026-09-23 */
export function lintGame(dir, game, {env = process.env} = {}) {
  const result = spawnSync('cargo', ['clippy', '-p', game.crate, '--lib', '--locked', '--offline', '--quiet', '--',
    '-A', 'clippy::all', '-D', 'clippy::disallowed_methods', '-D', 'clippy::disallowed_types'], {cwd:resolve(dir, '.shells'), env, encoding:'utf8', maxBuffer:64 * 1024 * 1024});
  if (result.error || result.status !== 0) throw new Error(`determinism lints refused ${game.crate} (game/README.md, "Determinism — the contract"):\n${result.stderr || result.error?.message}`);
  if (game.render) {
    const result = spawnSync('cargo', ['clippy', '-p', game.render.crate, '--all-targets', '--locked', '--offline', '--quiet', '--',
      '-D', 'warnings', '-A', 'clippy::disallowed_methods', '-A', 'clippy::disallowed_types'], {cwd:resolve(dir, '.shells'), env, encoding:'utf8', maxBuffer:64 * 1024 * 1024});
    if (result.error || result.status !== 0) throw new Error(`render lints refused ${game.render.crate}:\n${result.stderr || result.error?.message}`);
  }
}

/** A throwaway workspace of the union of every generated shell's dependencies,
 * locked by `lock`; `use` runs in it and the stage is removed afterwards. */
function sdkStage(source, lock, use) {
  const stage = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-game-lock-')));
  try {
    const cargo = Bun.TOML.parse(readFileSync(resolve(source, 'Cargo.toml'), 'utf8'));
    for (const deps of [cargo.workspace.dependencies, ...Object.values(cargo.patch ?? {})])
      for (const dep of Object.values(deps ?? {})) if (dep.path) dep.path = relative(stage, resolve(source, dep.path));
    const toml = value => value && typeof value === 'object' && !Array.isArray(value)
      ? `{ ${Object.entries(value).map(([key, item]) => `${JSON.stringify(key)} = ${toml(item)}`).join(', ')} }`
      : Array.isArray(value) ? `[${value.map(toml).join(', ')}]` : JSON.stringify(value);
    const tables = {workspace:{members:['union'], resolver:cargo.workspace.resolver ?? '2'}, 'workspace.package':cargo.workspace.package,
      'workspace.dependencies':cargo.workspace.dependencies, ...Object.fromEntries(Object.entries(cargo.patch ?? {}).map(([key, value]) => [`patch.${JSON.stringify(key)}`, value]))};
    writeFileSync(resolve(stage, 'Cargo.toml'), Object.entries(tables).map(([key, value]) => `[${key}]\n${Object.entries(value).map(([k, v]) => `${JSON.stringify(k)} = ${toml(v)}\n`).join('')}`).join('\n'));
    mkdirSync(resolve(stage, 'union'));
    writeFileSync(resolve(stage, 'union/lib.rs'), '');
    writeFileSync(resolve(stage, 'union/Cargo.toml'), `[package]\nname = "exact-game-shells"\nversion = "0.1.0"\nedition = "2021"\npublish = false\n\n[lib]\npath = "lib.rs"\n\n[dependencies]\n${Object.keys(cargo.workspace.dependencies).map(dep => `${dep}.workspace = true\n`).join('')}${cargo.workspace.dependencies.serde_json ? '' : 'serde_json = "1"\n'}`);
    if (lock !== null) writeFileSync(resolve(stage, 'Cargo.lock'), lock);
    return use(stage);
  } finally { rmSync(stage, {recursive:true, force:true}); }
}

/** Check the SDK lock against the union of every generated shell's
 * dependencies, or (`update`) rewrite it from that union. */
export function sdkLock(source = gameRoot, {update = false, env = process.env} = {}) {
  const path = resolve(source, 'app/shells.lock');
  // A refresh takes the root lock's versions for packages new to the SDK.
  const lock = existsSync(path) ? (update ? withRootPins(readFileSync(path, 'utf8')) : readFileSync(path, 'utf8')) : null;
  sdkStage(source, lock, stage => {
    const result = cargoMetadata(stage, update ? [] : ['--locked', '--offline'], env);
    if (result.status !== 0) throw new Error(`${update ? 'SDK lock update' : `${path} is stale for the SDK's shell dependencies; refresh it: bun game/app/shells.mjs --update-lock`}\n${result.stderr || result.error?.message}`);
    if (update) writeChanged(path, readFileSync(resolve(stage, 'Cargo.lock'), 'utf8'), false);
  });
}

/** Put the SDK lock's crates in Cargo's cache, as a game's offline bake reads
 * them (`exact setup`), or (`offline`) only say whether they are all there
 * (`setup --check`): the platformer's first build stopped on \`glam\` (its diary, R2).
 * The lock is the one a new game starts from, the root's pins included. */
export function sdkFetch({offline = false, source = gameRoot, env = process.env} = {}) {
  const path = sdkLockFile(source);
  if (!path) return {ok:false, message:`no SDK lock at ${resolve(source, 'app/shells.lock')}`};
  return sdkStage(source, withRootPins(readFileSync(path, 'utf8')), stage => {
    const result = spawnSync('cargo', ['fetch', ...(offline ? ['--offline'] : [])], {cwd:stage, env, encoding:'utf8', stdio:offline ? 'pipe' : ['ignore', 'inherit', 'inherit']});
    return {ok:result.status === 0, message:result.stderr?.trim() || result.error?.message || ''};
  });
}

if (import.meta.main) {
  const args = process.argv.slice(2), requested = args.find(arg => !arg.startsWith('--'));
  if (args.includes('--test')) {
    let failed=false;
    const dirs = requested ? [resolve(requested)] : ['games','bench'].flatMap(group =>
      readdirSync(resolve(gameRoot, group)).map(name => resolve(gameRoot, group, name)))
      .filter(dir => existsSync(resolve(dir, 'logic/src/lib.rs')));
    if (!requested) try { sdkLock(); } catch (error) { console.error(error.message); failed = true; }
    for (const dir of dirs) {
      try {
        const game = gameDefaults(dir).game;
        prepareGame(dir, game);
        const env = {...process.env, CARGO_TARGET_DIR:process.env.CARGO_TARGET_DIR ?? resolve(gameRoot,'target')};
        if (hasArt(dir)) {
          const result=spawnSync('cargo',['run','--manifest-path',resolve(gameRoot,'Cargo.toml'),'-p','exact-game-bake','--locked','--offline','--','--art',dir], {
            cwd:gameRoot,env,stdio:'inherit',
          });
          if (result.error || result.status !== 0) throw new Error(`game art bake: ${result.error?.message ?? `cargo exited ${result.status ?? result.signal}`}`);
        }
        try { lintGame(dir, game, {env}); } catch (error) { console.error(error.message); failed = true; }
        // The author's crates only: generated adapters are products, built by bakes.
        const result=spawnSync('cargo',['test',...[game.crate, game.data?.crate, game.render?.crate].filter(Boolean).flatMap(crate => ['-p', crate]),'--locked','--offline','--no-fail-fast'], {
          cwd:resolve(dir,'.shells'),env,stdio:'inherit',
        });
        failed ||= result.status !== 0;
      } catch(error) {console.error(error);failed=true;}
    }
    process.exitCode=failed?1:0;
  } else if (!requested && args.includes('--update-lock')) sdkLock(gameRoot, {update:true});
  else {
    const dir = resolve(requested);
    const game = gameDefaults(dir).game;
    if (args.includes('--update-lock')) prepareGame(dir, game, gameRoot, {updateLock:true});
    else gameShells(dir, game, gameRoot);
  }
}
