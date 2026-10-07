#!/usr/bin/env bun
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { gameDefaults } from './app/shells.mjs';
import { cargoEnvironment, pathFrom, patchLines } from '../scripts/app.mjs';

/** Type names a game's generated type may not take: the engine's public items
 * (what `use exact_game::*` brings in) and the template's own. */
export function takenTypes(directory = import.meta.dir) {
  const read = path => existsSync(path) ? readFileSync(path, 'utf8') : '';
  const lib = read(resolve(directory, 'engine/src/lib.rs'));
  const exported = [...lib.matchAll(/^pub use [^;]*;/gms)].flatMap(m => m[0].match(/\b[A-Z][A-Za-z0-9]*\b/g) ?? []);
  const items = text => [...text.matchAll(/^\s*pub (?:struct|enum|trait|type) ([A-Z][A-Za-z0-9]*)/gm)].map(m => m[1]);
  const template = [...read(resolve(directory, 'new/logic/src/lib.rs')).matchAll(/^\s*(?:pub )?(?:struct|enum|trait|type) ([A-Z][A-Za-z0-9]*)/gm)].map(m => m[1]);
  return new Set([...exported, ...items(read(resolve(directory, 'engine/src/scene.rs'))), ...template.filter(t => t !== 'SmallGame')]);
}
export function createGame(destination, directory = import.meta.dir, options = {}) {
  const local = destination === '.' || /[\\/]/.test(destination ?? '');
  const name = local ? basename(resolve(destination)) : destination;
  if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name ?? '') || /-(web|apple|linux|windows|gpu)$/.test(name)) {
    throw new Error('Usage: bun game/new.mjs <name|path> [--assets] [--render] (lowercase-hyphenated name, no host suffix)');
  }
  // The type is `use exact_game::*;`'s neighbour: it may not shadow an engine export
  // (World, Camera…) or another of the template's items (Beacon, Options).
  const collided = takenTypes(directory).has(name.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(''));
  if (collided) throw new Error(`${name}: its Rust type would collide with an exact_game export or a template item; choose another name, such as ${name}-game`);
  destination = local ? resolve(destination) : resolve(directory, 'games', name);
  if (existsSync(destination) && readdirSync(destination).length) throw new Error(`Game already exists: ${destination}`);
  cpSync(resolve(directory, 'new'), destination, {recursive:true});
  destination = realpathSync(destination);
  directory = realpathSync(directory);
  const quote = path => `'${path.replaceAll("'", "'\\''")}'`;
  const script = (file, from = process.cwd()) => quote(relative(from, resolve(directory, file)));
  const title = name.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
  const type = title.replaceAll(' ', '');
  for (const file of readdirSync(destination, {recursive:true, withFileTypes:true})) {
    if (!file.isFile()) continue;
    const path = resolve(file.parentPath, file.name);
    writeFileSync(path, readFileSync(path, 'utf8').replaceAll('small-game', name)
      .replaceAll('small_game', name.replaceAll('-', '_'))
      .replaceAll('SmallGame', type)
      .replaceAll('Small game', title)
      .replaceAll('bun /path/to/exact2/game/prove.mjs', `bun ${script('prove.mjs', destination)}`)
      .replaceAll('bun /path/to/exact2/game/app/shells.mjs', `bun ${script('app/shells.mjs', destination)}`));
  }
  writeFileSync(resolve(destination, '.gitignore'), '/target/\n/dist/\n/dist.previous/\n/artifacts/\n/.shells/\n/app.contract.d.ts\n');
  const proofPath = resolve(destination,'proof.mjs');
  if (existsSync(proofPath)) writeFileSync(proofPath,readFileSync(proofPath,'utf8').replace("'../../proof.mjs'",JSON.stringify(relative(destination,resolve(directory,'proof.mjs')))));
  // Identity derives from the directory and Game::ID; app.json holds authored keys only.
  if (!gameDefaults(destination)) throw new Error(`${destination}/logic/src/lib.rs: the template's Game declaration was not found`);
  const manifest = {game:{...(options.assets === true ? {assets:true} : {})}};
  if (options.render === true) {
    // The GPU-only hooks crate (LLP 1046.008, game.render): an empty pass set and
    // an explicitly declared shader root, reflected at build.
    const files = {
      'render/Cargo.toml': `# Render hooks (app.json game.render): what the GPU module draws beyond the\n# engine's frame. Only the GPU shell links this crate; the simulation never does.\n[package]\nname = "${name}-render"\nversion = "0.1.0"\nedition = "2021"\nlicense = "MIT"\nworkspace = "../.shells"\n\n[dependencies]\nexact-game-render.workspace = true\nexact-gpu.workspace = true\n\n[build-dependencies]\nexact-gpu-reflect.workspace = true\n`,
      'render/build.rs': `//! Reflect the shader inventory the game bake assembles (gpu.shaderRoots, each\n//! after its gpu.shaderPreludes) in EXACT_GAME_SHADERS: \`shaders::SHADERS\`\n//! names the interfaces the hosts register.\nfn main() {\n    println!("cargo:rerun-if-env-changed=EXACT_GAME_SHADERS");\n    let dir = std::path::PathBuf::from(std::env::var_os("EXACT_GAME_SHADERS").expect("build through the game bake"));\n    println!("cargo:rerun-if-changed={}", dir.display());\n    let generated = exact_gpu_reflect::generate(&dir).unwrap_or_else(|e| panic!("{e}"));\n    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("shaders.rs");\n    std::fs::write(out, generated.rust).unwrap();\n}\n`,
      'render/src/lib.rs': `//! ${title}'s render hooks: implement exact_game_render::Hooks stages here.\n\n/// The reflected shader registry of render/shaders.\npub mod shaders {\n    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));\n}\n\n/// The game's passes; every stage defaults to drawing nothing.\n#[derive(Default)]\npub struct Passes;\nimpl exact_game_render::Hooks for Passes {}\n`,
    };
    for (const [file, text] of Object.entries(files)) {
      mkdirSync(dirname(resolve(destination, file)), {recursive:true});
      writeFileSync(resolve(destination, file), text);
    }
    mkdirSync(resolve(destination, 'render/shaders'), {recursive:true});
    manifest.game.render = {crate:`${name}-render`, hooks:'Passes', shaders:'shaders::SHADERS'};
    manifest.gpu = {shaderRoots:['render/shaders']};
  }
  if (Object.keys(manifest.game).length) writeFileSync(resolve(destination, 'app.json'), JSON.stringify(manifest, null, 2) + "\n");

  const argument = local ? quote(destination === process.cwd() ? '.' : destination) : name;
  const proof = quote(relative(process.cwd(), resolve(destination, 'proof.mjs')));
  // A game outside this checkout gets what an app does (LLP 1086; the
  // platformer's diary, R1): its exact.mjs verbs, AGENTS.md and the diary.
  if (local && !`${destination}/`.startsWith(`${dirname(directory)}/`)) {
    writeFileSync(resolve(destination, 'exact.mjs'), commandsFor(destination, name, {game:true}));
    writeFileSync(resolve(destination, 'AGENTS.md'), agentNotes(destination, name, {game:true}));
    linkClaude(destination);
    editorTasks(destination, {game:true});
    writeFileSync(resolve(destination, '.gitignore'), `${readFileSync(resolve(destination, '.gitignore'), 'utf8')}/.exact/\n`);
    return `Created ${destination}
  cd ${quote(destination)}
  bun exact.mjs test-rust     the hostless Rust tests (logic/tests) and determinism lints
  bun exact.mjs web           the web dev loop
  bun exact.mjs test web      build (wasm), then run app.test.contract
  bun exact.mjs agent web tree  inspect or drive the game
  bun exact.mjs mac --run     build and launch on this Mac
  bun exact.mjs windows --run build and launch on Windows
  bun exact.mjs prove         the proof's first baseline (pins.json)`;
  }
  return `Created ${local ? destination : `game/games/${name}`}\n  bun ${script('dev.mjs')} ${argument}\n  bun ${script('prove.mjs')} ${argument}
  bun ${proof} web --screenshot-only`;
}

