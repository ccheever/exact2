#!/usr/bin/env bun
// A native drive's build, before the drive (LLP 1116.002 D4). The driver
// refuses a build its own record shows is missing or stale, and never builds
// (LLP 1012.001.000 D1: a cold build in `agent macos tree` would be minutes
// unexplained). An app's `exact.mjs` runs this first for `test` and `agent` on
// ios and macos, as it runs web-build before a web drive: a fresh build costs
// the check and says nothing; a missing or stale one is built by the command
// `bun exact.mjs ios|mac` runs, said in one stderr line. The build's output
// goes to the app's `.exact/<host>-build.log`, and to stderr, summarised, only
// when it fails. stdout stays the drive's (`--json`).
//
// A build stale only in what its plan is made of — the Contract and what
// app.ts imports, which nothing native reads — is not rebuilt (LLP 1116.003):
// the bake alone runs (`exact-js-bake`, the web dev loop's producer, about a
// second), its plan, module and source map are kept beside the build, and the
// driver launches the build with them as its launch generation (EXACT_PLAN),
// the first boot a build that baked them would make. The driver runs them only
// while they are the bake of the sources as they now are, on the build they
// were made for: any other edit, or a real build since, retires them.
//
//   bun scripts/agent-build.mjs <ios|macos> --app <name>

import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, createWriteStream, existsSync, mkdirSync, openSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { appleArtifacts, appleRunBinary } from '../host/apple/build.mjs';
import { compilerWarnings, CONFIG_JSON, DECLARATIONS, depInfoInputs, moduleGraph, newerThan, notBuildInput, receiptChanges, refuseStale } from './agent-launch.mjs';
import { claimLive, developmentBuildEnv, resolveApp, shaderWatchRoots } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const label = host => host === 'ios' ? 'iOS' : 'macOS';
/** A path as receiptChanges names it, from the app's directory when it is in it. */
const near = (app, path) => { const at = resolve(ROOT, path); return app?.dir && at.startsWith(`${app.dir}/`) ? relative(app.dir, at) : path; };
const named = (app, paths) => paths.slice(0, 3).map(path => near(app, path)).join(', ') + (paths.length > 3 ? ` and ${paths.length - 3} more` : '');
/** A file as a line names it: from the current directory when it is below it. */
const shown = file => relative(process.cwd(), file).startsWith('..') ? file : relative(process.cwd(), file);

/** What a drive of `app` on `host` (`ios`, else macOS) launches, by its own
 * record: `missing`, or the inputs `changed` since its receipt ([] when fresh,
 * or when a macOS binary has no receipt). `sample` is the sample host's.
 * `checked` is the digest of a receipt whose inputs this drive's own check
 * (`buildFirst`, run first by an app's exact.mjs) has just hashed: the same
 * receipt's are not hashed twice (LLP 1012.001.000 D1: once a drive). */
export function nativeBuild(host, app, { sample = false, checked = null } = {}) {
  const ios = host === 'ios';
  const built = ios ? appleArtifacts(app, { destination: 'ios-simulator', host: sample }).bundle : appleRunBinary(app, sample);
  if (!existsSync(built)) return { built, missing: true, changed: [] };
  const receipt = ios ? resolve(built, 'receipt.json')
    : [resolve(built, '..', 'receipt.json'), resolve(appleArtifacts(app, { host: sample }).bundle, 'Contents/Resources/receipt.json')].find(existsSync);
  return { built, receipt, changed: receipt ? receiptChanges(receipt, app, { hashed: !!checked && checked === digestOf(receipt) }) : [] };
}

/** Why a drive must build first, or null when it need not: no build yet, or
 * one older than its sources (up to three named, from the app's directory). */
export function buildReason(state, app) {
  if (state.missing) return 'there is no build yet';
  if (!state.changed.length) return null;
  return `the build is older than its sources (${named(app, state.changed)} changed)`;
}

/** The command that builds `app` for a drive (`ios` or `mac`): an `exact new`
 * app's own `exact.mjs` verb, else exact2's build. An `exact.mjs` written
 * before test and agent built first is told how to get that. */
export function rebuildCommand(app, verb) {
  const own = app.dir && !resolve(app.dir).startsWith(`${ROOT}/`) && existsSync(resolve(app.dir, 'exact.mjs'));
  if (!own) return `bun host/apple/build.mjs${verb === 'ios' ? ' --ios' : ''} ${app.crate('apple')}`;
  const command = `(cd '${String(app.dir).replaceAll("'", "'\\''")}' && bun exact.mjs ${verb})`;
  const runner = readFileSync(resolve(app.dir, 'exact.mjs'), 'utf8');
  // A launcher (LLP 1116.006 D7) runs this checkout's app-cli.mjs, which builds first.
  return runner.includes('agent-build.mjs') || runner.includes('scripts/app-cli.mjs') ? command
    : `${command} (after bun exact.mjs update, test and agent build it themselves)`;
}

