#!/usr/bin/env bun
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { gameDefaults } from './app/shells.mjs';
import { pathFrom, patchLines } from '../scripts/app.mjs';

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
    throw new Error('Usage: bun game/new.mjs <name|path> [--assets] (lowercase-hyphenated name, no host suffix)');
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
  if (options.assets === true) writeFileSync(resolve(destination, 'app.json'), JSON.stringify({game:{assets:true}}, null, 2) + "\n");

  const argument = local ? quote(destination === process.cwd() ? '.' : destination) : name;
  const proof = quote(relative(process.cwd(), resolve(destination, 'proof.mjs')));
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

function writeApp(dir, name) {
  for (const host of ['apple', 'web']) mkdirSync(resolve(dir, host));
  const title = name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' ');
  const exact2 = pathFrom(dir, ROOT);
  const dep = (host, path) => `{ path = ${JSON.stringify(pathFrom(resolve(dir, host), resolve(ROOT, path)))} }`;
  const files = {
    'Cargo.toml': `# ${title}: an Exact2 app outside the exact2 checkout, which it uses by path
# (${exact2}). Build profiles are injected by exact2's scripts; the patches
# below are generated by \`exact new\` and checked on every run.
[workspace]
members = ["apple", "web"]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"

[patch.crates-io]
${patchLines(dir).join('\n')}
`,
    'rust-toolchain.toml': readFileSync(resolve(ROOT, 'rust-toolchain.toml'), 'utf8'),
    'exact.mjs': commandsFor(dir, name),
    '.gitignore': '/target/\n/dist/\n/app.contract.d.ts\n/.exact/\n',
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
    main testId="root" width="100%" height="100%" padding=24 background-color="light-dark(#ffffff, #111111)"
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
exact_apple::host!(AppData, PLAN, COMPAT, None, std::ptr::null(), app_data);
`,
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
  writeFileSync(resolve(dir, 'Cargo.lock'), readFileSync(resolve(ROOT, 'Cargo.lock')));
  const deferred = resolveOffline(dir, true);
  const run = 'bun exact.mjs';
  return `Created ${dir}
  cd '${dir.replaceAll("'", "'\\''")}'
  ${run} web          the web dev loop
  ${run} contract types app.contract -o app.contract.d.ts   the types app.ts imports
  ${run} test web     build, then run app.test.contract (web; macos or ios after mac/ios)
  ${run} agent web tree  inspect or drive the app
  ${run} ios --run    build and launch on an iOS simulator
  ${run} mac --run    build and launch on this Mac${deferred ? `
Cargo.lock is still exact2's: this machine's Cargo cache lacks some of its crates, so the
first build resolves it, fetching them once (it needs the network then, not now).` : ''}`;
}

const BEGIN = '<!-- exact:begin (exact new writes this block; bun exact.mjs update rewrites it) -->', END = '<!-- exact:end -->';

/** What an agent in the app's directory can't discover (LLP 1086 D1): where
 * the guides are, the app's commands, and the loop. Paths are from the app to
 * this checkout, so \`update\` follows a checkout that moved. */
function agentNotes(dir, name) {
  const doc = file => pathFrom(dir, resolve(ROOT, 'docs', file));
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
| \`bun exact.mjs web\` | the web dev loop, at http://127.0.0.1:8765/ |
| \`bun exact.mjs test web\` | build the web app if needed, then run \`app.test.contract\` (also \`macos\`, \`ios\`) |
| \`bun exact.mjs agent web tree "tap <id>" "screenshot out.png"\` | drive the app as a person would |
| \`bun exact.mjs mac --run\`, \`bun exact.mjs ios --run\` | build and launch natively |
| \`bun exact.mjs update\` | after exact2 moves or changes its patches |

The loop: generate the types, edit, \`contract build --json\` until it prints \`[]\`,
\`test web\`, look at it with \`agent web … screenshot\`, then the native hosts.
\`bun ${pathFrom(dir, resolve(ROOT, 'scripts/exact.mjs'))} setup --check\` names anything this machine is missing.

Generated, so don't edit: the \`[patch.crates-io]\` table in \`Cargo.toml\`,
\`rust-toolchain.toml\`, \`exact.mjs\`, and this block.

${readFileSync(resolve(ROOT, 'docs/diary.md'), 'utf8').replace(/^#/gm, '##').trimEnd()}
${END}
`;
}

/** CLAUDE.md is a copy of AGENTS.md, not exact2's own symlink: a build's
 * source capture refuses links in an app, and `update` rewrites both blocks. */
function linkClaude(dir) {
  writeFileSync(resolve(dir, 'CLAUDE.md'), readFileSync(resolve(dir, 'AGENTS.md')));
}

/** The diary's own block, from before it joined the generated one. */
const OLD_DIARY = /\n*<!-- exact diary[^>]*-->[\s\S]*?<!-- \/exact diary -->\n?/;

/** Rewrite the generated block in AGENTS.md and a CLAUDE.md that is a copy,
 * keeping what an author wrote around it; an app with neither gets both.
 * The diary's \`.exact/\` stays out of git. */