const ROOT = resolve(import.meta.dir, '..');

/** An ordinary app outside this repository (LLP 1036.001 D2, D3): a Cargo
 * workspace of its own that uses this checkout by path. Everything an author
 * used to copy by hand is written from the checkout itself: the crates.io
 * patches, the toolchain, the lockfile (re-resolved offline for the new
 * workspace, so it starts at exactly the versions exact2 builds with) and the
 * host crates. Build profiles are not written at all; the scripts inject the
 * root's (injectedProfiles, D1). `resolveApp` checks the patches and the
 * toolchain on every run (outsideWorkspaceProblems).
 *
 * `update: true` rewrites only those generated parts in an existing app,
 * after the checkout moved or the root gained a patch; the app's own files
 * are left alone. */
export function createApp(destination, options = {}) {
  const dir = resolve(destination ?? '');
  const name = basename(dir);
  if (!destination || !/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name) || /-(web|apple|linux|gpu)$/.test(name)) {
    throw new Error('Usage: exact new <path> [--update] (the last part names the app: lowercase-hyphenated, no host suffix)');
  }
  if (options.update) return updateApp(dir, name);
  if (existsSync(dir) && readdirSync(dir).length) throw new Error(`${dir} already exists and is not empty`);
  mkdirSync(dir, { recursive: true });
  try { return writeApp(dir, name); }
  catch (error) {
    rmSync(dir, { recursive: true, force: true });
    throw error;
  }
}

/** The app's Linux host crate (LLP 1086): one executable with the runner, the kernel, the
 * baked plan and the TypeScript data module, in the shape of the repo's own (apps/duo-lab/linux).
 * The Android host is this crate built for Android (LLP 1107). */
function linuxCrate(dir, name, title) {
  const dep = path => `{ path = ${JSON.stringify(pathFrom(resolve(dir, 'linux'), resolve(ROOT, path)))} }`;
  return {
    'linux/Cargo.toml': `[package]
name = "${name}-linux"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false
build = "build.rs"

[[bin]]
name = "${name}-linux"
path = "src/main.rs"

[dependencies]
exact-logic = ${dep('logic')}
exact-linux = ${dep('host/linux')}
exact-js = ${dep('js')}

[build-dependencies]
exact-js-bake = ${dep('js/bake')}
`,
    'linux/build.rs': `fn main() {
    exact_js_bake::build(std::path::Path::new(".."), "linux").expect("bake ${title}");
}
`,
    'linux/src/main.rs': `//! ${title} on Linux (and Android): the Contract UI and the TypeScript data module.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS)
        .with_canvas_surfaces(CANVAS_SURFACES)
        .placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
fn main() {
    std::process::exit(exact_linux::run::<AppData>(PLAN, COMPAT));
}
`,
  };
}

