#!/usr/bin/env bun
/**
 * async — the async lane (rules/RULES.md §Loop shape): every first-parent
 * commit on origin/main, checked out in a dedicated worktree with its own
 * target/, gets the five checks over the whole workspace plus the tests marked
 * `#[ignore = "async lane: …"]`, every web host unit test found by one glob,
 * the web host's app-building document test in its own step, the web JS target's conformance run
 * (`host/web-js/conform.mjs --strict`), the UIKit XCTests on a simulator when the commit
 * touches host/apple (`build.mjs --test --ios`; Charlie, 2026-09-23), then
 * `metrics.mjs --long` (every RULES budget;
 * a VIOLATION or FAILED row or a failed run counts, an OVER time does not — it
 * moves with the load). A failure that the previous commit did not have is filed with
 * `issue.mjs`, naming the commit that introduced it. A check past 2 h is
 * killed and filed as hung; the worktree's debug target is kept under 80 GiB.
 * Logs and timings (with the load average) stay in <worktree>/target/async/.
 *
 *   bun scripts/async.mjs                 watch: check new commits every 300 s
 *   bun scripts/async.mjs --once          check what is pending, then exit
 *   bun scripts/async.mjs --from <rev>    start after <rev> instead of the tip
 *   bun scripts/async.mjs --worktree <dir> --interval <s> --branch <ref>
 *
 * --branch (default origin/main) checks another line of history; the
 * worktree is the script's own and is checked out with --force.
 */
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, openSync, closeSync, readFileSync, writeFileSync } from 'node:fs';
import { loadavg } from 'node:os';
import { basename, resolve } from 'node:path';
import { main as issue } from './issue.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const option = (name, fallback) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : fallback;
const WT = resolve(option('--worktree', resolve(ROOT, '..', `${basename(ROOT)}-async`)));
const STATE_DIR = resolve(WT, 'target/async');
const STATE = resolve(STATE_DIR, 'state.json');
const BRANCH = option('--branch', 'origin/main');
const git = (args, cwd = ROOT) => {
  const r = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr.trim()}`);
  return r.stdout.trim();
};

/** Test names marked for this lane; libtest ORs several filters. */
function laneTests() {
  const r = spawnSync('git', ['grep', '-n', '-A3', '#\\[ignore = "async lane', '--', '*.rs'], { cwd: WT, encoding: 'utf8' });
  return [...new Set([...(r.stdout ?? '').matchAll(/fn (\w+)\s*\(/g)].map(m => m[1]))];
}

const workspace = ['--workspace'];
const WEB_APPS = ['realworld', 'weatherlight', 'completion-storm', 'video-player', 'caltrain', 'typetour', 'carousel', 'sparkline', 'svg-gallery', 'spark', 'markdown-stress', 'reflow', 'textflow', 'canvas-gallery', 'update-lab', 'native-fixture', 'photo-editor', 'recorder', 'fieldnotes', 'markdown', 'messages', 'interaction-gallery', 'motion-gallery'];
function checks(sha) {
  const lane = laneTests();
  const apple = git(['diff', '--name-only', `${sha}^`, sha, '--', 'host/apple'], WT) !== '';
  const glue = [...new Bun.Glob('host/web/**/*.test.mjs').scanSync({ cwd: WT, onlyFiles: true })].sort().map(file => `./${file}`);
  return [
    ['build', 'cargo', ['build', ...workspace, '--all-targets', '--keep-going']],
    ['test', 'cargo', ['test', ...workspace, '--lib', '--bins', '--tests', '--no-fail-fast']],
    ...(lane.length ? [['lane', 'cargo', ['test', ...workspace, '--lib', '--bins', '--tests', '--no-fail-fast', '--', '--ignored', ...lane]]] : []),
    ['clippy', 'cargo', ['clippy', ...workspace, '--all-targets', '--keep-going', '--', '-D', 'warnings']],
    ['fmt', 'cargo', ['fmt', '--all', '--', '--check']],
    ['caps', 'bun', ['scripts/caps.mjs']],
    ['boot', 'bun', ['scripts/boot.mjs']],
    // Every web host unit test (LLP 1012.001.000 D9; Charlie, 2026-09-30),
    // discovered by one glob so a new test cannot sit outside the lane.
    ['glue', 'env', ['EXACT_GLUE_FAST=1', 'bun', 'test', ...glue]],
    // This document proof builds Weatherlight's wasm target. Keep it out of
    // glue's seconds loop while retaining it in the asynchronous lane.
    ['web-build-test', 'bun', ['test', './host/web/tests/document.test.mjs', '--test-name-pattern', "a TypeScript app's served document"]],
    // The web build's JS target against the wasm runner, step by step (LLP
    // 1071 §4; Charlie, 2026-09-28): minutes and a network, so never blocking.
    ['conform', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--build', '--linux', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--out', resolve(STATE_DIR, 'conform')]],
    // The JS target in the other browser engines, with Chrome as its oracle.
    // These remain async-only; a missing Playwright browser is a named failure
    // whose log gives the exact outside-the-repo install command.
    ['conform-firefox', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--browser', 'firefox', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--out', resolve(STATE_DIR, 'conform-firefox')]],
    ['conform-webkit', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--browser', 'webkit', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--out', resolve(STATE_DIR, 'conform-webkit')]],
    ...(apple ? [['ios', 'bun', ['host/apple/build.mjs', '--test', '--ios']]] : []),
    ['metrics', 'bun', ['scripts/metrics.mjs', '--long']],
  ];
}

/** Every failure a log names, as stable strings (compared across commits). */
function failures(name, log, status) {
  const found = new Set();
  for (const m of log.matchAll(/error: could not compile `([^`]+)` \(([^)]+)\)/g)) found.add(`${name}: ${m[1]} (${m[2]}) does not compile`);
  // A failing binary ends with cargo's stable `-p <package> --test <target>`.
  let failed = [];
  for (const line of log.split('\n')) {
    if (/^\s+Running /.test(line)) failed = [];
    const test = /^test (\S+) \.\.\. FAILED$/.exec(line);
    if (test) failed.push(test[1]);
    const rerun = /^error: test failed, to rerun pass `([^`]+)`/.exec(line);
    if (rerun) for (const t of failed.splice(0)) found.add(`${name}: ${rerun[1]} ${t}`);
  }
  // metrics rows: a VIOLATION, or a measurement whose build FAILED.
  if (name === 'metrics') for (const m of log.matchAll(/^[ \t]+(\S[^\n]*?)[ \t]{2,}(?:FAILED\b|[^\n]*\bVIOLATION\b)/gm)) found.add(`${name}: ${m[1]} ${/\bVIOLATION\b/.test(m[0]) ? 'VIOLATION' : 'FAILED'}`);
  for (const m of log.matchAll(/Test Case '-\[(\S+) (\S+)\]' failed/g)) found.add(`${name}: ${m[1]} ${m[2]} failed`);
  for (const m of log.matchAll(/^Diff in (\S+?):\d+:/gm)) found.add(`${name}: ${m[1].replace(WT + '/', '')} is not formatted`);
  // conform --strict: a failing step by target and step (the what varies run to run).
  for (const m of log.matchAll(/^FAIL (\S+) ([^:\n]+):/gm)) found.add(`${name}: ${m[1]} ${m[2]}`);
  for (const m of log.matchAll(/^\(fail\) (.+?) \[[\d.]+m?s\]$/gm)) found.add(`${name}: ${m[1]}`);
  if (status !== 0 && !found.size) found.add(`${name}: exit ${status} (see log)`);
  return [...found];
}

// A check still running past this is hung (libtest has no per-test timeout;
// a test waiting on accept() held the lane for 15 hours): its process group
// is killed and the tests libtest saw running over 60 s are filed by name.
const HANG_MS = 2 * 60 * 60 * 1000;
function run(command, args, env, fd) {
  return new Promise((done) => {
    const child = spawn(command, args, { cwd: WT, env, stdio: ['ignore', fd, fd], detached: true });
    let hung = false;
    const timer = setTimeout(() => { hung = true; try { process.kill(-child.pid, 'SIGKILL'); } catch {} }, HANG_MS);
    child.on('error', () => { clearTimeout(timer); done({ status: 127, hung }); });
    child.on('exit', (status) => { clearTimeout(timer); done({ status: status ?? 128, hung }); });
  });
}

// The worktree's debug target grows with every commit (a full workspace
// build with debuginfo). Keep it bounded before each check: the incremental
// caches first, the whole debug profile if that is not enough.
const KIB = (path) => Number(spawnSync('du', ['-sk', path], { encoding: 'utf8' }).stdout?.split('\t')[0] || 0);
function prune() {
  const debug = resolve(WT, 'target/debug');
  if (!existsSync(debug) || KIB(debug) < 80 * 1024 * 1024) return;
  spawnSync('rm', ['-rf', resolve(debug, 'incremental')]);
  const left = KIB(debug);
  if (left >= 110 * 1024 * 1024) spawnSync('rm', ['-rf', debug]);
  console.log(`pruned ${WT}/target/debug to ${(KIB(debug) / 1024 / 1024).toFixed(0)} GiB (was over 80 GiB)`);
}

async function check(sha) {
  const dir = resolve(STATE_DIR, sha.slice(0, 12));
  mkdirSync(dir, { recursive: true });
  prune();
  git(['checkout', '--detach', '--force', sha], WT);
  const env = { ...process.env };
  delete env.EXACT_UPDATE_TRUST; delete env.CARGO_TARGET_DIR; delete env.EXACT_WEB_BROWSER;
  const installed = spawnSync('bun', ['install', '--frozen-lockfile'], { cwd: WT, env, encoding: 'utf8' });
  const result = { sha, subject: git(['log', '-1', '--format=%s', sha]), checks: {}, failures: [] };
  if (installed.status !== 0) result.failures.push(`install: bun install --frozen-lockfile exit ${installed.status}`);
  for (const [name, command, args] of checks(sha)) {
    const logPath = resolve(dir, `${name}.log`), fd = openSync(logPath, 'w');
    const load = loadavg()[0], start = performance.now();
    const r = await run(command, args, env, fd);
    closeSync(fd);
    const seconds = (performance.now() - start) / 1000, log = readFileSync(logPath, 'utf8');
    result.checks[name] = { command: [command, ...args].join(' '), status: r.status, seconds, load, ...(r.hung ? { hung: true } : {}) };
    result.failures.push(...failures(name, log, r.status));
    if (r.hung) {
      const slow = [...new Set([...log.matchAll(/^test (\S+) has been running for over 60 seconds/gm)].map(m => m[1]))];
      result.failures.push(...(slow.length ? slow.map(t => `${name}: ${t} hung`) : [`${name}: hung past ${HANG_MS / 60000} min`]));
    }
  }
  writeFileSync(resolve(dir, 'result.json'), JSON.stringify(result, null, 2));
  return result;
}

function file(result, fresh) {
  const short = result.sha.slice(0, 8);
  const timing = Object.entries(result.checks).map(([n, c]) => `${n} ${c.seconds.toFixed(0)} s (exit ${c.status}, load ${c.load.toFixed(0)})`).join(' · ');
  const body = [`The async lane found ${fresh.length} failure(s) that ${short}'s parent did not have:`, '',
    ...fresh.map(f => `- ${f}`), '', `Commit: ${short} ${result.subject}`, `Checks: ${timing}`,
    `Logs: ${resolve(STATE_DIR, result.sha.slice(0, 12))}/`].join('\n');
  issue(['new', `Async lane: ${fresh.length} new failure(s) at ${short}`, '--systems', 'async lane',
    '--slug', `async-${short}`, '--author', 'async lane (scripts/async.mjs)', '--body', body, '--quiet'], ROOT);
}