/** The driver's refusal (LLP 1012.001.000 D1) on what `nativeBuild` found. */
export function refuseBuild(host, state, app, { sample = false } = {}) {
  const ios = host === 'ios';
  const command = sample ? `bun host/apple/build.mjs${ios ? ' --ios' : ''} ${app.crate('apple')} --host` : rebuildCommand(app, ios ? 'ios' : 'mac');
  if (state.missing) throw new Error(`run ${command} first`);
  refuseStale(ios ? 'ios' : sample ? 'sample host' : 'macos', state.receipt, state.changed, command);
}

/** host/apple/build.mjs's arguments for the build `bun exact.mjs ios|mac` runs before a drive. An iOS build installs
 * nothing: the drive installs it, copying aside a Release build it replaces (devices.mjs `keepInstalledRelease`). */
export const buildArgs = (host, app) => [...(host === 'ios' ? ['--ios'] : []), app.crate('apple'), ...(host === 'ios' ? ['--no-install'] : [])];

/** What a failed build said, short: from its first error, at most 30 lines (else
 * its last 20), and its last line; then that it failed and where its log is. */
export function failureSummary(output, { host, code, log }) {
  const lines = output.split('\n').filter(line => line.trim());
  const first = lines.findIndex(line => /^error\b|\berror(\[E\d+\])?:/.test(line));
  const shown = first < 0 ? lines.slice(-20) : lines.slice(first, first + 30);
  if (lines.length && !shown.includes(lines.at(-1))) shown.push('…', lines.at(-1));
  return [...shown, `the ${label(host)} build failed (exit ${code}), so nothing was driven; its whole log: ${log}`].join('\n');
}

/** `command` with `args`, its output into `log` (`append`ed to it); its exit code and output. */
function logged(command, args, log, { cwd, env, append = false } = {}) {
  mkdirSync(resolve(log, '..'), { recursive: true });
  const file = createWriteStream(log, { flags: append ? 'a' : 'w' }), chunks = [];
  const child = spawn(command, args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
  const pass = signal => child.kill(signal), signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
  for (const signal of signals) process.on(signal, pass);
  for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => { file.write(chunk); chunks.push(chunk); });
  return new Promise(done => {
    child.on('error', error => chunks.push(Buffer.from(String(error.message))));
    child.on('close', (code, signal) => {
      for (const s of signals) process.off(s, pass);
      file.end(() => done({ code: code ?? (signal ? 128 : 1), output: Buffer.concat(chunks).toString('utf8') }));
    });
  });
}
/** host/apple/build.mjs with `args`, its output into `log`; its exit code and output. */
const appleBuild = (args, log) => logged(process.execPath, [resolve(ROOT, 'host/apple/build.mjs'), ...args], log);

// ---------------------------------------------------------------- a plan-only edit (LLP 1116.003)

// What a plan is made of and nothing native reads: the Contract, and the
// TypeScript and JSON app.ts imports (agent-launch.mjs `moduleGraph`; every
// TypeScript file when that cannot be read), outside the trees a native build
// takes files from and the app's Rust crates (which can include any file
// beside them). A file app.ts does not import is no input at all.
const NATIVE_TREE = /^(assets|deck|gpu|art|modules|fonts|strings|web|apple|ios|macos|linux)(\/|$)/;
const digestOf = path => { try { return createHash('sha256').update(readFileSync(path)).digest('hex'); } catch { return null; } };
const graphOf = app => moduleGraph(app.dir, app.manifest);

/** Whether a changed input (as receiptChanges names it) is only the plan's. */
export function planSource(path, app, graph = graphOf(app)) {
  const at = resolve(ROOT, path), rel = relative(app.dir, at);
  if (!rel || rel.startsWith('..') || isAbsolute(rel) || NATIVE_TREE.test(rel) || rel === DECLARATIONS) return false;
  for (let dir = dirname(at); dir.startsWith(`${app.dir}/`); dir = dirname(dir)) if (existsSync(resolve(dir, 'Cargo.toml'))) return false;
  if (/\.contract$/.test(rel)) return true;
  if (!/\.(tsx?|json)$/.test(rel) || CONFIG_JSON.test(rel)) return false;
  return graph ? graph.has(at) : /\.tsx?$/.test(rel);
}