function writeApp(dir, name) {
  for (const host of ['apple', 'linux', 'web']) mkdirSync(resolve(dir, host));
  const title = name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' ');
  const exact2 = pathFrom(dir, ROOT);
  const dep = (host, path) => `{ path = ${JSON.stringify(pathFrom(resolve(dir, host), resolve(ROOT, path)))} }`;
  const files = {
    'Cargo.toml': `# ${title}: an Exact2 app outside the exact2 checkout, which it uses by path
# (${exact2}). Build profiles are injected by exact2's scripts; the patches
# below are generated by \`exact new\` and checked on every run.
[workspace]
members = ["apple", "linux", "web"]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"

[patch.crates-io]
${patchLines(dir).join('\n')}
`,
    'rust-toolchain.toml': readFileSync(resolve(ROOT, 'rust-toolchain.toml'), 'utf8'),
    '.cargo/config.toml': readFileSync(resolve(ROOT, '.cargo/config.toml'), 'utf8'),
    'exact.mjs': commandsFor(dir, name),
    '.gitignore': '/target/\n/dist/\n/node_modules/\n/app.contract.d.ts\n/.exact/\n',
    // Contract libraries come through node_modules (LLP 1091 D9): \`bun add\`
    // one, or \`"name": "file:../path"\` for a local library.
    'package.json': JSON.stringify({ name, private: true, dependencies: {} }, null, 2) + '\n',
    'AGENTS.md': agentNotes(dir, name),
    'app.json': JSON.stringify({
      $schema: pathFrom(dir, resolve(ROOT, 'scripts/app.schema.json')),
      name: title, short_name: title, id: `com.example.${name}`, start_url: '/', display: 'standalone',
      app: { id: `com.example.${name}`, name: title },
      host: { ios: { minimumOS: '17.0', deviceFamily: ['iphone', 'ipad'] }, macos: { minimumOS: '14.0', window: { width: 900, height: 700 } }, web: {} },
      deploy: { store: { web: '0', macos: '0', ios: '0', linux: '0' } },
    }, null, 2) + '\n',
    'app.contract': `// ${title}: the view. app.ts answers what it asks for.
shape Greeting
  text: string

component ${title.replaceAll(' ', '')}
  resource greeting = greeting("${title}") as shape Greeting
  view
    main testId="root"
      viewport-fit="cover"
      width="100%"
      height="100%"
      box-sizing="border-box"
      padding-top="calc(env(safe-area-inset-top) + 24px)"
      padding-right="calc(env(safe-area-inset-right) + 24px)"
      padding-bottom="calc(env(safe-area-inset-bottom) + 24px)"
      padding-left="calc(env(safe-area-inset-left) + 24px)"
      background-color="light-dark(#ffffff, #111111)"
      text greeting.text font-size=28 color="light-dark(#111111, #eeeeee)" testId="greeting"
`,
    'app.test.contract': `test "the greeting loads"
  expect tree has "root"
  expect text "greeting" == "Hello from ${title}."
`,
    'app.ts': `import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.${name}';
export const grants = '';

const sources: Sources = {
  greeting: ([name]): Result<'greeting'> => ({ text: \`Hello from \${name}.\` }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
`,
    'apple/Cargo.toml': `[package]
name = "${name}-apple"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false
build = "build.rs"

[lib]
crate-type = ["rlib"]

[dependencies]
exact-logic = ${dep('apple', 'logic')}
exact-apple = ${dep('apple', 'host/apple')}
exact-js = ${dep('apple', 'js')}

[build-dependencies]
exact-js-bake = ${dep('apple', 'js/bake')}
`,
    'apple/build.rs': `fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(std::path::Path::new(".."), platform).expect("bake ${title}");
}
`,
    'apple/src/lib.rs': `//! ${title} on Apple: the Contract UI and the TypeScript data module.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));
const BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.hbc"));

type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
fn embedded_data() -> ExactEmbeddedData {
    exact_js::Module::new(BYTECODE.to_vec(), APP, GRANTS)
        .with_canvas_surfaces(CANVAS_SURFACES)
        .placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
include!(concat!(env!("OUT_DIR"), "/linked.rs"));
exact_apple::host!(AppData, PLAN, COMPAT, None, std::ptr::null(), app_data; linked = EXACT_LINKED);
`,
    ...linuxCrate(dir, name, title),
    'web/Cargo.toml': `[package]
name = "${name}-web"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false
build = "build.rs"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
exact-logic = ${dep('web', 'logic')}
exact-web = ${dep('web', 'host/web')}
exact-web-capabilities = ${dep('web', 'host/web-capabilities')}
exact-js-web = ${dep('web', 'js/web')}

[build-dependencies]
exact-js-bake = ${dep('web', 'js/bake')}
`,
    'web/build.rs': `fn main() {
    exact_js_bake::build(std::path::Path::new(".."), "web").expect("bake ${title}");
}
`,
    'web/src/lib.rs': `//! ${title} on the web: the Contract UI and the TypeScript data module.

include!(concat!(env!("OUT_DIR"), "/module.rs"));
const PLAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"));
const COMPAT: &str = include_str!(concat!(env!("OUT_DIR"), "/compat.json"));

type ExactEmbeddedData = exact_js_web::Module;
fn embedded_data() -> ExactEmbeddedData {
    exact_js_web::Module::new(APP, GRANTS, REVISION)
        .with_canvas_surfaces(CANVAS_SURFACES)
        .placed(TYPESCRIPT_PLACEMENT)
}
include!(concat!(env!("OUT_DIR"), "/logic.rs"));
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
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(resolve(dir, path)), { recursive: true });
    writeFileSync(resolve(dir, path), text);
  }
  linkClaude(dir);
  editorTasks(dir);
  writeFileSync(resolve(dir, 'Cargo.lock'), readFileSync(resolve(ROOT, 'Cargo.lock')));
  const deferred = resolveOffline(dir, true);
  const run = 'bun exact.mjs';
  return `Created ${dir}
  cd '${dir.replaceAll("'", "'\\''")}'
  ${run} web          the web dev loop
  ${run} contract types app.contract -o app.contract.d.ts   the types app.ts imports
  ${run} test web     build, then run app.test.contract, or the files named (web; macos or ios after mac/ios)
  ${run} agent web tree  inspect or drive the app
  ${run} ios --run    build and launch on an iOS simulator
  ${run} mac --run    build and launch on this Mac
  ${run} linux        build the Linux host (then test linux / agent linux, headless anywhere)${deferred ? `
Cargo.lock is still exact2's: this machine's Cargo cache lacks some of its crates, so the
first build resolves it, fetching them once (it needs the network then, not now).` : ''}`;
}

const BEGIN = '<!-- exact:begin (exact new writes this block; bun exact.mjs update rewrites it) -->', END = '<!-- exact:end -->';

/** What an agent in the app's directory can't discover (LLP 1086 D1): where
 * the guides are, the app's commands, and the loop. Paths are from the app to
 * this checkout, so \`update\` follows a checkout that moved. */
function agentNotes(dir, name, {game = false} = {}) {
  const doc = file => pathFrom(dir, resolve(ROOT, 'docs', file));
  if (game) return gameNotes(dir, name, doc);
  return `${BEGIN}
# ${name}: an Exact app

The view is \`app.contract\` (Contract), its data is \`app.ts\` (TypeScript), and
\`app.json\` is the manifest (its \`$schema\` gives an editor every key). The app uses
the exact2 checkout at \`${pathFrom(dir, ROOT)}\` by path (\`EXACT2\` overrides it).

Read before writing code:

- ${doc('contract-for-agents.md')}: the working guide. Start here.
- ${doc('agent-pitfalls.md')}: verified footguns, symptom → cause → fix.
- ${doc('contract-for-humans.md')}: explanations and complete examples, including the data module.
- ${doc('contract-grammar.md')}: exact forms, built-in functions, events.

Commands, from this directory:

| | |
|---|---|
| \`bun exact.mjs contract types app.contract -o app.contract.d.ts\` | the types \`app.ts\` imports; rerun after changing a source's signature |
| \`bun exact.mjs contract build app.contract --json\` | compile; \`[]\` or every diagnostic with its range |
| \`bun exact.mjs contract vocab [name]\` | the tags, attributes and CSS properties Contract accepts |
| \`bun exact.mjs web\` | the web dev loop, at the URL it prints (8765 unless another loop holds it) |
| \`bun exact.mjs test web\` | build the web app if needed, then run \`app.test.contract\` (also \`macos\`, \`ios\`) |
| \`bun exact.mjs agent web --storage s1 tree "tap <id>" "screenshot out.png"\` | drive the app as a person would; \`--storage <name>\` gives its storage sources a scratch store (without it their writes are refused; \`logs\` has the detail) |
| \`bun exact.mjs agent ios tree "tap <testId>" "screenshot s.png"\` | the same on an iOS simulator; drive by \`testId\`, never by coordinates |
| \`bun exact.mjs mac --run\`, \`bun exact.mjs ios --run\` | build and launch natively |
| \`bun exact.mjs linux\`, then \`test linux\` or \`agent linux …\` | build the Linux host and drive it headless (any machine, no display); \`bun exact.mjs android\` builds the same host for Android (LLP 1107) |
| \`bun exact.mjs hatch <word>\` | an access hatch: a stub for each target the app builds, and its \`app.json\` entry (\`--app\`, \`--window\` for those scopes) |
| \`bun exact.mjs update\` | after exact2 moves or changes its patches; it rewrites \`exact.mjs\` |
| app.json \`"commands": {"verify": ["bun", "verify.mjs"]}\` | the app's own verbs: \`bun exact.mjs verify web\` runs \`bun verify.mjs web\` here; \`update\` keeps them |

Keep drive scripts, evidence, logs and runtime files in \`.exact/\` (git-ignored):
no build, watcher or freshness check reads it. Anything else in this folder is a
source: changing it makes the driver refuse to drive until the app is rebuilt.

The loop: generate the types, edit, \`contract build --json\` until it prints \`[]\`,
\`test web\`, look at it with \`agent web … screenshot\`, then the native hosts.
Before the first native TypeScript build on a machine, run the one-time installer
\`bun ${pathFrom(dir, resolve(ROOT, 'scripts/exact.mjs'))} setup\`; it installs the pinned
host bundle and the iOS/tvOS bundles this Mac builds.
\`bun ${pathFrom(dir, resolve(ROOT, 'scripts/exact.mjs'))} setup --check\` only checks and
names anything this machine is missing. Cargo builds themselves are forced offline for Hermes.

Build it native. A hand-built lookalike of a system control is a bug; write the
Contract form and each host draws its own (the agent guide's "Prefer native
controls"): \`button appearance="auto"\`, \`list appearance="auto"\` with
\`section\`s for a settings screen, \`input type="checkbox" switch\`, \`type="range"\`,
date and time inputs, \`select\`, a \`popover="auto" role="menu"\`, a \`role="tablist"\`,
and a route whose first child is a \`header\` holding one heading (the nav bar).
A screen scrolls only inside a \`scroll\`, a \`list\` or an \`overflow-y="auto"\` box; right after the
header and named by the route's \`navigationScroll\`, it also collapses a large
title. A sheet swipes down, and a pushed screen swipes back, only when the route
has an enabled control whose \`id\` is the root's \`navigationBack\`.

Drive it by \`testId\`, never by screen coordinates: give every control a \`testId\`,
find targets with \`tree\` (\`tree --ax\` for the platform's accessibility tree), and
\`tap\`/\`type\` them with \`agent ios\` as with \`agent web\`. Under the agent the
authored header and tablist stand in for the native bars and take the same taps.

Match a reference's structure, controls and hierarchy, not its pixels: native
controls set their own metrics. Don't measure sub-point positions; stop when it
reads as the same app.

Access hatches, for what only the platform's own object can do: mark a node
\`hatch="word"\` and the app's native code (Swift in \`modules/apple\`, the page
module in \`modules/web/index.js\`) is handed the view or element Exact built,
at defined moments. \`bun exact.mjs hatch <word>\` writes the stubs. A hatch
configures what Exact made; it changes Contract state only by acting on an
authored node, as a person would: \`click()\`, \`focus()\`, \`blur()\`, and
\`input(text)\` for a field's whole value. It reads state only through
\`data-*\` words the Contract puts on the node. The app must work without it:
\`EXACT_HATCHES=off\` (\`?hatches=off\` on the web) runs a development build
with no hatch connected. Make a hatch say what it does, so the agent can see
it: \`diagnostics.log/count/measure/publish\` show in \`logs\`, \`state\`
(\`state.hatches\`) and \`agent … "perf hatches"\`; \`owns(view, "what")\` puts
what it added in \`tree\` under its node, and \`parts\` names a control it drew
so \`tap <testId>/<part>\` can reach it as a real click.

A backend: an app that keeps shared, server-authoritative data (accounts,
other people's rows, offline writes that sync) uses Snapback 4 through this
checkout's first-party client, never hand-written HTTP: read
${pathFrom(dir, resolve(ROOT, 'snapback4/README.md'))} before any data code. It
mounts \`${pathFrom(dir, resolve(ROOT, 'snapback4/ts'))}\` in \`app.json\`'s
\`typescript.sources\` and gives \`app.ts\` a local-first device
(\`Snapback.open\`, \`read\`, \`write\`, \`sync\`, \`outcome\`).

Contract libraries: \`use Card from "@scope/ui"\` reads an installed package's
\`.contract\` files (\`bun add @scope/ui\`, or \`"@me/ui": "file:../ui"\` in
\`package.json\` for a local one), and \`use Activity from "exact:motion"\` a
built-in. Each file sees only the names its \`use\` lines list.

Generated, so don't edit: the \`[patch.crates-io]\` table in \`Cargo.toml\`,
\`rust-toolchain.toml\`, \`.cargo/config.toml\`, \`exact.mjs\`, and this block.

${readFileSync(resolve(ROOT, 'docs/diary.md'), 'utf8').replace(/^#/gm, '##').trimEnd()}
${END}
`;
}

