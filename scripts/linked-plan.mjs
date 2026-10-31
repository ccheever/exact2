/**
 * Apple builds bake replaceable artifacts before Cargo and link them into
 * __TEXT. Cargo sees only generated Rust inputs, so a plan edit leaves the
 * archive untouched. TypeScript apps use exact-js-bake; Rust apps register
 * their own compiled build-script executable through exact_bake::take_linked.
 *
 * A bake runs again when a watched path or variable changes, or its tool
 * changes. The link carries the plan, compatibility document and bytecode
 * through exact_apple::baked.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargoEnvironment, developmentBuildEnv } from './app.mjs';
import { webOnly } from './app-cli.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const TOOL_TARGET = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(ROOT, 'target');

/** The bake tool (`exact-js-bake`), built once in this checkout's target and
 * shared by every app built from it: the command that builds it (a no-op when
 * built), and where it is. */
export const bakeTool = { command: 'cargo', args: ['build', '-q', '--profile', 'host-dev', '-p', 'exact-js-bake'], built: resolve(TOOL_TARGET, 'host-dev/exact-js-bake'),
  // Built with the build's own Mac deployment target when one asks (`bakeOutside`), as its host side shares the target directory.
  options: (build = {}) => ({ cwd: ROOT, env: { ...developmentBuildEnv(), ...(build.MACOSX_DEPLOYMENT_TARGET ? { MACOSX_DEPLOYMENT_TARGET: build.MACOSX_DEPLOYMENT_TARGET } : {}), EXACT_UPDATE_TRUST: 'development', CARGO_TARGET_DIR: TOOL_TARGET } }) };

/** The Apple build script `exact new` writes: it names the platform and
 * bakes, and does nothing else a bake made before Cargo would miss. */
export const APPLE_BUILD_SCRIPT = `fn main() {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        // tvOS and watchOS reuse the manifest's iOS policy, including interpreted Rust updates.
        Ok("ios" | "tvos" | "watchos") => "ios",
        _ => "macos",
    };
    exact_js_bake::build(std::path::Path::new(".."), platform).unwrap_or_else(|error| {
        // What the bake found (tsc's and the compiler's diagnostics), as it is: no panic around it.
        eprintln!("{error}");
        std::process::exit(1);
    });
}
`;

/** APPLE_BUILD_SCRIPT with the app at `app` from the crate: `..` for an authored crate beside app.json, `../../..` for
 * one generated under `.exact/shells/` (scripts/app-shells.mjs, LLP 1153 D1). */
export const appleBuildScript = (app = '..') => APPLE_BUILD_SCRIPT.replace('std::path::Path::new("..")', `std::path::Path::new(${JSON.stringify(app)})`);

/** The one `exact new` wrote before, which mapped iOS alone; `update`
 * replaces it with APPLE_BUILD_SCRIPT. */
export const EARLIER_APPLE_BUILD_SCRIPT = `fn main() {
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
`;

/** Whether this Apple entry can read a linked plan. Standard TypeScript
 * entries bake with exact-js-bake; Rust build scripts opt in with
 * exact_bake::take_linked and register their own compiled executable.
 * Other scripts and entries declaring their own PLAN/COMPAT still embed.
 * Embedding archives keep their plans, since their caller supplies no
 * linked sections. Widget builds link the plan into their shared framework. */
export function linkable(app, pkg) {
  const read = (target) => { try { return readFileSync(target.src_path, 'utf8'); } catch { return null; } };
  const script = pkg.targets.find(t => t.kind.includes('custom-build')), entry = pkg.targets.find(t => t.kind.some(k => ['lib', 'rlib', 'staticlib'].includes(k)));
  if (!script || !entry) return false;
  if (read(script)?.includes('exact_bake::take_linked(')) return !/^(?:pub )?const (?:PLAN|COMPAT):/m.test(read(entry) ?? '');
  if (!existsSync(resolve(app.dir, 'app.ts')) || existsSync(resolve(app.dir, 'data/Cargo.toml'))) return false;
  if (![APPLE_BUILD_SCRIPT, EARLIER_APPLE_BUILD_SCRIPT, appleBuildScript('../../..')].includes(read(script))) return false;
  const source = read(entry) ?? '';
  if (!/^const PLAN:/m.test(source) || !/^const COMPAT:/m.test(source)) return true;
  console.error(`host/apple: ${entry.src_path} declares PLAN and COMPAT, so the plan is compiled into the archive and a Contract edit compiles it again; \`bun exact.mjs update\` drops them, and the link then carries the plan`);
  return false;
}

