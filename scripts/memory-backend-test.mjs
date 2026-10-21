#!/usr/bin/env bun
/**
 * memory-backend-test — a Snapback app's web tests against a backend of
 * their own: `snapback4 dev --memory` on a free loopback port, never the
 * origin the app is written against (Harmony's 4400 is a live server real
 * people use, and `fresh` empties the server it reaches). The async lane runs
 * Harmony's this way (scripts/async.mjs).
 *
 *   bun scripts/memory-backend-test.mjs apps/harmony/app
 *
 * The app's directory holds `app.json` and `app.ts`; its backend's (with
 * `snapback/schema.q` and `snapback4` in its package.json) is the one above.
 * It installs the backend's packages and builds the Snapback device's wasm
 * when they are missing, starts the server, checks that it is a temporary
 * (in-memory) store before any test reaches it, runs `exact.mjs test web`
 * with `EXACT_BACKEND_ORIGIN` naming it (which overrides `app.ts` and
 * `.exact/backend-origin` alike, so the build's `net.fetch` grant and every
 * `fresh` go there) into a web dist of its own, and stops the server however
 * the run ends.
 */
import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { createServer } from 'node:net';
import { resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** A loopback port nothing listens on now. */
const freePort = () => new Promise((ok, fail) => {
  const s = createServer();
  s.once('error', fail);
  s.listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => ok(port)); });
});

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

export async function main(argv) {
  if (!argv[0]) throw new Error('name the app\'s directory (bun scripts/memory-backend-test.mjs apps/harmony/app)');
  const app = resolve(ROOT, argv[0]), backend = resolve(app, '..');
  if (!existsSync(resolve(backend, 'snapback/schema.q'))) throw new Error(`${backend} has no snapback/schema.q: not a Snapback app's backend`);
  if (!existsSync(resolve(backend, 'node_modules/snapback4/bin/snapback4.js'))) {
    step('install the backend\'s packages', process.execPath, ['install', '--frozen-lockfile'], { cwd: backend });
  }
  // Cargo decides whether the device is current.
  step('build the Snapback device', 'node', [resolve(ROOT, 'snapback4/web/build.mjs'), 'assets/snapback4.wasm'], { cwd: app });
  const port = await freePort(), origin = `http://127.0.0.1:${port}`;
  const server = spawn(process.execPath, [resolve(backend, 'node_modules/snapback4/bin/snapback4.js'), 'dev', '--memory', '--port', String(port)],
    { cwd: backend, stdio: ['ignore', 'inherit', 'inherit'] });
  const stop = () => { if (server.exitCode === null && server.signalCode === null) server.kill('SIGTERM'); };
  process.once('SIGINT', () => { stop(); process.exit(130); });
  process.once('SIGTERM', () => { stop(); process.exit(143); });
  try {
    let up = null;
    for (let i = 0; i < 600 && !up; i++) {
      if (server.exitCode !== null) throw new Error(`snapback4 dev --memory exited ${server.exitCode} before it answered`);
      up = await status(origin);
      if (!up) await new Promise((ok) => setTimeout(ok, 100));
    }
    if (!up) throw new Error(`snapback4 dev --memory answered nothing at ${origin} in 60 s`);
    // Only a temporary store is ours to empty.
    if (up.temporary !== true) throw new Error(`the server at ${origin} is no temporary store (temporary: ${up.temporary}): refusing to run tests that empty it`);
    console.error(`memory-backend-test: ${origin}, a temporary store (pid ${up.pid})`);
    const dist = resolve(ROOT, 'target', 'memory-backend-test', argv[0].replaceAll('/', '_'), 'web-dist');
    const r = spawnSync(process.execPath, ['exact.mjs', 'test', 'web', ...argv.slice(1)], {
      cwd: app, stdio: 'inherit', env: { ...process.env, EXACT_BACKEND_ORIGIN: origin, EXACT_WEB_DIST: dist },
    });
    return r.status ?? 1;
  } finally {
    stop();
  }
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  try { process.exitCode = await main(process.argv.slice(2)); }
  catch (e) { console.error(`memory-backend-test: ${e.message}`); process.exitCode = 1; }
}
