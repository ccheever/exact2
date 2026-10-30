#!/usr/bin/env bun
/**
 * async — the async lane (rules/RULES.md §Loop shape): every first-parent
 * commit on origin/main, checked out in a dedicated worktree with its own
 * target/, gets the six checks over the whole workspace (its build covers the
 * sixth, `cargo check --workspace`) plus the tests marked
 * `#[ignore = "async lane: …"]`, every web host unit test found by one glob
 * and the script capture fixtures,
 * the web host's app-building document test in its own step, the web JS target's conformance run
 * (`host/web-js/conform.mjs --strict`), the separate game workspace and in-repo game tests,
 * the UIKit XCTests on a simulator and the tvOS and watchOS compiles (`build.mjs
 * --tvos --no-install`, `--watchos --no-install`) for any range beyond documentation-only edits,
 * the
 * Contract semantics (`semantics/README.md`) as four checks — the Lean proofs,
 * the extracted VM's proofs, the differential run against the Lean semantics
 * and the web JS target's against the runner — each skipped when its inputs
 * are those of a run that passed (scripts/async-inputs.mjs), then
 * `metrics.mjs --long` (every RULES budget;
 * a VIOLATION or FAILED row or a failed run counts, an OVER time does not — it
 * moves with the load). A failure that the previous commit did not have is filed with
 * `issue.mjs`, naming the commit that introduced it. A check past 2 h is
 * killed and filed as hung; the worktree's debug target is kept under 80 GiB.
 * Logs and timings (with the load average) stay in <worktree>/target/async/. What each commit's
 * checks found is also its git note, `refs/notes/async` (tier 2: `async2`), pushed to origin for
 * `scripts/known.mjs` (LLP 1146 D1).
 *
 *   bun scripts/async.mjs                 watch: check new commits every 300 s
 *   bun scripts/async.mjs --once          check what is pending, then exit
 *   bun scripts/async.mjs --from <rev>    start after <rev> instead of the tip
 *   bun scripts/async.mjs --worktree <dir> --interval <s> --branch <ref>
 *   bun scripts/async.mjs --explain [<rev>]   the semantics checks at <rev> (default
 *                                         the branch tip): run or skip, and what changed
 *
 * The semantics checks' fingerprints and outcomes are kept in
 * <worktree>/target/async/fingerprints/<check>/ (scripts/async-inputs.mjs).
 *
 * --branch (default origin/main) checks another line of history; the
 * worktree is the script's own and is checked out with --force.
 *
 * --tier 2 is the second lane: the platforms that ride on another host's
 * code (tvOS on the UIKit presenter, cross-target Linux/Windows compile checks, and an Android APK), in its own
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
import { availableParallelism, loadavg } from 'node:os';
import { basename, resolve } from 'node:path';
import { INPUTS, Inputs, Store, checkEnv, decide, outcomeOf } from './async-inputs.mjs';
import { main as issue } from './issue.mjs';
import { noteOf, publish } from './known.mjs';

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
// The second lane: tvOS uses Caltrain (its TypeScript-free build needs no
// Hermes) and installs it on an Apple TV simulator, which the first lane's
// compile does not; Linux and Windows compile their target-only branches:
// Windows with Windows Desk, on x64 and ARM64 (LLP 1115.004 D0.3), Linux with every app (`*-linux` is the
// host and each `apps/<app>/linux`), Zig compiling Hermes's C++ bindings too. Android
// builds Weatherlight's APK, which compiles the Android target's Rust, the
// reader's Kotlin and a module's (LLP 1133.002 stage 4, Charlie's say-so).
export function tierTwoChecks() {
  return [
    ['tvos', 'env', ['EXACT_JS_ENGINE=stub', 'bun', 'host/apple/build.mjs', '--tvos']],
    ['linux', 'env', ['CRATE_CC_NO_DEFAULTS=1', 'CC_x86_64_unknown_linux_gnu=zig cc -target x86_64-linux-gnu', 'CXX_x86_64_unknown_linux_gnu=zig c++ -target x86_64-linux-gnu', 'cargo', 'check', '--locked', '--keep-going', '--target', 'x86_64-unknown-linux-gnu', '-p', '*-linux']],
    ['windows', 'env', ['CRATE_CC_NO_DEFAULTS=1', 'CC_x86_64_pc_windows_gnu=zig cc -target x86_64-windows-gnu', 'cargo', 'check', '--locked', '--keep-going', '--target', 'x86_64-pc-windows-gnu', '-p', 'exact-windows', '-p', 'windows-desk']],
    // Rust has no GNU MinGW target for ARM64; LLVM's (`gnullvm`) stands in for MSVC here as GNU's does for x64.
    ['windows-arm64', 'env', ['CRATE_CC_NO_DEFAULTS=1', 'CC_aarch64_pc_windows_gnullvm=zig cc -target aarch64-windows-gnu', 'cargo', 'check', '--locked', '--keep-going', '--target', 'aarch64-pc-windows-gnullvm', '-p', 'exact-windows', '-p', 'windows-desk']],
    ['android', 'bun', ['scripts/agent-android.mjs', 'build', 'weatherlight', '--apk']],
  ];
}
export const tvosCompile = ['tvos', 'env', ['EXACT_JS_ENGINE=stub', 'bun', 'host/apple/build.mjs', '--tvos', '--no-install']];
export const watchosCompile = ['watchos', 'bun', ['host/apple/build.mjs', '--watchos', '--no-install']];
export const gameScaffoldCheck = ['game-scaffold', 'bun', ['test', 'game/proof-game.test.mjs', 'game/new.test.mjs', '--test-name-pattern', 'proof script|worldPixels|AGENTS.md lists vocab|new logic/tests file']];
export const gameCheck = ['game', 'cargo', ['test', '--manifest-path', 'game/Cargo.toml', '--workspace', '--locked', '--lib', '--bins', '--tests', '--no-fail-fast']];
// The T3 Code clone's own workspace (examples/t3-code/Cargo.toml): its build script bakes the whole app on
// one core for minutes, so the root's members and the six checks leave it out. Built, tested and
// linted here, in the root's target (pruned with it), where the units the two workspaces share are built
// once. Clippy's build of the build script is a unit of its own, so linting bakes once more. The root's
// `cargo fmt --all` does not reach this workspace, so its step checks the formatting too, after the tests.
const t3Code = ['--manifest-path', 'examples/t3-code/Cargo.toml', '--target-dir', 'target', '--workspace', '--locked'];
export const t3CodeChecks = [
  ['t3-code', 'sh', ['-c', [
    'failed=0',
    `cargo test ${t3Code.join(' ')} --lib --bins --tests --no-fail-fast || failed=1`,
    'cargo fmt --manifest-path examples/t3-code/Cargo.toml --all -- --check || failed=1',
    'exit $failed',
  ].join('\n')]],
  ['t3-code-clippy', 'cargo', ['clippy', ...t3Code, '--all-targets', '--keep-going', '--', '-D', 'warnings']],
];
// Glue files that edit an app's checked-in sources while they run
const EDITS_APPS = /\/dev-scheduler\.test\.mjs$/;
/** The glue step's worker processes: half the cores the machine's load leaves idle, at least one. Each worker's file
 * drives a browser or a build of its own, so two idle cores a worker keep them from contending (18 cores at load 3:
 * seven workers; at load 15: one) */