/** The plan sources of `app` by content, as a record compares them: its own
 * (the drive's skips, as receiptChanges walks them) and what app.ts imports
 * from the directories app.json's `typescript.sources` mounts. */
export function planSources(app, graph = graphOf(app)) {
  const skip = notBuildInput(app.dir, shaderWatchRoots(app));
  const own = newerThan(-Infinity, [app.dir], path => /\/(apple|ios|macos|linux|web)$/.test(path) && path.startsWith(`${app.dir}/`) || skip(path))
    .filter(path => planSource(path, app, graph));
  const mounts = Object.values(app.manifest?.typescript?.sources ?? {}).map(path => resolve(app.dir, String(path)));
  const mounted = graph ? [...graph].filter(path => !path.startsWith(`${app.dir}/`)) : newerThan(-Infinity, mounts).filter(path => /\.tsx?$/.test(path));
  return Object.fromEntries([...own, ...mounted].sort().map(path => [path, digestOf(resolve(ROOT, path))]));
}

/** Where a plan-only edit's bake is kept: beside the build it runs on. */
export const overrideDir = (host, app) => resolve(appleArtifacts(app, host === 'ios' ? { destination: 'ios-simulator' } : {}).namespace, 'plan');

/** The bake the driver launches `state`'s build (`nativeBuild`) with, or null:
 * when the build is fresh, missing, or stale in anything but its plan's
 * sources, or the bake kept is for another build (a real build since retires
 * it) or for sources that have changed since it was made. */
export function planOverride(host, app, state, graph = graphOf(app)) {
  if (state.missing || !state.receipt || !state.changed.length || !state.changed.every(path => planSource(path, app, graph))) return null;
  const dir = overrideDir(host, app);
  let record; try { record = JSON.parse(readFileSync(resolve(dir, 'override.json'), 'utf8')); } catch { return null; }
  if (record.version !== 1 || record.receipt !== digestOf(state.receipt)) return null;
  if (JSON.stringify(record.sources) !== JSON.stringify(planSources(app, graph))) return null;
  if (!['app.plan', 'app.module.json', 'app.hbc'].every(file => existsSync(resolve(dir, file)))) return null;
  return { plan: resolve(dir, 'app.plan'), dir };
}

/** The module receipt the build baked: its build script's, found by the plan
 * its receipt names (the archive's dep-info names that script's output). */
export function builtModule(app, receipt) {
  const { target } = receipt, plan = receipt.build?.compat?.embedded?.plan?.sha256;
  if (!target || !plan) return null;
  const archive = `lib${app.crate(target.includes('-ios') ? 'ios' : 'macos').replace(/-/g, '_')}.d`;
  let infos = []; try { infos = readdirSync(resolve(app.target, target)).map(p => resolve(app.target, target, p, archive)).filter(existsSync); } catch {}
  for (const info of infos) {
    const baked = depInfoInputs(null, info).find(path => path.endsWith('/out/app.plan'));
    if (baked && digestOf(baked) === plan) try { return JSON.parse(readFileSync(resolve(dirname(baked), 'app.module.json'), 'utf8')); } catch {}
  }
  return null;
}

/** Why the build cannot run another plan of its app's, or null: what a
 * development build that links every capability and pairs a TypeScript module
 * alone can (a Rust data crate's plan is baked with it, by a build). */
export function buildRefusal(app, receipt) {
  if ((process.env.EXACT_UPDATE_TRUST ?? 'development') !== 'development') return 'the build is not a development build';
  const inputs = receipt.build?.compat?.inputs ?? {};
  if (inputs.dataCrate || (inputs.rustMode ?? 'off') !== 'off') return 'the app has a Rust data crate, whose plan only a build bakes';
  const uses = new Set([...(receipt.build?.graph?.uses ?? []), ...(app.manifest?.link ?? []), 'io']);
  if (Array.isArray(inputs.linked) && inputs.linked.every(name => uses.has(name))) return 'the build links only what its plan used (--link plan)';
  return builtModule(app, receipt) ? null : 'the build has no module receipt to check a new plan against';
}

/** Why the new module cannot replace the build's, or null: the runtime's own
 * refusals of a candidate module (js/src/paired.rs), said before a launch. */
export function moduleRefusal(built, baked) {
  if (baked.appId !== built.appId) return `app.ts names another app (${baked.appId}) than the build (${built.appId})`;
  if (String(baked.grants ?? '').trim() !== String(built.grants ?? '').trim()) return 'app.ts changed its grants, which only a build admits';
  if (baked.abi !== built.abi || JSON.stringify(baked.surfaces ?? {}) !== JSON.stringify(built.surfaces ?? {})) return 'app.ts changed its canvas surfaces, which only a build registers';
  if (baked.bytecodeVersion !== built.bytecodeVersion) return 'the bake made another Hermes bytecode version than the build';
  return null;
}

