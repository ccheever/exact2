// The platforms an app outside this repository ships, and the native workspace it builds them in (LLP 1153 D1).
//
// The manifest is the declaration: `host.<platform>` names a platform, with `deploy.store.<platform>` for the three
// that hold an update store. A native verb names its platform the first time it runs (`bringPlatform`), and the crates
// a build needs are generated under the app's `.exact/shells/`, as a game's bake writes `.shells/` (game/app/shells.mjs):
// nothing Rust enters the author's tree. An app that writes its own `Cargo.toml` is an authored workspace, which
// `resolveApp` takes instead (scripts/cargo-check.mjs `authoredWorkspace`).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargoEnvironment, pathFrom, patchLines } from './app.mjs';
import { authoredWorkspace, shellsDir } from './cargo-check.mjs';
import { APPLE_BUILD_SCRIPT, appleBuildScript } from './linked-plan.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** The platforms a manifest can name, each with the crate that serves it. */
export const PLATFORMS = { ios: 'apple', macos: 'apple', tvos: 'apple', linux: 'linux', android: 'android', windows: 'windows' };
/** The platforms that hold an update store, so a stream (scripts/deploy.mjs), and Windows, whose host refuses every
 * store level but "0" (it has no update store yet, LLP 1115.004 D15) while an absent key reads as "A". */
const STORED = ['ios', 'macos', 'linux', 'windows'];

/** A verb's platform (`ios`, `mac`, `linux`, `android`, `windows`), or a drive's host (`test ios`); null for the web and
 * every other verb. */
export function platformOf(verb, host = 'web') {
  const word = ['test', 'agent', 'verify'].includes(verb) ? host : verb;
  return { ios: 'ios', mac: 'macos', macos: 'macos', linux: 'linux', android: 'android', windows: 'windows' }[word] ?? null;
}

/** What `host.<platform>` holds when a verb names the platform: the scaffold's defaults for the two Apple platforms,
 * Shelf's for Windows (ink's composition swap chain, with Mica, LLP 1149 D12, and a desktop window's size), nothing
 * for the others (each key may be empty, scripts/app.schema.json). */
const hostDefaults = (platform) => ({
  ios: { minimumOS: '17.0', deviceFamily: ['iphone', 'ipad'] },
  macos: { minimumOS: '14.0', window: { width: 900, height: 700 } },
  windows: { painter: 'ink', window: { width: 1024, height: 720, minWidth: 360, minHeight: 500 } },
}[platform] ?? {});

const readManifest = (dir) => JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
const writeManifest = (dir, manifest) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify(manifest, null, 2) + '\n');

/** Whether the manifest names the platform. */
export const namesPlatform = (manifest, platform) => platform in (manifest.host ?? {});

/** Name `platform` in the app's manifest: `host.<platform>`, and its store for one that has a stream. Returns the
 * keys written, none when it was named already. */
export function namePlatform(dir, platform) {
  if (!(platform in PLATFORMS)) throw new Error(`${platform} is not a platform; the platforms are ${Object.keys(PLATFORMS).join(', ')}`);
  const manifest = readManifest(dir), wrote = [];
  manifest.host ??= {};
  if (!(platform in manifest.host)) { manifest.host[platform] = hostDefaults(platform); wrote.push(`host.${platform}`); }
  if (STORED.includes(platform)) {
    manifest.deploy ??= {};
    manifest.deploy.store ??= {};
    if (!(platform in manifest.deploy.store)) { manifest.deploy.store[platform] = '0'; wrote.push(`deploy.store.${platform}`); }
  }
  if (wrote.length) writeManifest(dir, manifest);
  return wrote;
}