export function glueWorkers(cores = availableParallelism(), load = loadavg()[0]) {
  return Math.max(1, Math.floor((cores - load) / 2));
}
// Glue files left out: `bundle-size` reads RealWorld's production app.js (`EXACT_REALWORLD_JS`), which no step prepares
// before the glue; `document-agent` fails here (QUEUE.md, "The page's frames drop to one a second")
const LEFT_OUT = /^host\/web-js\/(bundle-size|document-agent)\.test\.mjs$/;
export function glueTests(root) {
  const files = ['host/web', 'host/web-js'].flatMap(dir => [...new Bun.Glob(`${dir}/**/*.test.{mjs,js}`).scanSync({ cwd: root, onlyFiles: true })])
    .filter(file => !LEFT_OUT.test(file)).sort().map(file => `./${file}`);
  // The glue lane is macOS: link.test reads Mach-O load commands with Apple tools.
  return [...files, './host/apple/link.test.mjs', './host/apple/cloak.test.mjs', ...['app-workspace', 'app-bake', 'app-apple', 'app-keychain', 'app-manifest', 'exact', 'check-watchdog', 'push', 'agent', 'agent-launch', 'agent-drag', 'tamber-server', 'tamber-watch', 'tamber-events-get', 'tamber-edit-held', 'test-children', 'scale-check', 'tool-drive', 'help-parity', 'tamber-builds', 'tamber-inspect', 'tamber-select-walk', 'tamber-scrub', 'tamber-dtcg', 'design-tool-levels', 'tamber-tool-answers', 'design-tool', 'design-tool-ops', 'design-tool-library', 'design-tool-canvas', 'design-tool-scrub', 'design-tool-kinds', 'design-tool-inspect', 'design-tool-noaxes', 'design-tool-commit', 'design-tool-fields', 'design-tool-dock', 'design-tool-edit-dock', 'design-tool-poll', 'design-tool-scale', 'library-gate', 'stage12-gate', 'position-reads', 'wasm-check', 'tamber-body-limit', 'tamber-upstream', 'tamber-request', 'tamber-tool-via', 'harmony-seed', 'design-tool-harness', 'tamber-timing', 'agent-clock-until', 'agent-held', 'android-manifest'].map(name => `./scripts/${name}.test.mjs`), './scripts/deploy-paths.test.mjs', './scripts/async-inputs.test.mjs', './scripts/coverage.test.mjs'];
}
export function checks(sha, since, selection = {}) {
  if (TIER === 2) return tierTwoChecks();
  const lane = selection.lane ?? laneTests();
  const apple = selection.apple ?? appleTouched(sha, since);
  const glue = selection.glue ?? glueTests(WT);
  return [
    ['build', 'cargo', ['build', ...workspace, '--all-targets', '--keep-going']],
    // This commit's web builds for the tests that drive one (below): Caltrain wasm with every
    // capability linked (the document, placement, motion, focusout and no-GPU tests) and JS,
    // and Fieldnotes as the dev loop builds it (its Rust drive, in `test`).
    prepareCheck,
    ['test', 'env', ['EXACT_REQUIRED=1', `EXACT_FIELDNOTES_DIST=${resolve(DISTS, 'fieldnotes')}`, 'cargo', 'test', ...workspace, '--tests', '--no-fail-fast']],
    ['game-lock', 'bun', ['game/app/shells.mjs', '--update-lock']],
    gameCheck,
    ['game-recipes', 'cargo', ['test', '--manifest-path', 'game/Cargo.toml', '-p', 'exact-game', '--locked', '--doc', '--no-fail-fast']],
    ['game-vocab', 'bun', ['test', 'game/vocab.test.mjs']],
    ['game-pins', 'bun', ['game/app/shells.mjs', '--test']],
    // The scaffold's standing checks: seconds, then the one that builds a game's tests twice.
    gameScaffoldCheck,
    ['game-assets-scaffold', 'bun', ['test', 'game/new.test.mjs', '--timeout', '3600000', '--test-name-pattern', 'assets scaffold']],
    ...t3CodeChecks,
    ...(lane.length ? [['lane', 'cargo', ['test', ...workspace, '--tests', '--no-fail-fast', '--', '--ignored', ...lane]]] : []),
    ['clippy', 'cargo', ['clippy', ...workspace, '--all-targets', '--keep-going', '--', '-D', 'warnings']],
    ['fmt', 'cargo', ['fmt', '--all', '--', '--check']],
    ['caps', 'bun', ['scripts/caps.mjs']],
    ['boot', 'bun', ['scripts/boot.mjs']],
    // Every web host unit test (LLP 1012.001.000 D9; Charlie, 2026-09-30)
    // and the source capture fixtures, in the existing async glue step.
    // Run the files in as many worker processes as the machine's idle cores allow when the step starts (`glueWorkers`)
    // A file that edits an app's sources runs after them, alone, since a page another file opens meanwhile would refuse
    // that app's build as stale
    ['glue', 'env', ['EXACT_GLUE_FAST=1', ...required, ...distEnv, 'sh', '-c', [
      'failed=0',
      `workers=$(bun -e "import { glueWorkers } from './scripts/async.mjs'; console.log(glueWorkers())") || { echo 'error: glue: scripts/async.mjs failed to count the idle cores (above); fix it and rerun the step'; exit 1; }`,
      `bun test --parallel="$workers" ${glue.filter(file => !EDITS_APPS.test(file)).join(' ')} || failed=1`,
      ...glue.filter(file => EDITS_APPS.test(file)).map(file => `bun test ${file} || failed=1`),
      'exit $failed',
    ].join('\n')]],
    // This document proof builds Weatherlight's wasm target. Keep it out of
    // glue's seconds loop while retaining it in the asynchronous lane.
    ['web-build-test', 'env', [...required, 'bun', 'test', './host/web/tests/document.test.mjs', '--test-name-pattern', "a TypeScript app's served document"]],
    // Harmony's end-to-end tests on the web, against a Snapback server of
    // their own in memory on a free port: never the origin app.ts names,
    // where a live server real people use listens on this machine.
    ['harmony', 'env', [...required, 'bun', 'scripts/memory-backend-test.mjs', 'apps/harmony/app']],
    // The web build's JS target against the wasm runner, step by step (LLP
    // 1071 §4; Charlie, 2026-09-28): minutes and a network, so never blocking.
    ['conform', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--build', '--linux', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--out', resolve(STATE_DIR, 'conform')]],
    // `tree --ax` against Chrome's own tree (LLP 1080.002 §3), and the wasm
    // host's presence preload and zero-viewport recovery, over this commit's
    // Caltrain dists; a missing dist fails it.
    ['web-ax-test', 'env', [...required, ...distEnv, 'bun', 'test', './host/web/tests/accessibility-tree.test.mjs', './host/web/tests/presence-preload.test.mjs', './host/web/tests/zero-viewport.test.mjs']],
    // The JS target in the other browser engines, with Chrome as its oracle.
    // These remain async-only; a missing Playwright browser is a named failure
    // whose log gives the exact outside-the-repo install command.
    ['conform-firefox', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--browser', 'firefox', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--js-from', resolve(STATE_DIR, 'conform'), '--out', resolve(STATE_DIR, 'conform-firefox')]],
    ['conform-webkit', 'bun', ['host/web-js/conform.mjs', ...WEB_APPS, '--synthetic', '--browser', 'webkit', '--strict', '--wasm-root', resolve(STATE_DIR, 'conform-wasm'), '--js-from', resolve(STATE_DIR, 'conform'), '--out', resolve(STATE_DIR, 'conform-webkit')]],
    ...(apple ? [['ios', 'bun', ['host/apple/build.mjs', '--test', '--ios']]] : []),
    // Real touches through the XCTest runner on a simulator (LLP 1080.000 §6):
    // a runner that does not start fails here, never skips.
    ...(apple ? [['ios-touch', 'bun', ['scripts/smoke-touch.mjs', '--build']]] : []),
    // tvOS compiles the UIKit presenter's Swift a second time, and an iOS-only
    // name in shared code breaks it (twice on 2026-10-11, seen by hand). Built
    // and signed, installed on nothing (Charlie, 2026-10-12): it needs the tvOS
    // SDK and Rust's tvOS simulator target, and no simulator. Tier 2 installs.
    ...(apple ? [tvosCompile] : []),
    // watchOS compiles ExactCore and the snapshot driver's Swift a third time, and a
    // name watchOS lacks breaks it (CACurrentMediaTime in HostPhases, 2026-10-26). It
    // builds Caltrain's watch bundle with the pinned nightly and rust-src
    // (webToolchainEnv names their install), and no simulator.
    ...(apple ? [watchosCompile] : []),
    ...semanticsChecks(),
    ['metrics', 'bun', ['scripts/metrics.mjs', '--long']],
  ];
}

