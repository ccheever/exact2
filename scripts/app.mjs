import { compilerPaths, unitDepInfo, cargoBakeInputs, sourcePath } from './cargo-dep-info.mjs';
export { compilerPaths, unitDepInfo } from './cargo-dep-info.mjs';
import { LIBRARY_CACHE, directDependencies, prepareLibrary } from './package-libraries.mjs';
import { ensureBun } from './bun.mjs';
import { macResourceMappings, macResourceInventory, appleAssetCatalogInventory, launchImageInventory } from '../host/apple/assets.mjs';
// Where an app lives. Inside this repo an app is `apps/<name>` — its crates
// normally belong to the root workspace and build into `target/`; an explicit
// package.workspace uses that workspace's lock and target. Outside it (weird-castle:
// its own repo or a member of a surrounding workspace, depending on our crates by
// path so the two iterate together), `EXACT_APP_DIR` names the directory and
// everything else follows from it: the workspace cargo runs in, the target
// directory the artifacts land in, `app.contract`, `assets/`, `gpu/`. Every
// script that builds, serves, or drives an app resolves it here, so nothing
// else knows the difference.
//   bun host/web/build.mjs weird-castle-web          (EXACT_APP_DIR set)
//   bun host/apple/build.mjs --ios weird-castle-apple --run
//   bun host/web/dev.mjs --app weird-castle
//   bun scripts/agent.mjs --app weird-castle macos tree
//
// The app manifest (LLP 1030 D2; 1030.000 D7): `app.json` beside
// `app.contract` — the W3C Web App Manifest's own keys, which the web host
// copies out as `manifest.json`, plus `app` (identity: the id every platform
// derives its bundle id from, the name, the origin), `host.<platform>` (what
// bake generates each platform's host files from), and `deploy` (policy,
// never identity). Validated here against `scripts/app.schema.json` with a
// validator small enough to live beside the reader; an app without one gets
// the derived defaults it had before the manifest existed.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, symlinkSync, writeFileSync } from 'node:fs';
import { basename, delimiter, dirname, isAbsolute, relative, resolve, sep, toNamespacedPath } from 'node:path';
import { homedir, tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { BINARYEN } from '../host/web/stages.mjs';
import { SHELL_SOURCES } from '../host/web/look.mjs';
import { createHash } from 'node:crypto';
import { nativeSource, prepareRustBundle } from './rust.mjs';
import { bakeOutside, linkable, recordRustBake } from './linked-plan.mjs';
import { throwContractErrors } from './contract-diagnosis.mjs';
import { filesystem, installedModules, removePrivateTree, withHostDeployment } from './filesystem.mjs';
import { startSweep } from './sweep.mjs';
import { authoredWorkspace, knownWorkspace, rememberWorkspace, shellsDir } from './cargo-check.mjs';
import { ensureShells } from './app-shells.mjs';
import { installProblems } from './install-page.mjs';
import { gameDefaults, lintGame, prepareGame } from '../game/app/shells.mjs';
import { shareTargetProblems, webShareTarget } from './share-target.mjs';
import { bakedSettings, noteBunLoaded, settingChanges } from '../js/bake/src/__exact_env.mjs';
noteBunLoaded(); // before this script starts a process: the settings Bun loaded from `.env` files (LLP 1153 D4)
/** rustup puts cargo in `~/.cargo/bin` and a login profile puts that on PATH;
 * a non-interactive shell (an agent's, a launchd job's) often skips the
 * profile, and every cargo spawn below then fails as ENOENT. Every script that
 * builds resolves its app here, so this is the one place to add it back.
 * Children inherit the repaired PATH. */
export function cargoOnPath(env = process.env, home = homedir()) {
  const dirs = (env.PATH ?? '').split(delimiter).filter(Boolean);
  if (dirs.some(dir => existsSync(resolve(dir, 'cargo')))) return true;
  const rustup = resolve(env.CARGO_HOME ?? resolve(home, '.cargo'), 'bin');
  if (!existsSync(resolve(rustup, 'cargo'))) return false;
  env.PATH = [rustup, ...dirs].join(delimiter);
  return true;
}
cargoOnPath(); process.env.HERMES_LEAN_SYS_OFFLINE = '1';
/** Every Cargo process Exact starts refuses implicit Hermes acquisition. */
export function cargoEnvironment(env = process.env) { return withHostDeployment({ ...env, HERMES_LEAN_SYS_OFFLINE: '1' }); }
// @ref llp/1046.006.000-render-hooks.rfc.md#d5-shaders-that-live-with-the-game
/** Explicit source roots, relative to app.json. Only packaged names reach a host. */
export function shaderRoots(app) {
  return [...(app.manifest.game ? [] : ['gpu/shaders']), ...(shaderConfig(app).shaderRoots ?? [])]
    .map(path => shaderPath(app,path));
}
function shaderConfig(app) {
  const gpu = app.manifest.gpu;
  if (gpu !== undefined) {
    const problems = validate(gpu, schema().properties.gpu, 'gpu', schema());
    if (problems.length) throw new Error(problems.join('\n'));
  }
  return gpu ?? {};
}
function shaderPath(app,path) {
  if (isAbsolute(path)) throw new Error('shader paths must be relative to app.json so source snapshots remain relocatable');
  return resolve(app.dir,path);
}
/** The game renderer's WGSL a shader may name as a prelude, by the constant
 * `exact_game_render::hooks` exports for the same files (FRAME_WGSL,
 * MATERIAL_WGSL, MATERIAL_SHADOWS_WGSL), so a game outside this repository
 * never names the SDK's source paths. */
export const ENGINE_PRELUDES = {
  'exact-game-render:frame': ['frame.wgsl'],
  'exact-game-render:material': ['frame.wgsl', 'transform.wgsl', 'custom_instance.wgsl'],
  'exact-game-render:material_shadows': ['caustics.wgsl', 'shadow_sample.wgsl', 'fade.wgsl', 'lights.wgsl', 'material_shadows.wgsl'],
};
/** One declared prelude entry's files: an engine set, or a path relative to app.json. */
function preludePaths(app, path) {
  if (path.startsWith('exact-game-render:')) {
    const files = ENGINE_PRELUDES[path];
    if (!files) throw new Error(`unknown engine prelude ${path}; one of ${Object.keys(ENGINE_PRELUDES).join(', ')}`);
    return files.map(file => resolve(ROOT, 'game/render/src/shaders', file));
  }
  return [shaderPath(app, path)];
}
/** Shared WGSL libraries prepended to a named shader, in declared order. */
export function shaderPreludeFiles(app) {
  return [...new Set(Object.values(shaderConfig(app).shaderPreludes ?? {}).flat().flatMap(path => preludePaths(app, path)))];
}
/** Directory watches also cover edits to shared prelude files. */
export function shaderWatchRoots(app) {
  return [...new Set([...shaderRoots(app), ...shaderPreludeFiles(app).map(dirname)])];
}
/** Merge flat shader packs through the same no-symlink gate as other assets. */
export function shaderFiles(app) {
  const files = new Map();
  for (const root of shaderRoots(app)) {
    const tree = filesystem({op:'tree', root, optionalRoot: root === resolve(app.dir,'gpu/shaders')});
    for (const [name, base64] of Object.entries(tree ?? {})) {
      if (!/^[A-Za-z_][A-Za-z0-9_]*\.wgsl$/.test(name)) throw new Error(`shader pack ${root}: expected a flat WGSL filename, got ${name}`);
      if (files.has(name)) throw new Error(`duplicate shader ${name} across declared roots`);
      files.set(name, Buffer.from(base64, 'base64'));
    }
  }
  for (const [stem, paths] of Object.entries(shaderConfig(app).shaderPreludes ?? {})) {
    const name = `${stem}.wgsl`;
    if (!files.has(name)) throw new Error(`shader prelude names missing shader ${name}`);
    const parts = paths.flatMap(path => preludePaths(app, path)).map(full =>
      Buffer.from(filesystem({op:'get',root:dirname(full),path:basename(full)}),'base64'));
    files.set(name, Buffer.concat([...parts.flatMap(bytes => [bytes,Buffer.from('\n')]),files.get(name)]));
  }
  return files;
}
/** Package the complete validated inventory into a private build stage. */
export function copyShaders(app, target, {replace=false} = {}) {
  const files = shaderFiles(app);
  if (replace) rmSync(target, {recursive:true, force:true});
  if (files.size) mkdirSync(target, {recursive:true});
  for (const [name, bytes] of files) writeFileSync(resolve(target,name), bytes);
}
// @ref LLP 1009 D6 — one artifact per module, loaded by the surfaces it owns.
/** The GPU artifacts beside the primary `<app>-gpu`: module `m` is the crate
 * `<app>-gpu-m` and owns exactly the surface names listed for it. */
export function gpuModules(manifest) {
  return Object.entries(manifest.gpu?.modules ?? {}).map(([name, surfaces]) => ({ name, surfaces }));
}
function gpuModuleProblems(manifest) {
  const problems = [], owner = new Map();
  if (manifest.game && manifest.gpu?.modules) problems.push('gpu.modules: a game generates its one GPU module; modules name crates in an app workspace');
  for (const { name, surfaces } of gpuModules(manifest)) {
    if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name)) problems.push(`gpu.modules.${name}: a module name is lowercase letters and digits, joined by single hyphens`);
    if (!surfaces.length) problems.push(`gpu.modules.${name}: names no surface`);
    for (const surface of surfaces) {
      if (owner.has(surface)) problems.push(`gpu.modules: surface ${surface} is claimed by both ${owner.get(surface)} and ${name}`);
      owner.set(surface, name);
    }
  }
  return problems;
}

/** Existing locks are binding; the root workspace and generated game shells require theirs. */
export const cargoReproducibilityFlags = (app, workspace = app.workspace) =>
  (resolve(workspace) === ROOT || (app.manifest.game && resolve(workspace) === resolve(app.workspace)) || (existsSync(resolve(workspace, 'Cargo.toml')) && existsSync(resolve(workspace, 'Cargo.lock')))) ? ['--locked', '--offline'] : [];
/** The root workspace's `[profile.*]` tables, for a build of an app outside it
 * (LLP 1036.001 D1): passed as `--config` so nothing is copied and nothing can
 * drift. Cargo ranks config above a manifest, so an app's own copy is
 * overridden. A game's generated workspace carries its own. Empty for the root. */
export function injectedProfiles(app, workspace = app.workspace) {
  if (resolve(workspace) === ROOT || app.manifest?.game) return [];
  const profiles = Bun.TOML.parse(readFileSync(resolve(ROOT, 'Cargo.toml'), 'utf8')).profile ?? {};
  const value = v => typeof v === 'string' ? JSON.stringify(v) : String(v);
  const key = k => /^[A-Za-z0-9_-]+$/.test(k) ? k : JSON.stringify(k);
  const flags = [];
  const walk = (path, table) => {
    for (const [k, v] of Object.entries(table)) {
      if (v && typeof v === 'object' && !Array.isArray(v)) walk([...path, key(k)], v);
      else flags.push('--config', `${path.join('.')}.${key(k)}=${value(v)}`);
    }
  };
  for (const [name, table] of Object.entries(profiles)) walk(['profile', key(name)], table);
  return flags;
}

/** The root's crates.io patches as they must appear in an outside workspace
 * at `from`: each vendored crate, by a path relative to it. Two copies of taffy
 * or cosmic-text in one build fail far from the cause, so `exact new` writes
 * these and every run checks them (LLP 1036.001 D2). */
export function patchLines(from) {
  const root = Bun.TOML.parse(readFileSync(resolve(ROOT, 'Cargo.toml'), 'utf8'));
  return Object.entries(root.patch?.['crates-io'] ?? {}).map(([name, spec]) =>
    `${name} = { path = ${JSON.stringify(pathFrom(from, resolve(ROOT, spec.path)))} }`);
}
/** How an outside workspace at `from` names a path in this checkout: relative, with `/`
 * (Windows reads it; a Mac can't read Skirmish's `..\\exact2`), when the two share a directory
 * below the root (a checkout beside the app moves with it), absolute when they share nothing,
 * where a relative path would climb to / and break once either moved. `from` must exist;
 * both are resolved through symlinks (macOS's /tmp is one). */
export function pathFrom(from, to) {
  const [a, b] = [realpathSync(from), existsSync(to) ? realpathSync(to) : resolve(to)];
  const shared = a.split('/')[1] === b.split('/')[1] && a.split('/')[1] !== '';
  return shared ? (relative(a, b) || '.').split(sep).join('/') : b;
}

/** An outside workspace's patches and toolchain, against this checkout's.
 * Refuses with the exact text to paste rather than letting Cargo fail later
 * on a duplicate crate or a missing wasm target. */
export function outsideWorkspaceProblems(workspace) {
  const problems = [];
  const manifest = Bun.TOML.parse(readFileSync(resolve(workspace, 'Cargo.toml'), 'utf8'));
  const theirs = manifest.patch?.['crates-io'] ?? {};
  const root = Bun.TOML.parse(readFileSync(resolve(ROOT, 'Cargo.toml'), 'utf8')).patch?.['crates-io'] ?? {};
  const wrong = Object.entries(root).filter(([name, spec]) => !theirs[name]?.path || resolve(workspace, theirs[name].path) !== resolve(ROOT, spec.path));
  if (wrong.length) problems.push(`${workspace}/Cargo.toml: [patch.crates-io] must name exact2's vendored ${wrong.map(([name]) => name).join(', ')}. Use (or run \`bun exact.mjs update\` in an app \`exact new\` made):\n[patch.crates-io]\n${patchLines(workspace).join('\n')}`);
  // A nested workspace inherits the checkout's pinned toolchain from rustup.
  // An external workspace still needs its own pin.
  const localToolchain = resolve(workspace, 'rust-toolchain.toml');
  const toolchain = !existsSync(localToolchain) && under(ROOT, resolve(workspace))
    ? resolve(ROOT, 'rust-toolchain.toml') : localToolchain;
  const channel = existsSync(toolchain) ? /^channel\s*=\s*"([^"]+)"/m.exec(readFileSync(toolchain, 'utf8'))?.[1] : null;
  if (PINNED_RUST && channel !== PINNED_RUST) problems.push(`${toolchain}: ${channel ? `pins ${channel}` : 'is missing'}; exact2 builds with ${PINNED_RUST}. Copy ${resolve(ROOT, 'rust-toolchain.toml')} there (\`bun exact.mjs update\` does).`);
  return problems;
}

/** Resolve a binding lock, fetching its missing sources once without updating it. */
export function lockedMetadata(workspace, noDeps = false, env = process.env) {
  const args = ['metadata', '--locked', '--offline', ...(noDeps ? ['--no-deps'] : []), '--format-version', '1'];
  const options = { cwd: workspace, env: cargoEnvironment(env), encoding: 'utf8', maxBuffer: 1 << 26 };
  let result = spawnSync('cargo', args, options);
  if (result.status !== 0 && /--offline was specified|attempting to make an HTTP request|in the offline mode|offline mode \(via `--offline`\)/.test(result.stderr ?? '')) {
    console.error(`${workspace}: fetching missing locked Cargo sources (cargo fetch --locked)`);
    const fetched = spawnSync('cargo', ['fetch', '--locked'], options);
    if (fetched.status === 0) result = spawnSync('cargo', args, options);
    else result.stderr += `\ncargo fetch --locked:\n${fetched.stderr || fetched.error?.message}`;
  }
  return result;
}