/** A game's notes: the app's, for a world in Rust under Contract's menus. */
function gameNotes(dir, name, doc) {
  const sdk = file => pathFrom(dir, resolve(ROOT, file));
  return `${BEGIN}
# ${name}: an Exact game

The world is Rust: \`logic/src/lib.rs\` implements \`Game\` (its \`Options\` are the
canvas's arguments, \`setup\` and \`tick\` its gameplay). Menus, the HUD and accessible
controls are \`app.contract\` (Contract). \`app.json\` is optional and holds only keys
you author (a title, \`game.audio\`, \`game.assets\`, a data crate). The bake generates
the hosts under \`.shells/\` (ignored; never edit it). The game uses the exact2 checkout
at \`${pathFrom(dir, ROOT)}\` by path (\`EXACT2\` overrides it).

Read before writing code:

- ${sdk('game/README.md')}: the game target — the programming model, input, saves, determinism, the proof.
- ${sdk('game/engine/README.md')}: the engine's API.
- ${doc('contract-for-agents.md')}: Contract, for the menus and HUD.
- ${doc('agent-pitfalls.md')}: verified footguns, symptom → cause → fix.
- ${doc('contract-grammar.md')}: exact forms, built-in functions, events (keys at a game's canvas too).

Commands, from this directory:

| | |
|---|---|
| \`bun exact.mjs test-rust\` | the hostless Rust tests (\`logic/tests\`) and the determinism lints |
| \`bun exact.mjs contract build app.contract --json\` | compile; \`[]\` or every diagnostic with its range |
| \`bun exact.mjs web\` | the web dev loop, at the URL it prints (a game builds the wasm target) |
| \`bun exact.mjs test web\` | build the web game if needed, then run \`app.test.contract\` (also \`macos\`, \`ios\`) |
| \`bun exact.mjs agent web "tap play" "type world key ArrowRight for 800" "screenshot out.png"\` | drive the game as a person would |
| \`bun exact.mjs mac --run\`, \`bun exact.mjs ios --run\` | build and launch natively |
| \`bun exact.mjs linux\`, then \`test linux\` or \`agent linux …\` | build the Linux host and drive it headless (any machine, no display); \`bun exact.mjs android\` builds the same host for Android (LLP 1107) |
| \`bun exact.mjs windows --run\` | build and launch the standalone Windows game; omit \`--run\` to package only |
| \`bun exact.mjs prove\`, then \`bun proof.mjs web\` | the proof: a first baseline in \`pins.json\`, then real-host checks against it |
| \`bun exact.mjs update\` | after exact2 moves; it rewrites \`exact.mjs\` and this block |
| app.json \`"commands": {"replay": ["bun", "tools/replay.mjs"]}\` | the game's own verbs: \`bun exact.mjs replay web\` runs \`bun tools/replay.mjs web\` here |

The loop: \`test-rust\` while tuning gameplay, \`contract build --json\` until it prints
\`[]\`, \`test web\`, look at it with \`agent web … screenshot\`, then the native hosts.
\`proof.mjs\` asserts the starter's gameplay (its beacons, KeyE): rewrite it when you
change \`Options\` or the scene, before the first \`prove\`.
\`bun ${sdk('scripts/exact.mjs')} setup --check\` names anything this machine is missing.

Generated, so don't edit: \`exact.mjs\` and this block.

${readFileSync(resolve(ROOT, 'docs/diary.md'), 'utf8').replace(/^#/gm, '##').trimEnd()}
${END}
`;
}