const changedPaths = (sha, since, diff) => diff(['diff', '--name-only', since ?? `${sha}^`, sha], WT).split('\n').filter(Boolean);

// The lane owns the coverage of its checks (scripts/coverage.mjs): a missing
// prerequisite (a browser, a toolchain, a built fixture) fails a test instead
// of skipping it.
const required = ['EXACT_REQUIRED=1'];
const DISTS = resolve(STATE_DIR, 'dists');
export const prepareCheck = ['prepare', 'sh', ['-c', [
  `rm -rf ${JSON.stringify(DISTS)} && mkdir -p ${JSON.stringify(DISTS)}`,
  `EXACT_WEB_LINK=all EXACT_WEB_DIST=${JSON.stringify(resolve(DISTS, 'caltrain-wasm'))} bun host/web/build.mjs caltrain-web --wasm`,
  `EXACT_WEB_DIST=${JSON.stringify(resolve(DISTS, 'caltrain-js'))} bun host/web/build.mjs caltrain-web`,
  `EXACT_WEB_LINK=all EXACT_WEB_DIST=${JSON.stringify(resolve(DISTS, 'fieldnotes'))} bun host/web/build.mjs fieldnotes-web`,
].join(' && ')]];
const distEnv = [`EXACT_WEB_DIST=${resolve(DISTS, 'caltrain-wasm')}`, `EXACT_WEB_JS_DIST=${resolve(DISTS, 'caltrain-js')}`,
  `EXACT_NO_GPU_WASM_DIST=${resolve(DISTS, 'caltrain-wasm')}`, `EXACT_NO_GPU_JS_DIST=${resolve(DISTS, 'caltrain-js')}`,
  `EXACT_MOTION_DIST=${resolve(DISTS, 'caltrain-wasm')}`];