/** Stop naming `platform`: its `host`, `deploy.store` and `deploy.binaries` keys go. Returns the keys removed. */
export function withoutPlatformKeys(dir, platform) {
  if (platform === 'web') throw new Error('update --without web: the web host is the dev loop\'s and the agent\'s (`web`, `test web`, `agent web`), so it stays');
  if (!(platform in PLATFORMS)) throw new Error(`update --without ${platform}: the platforms are ${Object.keys(PLATFORMS).join(', ')}`);
  const manifest = readManifest(dir), removed = [];
  for (const [key, table] of [['host', manifest.host], ['deploy.store', manifest.deploy?.store], ['deploy.binaries', manifest.deploy?.binaries]]) {
    if (table && platform in table) { delete table[platform]; removed.push(`${key}.${platform}`); }
  }
  if (removed.length) writeManifest(dir, manifest);
  return removed;
}

/** The crates the shells hold for a manifest: `web/` always (delivery's bake and the resident loop a native window
 * live-reloads from), and one per crate the named platforms share. Android has two: `android/`, the library the
 * Canvas reader's APK loads, and `linux/`, the Linux host built for Android, which `--headless` drives under adb. */
export function shellCrates(manifest) {
  return ['web', ...new Set(Object.keys(manifest.host ?? {}).filter(p => p in PLATFORMS).flatMap(p => p === 'android' ? ['linux', 'android'] : [PLATFORMS[p]]))];
}

/** A host crate's files, keyed by path under the crate's directory. `dep(path)` is a dependency's path from there and
 * `app` the app directory's, as Rust strings' content. */