/** The bake tool, the web dev loop's producer (`exact-js-bake`), built once in
 * this checkout's target and shared by every app driven from it: the command
 * that builds it (a no-op when built), and where it is. */
const TOOL_TARGET = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(ROOT, 'target');
const bakeTool = { command: 'cargo', args: ['build', '-q', '--profile', 'host-dev', '-p', 'exact-js-bake'], built: resolve(TOOL_TARGET, 'host-dev/exact-js-bake'),
  options: () => ({ cwd: ROOT, env: { ...developmentBuildEnv(), EXACT_UPDATE_TRUST: 'development', CARGO_TARGET_DIR: TOOL_TARGET } }) };

/** The bake tool baking `app` into the new directory `out`. */
async function bakePlan(app, out, log) {
  const options = bakeTool.options(), tool = await logged(bakeTool.command, bakeTool.args, log, options);
  if (tool.code) return { ...tool, tool: true };
  return logged(bakeTool.built, [app.dir, '--out', out], log, { ...options, append: true });
}

/** Bakes the plan of `state`'s plan-only edit and keeps it beside the build:
 * `{ made }`; `{ failed }`, what the bake said (a build would fail on it too);
 * or `{ refusal }`, why the build needs a rebuild after all. */
export async function makeOverride(host, app, state, { bake = bakePlan, log } = {}) {
  let receipt; try { receipt = JSON.parse(readFileSync(state.receipt, 'utf8')); } catch { return { refusal: 'its receipt is unreadable' }; }
  const refused = buildRefusal(app, receipt);
  if (refused) return { refusal: refused };
  const dir = overrideDir(host, app), stage = `${dir}.${process.pid}.tmp`, sources = planSources(app);
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(dirname(dir), { recursive: true });
  try {
    const { code, output = '', tool } = await bake(app, stage, log);
    if (tool) return { refusal: `the bake tool did not build (exit ${code}; its log: ${shown(log)})` };
    if (code) return { failed: output.trim() || `the bake exited ${code}` };
    const why = moduleRefusal(builtModule(app, receipt), JSON.parse(readFileSync(resolve(stage, 'app.module.json'), 'utf8')));
    if (why) return { refusal: why };
    if (JSON.stringify(planSources(app)) !== JSON.stringify(sources)) return { refusal: 'the sources changed while the plan baked' };
    writeFileSync(resolve(stage, 'override.json'), JSON.stringify({ version: 1, receipt: digestOf(state.receipt), sources, changed: state.changed }, null, 2) + '\n');
    rmSync(dir, { recursive: true, force: true });
    renameSync(stage, dir);
    return { made: true, output };
  } finally { rmSync(stage, { recursive: true, force: true }); }
}

/** A build of this app already running (one started in the background) ends
 * before the check, so the drive neither races it nor is refused as busy. */
async function waitForBuild(app, say) {
  const lock = appleArtifacts(app).lock;
  for (let told = false; ; await new Promise(r => setTimeout(r, 250))) {
    let held; try { held = readFileSync(lock, 'utf8'); } catch { return; }
    if (!claimLive(held)) return;
    if (!told) { told = true; say(`waiting for the Apple build of ${app.name} already running`); }
  }
}

/** Builds first when the drive would refuse (`nativeBuild`): one line on `say`,
 * the build's output in the app's `.exact/`, its summary when it fails; the
 * exit code. A fresh build runs nothing and says nothing; nor does one whose
 * plan-only edit is already baked beside it. A new plan-only edit is baked
 * instead of built, in one line; what its bake refuses fails the command, as
 * the build would have. A build another process claims first is waited for and the check made again. */