/** Settings a check inherits from the lane's own environment that change what it tests. */
export const ENV_DROPPED = ['EXACT_UPDATE_TRUST', 'CARGO_TARGET_DIR', 'EXACT_WEB_BROWSER', 'EXACT_JS_ENGINE', 'EXACT_JS_GPU_SURFACES',
  'EXACT_WEB_LINK', 'EXACT_WEB_DIST', 'EXACT_WEB_JS_DIST', 'EXACT_NO_GPU_WASM_DIST', 'EXACT_NO_GPU_JS_DIST',
  'EXACT_NO_GPU_CHROME', 'EXACT_MOTION_DIST', 'EXACT_GLUE_FAST', 'EXACT_REQUIRED', 'EXACT_BACKEND_ORIGIN'];
export const laneEnv = base => {
  const env = { ...base, HERMES_LEAN_SYS_OFFLINE: '1' };
  for (const k of Object.keys(env)) if (ENV_DROPPED.includes(k) || k.startsWith('EXACT_AGENT')) delete env[k];
  return env;
};

export function appleTouched(sha, since, diff = git) {
  const paths = changedPaths(sha, since, diff);
  // Markdown an app or example bundles is its input, not documentation.
  const bundled = path => path.startsWith('apps/') || path.startsWith('examples/');
  const documentation = path => (path.endsWith('.md') && !bundled(path)) ||
    ['llp/', 'issues/', 'rules/', 'docs/'].some(dir => path.startsWith(dir)) ||
    path === 'QUEUE.md';
  return paths.some(path => path && !documentation(path));
}