/** CLAUDE.md is AGENTS.md, as in exact2 itself: a link (a build's capture
 * skips a link it never reads), or a copy where the filesystem has none
 * (`update` rewrites a copy's block too). */
function linkClaude(dir) {
  try { symlinkSync('AGENTS.md', resolve(dir, 'CLAUDE.md')); }
  catch { writeFileSync(resolve(dir, 'CLAUDE.md'), readFileSync(resolve(dir, 'AGENTS.md'))); }
}

/** VS Code's tasks for the app's exact.mjs verbs. `$exact-contract` is the
 * problem matcher of exact2's editors/vscode extension, `$rustc` is
 * rust-analyzer's. Written once and the author's after that: `update` writes
 * it only when it is missing. */
function editorTasks(dir, {game = false} = {}) {
  const path = resolve(dir, '.vscode/tasks.json');
  if (existsSync(path)) return false;
  const builds = ['$exact-contract', '$rustc'];
  const task = (label, command, problemMatcher, extra = {}) => ({label, type: 'shell', command: `bun exact.mjs ${command}`, problemMatcher, ...extra});
  const tasks = [
    task('contract: build this file', 'contract build "${file}"', '$exact-contract', {presentation: {reveal: 'silent', clear: true}}),
    task('contract: format this file', 'contract fmt "${file}"', '$exact-contract', {presentation: {reveal: 'silent', clear: true}}),
    task('app: web dev loop', 'web', '$exact-contract'),
    task('app: test on web', 'test web', builds, {group: {kind: 'test', isDefault: true}}),
    task('app: run on macOS', 'mac --run', builds),
    task('app: run on an iOS simulator', 'ios --run', builds),
    ...(game ? [task('game: Rust tests', 'test-rust', '$rustc', {group: 'test'})] : []),
  ];
  mkdirSync(dirname(path), {recursive: true});
  writeFileSync(path, JSON.stringify({version: '2.0.0', tasks}, null, 2) + '\n');
  return true;
}