function updateNotes(dir, name) {
  const block = agentNotes(dir, name), changed = [];
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
function commandsFor(dir, name) {
  return `#!/usr/bin/env bun
// ${name}'s commands, run with the exact2 checkout named by EXACT2
// (default ${pathFrom(dir, ROOT)}). Generated by \`exact new\`; \`bun exact.mjs update\` rewrites
// this file, the crates.io patches, the toolchain and the exact2 dependency paths.
import { spawnSync } from 'node:child_process';
import { appendFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const EXACT2 = resolve(import.meta.dir, process.env.EXACT2 ?? ${JSON.stringify(pathFrom(dir, ROOT))});
const [verb, ...rest] = process.argv.slice(2);
const host = (verb === 'test' || verb === 'agent') && rest[0] && !rest[0].startsWith('-') ? rest.shift() : 'web';
const verbs = {
  web: ['host/web/dev.mjs', '--app', '${name}'],
  'web-build': ['host/web/build.mjs', '${name}'],
  test: ['scripts/agent.mjs', host, '--app', '${name}', '--test', resolve(import.meta.dir, 'app.test.contract')],
  agent: ['scripts/agent.mjs', host, '--app', '${name}'],
  ios: ['host/apple/build.mjs', '--ios', '${name}-apple'],
  mac: ['host/apple/build.mjs', '${name}-apple'],
  update: ['scripts/exact.mjs', 'new', import.meta.dir, '--update'],
  contract: ['scripts/exact.mjs', 'contract'],
  feedback: ['scripts/feedback.mjs'],
};
if (!verbs[verb]) {
  console.error(\`Usage: bun exact.mjs <\${Object.keys(verbs).join('|')}> [arguments for that script]\`);
  process.exit(2);
}
// The automatic build reports on stderr, so a drive's stdout stays its reply (\`--json\`).
const run = ([script, ...args], more = [], stdio = 'inherit') => spawnSync(process.execPath, [resolve(EXACT2, script), ...args, ...more], {
  stdio,
  env: { ...process.env, EXACT_APP_DIR: import.meta.dir },
}).status ?? 1;
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
if (drivesWeb) { const built = run(verbs['web-build'], [], [0, 2, 2]); if (built) process.exit(log(built)); }
process.exit(log(run(verbs[verb], rest)));
`;
}

/** Cargo re-resolves the copied lock for this workspace, offline and minimally.
 * A new app whose crates this machine's Cargo cache lacks keeps exact2's lock
 * as copied (`deferrable`); `resolveApp` resolves a lock still identical to
 * exact2's at its first build, which may fetch. Returns whether it deferred. */
function resolveOffline(dir, deferrable = false) {
  const lock = spawnSync('cargo', ['metadata', '--offline', '--format-version', '1'], { cwd: dir, stdio: ['ignore', 'ignore', 'pipe'], encoding: 'utf8' });
  if (lock.status === 0) return false;
  if (lock.error?.code === 'ENOENT') throw new Error('cargo was not found on PATH or in ~/.cargo/bin; install Rust with rustup (https://rustup.rs)');
  // Cargo may have rewritten the lock before failing to download; the first
  // build recognises exact2's bytes, so put them back.
  if (deferrable && /no matching package named|attempting to make an HTTP request|in the offline mode/.test(lock.stderr ?? '')) return writeFileSync(resolve(dir, 'Cargo.lock'), readFileSync(resolve(ROOT, 'Cargo.lock'))), true;
  throw new Error(`cargo could not resolve ${dir} offline:\n${lock.stderr}`);
}

function updateApp(dir, name) {
  if (!existsSync(resolve(dir, 'app.contract')) || !existsSync(resolve(dir, 'Cargo.toml'))) throw new Error(`${dir}: no app workspace here to update (no app.contract or Cargo.toml)`);
  const manifest = readFileSync(resolve(dir, 'Cargo.toml'), 'utf8');
  const block = `[patch.crates-io]\n${patchLines(dir).join('\n')}\n`;
  // The table runs from its header to the next header; its trailing blank line stays.
  const section = /^\[patch\.crates-io\]\n(?:(?!\[)[^\n]*\n?)*/m, at = manifest.search(section);
  writeFileSync(resolve(dir, 'Cargo.toml'), at < 0 ? `${manifest.trimEnd()}\n\n${block}`
    : manifest.replace(section, table => block + (at + table.length < manifest.length ? '\n' : '')));
  writeFileSync(resolve(dir, 'rust-toolchain.toml'), readFileSync(resolve(ROOT, 'rust-toolchain.toml')));
  writeFileSync(resolve(dir, 'exact.mjs'), commandsFor(dir, name));
  const notes = updateNotes(dir, name);
  const manifestPath = resolve(dir, 'app.json');
  if (existsSync(manifestPath)) {
    const text = readFileSync(manifestPath, 'utf8');
    const next = text.replace(/("\$schema"\s*:\s*)"[^"]*"/, (_, head) => head + JSON.stringify(pathFrom(dir, resolve(ROOT, 'scripts/app.schema.json'))));
    if (next !== text) writeFileSync(manifestPath, next);
  }
  // An older app may predate the generated test command. Preserve authored
  // tests; otherwise start with a boot check that assumes no app-specific IDs.
  const test = resolve(dir, 'app.test.contract');
  if (!existsSync(test)) writeFileSync(test, `// Add assertions for this app after its initial work settles.
test "the app opens"
  clock settle
`);
  // Each exact2 crate is found by name, so a checkout that moved is followed.
  const metadata = spawnSync('cargo', ['metadata', '--no-deps', '--offline', '--format-version', '1'], { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 26 });
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
  return `Updated ${dir}: patches, toolchain, exact.mjs${notes.length ? `, ${notes.join(', ')}` : ''}${changed.length ? `, exact2 paths in ${changed.join(', ')}` : ''}`;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args[0] === '--app') console.log(createApp(args.find((a, i) => i > 0 && !a.startsWith('--')), { update: args.includes('--update') }));
  else console.log(createGame(args[0], undefined, {assets:args.includes("--assets")}));
}