/** Finding an outside app never changes its lock, except to resolve the copy an offline
 * `exact new` left (below). Dependency updates are explicit. */
function checkOutsideLock(workspace) {
  let result = lockedMetadata(workspace);
  // `exact new` offline, without exact2's crates in Cargo's cache, leaves the
  // copied lock for the first build to resolve (game/new.mjs resolveOffline).
  const lock = resolve(workspace, 'Cargo.lock'), root = resolve(ROOT, 'Cargo.lock');
  if (result.status !== 0 && existsSync(lock) && existsSync(root) && readFileSync(lock).equals(readFileSync(root))) {
    console.error(`${workspace}: resolving the Cargo.lock \`exact new\` copied from exact2 (cargo metadata)`);
    const options = { cwd: workspace, env: cargoEnvironment(), stdio: ['ignore', 'ignore', 'pipe'], encoding: 'utf8' };
    const offline = spawnSync('cargo', ['metadata', '--offline', '--format-version', '1'], options);
    const resolved = offline.status === 0 ? offline : spawnSync('cargo', ['metadata', '--format-version', '1'], options);
    if (resolved.status !== 0) throw new Error(`cargo metadata in ${workspace}:\n${resolved.stderr}`);
    result = lockedMetadata(workspace);
  }
  const fix = `\`bun exact.mjs lock\` in the app re-resolves it, changing only what the manifests changed (or \`cargo metadata --offline --format-version 1\` in ${workspace})`, stale = existsSync(lock) && (result.stderr ?? '').split('\n').find(line => /^error: .*(?:cannot update the lock file|needs to be updated).*--locked/.test(line));
  if (result.status !== 0) throw new Error(stale ? `${workspace}/Cargo.lock does not match its Cargo.toml files (a workspace member, dependency or patch changed since it was resolved), and a build never changes the lock: ${fix}.\n${stale}` : `cargo metadata --locked --offline in ${workspace}:\n${result.stderr || result.error?.message}\nTo update the lock explicitly: ${fix}.`);
  // The path packages' manifests, which the lock was checked against.
  return JSON.parse(result.stdout).packages.filter(pkg => pkg.source == null).map(pkg => pkg.manifest_path).sort();
}

export const hatchWords = (manifest, platform, h = manifest?.hatches ?? []) => (Array.isArray(h) ? [...h] : Object.keys(h).filter(w => h[w].includes(platform))).sort(); // the hatch words `platform`'s module handles (LLP 1075.003.000.001 §5; the bake's `hatches_on`): a plain list is every platform's
export const runnerOwnedSource = name => ['exactDelivery', 'exactViewport', 'exactSurface', 'exactActivities', 'startActivity', 'updateActivity', 'endActivity'].includes(name);

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** Source paths inside the wasm (panic locations) name no machine: the
 * toolchain's sources as std's own rlibs do (`/rustc/<commit>`), Cargo's
 * home as `cargo`, and the checkout, the app and the target relatively, so
 * the bytes that hashes and signatures cover are the same on every machine
 * and in every checkout. Every web-profile wasm build passes the same flags,
 * so they share one set of artifacts. Later prefixes win (rustc's rule). */
const rustcFacts = new Map();
export function wasmRemapFlags(app, toolchain = null) {
  const cwd = app?.workspace ?? ROOT, key = `${cwd}\0${toolchain ?? ''}`;
  const env = toolchain ? { ...process.env, RUSTUP_TOOLCHAIN: toolchain } : process.env;
  const rustc = (...args) => spawnSync('rustc', args, { cwd, env, encoding: 'utf8' }).stdout ?? '';
  if (!rustcFacts.has(key)) rustcFacts.set(key, { commit: /^commit-hash: (\S+)$/m.exec(rustc('-vV'))?.[1], sysroot: rustc('--print', 'sysroot').trim() });
  const { commit, sysroot } = rustcFacts.get(key);
  const pairs = [[resolve(process.env.CARGO_HOME ?? resolve(homedir(), '.cargo')), 'cargo']];
  if (sysroot && commit) pairs.push([resolve(sysroot, 'lib/rustlib/src/rust'), `/rustc/${commit}`]);
  pairs.push([ROOT, '']);
  if (app && resolve(app.workspace) !== ROOT) pairs.push([resolve(app.workspace), '']);
  if (app) pairs.push([resolve(app.target), 'target']);
  // Target rustflags replace [build].rustflags: retain the game's determinism promise.
  const flags = [...pairs.map(([from, to]) => `--remap-path-prefix=${from}=${to}`),
    ...(app?.manifest?.game ? ['-C', 'llvm-args=-fp-contract=off'] : []),
    ...(toolchain === WEB_TOOLCHAIN ? WEB_RUSTFLAGS : [])].map(f => JSON.stringify(f));
  return ['--config', `target.wasm32-unknown-unknown.rustflags=[${flags.join(',')}]`];
}

/** The web artifacts' toolchain (LLP 1047 §6, §10): a pinned nightly builds
 * std for size, and a panic aborts without its message or location. The web
 * shows neither: wasm32-unknown-unknown has no panic output and the glue
 * reads no trap. Everything else builds with rust-toolchain.toml's stable. */
export const WEB_TOOLCHAIN = 'nightly-2026-08-21';
export const WEB_STD = ['-Zbuild-std=std,panic_abort', '-Zbuild-std-features=optimize_for_size'];
// Deprecation is stable's to judge (the six checks' clippy): the nightly
// deprecates names, like `fetch_update`, before stable has their successors.
const WEB_RUSTFLAGS = ['-Zunstable-options', '-Cpanic=immediate-abort', '-Zlocation-detail=none', '-Adeprecated'];
export const WEB_SIZE = [...WEB_STD, ...WEB_RUSTFLAGS].join(' ');
/** The environment for a web build, refusing a machine without the toolchain
 * or its std sources (Cargo builds offline, so std's dependencies too). */
export function webToolchainEnv(env) {
  const sysroot = spawnSync('rustc', ['--print', 'sysroot'], { env: { ...env, RUSTUP_TOOLCHAIN: WEB_TOOLCHAIN }, encoding: 'utf8' });
  if (sysroot.status !== 0 || !existsSync(resolve(sysroot.stdout.trim(), 'lib/rustlib/src/rust/library/Cargo.lock'))) throw new Error(
    `the web artifact builds with ${WEB_TOOLCHAIN} and its std sources (LLP 1047):\n` +
    `  rustup toolchain install ${WEB_TOOLCHAIN} --profile minimal --component rust-src --component llvm-tools\n` +
    `  cargo +${WEB_TOOLCHAIN} fetch --manifest-path "$(rustc +${WEB_TOOLCHAIN} --print sysroot)/lib/rustlib/src/rust/library/Cargo.toml"`);
  // `-Zbuild-std` builds std offline, so std's own locked sources must be in
  // Cargo's cache. Checked once per toolchain (a marker beside the sysroot's
  // library) and fetched when missing, rather than failing mid-build.
  const library = resolve(sysroot.stdout.trim(), 'lib/rustlib/src/rust/library');
  const marker = resolve(library, '.exact-fetched');
  if (!existsSync(marker)) {
    const offline = spawnSync('cargo', ['metadata', '--offline', '--locked', '--format-version', '1', '--manifest-path', resolve(library, 'Cargo.toml')], { env: cargoEnvironment({ ...env, RUSTUP_TOOLCHAIN: WEB_TOOLCHAIN }), encoding: 'utf8', maxBuffer: 1 << 26 });
    let ready = offline.status === 0;
    if (!ready) {
      console.error(`${WEB_TOOLCHAIN}: std's sources are not fetched; fetching them once (cargo fetch --manifest-path …/library/Cargo.toml)`);
      ready = spawnSync('cargo', ['fetch', '--locked', '--manifest-path', resolve(library, 'Cargo.toml')], { env: cargoEnvironment({ ...env, RUSTUP_TOOLCHAIN: WEB_TOOLCHAIN }), stdio: ['ignore', 'inherit', 'inherit'] }).status === 0;
    }
    if (ready) try { writeFileSync(marker, ''); } catch { /* a read-only toolchain checks again next time */ }
  }
  return cargoEnvironment({ ...env, RUSTUP_TOOLCHAIN: WEB_TOOLCHAIN, EXACT_WEB_SIZE: WEB_SIZE });
}

// A Bun older than package.json's pin is refused before anything builds.
// Partial fixture copies of these scripts carry no package.json and no pin.
// Asking for help builds nothing, so it is answered on any Bun: an old Bun is
// exactly when someone reaches for `--help` first.
const HELP = process.argv.slice(2).some(a => a === '--help' || a === '-h' || a === 'help');
const PINNED_BUN = existsSync(resolve(ROOT, 'package.json'))
  ? JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).packageManager?.replace(/^bun@/, '') : null;
if (process.versions.bun && PINNED_BUN && !HELP) ensureBun(PINNED_BUN);

// `exact setup` installs Binaryen privately; every build finds the same pin.
const binaryenBin = resolve(homedir(), '.cache/exact/binaryen', BINARYEN.replace(' ', '_'), 'bin');
if (existsSync(resolve(binaryenBin, process.platform === 'win32' ? 'wasm-opt.exe' : 'wasm-opt'))) process.env.PATH = `${binaryenBin}${delimiter}${process.env.PATH ?? ''}`;

// @ref LLP 1043.000 §3 D7/D8 — one inventory for host builds, serving and fixtures.
// Groups preserve capability-based shipping; none of these imports enters boot.
// A group's files import statically only from it or base (web-host-groups.test.mjs): base's documents-glue.js
// reads storage-environment.js, so it ships in base, not only with storage.
const WEB_HOST_GROUPS = {
  base: ['glue.js', 'navigation.js', 'frame-messages.js', 'frame-navigation.js', 'textflow-glue.js', 'timer-glue.js', 'timer-waits-glue.js', 'timer-observations.js', 'input-glue.js', 'agent-input.js',
    'http-body.js', 'grant-admission.js', 'faults.js', 'address-space.js', 'media-glue.js', 'list-selection.js', 'markup-editor.js', 'atoms.js', 'decorations.js', 'live-time-glue.js', 'document-glue.js',
    'motion-glue.js', 'group-glue.js', 'collection-glue.js', 'canvas2d-glue.js', 'presence-glue.js', 'picker-glue.js',
    'documents-glue.js', 'storage-environment.js', 'auth-glue.js', 'image-glue.js', 'geometry-glue.js', 'resize-glue.js', 'intersect-glue.js', 'sound-glue.js', 'midi-glue.js', 'prompt-glue.js', 'alert-message.js', 'wakelock-glue.js', 'orientationlock-glue.js', 'menu-rows.js', 'fragment.js'],
  module: ['module-glue.js', 'module-worker.js', 'module-binary.js', 'module-prelude.js'],
  storage: ['storage-request.js', 'storage.js', 'storage-fs.js', 'storage-image.js', 'storage-sqlite.js',
    'storage-worker.js', 'sqlite3.mjs', 'sqlite3.wasm'],
  rust: ['rust-glue.js'],
  gpu: ['gpu-glue.js', 'pace.js', 'gpu-assets.js', 'gpu-pads.js', 'gpu-tier.js', 'gpu-refusal.js'],
  gpuModules: ['gpu-modules.js'],
  native: ['native-glue.js'],
  // A development page's frame sampler (LLP 1079 D3); a production build ships none.
  development: ['frames.js'],
};

/** A game's world surfaces by name, as its last GPU build declared them (`.shells/surfaces.json`); none before one. */
export function worldSurfaces(dir) {
  try { return Object.keys(JSON.parse(readFileSync(resolve(dir, '.shells/surfaces.json'), 'utf8'))); } catch { return []; }
}

/** The shell page with its world surfaces on its root (`<html data-exact-worlds="world …">`, LLP 1139.003 D1),
 * inserted into the `<html>` tag whatever its other attributes (the manifest's `lang`). */