/** The diary's own block, from before it joined the generated one. */
const OLD_DIARY = /\n*<!-- exact diary[^>]*-->[\s\S]*?<!-- \/exact diary -->\n?/;

/** Rewrite the generated block in AGENTS.md and a CLAUDE.md that is a copy,
 * keeping what an author wrote around it; an app with neither gets both.
 * The diary's \`.exact/\` stays out of git. */
function updateNotes(dir, name, options = {}) {
  const block = agentNotes(dir, name, options), changed = [];
  const ignorePath = resolve(dir, '.gitignore'), ignore = existsSync(ignorePath) ? readFileSync(ignorePath, 'utf8') : '';
  if (!/^\/?\.exact\/?$/m.test(ignore)) { writeFileSync(ignorePath, `${ignore}${ignore && !ignore.endsWith('\n') ? '\n' : ''}/.exact/\n`); changed.push('.gitignore'); }
  const present = ['AGENTS.md', 'CLAUDE.md'].filter(file => existsSync(resolve(dir, file)) && !lstatSync(resolve(dir, file)).isSymbolicLink());
  if (!present.length) {
    writeFileSync(resolve(dir, 'AGENTS.md'), block);
    if (!existsSync(resolve(dir, 'CLAUDE.md'))) linkClaude(dir);
    return [...changed, 'AGENTS.md', 'CLAUDE.md'];
  }
  for (const file of present) {
    const path = resolve(dir, file), text = existsSync(path) ? readFileSync(path, 'utf8').replace(OLD_DIARY, '\n') : '';
    const at = text.indexOf(BEGIN), end = text.indexOf(END, at);
    const next = at >= 0 && end > at ? text.slice(0, at) + block.trimEnd() + text.slice(end + END.length)
      : text ? `${text.trimEnd()}\n\n${block}` : block;
    if (next !== text) { writeFileSync(path, next); changed.push(file); }
  }
  return changed;
}

/** The app's own command runner. EXACT2 names the checkout once (D3); the
 * default is the one that created the app, so a sibling checkout needs nothing. */