function crateFiles(kind, { name, title, dep, app }) {
  const header = (target) => `[package]
name = "${name}-${kind}"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false
build = "build.rs"

${target}

[dependencies]
`;
  const bake = (platform) => `fn main() {
    exact_js_bake::build(std::path::Path::new(${JSON.stringify(app)}), ${JSON.stringify(platform)}).unwrap_or_else(|error| {
        // What the bake found (tsc's and the compiler's diagnostics), as it is: no panic around it.
        eprintln!("{error}");
        std::process::exit(1);
    });
}
`;
  const module = (web) => web
    ? `type ExactEmbeddedData = exact_js_web::Module;
fn embedded_data() -> ExactEmbeddedData {
    exact_js_web::Module::new(APP, GRANTS, REVISION)
        .with_canvas_surfaces(CANVAS_SURFACES)
        .placed(TYPESCRIPT_PLACEMENT)
}
`
    : `type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(APP_BYTECODE, APP, GRANTS)
        .with_canvas_surfaces(CANVAS_SURFACES)
        .placed(TYPESCRIPT_PLACEMENT)
}
`;
  const includes = (plan) => `include!(concat!(env!("OUT_DIR"), "/module.rs"));
${plan ? `const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
` : ''}`;
  const build = `[build-dependencies]
exact-js-bake = ${dep('js/bake')}
`;
  if (kind === 'web') return {
    'Cargo.toml': `${header('[lib]\ncrate-type = ["cdylib", "rlib"]')}exact-logic = ${dep('logic')}
exact-web = ${dep('host/web')}
exact-web-capabilities = ${dep('host/web-capabilities')}
exact-js-web = ${dep('js/web')}

${build}`,
    'build.rs': bake('web'),
    'src/lib.rs': `//! ${title} on the web: the Contract UI and the TypeScript data module.

${includes(true)}
${module(true)}include!(concat!(env!("OUT_DIR"), "/logic.rs"));
exact_web::host!(
    AppData,
    PLAN,
    COMPAT,
    app_data,
    [
        include_bytes!(concat!(env!("OUT_DIR"), "/app.module.json")) as &[u8],
        include_bytes!(concat!(env!("OUT_DIR"), "/app.js")) as &[u8]
    ]
);
`,
  };
  if (kind === 'apple') return {
    'Cargo.toml': `${header('[lib]\ncrate-type = ["rlib"]')}exact-logic = ${dep('logic')}
exact-apple = ${dep('host/apple')}
exact-apple-update = ${dep('host/apple-update')}
exact-js = ${dep('js')}

${build}`,
    'build.rs': appleBuildScript(app),
    'src/lib.rs': `//! ${title} on Apple: the Contract UI and the TypeScript data module.

${includes(false)}
${module(false)}include!(concat!(env!("OUT_DIR"), "/logic.rs"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
// The host app.json's deploy.store names (with an update store, signed updates), and the plan it boots.
include!(concat!(env!("OUT_DIR"), "/host.rs"));
`,
  };
  if (kind === 'linux') return {
    'Cargo.toml': `${header(`[[bin]]\nname = "${name}-linux"\npath = "src/main.rs"`)}exact-logic = ${dep('logic')}
exact-linux = ${dep('host/linux')}
exact-js = ${dep('js')}

${build}`,
    'build.rs': bake('linux'),
    'src/main.rs': `//! ${title} on Linux (and Android): the Contract UI and the TypeScript data module.

${includes(true)}
${module(false)}include!(concat!(env!("OUT_DIR"), "/logic.rs"));
// The packages' Linux implementations the build composes (exact_packages()).
include!(concat!(env!("OUT_DIR"), "/packages.rs"));
fn main() {
    let composed = exact_linux::Composed { packages: exact_packages(), ..Default::default() };
    std::process::exit(exact_linux::run_composed::<AppData>(PLAN, COMPAT, composed));
}
`,
  };
  // The library the Canvas reader loads (LLP 1107, host/android/README.md), in Shelf's shape: empty off Android.
  if (kind === 'android') return {
    'Cargo.toml': `${header('[lib]\ncrate-type = ["cdylib"]\ntest = false\nbench = false').replace(/\n\[dependencies\]\n$/, '')}
[target.'cfg(target_os = "android")'.dependencies]
exact-logic = ${dep('logic')}
exact-linux = ${dep('host/linux')}
exact-js = ${dep('js')}

${build}`,
    'build.rs': `fn main() {
    exact_js_bake::build_android(std::path::Path::new(${JSON.stringify(app)})).unwrap_or_else(|error| {
        // What the bake found (tsc's and the compiler's diagnostics), as it is: no panic around it.
        eprintln!("{error}");
        std::process::exit(1);
    });
}
`,
    'src/lib.rs': `//! ${title} on Android: the Contract UI and the TypeScript data module, in the library
//! exact2's Canvas reader loads (\`bun exact.mjs android\` builds the APK).

#[cfg(target_os = "android")]
mod android {
    #[allow(dead_code)]
    mod baked {
        include!(concat!(env!("OUT_DIR"), "/module.rs"));
    }
    use baked::*;

    ${module(false).replace(/\n(?=.)/g, '\n    ')}    include!(concat!(env!("OUT_DIR"), "/logic.rs"));
    include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

    exact_linux::canvas_jni!(super::AppData, hatches = crate::android::exact_hatches());
}
`,
  };
  if (kind === 'windows') return {
    'Cargo.toml': `${header(`[[bin]]\nname = "${name}-windows"\npath = "src/main.rs"\ntest = false\nbench = false`)}exact-logic = ${dep('logic')}
exact-windows = ${dep('host/windows')}
# The packages' Windows implementations name \`exact_linux::package\` (LLP 1133.022 D10 item 5).
exact-linux = ${dep('host/linux')}
exact-js = ${dep('js')}

${build}`,
    'build.rs': bake('windows'),
    // A GUI executable in every profile, as Excord's: opened from Explorer or the desktop, a development build shows
    // no console window beside its own; a parent's handles still carry its stderr.
    'src/main.rs': `//! ${title} on Windows: the host's window over the app's TypeScript data module and its baked plan.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

${includes(true)}
${module(false)}include!(concat!(env!("OUT_DIR"), "/logic.rs"));
// The packages' Windows implementations the build composes (exact_packages()).
include!(concat!(env!("OUT_DIR"), "/packages.rs"));
fn main() {
    let composed = exact_linux::Composed { packages: exact_packages(), ..Default::default() };
    std::process::exit(exact_windows::run_composed::<AppData>(${JSON.stringify(title)}, window(), PLAN, COMPAT, composed));
}
`,
  };
  throw new Error(`no host crate of kind ${kind}`);
}