async function once(state) {
  git(['fetch', '-q', 'origin']);
  const tip = git(['rev-parse', BRANCH]);
  const pending = state.last
    ? git(['rev-list', '--first-parent', '--reverse', `${state.last}..${tip}`]).split('\n').filter(Boolean)
    : [tip];
  for (const sha of pending) {
    const result = await check(sha);
    // A commit outside host/apple runs no iOS tests: the last ones stand.
    if (!result.checks.ios) result.failures.push(...(state.failures ?? []).filter(f => f.startsWith('ios: ')));
    // The first commit checked has no parent result: it sets the baseline,
    // since its failures cannot be attributed to it.
    const baseline = state.failures === undefined;
    const before = new Set(state.failures ?? []);
    const fresh = result.failures.filter(f => !before.has(f));
    console.log(`${sha.slice(0, 8)} ${result.subject}: ${result.failures.length} failure(s), ${baseline ? 'baseline' : `${fresh.length} new`}`);
    if (fresh.length && !baseline) file(result, fresh);
    Object.assign(state, { last: sha, failures: result.failures });
    writeFileSync(STATE, JSON.stringify(state, null, 2));
  }
}

if (!existsSync(WT)) git(['worktree', 'add', '--detach', WT, BRANCH]);
mkdirSync(STATE_DIR, { recursive: true });
const state = existsSync(STATE) ? JSON.parse(readFileSync(STATE, 'utf8')) : {};
if (option('--from')) state.last = git(['rev-parse', option('--from')]);
const interval = Number(option('--interval', 300)) * 1000;
for (;;) {
  await once(state);
  if (process.argv.includes('--once')) break;
  await new Promise(done => setTimeout(done, interval));
}