export function markWorlds(html, names) {
  const value = names.join(' ').replace(/[&"<>]/g, c => ({ '&': '&amp;', '"': '&quot;', '<': '&lt;', '>': '&gt;' })[c]);
  return names.length ? html.replace(/<html\b/, `<html data-exact-worlds="${value}"`) : html;
}

/** The AT Protocol client-metadata documents an app with auth sessions
 * serves beside its callback page (LLP 1069.006 D6; after review, item 3):
 * one per `application_type`, since a document names one type and a `web`
 * client's redirect URIs are https only. `client-metadata.native` lists the
 * private-use schemes and claimed https callbacks, `client-metadata.web` the
 * origin's `/.exact/auth/callback`; each `client_id` is its own URL on the
 * manifest's origin. Nothing without an origin, or for a type with no
 * callback. The scope is the manifest's `auth.scope` (default `atproto`). */
export function authClientMetadata(app, callbacks) {
  if (!app.origin) return {};
  const origin = new URL(app.origin).origin, page = `${origin}/.exact/auth/callback`;
  const base = (type) => ({
    client_id: `${origin}/.exact/auth/client-metadata.${type}.json`, client_name: app.displayName, client_uri: origin,
    application_type: type, grant_types: ['authorization_code', 'refresh_token'], response_types: ['code'],
    scope: app.manifest.auth?.scope ?? 'atproto', token_endpoint_auth_method: 'none', dpop_bound_access_tokens: true,
  });
  const native = callbacks.filter((c) => !c.startsWith('http:') && c !== page);
  const web = callbacks.filter((c) => c === page);
  return Object.fromEntries([['client-metadata.native', native, 'native'], ['client-metadata.web', web, 'web']]
    .filter(([, uris]) => uris.length).map(([name, uris, type]) => [name, { ...base(type), redirect_uris: uris }]));
}

/** A build's module artifact against the app's roster (LLP 1024 D1, D8.4):
 * a roster tag the artifact has no factory for fails a release build, named,
 * and is a warning in development, where the node reports its status. */
export function checkModuleRoster(app, provided, where, release) {
  const missing = app.modules.tags.filter((tag) => !provided.includes(tag));
  if (!missing.length) return;
  const message = `${where}: the roster (app.json modules) names ${missing.join(', ')}, which the module artifact lacks${provided.length ? ` (it serves ${provided.join(', ')})` : ''}`;
  if (release) throw new Error(message);
  console.warn(`warning: ${message}; the node reports "error" at runtime`);
}
/** Public filename -> repo-relative source; omit groups to inventory every host file. */
export function webHostFiles(...groups) {
  return Object.fromEntries((groups.length ? groups : Object.keys(WEB_HOST_GROUPS))
    .flatMap(group => WEB_HOST_GROUPS[group]).map(name => [name,
      name === 'module-prelude.js' ? 'js/src/prelude.js' : name === 'module-binary.js' ? 'js/src/binary.js'
        : name.startsWith('sqlite3.') ? 'node_modules/@sqlite.org/sqlite-wasm/dist/' + (name === 'sqlite3.mjs' ? 'index.mjs' : name)
          : 'host/web/' + name]));
}

/** The app's GPU modules for the web (LLP 1009 D2, D6) into `stage`: each
 * GPU crate's wasm with wasm-bindgen's glue (its exports are the module's ABI
 * on the web), then wasm-opt; `<stem>.js` + `<stem>_bg.wasm`, the primary as
 * `gpu`, a declared module as `gpu/<name>`. `cargo` builds the crates first,
 * on stable (@ref LLP 1047: a GPU crate keeps its toolchain's std), as the wasm target's bake does
 * (the JS target has no bake); the wasm target passes false, having built them. Missing packaging tools refuse the build. */
export function webGpuArtifacts(app, stage, { cargo = false, env = process.env, profile = 'web' } = {}) {
  const artifacts = [...(app.hasGpu ? [{ crate: app.crate('gpu'), stem: 'gpu' }] : []),
    ...gpuModules(app.manifest).map(({ name }) => ({ crate: app.crate(`gpu-${name}`), stem: `gpu/${name}` }))];
  const kib = (n) => `${(n / 1024).toFixed(0)} KiB`;
  let note = '';
  for (const { crate, stem } of artifacts) {
    if (cargo) buildCommand('cargo', ['build', ...cargoReproducibilityFlags(app), ...injectedProfiles(app), ...wasmRemapFlags(app), '-p', crate,
      '--target', 'wasm32-unknown-unknown', '--profile', profile, '--lib', '--config', `profile.${profile}.strip=false`], app, cargoEnvironment({ ...env, CARGO_TARGET_DIR: app.target }), 'inherit');
    const wasm = resolve(app.target, 'wasm32-unknown-unknown', profile, crate.replace(/-/g, '_') + '.wasm');
    const [dir, name] = stem.includes('/') ? [resolve(stage, 'gpu'), stem.slice(4)] : [stage, stem];
    const wb = spawnSync('wasm-bindgen', ['--target', 'web', '--no-typescript', '--out-dir', dir, '--out-name', name, wasm], { stdio: 'inherit' });
    if (wb.error?.code === 'ENOENT') throw new Error('wasm-bindgen not on PATH; run bun scripts/exact.mjs setup in the SDK checkout');
    if (wb.status !== 0) throw new Error(`wasm-bindgen ${crate} failed`);
    const bg = resolve(stage, `${stem}_bg.wasm`);
    // `EXACT_WEB_NAMES=1` keeps the name section, as host/web/build.mjs does for app.wasm: a profile then names GPU functions.
    const o = profile === 'gpu-dev' ? {status:null} : spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', env.EXACT_WEB_NAMES === '1' ? '-g' : '--strip-debug', '--strip-producers', '-o', bg, bg], { stdio: 'inherit' });
    const bytes = readFileSync(bg);
    note += `${note ? '; ' : ''}${stem}_bg.wasm ${kib(bytes.length)} (${kib(gzipSync(bytes, { level: 9 }).length)} gzip${o.status === 0 ? ', wasm-opt' : ''}), ${stem}.js ${kib(readFileSync(resolve(stage, `${stem}.js`)).length)}`;
  }
  return { note: note || 'no GPU crate', built: artifacts.length > 0 };
}

/** The checkout containing `dir` and its repository's common git directory. */
function checkoutOf(dir) {
  for (let at = dir; ; at = dirname(at)) {
    const git = resolve(at, '.git');
    if (existsSync(git)) {
      if (statSync(git).isDirectory()) return { top: at, common: realpathSync(git) };
      const own = resolve(at, /^gitdir: (.+)$/m.exec(readFileSync(git, 'utf8'))?.[1].trim() ?? '.');
      const common = existsSync(resolve(own, 'commondir')) ? resolve(own, readFileSync(resolve(own, 'commondir'), 'utf8').trim()) : own;
      return { top: at, common: existsSync(common) ? realpathSync(common) : common };
    }
    if (dirname(at) === at) return null;
  }
}

/** The web build's output directory: `EXACT_WEB_DIST` when set; for an app
 * outside this repository, its own `target/web-dist` — a checkout's
 * `host/web/dist` is one slot every in-repo build writes (the checks rebuild
 * Caltrain into it), and an outside app served from it silently became
 * Caltrain (LLP 1054 O5); otherwise `host/web/dist`. */
export function webDist() {
  if (process.env.EXACT_WEB_DIST) return resolve(process.env.EXACT_WEB_DIST);
  if (process.env.EXACT_APP_DIR) return resolve(resolveApp().target, 'web-dist');
  return resolve(ROOT, 'host/web/dist');
}

/** The command that rebuilds `dist` for `app`: an app outside this checkout
 * with its own runner (`exact new`) is told its own `bun exact.mjs web-build`. */
export function webBuildCommand(app, dist, flags = '', otherwise = null) {
  const quote = value => "'" + String(value).replaceAll("'", "'\\''") + "'";
  const own = app.dir ? resolve(app.dir, 'exact.mjs') : null;
  if (own && process.env.EXACT_APP_DIR && !process.env.EXACT_WEB_DIST && existsSync(own) && !own.startsWith(`${ROOT}/`))
    return `cd ${quote(app.dir)} && bun exact.mjs web-build${flags}`;
  return otherwise ?? `EXACT_APP_DIR=${quote(app.dir)} EXACT_WEB_DIST=${quote(resolve(dist))} bun host/web/build.mjs ${app.crate('web')}${flags}`;
}

/** Refuse a target directory inside another checkout of the same repository:
 * worktrees sharing one target/ have failed with inputs that have no captured
 * source identity and with stale generated files. A private CARGO_TARGET_DIR
 * outside any checkout, or another repository's (an app consuming exact2 by
 * path), is fine. */
export function assertOwnTarget(target, workspace) {
  if (!existsSync(target)) return;
  const real = realpathSync(target), mine = checkoutOf(realpathSync(workspace)), theirs = checkoutOf(real);
  if (mine && theirs && theirs.common === mine.common && theirs.top !== mine.top) {
    throw new Error(`target directory ${target} resolves to ${real}, inside ${theirs.top}, another checkout of this repository. Give ${mine.top} its own target/ (remove the symlink), or set CARGO_TARGET_DIR to a directory outside every checkout.`);
  }
}

/** Cargo's package kind for an OS, including apps with a shared Apple crate. */
export function platformCrateKind(dir, kind) {
  return ['ios', 'macos', 'tvos'].includes(kind)
    ? (existsSync(resolve(dir, kind, 'Cargo.toml')) ? kind : 'apple') : kind;
}

/** Platform-specific source folder; shared Apple modules remain valid for existing apps. */
export function moduleDirectory(dir, platform) {
  const own = resolve(dir, platform, 'modules');
  return existsSync(own) ? own : resolve(dir, 'modules', ['ios', 'macos', 'tvos'].includes(platform) ? 'apple' : platform);
}

/** The app `nameOrCrate` names (`caltrain`, `caltrain-web`, …; `EXACT_APP_DIR`'s basename when unset): its directory, cargo workspace, target directory, crate names, and manifest. */
export function resolveApp(nameOrCrate) {
  // A path (`--app ../my-app`, `--app /abs/app`, `--app .`) names an app as
  // EXACT_APP_DIR does, which the processes it starts inherit: its name is
  // its directory's, so nothing built from the name (a crate, a target
  // subdirectory) lands in the app's own directory.
  if (nameOrCrate && /[\\/]|^\.\.?$/.test(String(nameOrCrate))) {
    process.env.EXACT_APP_DIR = resolve(String(nameOrCrate));
    nameOrCrate = undefined;
  }
  const outside = process.env.EXACT_APP_DIR ? resolve(process.env.EXACT_APP_DIR) : null;
  let name = nameOrCrate ? String(nameOrCrate).replace(/-(web|apple|ios|macos|linux|windows|gpu)$/, '') : outside ? basename(outside) : 'caltrain';
  let dir = outside ?? resolve(ROOT, 'apps', name);
  if (!outside && !existsSync(resolve(dir, 'app.contract')) && existsSync(resolve(ROOT, 'game/games', name, 'app.contract'))) dir = resolve(ROOT, 'game/games', name);
  if (!existsSync(resolve(dir, 'app.contract'))) throw new Error(`no app at ${dir} (no app.contract)${outside ? '' : '; set EXACT_APP_DIR for an app outside this repo'}`);
  dir = realpathSync(dir);
  // The bake runs this checkout's TypeScript compiler and bundler (partial
  // fixture copies carry no package.json, as PINNED_BUN says).
  if (PINNED_BUN) installedModules();
  // Materialize defaults before Cargo inspects workspace members on a clean checkout.
  const manifest = readManifest(dir, name);
  if (dirname(dir) === resolve(ROOT, 'game/games') && manifest.game === undefined) {
    throw new Error(`${dir}/app.json: game is required for an app under game/games/`);
  }
  let workspace = ROOT, checkLock = null;
  if (manifest.game === undefined) {
    // Cargo's package.workspace can put an in-repo app in a separate lock
    // without moving its sources. Look at authored member manifests rather
    // than guessing a workspace from the app's name or enclosing directory.
    const memberDirs = [dir, ...readdirSync(dir, { withFileTypes: true }).filter(entry => entry.isDirectory()).map(entry => resolve(dir, entry.name))];
    const declared = [...new Set(memberDirs.flatMap(member => {
      const path = resolve(member, 'Cargo.toml');
      if (!existsSync(path)) return [];
      const owner = Bun.TOML.parse(readFileSync(path, 'utf8')).package?.workspace;
      return owner === undefined ? [] : [realpathSync(resolve(member, owner))];
    }))];
    if (declared.length > 1) throw new Error(`${dir}: app crates declare different Cargo workspaces: ${declared.join(', ')}`);
    // Ordinary external apps may belong to an enclosing Cargo workspace. Games
    // always use their generated workspace below and need no Cargo process here.
    const manifests = memberDirs.map(member => resolve(member, 'Cargo.toml')), known = knownWorkspace(dir, !!outside, manifests);
    if (known) workspace = known;
    else {
      if (declared.length) workspace = declared[0];
      // From the files, never `cargo locate-project`: a web verb runs no Cargo in the app (LLP 1153 D2.1). An app that
      // authored no workspace builds in the generated one (scripts/app-shells.mjs), written when a build asks (`rust`).
      else if (outside) workspace = authoredWorkspace(dir) ?? shellsDir(dir);
      // EXACT_APP_DIR may name an app of this repo; only another workspace is checked, its lock when a build asks.
      if (workspace !== ROOT && workspace !== shellsDir(dir) && existsSync(resolve(workspace, 'Cargo.toml'))) {
        const problems = outsideWorkspaceProblems(workspace);
        if (problems.length) throw new Error(problems.join('\n'));
        checkLock = () => { checkLock = null; rememberWorkspace(dir, !!outside, manifests, workspace, () => checkOutsideLock(workspace)); };
      }
    }
  }
  if (manifest.game !== undefined) {
    name = manifest.game.crate.slice(0, -'-logic'.length);
    workspace = resolve(dir, '.shells');
  }
  const generated = !!outside && workspace === shellsDir(dir), own = manifest.game || generated ? dir : workspace;
  const target = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(own, 'target');
  assertOwnTarget(target, own);
  // What a build asks for before it runs Cargo in the workspace, once: a generated workspace written current with the
  // manifest (LLP 1153 D1.3) and its copied lock resolved (the first build fetches what the cache lacks), or an
  // authored workspace's lock checked. Every build helper reads `workspace`, so every build passes here; a web
  // verb reads `dir` and `target` and runs no Cargo (D2.1).
  let rusted = false;
  const rust = () => {
    if (!rusted) { rusted = true; if (generated) { ensureShells(dir, manifest); checkOutsideLock(workspace); } else checkLock?.(); }
    return workspace;
  };
  startSweep(target);
  let packages;
  const prepare = (refresh = false, options) => {
    if (manifest.game && (refresh || !packages)) {
      const metadata = prepareGame(dir, manifest.game, resolve(ROOT, 'game'), options);
      packages = metadata.packages;
      return metadata;
    }
  };
  const platformKind = kind => platformCrateKind(dir, kind);
  const crate = kind => `${name}-${platformKind(kind)}`;
  const cargoPackage = kind => {
    prepare();
    if (!packages) {
      rust();
      const result = lockedMetadata(workspace, true);
      if (result.status !== 0) throw new Error(`cargo metadata: ${result.stderr || result.error?.message}`);
      packages = JSON.parse(result.stdout).packages;
    }
    return packages.find(pkg => pkg.name === crate(kind));
  };
  return {
    name, dir, target, crate, generated, rust,
    get workspace() { return rust(); },
    cargoPackage, prepare,
    get hasGpu() {
      if (manifest.game !== undefined) return true;
      const gpu = resolve(dir, 'gpu');
      if (!existsSync(resolve(gpu, 'Cargo.toml'))) return false;
      const pkg = cargoPackage('gpu');
      return !!pkg && realpathSync(dirname(pkg.manifest_path)) === realpathSync(gpu);
    },
    /** The native-module roster (LLP 1024 D1) and its sources: the Swift under
     * `modules/apple` that becomes `libexact_modules.dylib`, and the web
     * executor `modules/web/index.js`. Empty tags: the app has no module
     * views; its web executor may still answer `native.later` on the page
     * (LLP 1067 D5), and its Swift still makes the artifact, for `later` and
     * the hatches (LLP 1075.003 §3.2). */
    modulesFor(platform) {
      const folder = moduleDirectory(dir, platform);
      const under = suffix => existsSync(folder) ? readdirSync(folder).filter(f => f.endsWith(suffix)).sort().map(f => resolve(folder, f)) : [];
      return { tags: manifest.modules ?? [], apple: under('.swift'), frameworks: under('.xcframework'), web: existsSync(resolve(folder, 'index.js')) ? resolve(folder, 'index.js') : null };
    },
    get modules() {
      return { ...this.modulesFor('apple'), web: this.modulesFor('web').web };
    },
    /** The manifest, validated; the derived defaults when the app has none. */
    manifest,
    /** The app identity, reverse-DNS: the bundle id on every platform (`build.mjs:48–49` derived it from the crate name before the manifest). */
    id: manifest.app.id,
    /** The name people see. */
    displayName: manifest.app.name,
    /** The production origin (1023 D1's URL), or null in an app that has not declared one. */
    origin: manifest.app.origin ?? null,
    /** Whether the app declares its own manifest (false: the defaults above stand in). */
    declared: existsSync(resolve(dir, 'app.json')),
  };
}

/** An app's authored tests (LLP 1017 P7), the files that exist: `app.test.contract`, then every other `*.test.contract`
 * beside it in name order (an app's tests split by area under the line cap). `test`, `verify` and the smoke run these. */
export function authoredTests(dir) {
  const others = readdirSync(dir).filter((n) => n.endsWith('.test.contract') && n !== 'app.test.contract').sort();
  return [...(existsSync(resolve(dir, 'app.test.contract')) ? ['app.test.contract'] : []), ...others].map((n) => resolve(dir, n));
}

/** `app.json` from `dir`, validated — or the defaults an app had before the manifest: `com.exact.<name>`, the name capitalized. */
export function readManifest(dir, name) {
  const path = resolve(dir, 'app.json');
  const fallback = { name: name[0].toUpperCase() + name.slice(1), app: { id: `com.exact.${name}`, name: name[0].toUpperCase() + name.slice(1) }, host: {}, deploy: {} };
  const game = gameDefaults(dir);
  if (!existsSync(path) && !game) return fallback;
  let parsed;
  try { parsed = game ?? JSON.parse(readFileSync(path, 'utf8')); } catch (e) { throw new Error(`${path}: ${e.message}`); }
  // LLP 1069.008 D4: a device's usage text is derived from its grant, never hand-written.
  const problems = validate(parsed, schema(), '', schema()).map((p) => /^host\.(ios|macos)\.permissions: /.test(p)
    ? `${p.split(':')[0]}: deleted (LLP 1069.008); declare the device in the source's grants as \`device.<name> <strings key>\` (e.g. \`device.microphone purpose.microphone\`) and put the text in strings/<locale>.json`
    // LLP 1046.008 amendment (2026-10-04): the GPU hooks crate was renamed.
    : /^game\.presentation: /.test(p)
      ? 'game.presentation is now game.render: rename the key, move presentation/ to render/, name the crate <name>-render and rename its "type" to "hooks" (LLP 1046.008, amendment of 2026-10-04)'
      : p);
  if (!problems.length) problems.push(...installProblems(parsed), ...gpuModuleProblems(parsed), ...documentTypeProblems(parsed), ...shareTargetProblems(parsed), ...protocolHandlerProblems(parsed), ...appleIconProblems(parsed, dir));
  if (problems.length) throw new Error(`${path} does not conform to scripts/app.schema.json:\n  ${problems.join('\n  ')}`);
  macResourceMappings(parsed);
  return { host: {}, deploy: {}, ...parsed };
}

import { DOCUMENT_UTIS } from './document-utis.mjs';
export { DOCUMENT_UTIS };
;

// The icon the Apple bundles are drawn from (host/apple/build.mjs
// `appIcon`): the first square one of at least 512 px, which must be there,
// as must the iOS launch screen's picture (`launchScreen`); refused when any
// build reads the manifest, as the file types are.
function appleIconProblems(manifest, dir) {
  const icon = (manifest.icons ?? []).find((i) => { const m = /^(\d+)x(\d+)$/.exec(i.sizes ?? ''); return m && m[1] === m[2] && Number(m[1]) >= 512; });
  const launch = manifest.host?.ios?.launchImage?.src;
  return [
    ...(icon && !existsSync(resolve(dir, icon.src)) ? [`icons: ${icon.src}, the square icon the Apple bundles are drawn from, does not exist`] : []),
    ...(launch && !existsSync(resolve(dir, launch)) ? [`host.ios.launchImage: ${launch}, the launch screen's picture, does not exist`] : []),
  ];
}

/** An app's own document format (studio diary R13): a MIME type in IANA's
 * vendor or unregistered trees (`application/vnd.studio.board+json`,
 * `application/x-studio-board`), which no system type names. The Apple
 * bake exports it as the app's: `<app id>.<subtype>`, conforming to what
 * its structured-syntax suffix says (`+json` is JSON) and to data; the app
 * is its editor and owner. `null` for any other type. */
export function ownDocumentType(appId, mime) {
  const m = /^[a-z]+\/(?:x-|vnd\.|prs\.)([a-z0-9][a-z0-9.+-]*)$/i.exec(mime);
  if (!m || Object.hasOwn(DOCUMENT_UTIS, mime)) return null;
  const [subtype, suffix] = m[1].toLowerCase().split('+');
  const base = { json: 'public.json', xml: 'public.xml', zip: 'public.zip-archive' }[suffix];
  return {
    identifier: `${appId}.${subtype.replace(/[^a-z0-9.-]/g, '-')}`,
    conformsTo: [...(base ? [base] : mime.startsWith('text/') ? ['public.plain-text'] : ['public.data']), 'public.content'],
  };
}
function documentTypeProblems(manifest) {
  return (manifest.file_handlers ?? []).flatMap((handler, i) => Object.entries(handler.accept)
    .flatMap(([mime, extensions]) => Object.hasOwn(DOCUMENT_UTIS, mime) ? []
      : !ownDocumentType('app', mime) ? [`file_handlers[${i}].accept: ${mime} names no type the Apple hosts map (they map ${Object.keys(DOCUMENT_UTIS).join(', ')}), nor an app's own (application/vnd.<app>.<format> or application/x-<format>, +json for JSON)`]
      : ![extensions].flat().some((e) => /^\.[^./]+$/.test(e)) ? [`file_handlers[${i}].accept: ${mime}, the app's own type, names no extension (".board")`]
      : []));
}
/** Whether a `protocol_handlers` scheme is the web's own: `http` and
 * `https` make a native app a web browser (LLP 1030 D2, 2026-10-20), and
 * no browser registers them (WHATWG HTML's protocol handler safelist). */
export const isWebScheme = (protocol) => ['http', 'https'].includes(protocol.toLowerCase());
function protocolHandlerProblems({ protocol_handlers = [], host }) {
  const web = protocol_handlers.filter((h) => isWebScheme(h.protocol));
  const pair = web.length && (new Set(web.map((h) => h.protocol.toLowerCase())).size !== 2 || web.some((h) => h.url !== web[0].url || !h.distribution_only !== !web[0].distribution_only));
  const claimed = ['ios', 'macos'].flatMap((platform) => (host?.[platform]?.urlSchemes ?? []).filter(isWebScheme).map((s) => `host.${platform}.urlSchemes: ${s} is the web's own scheme; declare http and https together in protocol_handlers`));
  return [...claimed, ...protocol_handlers.flatMap(({ protocol }, i) => protocol_handlers.findIndex(h => h.protocol.toLowerCase() === protocol.toLowerCase()) === i ? [] : [`protocol_handlers[${i}].protocol: ${protocol} is declared more than once`]),
    ...(host?.ios?.webBrowser && web.length === 0 ? ['host.ios.webBrowser: declare http and https together in protocol_handlers'] : []),
    ...(pair ? ['protocol_handlers: http and https come together, with one url and one distribution_only: a web browser claims both'] : [])];
}
/** The W3C keys of `app.json`, as the web builds write `manifest.json`
 * (LLP 1030 D2/D10; 1030.000 D7). A browser opens files only, so
 * `inode/directory` (the Apple bake's word for a folder) goes, and it
 * registers neither `http` nor `https`, so those handlers go too. */
export function webAppManifest(manifest, name, notes = []) {
  const keys = ['name', 'short_name', 'description', 'id', 'start_url', 'display', 'display_override', 'theme_color', 'background_color', 'icons', 'lang', 'orientation', 'file_handlers', 'protocol_handlers', 'launch_handler', 'shortcuts'];
  const web = Object.fromEntries(keys.filter((k) => manifest[k] !== undefined).map((k) => [k, manifest[k]]));
  if (web.file_handlers) web.file_handlers = web.file_handlers.filter((h) => !Object.keys(h.accept ?? {}).includes('inode/directory'));
  if (web.protocol_handlers) web.protocol_handlers = web.protocol_handlers.filter((h) => !isWebScheme(h.protocol));
  // A shortcut's `apple` and `android` words are the native bakes' (LLP 1133.023 D11).
  if (web.shortcuts) web.shortcuts = web.shortcuts.map(({ apple, android, ...item }) => item);
  if (manifest.share_target) web.share_target = webShareTarget(manifest.share_target, notes);
  for (const key of ['file_handlers', 'protocol_handlers', 'shortcuts']) if (!web[key]?.length) delete web[key];
  web.name ??= name;
  web.start_url ??= '/';
  // The document's language (`<html lang>`, WCAG 3.1.1). Every app here is in
  // English, so an app that declares none is `en`.
  web.lang ??= 'en';
  return web;
}
export function nativeManifestMetadata(manifest, platform) { return {app:manifest.app,host:manifest.host?.[platform]??{},icons:manifest.icons??[],...(manifest.frame_health ? {frame_health:manifest.frame_health} : {}),...(['macos','ios'].includes(platform) && manifest.protocol_handlers?.length ? {protocol_handlers:manifest.protocol_handlers} : {}),...(['ios','android'].includes(platform) && manifest.orientation ? {orientation:manifest.orientation} : {})}; }
let cachedSchema = null;
function schema() {
  return cachedSchema ??= JSON.parse(readFileSync(resolve(ROOT, 'scripts/app.schema.json'), 'utf8'));
}
/** The subset of JSON Schema the manifest's schema uses — type, required, properties, additionalProperties, items, minItems, enum, pattern, minLength, oneOf, $ref into $defs — checked by hand so the reader needs no dependency. Every problem in one pass. */
export function validate(value, node, at, root) {
  const problems = [];
  const where = at || '(root)';
  if (node.$ref) {
    const target = node.$ref.replace(/^#\//, '').split('/').reduce((o, k) => o?.[k], root);
    if (!target) return [`${where}: schema reference ${node.$ref} does not resolve`];
    return validate(value, target, at, root);
  }
  if (node.oneOf) {
    const fits = node.oneOf.filter((alt) => validate(value, alt, at, root).length === 0);
    if (fits.length !== 1) problems.push(`${where}: ${JSON.stringify(value)} matches ${fits.length} of the allowed forms (needs exactly one)`);
    return problems;
  }
  const types = node.type ? [].concat(node.type) : null;
  const actual = value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value;
  if (types && !types.includes(actual)) { problems.push(`${where}: expected ${types.join(' or ')}, got ${actual}`); return problems; }
  if (node.enum && !node.enum.includes(value)) problems.push(`${where}: ${JSON.stringify(value)} is not one of ${node.enum.map((e) => JSON.stringify(e)).join(', ')}`);
  if (typeof value === 'string') {
    if (node.minLength != null && value.length < node.minLength) problems.push(`${where}: shorter than ${node.minLength}`);
    if (node.pattern && !new RegExp(node.pattern).test(value)) problems.push(`${where}: ${JSON.stringify(value)} does not match ${node.pattern}${node['x-hint'] ? `: ${node['x-hint']}` : ''}`);
  }
  if (actual === 'number' && (!Number.isFinite(value) || (node.minimum != null && value < node.minimum) || (node.exclusiveMinimum != null && value <= node.exclusiveMinimum) || (node.maximum != null && value > node.maximum))) problems.push(`${where}: outside the allowed numeric range`);
  if (actual === 'array' && node.minItems != null && value.length < node.minItems) problems.push(`${where}: fewer than ${node.minItems} items`);
  if (actual === 'array' && node.maxItems != null && value.length > node.maxItems) problems.push(`${where}: more than ${node.maxItems} items`);
  if (actual === 'array' && node.items) value.forEach((v, i) => problems.push(...validate(v, node.items, `${at}[${i}]`, root)));
  if (actual === 'object') {
    for (const key of node.required ?? []) if (!(key in value)) problems.push(`${where}: missing required ${JSON.stringify(key)}`);
    for (const [key, v] of Object.entries(value)) {
      const sub = node.properties?.[key];
      const path = at ? `${at}.${key}` : key;
      if (sub) problems.push(...validate(v, sub, path, root));
      else if (node.additionalProperties && typeof node.additionalProperties === 'object') problems.push(...validate(v, node.additionalProperties, path, root));
      else if (node.additionalProperties === false) problems.push(`${path}: not a known key`);
    }
  }
  return problems;
}

// rust-toolchain.toml pins the toolchain; an ambient RUSTUP_TOOLCHAIN (`mise exec`
// exports `stable`) would override it with one that lacks the wasm target, and
// Cargo's error names neither. Partial fixture copies carry no toolchain file.
const PINNED_RUST = existsSync(resolve(ROOT, 'rust-toolchain.toml'))
  ? /^channel\s*=\s*"([^"]+)"/m.exec(readFileSync(resolve(ROOT, 'rust-toolchain.toml'), 'utf8'))?.[1] : null;
let ignoredToolchain = null;

/** Developer entrypoints explicitly bake unsigned-update permission. Direct Cargo/contract bakes default to production; release callers can select it here too.
 * The Rust build scripts spawn `bun`: the one that ran this script's version check leads the child's PATH. */
export function developmentBuildEnv() {
  const env = cargoEnvironment({ ...process.env, EXACT_UPDATE_TRUST: process.env.EXACT_UPDATE_TRUST ?? 'development' });
  const toolchain = env.RUSTUP_TOOLCHAIN;
  if (toolchain && PINNED_RUST && toolchain !== PINNED_RUST && !toolchain.startsWith(`${PINNED_RUST}-`)) {
    if (ignoredToolchain !== toolchain) process.stderr.write(`ignoring RUSTUP_TOOLCHAIN=${toolchain}: rust-toolchain.toml pins ${PINNED_RUST}\n`);
    ignoredToolchain = toolchain;
    delete env.RUSTUP_TOOLCHAIN;
  }
  // process.env looks up Windows names case-insensitively; spreading it into a
  // plain object does not. Preserve Path and emit only one spelling for children.
  const path = env.PATH ?? Object.entries(env).find(([key]) => key.toLowerCase() === 'path')?.[1] ?? '';
  for (const key of Object.keys(env)) if (key !== 'PATH' && key.toLowerCase() === 'path') delete env[key];
  if (process.versions.bun) env.BUN = process.execPath;
  env.PATH = [...(process.versions.bun ? [dirname(process.execPath)] : []), ...path.split(delimiter).filter(Boolean)].join(delimiter);
  return env;
}
/** Rust replacement capability baked into this platform/environment, independent
 * of release cadence and rebuild triggers. @ref LLP 1029.000 §2. */
export function rustPolicy(manifest, platform, environment = 'dev') {
  if (!['web', 'ios', 'macos', 'linux', 'android', 'windows'].includes(platform)) throw new Error(`unknown Rust replacement platform: ${platform}`);
  if (!['dev', 'prod'].includes(environment)) throw new Error(`unknown Rust replacement environment: ${environment}`);
  const policy = manifest.rust;
  if (policy !== undefined) {
    const problems = validate(policy, schema().properties.rust, 'rust', schema());
    if (problems.length) throw new Error(problems.join('\n'));
  }
  const choice = (value) => value === true ? 'auto' : value === false ? 'off' : typeof value === 'string' ? value : value?.mode;
  const surface = policy?.platforms?.[platform];
  let mode = 'auto';
  for (const value of [policy, policy?.[environment], surface, surface?.[environment]]) mode = choice(value) ?? mode;
  if (mode === 'off') return 'off';
  if (['native', 'tiered'].includes(mode) && ['web', 'ios'].includes(platform)) throw new Error(`rust: ${mode} replacement is unavailable on ${platform}; use wasm or off`);
  if (platform === 'web') return 'browser';
  if (mode !== 'auto') return mode;
  return platform === 'ios' || (platform === 'android' && environment === 'prod') ? 'wasm' : 'native';
}

/** Starting the development watcher opts into save-triggered builds; agents
 * can select manual per language and issue an explicit rebuild when ready. */
export function rebuildPolicy(manifest) {
  if (manifest.dev !== undefined) {
    const problems = validate(manifest.dev, schema().properties.dev, 'dev', schema());
    if (problems.length) throw new Error(problems.join('\n'));
  }
  return { rust: 'save', typescript: 'save', ...manifest.dev?.rebuild };
}

/** An app-specific OS opening action, not the app URL or an authentication credential. */
export const developmentURLScheme = (appId) => 'exact2-' + createHash('sha256').update(appId).digest('hex').slice(0, 32);

/** Canonical source ownership, including external apps and shared targets. */
export const appSourceKey = (app) => createHash('sha256').update(realpathSync(app.dir)).digest('hex').slice(0, 24);
/** The private directory receiving documents emitted by actual app build scripts. */
export function bakeOutput(app, env = process.env) {
  return env.EXACT_BAKE_OUTPUT ?? resolve(app.target, 'bake', appSourceKey(app), app.id, env.EXACT_UPDATE_TRUST ?? 'development');
}

/** Read and validate the receipt emitted by the app's actual target/grants bake. */
export function readBake(app, platform, target, directory = bakeOutput(app)) {
  const receipt = JSON.parse(readFileSync(resolve(directory, `${platform}-${target}.json`), 'utf8'));
  const canonical = (v) => v === null || typeof v !== 'object' ? JSON.stringify(v) : Array.isArray(v) ? `[${v.map(canonical).join(',')}]` : `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canonical(v[k])}`).join(',')}}`;
  const id = createHash('sha256').update('exact2 compatibility id v1\n').update(canonical(receipt.inputs)).digest('hex').slice(0, 32);
  if (receipt.id !== id || receipt.inputs.app !== app.id || receipt.inputs.platform !== platform || receipt.target !== target || !receipt.embedded || !Array.isArray(receipt.embedded.assets)) throw new Error(`invalid baked receipt for ${app.id} ${platform} ${target}`);
  return receipt;
}

/** Refuse packaging bytes that differ from the binary's complete bake receipt.
 * The usual cause is an edit while the build ran: the bake read one version of a
 * file and packaging copied another. The error's `rebuild` is true, so a build
 * can name its own command (host/apple/build.mjs). */
export function verifyBakeFiles(receipt, plan, assets) {
  const embedded = receipt.embedded;
  const ordered = (cards) => [...cards].sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  const refuse = (what) => Object.assign(new Error(`the packaged ${what} differ${what === 'plan' ? 's' : ''} from the binary bake receipt: a source changed after the bake read it (an edit while the build ran); build again`), { rebuild: true });
  if (!embedded || !Array.isArray(embedded.assets)
      || embedded.plan?.sha256 !== createHash('sha256').update(plan).digest('hex')
      || embedded.plan?.bytes !== plan.length) {
    throw refuse('plan');
  }
  const copied = ordered(assets), baked = ordered(embedded.assets);
  const same = (a, b) => a && b && ['name', 'sha256', 'bytes'].every((key) => a[key] === b[key]);
  const differ = [...new Set([...copied, ...baked].map((a) => a.name))]
    .filter((name) => !same(copied.find((a) => a.name === name), baked.find((a) => a.name === name)));
  if (differ.length) throw refuse(`static files (${differ.slice(0, 3).join(', ')}${differ.length > 3 ? `, ${differ.length - 3} more` : ''})`);
}

// The compiler owns both the loaded files and the bundle requirements. This
// outer receipt is completed after Cargo succeeds; it is not embedded in the
// product whose inputs it describes. @ref LLP 1030 D3/D3a.
const canonicalBuild = (v) => v === null || typeof v !== 'object' ? JSON.stringify(v) : Array.isArray(v) ? `[${v.map(canonicalBuild).join(',')}]` : `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canonicalBuild(v[k])}`).join(',')}}`;
const buildHash = (v) => createHash('sha256').update(v).digest('hex');
/** A development receipt names four thousand inputs by SHA-256, on every build,
 * and nearly all of them are the files they were at the last one: reading them
 * again was 0.3 s of a 2.5 s build with nothing changed. Each hash is kept in
 * the target directory with the size and time its file had, which is what
 * Cargo goes by to say the unit built from it is fresh. A production receipt
 * reads every byte. */