// The Contract semantics (semantics/README.md; Charlie, 2026-10-03), four
// checks, each with its inputs (scripts/async-inputs.mjs). A `sorry` fails
// `lean`: a proof that is not there is not checked. The sweeps' seeds are
// fixed, so a divergence is attributed to the commit that made it. Every part
// of a check runs whatever the one before it found.
const difftest = args => `cargo run -q -p contract-difftest -- ${args} || failed=1`;
const script = lines => ['sh', ['-c', ['failed=0', ...lines, 'exit $failed'].join('\n')]];
/** The semantics' environment (lake on PATH), with `extra`. */
const semanticsEnv = (base, extra) => checkEnv({ ...base, PATH: `${base.HOME}/.elan/bin:${base.PATH}` }, extra);
export function semanticsChecks(base = process.env) {
  const env = semanticsEnv(base);
  return [
    ['lean', ...script([
      'out=$(cd semantics && lake build 2>&1) || failed=1; echo "$out"',
      'if echo "$out" | grep -q "declaration uses .sorry."; then echo "error: semantics: a proof uses sorry"; failed=1; fi',
    ]), { env, inputs: INPUTS.lean }],
    // Exits 77 (UNAVAILABLE: not a pass, not a failure) where Charon or Aeneas is missing.
    ['vm-extract', 'sh', ['semantics/vm-extract/check.sh'], { env, inputs: INPUTS['vm-extract'] }],
    ['difftest', ...script([
      'apps', 'corpus', 'explore contract/corpus apps/*/app.contract', 'random --seed 1 --count 5000', 'lowering-corpus',
      'lowering --seed 1 --count 300', 'numbers --count 200000', 'types --seed 1 --count 100',
      'expansion semantics/corpus --seed 1 --count 200',
    ].map(difftest)), { env: semanticsEnv(base, { DIFFTEST_CACHE: '1' }), inputs: INPUTS.difftest }],
    ['difftest-js', ...script([
      'corpus --js-only', 'explore contract/corpus apps/*/app.contract --js-only', 'random --seed 1 --count 500 --js-only',
    ].map(difftest)), { env, inputs: INPUTS['difftest-js'] }],
  ];
}

