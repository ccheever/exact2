#!/usr/bin/env bun
/**
 * async — the async lane (rules/RULES.md §Loop shape): every first-parent
 * commit on origin/main, checked out in a dedicated worktree with its own
 * target/, gets the five checks over the whole workspace plus the tests marked
 * `#[ignore = "async lane: …"]`. A failure that the previous commit did not
 * have is filed with `issue.mjs`, naming the commit that introduced it.
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
import { spawnSync } from 'node:child_process';
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
function checks() {
  const lane = laneTests();
  return [
    ['build', 'cargo', ['build', ...workspace, '--all-targets', '--keep-going']],
    ['test', 'cargo', ['test', ...workspace, '--lib', '--bins', '--tests', '--no-fail-fast']],
    ...(lane.length ? [['lane', 'cargo', ['test', ...workspace, '--lib', '--bins', '--tests', '--no-fail-fast', '--', '--ignored', ...lane]]] : []),
    ['clippy', 'cargo', ['clippy', ...workspace, '--all-targets', '--keep-going', '--', '-D', 'warnings']],
    ['fmt', 'cargo', ['fmt', '--all', '--', '--check']],
    ['caps', 'bun', ['scripts/caps.mjs']],
    ['boot', 'bun', ['scripts/boot.mjs']],
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
  for (const m of log.matchAll(/^Diff in (\S+?):\d+:/gm)) found.add(`${name}: ${m[1].replace(WT + '/', '')} is not formatted`);
  if (status !== 0 && !found.size) found.add(`${name}: exit ${status} (see log)`);
  return [...found];
}

function check(sha) {
  const dir = resolve(STATE_DIR, sha.slice(0, 12));
  mkdirSync(dir, { recursive: true });
  git(['checkout', '--detach', '--force', sha], WT);
  const env = { ...process.env };
  delete env.EXACT_UPDATE_TRUST; delete env.CARGO_TARGET_DIR;
  const installed = spawnSync('bun', ['install', '--frozen-lockfile'], { cwd: WT, env, encoding: 'utf8' });
  const result = { sha, subject: git(['log', '-1', '--format=%s', sha]), checks: {}, failures: [] };
  if (installed.status !== 0) result.failures.push(`install: bun install --frozen-lockfile exit ${installed.status}`);
  for (const [name, command, args] of checks()) {
    const logPath = resolve(dir, `${name}.log`), fd = openSync(logPath, 'w');
    const load = loadavg()[0], start = performance.now();
    const r = spawnSync(command, args, { cwd: WT, env, stdio: ['ignore', fd, fd] });
    closeSync(fd);
    const seconds = (performance.now() - start) / 1000;
    result.checks[name] = { command: [command, ...args].join(' '), status: r.status, seconds, load };
    result.failures.push(...failures(name, readFileSync(logPath, 'utf8'), r.status));
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
    const result = check(sha);
    const before = new Set(state.failures ?? []);
    const fresh = result.failures.filter(f => !before.has(f));
    console.log(`${sha.slice(0, 8)} ${result.subject}: ${result.failures.length} failure(s), ${fresh.length} new`);
    if (fresh.length) file(result, fresh);
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
