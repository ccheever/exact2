#!/usr/bin/env bun
/**
 * async — the async lane (rules/RULES.md §Loop shape): every first-parent
 * commit on origin/main, checked out in a dedicated worktree with its own
 * target/, gets the five checks over the whole workspace plus the tests marked
 * `#[ignore = "async lane: …"]`, every web host unit test found by one glob,
 * the web host's app-building document test in its own step, the web JS target's conformance run
 * (`host/web-js/conform.mjs --strict`), the UIKit XCTests on a simulator when the commit
 * touches host/apple (`build.mjs --test --ios`; Charlie, 2026-09-23), the
 * Contract semantics' proofs and differential run (`semantics/README.md`), then
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
 *
 * --tier 2 is the second lane: the platforms that ride on another host's
 * code (tvOS on the UIKit presenter; Charlie, 2026-10-03), in its own
 * worktree (<repo>-async2) and state, hourly by default. It checks only the
 * newest pending commit, so a failure it files names the range since the
 * last commit it checked, never one commit.
 *
 *   bun scripts/async.mjs --tier 2        watch the second lane
 *
 * --newest makes the first lane coalesce the same way. Main outruns a check
 * that takes an hour (some 500 first-parent commits in 3.5 days, 2026-10-06),
 * so the lane on the mini runs `--newest --interval 3600` (Charlie, 2026-10-06).
 */
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, openSync, closeSync, readFileSync, writeFileSync } from 'node:fs';
import { loadavg } from 'node:os';
import { basename, resolve } from 'node:path';
import { main as issue } from './issue.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const option = (name, fallback) => process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : fallback;
const TIER = Number(option('--tier', 1));
if (TIER !== 1 && TIER !== 2) throw new Error(`--tier ${TIER}: the lanes are 1 and 2`);
const WT = resolve(option('--worktree', resolve(ROOT, '..', `${basename(ROOT)}-async${TIER === 2 ? '2' : ''}`)));
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
const WEB_APPS = ['realworld', 'weatherlight', 'completion-storm', 'video-player', 'caltrain', 'typetour', 'carousel', 'sparkline', 'svg-gallery', 'spark', 'markdown-stress', 'reflow', 'textflow', 'canvas-gallery', 'duo-lab', 'update-lab', 'native-fixture', 'photo-editor', 'recorder', 'fieldnotes', 'markdown', 'messages', 'interaction-gallery', 'motion-gallery'];
// The second lane: each platform built for its simulator, Caltrain as the app
// (its TypeScript-free build needs no Hermes for the platform).
const TIER_2 = [
  ['tvos', 'env', ['EXACT_JS_ENGINE=stub', 'bun', 'host/apple/build.mjs', '--tvos']],
];
function checks(sha) {
  if (TIER === 2) return TIER_2;
  const lane = laneTests();
  const apple = git(['diff', '--name-only', `${sha}^`, sha, '--', 'host/apple'], WT) !== '';
  const glue = [...new Bun.Glob('host/web/**/*.test.mjs').scanSync({ cwd: WT, onlyFiles: true })].sort().map(file => `./${file}`);
  return [
    ['build', 'cargo', ['build', ...workspace, '--all-targets', '--keep-going']],
    ['test', 'env', ['EXACT_PURE_CHROME_REQUIRED=1', 'cargo', 'test', ...workspace, '--lib', '--bins', '--tests', '--no-fail-fast']],
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
    // `tree --ax` against Chrome's own tree (LLP 1080.002 §3), over the
    // wasm Caltrain dist conformance just built; a missing dist fails it.
    ['web-ax-test', 'env', [`EXACT_WEB_DIST=${resolve(STATE_DIR, 'conform-wasm', 'caltrain')}`, 'EXACT_AX_REQUIRED=1', 'bun', 'test', './host/web/tests/accessibility-tree.test.mjs']],
    // The VS Code grammar under VS Code's own engine: a sample's scopes, and
    // every .contract file with no invalid token and nothing left open
    // (editors/vscode/README.md). Its word lists are checked by cargo test.
    ['vscode-grammar', 'bun', ['test', './editors/vscode']],
    // The JS target in the other browser engines, with Chrome as its oracle.
    // These remain async-only; a missing Playwright browser is a named failure
    // whose log gives the exact outside-the-repo install command.
    ['conform-firefox', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--browser', 'firefox', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--out', resolve(STATE_DIR, 'conform-firefox')]],
    ['conform-webkit', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--browser', 'webkit', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--out', resolve(STATE_DIR, 'conform-webkit')]],
    ...(apple ? [['ios', 'bun', ['host/apple/build.mjs', '--test', '--ios']]] : []),
    // Real touches through the XCTest runner on a simulator (LLP 1080.000 §6):
    // a runner that does not start fails here, never skips.
    ...(apple ? [['ios-touch', 'bun', ['scripts/smoke-touch.mjs', '--build']]] : []),
    // The Contract semantics (semantics/README.md; Charlie, 2026-10-03): the
    // Lean project builds with every proof checked (the app proofs among
    // them, over embeddings `difftest apps` checks are current), then the runner against
    // the semantics over the scripted corpus and a fixed random sweep (fixed
    // seeds, so a divergence is attributed to the commit that made it), and
    // the compiler's bytecode on the Lean VM model against the same,
    // and the Lean type checker against the Rust one on those programs and mutants,
    // and component expansion against the Lean expander and the component-level semantics.
    // The corpus and the explored programs also run on the web JS target (`--js`,
    // the second implementation, against the runner), and a smaller random sweep.
    // A `sorry` fails it: a proof that is not there is not checked. Every
    // part runs whatever the one before it found.
    ['semantics', 'sh', ['-c', [
      'export PATH="$HOME/.elan/bin:$PATH"; failed=0',
      'out=$(cd semantics && lake build 2>&1) || failed=1; echo "$out"',
      'if echo "$out" | grep -q "declaration uses .sorry."; then echo "error: semantics: a proof uses sorry"; failed=1; fi',
      // The shipped VM machine extracted (Charon, Aeneas) and proved against
      // Contract/Vm.lean (semantics/vm-extract/README.md); skips, naming the
      // tool, where Charon or Aeneas is not installed.
      'sh semantics/vm-extract/check.sh || failed=1',
      'cargo run -q -p contract-difftest -- apps || failed=1',
      'cargo run -q -p contract-difftest -- corpus --js || failed=1',
      'cargo run -q -p contract-difftest -- explore contract/corpus apps/*/app.contract --js || failed=1',
      'cargo run -q -p contract-difftest -- random --seed 1 --count 5000 || failed=1',
      'cargo run -q -p contract-difftest -- random --seed 1 --count 500 --js-only || failed=1',
      'cargo run -q -p contract-difftest -- lowering-corpus || failed=1',
      'cargo run -q -p contract-difftest -- lowering --seed 1 --count 300 || failed=1',
      'cargo run -q -p contract-difftest -- numbers --count 200000 || failed=1',
      'cargo run -q -p contract-difftest -- types --seed 1 --count 100 || failed=1',
      'cargo run -q -p contract-difftest -- expansion semantics/corpus --seed 1 --count 200 || failed=1',
      'exit $failed',
    ].join('\n')]],
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
  // Swift diagnostics by file and message: line numbers move with every edit.
  for (const m of log.replace(/\x1b\[[0-9;]*m/g, '').matchAll(/(?:^|\/)Sources\/(\S+?\.swift):\d+:\d+: error: (.+)$/gm)) found.add(`${name}: ${m[1]}: ${m[2].trim()}`);
  for (const m of log.matchAll(/^Diff in (\S+?):\d+:/gm)) found.add(`${name}: ${m[1].replace(WT + '/', '')} is not formatted`);
  // conform --strict: a failing step by target and step (the what varies run to run).
  for (const m of log.matchAll(/^FAIL (\S+) ([^:\n]+):/gm)) found.add(`${name}: ${m[1]} ${m[2]}`);
  for (const m of log.matchAll(/^\(fail\) (.+?) \[[\d.]+m?s\]$/gm)) found.add(`${name}: ${m[1]}`);
  // difftest: a case that diverged (on the JS target too), failed an expectation or was refused.
  for (const m of log.matchAll(/^(DIVERGE|DIVERGE-JS|ERROR-JS|EXPECT|EMIT|SCRIPT|REFUSED) (.+?)(?: at line \d+)?:/gm)) found.add(`${name}: ${m[1]} ${m[2]}`);
  // difftest types: the two checkers disagreed on a program or a mutant.
  for (const m of log.matchAll(/^LEAN (ACCEPTS|REFUSES) (.+?) \(/gm)) found.add(`${name}: LEAN ${m[1]} ${m[2]}`);
  for (const m of log.matchAll(/^error: (\S+\.lean):\d+:\d+: (.*)$/gm)) found.add(`${name}: ${m[1]} ${m[2]}`);
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
  const env = { ...process.env, HERMES_LEAN_SYS_OFFLINE: '1' };
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

function file(result, fresh, since) {
  const short = result.sha.slice(0, 8);
  const timing = Object.entries(result.checks).map(([n, c]) => `${n} ${c.seconds.toFixed(0)} s (exit ${c.status}, load ${c.load.toFixed(0)})`).join(' · ');
  const what = since ? `that ${since.slice(0, 8)} (the last commit it checked) did not have, from a commit in ${since.slice(0, 8)}..${short}` : `that ${short}'s parent did not have`;
  const body = [`The tier ${TIER} async lane found ${fresh.length} failure(s) ${what}:`, '',
    ...fresh.map(f => `- ${f}`), '', `Commit: ${short} ${result.subject}`, `Checks: ${timing}`,
    `Logs: ${resolve(STATE_DIR, result.sha.slice(0, 12))}/`].join('\n');
  issue(['new', `Async lane${TIER === 2 ? ' (tier 2)' : ''}: ${fresh.length} new failure(s) at ${short}`, '--systems', 'async lane',
    '--slug', `async${TIER === 2 ? '2' : ''}-${short}`, '--author', 'async lane (scripts/async.mjs)', '--body', body, '--quiet'], ROOT);
}

async function once(state) {
  git(['fetch', '-q', 'origin']);
  const tip = git(['rev-parse', BRANCH]);
  const pending = state.last
    ? git(['rev-list', '--first-parent', '--reverse', `${state.last}..${tip}`]).split('\n').filter(Boolean)
    : [tip];
  // The second lane, and the first under --newest, coalesces: only the newest commit is checked.
  const newest = TIER === 2 || process.argv.includes('--newest');
  for (const sha of newest ? pending.slice(-1) : pending) {
    const since = newest && pending.length > 1 ? state.last : null;
    const result = await check(sha);
    // A commit outside host/apple runs no iOS tests: the last ones stand.
    if (!result.checks.ios) result.failures.push(...(state.failures ?? []).filter(f => f.startsWith('ios: ')));
    // The first commit checked has no parent result: it sets the baseline,
    // since its failures cannot be attributed to it.
    const baseline = state.failures === undefined;
    const before = new Set(state.failures ?? []);
    const fresh = result.failures.filter(f => !before.has(f));
    console.log(`${sha.slice(0, 8)} ${result.subject}: ${result.failures.length} failure(s), ${baseline ? 'baseline' : `${fresh.length} new`}`);
    if (fresh.length && !baseline) file(result, fresh, since);
    Object.assign(state, { last: sha, failures: result.failures });
    writeFileSync(STATE, JSON.stringify(state, null, 2));
  }
}

if (!existsSync(WT)) git(['worktree', 'add', '--detach', WT, BRANCH]);
mkdirSync(STATE_DIR, { recursive: true });
const state = existsSync(STATE) ? JSON.parse(readFileSync(STATE, 'utf8')) : {};
if (option('--from')) state.last = git(['rev-parse', option('--from')]);
const interval = Number(option('--interval', TIER === 2 ? 3600 : 300)) * 1000;
for (;;) {
  await once(state);
  if (process.argv.includes('--once')) break;
  await new Promise(done => setTimeout(done, interval));
}
