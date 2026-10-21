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
 * The app's directory holds `app.json` and `app.ts`; its backend's (with
 * `snapback/schema.q` and `snapback4` in its package.json) is the one above.
 * It installs the backend's packages and builds the Snapback device's wasm
 * when they are missing, then:
 *   - starts the server binary itself (`--port 0 --memory --no-watch`), reads
 *     the URL it prints, and requires that origin's `/dev/status` to be this
 *     process (`pid`) and a temporary store before anything else reaches it;
 *   - builds the web app into a directory of its own with
 *     `EXACT_BACKEND_ORIGIN` naming the server (which overrides `app.ts` and
 *     `.exact/backend-origin` alike), and requires the build's loopback
 *     `net.fetch` grants to be that origin alone;
 *   - runs `exact.mjs test web` with the same environment, whose build (the
 *     same inputs, so the same grants: a second when nothing changed) is the
 *     one checked, and whose `fresh` resets only those grants;
 *   - stops the test run and the server however it ends (a signal included),
 *     killing what has not exited after a grace period, and removes the build.
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

const alive = (child) => child && child.exitCode === null && child.signalCode === null;

/** `child` stopped: SIGTERM, then SIGKILL if it has not exited within the grace period. */
async function stop(child) {
  if (!alive(child)) return;
  const exited = new Promise((ok) => child.once('exit', ok));
  child.kill('SIGTERM');
  const timer = setTimeout(() => { if (alive(child)) child.kill('SIGKILL'); }, GRACE_MS);
  await exited;
  clearTimeout(timer);
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
  const children = { server: null, tests: null };
  const work = resolve(ROOT, 'target', 'memory-backend-test');
  mkdirSync(work, { recursive: true });
  const dir = mkdtempSync(resolve(work, `${argv[0].replaceAll('/', '_')}-`)), dist = resolve(dir, 'web-dist');
  const end = async (code) => { await stop(children.tests); await stop(children.server); rmSync(dir, { recursive: true, force: true }); process.exit(code); };
  process.once('SIGINT', () => end(130));
  process.once('SIGTERM', () => end(143));
  try {
    const server = children.server = spawn(resolveBinary(), ['dev', '--port', '0', '--memory', '--no-watch'],
      { cwd: backend, stdio: ['ignore', 'pipe', 'inherit'] });
    // Readiness is the first stdout line, the listening URL.
    const url = await new Promise((ok, fail) => {
      const timer = setTimeout(() => fail(new Error('snapback4 dev --memory printed no URL in 60 s')), 60_000);
      createInterface({ input: server.stdout }).once('line', (line) => { clearTimeout(timer); ok(line.trim()); });
      server.once('exit', (code) => { clearTimeout(timer); fail(new Error(`snapback4 dev --memory exited ${code} before it printed its URL`)); });
    });
    server.stdout.resume();
    const origin = new URL(url).origin;
    if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(origin)) throw new Error(`snapback4 dev printed ${url}, not a loopback origin`);
    const up = await status(origin);
    // Ours, and only a temporary store is ours to empty.
    if (up?.pid !== server.pid) throw new Error(`${origin}/dev/status names pid ${up?.pid}, not the server this run started (${server.pid})`);
    if (up.temporary !== true) throw new Error(`the server at ${origin} is no temporary store (temporary: ${up.temporary}): refusing to run tests that empty it`);
    console.error(`memory-backend-test: ${origin}, a temporary store (pid ${up.pid})`);
    const env = { ...process.env, EXACT_BACKEND_ORIGIN: origin, EXACT_WEB_DIST: dist, EXACT_APP_DIR: app };
    step('build the web app against it', process.execPath, ['host/web/build.mjs', resolveApp(app).crate('web')], { cwd: ROOT, env });
    const grants = loopbackOrigins(dist);
    if (grants.length !== 1 || grants[0] !== origin) throw new Error(`the build's loopback grants are ${grants.join(', ') || 'none'}, not ${origin} alone: refusing to run tests whose fresh resets them`);
    const tests = children.tests = spawn(process.execPath, ['exact.mjs', 'test', 'web'], { cwd: app, stdio: 'inherit', env });
    return await new Promise((ok) => tests.once('exit', (c) => ok(c ?? 1)));
  } finally {
    await stop(children.tests);
    await stop(children.server);
    rmSync(dir, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = await main(process.argv.slice(2)); }
  catch (e) { console.error(`memory-backend-test: ${e.message}`); process.exitCode = 1; }
}