/** A host crate's files for an authored workspace: the crate at `<dir>/<kind>`, the app one level up. `update` writes
 * `linux/` into an older authored app with it (game/new.mjs). */
export const authoredCrate = (dir, kind, name, title) =>
  Object.fromEntries(Object.entries(crateFiles(kind, { name, title, app: '..', dep: path => `{ path = ${JSON.stringify(pathFrom(resolve(dir, kind), resolve(ROOT, path)))} }` })).map(([file, text]) => [`${kind}/${file}`, text]));

const titleOf = name => name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' ');
const stampOf = (shells) => { try { return JSON.parse(readFileSync(resolve(shells, '.stamp.json'), 'utf8')); } catch { return null; } };

/** Write a file only when its bytes differ, so Cargo's times stay. */
function writeChanged(path, text) {
  if (existsSync(path) && readFileSync(path, 'utf8') === text) return false;
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  return true;
}

/** The generated workspace for the app at `dir`, current with its inputs: the manifest's platforms and name, the
 * checkout's path and the files the workspace copies from it, and this module. Returns the workspace directory and
 * whether anything was written. The lock is exact2's, re-resolved offline for these members; one this machine's cache
 * cannot resolve is left as the copy for the first build, which fetches (scripts/app.mjs `checkOutsideLock`). */