function inputHashes(app, env) {
  const kept = resolve(app.target, 'bake-input-hashes.json'), production = env.EXACT_UPDATE_TRUST === 'production';
  let known = {}, grew = false;
  if (!production) try { known = JSON.parse(readFileSync(kept, 'utf8')); } catch { /* none yet */ }
  const used = {};
  return {
    of(path, info) {
      const was = known[path];
      const now = was && was[0] === info.mtimeMs && was[1] === info.size ? was : [info.mtimeMs, info.size, buildHash(readFileSync(path))];
      if (now !== was) grew = true;
      used[path] = now;
      return now[2];
    },
    save() {
      if (production || !grew) return;
      // Every app of a checkout shares the file; one that has grown past any of them starts again from this build's.
      const next = Object.keys(known).length > 40_000 ? used : { ...known, ...used };
      writeFileSync(`${kept}.${process.pid}.tmp`, JSON.stringify(next)); renameSync(`${kept}.${process.pid}.tmp`, kept);
    },
  };
}
const under = (root, path) => {
  const child = relative(root, path);
  return child === '' || (!isAbsolute(child) && child !== '..' && !child.startsWith('..' + sep));
};
const orderedBuild = (rows) => rows.sort((a, b) => Buffer.compare(Buffer.from(canonicalBuild(a)), Buffer.from(canonicalBuild(b))));
function buildCommand(command, args, app, env, stderr = 'pipe') {
  if (command === 'cargo') env = cargoEnvironment(env);
  let result = spawnSync(command, args, { cwd: app.workspace, env, stdio: ['ignore','pipe',stderr], encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
  // An offline Cargo that lacks a source it needs (a new workspace, a new
  // lock entry) fetches the lock's sources once and tries again, instead of
  // failing on the first missing crate (LLP 1054 O2).
  if (command === 'cargo' && result.status !== 0 && /--offline was specified|attempting to make an HTTP request|in the offline mode|offline mode \(via `--offline`\)/.test(result.stderr ?? '')) {
    console.error(`${app.name}: Cargo's sources are not all fetched; fetching the locked ones (cargo fetch --locked) and trying again`);
    const fetched = spawnSync('cargo', ['fetch', '--locked'], { cwd: app.workspace, env, stdio: ['ignore', 'inherit', 'inherit'] });
    if (fetched.status === 0) result = spawnSync(command, args, { cwd: app.workspace, env, stdio: ['ignore','pipe',stderr], encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
  }
  if (result.error || result.status !== 0) {
    if (command === 'cargo') throwContractErrors(app, args, result, ROOT);
    // Said short: the profiles' `--config` pairs and a JSON build's artifact lines bury the diagnostics above.
    const said = args.filter((arg, i) => arg !== '--config' && args[i - 1] !== '--config').join(' '), out = (result.stdout ?? '').split('\n').filter(line => !line.includes('"reason":')).join('\n').slice(-4000);
    throw new Error(`${command} ${said} failed: ${result.error?.message ?? result.stderr ?? `exit ${result.signal ?? result.status} (see diagnostics above)`}${out.trim() ? `\n${out}` : ''}`);
  }
  return result;
}
/** A build script's call into the bake: a Contract that does not compile
 * ends the process with its diagnostics as the last thing printed, with no
 * stack after them (LLP 1054 L9); any other failure is thrown on. */
export function contractLast(build) {
  try { return build(); } catch (error) { if (!error?.contract) throw error; console.error(error.message); process.exit(1); }
}
export const HERMES_INSTALLER = resolve(ROOT, 'vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml');
/** Rust's host triple; cross targets such as iOS are named by their build. */
export function hermesTarget(os = process.platform, cpu = process.arch) {
  const arch = cpu === 'arm64' ? 'aarch64' : cpu === 'x64' ? 'x86_64' : null;
  const suffix = os === 'darwin' ? 'apple-darwin' : os === 'linux' ? 'unknown-linux-gnu' : os === 'win32' && arch === 'x86_64' ? 'pc-windows-msvc' : null;
  return arch && suffix ? `${arch}-${suffix}` : null;
}
/** The selected digest-addressed bundle; acquisition is installer-only. */
export function hermesBundle(target = hermesTarget(), env = process.env) {
  if (!target) throw new Error(`no pinned Hermes bundle target for ${process.platform}/${process.arch}`);
  const support = readFileSync(resolve(ROOT, 'vendor/ibex/crates/hermes-lean-sys/build_support.rs'), 'utf8'), tag = /RELEASE_TAG: &str = "([^"]+)"/.exec(support)?.[1];
  const blocks = [...support.matchAll(/BundlePin \{([\s\S]*?)\n    \}/g)].map(match => match[1]);
  const block = blocks.find(value => new RegExp(`target:\\s*"${target.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}"`).test(value));
  const digest = /sha256:\s*"([0-9a-f]{64})"/.exec(block ?? '')?.[1];
  if (!tag || !digest) throw new Error(`hermes-lean-sys has no pinned bundle for ${target}`);
  const cache = resolve(env.CARGO_HOME ?? resolve(env.HOME ?? homedir(), '.cargo'), 'hermes-lean-sys'), root = resolve(env.HERMES_LEAN_SYS_DIR ?? resolve(cache, tag, digest));
  const host = target === hermesTarget();
  const lean = resolve(root, 'lib', target.endsWith('-windows-msvc') ? 'hermesvmlean_a.lib' : 'libhermesvmlean_a.a'), hermesc = resolve(root, 'bin', target.endsWith('-windows-msvc') ? 'hermesc.exe' : 'hermesc');
  const required = [resolve(root, 'include'), lean, resolve(root, 'hermes-input-receipt.json'), ...(host ? [hermesc] : [])];
  const missing = required.filter(path => !existsSync(path));
  return { target, tag, digest, root, lean, hermesc, installed: missing.length === 0, missing, fix: `cargo run --manifest-path ${HERMES_INSTALLER} -- --target ${target}` };
}
/** Classification roots for Ibex's verified install-once Hermes bundles.
 * The digest directory remains part of the receipt name, so a bundle update
 * necessarily changes the bake identity. */
export function hermesLeanSysRoots(env = process.env) {
  const requested = resolve(env.HERMES_LEAN_SYS_DIR
    ?? resolve(env.CARGO_HOME ?? resolve(env.HOME ?? homedir(), '.cargo'), 'hermes-lean-sys'));
  const roots = [requested, ...(existsSync(requested) ? [realpathSync.native(requested)] : [])];
  return [...new Set(roots.flatMap(path => process.platform === 'win32' ? [path,toNamespacedPath(path)] : [path]))];
}
/** The profile a development native build compiles with (Cargo.toml): an
 * Apple app through host/apple/build.mjs, the Linux host by `linuxBuild`.
 * What ships is baked at `release`. */
export const HOST_DEV = 'host-dev';
/** The Linux host an agent drives: the app's development executable, or
 * another binary of its Linux crate (`<app>-render`). */
export const linuxBinary = (app, bin = app.crate('linux')) => resolve(app.target, HOST_DEV, bin);
/** The command that builds it. An app outside this repo gets the root's
 * profiles on the command line, as its Apple build does (injectedProfiles). */
export const linuxBuild = (app, bin = null) => ['cargo', 'build', ...injectedProfiles(app), '--profile', HOST_DEV, '-p', app.crate('linux'), ...(bin ? ['--bin', bin] : [])];
/** The executable (and Apple bundle) as Xcode and Visual Studio name a project's: the app's name, less what a file name cannot carry (LLP 1030 D2); `windowsFile` is a Windows build's name for a Cargo product. */
export const executableName = (app) => app.displayName.replace(/[<>:"/\\|?*\x00-\x1f]/g, '-').replace(/^[. ]+|[. ]+$/g, '') || app.id.split('.').pop();
export const windowsFile = (app, path) => basename(path) === `${app.crate('windows')}.exe` ? `${executableName(app)}.exe` : basename(path);
export function bakeTarget(platform) {
  if (platform === 'web') return 'wasm32-unknown-unknown';
  if (platform === 'ios') return 'aarch64-apple-ios';
  // The Linux host is also a supported headless executable on macOS. This
  // is the actual target Cargo will build here, never a guessed architecture.
  const result = spawnSync('rustc', ['-vV'], { encoding: 'utf8' });
  const host = /^host: (.+)$/m.exec(result.stdout ?? '')?.[1];
  if (result.status !== 0 || !host) throw new Error('rustc did not report its host target');
  return host;
}
/** Cargo's metadata for a development bake, asked once for a lock and a set of
 * manifests (0.17 s of every build): kept in the target directory with the
 * size and time of the lock, the toolchain file and every path crate's
 * manifest, and asked again when one differs. A production bake asks. */
function bakeMetadata(app, target, env) {
  const args = ['metadata', ...cargoReproducibilityFlags(app), '--format-version', '1', '--filter-platform', target];
  const ask = () => buildCommand('cargo', args, app, env).stdout;
  if (env.EXACT_UPDATE_TRUST === 'production') return JSON.parse(ask());
  const seen = (path) => { try { const info = statSync(path); return [info.mtimeMs, info.size]; } catch { return null; } };
  const fixed = ['Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml', '.cargo/config.toml'].map(file => resolve(app.workspace, file));
  const kept = resolve(app.target, 'bake-metadata', `${buildHash(canonicalBuild([args, app.workspace, env.CARGO_TARGET_DIR ?? null, env.CARGO_BUILD_BUILD_DIR ?? null, env.RUSTUP_TOOLCHAIN ?? null, env.CARGO_HOME ?? null])).slice(0, 16)}.json`);
  try {
    const was = JSON.parse(readFileSync(kept, 'utf8'));
    if (was.watched.every(([path, at]) => canonicalBuild(seen(path)) === canonicalBuild(at))) return was.metadata;
  } catch { /* none kept, or not as it was */ }
  const text = ask(), metadata = JSON.parse(text);
  const watched = [...fixed, ...metadata.packages.filter(p => p.source === null).map(p => p.manifest_path)].map(path => [path, seen(path)]);
  mkdirSync(dirname(kept), { recursive: true });
  writeFileSync(`${kept}.${process.pid}.tmp`, JSON.stringify({ watched, metadata })); renameSync(`${kept}.${process.pid}.tmp`, kept);
  return metadata;
}
function buildGraph(app, target, kind, env, gpu) {
  const prepared = app.prepare?.(true, {target, env});
  const metadata = prepared?.workspace_root === app.workspace ? prepared : bakeMetadata(app, target, env);
  const packages = new Map(metadata.packages.map((p) => [p.id, p]));
  const nodes = new Map(metadata.resolve.nodes.map((n) => [n.id, n]));
  const root = metadata.packages.find((p) => p.name === app.crate(kind));
  const surface = gpu && metadata.packages.find((p) => p.name === app.crate('gpu'));
  if (!root) throw new Error(`Cargo has no ${app.crate(kind)} target`);
  if (gpu && !surface) throw new Error(`Cargo has no GPU surface ${app.crate('gpu')}`);
  const modules = gpuModules(app.manifest).map(({name}) => metadata.packages.find((p) => p.name === app.crate(`gpu-${name}`))
    ?? (() => { throw new Error(`gpu.modules.${name}: Cargo has no ${app.crate(`gpu-${name}`)}`); })());
  const roles = new Map(), pending = [[root.id, target], ...[surface, ...modules].filter(Boolean).map((pkg) => [pkg.id, target])];
  while (pending.length) {
    let [id, role] = pending.pop();
    const pkg = packages.get(id), node = nodes.get(id);
    if (!pkg || !node) throw new Error(`incomplete Cargo dependency graph: ${id}`);
    if (pkg.targets.some((t) => t.kind.includes('proc-macro'))) role = 'host';
    const known = roles.get(id) ?? new Set(); if (known.has(role)) continue;
    known.add(role); roles.set(id, known);
    for (const dep of node.deps) if (dep.dep_kinds.some((k) => k.kind === null)) pending.push([dep.pkg, role]);
  }
  return { metadata, packages, root, surface, modules, roles };
}
function completeBuild(app, platform, target, graph, messages, roots, env, prepared = new Map()) {
  const scripts = messages.filter((m) => m.reason === 'build-script-executed' && graph.roles.has(m.package_id));
  const targetDirs = [app.target, graph.metadata.build_directory].map(dir => resolve(dir, target));
  const roleOf = (path) => targetDirs.some(dir => under(dir, path)) ? target : 'host';
  const generated = scripts.map((m) => ({ path: resolve(m.out_dir), pkg: graph.packages.get(m.package_id), role: roleOf(m.out_dir) })).sort((a,b) => b.path.length-a.path.length);
  const rootOutput = generated.find((g) => g.pkg.id === graph.root.id && g.role === target)?.path;
  if (!rootOutput) throw new Error('Cargo did not report the selected target bake output');
  const baked = existsSync(resolve(rootOutput, 'compat.json')) ? rootOutput : env.EXACT_BAKED /* a bake before Cargo keeps the plan and its receipts apart (linked-plan.mjs) */, compat = JSON.parse(readFileSync(resolve(baked, 'compat.json'), 'utf8'));
  if (compat.target !== target || compat.inputs.platform !== platform || compat.inputs.app !== app.id) throw new Error('actual bake receipt names another target, platform or app');
  const bundleGraph = JSON.parse(readFileSync(resolve(baked, 'artifacts.json'), 'utf8'));
  // Recreate packaging outputs from Cargo's actual bake, even on a cache hit.
  // A build script must never watch its own receipt: writing it dirties every next build.
  const stem = `${platform}-${target}`;
  for (const [file, suffix] of [['compat.json', '.json'], ['app.plan', '.plan'], ['artifacts.json', '.artifacts.json']]) writeFileSync(resolve(env.EXACT_BAKE_OUTPUT, stem + suffix), readFileSync(resolve(baked, file)));
  // A development bake's source map, for the agent driver only (LLP 1012.001.000 D6); none is carried stale.
  const map = resolve(baked, 'app.plan.map.json'), mapCopy = resolve(env.EXACT_BAKE_OUTPUT, stem + '.plan.map.json');
  if (existsSync(map)) writeFileSync(mapCopy, readFileSync(map)); else rmSync(mapCopy, { force: true });
  const replaced = new Set(['app.plan','compat.json','artifacts.json'].map((n) => resolve(rootOutput,n)));
  const packages = [...graph.roles.keys()].map((id) => graph.packages.get(id));
  const locations = packages.map((p) => ({ path: dirname(p.manifest_path), name:`crate:${p.name}@${p.version}` })).sort((a,b) => b.path.length-a.path.length);
  const hermesLeanRoots = hermesLeanSysRoots(env);
  // Index roots once; each input walks ancestors to the nearest declaring root.
  const rootsAt = (roots) => { const at = new Map(); for (const root of roots) if (!at.has(root.path)) at.set(root.path, root); return at; };
  // Each installed direct dependency, where it is installed and where its files really are: Bun installs a file:
  // package as directories of links to its source's files, so a compiler reads them from the source directory.
  const dependencies = directDependencies(app.dir).flatMap(({ name, root }) => [realpathSync(root), dirname(realpathSync(resolve(root, 'package.json')))].map(path => ({ path, name }))).sort((a,b) => b.path.length-a.path.length);
  const generatedAt = rootsAt(generated), locatedAt = rootsAt(locations), installedAt = rootsAt(dependencies);
  const rootOf = (roots, path) => { for (let at = path; ; at = dirname(at)) { const root = roots.get(at); if (root || dirname(at) === at) return root; } };
  const nameOf = (path) => {
    path = sourcePath(resolve(path));
    const made = rootOf(generatedAt, path);
    if (made) return `generated:${made.pkg.name}:${made.role}/${relative(made.path,path)}`;
    const pkg = rootOf(locatedAt, path);
    if (pkg) return `${pkg.name}/${relative(pkg.path,path)}`;
    const hermesLeanRoot = hermesLeanRoots.find(root => under(root,path));
    if (hermesLeanRoot) return `hermes-lean-sys/${relative(hermesLeanRoot,path)}`;
    if (under(LIBRARY_CACHE,path)) return `package-library/${relative(LIBRARY_CACHE,path)}`; // by its hash and target (package-libraries.mjs)
    const installed = rootOf(installedAt, path);
    if (installed) return `package/${installed.name}/${relative(installed.path,path)}`;
    if (under(app.dir,path)) return `app/${relative(app.dir,path)}`;
    if (under(ROOT,path)) return `exact/${relative(ROOT,path)}`;
    if (under(graph.metadata.workspace_root,path)) return `workspace/${relative(graph.metadata.workspace_root,path)}`;
    throw new Error(`compiler input has no captured source identity: ${path}`);
  };
  const inputs = new Map(), absent = new Map(), directories = new Map(), hashes = inputHashes(app, env);
  const add = (path, optional = false) => {
    path = resolve(path); if (replaced.has(path)) return;
    if (!existsSync(path)) { if (optional) { absent.set(nameOf(path), path); return; } throw new Error(`stale compiler dependency names missing input ${path}; rebuild that Cargo unit`); }
    const name = nameOf(path);
    if (inputs.get(name)?.path === path) return;
    const info = statSync(path);
    if (info.isDirectory()) { const names = readdirSync(path).sort(); directories.set(name, {path,names}); for (const entry of names) add(resolve(path,entry)); }
    else if (info.isFile()) inputs.set(name, {name,path,sha256:hashes.of(path, info)});
    else throw new Error(`unsupported compiler input ${path}`);
  };
  const normalizeEnv = ([key,value]) => [key, value == null ? null : ['OUT_DIR','CARGO_MANIFEST_DIR'].includes(key) ? nameOf(value) : buildHash(value)];
  const units = [], usedPackages = new Set();
  for (const m of messages.filter((m) => m.reason === 'compiler-artifact' && !m.target.kind.includes('custom-build') && graph.roles.has(m.package_id))) {
    const role = roleOf(m.filenames[0]); if (!graph.roles.get(m.package_id).has(role)) continue;
    usedPackages.add(m.package_id);
    const dep = readFileSync(unitDepInfo(m,graph.metadata.workspace_root,graph.metadata), 'utf8');
    for (const path of compilerPaths(dep,graph.metadata.workspace_root)) add(path);
    const environment = dep.split('\n').filter((s) => s.startsWith('# env-dep:')).map((s) => { const pair=s.slice(10),at=pair.indexOf('=');return at<0?[pair,null]:[pair.slice(0,at),pair.slice(at+1)]; }).map(normalizeEnv);
    units.push({package:graph.packages.get(m.package_id).name,role,target:m.target.name,kind:m.target.kind,features:m.features,profile:m.profile,environment});
  }
  const builders = [];
  for (const m of scripts) {
    if(!usedPackages.has(m.package_id))continue;
    const role = roleOf(m.out_dir); if (!graph.roles.get(m.package_id).has(role)) continue;
    const pkg = graph.packages.get(m.package_id), source = pkg.targets.find((t) => t.kind.includes('custom-build'));
    if (source) add(source.src_path);
    // The script's stdout: `output` beside `out`, or `run/stdout` in Cargo's new build-dir layout (the web toolchain's).
    const output = readFileSync([resolve(m.out_dir,'../output'), resolve(m.out_dir,'../run/stdout')].find(existsSync) ?? resolve(m.out_dir,'../output'),'utf8'); const environment = [];
    for (const line of output.split('\n')) {
      const changed = /^cargo::?rerun-if-changed=(.*)$/.exec(line);
      // The app's plan/manifest/static watches are inputs of replaceable
      // artifacts. Its generated entry and typed metadata are binary inputs.
      if (changed && pkg.id !== graph.root.id) {
        const path = resolve(dirname(pkg.manifest_path), changed[1]);
        if (!shaderRoots(app).some(root => under(root, path))) add(path, true);
      }
      const variable = /^cargo::?rerun-if-env-changed=(.*)$/.exec(line)?.[1];
      if (variable && !variable.startsWith('EXACT_') && !['OUT_DIR','CARGO_MANIFEST_DIR'].includes(variable)) environment.push(normalizeEnv([variable,env[variable]??null]));
    }
    // A linked native archive is an input too; Rust dep-info cannot name its C/ObjC bytes.
    for (const raw of m.linked_paths) {
      const dir = raw.replace(/^native=/, '');
      if (under(resolve(m.out_dir), resolve(dir))) { for (const file of readdirSync(dir)) if (/\.(a|o|dylib|so|lib|obj|dll)$/.test(file)) add(resolve(dir,file)); }
      else if (under(LIBRARY_CACHE, resolve(dir))) add(resolve(dir)); // a package's library, which the product carries
    }
    builders.push({package:pkg.name,role,cfgs:m.cfgs,environment:orderedBuild(environment),libraries:m.linked_libs,env:m.env.filter(([key])=>!key.startsWith('EXACT_')).map(normalizeEnv)});
  }
  for (const pkg of packages) if(usedPackages.has(pkg.id))add(pkg.manifest_path);
  add(resolve(graph.metadata.workspace_root,'Cargo.toml'));add(resolve(graph.metadata.workspace_root,'Cargo.lock'));add(resolve(app.dir,'.exact/backend-origin'),true); // the bake's backend selection, by bytes or absence
  for (const file of app.manifest?.env?.length ? ['.env.local', '.env'] : []) add(resolve(app.dir, file), true); // its settings' files (LLP 1153 D4), likewise
  // Watch absent art without a missing Cargo input that dirties every build.
  if (app.manifest.game) add(resolve(app.dir, 'art'), true);
  if (app.manifest.game?.render && graph.surface) {
    add(resolve(app.dir, 'render/Cargo.toml'));
    add(resolve(app.dir, 'render/src'), true);
    add(resolve(app.dir, 'render/build.rs'), true);
  }
  if (platform === 'macos' || platform === 'ios') {
    const packageRoot=resolve(ROOT,'host/apple');
    const swiftEnv = {...env, EXACT_APP_COMPOSITION: compat.inputs.store.L === '0' ? 'embedded' : 'updating'}; delete swiftEnv.SDKROOT;
    // Cache SwiftPM's description by manifest, composition and source inventory.
    const tree=(dir)=>readdirSync(dir,{withFileTypes:true}).flatMap((e)=>e.isDirectory()?tree(resolve(dir,e.name)):[resolve(dir,e.name)]).sort();
    const described=resolve(app.target,'apple-swift',`package-${buildHash(canonicalBuild([readFileSync(resolve(packageRoot,'Package.swift'),'utf8'),swiftEnv.EXACT_APP_COMPOSITION,swiftEnv.EXACT_TESTS??null,swiftEnv.EXACT_WIDGET_GALLERY??null,tree(resolve(packageRoot,'Sources')).map((f)=>relative(packageRoot,f))])).slice(0,16)}.json`);
    if(!existsSync(described)) {
      mkdirSync(dirname(described),{recursive:true});
      writeFileSync(`${described}.${process.pid}.tmp`,buildCommand('swift',['package','--package-path',packageRoot,'describe','--type','json'],app,swiftEnv).stdout);
      renameSync(`${described}.${process.pid}.tmp`,described);
      const kept=readdirSync(dirname(described)).filter((f)=>/^package-[0-9a-f]{16}\.json$/.test(f)).map((f)=>resolve(dirname(described),f)).sort((a,b)=>statSync(b).mtimeMs-statSync(a).mtimeMs);
      for(const old of kept.slice(6))rmSync(old,{force:true});
    }
    const swift=JSON.parse(readFileSync(described,'utf8'));
    const pending=[platform==='ios'?'ExactIOS':'ExactMac'],seen=new Set();
    while(pending.length) { const name=pending.pop();if(seen.has(name))continue;seen.add(name);const unit=swift.targets.find((t)=>t.name===name);if(!unit)throw new Error(`Swift package has no ${name}`);
      for(const file of unit.sources??[])add(resolve(packageRoot,unit.path,file));
      for(const resource of unit.resources??[])add(resolve(packageRoot,resource.path));
      if(unit.type==='system-target')add(resolve(packageRoot,unit.path));pending.push(...(unit.target_dependencies??[]));
    }
    add(resolve(packageRoot,'Package.swift'));add(resolve(packageRoot,'webarm/WebArm.swift'));add(resolve(packageRoot,'webarm/FrameBrowser.swift'));add(resolve(packageRoot,'webarm/FrameWindows.swift'));add(resolve(packageRoot,'webarm/FrameCapture.swift'));add(resolve(packageRoot,'webarm/FramePDF.swift'));add(resolve(packageRoot,'webarm/FrameAutomation.swift'));add(resolve(packageRoot,'webarm/FrameFiles.swift'));add(resolve(packageRoot,'webarm/FrameInspector.swift'));add(resolve(packageRoot,'videoarm/VideoArm.swift'));add(resolve(packageRoot,'videoarm/PictureInPicture.swift'));add(resolve(packageRoot,'build.mjs'));add(resolve(packageRoot,'assets.mjs'));add(resolve(packageRoot,'bundle.mjs'));add(resolve(packageRoot,'widgets.mjs'));
    const entries=platform==='ios'&&(app.manifest.widgets?.length||app.manifest.live_activities?.length); if(entries)add(resolve(packageRoot,'widget'),true); if(entries||env.EXACT_WIDGET_GALLERY==='1')add(resolve(packageRoot,'Sources/ExactSnapshot'),true);
  }
  if(platform==='web') {
    for(const path of [...Object.values(webHostFiles('base','rust','gpu',...(gpuModules(app.manifest).length?['gpuModules']:[]))),'scripts/app.mjs','scripts/rust.mjs',...SHELL_SOURCES,'host/web/build.mjs','package.json','bun.lock']) add(resolve(ROOT,path));
    if (existsSync(resolve(app.dir, 'app.ts'))) {
      // The TS producer is a build dependency, outside the runtime Cargo graph.
      // Its canonical API declaration still determines the accepted app module.
      add(resolve(ROOT, 'vendor/ibex/crates/ibex2/src/bindings/storage.d.ts'));
      for (const path of Object.values(webHostFiles('module'))) add(resolve(ROOT, path));
    }
    if (existsSync(resolve(app.dir, 'app.ts')) || /^\s*(?:fs\.|sqlite\.)/m.test(compat.inputs.grantCeiling ?? '')) {
      for (const path of [...Object.values(webHostFiles('storage')),
        'package.json', 'bun.lock', 'node_modules/@sqlite.org/sqlite-wasm/package.json']) add(resolve(ROOT, path));
    }
  }
  const nativeResources = platform === 'macos' ? macResourceInventory(app) : [];
  const assetCatalogApp = ['macos', 'ios'].includes(platform) ? { dir: app.dir, platform: target.includes('-tvos') ? 'tvos' : platform, manifest: { host: app.manifest.host } } : null;
  const appleAssetCatalogs = assetCatalogApp ? appleAssetCatalogInventory(assetCatalogApp, assetCatalogApp.platform) : [];
  if (assetCatalogApp) for (const path of [resolve(app.dir, platform, 'modules'), resolve(app.dir, 'modules/apple')]) {
    if (existsSync(path)) directories.set(nameOf(path), {path, names: readdirSync(path).sort()});
    else absent.set(nameOf(path), path);
  }
  for (const entry of appleAssetCatalogs) add(resolve(app.dir, entry.path));
  // The launch screen's picture: its bytes are the binary's, and packaging draws the ones recorded.
  const launchImage = platform === 'ios' ? launchImageInventory(app) : null;
  if (launchImage) add(resolve(app.dir, launchImage.src));
  const metadata={...(appleAssetCatalogs.length ? {appleAssetCatalogs} : {}),...(launchImage ? {launchImage} : {}),...(nativeResources.length ? {nativeResources} : {}),...nativeManifestMetadata(app.manifest, platform),delivery:compat.delivery,store:compat.inputs.store,keys:compat.inputs.keys};
  const configuration={target,units:orderedBuild([...new Map(units.map(u=>[canonicalBuild(u),u])).values()]),builders:orderedBuild([...new Map(builders.map(u=>[canonicalBuild(u),u])).values()]),rustc:buildCommand('rustc',['-vV'],app,env).stdout,flags:{...Object.fromEntries(['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','MACOSX_DEPLOYMENT_TARGET','IPHONEOS_DEPLOYMENT_TARGET'].map((k)=>[k,env[k]??null])),...(env.EXACT_WEB_LINK?{EXACT_WEB_LINK:env.EXACT_WEB_LINK}:{}),...(env.EXACT_WEB_SIZE?{EXACT_WEB_SIZE:env.EXACT_WEB_SIZE}:{})}};
  const files=[...inputs.values()].sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0);
  const fingerprint={files:files.map(({name,sha256})=>({name,sha256})),absent:[...absent.keys()].sort(),configuration,metadata};
  const products=roots.flatMap((r)=>messages.filter((m)=>m.reason==='compiler-artifact'&&m.package_id===r.package&&m.target.name===r.name).flatMap((m)=>m.filenames)).filter((p)=>!p.endsWith('.d')&&!p.endsWith('.dSYM')).map((path)=>prepared.get(path)??path).map((path)=>{const info=statSync(path);return {path,bytes:info.size,sha256:hashes.of(path,info)};});
  hashes.save();
  return {version:1,...(env.EXACT_RUST_BUNDLE?{rust:resolve(baked,'rust')}:{}),...(baked!==rootOutput?{linkedPlan:baked}:{}),...bakedSettings(baked,app.dir),trust:env.EXACT_UPDATE_TRUST??'development',compat,graph:bundleGraph,binary:{...(assetCatalogApp ? {assetCatalogApp} : {}),...(nativeResources.length ? {nativeResourceApp:{dir:app.dir,manifest:app.manifest}} : {}),sha256:buildHash(canonicalBuild(fingerprint)),...fingerprint,inputs:files,directories:[...directories.values()],missing:[...absent.values()]},products};
}

/** Ephemeral output ownership shared by Apple builders and Cargo bakes.
 * Never steal a stale claim: the operator must verify its PID first. */
export function claimBuildOutput(app, path) {
  mkdirSync(resolve(path, '..'), { recursive: true });
  try { writeFileSync(path, JSON.stringify({ pid: process.pid, app: app.id, source: realpathSync(app.dir), started: new Date().toISOString() }) + '\n', { flag: 'wx' }); }
  catch (error) {
    if (error.code !== 'EEXIST') throw error;
    const held = readFileSync(path, 'utf8');
    // A killed build never runs its exit handler: a claim whose process is
    // gone, or whose pid now names a process started after it, is stale.
    if (!claimLive(held) && readFileSync(path, 'utf8') === held) { rmSync(path, { force: true }); return claimBuildOutput(app, path); }
    throw new Error(`Apple build busy for ${app.id}: ${held.trim()} (${path}); remove this lock explicitly only after verifying the owner is no longer running`);
  }
  let held = true;
  const release = () => { if (held) { held = false; rmSync(path); process.removeListener('exit', release); } };
  process.once('exit', release);
  return release;
}

/** A claim on an output several apps share and each holds briefly (the Swift
 * scratch): wait for a live owner instead of refusing. `waiting` hears the
 * owner's app once. A dead owner's claim is taken, as claimBuildOutput takes it. */
export function awaitBuildOutput(app, path, waiting) {
  const pause = new Int32Array(new SharedArrayBuffer(4));
  for (let told = false; ; Atomics.wait(pause, 0, 0, 200)) {
    try { return claimBuildOutput(app, path); }
    catch (error) {
      if (!/^Apple build busy for /.test(error.message)) throw error;
      // The message carries the owner's claim: its app, not the one waiting.
      if (!told) { told = true; waiting?.(/"app":"([^"]+)"/.exec(error.message)?.[1] ?? 'another app'); }
    }
  }
}

export function claimLive(text) {
  let claim;
  try { claim = JSON.parse(text); } catch { return true; } // unreadable: never guess
  if (!Number.isSafeInteger(claim.pid) || claim.pid <= 0) return true;
  try { process.kill(claim.pid, 0); } catch (error) { if (error.code === 'ESRCH') return false; } // gone (EPERM: alive, another user's)
  const ps = spawnSync('ps', ['-o', 'lstart=', '-p', String(claim.pid)], { encoding: 'utf8', env: { ...process.env, LC_ALL: 'C' } }), started = Date.parse(ps.stdout?.trim() ?? '');
  if (ps.status !== 0 || !Number.isFinite(started)) return true; // running, and `ps` cannot say since when: never guess
  return !(started > Date.parse(claim.started) + 1000); // `lstart` has whole seconds: a holder started after its claim is a reused pid
}

/** Cargo library filenames use the selected target name, with hyphens
 * normalized to underscores, even when the package has another name. */
export const cargoLibraryTarget = (pkg) => pkg.targets.find(t => t.kind.some(k => ['cdylib','staticlib','rlib','lib'].includes(k)));
export const appleCargoClaims = (app, target, units) => [...new Set(units.map(unit =>
  resolve(app.target, '.apple-cargo-locks', target, `lib${unit.name.replace(/-/g, '_')}.lock`)))].sort();

// Only explicitly developmental gpu-dev hosts may be reused across GPU edits.
export const bindGpuProduct = (profile, trust) => profile !== 'gpu-dev' || trust !== 'development';
export function bakeSelection(graph, part) {
  if (part && !['gpu','host'].includes(part)) throw new Error(`unknown bake part: ${part}`);
  const gpu = [graph.surface, ...(graph.modules ?? [])];
  return (part === 'gpu' ? gpu : part === 'host' ? [graph.root] : [...gpu,graph.root]).filter(Boolean);
}

/** What ships to an Apple device or an Android phone is optimized as one module: fat LTO
 * makes the stripped binary smaller than thin — 4.8% on Apple, no slower to boot, for 16 s
 * of a production build (LLP 1036.000 §8); 6.4% for Caltrain's Android library (16.6 →
 * 15.6 MB), whose release build takes about a minute longer. Said on the command line and
 * not in `[profile.release]`, which every developer tool builds with. Cargo's fingerprints
 * keep thin and fat apart: a target directory used both ways (a macOS and a Linux bake on a
 * Mac) recompiles, and never reuses the other's artifacts. */
export function wholeProgram(kind, profile) {
  return ['apple','ios','macos','tvos','android'].includes(kind) && profile === 'release' ? ['--config','profile.release.lto="fat"'] : [];
}

/** One actual target build, including the optional GPU artifact. Consumers
 * classify its completed receipt; compatibility is never recomputed in JS.
 * `check` runs `cargo check`: the build scripts (the bake) and the receipt,
 * with no linked product (delivery's web bake, host/web/build.mjs --bake).
 * `options.kind` overrides the crate kind the platform implies. */
/** What an app's bake watches besides its sources (its asset roots that exist, whether a backend origin is chosen, which of the settings files `.env.local` and `.env` exist); unset, a missing one stays an input where the app holds its build output, and every build reruns the script and relinks (bake/src/receipt/watch.rs). Every baking build passes it. `app`: its `dir`, `manifest`. */
export const bakeWatchEnv = app => ({ EXACT_ASSET_ROOTS: ['assets', 'deck', ...(app.manifest.game ? [] : ['gpu/shaders'])].filter(root => (root === 'assets' && app.manifest.game && (app.manifest.game.assets === true || existsSync(resolve(app.dir, 'art')))) || existsSync(resolve(app.dir, root))).join(','), EXACT_BACKEND_SELECTION: existsSync(resolve(app.dir, '.exact/backend-origin')) ? 'present' : 'absent', EXACT_ENV_FILES: ['.env.local', '.env'].filter(file => existsSync(resolve(app.dir, file))).join(',') });
/** The dev loop's bake driver (host/web/dev.mjs): an app's `<app>-dev` bin (`devBin`), else `exact-dev`, on host-dev (seconds after an edit, not release's minute), with `bakeWatchEnv` (nothing rebuilt when nothing changed): its cargo args before `--`, cwd and env over `env`. */
export const devBinRun = (app, devBin, env) => ({ args: ['run', '-q', ...(devBin ? injectedProfiles(app) : []), '--profile', HOST_DEV, '-p', devBin ? app.crate('web') : 'exact-web', '--bin', devBin ?? 'exact-dev'], cwd: devBin ? app.workspace : ROOT, env: { ...env, ...bakeWatchEnv(app) } });
export function buildBake(app, platform, target, options = {}) {
  // `kind` names another crate for the platform's cohort: Android's Canvas host bakes as Linux from `<app>-android`.
  const kind=options.kind??platformCrateKind(app.dir,platform), apple=['apple','ios','macos','tvos'].includes(kind);
  let env=withHostDeployment({...process.env,...options.env});env.CARGO_TARGET_DIR=app.target;env.EXACT_BAKE_OUTPUT=options.output??bakeOutput(app,env);
  const watch = target.includes('apple-watchos');
  if(platform==='web'||watch)env=webToolchainEnv(env);
  if(options.analysis && env.EXACT_UPDATE_TRUST==='production')env.EXACT_BAKE_ANALYSIS='1';else delete env.EXACT_BAKE_ANALYSIS;
  mkdirSync(env.EXACT_BAKE_OUTPUT,{recursive:true});
  if(platform!=='web')prepareLibrary(app.dir,target); // a package's library, fetched and checked before Cargo links it (LLP 1154 D3)
  const rustBundle=prepareRustBundle(app,platform,target,env);
  if(rustBundle)env.EXACT_RUST_BUNDLE=rustBundle;
  const native=platform==='web'?null:nativeSource(app,env); // what the binary compiles its Rust from (Swappable::with_native)
  if(native)env.EXACT_NATIVE_SOURCE=native;else delete env.EXACT_NATIVE_SOURCE;
  const graph=buildGraph(app,target,kind,env,app.hasGpu),messages=[],roots=[];
  // Production-profile game bakes (web, Apple, release Linux, deploy) hold the
  // logic to determinism lints; the gpu-dev loop runs them in its preceding check.
  if(app.manifest.game&&(options.profile??'')!=='gpu-dev')lintGame(app.dir,app.manifest.game,{env});
  const gpuPackage=(pkg)=>pkg.id===graph.surface?.id||graph.modules.some((m)=>m.id===pkg.id), moduleProducts={}, preparedProducts=new Map();
  delete env.EXACT_GPU_PRODUCT; delete env.EXACT_GPU_MODULES; delete env.EXACT_BAKE_INPUTS;
  if (options.part && !bindGpuProduct(options.profile, env.EXACT_UPDATE_TRUST) && graph.surface) {
    const extension = target.includes('apple') ? 'dylib' : target.includes('windows') ? 'dll' : 'so';
    env.EXACT_GPU_DEVELOPMENT = `lib${cargoLibraryTarget(graph.surface).name.replaceAll('-','_')}.${extension}`;
  }
  else delete env.EXACT_GPU_DEVELOPMENT;
  if (options.part && (kind !== 'linux' || bindGpuProduct(options.profile, env.EXACT_UPDATE_TRUST))) throw new Error('partial bakes require development gpu-dev Linux');
  const selected = bakeSelection(graph, options.part).map(pkg => {
    const unit = pkg.id === graph.root.id && ['linux','windows'].includes(kind) ? pkg.targets.find(t => t.kind.includes('bin')) : cargoLibraryTarget(pkg);
    if (!unit) throw new Error(`Cargo has no buildable target for ${pkg.name}`);
    return {pkg, unit};
  });
  const releases = [];
  try {
    if (apple) for (const path of appleCargoClaims(app, target, selected.map(({unit}) => unit))) {
      releases.push(claimBuildOutput(app, path));
    }
  for(const {pkg,unit} of selected) {
    const profile = (gpuPackage(pkg) ? options.gpuProfile : null) ?? options.profile ?? (platform === 'web' ? 'web' : 'release');
    // The GPU bake can create the first asset directory (for a typed level).
    Object.assign(env, bakeWatchEnv(app));
    // The app's own web artifact builds std for size; a GPU crate keeps the toolchain's std.
    const sized=platform==='web'&&!gpuPackage(pkg);
    // An Apple app's crate is an rlib to Cargo, so `--workspace` builds type-check it without
    // bundling its whole dependency graph into a 700 MB archive nobody reads. The archive the
    // app links is asked for here, where it is built to be launched.
    const archive=apple&&pkg.id===graph.root.id;
    const whole=wholeProgram(kind,options.profile??'release');
    const args=[archive?'rustc':options.check?'check':'build',...(archive?['--crate-type','staticlib']:[]),...cargoReproducibilityFlags(app),...injectedProfiles(app),...whole,...(sized||watch?WEB_STD:[]),...(target==='wasm32-unknown-unknown'?wasmRemapFlags(app,sized?WEB_TOOLCHAIN:null):[]),'-p',pkg.name,'--target',target,'--profile',profile,...(['linux','windows'].includes(kind)&&pkg.id===graph.root.id?['--bin',unit.name]:['--lib']),...(gpuPackage(pkg)?['--config',`profile.${profile}.strip=false`]:[]),'--message-format=json-render-diagnostics'];
    // Refresh Rust bakers through an rlib before they read a coordinated
    // Rust/manifest edit. Generated inputs also predate the first archive.
    if (archive && options.link && !env.EXACT_RUST_BUNDLE && linkable(app,pkg)) {
      env.EXACT_BAKED = bakeOutside(app,platform,target,env,dir => {
        buildCommand('cargo',['build',...args.slice(3)],app,{...env,EXACT_BAKED:dir},'inherit');
      });
    } else delete env.EXACT_BAKED;
    const recordInputs = pkg.id === graph.root.id ? cargoBakeInputs(pkg,target,profile,env) : () => {};
    const result=buildCommand('cargo',args,app,env,'inherit');
    const output=result.stdout.split('\n').filter(Boolean).map((line)=>JSON.parse(line));messages.push(...output);roots.push({package:pkg.id,name:unit.name});
    recordInputs(output);
    if (archive && env.EXACT_BAKED) recordRustBake(env);
    if (gpuPackage(pkg) && platform !== 'web') {
      const product = output.filter(m => m.reason === 'compiler-artifact' && m.package_id === pkg.id)
        .flatMap(m => m.filenames).find(path => /\.(so|dylib|dll)$/.test(path));
      if (!product) throw new Error(`GPU product missing for ${pkg.name}`);
      // A host may prepare (sign) a copy; that copy is the product from here on.
      // Cargo's messages keep Cargo's file: its rustc dep-info is found beside it.
      const prepared = options.prepareGpu?.(product) ?? product;
      if (prepared !== product) preparedProducts.set(product, prepared);
      // Each module's signed digest is bound beside the primary's (LLP 1009 D6).
      const module = gpuModules(app.manifest).find(({name}) => app.crate(`gpu-${name}`) === pkg.name);
      if (module) { moduleProducts[module.name] = prepared; env.EXACT_GPU_MODULES = JSON.stringify(moduleProducts); }
      else if (!options.part || bindGpuProduct(options.profile, env.EXACT_UPDATE_TRUST)) env.EXACT_GPU_PRODUCT = prepared;
    }
  }
  if (options.part === 'gpu') {
    // The proof completes the independent input/product receipt. There is no
    // host bake output to classify when only the surface graph was selected.
    return {products:messages.filter(m=>m.reason==='compiler-artifact' && bakeSelection(graph,'gpu').some(pkg=>pkg.id===m.package_id)).flatMap(m=>m.filenames).map(f=>preparedProducts.get(f)??f)};
  }
  const receipt=completeBuild(app,platform,target,graph,messages,roots,env,preparedProducts);
  writeFileSync(resolve(env.EXACT_BAKE_OUTPUT,`${platform}-${target}.build.json`),JSON.stringify(receipt)+'\n');
  options.capture?.(receipt);
  return receipt;
  } finally { for (const release of releases.reverse()) release(); }
}

/** Read completed binary-producing receipts only; absent platforms remain
 * unbuilt rather than acquiring a second, guessed compatibility identity. */
export function readBuilds(app, env = process.env) {
  const directory=bakeOutput(app,env);if(!existsSync(directory))return [];
  return readdirSync(directory).filter((name)=>name.endsWith('.build.json')).map((name)=>JSON.parse(readFileSync(resolve(directory,name),'utf8'))).filter((r)=>r.version===1&&r.compat?.inputs?.app===app.id);
}

/** Frozen capabilities and the binary input identity; small enough to bind
 * inside the signed stream envelope. Absolute source paths stay private. */
export function cohortReceipt(build) {
  if (build?.version !== 1 || build.graph?.version !== 1 || !build.binary?.sha256) throw new Error('missing completed bake graph and binary receipt');
  return {version:1,compat:{id:build.compat.id,inputs:build.compat.inputs,target:build.compat.target},sources:build.graph.sources,surfaceCalls:build.graph.surfaceCalls??{},binary:build.binary.sha256};
}

/** The shared dev/deploy comparison (LLP 1030 D3). A new binary and a safe
 * bundle are independent results. Compatibility-id equality decides neither. */
export function classifyArtifacts(candidate, cohort, signingKey = null) {
  const missing=[];
  if (!candidate?.graph?.artifacts || !cohort?.sources || !cohort?.compat?.inputs) return {binary:true,bundle:false,missing:['bake graph: the installed cohort has no authenticated capability receipt'],warnings:[]};
  const have=cohort.compat.inputs;
  if(candidate.trust==='production'&&have.store?.L!=='0') {
    if(!signingKey||have.keys?.[signingKey.id]!==signingKey.public)missing.push(`envelope: signing key ${signingKey?.id??'(not selected)'} is not carried by this cohort`);
  }
  for(const artifact of candidate.graph.artifacts) for(const [key,need] of Object.entries(artifact.requires)) {
    const fail=(detail)=>missing.push(`${artifact.name}: ${key}${detail}`);
    if(key==='sources') {
      for(const [name,shape] of Object.entries(need)) if(canonicalBuild(cohort.sources[name]??null)!==canonicalBuild(shape)) fail(`.${name} (missing source or different parameter/result shape)`);
    } else if(key==='surfaceCalls') {
      for(const [name,arities] of Object.entries(need)) for(const arity of arities) if(!cohort.surfaceCalls?.[name]?.includes(arity)) fail(`.${name}/${arity} (not demanded by the installed plan)`);
    } else if(key==='gpuSurfaces') {
      for(const surface of need) if(!have.gpuSurfaces?.some(s=>canonicalBuild(s)===canonicalBuild(surface))) fail(`.${surface.name} (interface ${surface.interface})`);
    } else if(key==='executors'||key==='hatches') { for(const item of need) if(!have[key]?.includes(item)) fail(key==='hatches'?`.${item} (handled by this platform's module, not by the installed one)`:`.${item}`); // hatches: LLP 1075.003.000.001 §4.3, a cohort from before the capability handles none
    } else if(key==='typescriptRuntime') { if(!((have[key]??0)>=need)) fail(` (requires a prelude of runtime ${need} or later; this cohort's is ${have[key]??0})`); /* LLP 1108 D6 R2: an older prelude ignores exactBodyFrom */ } else if(key==='grantCeiling') {
      const grants=new Set((have.grantCeiling??'').split('\n').filter(Boolean));
      if(need===null||have.grantCeiling===null) fail(' (unknown baked grants)');
      else for(const grant of need.split('\n').filter(Boolean)) if(!grants.has(grant)) fail(` (${grant})`);
    } else if(canonicalBuild(have[key]??null)!==canonicalBuild(need)) fail(` (requires ${canonicalBuild(need)})`);
  }
  const changed=candidate.binary.sha256!==cohort.binary || (candidate.pendingInputs?.length ?? 0)>0;
  const warnings=[];
  if(changed&&!candidate.graph.artifacts.some(a=>a.name==='rust/app.module.json')&&canonicalBuild(candidate.compat.inputs.dataCrate)!==canonicalBuild(have.dataCrate)&&Object.keys(candidate.graph.sources).some(n=>!runnerOwnedSource(n)&&cohort.sources[n])) warnings.push(`same name, same shape, new native code: cohort ${cohort.compat.id} will run the old code`);
  if(changed&&Object.keys(candidate.graph.surfaceCalls??{}).length) warnings.push(`same surface name and arity: cohort ${cohort.compat.id} retains its old GPU implementation`);
  return {binary:changed,bundle:missing.length===0&&have.store?.L!=='0',missing:have.store?.L==='0'?['store.L=0: this binary has no bundle carrier']:missing,warnings};
}

/** Overlay the resident compiler's plan graph with the complete static
 * candidate captured by dev. Reflected shader interfaces are required. */
export function developmentCandidate(build, plan, assets, surfaces) {
  const node=build.graph.artifacts.find(a=>a.name==='app.plan');
  if(node?.sha256!==plan.sha256||node.bytes!==plan.bytes) throw new Error('the candidate plan and its compiler graph differ');
  return {...build,graph:{...build.graph,artifacts:[node,...assets.map(asset=>{
    const stem=asset.name.startsWith('shaders/')&&asset.name.endsWith('.wgsl')?asset.name.slice(8,-5):null;
    if(stem!==null&&!surfaces.has(stem)) throw new Error(`shader ${stem} has no reflected interface`);
    return {...asset,kind:'bundle',requires:stem===null?{}:{gpuSurfaces:[{name:stem,interface:surfaces.get(stem)}]}};
  })]}};
}

/** Source staleness is provisional until the next actual bake. These paths
 * came from the previous compiler receipt, including absent watched inputs. */
export function pendingBuildInputs(build) {
  const changed=[];
  try {if(build.binary.assetCatalogApp && canonicalBuild(appleAssetCatalogInventory(build.binary.assetCatalogApp, build.binary.assetCatalogApp.platform)) !== canonicalBuild(build.binary.metadata.appleAssetCatalogs ?? [])) changed.push('Apple asset catalogs');} catch {changed.push('Apple asset catalogs');}
  try {if(build.binary.nativeResourceApp && canonicalBuild(macResourceInventory(build.binary.nativeResourceApp)) !== canonicalBuild(build.binary.metadata.nativeResources)) changed.push('host.macos.resources');} catch {changed.push('host.macos.resources');}
  for(const file of build.binary.inputs) {
    try {if(!statSync(file.path).isFile()||buildHash(readFileSync(file.path))!==file.sha256)changed.push(file.name);}
    catch {changed.push(file.name);}
  }
  for(const path of build.binary.missing)if(existsSync(path))changed.push(path);
  for(const {name} of build.settings?settingChanges(build.settings.env,process.env,build.settings.dir):[])changed.push(`import.meta.env.${name}`); // a setting the module inlined, as its sources give it now (LLP 1153 D4)
  for(const {path,names} of build.binary.directories) {
    try {if(!statSync(path).isDirectory()||canonicalBuild(readdirSync(path).sort())!==canonicalBuild(names))changed.push(path);}
    catch {changed.push(path);}
  }
  return changed;
}

/** Diagnostics that edit inputs run on the deploy snapshot's closed source
 * graph. Build outputs and child process state belong to this invocation.
 * `warm` keeps one Cargo target per checkout and app between runs: the capture
 * and run directories sit at stable paths (Cargo keys path crates by path),
 * staged files keep the live files' mtimes (so only real edits rebuild), and
 * a lock refuses a concurrent run on the same directories. */
export async function withAppFixture(app, use, { warm = false } = {}) {
  const key = createHash('sha256').update(`${realpathSync(ROOT)}\0${app.dir}`).digest('hex').slice(0, 16);
  if (!warm) return fixtureRun(app, use, realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-diagnostic-'))), null);
  const run = resolve(realpathSync(tmpdir()), `exact-diagnostic-${key}`);
  mkdirSync(run, { recursive: true });
  // An OS lock a helper process holds: released however this process ends,
  // never mistaken for a reused process id.
  const { filesystemLock } = await import('./filesystem.mjs');
  let entered = false;
  try {
    return await filesystemLock(run, 'held/.lock', () => { entered = true; return fixtureRun(app, use, run, key); });
  } catch (error) {
    if (!entered && error.message.includes('locked by another')) throw new Error(`another diagnostic run is using ${run}; wait for it`);
    throw error;
  }
}
async function fixtureRun(app, use, run, key) {
  const started = Date.now(), warm = key !== null;
  const { snapshotOf, materializeSnapshot, disposeSnapshot } = await import('./deploy.mjs');
  let snapshot;
  try {
    snapshot = snapshotOf(app, { dirty: true, ...(warm ? { captureRoot: resolve(realpathSync(tmpdir()), `exact-capture-${key}`) } : {}) }, ROOT);
    // materializeSnapshot honors the caller's target override. Scope this
    // synchronous call to the diagnostic's private target, then restore it.
    const previousTarget = process.env.CARGO_TARGET_DIR;
    let fixture;
    try {
      process.env.CARGO_TARGET_DIR = resolve(run, 'target');
      fixture = materializeSnapshot(snapshot, run, app);
    } finally {
      if (previousTarget === undefined) delete process.env.CARGO_TARGET_DIR;
      else process.env.CARGO_TARGET_DIR = previousTarget;
    }
    // Hermes (js/build.rs) is provisioned beside the checkout, in `ibex`; a
    // capture has none there, so a TypeScript app's bake failed in it. Link
    // the live checkout's, as that layout expects.
    const ibex = resolve(dirname(ROOT), 'ibex'), besideCapture = resolve(dirname(fixture.exactRoot), 'ibex');
    if (existsSync(ibex) && !existsSync(besideCapture)) symlinkSync(ibex, besideCapture);
    const env = { ...process.env, EXACT_APP_DIR: fixture.app.dir, EXACT2: fixture.exactRoot,
      CARGO_TARGET_DIR: resolve(run, 'target'), EXACT_WEB_DIST: resolve(fixture.exactRoot, 'host/web/dist'),
      EXACT_BAKE_OUTPUT: resolve(run, 'bake'), EXACT_UPDATE_DIR: resolve(run, 'update'),
      EXACT_DIAGNOSTIC_ROOT: fixture.exactRoot, GIT_CEILING_DIRECTORIES: dirname(fixture.sourceRoot),
      GIT_DISCOVERY_ACROSS_FILESYSTEM: '0', EXACT_DIAGNOSTIC_SOURCE: JSON.stringify({
        app: { name: app.name, id: app.id, dir: app.dir }, snapshot: snapshot.id, started,
        sources: snapshot.sources.map(({ repo, roles, commit, workingSha256 }) => ({ repo, roles, commit, workingSha256 })),
      }) };
    for (const name of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE',
      'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'EXACT_DEPLOY_CAPSULE',
      'EXACT_UPDATE_RECEIPT', 'EXACT_UPDATE_GENESIS', 'EXACT_UPDATE_ORIGIN', 'EXACT_GPU_DYLIB', 'EXACT_RUST_BUNDLE', 'EXACT_NATIVE_SOURCE',
      'EXACT_DEV_PLAN', 'EXACT_PLAN', 'EXACT_ASSETS']) delete env[name];
    const barrier = resolve(fixture.sourceRoot, '.git');
    if (readFileSync(barrier, 'utf8') !== 'exact deploy source boundary\n') throw new Error('unexpected diagnostic Git boundary');
    rmSync(barrier);
    const git = (args) => {
      const result = spawnSync('git', ['-c', 'gc.auto=0', '-c', 'maintenance.auto=false', '-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false',
        '-c', 'user.name=Exact diagnostic', '-c', 'user.email=diagnostic@exact.invalid', ...args],
      { cwd: fixture.sourceRoot, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
      if (result.status !== 0) throw new Error(`diagnostic git ${args[0]}: ${result.stderr || result.error?.message}`);
      return result.stdout;
    };
    // The capture's installed node_modules is an output, as in the live checkout.
    const installed = [...new Set([fixture.exactRoot, fixture.app.workspace])].map((root) => `:(exclude)${relative(fixture.sourceRoot, resolve(root, 'node_modules'))}`);
    for (const args of [['init', '-q', '-b', 'main'], ['add', '-f', '-A', '--', '.', ...installed], ['commit', '-qm', 'Captured diagnostic source']]) git(args);
    const manifest = readManifest(fixture.app.dir, app.name);
    return await use({ ...fixture, run, env, git, snapshot, app: { ...app, ...fixture.app,
      target: env.CARGO_TARGET_DIR, manifest, id: manifest.app.id, displayName: manifest.app.name,
      origin: manifest.app.origin ?? null } });
  } finally {
    try { if (snapshot) disposeSnapshot(snapshot); }
    finally {
      if (!warm) removePrivateTree(run);
      else for (const entry of readdirSync(run)) if (entry !== 'target' && entry !== 'held') removePrivateTree(resolve(run, entry));
    }
  }
}
