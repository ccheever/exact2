/**
 * The plan an Apple build links: a TypeScript app's bake, made before Cargo.
 *
 * Cargo compiles again every crate whose build script ran, and the archive
 * crate's build script was the bake, watching the Contract: a one-word edit
 * put the archive's whole graph through fat LTO again (44 of a 48 s Release
 * build of Euxy). Here the build script's own code (`exact-js-bake --cargo`)
 * bakes into a directory of its own; the build script takes from it only the
 * files Rust compiles (EXACT_BAKED, `exact_js_bake::build`), which an edit to
 * the plan leaves as they were, and the link puts the plan and compat.json in
 * the executable's `__TEXT` (`exact_apple::baked`, host/apple/build.mjs).
 *
 * The bake runs again when something it named has changed since it started,
 * as Cargo would run the build script: a watched file, anything under a
 * watched directory, a watched variable, or the bake tool itself.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargoEnvironment, developmentBuildEnv } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const TOOL_TARGET = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(ROOT, 'target');

/** The bake tool (`exact-js-bake`), built once in this checkout's target and
 * shared by every app built from it: the command that builds it (a no-op when
 * built), and where it is. */
export const bakeTool = { command: 'cargo', args: ['build', '-q', '--profile', 'host-dev', '-p', 'exact-js-bake'], built: resolve(TOOL_TARGET, 'host-dev/exact-js-bake'),
  options: () => ({ cwd: ROOT, env: { ...developmentBuildEnv(), EXACT_UPDATE_TRUST: 'development', CARGO_TARGET_DIR: TOOL_TARGET } }) };

/** Whether `app`'s plan can be baked before Cargo: a TypeScript app with no
 * Rust data crate, whose bake runs no Rust of the app's (`build_mixed`). An
 * embedding archive carries its plan (`--embed`: no link of ours adds it).
 * An entry that declares `const PLAN` and `const COMPAT` itself (an outside
 * app from before) reads the plan from the archive, and builds as it did. */
export function linkable(app) {
  if (!existsSync(resolve(app.dir, 'app.ts')) || existsSync(resolve(app.dir, 'data/Cargo.toml'))) return false;
  let entry = ''; try { entry = readFileSync(resolve(app.dir, 'apple/src/lib.rs'), 'utf8'); } catch { /* none */ }
  if (!/^const PLAN:/m.test(entry) || !/^const COMPAT:/m.test(entry)) return true;
  console.error(`host/apple: apple/src/lib.rs declares PLAN and COMPAT, so the plan is compiled into the archive and a Contract edit compiles it again; \`bun exact.mjs update\` drops them, and the link then carries the plan`);
  return false;
}

/** Where an Apple build for `target` keeps the bake it links. */
export const linkedBakeDir = (env, platform, target) => resolve(env.EXACT_BAKE_OUTPUT, `${platform}-${target}.baked`);

/** The linker arguments that put a linked bake's plan and compat.json in the executable. */
export const linkedPlanArgs = (dir) => !dir ? [] : [['__exact_plan', 'app.plan'], ['__exact_compat', 'compat.json']]
  .flatMap(([section, file]) => ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', section, '-Xlinker', resolve(dir, file)]);

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

/** Whether the bake `record` describes is the one its inputs would make now. */
function fresh(record, env, tool) {
  if (record?.version !== 1 || record.tool !== tool) return false;
  if (Object.entries(record.env).some(([name, value]) => (env[name] ?? null) !== value)) return false;
  return record.paths.every(path => { const at = newest(path); return at !== null && at < record.at; });
}

/** Bakes `app` for `platform` and `target` into its linked bake directory,
 * unless the bake there is fresh; the directory. A bake that fails throws
 * what it said, as the build script's failure ends a build. */
export function bakeOutside(app, platform, target, env) {
  const dir = linkedBakeDir(env, platform, target), at = resolve(dir, 'inputs.json');
  const built = spawnSync(bakeTool.command, bakeTool.args, { ...bakeTool.options(), stdio: ['ignore', 'inherit', 'inherit'] });
  if (built.status !== 0) throw new Error(`the bake tool did not build (${bakeTool.command} ${bakeTool.args.join(' ')})`);
  const tool = `${bakeTool.built} ${statSync(bakeTool.built).mtimeMs}`;
  // Its warnings are said on every build, as Cargo replays a build script's.
  const warn = (warnings) => { for (const warning of warnings ?? []) console.error(`warning: ${app.name}: ${warning}`); };
  let record = null; try { record = JSON.parse(readFileSync(at, 'utf8')); } catch { /* none yet */ }
  if (fresh(record, env, tool)) { warn(record.warnings); return dir; }
  rmSync(at, { force: true });
  const started = Date.now();
  const r = spawnSync(bakeTool.built, [app.dir, '--cargo', platform, target, dir], { cwd: app.dir, env: cargoEnvironment(env), encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  const lines = (r.stdout ?? '').split('\n'), said = (key) => lines.map(line => new RegExp(`^cargo::?${key}=(.*)$`).exec(line)?.[1]).filter(value => value !== undefined);
  warn(said('warning'));
  if (r.status !== 0) {
    const error = new Error(`${app.name}: the bake failed:\n${(r.stderr || r.error?.message || `exit ${r.status}`).trim()}`);
    error.stack = error.message; error.contract = true;
    throw error;
  }
  record = { version: 1, at: started, tool, warnings: said('warning'), env: Object.fromEntries(said('rerun-if-env-changed').map(name => [name, env[name] ?? null])), paths: said('rerun-if-changed').map(path => resolve(app.dir, path)) };
  mkdirSync(dir, { recursive: true });
  writeFileSync(`${at}.${process.pid}.tmp`, JSON.stringify(record));
  renameSync(`${at}.${process.pid}.tmp`, at);
  return dir;
}
