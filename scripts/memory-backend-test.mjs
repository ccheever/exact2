#!/usr/bin/env bun
/**
 * memory-backend-test — a Snapback app's web tests against a backend of
 * their own: `snapback4 dev --memory` on a port the server picks, never the
 * origin the app is written against (Harmony's 4400 is a live server real
 * people use, and `fresh` empties the server it reaches). The async lane runs
 * Harmony's this way (scripts/async.mjs).
 *
 *   bun scripts/memory-backend-test.mjs apps/harmony/app
 *
 * The app's directory holds `app.json`, `app.ts` and `app.test.contract`; its
 * backend's (with `snapback/schema.q` and `snapback4` in its package.json) is
 * the one above. It installs the backend's packages and builds the Snapback
 * device's wasm when they are missing, then:
 *   - starts the server binary itself (`--port 0 --memory --no-watch`, under
 *     this process's supervision, so it ends with it), reads the URL it
 *     prints, and requires that origin's `/dev/status` to be this child
 *     (`pid`) and a temporary store before anything else reaches it;
 *   - builds the web app into a directory of its own with
 *     `EXACT_BACKEND_ORIGIN` naming the server (which overrides `app.ts` and
 *     `.exact/backend-origin` alike), and requires the build's loopback
 *     `net.fetch` grants to be that origin alone;
 *   - runs the tests with the driver on that very build (`scripts/agent.mjs`,
 *     which builds nothing), so `fresh` resets only those grants, and then the
 *     app's `live-push.mjs` when it has one (a check that needs a write from
 *     another client mid-drive) on the same server and build;
 *   - stops every child however it ends (a signal included: no step starts
 *     after one), killing what has not exited after a grace period, and
 *     removes the build.
 * It takes no other arguments: one that chose another backend or build would
 * undo all of this.
 */
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { loopbackOrigins } from './agent-test.mjs';
import { resolveApp } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const GRACE_MS = 10_000;

/** The server's `/dev/status`, or null while it does not answer. */
async function status(origin) {
  try {
    const r = await fetch(`${origin}/dev/status`, { signal: AbortSignal.timeout(1000) });
    return r.ok ? await r.json() : null;
  } catch { return null; }
}

const step = (what, cmd, args, opts) => {
  const r = spawnSync(cmd, args, { stdio: 'inherit', ...opts });
  if (r.status !== 0) throw new Error(`${what}: ${cmd} ${args.join(' ')} exited ${r.status ?? r.error?.message}`);
};

const alive = (child) => child?.pid !== undefined && child.exitCode === null && child.signalCode === null;

/** Every process now: its parent and its start time, which, with its pid, names it (a pid is reused). */
function processes() {
  const r = spawnSync('ps', ['-A', '-o', 'pid=,ppid=,lstart='], { encoding: 'utf8' });
  const all = new Map();
  for (const line of (r.stdout ?? '').split('\n')) {
    const m = /^\s*(\d+)\s+(\d+)\s+(.+?)\s*$/.exec(line);
    if (m) all.set(Number(m[1]), { ppid: Number(m[2]), start: m[3] });
  }
  return all;
}

/** `pid`'s descendants now, deepest first (a build's own builder, the driver's browser), each `{pid, start}`. */
function descendants(pid, all = processes()) {
  const kids = new Map();
  for (const [p, { ppid }] of all) kids.set(ppid, [...(kids.get(ppid) ?? []), p]);
  const out = [];
  const walk = (p) => { for (const k of kids.get(p) ?? []) { walk(k); out.push({ pid: k, start: all.get(k).start }); } };
  walk(pid);
  return out;
}

/** `sig` to each of `procs` that is still the same process (its pid and start time), never one that reused a pid. */
function signal(procs, sig) {
  const all = processes();
  for (const { pid, start } of procs) if (all.get(pid)?.start === start) try { process.kill(pid, sig); } catch { /* gone */ }
}

/** `child` and everything it started stopped: SIGTERM, then SIGKILL to what remains after the grace period. */
async function stop(child) {
  if (!alive(child)) return;
  const tree = descendants(child.pid);
  const exited = new Promise((ok) => child.once('exit', ok));
  signal(tree, 'SIGTERM');
  child.kill('SIGTERM');
  const timer = setTimeout(() => { signal(tree, 'SIGKILL'); if (alive(child)) child.kill('SIGKILL'); }, GRACE_MS);
  await exited;
  clearTimeout(timer);
  // What outlived its parent's exit, and what those started since: only processes still the ones seen.
  const all = processes(), same = tree.filter(({ pid, start }) => all.get(pid)?.start === start);
  const left = [...same, ...same.flatMap(({ pid }) => descendants(pid, all))];
  if (left.length) { signal(left, 'SIGTERM'); await new Promise((ok) => setTimeout(ok, 1000)); signal(left, 'SIGKILL'); }
}