/** Where an Apple build for `target` keeps the bake it links. */
export const linkedBakeDir = (env, platform, target) => resolve(env.EXACT_BAKE_OUTPUT, `${platform}-${target}.baked`);

/** The linker arguments that put a linked bake's plan, compat.json and module bytecode in the executable; the
 * bytecode 16-byte aligned, which Hermes needs to read it in place (js/src/shim.cc). */
export const linkedPlanArgs = (dir) => !dir ? [] : [...[['__exact_plan', 'app.plan'], ['__exact_compat', 'compat.json'], ['__exact_hbc', 'app.hbc']]
  .flatMap(([section, file]) => ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', section, '-Xlinker', resolve(dir, file)]),
  '-Xlinker', '-sectalign', '-Xlinker', '__TEXT', '-Xlinker', '__exact_hbc', '-Xlinker', '0x10'];

/** The time of the newest change at `path`: its own, a link's target's, and
 * for a directory every entry's under it (a link in it is not followed);
 * null when it is missing. */
function newest(path, watched = true) {
  let own; try { own = lstatSync(path); } catch { return null; }
  let at = own.mtimeMs, info = own;
  if (own.isSymbolicLink()) { try { info = statSync(path); at = Math.max(at, info.mtimeMs); } catch { return at; } }
  if (info.isDirectory() && (watched || !own.isSymbolicLink())) for (const name of readdirSync(path)) at = Math.max(at, newest(resolve(path, name), false) ?? at);
  return at;
}

/** The digest of a bake's plan and compat.json together, or null when either is missing. */
function pairOf(dir) {
  try { return createHash('sha256').update(readFileSync(resolve(dir, 'app.plan'))).update(readFileSync(resolve(dir, 'compat.json'))).digest('hex'); } catch { return null; }
}

/** Whether the bake `record` describes is the one its inputs would make now,
 * and the plan and compat.json beside it are the pair it made. */
function fresh(record, dir, env, tool) {
  if (record?.version !== 1 || record.tool !== tool || !record.pair || record.pair !== pairOf(dir)) return false;
  if (Object.entries(record.env).some(([name, value]) => (env[name] ?? null) !== value)) return false;
  return record.paths.every(path => { const at = newest(path); return at !== null && at < record.at; });
}

/** Bakes `app` for `platform` and `target` into its linked bake directory,
 * unless the bake there is fresh; the directory. A bake that fails throws
 * what it said, as the build script's failure ends a build. `prepareRust`
 * refreshes the compiled baker through Cargo before it reads new inputs.
 * The caller holds the app's build claim (host/apple/build.mjs `appleBuildLock`), the one
 * writer of the directory. */
export function bakeOutside(app, platform, target, env, prepareRust) {
  const dir = linkedBakeDir(env, platform, target), at = resolve(dir, 'inputs.json');
  let executable = bakeTool.built, args = [app.dir, '--cargo', platform, target, dir], cwd = app.dir, runEnv = cargoEnvironment(env);
  const rust = existsSync(resolve(app.dir, 'data/Cargo.toml'));
  if (rust) {
    prepareRust?.(dir);
    // Cargo registers the app's own compiled build script on its first run.
    // It also refreshes that registration and bake when Rust inputs change.
    const registry = resolve(dir, 'rust-baker.json');
    if (!existsSync(registry)) return dir;
    const baker = JSON.parse(readFileSync(registry, 'utf8'));
    if (!existsSync(baker.executable)) return dir;
    executable = baker.executable; args = []; cwd = baker.cwd;
    runEnv = { ...runEnv, ...baker.env, OUT_DIR: dir, EXACT_BAKED: dir, EXACT_RUST_BAKE: '1' };
  } else {
    const built = spawnSync(bakeTool.command, bakeTool.args, { ...bakeTool.options(env), stdio: ['ignore', 'inherit', 'inherit'] });
    if (built.status !== 0) throw new Error(`the bake tool did not build (${bakeTool.command} ${bakeTool.args.join(' ')})`);
  }
  const tool = `${executable} ${statSync(executable).mtimeMs}`;
  // Its warnings are said on every build, as Cargo replays a build script's: the SF Symbols only the web draws blank
  // one line where the first was, as `contract build` says them (18 of them every euxy build).
  const warn = (warnings) => { for (const line of bakeWarnings(app.name, warnings ?? [])) console.error(line); };
  let record = null; try { record = JSON.parse(readFileSync(at, 'utf8')); } catch { /* none yet */ }
  if (fresh(record, dir, runEnv, tool)) { warn(record.warnings); return dir; }
  rmSync(at, { force: true });
  const started = Date.now();
  const r = spawnSync(executable, args, { cwd, env: runEnv, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  const lines = (r.stdout ?? '').split('\n'), said = (key) => lines.map(line => new RegExp(`^cargo::?${key}=(.*)$`).exec(line)?.[1]).filter(value => value !== undefined);
  warn(said('warning'));
  if (r.status !== 0) {
    const error = new Error(`${app.name}: the bake failed:\n${(r.stderr || r.error?.message || `exit ${r.status}`).trim()}`);
    error.stack = error.message; error.contract = true;
    throw error;
  }
  if (rust) writeFileSync(resolve(dir, 'rust-output.txt'), r.stdout ?? '');
  record = { version: 1, at: started, tool, pair: pairOf(dir), warnings: said('warning'), env: Object.fromEntries(said('rerun-if-env-changed').map(name => [name, runEnv[name] ?? null])), paths: said('rerun-if-changed').map(path => resolve(cwd, path)) };
  mkdirSync(dir, { recursive: true });
  writeFileSync(`${at}.${process.pid}.tmp`, JSON.stringify(record));
  renameSync(`${at}.${process.pid}.tmp`, at);
  return dir;
}

/** Publish the watches of a Rust bake bootstrapped or refreshed by Cargo.
 * A first build must have this record before its driver checks freshness. */
export function recordRustBake(env) {
  const dir = env.EXACT_BAKED;
  if (!dir || !existsSync(resolve(dir, 'rust-baker.json'))) return;
  const baker = JSON.parse(readFileSync(resolve(dir, 'rust-baker.json'), 'utf8'));
  const at = resolve(dir, 'inputs.json'), tool = `${baker.executable} ${statSync(baker.executable).mtimeMs}`, pair = pairOf(dir);
  let previous; try { previous = JSON.parse(readFileSync(at, 'utf8')); } catch { /* bootstrap */ }
  if (previous?.tool === tool && previous.pair === pair && previous.at >= baker.started) return;
  const runEnv = { ...cargoEnvironment(env), ...baker.env, OUT_DIR: dir, EXACT_BAKED: dir, EXACT_RUST_BAKE: '1' };
  const lines = readFileSync(resolve(dir, 'rust-output.txt'), 'utf8').split('\n');
  const said = key => lines.map(line => new RegExp(`^cargo::?${key}=(.*)$`).exec(line)?.[1]).filter(value => value !== undefined);
  const record = { version: 1, at: baker.started, tool, pair, warnings: said('warning'),
    env: Object.fromEntries(said('rerun-if-env-changed').map(name => [name, runEnv[name] ?? null])),
    paths: said('rerun-if-changed').map(path => resolve(baker.cwd, path)) };
  writeFileSync(`${at}.${process.pid}.tmp`, JSON.stringify(record));
  renameSync(`${at}.${process.pid}.tmp`, at);
}

/** An Apple bake's warning lines for app `name`, in the compiler's order: each, but the SF Symbols only the web draws
 * blank (`webOnly`), which are one line counting them at the first one's place. */
export function bakeWarnings(name, warnings) {
  const web = warnings.filter(webOnly), one = web.length === 1, lines = [];
  for (const warning of warnings) {
    if (!webOnly(warning)) lines.push(warning);
    else if (warning === web[0]) lines.push(`${warning.split(' warning [')[0]} warning [lower-symbol-platform] ${web.length} SF Symbol use${one ? '' : 's'} draw${one ? 's' : ''} blank on the web, here first: an icon token or a role draws on every host (\`bun exact.mjs contract build --json\` says each, and how)`);
  }
  return lines.map((line) => `warning: ${name}: ${line}`);
}

/** Places what the link carries, `plan`, the `compat` text naming it (`id`) and the module's `bytecode`, in `dir`,
 * where one app's links alone read it (`linkedPlanArgs`): each file replaced whole and only when its bytes change.
 * Its directory and digest, which `<product>.linked` records. */
export function placeLinkedPlan(dir, plan, compat, id, bytecode) {
  if (JSON.parse(compat).id !== id) throw new Error(`host/apple: the linked compat.json is not the bake's (${id})`);
  mkdirSync(dir, { recursive: true });
  for (const [file, bytes] of [['app.plan', plan], ['compat.json', compat], ['app.hbc', bytecode]]) {
    const path = resolve(dir, file);
    if (existsSync(path) && readFileSync(path).equals(bytes)) continue;
    writeFileSync(`${path}.${process.pid}.tmp`, bytes);
    renameSync(`${path}.${process.pid}.tmp`, path);
  }
  return { dir, digest: createHash('sha256').update(plan).update(compat).update(bytecode).digest('hex') };
}