export function ensureShells(dir, manifest = readManifest(dir)) {
  dir = realpathSync(dir);
  const shells = shellsDir(dir), name = basename(dir), title = manifest.app?.name ?? manifest.name ?? titleOf(name);
  const crates = shellCrates(manifest);
  // The paths written are relative where the two trees share a directory (`pathFrom`), which reads real directories.
  for (const kind of crates) mkdirSync(resolve(shells, kind), { recursive: true });
  const copied = ['rust-toolchain.toml', '.cargo/config.toml'].map(file => [file, readFileSync(resolve(ROOT, file), 'utf8')]);
  const patches = patchLines(shells);
  // The inputs whose change rewrites the workspace: the checkout's path and commit, the templates (this module), the
  // files copied from the checkout, its patch table and its lock (a dependency the SDK moved), the manifest's platforms
  // and the app's name.
  const commit = spawnSync('git', ['-C', ROOT, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout?.trim() ?? '';
  const stamp = {
    exact2: ROOT, commit, name, title, crates,
    inputs: createHash('sha256').update([readFileSync(fileURLToPath(import.meta.url), 'utf8'), APPLE_BUILD_SCRIPT, ...copied.map(([, text]) => text), ...patches, readFileSync(resolve(ROOT, 'Cargo.lock'), 'utf8')].join('\0')).digest('hex'),
  };
  const before = stampOf(shells);
  const expected = ['Cargo.toml', 'Cargo.lock', ...copied.map(([file]) => file), ...crates.flatMap(kind => Object.keys(crateFiles(kind, { name, title, dep: () => '', app: '' })).map(file => `${kind}/${file}`))];
  const complete = expected.every(file => existsSync(resolve(shells, file)));
  if (before && JSON.stringify(before) === JSON.stringify(stamp) && complete) return { workspace: shells, wrote: false, crates };
  let wrote = writeChanged(resolve(shells, 'Cargo.toml'), `# ${title}'s native workspace, generated by exact2 from app.json (LLP 1153 D1): never edited, rewritten
# when the manifest's platforms or the checkout change. An app that needs Rust of its own writes a Cargo.toml of its
# own beside app.json, which is used instead.
[workspace]
members = [${crates.map(c => JSON.stringify(c)).join(', ')}]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"

[patch.crates-io]
${patches.join('\n')}
`);
  for (const [file, text] of copied) wrote = writeChanged(resolve(shells, file), text) || wrote;
  for (const kind of crates) {
    const files = crateFiles(kind, { name, title, app: '../../..', dep: path => `{ path = ${JSON.stringify(pathFrom(resolve(shells, kind), resolve(ROOT, path)))} }` });
    for (const [file, text] of Object.entries(files)) wrote = writeChanged(resolve(shells, kind, file), text) || wrote;
  }
  // A crate for a platform no longer named goes with its platform.
  for (const entry of readdirSync(shells, { withFileTypes: true })) {
    if (entry.isDirectory() && Object.values(PLATFORMS).concat('web').includes(entry.name) && !crates.includes(entry.name)) { rmSync(resolve(shells, entry.name), { recursive: true, force: true }); wrote = true; }
  }
  // The lock follows every change: exact2's, re-resolved for these members.
  {
    writeFileSync(resolve(shells, 'Cargo.lock'), readFileSync(resolve(ROOT, 'Cargo.lock')));
    const lock = spawnSync('cargo', ['metadata', '--offline', '--format-version', '1'], { cwd: shells, env: cargoEnvironment(), stdio: ['ignore', 'ignore', 'pipe'], encoding: 'utf8' });
    if (lock.error?.code === 'ENOENT') throw new Error('cargo was not found on PATH or in ~/.cargo/bin; install Rust with rustup (https://rustup.rs)');
    // Cargo may have rewritten the lock before failing to download; the first build recognises exact2's bytes.
    if (lock.status !== 0) writeFileSync(resolve(shells, 'Cargo.lock'), readFileSync(resolve(ROOT, 'Cargo.lock')));
  }
  writeFileSync(resolve(shells, '.stamp.json'), JSON.stringify(stamp) + '\n');
  return { workspace: shells, wrote: true, crates };
}

/** What a native verb does before its build (LLP 1153 D1.2): when the manifest does not name `platform`, check that
 * platform's setup rows (`report`: the rows, or `{ error }` when they could not be read) and stop on a missing one,
 * nothing written; then name it, writing an app.json the app lacked. Then keep the generated workspace current unless
 * the app authored one. Returns the lines to say, or `{ refused }`. */
export async function bringPlatform(dir, platform, { report = null, os = process.platform } = {}) {
  const lines = [], verb = `bun exact.mjs ${platform === 'macos' ? 'mac' : platform}`;
  const manifest = existsSync(resolve(dir, 'app.json')) ? readManifest(dir) : null;
  if (!namesPlatform(manifest ?? {}, platform)) {
    if (['ios', 'macos', 'tvos'].includes(platform) && os !== 'darwin') return { refused: `${verb}: an Apple build needs a Mac with Xcode; nothing was written` };
    if (platform === 'windows' && os !== 'win32') return { refused: `${verb}: a Windows build needs Windows with the MSVC Rust target; nothing was written` };
    if (report?.error) return { refused: `${verb}: the setup report could not be read, so nothing was written: ${report.error}` };
    const missing = (report ?? []).filter(row => !row.ok);
    if (missing.length) return { refused: `${verb}: this machine lacks what a ${platform} build needs; nothing was written:\n${missing.map(row => `  ${row.name}: ${row.have} (SDK ${row.want})\n    ${row.fix}`).join('\n')}` };
    if (!manifest) {
      // An app without app.json had the defaults scripts/app.mjs `readManifest` supplies; naming a platform writes them.
      const name = basename(dir), title = titleOf(name);
      writeManifest(dir, { name: title, app: { id: `com.exact.${name}`, name: title }, host: { web: {} }, deploy: { store: { web: '0' } } });
      lines.push('app.json: written with the defaults the app had without one');
    }
    const wrote = namePlatform(dir, platform);
    if (wrote.length) lines.push(`app.json: ${wrote.join(', ')} added: the app now ships ${platform} (bun exact.mjs update --without ${platform} stops it)`);
  }
  if (!authoredWorkspace(dir)) {
    const { workspace, wrote, crates } = ensureShells(dir);
    if (wrote) lines.push(`${workspace}: the native workspace (${crates.join(', ')}), generated from app.json`);
  }
  return { lines };
}