/** What the semantics checks read at the worktree's checkout, read once for all four. */
const semanticsInputs = () => new Inputs(WT, semanticsEnv(process.env));

export function retainSkippedFailures(previous, result, conditionalNames = TIER === 1 ? ['lane', 'ios', 'ios-touch', 'tvos', 'watchos'] : []) {
  const conditional = new Set(conditionalNames);
  result.failures.push(...previous.filter(f => {
    const name = f.split(': ', 1)[0];
    return conditional.has(name) && !result.checks[name];
  }));
}

/** Every failure a log names, as stable strings (compared across commits). */
export function failures(name, log, status) {
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
  // xcodebuild's parallel testing names a failure by class and method only.
  for (const m of log.matchAll(/^Test case '(\w+)\.(\w+)\(\)' failed on /gm)) found.add(`${name}: ExactKitTests.${m[1]} ${m[2]} failed`);
  // Swift diagnostics by file and message: line numbers move with every edit.
  for (const m of log.replace(/\x1b\[[0-9;]*m/g, '').matchAll(/(?:^|\/)Sources\/(\S+?\.swift):\d+:\d+: error: (.+)$/gm)) found.add(`${name}: ${m[1]}: ${m[2].trim()}`);
  for (const m of log.matchAll(/^Diff in (\S+?):\d+:/gm)) found.add(`${name}: ${m[1].replace(WT + '/', '')} is not formatted`);
  // conform --strict: a failing step by target and step (the what varies run to run).
  for (const m of log.matchAll(/^FAIL (\S+) ([^:\n]+):/gm)) found.add(`${name}: ${m[1]} ${m[2]}`);
  for (const m of log.matchAll(/^\(fail\) (.+?) \[[\d.]+m?s\]$/gm)) found.add(`${name}: ${m[1]}`);
  // A bun step that selected no test passed on nothing (`-t` matching none exits 0).
  if (/^bun test v/m.test(log) && /matched 0 tests|had no matches|^Ran 0 tests/m.test(log)) found.add(`${name}: ran no tests`);
  // difftest: a case that diverged (on the JS target too), failed an expectation or was refused.
  for (const m of log.matchAll(/^(DIVERGE|DIVERGE-JS|ERROR-JS|EXPECT|EMIT|SCRIPT|REFUSED) (.+?)(?: at line \d+)?:/gm)) found.add(`${name}: ${m[1]} ${m[2]}`);
  // difftest apps: an app embedding that is not what the compiler makes of the app now.
  for (const m of log.matchAll(/^STALE \S*?(semantics\/Apps\/\S+?):/gm)) found.add(`${name}: STALE ${m[1]}`);
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

async function check(sha, since) {
  const dir = resolve(STATE_DIR, sha.slice(0, 12));
  mkdirSync(dir, { recursive: true });
  prune();
  git(['checkout', '--detach', '--force', sha], WT);
  const env = laneEnv(process.env);
  const installed = spawnSync('bun', ['install', '--frozen-lockfile'], { cwd: WT, env, encoding: 'utf8' });
  const result = { sha, subject: git(['log', '-1', '--format=%s', sha]), checks: {}, failures: [] };
  if (installed.status !== 0) result.failures.push(`install: bun install --frozen-lockfile exit ${installed.status}`);
  const store = new Store(resolve(STATE_DIR, 'fingerprints'));
  let inputs;
  for (const [name, command, args, opts] of checks(sha, since)) {
    const ran = { command: [command, ...args].join(' ') };
    let gated = null;
    if (opts) {
      const start = performance.now();
      gated = await decide(store, inputs ??= semanticsInputs(), name, command, args, opts);
      Object.assign(ran, gated.error ? { gateError: gated.error } : { fingerprint: gated.fp.digest, changed: gated.changed },
        { gateSeconds: (performance.now() - start) / 1000 });
      if (gated.passed) {
        result.checks[name] = { ...ran, skipped: `inputs of ${gated.passed.sha?.slice(0, 12)}'s pass` };
        continue;
      }
    }
    const logPath = resolve(dir, `${name}.log`), fd = openSync(logPath, 'w');
    const load = loadavg()[0], start = performance.now();
    const r = await run(command, args, opts?.env ?? env, fd);
    closeSync(fd);
    const seconds = (performance.now() - start) / 1000, log = readFileSync(logPath, 'utf8');
    result.checks[name] = { ...ran, status: r.status, seconds, load, ...(r.hung ? { hung: true } : {}) };
    const outcome = gated && outcomeOf(r.status, r.hung);
    if (gated?.fp) store.record(name, gated.fp, outcome, { sha });
    // A check that could not run here found nothing.
    if (outcome !== 'unavailable') result.failures.push(...failures(name, log, r.status));
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
  const timing = Object.entries(result.checks).map(([n, c]) => c.skipped ? `${n} skipped (${c.skipped})` : `${n} ${c.seconds.toFixed(0)} s (exit ${c.status}, load ${c.load.toFixed(0)})`).join(' · ');
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
    const result = await check(sha, since);
    // Conditional checks that did not run retain their last failures.
    retainSkippedFailures(state.failures ?? [], result);
    // The first commit checked has no parent result: it sets the baseline,
    // since its failures cannot be attributed to it.
    const baseline = state.failures === undefined;
    const before = new Set(state.failures ?? []);
    const fresh = result.failures.filter(f => !before.has(f));
    console.log(`${sha.slice(0, 8)} ${result.subject}: ${result.failures.length} failure(s), ${baseline ? 'baseline' : `${fresh.length} new`}`);
    if (fresh.length && !baseline) file(result, fresh, since);
    // What it found, on the commit as a git note other checkouts read (LLP 1146 D1, `known.mjs`).
    const unpublished = publish(noteOf(result, TIER));
    if (unpublished) console.log(`${sha.slice(0, 8)}: the note was not published: ${unpublished}`);
    Object.assign(state, { last: sha, failures: result.failures });
    writeFileSync(STATE, JSON.stringify(state, null, 2));
  }
}

if (import.meta.main) {
  if (process.argv.includes('--help')) {
    console.log('Usage: bun scripts/async.mjs [--once] [--from <rev>] [--worktree <dir>] [--interval <s>] [--branch <ref>] [--tier 1|2] [--newest] [--explain [<rev>]]');
    process.exit(0);
  }
  if (!existsSync(WT)) git(['worktree', 'add', '--detach', WT, BRANCH]);
  mkdirSync(STATE_DIR, { recursive: true });
  const state = existsSync(STATE) ? JSON.parse(readFileSync(STATE, 'utf8')) : {};
  if (option('--from')) state.last = git(['rev-parse', option('--from')]);
  if (process.argv.includes('--explain')) {
    const at = process.argv[process.argv.indexOf('--explain') + 1];
    git(['fetch', '-q', 'origin']);
    const sha = git(['rev-parse', at && !at.startsWith('--') ? at : BRANCH]);
    git(['checkout', '--detach', '--force', sha], WT);
    const store = new Store(resolve(STATE_DIR, 'fingerprints')), inputs = semanticsInputs();
    for (const [name, command, args, opts] of semanticsChecks()) {
      const { error, passed, changed } = await decide(store, inputs, name, command, args, opts);
      console.log(`${name}: ${error ? `run (the gate failed: ${error})` : passed ? 'skip' : `run (changed: ${changed.join(', ')})`}`);
    }
    process.exit(0);
  }
  const interval = Number(option('--interval', TIER === 2 ? 3600 : 300)) * 1000;
  for (;;) {
    await once(state);
    if (process.argv.includes('--once')) break;
    await new Promise(done => setTimeout(done, interval));
  }
}