export async function main(argv) {
  if (argv.length !== 1) throw new Error('name the app\'s directory alone (bun scripts/memory-backend-test.mjs apps/harmony/app): another argument could choose another backend or build');
  const app = resolve(ROOT, argv[0]), backend = resolve(app, '..');
  if (!existsSync(resolve(backend, 'snapback/schema.q'))) throw new Error(`${backend} has no snapback/schema.q: not a Snapback app's backend`);
  if (!existsSync(resolve(backend, 'node_modules/snapback4/package.json'))) {
    step('install the backend\'s packages', process.execPath, ['install', '--frozen-lockfile'], { cwd: backend });
  }
  // Cargo decides whether the device is current.
  step('build the Snapback device', process.execPath, [resolve(ROOT, 'snapback4/web/build.mjs'), 'assets/snapback4.wasm'], { cwd: app });
  // The binary itself, not the npm wrapper: `/dev/status`'s `pid` is then this child's.
  const { resolveBinary } = await import(pathToFileURL(resolve(backend, 'node_modules/snapback4/dist/bin-path.js')).href);

  // Every child, the one cleanup, and whether a signal asked for it: no step starts after one.
  const children = [];
  let cancelled = false, cleaning = null;
  const work = resolve(ROOT, 'target', 'memory-backend-test');
  mkdirSync(work, { recursive: true });
  const dir = mkdtempSync(resolve(work, `${argv[0].replaceAll('/', '_')}-`)), dist = resolve(dir, 'web-dist');
  const cleanup = () => cleaning ??= (async () => {
    for (const child of [...children].reverse()) await stop(child);
    rmSync(dir, { recursive: true, force: true });
  })();
  const onSignal = (code) => () => { cancelled = true; cleanup().then(() => process.exit(code)); };
  process.once('SIGINT', onSignal(130));
  process.once('SIGTERM', onSignal(143));
  const check = () => { if (cancelled) throw new Error('stopped by a signal'); };
  /** `cmd` started and tracked: its exit code, or its spawn error thrown. */
  const run = (cmd, args, opts) => {
    check();
    const child = spawn(cmd, args, opts);
    children.push(child);
    const done = new Promise((ok, fail) => {
      child.once('error', fail);
      child.once('exit', (code) => ok(code ?? 1));
    });
    // (observed at once: a spawn that fails is thrown where the step awaits it)
    done.catch(() => {});
    return { child, done };
  };

  try {
    const { child: server, done: serverDone } = run(resolveBinary(), ['dev', '--port', '0', '--memory', '--no-watch'],
      { cwd: backend, stdio: ['ignore', 'pipe', 'inherit'], env: { ...process.env, SNAPBACK4_SUPERVISOR_PID: String(process.pid) } });
    // Readiness is the first stdout line, the listening URL.
    if (!server.stdout) throw await serverDone.then((code) => new Error(`snapback4 dev --memory did not start (exit ${code})`), (e) => e);
    const lines = createInterface({ input: server.stdout });
    let timer;
    const url = await Promise.race([
      new Promise((ok) => lines.once('line', (line) => ok(line.trim()))),
      serverDone.then((code) => { throw new Error(`snapback4 dev --memory exited ${code} before it printed its URL`); }),
      new Promise((_, fail) => { timer = setTimeout(() => fail(new Error('snapback4 dev --memory printed no URL in 60 s')), 60_000); }),
    ]).finally(() => { clearTimeout(timer); lines.close(); });
    server.stdout.resume();
    check();
    const origin = new URL(url).origin;
    if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(origin)) throw new Error(`snapback4 dev printed ${url}, not a loopback origin`);
    const up = await status(origin);
    check();
    // Ours, and only a temporary store is ours to empty.
    if (up?.pid !== server.pid) throw new Error(`${origin}/dev/status names pid ${up?.pid}, not the server this run started (${server.pid})`);
    if (up.temporary !== true) throw new Error(`the server at ${origin} is no temporary store (temporary: ${up.temporary}): refusing to run tests that empty it`);
    console.error(`memory-backend-test: ${origin}, a temporary store (pid ${up.pid})`);
    const env = { ...process.env, EXACT_BACKEND_ORIGIN: origin, EXACT_WEB_DIST: dist, EXACT_APP_DIR: app };
    const built = await run(process.execPath, ['host/web/build.mjs', resolveApp(app).crate('web')], { cwd: ROOT, stdio: 'inherit', env }).done;
    check();
    if (built !== 0) throw new Error(`the web build against ${origin} exited ${built}`);
    const grants = loopbackOrigins(dist);
    if (grants.length !== 1 || grants[0] !== origin) throw new Error(`the build's loopback grants are ${grants.join(', ') || 'none'}, not ${origin} alone: refusing to run tests whose fresh resets them`);
    // The driver on the build just checked: it builds nothing.
    const tested = await run(process.execPath, ['scripts/agent.mjs', 'web', '--app', app, '--test', resolve(app, 'app.test.contract')],
      { cwd: ROOT, stdio: 'inherit', env }).done;
    if (tested !== 0 || !existsSync(resolve(app, 'live-push.mjs'))) return tested;
    check();
    return await run(process.execPath, [resolve(app, 'live-push.mjs')], { cwd: ROOT, stdio: 'inherit', env }).done;
  } finally {
    await cleanup();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = await main(process.argv.slice(2)); }
  catch (e) { console.error(`memory-backend-test: ${e.message}`); process.exitCode = 1; }
}