function commandsFor(dir, name, {game = false} = {}) {
  return `#!/usr/bin/env bun
// ${name}'s commands, run with the exact2 checkout named by EXACT2
// (default ${pathFrom(dir, ROOT)}). Generated by \`exact new\`; \`bun exact.mjs update\` rewrites
// this file, the crates.io patches, the toolchain and the exact2 dependency paths. The app's own
// verbs live in app.json's \`commands\`, which update leaves alone.
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { constants } from 'node:os';
import { relative, resolve } from 'node:path';

const EXACT2 = resolve(import.meta.dir, process.env.EXACT2 ?? ${JSON.stringify(pathFrom(dir, ROOT))});
const [verb, ...rest] = process.argv.slice(2);
const host = (verb === 'test' || verb === 'agent') && rest[0] && !rest[0].startsWith('-') ? rest.shift() : 'web';
const verbs = {
  web: ['host/web/dev.mjs', '--app', '${name}'],
  'web-build': ['host/web/build.mjs', '${name}'],
  test: ['scripts/agent.mjs', host, '--app', '${name}'],
  agent: ['scripts/agent.mjs', host, '--app', '${name}'],
  ios: ['host/apple/build.mjs', '--ios', '${name}-apple'],
  mac: ['host/apple/build.mjs', '${name}-apple'],
  linux: ['scripts/build-linux.mjs', '${name}'],
  android: ['scripts/agent-android.mjs', 'build', '${name}'],
  update: ['scripts/exact.mjs', 'new', import.meta.dir, '--update'],
  contract: ['scripts/exact.mjs', 'contract'],
  hatch: ['scripts/exact.mjs', 'hatch'],
  feedback: ['scripts/feedback.mjs'],${game ? `
  // A game's own: its hostless Rust tests and its proof's baseline (exact2's game/README.md).
  'test-rust': ['game/app/shells.mjs', import.meta.dir, '--test'],
  windows: ['host/windows/build.mjs', '${name}'],
  prove: ['game/prove.mjs', import.meta.dir],` : ''}
};
// The app's own verbs (paint, minesweeper, ledger: a regenerated file dropped the ones added here): app.json's
// \`commands\`, each a command as argv (\`"verify": ["bun", "verify.mjs"]\`), run in this directory with the
// arguments after the verb. A game's app.json is optional.
const manifest = resolve(import.meta.dir, 'app.json');
const own = (existsSync(manifest) ? JSON.parse(readFileSync(manifest, 'utf8')).commands : null) ?? {};
const clash = Object.keys(own).find((name) => verbs[name]);
if (clash) {
  console.error(\`app.json commands.\${clash}: \${clash} is one of exact.mjs's own verbs; give the app's another name\`);
  process.exit(2);
}
if (!verbs[verb] && !own[verb]) {
  console.error(\`Usage: bun exact.mjs <\${[...Object.keys(verbs), ...Object.keys(own)].join('|')}> [arguments for that script]\`);
  process.exit(2);
}
// The automatic build reports on stderr, so a drive's stdout stays its reply (\`--json\`).
// The child ends with this process: a signal here is passed on, and a dev server
// stops when this process is gone however it ended (\`EXACT_LAUNCHER_PID\`).
const spawned = (command, args, stdio = 'inherit', cwd = undefined) => new Promise((done) => {
  const child = spawn(command, args, {
    stdio,
    cwd,
    env: { ...process.env, HERMES_LEAN_SYS_OFFLINE: '1', EXACT_APP_DIR: import.meta.dir, EXACT2, EXACT_LAUNCHER_PID: String(process.pid) },
  });
  const pass = (signal) => child.kill(signal), signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
  for (const signal of signals) process.on(signal, pass);
  child.on('error', () => done(1));
  child.on('exit', (code, signal) => {
    for (const s of signals) process.off(s, pass);
    done(code ?? 128 + (constants.signals[signal] ?? 0));
  });
});
const run = ([script, ...args], more = [], stdio = 'inherit') => spawned(process.execPath, [resolve(EXACT2, script), ...args, ...more], stdio);
const started = Date.now();
// The diary's command log (exact2's docs/diary.md): which exact2, what ran, how it ended. No arguments.
const log = exit => { if (verb !== 'feedback') try {
  const exact2 = spawnSync('git', ['-C', EXACT2, 'rev-parse', '--short=9', 'HEAD'], { encoding: 'utf8' }).stdout?.trim() || null;
  mkdirSync(resolve(import.meta.dir, '.exact'), { recursive: true });
  appendFileSync(resolve(import.meta.dir, '.exact/commands.jsonl'), JSON.stringify({ at: new Date(started).toISOString(), exact2, verb: verb === 'test' || verb === 'agent' ? \`\${verb} \${host}\` : verb, exit, ms: Date.now() - started }) + '\\n');
} catch {} return exit; };
// The web build is about a second when nothing changed, so a web drive builds
// first rather than refusing a stale build; a native build stays explicit.
const drivesWeb = (verb === 'test' || verb === 'agent') && host === 'web' && !rest.some(a => a === '--url' || a === '--web-dist');
// \`test [host] [file|glob …]\`: the test files named (each \`.contract\`, a glob or \`--test <file>\`, from the
// current directory), else app.test.contract; each runs in turn and any failure fails the command.
const tests = [];
for (let i = verb === 'test' ? 0 : rest.length; i < rest.length;) {
  const named = rest[i] === '--test' ? rest.splice(i, 2)[1] : /\\.contract$|\\*/.test(rest[i]) ? rest.splice(i, 1)[0] : (i++, null);
  if (named == null) continue;
  const found = named.includes('*') ? [...new Bun.Glob(named).scanSync({ absolute: true })].sort() : [resolve(named)];
  if (!found.length) { console.error(\`no test file matches \${named}\`); process.exit(log(2)); }
  tests.push(...found);
}
if (drivesWeb) { const built = await run(verbs['web-build'], [], [0, 2, 2]); if (built) process.exit(log(built)); }
if (own[verb]) { const [command, ...args] = own[verb]; process.exit(log(await spawned(command === 'bun' ? process.execPath : command, [...args, ...rest], 'inherit', import.meta.dir))); }
if (verb !== 'test') process.exit(log(await run(verbs[verb], rest)));
let failed = 0;
for (const file of tests.length ? tests : [resolve(import.meta.dir, 'app.test.contract')]) {
  if (tests.length > 1) console.log(\`# \${relative(process.cwd(), file)}\`);
  failed = (await run(verbs.test, ['--test', file, ...rest])) || failed;
}
process.exit(log(failed));
`;
}

/** Cargo re-resolves the copied lock for this workspace, offline and minimally.
 * A new app whose crates this machine's Cargo cache lacks keeps exact2's lock
 * as copied (`deferrable`); `resolveApp` resolves a lock still identical to
 * exact2's at its first build, which may fetch. Returns whether it deferred. */
function resolveOffline(dir, deferrable = false) {
  const lock = spawnSync('cargo', ['metadata', '--offline', '--format-version', '1'], { cwd: dir, env: cargoEnvironment(), stdio: ['ignore', 'ignore', 'pipe'], encoding: 'utf8' });
  if (lock.status === 0) return false;
  if (lock.error?.code === 'ENOENT') throw new Error('cargo was not found on PATH or in ~/.cargo/bin; install Rust with rustup (https://rustup.rs)');
  // Cargo may have rewritten the lock before failing to download; the first
  // build recognises exact2's bytes, so put them back.
  if (deferrable && /no matching package named|attempting to make an HTTP request|in the offline mode/.test(lock.stderr ?? '')) return writeFileSync(resolve(dir, 'Cargo.lock'), readFileSync(resolve(ROOT, 'Cargo.lock'))), true;
  throw new Error(`cargo could not resolve ${dir} offline:\n${lock.stderr}`);
}

