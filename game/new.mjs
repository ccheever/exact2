#!/usr/bin/env bun
import { spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { chmodSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { capturedLockDrift, gameDefaults, gameShells, prepareGame } from './app/shells.mjs';
import { readDeviceStamp } from '../snapback4/web/device-build.mjs';
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
  writeFileSync(resolve(destination, '.gitignore'), '/target/\n/dist/\n/dist.previous/\n/artifacts/\n/.shells/\n/app.contract.d.ts\n/assets/exact-tables.json\n');
  const proofPath = resolve(destination,'proof.mjs');
  if (existsSync(proofPath)) writeFileSync(proofPath,readFileSync(proofPath,'utf8').replace("'../../proof.mjs'",JSON.stringify(relative(destination,resolve(directory,'proof.mjs')))));
  // Identity derives from the directory and Game::ID; app.json holds authored keys only.
  if (!gameDefaults(destination)) throw new Error(`${destination}/logic/src/lib.rs: the template's Game declaration was not found`);
  const manifest = {game:{...(options.assets === true ? {assets:true} : {})}};
  // A game outside this checkout gets exact.mjs, AGENTS.md and the diary (below).
  const outside = local && !`${destination}/`.startsWith(`${dirname(directory)}/`);
  if (options.render === true) {
    // The GPU-only hooks crate (LLP 1046.008, game.render): an empty pass set and
    // an explicitly declared shader root, reflected at build.
    const files = {
      'render/Cargo.toml': `# Render hooks (app.json game.render): what the GPU module draws beyond the\n# engine's frame. Only the GPU shell links this crate; the simulation never does.\n# ../.shells is generated: \`${outside ? 'bun exact.mjs shells' : `bun ${script('app/shells.mjs', destination)} .`}\` writes it, as any bake does.\n[package]\nname = "${name}-render"\nversion = "0.1.0"\nedition = "2021"\nlicense = "MIT"\nworkspace = "../.shells"\n\n[dependencies]\nexact-game-render.workspace = true\nexact-gpu.workspace = true\n\n[build-dependencies]\nexact-gpu-reflect.workspace = true\n`,
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

  // Resolve against this SDK now, before the first locked test or proof.
  // No captured lock is needed unless the author adds dependencies.
  prepareGame(destination, gameDefaults(destination).game, directory);

  const argument = local ? quote(destination === process.cwd() ? '.' : destination) : name;
  const proof = quote(relative(process.cwd(), resolve(destination, 'proof.mjs')));
  // A game outside this checkout gets what an app does (LLP 1086; the
  // platformer's diary, R1): its exact.mjs verbs, AGENTS.md and the diary.
  if (outside) {
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
    throw new Error('Usage: exact new <path> [--update | --snapback [--port <n>]] (the last part names the app: lowercase-hyphenated, no host suffix)');
  }
  if (options.update) return updateApp(dir, name, options);
  if (existsSync(dir) && readdirSync(dir).length) throw new Error(`${dir} already exists and is not empty`);
  const port = options.snapback ? snapbackPort(options.snapback.port) : null;
  if (options.snapback?.online && !existsSync(resolve(dir, '..', 'snapback/schema.q'))) throw new Error('exact new --snapback --online: write the backend schema above the app first; the guestbook starter uses a device');
  mkdirSync(dir, { recursive: true });
  let created;
  const existingBackend = options.snapback && existsSync(resolve(dir, '..', 'snapback/schema.q'));
  try {
    created = writeApp(dir, name);
    if (!options.snapback) return created;
    if (!existingBackend) snapbackStarter(dir, name, port, JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).app.id);
    else {
      const source = resolve(dir, 'app.ts');
      const ts = readFileSync(source, 'utf8');
      writeFileSync(source, ts.replace("export const grants = '';", `const origin = 'http://127.0.0.1:${port}';\nexport const grants = \`net.fetch \${origin}${options.snapback.online ? '' : '\\nsqlite.open app:/data'}\`;`));
    }
  } catch (error) {
    rmSync(dir, { recursive: true, force: true });
    throw error;
  }
  // The app stands; what wiring could not do (no server installed yet, a cold device build that failed) it says.
  const wired = wireSnapback(dir, { build: options.snapback.build ?? true, port, online: options.snapback.online ?? false });
  const roster = seedRoster(dir);
  return `${created}${existingBackend ? '\nThe backend already has a schema: kept the greeting app until generated operations are available; online wiring replaces it with one query and mutation. No guestbook was substituted.' : ''}\n${wired}${roster ? '\nseed.json: sample roster rows; review the persona, role and field values before testing. app.test.contract uses fresh "seed.json".' : ''}`;
}

const rosterRule = /^[ \t]+insert[ \t]*<-[ \t]*deny[ \t]*(?:--[^\n]*)?$/m;

/** Development fixtures are examples: schema types cannot determine an app's intended privileges. */
export function rosterSeeds(schema) {
  schema = schema.replace(/\r\n?/g, '\n').replace(/^[ \t]*--[^\n]*$/gm, '') + '\n';
  const persona = /^use personas\s+([\w-]+)/m.exec(schema)?.[1];
  const tables = [...schema.matchAll(/^table[ \t]+(\w+):[ \t]*(?:--[^\n]*)?\n((?:[ \t]+[^\n]*\n|\n)*)/gm)]
    .filter(m => rosterRule.test(m[2]));
  return tables.map(([, table, body]) => {
    let bits = BigInt(`0x${createHash('sha256').update(table).digest('hex').slice(0,32)}`), id = '';
    const alphabet = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
    do { id = alphabet[Number(bits & 31n)] + id; bits >>= 5n; } while (bits);
    const row = {id: id.padStart(26, '0')};
    for (const [, field, raw] of body.matchAll(/^[ \t]+(\w+):[ \t]*([^\n]+)/gm)) {
      const type = raw.replace(/\s+--.*$/, '').trim();
      if (/\?|\bdefault\b/.test(type)) continue;
      if (type === 'principal') {
        if (!persona) throw new Error(`seed.json: ${table}.${field} needs an authenticated principal; create a backend seed.json with [{table,row}] before scaffolding`);
        row[field] = `dev:${persona}`;
      } else if (/^enum\(/.test(type)) row[field] = /['"]([^'"]+)['"]/.exec(type)?.[1];
      else if (/^text\b/.test(type)) row[field] = 'example'.padEnd(Number(/text\s+(\d+)\.\./.exec(type)?.[1] ?? 1), 'x').slice(0, Number(/(?:\.\.|<=)\s*(\d+)/.exec(type)?.[1] ?? 80));
      else if (type === 'bool') row[field] = true;
      else if (type === 'time') row[field] = Date.now();
      else if (/^(?:int|number)\b/.test(type)) row[field] = Number(/(-?\d+)\.\./.exec(type)?.[1] ?? 0);
      else throw new Error(`seed.json: cannot infer ${table}.${field}: ${type}; create a backend seed.json with [{table,row}] before scaffolding`);
    }
    return {table, row};
  });
}

function seedRoster(dir) {
  const schema = readFileSync(resolve(dir, '../snapback/schema.q'), 'utf8');
  if (!rosterRule.test(schema.replace(/\r\n?/g, '\n'))) return false;
  const fixture = resolve(dir, '../seed.json');
  const rows = existsSync(fixture) ? JSON.parse(readFileSync(fixture, 'utf8')) : rosterSeeds(schema);
  if (!Array.isArray(rows) || rows.some(r => typeof r?.table !== 'string' || !r.row || Array.isArray(r.row) || typeof r.row !== 'object')) throw new Error('backend seed.json must be a list of {table,row}');
  writeFileSync(resolve(dir, 'seed.json'), JSON.stringify(rows, null, 2) + '\n');
  const test = resolve(dir, 'app.test.contract');
  writeFileSync(test, `fresh "seed.json"\n${readFileSync(test, 'utf8').replace(/^fresh(?:\s+"[^"]*")?\s*\n/m, '')}`);
  return true;
}

/** The Snapback origin's port `--port` names (`snapback4 dev`'s default, 4400, else). */
function snapbackPort(port = 4400, command = 'exact new --snapback --port') {
  const n = Number(port);
  if (!Number.isInteger(n) || n < 1 || n > 65535) throw new Error(`${command}: ${port} is not a port (1-65535)`);
  return n;
}

// @ref LLP 1086 — `exact new --snapback` and `bun exact.mjs snapback` (the author's ruling of 2026-10-12, via the
// sb4 app farm): what the README's pasted setup block did, as a verb. The backend is the directory above the app.
const SNAPBACK_SCHEMA = `use personas alice, bob

table notes:
  author: principal
  body: text 1..200
  at: time
  by byTime: at, id
  public 'notes are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  sync public last 100 by byTime

query recent():
  return notes last 50 by byTime

mutation post(body: text 1..200):
  row = insert notes { author: viewer, body, at: now }
  return { id: row.id }
`;

/** The snapback4 server version this checkout's driver is pinned to (its root package.json). */
export function snapbackVersion() {
  return JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).devDependencies?.snapback4 ?? 'latest';
}

/** A guestbook on Snapback in place of the greeting (snapback4/README.md's complete app): the view, the data module
 * at the origin `port` names, its tests, and the backend's schema when the directory above has none. */
function snapbackStarter(dir, name, port, id) {
  const title = name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' ');
  const origin = `http://127.0.0.1:${port}`;
  const files = {
    'app.contract': `// ${title}: the view. app.ts answers it through Snapback4 (a guestbook to start from).
shape Clock
  epochAtZero: number
shape Note
  id: string
  author: string
  body: string
  pending: bool
shape Board
  online: bool
  message: string
  latest: string
  sending: number
  notes: Note[]
shape Ack
  ok: bool
  message: string
  id: string

component ${title.replaceAll(' ', '')}
  state persona = "alice"
  state draft = ""
  state notice = ""
  state sampledAt = 0
  resource time = exactTime() as shape Clock
  resource board = notes(persona, time.epochAtZero + sampledAt) as shape Board else empty(message="Connecting…")
  mutation posted as shape Ack queue refreshes board then afterPost
  derive banner = match failure(board) {
    case some(f) => (f.code == "offline" || f.code == "E_OFFLINE") ? "You're offline" : f.reason != "" ? \`Couldn't load notes (\${f.reason})\` : \`Couldn't load notes (\${f.message})\`,
    case none => ""
  }
  task live mount
    every(5000, refreshBoard)
  action refreshBoard
    sampledAt = performanceNow()
    refresh board
  action choosePersona(value: string)
    persona = value
  action editDraft(value: string)
    draft = value
  action postNote
    send posted = post(persona, draft.trim(), time.epochAtZero + performanceNow())
  action afterPost
    match posted
      case some(result)
        notice = result.message
        draft = ""
      case none
        notice = ""
  view
    main testId="root" padding=24
      column gap=12 max-width=600
        text "${title}" role="heading" aria-level=1
        select value=persona change=choosePersona appearance="auto" testId="persona" aria-label="Signed in as"
          option value="alice"
            text "Alice"
          option value="bob"
            text "Bob"
        if banner != ""
          text banner testId="banner"
        text (board.online ? "Synced" : board.message) testId="status"
        text \`\${board.sending} sending\` testId="sending"
        row gap=8
          input value=draft input=editDraft testId="draft" placeholder="Say something" aria-label="Note" flex=1
          button appearance="auto" press=postNote disabled=(draft.trim() == "") testId="post"
            text "Post"
        text notice testId="notice"
        if failed(posted)
          text (match failure(posted) { case some(f) => f.message, case none => "" }) testId="post-error"
        text board.latest testId="latest"
        for note, i in board.notes key=note.id
          text \`\${note.author}: \${note.body}\${note.pending ? " (sending)" : ""}\` testId=\`note-\${i}\`
`,
    'app.ts': `import type { Answer, Result, Sources } from './app.contract.d.ts';
import { Snapback, refused, pick, type Sent } from './snapback4/snapback.ts';
import { api } from './backend/api.ts';                 // the backend's generated operations (app.json mounts ../snapback/generated)

export const appId = '${id}';            // app.json's "app": { "id" }: change the two together
// The Snapback origin: \`snapback4 dev --port ${port}\` in the directory above, and the tests' fetch prefixes.
const origin = '${origin}';
export const grants = \`net.fetch \${origin}\\nsqlite.open app:/data\`;

// One partition open at a time: switching persona waits for the other's work in flight, then closes its partition.
const personas = Snapback.switcher((persona: string) => ({ app: appId, name: \`${name}-\${persona}\`, origin,
  viewer: \`dev:\${persona}\`, headers: () => ({ 'x-snapback-persona': persona }) }));

function ack(fate: Sent<{id: string} | null>): Result<'post'> {
  if (fate.state === 'failed') throw refused(fate.why);
  if (fate.state === 'sent') return {ok: true, message: 'Posted.', id: fate.result?.id ?? ''};
  return {ok: true, message: 'Saved on this device; it sends when online.', id: ''};
}
const note = pick<Result<'notes'>['notes'][number]>();

const sources: Sources = {
  notes: ([persona, now], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'notes'>> => {
    const round = await db.sync();                      // offline is fine once synced
    const read = await db.read(api.recent, {}, now);
    if (read.denied) throw refused(read.denied);        // failure(board): \`offline\`, or \`error\` with the refusal's code as \`reason\`
    // Project each row onto the shape: an answer's extra fields are refused.
    const notes = (read.data ?? []).map(row => note({...row, pending: row.pending === true}, 'id', 'author', 'body', 'pending'));
    return { online: round.ok, message: round.ok ? '' : 'Offline: showing what this device keeps',
      latest: notes.length ? \`\${notes[0].author}: \${notes[0].body}\` : '',
      sending: notes.filter(note => note.pending).length, notes };
  }),
  post: ([persona, body, now], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'post'>> => {
    // Write, sync and the write's fate in one call: \`sent\`, \`failed\` with the refusal's code, or \`pending\` (kept; sent later).
    return ack(await db.send(api.post, { body }, now));
  }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
`,
    'app.test.contract': `// Run \`snapback4 dev --port ${port}\` above; fresh isolates its rows, epoch now dates the tests as the server does.
fresh
epoch now

test "a note posts, and another persona sees it"
  clock data
  expect text "status" == "Synced"
  expect tree missing "banner"
  type "draft" "hello from alice"
  tap "post"
  clock data
  expect text "notice" == "Posted."
  type "persona" "bob"
  clock data
  expect text "latest" == "dev:alice: hello from alice"

test "offline, a note is kept, then delivered on reconnect"
  clock data
  fail fetch "backend"
  type "draft" "from the tunnel"
  tap "post"
  clock data
  expect text "notice" == "Saved on this device; it sends when online."
  expect text "sending" == "1 sending"
  pass fetch "backend"
  clock +5000
  clock data
  expect text "sending" == "0 sending"
`,
  };
  for (const [path, text] of Object.entries(files)) writeFileSync(resolve(dir, path), text);
  const ignore = resolve(dir, '.gitignore');
  writeFileSync(ignore, `${readFileSync(ignore, 'utf8')}/assets/snapback4.wasm\n`);
  const schema = resolve(dir, '..', 'snapback/schema.q');
  if (!existsSync(schema)) { mkdirSync(dirname(schema), { recursive: true }); writeFileSync(schema, SNAPBACK_SCHEMA); }
}

/** The driver's mount, from the app to this checkout: relative always, since the native bake refuses an absolute one. */
function driverMount(dir, online = false) {
  return relative(realpathSync(dir), realpathSync(resolve(ROOT, online ? 'snapback4/online' : 'snapback4/ts'))).replaceAll('\\', '/');
}

/** Replace only the untouched greeting; arbitrary schemas get typed calls, never invented columns. */
function onlineStarter(dir, generated, port) {
  const source = resolve(dir, 'app.ts'), view = resolve(dir, 'app.contract');
  if (!existsSync(generated)) return false;
  const name = basename(dir), manifest = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8'));
  const pristine = appSources(name, name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' '), manifest.app.id);
  const normalized = readFileSync(source, 'utf8')
    .replace(/^const origin = ['"`][^'"`]+['"`];\n/m, '')
    .replace('export const grants = `net.fetch ${origin}`;', "export const grants = '';")
    .replace("export const grants = '' + `\\nnet.fetch ${origin}`;", "export const grants = '';");
  if (normalized !== pristine['app.ts'] || readFileSync(view, 'utf8') !== pristine['app.contract'] || readFileSync(resolve(dir, 'app.test.contract'), 'utf8') !== pristine['app.test.contract']) return false;
  const handles = readFileSync(generated, 'utf8').split(/export const api[^=]*=/)[1] ?? '';
  const operations = [...handles.matchAll(/^\s*([\w$]+):\s*\{\s*kind:\s*["'](query|mutation)["']/gm)];
  const query = operations.find(m => m[2] === 'query')?.[1], mutation = operations.find(m => m[2] === 'mutation')?.[1];
  if (!query || !mutation) return false;
  const appId = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).app.id;
  const old = readFileSync(source, 'utf8');
  const origin = /const origin\s*=\s*['"`](https?:\/\/[^'"`]+)['"`]/.exec(old)?.[1] ?? `http://127.0.0.1:${port ?? 4400}`;
  const schema = readFileSync(resolve(dir, '../snapback/schema.q'), 'utf8');
  const persona = /^use personas\s+([\w-]+)/m.exec(schema)?.[1];
  writeFileSync(source, `import type { Answer, Result, Sources, ArgsOf } from './app.contract.d.ts';
import { Snapback, refused } from './snapback4/snapback.ts';
import { api } from './backend/api.ts';
export const appId = ${JSON.stringify(appId)};
// One origin for requests, grants, backend launcher and test fetch "backend".
const origin = ${JSON.stringify(origin)};
export const grants = \`net.fetch \${origin}\`;
const db = new Snapback({origin${persona ? `, headers: () => ({'x-snapback-persona': ${JSON.stringify(persona)}})` : ''}});
// The server validates JSON arguments. Replace these editors with your app's typed fields.
const sources: Sources = {
  queryBackend: async ([json]): Promise<Result<'queryBackend'>> => {
    const args: ArgsOf<typeof api.${query}> = JSON.parse(json);
    const read = await db.read(api.${query}, args);
    if (read.denied) throw refused(read.denied);
    return {text: JSON.stringify(read.data), complete: read.complete === true};
  },
  mutateBackend: async ([json]): Promise<Result<'mutateBackend'>> => {
    const args: ArgsOf<typeof api.${mutation}> = JSON.parse(json);
    const sent = await db.send(api.${mutation}, args);
    if (sent.state === 'failed') throw refused(sent.why);
    return {text: JSON.stringify(sent.result)};
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
`);
  writeFileSync(view, `shape QueryReply
  text: string
  complete: bool
shape MutationReply
  text: string
component App
  state queryArgs = "{}"
  state mutationArgs = "{}"
  resource result = queryBackend(queryArgs) as shape QueryReply else empty()
  mutation changed as shape MutationReply refreshes result
  action editQuery(value: string)
    queryArgs = value
  action editMutation(value: string)
    mutationArgs = value
  action runMutation
    send changed = mutateBackend(mutationArgs)
  action reloadQuery
    refresh result
  view
    column gap=8 padding=16 testId="backend"
      text "Query: ${query}" role="heading" aria-level=1
      textarea value=queryArgs input=editQuery testId="query-args" aria-label="Query arguments as JSON"
      button "Refresh" press=reloadQuery testId="refresh"
      if failed(result)
        text (match failure(result) { case some(f) => f.code, case none => "" }) testId="query-error"
      else
        text result.text testId="query-result"
        if not result.complete
          text "Partial answer; more rows may exist." testId="partial"
      text "Mutation: ${mutation}" role="heading" aria-level=2
      textarea value=mutationArgs input=editMutation testId="mutation-args" aria-label="Mutation arguments as JSON"
      button "Send" press=runMutation disabled=pending(changed) testId="send"
      text (match changed { case some(r) => r.text, case none => "" }) testId="mutation-result"
      text (match failure(changed) { case some(f) => f.code, case none => "" }) testId="mutation-error"
`);
  writeFileSync(resolve(dir, 'app.test.contract'), `epoch now

test "the backend operations open"
  expect tree has "query-args"
  expect tree has "mutation-args"
`);
  return true;
}

const onlineMount = path => /(?:^|\/)snapback4\/online\/?$/.test((path ?? '').replaceAll('\\', '/'));

/** `bun exact.mjs snapback` (and the end of \`exact new --snapback\`): what the README's setup block did. Generates the backend's
 * operations once its server is installed above, mounts the driver and those operations in app.json, builds the
 * device the page loads, and says what is left: the server to install or start, an appId that disagrees. */
export function wireSnapback(dir, { build = true, port = null, online = null, artifactOnly = false, rebuild = false } = {}) {
  dir = resolve(dir);
  const manifestPath = resolve(dir, 'app.json');
  if (!existsSync(manifestPath)) throw new Error(`${dir}: no app.json here (bun exact.mjs snapback runs in the app's directory, the backend's above it)`);
  if (artifactOnly) {
    if (port !== null || online !== null) throw new Error('exact snapback --artifact-only: cannot also change --port or --online');
    if (build) buildSnapbackDevice(dir);
    return 'assets/snapback4.wasm: rebuilt; app sources and mounts kept unchanged.';
  }
  const parent = dirname(dir), said = [], version = snapbackVersion();
  const existing = JSON.parse(readFileSync(manifestPath, 'utf8')).typescript?.sources ?? {};
  online ??= onlineMount(existing.snapback4);
  const expected = { snapback4: driverMount(dir, online), backend: '../snapback/generated' };
  for (const [name, path] of Object.entries(expected)) {
    if (existing[name] !== undefined && resolve(dir, existing[name]) !== resolve(dir, path))
      throw new Error(`exact snapback: refusing to rewrite an edited mount in app.json:\n- ${name}: ${JSON.stringify(existing[name])}\n+ ${name}: ${JSON.stringify(path)}\nKeep your mount, or explicitly edit app.json to the proposed path before running again.`);
  }
  if (port !== null) {
    port = snapbackPort(port, 'exact snapback --port');
    const source = resolve(dir, 'app.ts');
    let ts = existsSync(source) ? readFileSync(source, 'utf8') : '';
    let origin = /const origin\s*=\s*['"`](https?:\/\/[^'"`]+)['"`]/.exec(ts)?.[1];
    if (!origin) {
      // A plain `exact new` app has no backend yet. Only add a declaration
      // where no differently expressed origin could be silently shadowed.
      if (/\b(?:const|let|var)\s+origin\b/.test(ts)) throw new Error('exact snapback --port: origin is not a literal URL; edit its declaration explicitly');
      const grants = /export const grants\s*=\s*(['"`])([\s\S]*?)\1\s*;/;
      if (!grants.test(ts)) throw new Error('exact snapback --port: app.ts needs a literal export const grants; write its net.fetch grant explicitly');
      origin = `http://127.0.0.1:${port}`;
      ts = `const origin = '${origin}';\n` + ts.replace(grants, (_, quote, value) =>
        `export const grants = ${quote}${value}${quote} + \`\\nnet.fetch \${origin}${online ? '' : '\\nsqlite.open app:/data'}\`;`);
      writeFileSync(source, ts);
      said.push(`Snapback origin: added ${origin} and its ${online ? 'net.fetch' : 'net.fetch/sqlite.open'} grants to app.ts.`);
    }
    if (!['127.0.0.1','localhost','[::1]'].includes(new URL(origin).hostname)) throw new Error(`exact snapback --port: ${origin} is not a loopback backend`);
    mkdirSync(resolve(dir,'.exact'), {recursive:true});
    const selected = `http://127.0.0.1:${port}`;
    writeFileSync(resolve(dir,'.exact/backend-origin'), selected + '\n');
    said.push(`Snapback origin: ${selected}; app.ts's origin constant is overridden at bake by .exact/backend-origin (EXACT_BACKEND_ORIGIN takes precedence).`);
  }
  const cli = resolve(parent, 'node_modules/.bin/snapback4'), generated = resolve(parent, 'snapback/generated/api.ts');
  if (!existsSync(resolve(parent, 'snapback/schema.q'))) said.push(`No backend above: ${parent}/snapback/schema.q is missing. Write one there (snapback4/README.md's guestbook is a start), then run this again.`);
  else if (!existsSync(cli)) said.push(`The backend's server is not installed in ${parent}: \`npm i snapback4@${version}\` there (or \`bun add\`), then \`bun exact.mjs snapback\` here.`);
  else if (!existsSync(generated)) {
    const check = spawnSync(process.execPath, [resolve(parent, 'node_modules/snapback4/bin/snapback4.js'), 'check'], { cwd: parent, encoding: 'utf8' });
    said.push(check.status === 0 && existsSync(generated) ? `snapback4 check: wrote ${parent}/snapback/generated/`
      : `snapback4 check in ${parent} failed; fix the schema, then run this again:\n${(check.stderr || check.stdout || '').trim()}`);
  }
  if (online && onlineStarter(dir, generated, port)) said.push('app.ts and app.contract: one generated query and mutation, with JSON argument editors. Replace them with your app fields; password sessions are in snapback4/README.md.');
  const text = readFileSync(manifestPath, 'utf8'), app = JSON.parse(text);
  const beforeSources = JSON.stringify(app.typescript);
  const sources = ((app.typescript ??= {}).sources ??= {});
  sources.snapback4 = driverMount(dir, online);
  if (existsSync(generated)) sources.backend = '../snapback/generated';
  const next = JSON.stringify(app, null, 2) + '\n';
  if (JSON.stringify(app.typescript) !== beforeSources) writeFileSync(manifestPath, next);
  const ignore = resolve(dir, '.gitignore');
  const ignored = existsSync(ignore) ? readFileSync(ignore, 'utf8') : '';
  const deviceIgnores = ['/assets/snapback4.wasm', '/assets/snapback4.wasm.json'].filter(path => !ignored.split('\n').includes(path));
  if (deviceIgnores.length) writeFileSync(ignore, `${ignored}${ignored.endsWith('\n') || !ignored ? '' : '\n'}${deviceIgnores.join('\n')}\n`);
  said.unshift(`app.json: ${JSON.stringify({ typescript: app.typescript })}`);
  if (!sources.backend) said.push('app.json mounts no `backend` yet: app.ts imports ./backend/api.ts once ../snapback/generated exists.');
  const ts = existsSync(resolve(dir, 'app.ts')) ? readFileSync(resolve(dir, 'app.ts'), 'utf8') : '';
  const appId = /export const appId\s*=\s*['"`]([^'"`]*)['"`]/.exec(ts)?.[1];
  if (appId !== app.app?.id) said.push(`app.ts must say: export const appId = ${JSON.stringify(app.app?.id)};${appId ? ` (it says ${JSON.stringify(appId)})` : ''}`);
  if (build && !online) {
    let compatible = false;
    try { compatible = readDeviceStamp(resolve(dir,'assets/snapback4.wasm')).version === version; } catch { /* Missing or malformed: rebuild. */ }
    if (rebuild || !compatible || port===null) buildSnapbackDevice(dir);
    said.push('assets/snapback4.wasm: built with assets/snapback4.wasm.json (the device the page loads)');
  }
  if (online) said.push('Online-only adapter: no wasm or SQLite partition; mutations go to the server, with no outbox or automatic retry.');
  const origin = /const origin\s*=\s*['"`](https?:\/\/[^'"`]+)['"`]/.exec(ts)?.[1];
  const at = port ?? (origin ? new URL(origin).port || 80 : 4400);
  said.push(`Before \`bun exact.mjs test web\`: \`bunx snapback4 dev --port ${at}\` in ${parent} (snapback4@${version}).`);
  // The notes' backend paragraph now describes a wired app.
  if (existsSync(resolve(dir, 'AGENTS.md'))) updateNotes(dir, basename(dir));
  if (rebuild && port === null && sources.backend && appId === app.app?.id && existsSync(cli))
    return `Snapback rebuilt: ./snapback4 → ${sources.snapback4}; ./backend → ${sources.backend}${online ? ' (online)' : '; assets/snapback4.wasm'}.`;
  return said.join('\n');
}

function buildSnapbackDevice(dir) {
  const built = spawnSync(process.execPath, [resolve(ROOT, 'snapback4/web/build.mjs'), 'assets/snapback4.wasm'], { cwd: dir, stdio: ['ignore', 'inherit', 'inherit'] });
  if (built.error) throw built.error;
  if (built.status !== 0) throw new Error('the device did not build (above); bun exact.mjs snapback --rebuild builds it again');
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
    exact_js_bake::build(std::path::Path::new(".."), "linux").unwrap_or_else(|error| {
        // What the bake found (tsc's and the compiler's diagnostics), as it is: no panic around it.
        eprintln!("{error}");
        std::process::exit(1);
    });
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

/** A new app's identity (LLP 1116.004 D8): its name and six random hex digits,
 * `com.example.notes-3f9a2c`, so two apps of one name (a bake-off's rounds, a
 * copy beside the first) never share a store on a device or a web origin. An
 * existing app keeps the id it has: `update` and `pattern` read it from app.json. */
export const newAppId = (name, random = randomBytes(3).toString('hex')) => `com.example.${name}-${random}`;

/** The three files \`exact new\` writes for the app itself: the view, its data
 * module and its tests. \`exact.mjs pattern\` replaces them, and reads them
 * to tell an untouched file from one the author edited (LLP 1116.001 D3). */
export function appSources(name, title, id = `com.example.${name}`) {
  return {
    'app.contract': `// ${title}: the view. app.ts answers what it asks for. Colours, fonts,
// sizes and the safe area are left unsaid, so each platform supplies its own:
// the header is the navigation bar, its heading the title, its button a bar item.
shape Greeting
  text: string

routes nav
  home "/"

component ${title.replaceAll(' ', '')}
  resource greeting = greeting("${title}") as shape Greeting
  state liked = false
  action like
    liked = not liked
  view
    main testId="root" navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` navigationScroll="content" position="absolute" inset=0 display="flex" flex-direction="column"
          header display="flex" align-items="center"
            text "${title}" role="heading" aria-level=1 flex=1
            button press=like aria-label=(liked ? "Unlike" : "Like") testId="like"
              image (liked ? "symbol:heart-fill" : "symbol:heart")
          scroll id="content" flex=1 min-height=0 padding=16
            text greeting.text testId="greeting"
`,
    'app.test.contract': `test "the greeting loads"
  expect tree has "root"
  expect text "greeting" == "Hello from ${title}."

test "the header's button likes it"
  tap "like"
  expect state liked == true
`,
    'app.ts': `import type { Answer, Result, Sources } from './app.contract.d.ts';

// app.json’s app.id: change the two together.
export const appId = '${id}';
export const grants = '';

const sources: Sources = {
  greeting: ([name]): Result<'greeting'> => ({ text: \`Hello from \${name}.\` }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
`,
  };
}

/** The starter patterns (LLP 1116.001 D3): each a working app under
 * docs/patterns/<name>/, its app.contract, app.ts and app.test.contract and any
 * file it needs beside them (rest's server.ts), compiled by the docs check and run
 * on the web by the async lane. A pattern.json there holds keys the app's app.json
 * gains (rest's `commands`). Its first line names the app `Starter` and says what it is. */
export const PATTERNS = resolve(ROOT, 'docs/patterns');
const PATTERN_ORDER = ['one-screen', 'list-detail', 'tabs', 'dashboard', 'rest'];

/** Each pattern's name and line, in the order to read them. */
export function patterns() {
  const rank = name => (PATTERN_ORDER.indexOf(name) + 1) || PATTERN_ORDER.length + 1;
  return readdirSync(PATTERNS).filter(name => existsSync(resolve(PATTERNS, name, 'app.contract')))
    .sort((a, b) => rank(a) - rank(b) || a.localeCompare(b))
    .map(name => ({ name, line: readFileSync(resolve(PATTERNS, name, 'app.contract'), 'utf8').split('\n')[0].replace(/^\/\/ Starter: /, '') }));
}

/** A pattern's files as an app writes them, by path: its placeholder id and name
 * (`com.example.starter`, `Starter`) become the app's; and the keys its pattern.json adds. */
export function patternFiles(pattern, { id, title }) {
  const dir = resolve(PATTERNS, pattern), files = {};
  const quoted = JSON.stringify(title).slice(1, -1).replaceAll('$', '\\$');
  for (const entry of readdirSync(dir, { recursive: true, withFileTypes: true })) {
    if (!entry.isFile() || entry.name === 'pattern.json' || entry.name === 'app.contract.d.ts') continue;
    const path = relative(dir, resolve(entry.parentPath, entry.name));
    let text = readFileSync(resolve(dir, path), 'utf8').replaceAll('com.example.starter', id);
    if (path.endsWith('.contract')) text = text.replaceAll('"Starter"', `"${quoted}"`).replace(/^\/\/ Starter: /, `// ${title}: `);
    files[path] = text;
  }
  const keys = existsSync(resolve(dir, 'pattern.json')) ? JSON.parse(readFileSync(resolve(dir, 'pattern.json'), 'utf8')) : {};
  return { files, keys };
}

/** `exact.mjs pattern <name> [--force]`: the pattern's files in place of the app's
 * own. A file is replaced when it is missing, or what `exact new` or a pattern wrote
 * for this app; one the author edited is refused, unless `force`. Another pattern's
 * files the author left alone go. Returns what it did and the next command. */
export function applyPattern(dir, pattern, { force = false } = {}) {
  const known = patterns().map(p => p.name);
  if (!known.includes(pattern)) throw new Error(`pattern: no pattern ${pattern}; there are ${known.join(', ')}`);
  const manifestPath = resolve(dir, 'app.json');
  if (!existsSync(manifestPath) || !existsSync(resolve(dir, 'app.contract'))) throw new Error(`pattern: ${dir} is not an app (no app.json and app.contract); run it in the directory exact new made`);
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const id = manifest.app?.id ?? manifest.id, title = manifest.app?.name ?? manifest.name;
  // The template's name is its crates', as `update` reads it: a renamed folder keeps it.
  const name = ['web', 'apple', 'linux'].map(kind => resolve(dir, kind, 'Cargo.toml')).filter(existsSync)
    .map(path => String(Bun.TOML.parse(readFileSync(path, 'utf8')).package?.name ?? '').match(/^(.+)-(?:web|apple|linux)$/)?.[1]).find(Boolean) ?? basename(dir);
  const template = appSources(name, name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' '), id);
  const others = known.filter(p => p !== pattern).map(p => patternFiles(p, { id, title }));
  const untouched = (path, text) => text === template[path] || others.some(o => o.files[path] === text);
  const { files, keys } = patternFiles(pattern, { id, title });
  const read = path => existsSync(resolve(dir, path)) ? readFileSync(resolve(dir, path), 'utf8') : null;
  const edited = Object.entries(files).filter(([path, text]) => { const now = read(path); return now !== null && now !== text && !untouched(path, now); }).map(([path]) => path);
  if (edited.length && !force) throw new Error(`pattern: ${edited.join(', ')} ${edited.length > 1 ? 'were' : 'was'} edited (not what exact new or a pattern wrote); --force replaces ${edited.length > 1 ? 'them' : 'it'}`);
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(resolve(dir, path)), { recursive: true });
    writeFileSync(resolve(dir, path), text);
  }
  // Another pattern's files and commands the author left as written go with it.
  const gone = [];
  for (const other of others) for (const [path, text] of Object.entries(other.files)) {
    if (!(path in files) && read(path) === text) { rmSync(resolve(dir, path)); gone.push(path); }
  }
  const commands = { ...(manifest.commands ?? {}) };
  for (const other of others) for (const [verb, argv] of Object.entries(other.keys.commands ?? {})) {
    if (!keys.commands?.[verb] && JSON.stringify(commands[verb]) === JSON.stringify(argv)) delete commands[verb];
  }
  Object.assign(commands, keys.commands ?? {});
  if (JSON.stringify(commands) !== JSON.stringify(manifest.commands ?? {})) {
    const next = { ...manifest, commands };
    if (!Object.keys(commands).length) delete next.commands;
    writeFileSync(manifestPath, JSON.stringify(next, null, 2) + '\n');
  }
  const first = ['app.contract', 'app.ts', 'app.test.contract'];
  const wrote = [...first.filter(path => path in files), ...Object.keys(files).filter(path => !first.includes(path)).sort()];
  const verbs = Object.keys(keys.commands ?? {});
  return { said: `${pattern}: wrote ${wrote.join(', ')} as ${title} (${id})${verbs.length ? `; app.json commands: ${verbs.join(', ')}` : ''}${gone.length ? `; removed ${gone.sort().join(', ')}` : ''}`,
    next: `Then: bun exact.mjs ${verbs[0] ?? 'test'} web` };
}

/** What `exact.mjs pattern` with no name prints. */
export function patternList() {
  const list = patterns(), width = Math.max(...list.map(p => p.name.length));
  return ['bun exact.mjs pattern <name> [--force]   writes a working, tested starting app here:', '',
    ...list.map(p => `  ${p.name.padEnd(width)}  ${p.line}`), '',
    'It replaces app.contract, app.ts and app.test.contract (refusing a file you edited unless',
    '--force), with this app\'s id and name. Then: bun exact.mjs test web.'].join('\n');
}

function writeApp(dir, name) {
  for (const host of ['apple', 'linux', 'web']) mkdirSync(resolve(dir, host));
  const title = name.split('-').map(w => w[0].toUpperCase() + w.slice(1)).join(' '), id = newAppId(name);
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
      name: title, short_name: title, id, start_url: '/', display: 'standalone',
      app: { id, name: title },
      host: { ios: { minimumOS: '17.0', deviceFamily: ['iphone', 'ipad'] }, macos: { minimumOS: '14.0', window: { width: 900, height: 700 } }, web: {} },
      deploy: { store: { web: '0', macos: '0', ios: '0', linux: '0' } },
    }, null, 2) + '\n',
    ...appSources(name, title, id),
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
exact-apple-update = ${dep('apple', 'host/apple-update')}
exact-js = ${dep('apple', 'js')}

[build-dependencies]
exact-js-bake = ${dep('apple', 'js/bake')}
`,
    'apple/build.rs': `fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(std::path::Path::new(".."), platform).unwrap_or_else(|error| {
        // What the bake found (tsc's and the compiler's diagnostics), as it is: no panic around it.
        eprintln!("{error}");
        std::process::exit(1);
    });
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
// The host app.json's deploy.store names: with an update store, signed updates.
include!(concat!(env!("OUT_DIR"), "/host.rs"));
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
    exact_js_bake::build(std::path::Path::new(".."), "web").unwrap_or_else(|error| {
        // What the bake found (tsc's and the compiler's diagnostics), as it is: no panic around it.
        eprintln!("{error}");
        std::process::exit(1);
    });
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
  chmodSync(resolve(dir, 'exact.mjs'), 0o755);
  linkClaude(dir);
  editorTasks(dir);
  writeFileSync(resolve(dir, 'Cargo.lock'), readFileSync(resolve(ROOT, 'Cargo.lock')));
  const deferred = resolveOffline(dir, true);
  const run = 'bun exact.mjs';
  return `Created ${dir}
  cd '${dir.replaceAll("'", "'\\''")}'
  ${run} pattern      start from a working app: ${patterns().map(p => p.name).join(', ')}
  ${run} web          the web dev loop
  ${run} test web     build, then run app.test.contract, or the files named (web, ios or macos)
  ${run} verify       build, the tests, check where each ends, relaunch: done when clean
  ${run} agent web tree  inspect or drive the app
  ${run} ios --run    build and launch on an iOS simulator
  ${run} mac --run    build and launch on this Mac
  ${run} linux        build the Linux host (then test linux / agent linux, headless anywhere)
  ${run} help         every verb; ${run} <verb> --help gives its flags and an example${deferred ? `
Cargo.lock is still exact2's: this machine's Cargo cache lacks some of its crates, so the
first build resolves it, fetching them once (it needs the network then, not now).` : ''}`;
}

const BEGIN = '<!-- exact:begin (exact new writes this block; bun exact.mjs update rewrites it) -->', END = '<!-- exact:end -->';

/** The notes' backend paragraph: an app `bun exact.mjs snapback` wired (app.json mounts `snapback4`), or how to wire one. */
function snapbackNotes(dir) {
  const readme = pathFrom(dir, resolve(ROOT, 'snapback4/README.md'));
  let wired = false, online = false;
  try { const mount = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).typescript?.sources?.snapback4; wired = Boolean(mount); online = onlineMount(mount); } catch {}
  if (online) return `A backend: this app uses the server-only Snapback adapter; read ${readme} before changing data code.
\`app.json\` mounts \`snapback4/online\` as \`snapback4\`, and the generated operations as \`backend\`.
Use \`new Snapback({origin, headers})\`, \`read(api.query, args)\` and \`send(api.mutation, args)\`.
There is no wasm device, SQLite partition or offline outbox. Start the server with \`bun exact.mjs backend\`.
\`snapback\` and \`update\` preserve this adapter selection. \`failure(x).code\` preserves a domain refusal code.`;
  if (wired) return `A backend: this app's data is Snapback 4's (${readme}). The backend is above it:
\`../snapback/schema.q\`, with \`snapback4@${snapbackVersion()}\` installed. \`app.json\` mounts the driver and
\`../snapback/generated\`; \`app.ts\` names and grants its origin. \`bun exact.mjs snapback --rebuild\` generates operations
and rebuilds the device, including after exact2 moves; \`snapback --port <n>\` moves the origin. Start
\`bunx snapback4 dev --port <the origin's port>\` above before \`test web\`. \`fresh\` resets it per test
(Snapback4 ${snapbackVersion()}, dev only). Typed reads use \`db.read(api.recent, {}, now)\`; \`send\` writes, syncs and answers
the write's fate. \`failure(x).reason\` carries a refusal's code. Read the guide before data code.`;
  return `Shared data: \`exact new <path> --snapback\`, or \`bun exact.mjs snapback\` here.
Install \`snapback4@${snapbackVersion()}\` above; ${readme} has setup and server commands.`;
}

/** What an agent in the app's directory can't discover (LLP 1086 D1): what
 * its files are, where to start and the one guide, at most 1.5 KB because every step
 * carries it (LLP 1116.007 D2). It orders no step: builders load this file as
 * instructions, and each instruction they follow re-sends their context (LLP 1116.012 D1). Paths are from the app to
 * this checkout, so \`update\` follows a checkout that moved. The authoring
 * diary's instructions are \`feedback status\`'s to print, for a project that
 * keeps one (LLP 1116 D3), so a project that doesn't never carries them. */
function agentNotes(dir, name, {game = false} = {}) {
  const doc = file => pathFrom(dir, resolve(ROOT, 'docs', file));
  const sdk = file => pathFrom(dir, resolve(ROOT, file));
  if (game) return gameNotes(dir, name, doc);
  return `${BEGIN}
# ${name}: an Exact app

\`app.contract\` is the view, \`app.ts\` its data module, \`app.test.contract\` its tests, \`app.json\`
the manifest. The app uses the exact2 checkout at \`${pathFrom(dir, ROOT)}\` by path (\`EXACT2\`
overrides it).

This file is already in your instructions. Start from the nearest pattern: \`bun exact.mjs
pattern\` lists them and \`pattern <name>\` writes one, tested. ${doc('start-here.md')} is the guide,
the only required reading (LLP 1115 D7). When a name is in question, \`bun exact.mjs contract
vocab <name>\` answers it (no name: the card, which ${doc('card.md')} holds too). Run
\`bun exact.mjs verify\` when the features work, and \`verify ios\` once at the end for an iPhone
app. \`bun exact.mjs help\` lists the verbs, and your own in app.json \`"commands"\`.
\`bun exact.mjs update --without linux\` stops shipping Linux.
\`bun ${sdk('scripts/exact.mjs')} setup --check\` names what a native build lacks.
${DIARY_LINE}
${snapbackNotes(dir)}

Generated, so don't edit: \`exact.mjs\`, \`rust-toolchain.toml\`, \`.cargo/config.toml\`,
\`Cargo.toml\`'s \`[patch.crates-io]\` and this block.
${END}
`;
}

/** The one line that stands for the authoring diary (docs/diary.md): \`feedback
 * status\` prints the instructions when the project keeps a diary. */
const DIARY_LINE = 'The authoring diary: `bun exact.mjs feedback status` says whether this project keeps one.';

/** A game's notes: the app's, for a world in Rust under Contract's menus. */
function gameNotes(dir, name, doc) {
  const sdk = file => pathFrom(dir, resolve(ROOT, file));
  return `${BEGIN}
# ${name}: an Exact game

The world is Rust: \`logic/src/lib.rs\` implements \`Game\` (its \`Options\` are the
canvas's arguments, \`setup\` and \`tick\` its gameplay). Menus, the HUD and accessible
controls are \`app.contract\` (Contract). \`app.json\` is optional and holds only keys
you author (a title, \`game.audio\`, \`game.assets\`, a data crate). The bake generates
the hosts under \`.shells/\` (ignored; never edit it), and so does \`bun exact.mjs test-rust\`:
run that once before \`cargo\` in \`render/\` or a render example, whose workspace it is. The game uses the exact2 checkout
at \`${pathFrom(dir, ROOT)}\` by path (\`EXACT2\` overrides it).

Read before writing code:

- ${sdk('game/README.md')}: the game target — the programming model, input, saves, determinism, the proof.
- ${sdk('game/engine/README.md')}: the engine's API.
- ${doc('contract-for-agents.md')}: Contract, for the menus and HUD.
- ${doc('agent-pitfalls.md')}: verified footguns, symptom → cause → fix.
- ${doc('contract-grammar.md')}: exact forms, built-in functions, events (keys at a game's canvas too).

Commands, from this directory (\`exact.mjs\` runs from any other too, and finds a file
you name where you are, else here):

| | |
|---|---|
| \`bun exact.mjs <verb> --help\`, \`bun exact.mjs help\` | a verb's flags, whether it stays in the foreground, and an example (\`--help all\`: every flag, its ports); the verbs |
| \`bun exact.mjs test-rust\` | the hostless Rust tests (\`logic/tests\`) and the determinism lints |
| \`bun exact.mjs contract build app.contract --json\` | compile; \`[]\` or every diagnostic with its range |
| \`bun exact.mjs web\` | the web dev loop, at the URL it prints (a game builds the wasm target) |
| \`bun exact.mjs test web\` | build the web game if needed, then run \`app.test.contract\` (also \`macos\`, \`ios\`) |
| \`bun exact.mjs agent web "tap play" "type world key ArrowRight for 800" "screenshot out.png"\` | drive the game as a person would |
| \`bun exact.mjs mac --run\`, \`bun exact.mjs ios --run\` | build and launch natively |
| \`bun exact.mjs linux\`, then \`test linux\` or \`agent linux …\` | build the Linux host and drive it headless (any machine, no display); \`bun exact.mjs android\` builds the same host for Android (LLP 1107) |
| \`bun exact.mjs windows --run\` | build and launch the standalone Windows game; omit \`--run\` to package only |
| \`bun exact.mjs prove\`, then \`bun proof.mjs web\` | the proof: a first baseline in \`pins.json\`, then real-host checks against it |
| \`bun exact.mjs update\` | after exact2 moves; it rewrites \`exact.mjs\` and this block, and says when \`Cargo.lock\` needs \`lock\` |
| \`bun exact.mjs lock\` | when a build says exact2 moved since the game's \`Cargo.lock\` was captured, or after adding a dependency: fetches (the network), resolves again and prints what changed; builds never change the lock |
| \`bun exact.mjs shells\` | write \`.shells/\`, the generated workspace a crate here names, without a build (\`cargo run --example\` in \`render/\` needs it) |
| app.json \`"commands": {"replay": ["bun", "tools/replay.mjs"]}\` | the game's own verbs: \`bun exact.mjs replay web\` runs \`bun tools/replay.mjs web\` here |

The loop: \`test-rust\` while tuning gameplay, \`contract build --json\` until it prints
\`[]\`, \`test web\`, look at it with \`agent web … screenshot\`, then the native hosts.
\`proof.mjs\` asserts the starter's gameplay (its beacons, KeyE): rewrite it when you
change \`Options\` or the scene, before the first \`prove\`.
\`bun ${sdk('scripts/exact.mjs')} setup --check\` names anything this machine is missing.

Generated, so don't edit: \`exact.mjs\` and this block.

${DIARY_LINE}
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

/** The app's command runner: a launcher that finds the checkout (EXACT2 names it once, D3; the default
 * is the one that created the app, so a sibling checkout needs nothing) and hands the verb to its
 * scripts/app-cli.mjs, which holds the verbs (LLP 1116.006 D7), so a builder reading it reads a few lines. */
function commandsFor(dir, name, {game = false} = {}) {
  return `#!/bin/sh
':' //; if [ -n "$BUN" ]; then exec "$BUN" "$0" "$@"; elif [ -x "$HOME/.bun-${JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).packageManager.replace('bun@', '')}/bin/bun" ]; then exec "$HOME/.bun-${JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).packageManager.replace('bun@', '')}/bin/bun" "$0" "$@"; else exec ${"'" + process.execPath.replaceAll("'", "'\\''") + "'"} "$0" "$@"; fi
// ${name}'s commands: \`bun exact.mjs --help\` lists the verbs, \`bun exact.mjs <verb> --help\` a verb's flags.
// They live in the exact2 checkout named by EXACT2 (default ${pathFrom(dir, ROOT)}), in its
// scripts/app-cli.mjs. Generated by \`exact new\`; \`bun exact.mjs update\` rewrites this file, the crates.io
// patches, the toolchain and the exact2 dependency paths. The app's own verbs are app.json's \`commands\`.
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

const EXACT2 = resolve(import.meta.dir, process.env.EXACT2 ?? ${JSON.stringify(pathFrom(dir, ROOT))});
const cli = resolve(EXACT2, 'scripts/app-cli.mjs');
if (!existsSync(cli)) {
  console.error(\`no exact2 at \${EXACT2} (no scripts/app-cli.mjs): EXACT2 names the checkout\`);
  process.exit(1);
}
const { ensureBun } = await import(resolve(EXACT2, 'scripts/bun.mjs'));
ensureBun(${JSON.stringify(JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).packageManager.replace('bun@', ''))});
const { run } = await import(cli);
process.exit(await run({ dir: import.meta.dir, name: ${JSON.stringify(name)},${game ? ' game: true,' : ''} exact2: EXACT2, args: process.argv.slice(2) }));
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

/** A workspace member's path as Cargo compares it: `./linux/` is `linux`. */
const memberPath = m => String(m).replace(/^(?:\.\/)+/, '').replace(/\/+$/, '');

/** `text`, a workspace Cargo.toml, without `member` in the `[workspace]` table's own
 * `members` and `default-members`: each string naming it (either quote, `./`, a multiline
 * array with comments) goes with its comma, and the rest of the file is left as written.
 * Only a form it reads with certainty is edited: one `[workspace]` header, bare keys, plain
 * strings. Anything else is refused, naming the edit to make by hand, and so is a result
 * that parses to anything but the old document less that member. */
export function withoutMember(text, member) {
  const refuse = why => { throw new Error(`Cargo.toml's [workspace] is in a form this does not edit (${why}): remove "${member}" from [workspace] members and default-members by hand, then run it again`); };
  let doc;
  try { doc = Bun.TOML.parse(text); } catch (error) { refuse(`it does not parse: ${error.message}`); }
  const named = l => Array.isArray(l) && l.some(m => memberPath(m) === member);
  if (!named(doc.workspace?.members) && !named(doc.workspace?.['default-members'])) return text;
  const expected = structuredClone(doc);
  for (const key of ['members', 'default-members']) if (Array.isArray(expected.workspace[key])) expected.workspace[key] = expected.workspace[key].filter(m => memberPath(m) !== member);
  if (expected.workspace['default-members']?.length === 0) refuse('default-members would be empty');
  // The `[workspace]` table's body: from its one header to the next table's.
  const headers = [...text.matchAll(/^[ \t]*\[[ \t]*workspace[ \t]*\][ \t]*(?:#[^\n]*)?$/gm)];
  if (headers.length !== 1) refuse(headers.length ? 'two [workspace] headers' : 'no [workspace] header');
  const from = headers[0].index + headers[0][0].length;
  // A line opening with `[` inside a value ends the body early; the check below then refuses.
  const next = /^[ \t]*\[/m.exec(text.slice(from));
  const to = next ? from + next.index : text.length;
  let body = text.slice(from, to);
  if (/^[ \t]*["'](?:default-)?members["']/m.test(body)) refuse('a quoted members key');
  for (const key of ['members', 'default-members']) {
    const head = new RegExp(`^[ \\t]*${key}[ \\t]*=[ \\t]*\\[`, 'm').exec(body);
    if (!head) continue;
    // The array's string tokens, to its `]`, past quotes and `#` comments.
    const tokens = [];
    let i = head.index + head[0].length;
    for (; i < body.length && body[i] !== ']'; i++) {
      if (body[i] === '#') { while (i < body.length && body[i] !== '\n') i++; continue; }
      if (/\s|,/.test(body[i])) continue;
      if (body[i] !== '"' && body[i] !== "'") refuse(`${key} holds something other than a string`);
      if (body.startsWith('"""', i) || body.startsWith("'''", i)) refuse(`${key} holds a multiline string`);
      const quote = body[i], start = i;
      for (i++; i < body.length && body[i] !== quote && body[i] !== '\n'; i++) if (body[i] === '\\') refuse(`${key} holds an escaped string`);
      if (body[i] !== quote) refuse(`${key} holds an unclosed string`);
      tokens.push({ start, end: i + 1, value: body.slice(start + 1, i) });
    }
    if (i >= body.length) refuse(`${key} is not closed`);
    let out = body;
    for (const t of tokens.reverse().filter(t => memberPath(t.value) === member)) {
      // Its comma after it, else the one before it (the last element); a line it had to
      // itself goes whole, with the comment on it.
      const after = /^[ \t]*,/.exec(out.slice(t.end));
      if (!after) {
        const comma = /,[ \t\r\n]*$/.exec(out.slice(0, t.start));
        out = out.slice(0, comma ? comma.index : t.start) + out.slice(t.end);
        continue;
      }
      const lineStart = out.lastIndexOf('\n', t.start - 1) + 1, rest = out.slice(t.end + after[0].length);
      const tail = /^[ \t]*(?:#[^\n]*)?\n/.exec(rest);
      out = /^[ \t]*$/.test(out.slice(lineStart, t.start)) && tail
        ? out.slice(0, lineStart) + rest.slice(tail[0].length)
        : out.slice(0, t.start) + rest.replace(/^[ \t]+/, '');
    }
    body = out;
  }
  const out = text.slice(0, from) + body + text.slice(to);
  let edited = null;
  try { edited = Bun.TOML.parse(out); } catch {}
  if (JSON.stringify(edited) !== JSON.stringify(expected)) refuse('the edit would change more than that member');
  return out;
}

/** `update --without <platform>`: the app stops building and shipping it. Its workspace
 * member goes, with app.json's `host`, `deploy.store` and `deploy.binaries` keys for it,
 * which are what the compiler reads as shipping it (contract/lower/src/icons.rs `targets`).
 * Its crate is moved aside, Cargo.lock is re-resolved without it, and only then is the
 * crate deleted. Any failure before that puts Cargo.toml, app.json, Cargo.lock (or its
 * absence) and the crate back as they were; a failed delete leaves the app without the
 * platform and says what to delete. Returns what it removed. */
function dropPlatform(dir, platform) {
  if (platform === 'web') throw new Error('update --without web: the web host is the dev loop\'s and the agent\'s (`web`, `test web`, `agent web`), so it stays');
  if (!['linux', 'android'].includes(platform)) throw new Error(`update --without ${platform}: it takes linux (which Android builds too) or android`);
  const workspacePath = resolve(dir, 'Cargo.toml'), manifestPath = resolve(dir, 'app.json');
  const workspace = readFileSync(workspacePath, 'utf8'), manifestText = existsSync(manifestPath) ? readFileSync(manifestPath, 'utf8') : null;
  // Each file as it was, `null` for one that was absent; `cargo metadata` may rewrite the lock before it fails.
  const saved = [workspacePath, manifestPath, resolve(dir, 'Cargo.lock')].map(path => [path, existsSync(path) ? readFileSync(path) : null]);
  const crate = resolve(dir, platform), aside = resolve(dir, `.${platform}-removed-${randomBytes(4).toString('hex')}`);
  let moved = false;
  const removed = [];
  try {
    const members = withoutMember(workspace, platform);
    if (members !== workspace) { writeFileSync(workspacePath, members); removed.push('its workspace member'); }
    if (manifestText !== null) {
      const manifest = JSON.parse(manifestText), deploy = manifest.deploy ?? {};
      for (const [key, table] of [['host', manifest.host], ['deploy.store', deploy.store], ['deploy.binaries', deploy.binaries]]) {
        if (table && platform in table) { delete table[platform]; removed.push(`app.json ${key}.${platform}`); }
      }
      // A deploy naming no native platform is read as every one (scripts/deploy.mjs `nativePlatforms`), Linux included.
      const native = [...Object.keys(deploy.store ?? {}), ...Object.keys(deploy.binaries ?? {})].filter(p => p !== 'web');
      if ((deploy.store || deploy.binaries) && !native.length) throw new Error(`app.json deploy.store would name no native platform, which deploy reads as all of them, ${platform} included: name the ones the app ships there (as \`"ios": "0"\`) first`);
      writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
    }
    if (existsSync(resolve(crate, 'Cargo.toml'))) { renameSync(crate, aside); moved = true; }
    resolveOffline(dir);
  } catch (error) {
    const unrestored = [];
    for (const [path, bytes] of saved) {
      try { if (bytes === null) rmSync(path, { force: true }); else writeFileSync(path, bytes); } catch { unrestored.push(relative(dir, path)); }
    }
    if (moved) try { renameSync(aside, crate); } catch { unrestored.push(`${platform}/ (it is at ${relative(dir, aside)})`); }
    throw new Error(`update --without ${platform}: ${error.message}; ${unrestored.length ? `could not put back ${unrestored.join(', ')}` : `Cargo.toml, app.json, Cargo.lock and ${platform}/ are as they were`}`);
  }
  if (moved) {
    try { rmSync(aside, { recursive: true, force: true }); removed.push(`${platform}/`); } catch (error) {
      removed.push(`${platform}/ from the workspace; deleting its files failed (${error.message}), so delete ${relative(dir, aside)} by hand`);
    }
  }
  return removed.length ? `without ${platform} (removed ${removed.join(', ')})` : `without ${platform} (it shipped none already)`;
}

function updateApp(dir, name, { without = null } = {}) {
  // A game has no workspace of its own (the bake generates .shells/): its runner and notes are all there is to rewrite.
  if (existsSync(resolve(dir, 'app.contract')) && !existsSync(resolve(dir, 'Cargo.toml')) && gameDefaults(dir)) {
    if (without) throw new Error('update --without: a game\'s hosts are its bake\'s (game/README.md), not crates of its own');
    writeFileSync(resolve(dir, 'exact.mjs'), commandsFor(dir, name, {game:true}));
    const notes = updateNotes(dir, name, {game:true});
    if (editorTasks(dir, {game:true})) notes.push('.vscode/tasks.json');
    // A captured lock exact2 has moved past is said now, not at the next build (LLP 1046.011 D2).
    let drift = null;
    if (existsSync(resolve(dir, 'Cargo.lock'))) try {
      gameShells(dir, gameDefaults(dir).game, resolve(ROOT, 'game'));
      drift = capturedLockDrift(dir);
    } catch { /* The next build says what is wrong. */ }
    return `Updated ${dir}: exact.mjs${notes.length ? `, ${notes.join(', ')}` : ''}${drift ? `\n${drift}` : ''}`;
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
  chmodSync(resolve(dir, 'exact.mjs'), 0o755);
  const dropped = without ? dropPlatform(dir, without) : null;
  // An app made before the scaffold wrote a Linux host gains one (LLP 1086), unless it has its own
  // or ships no Linux: app.json deploy.store.linux, which the scaffold writes and `--without linux`
  // removes, says it does.
  let storesLinux = false;
  try { storesLinux = 'linux' in (JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).deploy?.store ?? {}); } catch {}
  const addedLinux = storesLinux && !existsSync(resolve(dir, 'linux'));
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
    let next = text.replace(/("\$schema"\s*:\s*)"[^"]*"/, (_, head) => head + JSON.stringify(pathFrom(dir, resolve(ROOT, 'scripts/app.schema.json'))));
    // A Snapback app's driver mount follows the checkout, as the Cargo paths below do (`bun exact.mjs snapback` wrote it).
    next = next.replace(/("snapback4"\s*:\s*)"[^"]*"/, (line, head) => JSON.parse(next).typescript?.sources?.snapback4 ? head + JSON.stringify(driverMount(dir, onlineMount(JSON.parse(next).typescript.sources.snapback4))) : line);
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
  return `Updated ${dir}: patches, toolchain, exact.mjs${addedLinux ? ', a linux/ host crate' : ''}${dropped ? `, ${dropped}, Cargo.lock` : ''}${notes.length ? `, ${notes.join(', ')}` : ''}${changed.length ? `, exact2 paths in ${changed.join(', ')}` : ''}`;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args[0] === '--app') console.log(createApp(args.find((a, i) => i > 0 && !a.startsWith('--')), { update: args.includes('--update') }));
  else console.log(createGame(args[0], undefined, {assets:args.includes("--assets"), render:args.includes("--render")}));
}