export async function buildFirst(host, app, { say = line => console.error(line), build = appleBuild, wait = waitForBuild, bake = bakePlan, retries = 2 } = {}) {
  host = host === 'ios' ? 'ios' : 'macos';
  if (host === 'macos' && process.env.EXACT_MAC_BIN) return 0; // the caller's own binary: the driver says it did not check
  await wait(app, say);
  const state = nativeBuild(host, app);
  // What the drive after this need not hash again: its receipt, when nothing is rebuilt (`nativeBuild`'s `checked`).
  const hashed = () => { if (process.env.EXACT_BUILD_CHECK_OUT && state.receipt) writeFileSync(process.env.EXACT_BUILD_CHECK_OUT, digestOf(state.receipt)); return 0; };
  let reason = buildReason(state, app);
  if (!reason) return hashed();
  const graph = graphOf(app);
  if (!state.missing && state.changed.every(path => planSource(path, app, graph))) {
    if (planOverride(host, app, state, graph)) return hashed();
    const log = resolve(app.dir, '.exact', `${host}-plan.log`), made = await makeOverride(host, app, state, { bake, log }), what = named(app, state.changed);
    if (made.made) {
      say(`${label(host)}: ${what} changed; running the new plan on the existing build (no native rebuild)`);
      for (const warning of compilerWarnings(made.output)) say(warning);
      return hashed();
    }
    if (made.failed) {
      say(`${made.failed}\nthe ${label(host)} plan did not bake (${what} changed), so nothing was driven; its whole log: ${shown(log)}`);
      return 1;
    }
    reason += `, and ${made.refusal}`;
  }
  rmSync(overrideDir(host, app), { recursive: true, force: true }); // the build supersedes it
  const log = resolve(app.dir, '.exact', `${host}-build.log`);
  say(`building for ${label(host)}: ${reason}; its log: ${shown(log)}`);
  const { code, output } = await build(buildArgs(host, app), log);
  // Another build took the claim between the check and this one (two started together): wait for it, then decide again.
  if (code && retries > 0 && /Apple build busy for/.test(output)) return buildFirst(host, app, { say, build, wait, bake, retries: retries - 1 });
  if (code) say(failureSummary(output, { host, code, log: shown(log) }));
  // The compiler's warnings, which Cargo said for the bake's build script (LLP 1116.004 D7).
  else for (const warning of compilerWarnings(output)) say(warning);
  return code;
}

/** `bun exact.mjs ios --prebuild` (LLP 1116.003 D1.2): the build host/apple/build.mjs's `args` name,
 * then the bake tool a plan-only edit needs, started in a session of their own, so a shell that ends
 * what it started when a command returns (round 5: seven builders lost a first build put in the
 * background with &) cannot reach them; their output in the app's `.exact/<host>-build.log`. Back
 * once the build holds the app's build claim, so a `test` or `agent` started next waits for it
 * (`waitForBuild`) instead of building beside it. */
export async function prebuild(app, args, { timeout = 30_000, say = line => console.error(line) } = {}) {
  const host = args.includes('--tvos') ? 'tvos' : args.includes('--ios') || args.includes('--device') ? 'ios' : 'macos';
  const log = resolve(app.dir, '.exact', `${host}-build.log`), lock = appleArtifacts(app).lock, since = Date.now() - 1000;
  mkdirSync(dirname(log), { recursive: true });
  const out = openSync(log, 'w');
  const build = [resolve(ROOT, 'host/apple/build.mjs'), ...args.filter(arg => arg !== '--prebuild')];
  const child = spawn('/bin/sh', ['-c', '"$0" "$@"; built=$?; "$0" "$TOOL" --tool; exit $built', process.execPath, ...build],
    { detached: true, stdio: ['ignore', out, out], env: { ...process.env, TOOL: fileURLToPath(import.meta.url) } });
  closeSync(out);
  let ended = null;
  child.on('exit', code => { ended = code ?? 1; });
  for (const started = Date.now(); ended === null && Date.now() - started < timeout; await new Promise(r => setTimeout(r, 100))) {
    let held = null; try { held = JSON.parse(readFileSync(lock, 'utf8')); } catch {}
    if (held && Date.parse(held.started) >= since) break; // claimed since: this build's
  }
  child.unref();
  const what = host === 'ios' ? 'iOS' : host === 'tvos' ? 'tvOS' : 'macOS';
  if (ended !== null) { say(ended ? `the ${what} build failed (exit ${ended}); its log: ${shown(log)}` : `built for ${what}; its log: ${shown(log)}`); return ended; }
  say(`building for ${what} in the background (pid ${child.pid}, apart from this shell); its log: ${shown(log)}. test and agent wait for it.`);
  return 0;
}

if (import.meta.main && process.argv[2] === '--tool') { // the bake tool, after a prebuild
  const child = spawn(bakeTool.command, bakeTool.args, { ...bakeTool.options(), stdio: 'inherit' });
  child.on('exit', code => process.exit(code ?? 1));
} else if (import.meta.main) {
  const args = process.argv.slice(2), value = flag => args.includes(flag) ? args[args.indexOf(flag) + 1] : undefined;
  if (!['ios', 'macos', 'mac'].includes(args[0])) {
    console.error('usage: bun scripts/agent-build.mjs <ios|macos> --app <name>');
    process.exit(2);
  }
  Promise.resolve().then(() => buildFirst(args[0], resolveApp(value('--app'))))
    .then(code => process.exit(code), error => { console.error(error.message); process.exit(1); });
}