function updateApp(dir, name) {
  // A game has no workspace of its own (the bake generates .shells/): its runner and notes are all there is to rewrite.
  if (existsSync(resolve(dir, 'app.contract')) && !existsSync(resolve(dir, 'Cargo.toml')) && gameDefaults(dir)) {
    writeFileSync(resolve(dir, 'exact.mjs'), commandsFor(dir, name, {game:true}));
    const notes = updateNotes(dir, name, {game:true});
    if (editorTasks(dir, {game:true})) notes.push('.vscode/tasks.json');
    return `Updated ${dir}: exact.mjs${notes.length ? `, ${notes.join(', ')}` : ''}`;
  }
  if (!existsSync(resolve(dir, 'app.contract')) || !existsSync(resolve(dir, 'Cargo.toml'))) throw new Error(`${dir}: no app workspace here to update (no app.contract or Cargo.toml)`);
  // The app's crates name it, not its folder: a renamed folder keeps `<name>-web`, `<name>-apple`
  // (authoring bench: `exact.mjs ios` asked Cargo for `todo-apple` in a `todo/` holding `todo-list-*`).
  name = ['web', 'apple', 'linux'].map(kind => resolve(dir, kind, 'Cargo.toml')).filter(existsSync)
    .map(path => String(Bun.TOML.parse(readFileSync(path, 'utf8')).package?.name ?? '').match(/^(.+)-(?:web|apple|linux)$/)?.[1]).find(Boolean) ?? name;
  const manifest = readFileSync(resolve(dir, 'Cargo.toml'), 'utf8');
  const block = `[patch.crates-io]\n${patchLines(dir).join('\n')}\n`;
  // The table runs from its header to the next header; its trailing blank line stays.
  const section = /^\[patch\.crates-io\]\n(?:(?!\[)[^\n]*\n?)*/m, at = manifest.search(section);
  writeFileSync(resolve(dir, 'Cargo.toml'), at < 0 ? `${manifest.trimEnd()}\n\n${block}`
    : manifest.replace(section, table => block + (at + table.length < manifest.length ? '\n' : '')));
  writeFileSync(resolve(dir, 'rust-toolchain.toml'), readFileSync(resolve(ROOT, 'rust-toolchain.toml')));
  mkdirSync(resolve(dir, '.cargo'), { recursive: true });
  writeFileSync(resolve(dir, '.cargo/config.toml'), readFileSync(resolve(ROOT, '.cargo/config.toml')));
  writeFileSync(resolve(dir, 'exact.mjs'), commandsFor(dir, name));
  // An app made before the scaffold wrote a Linux host gains one (LLP 1086), unless it has its own.
  const addedLinux = !existsSync(resolve(dir, 'linux'));
  if (addedLinux) {
    mkdirSync(resolve(dir, 'linux'));
    const title = name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' ');
    for (const [path, text] of Object.entries(linuxCrate(dir, name, title))) {
      mkdirSync(dirname(resolve(dir, path)), { recursive: true });
      writeFileSync(resolve(dir, path), text);
    }
    const workspace = readFileSync(resolve(dir, 'Cargo.toml'), 'utf8');
    writeFileSync(resolve(dir, 'Cargo.toml'), workspace.replace(/^members\s*=\s*\[([^\]]*)\]/m, (line, list) =>
      /"linux"/.test(list) ? line : `members = [${[...list.split(',').map(m => m.trim()).filter(Boolean), '"linux"'].sort().join(', ')}]`));
  }
  const notes = updateNotes(dir, name);
  if (editorTasks(dir)) notes.push('.vscode/tasks.json');
  const manifestPath = resolve(dir, 'app.json');
  if (existsSync(manifestPath)) {
    const text = readFileSync(manifestPath, 'utf8');
    const next = text.replace(/("\$schema"\s*:\s*)"[^"]*"/, (_, head) => head + JSON.stringify(pathFrom(dir, resolve(ROOT, 'scripts/app.schema.json'))));
    if (next !== text) writeFileSync(manifestPath, next);
  }
  // An older app may predate the generated test command. Preserve authored
  // tests; otherwise start with a boot check that assumes no app-specific IDs.
  const test = resolve(dir, 'app.test.contract');
  // The app's data lands before a test's first step (`clock data`), so the
  // check needs no `clock settle` and has no step at all.
  if (!existsSync(test)) writeFileSync(test, `// The app boots and its data lands; add this app's own assertions.
test "the app opens"
`);
  // Each exact2 crate is found by name, so a checkout that moved is followed.
  const metadata = spawnSync('cargo', ['metadata', '--no-deps', '--offline', '--format-version', '1'], { cwd: ROOT, env: cargoEnvironment(), encoding: 'utf8', maxBuffer: 1 << 26 });
  if (metadata.status !== 0) throw new Error(`cargo metadata in ${ROOT}:\n${metadata.stderr}`);
  const crates = new Map(JSON.parse(metadata.stdout).packages.map(p => [p.name, dirname(p.manifest_path)]));
  const changed = [];
  for (const file of readdirSync(dir, { recursive: true }).filter(f => basename(f) === 'Cargo.toml' && !/^(target|dist)\//.test(f))) {
    const path = resolve(dir, file), text = readFileSync(path, 'utf8');
    const next = text.replace(/^(exact-[a-z0-9-]+)(\s*=\s*\{[^}\n]*\bpath\s*=\s*)"[^"]*"/gm, (line, crate, head) =>
      crates.has(crate) ? `${crate}${head}${JSON.stringify(pathFrom(dirname(path), crates.get(crate)))}` : line);
    if (next !== text) { writeFileSync(path, next); changed.push(file); }
  }
  resolveOffline(dir);
  return `Updated ${dir}: patches, toolchain, exact.mjs${addedLinux ? ', a linux/ host crate' : ''}${notes.length ? `, ${notes.join(', ')}` : ''}${changed.length ? `, exact2 paths in ${changed.join(', ')}` : ''}`;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args[0] === '--app') console.log(createApp(args.find((a, i) => i > 0 && !a.startsWith('--')), { update: args.includes('--update') }));
  else console.log(createGame(args[0], undefined, {assets:args.includes("--assets"), render:args.includes("--render")}));
}
