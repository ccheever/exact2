// serve.mjs serves a build as this environment's settings make it (LLP 1153 D4): a setting the environment changes is
// built again, on the build's own target, when the app's sources are beside the build, and refused, naming both
// values, when they are not.
import { test, expect } from 'bun:test';
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { need } from '../../../scripts/coverage.mjs';
import { WEB_TOOLCHAIN } from '../../../scripts/app.mjs';

const serve = resolve(import.meta.dir, '../serve.mjs'), build = resolve(import.meta.dir, '../build.mjs');
const changed = 'import.meta.env.API_BASE_URL was built as "http://a.example" (environment) and is "http://b.example" (environment) here';

/** A new app in `root` whose data module answers and grants the setting API_BASE_URL, and the environment its builds
 * and servers run in. */
async function settingApp(root) {
  const { createApp } = await import('../../../game/new.mjs');
  const app = join(root, 'app');
  createApp(app);
  const manifest = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8'));
  writeFileSync(join(app, 'app.json'), JSON.stringify({ ...manifest, env: ['API_BASE_URL'] }));
  writeFileSync(join(app, 'app.contract'), 'component App\n  resource origin = origin() as shape string else empty()\n  view\n    text origin\n');
  writeFileSync(join(app, 'app.ts'), `import type { Answer } from './app.contract.d.ts';\nexport const appId = ${JSON.stringify(manifest.app.id)};\nconst API = import.meta.env.API_BASE_URL;\nexport const grants = \`net.fetch \${API}\`;\nexport const answer: Answer = () => API;\n`);
  const env = { ...process.env, EXACT_APP_DIR: app };
  for (const name of ['EXACT_WEB_DIST', 'EXACT_BACKEND_ORIGIN', 'API_BASE_URL']) delete env[name];
  return { app, env };
}

const freePort = () => { const free = Bun.serve({ port: 0, fetch: () => new Response() }), { port } = free; free.stop(true); return port; };

/** `serve.mjs` on `dist` under `env`, until it listens or ends: the child, what it said, whether it listens. */
async function served(dist, port, env) {
  let log = '';
  const child = spawn(process.execPath, [serve, String(port), '--loopback', '--origin', dist], { env, stdio: ['ignore', 'pipe', 'pipe'] });
  for (const stream of [child.stdout, child.stderr]) stream.on('data', (b) => { log += b; });
  const listening = await new Promise((done) => {
    const end = Date.now() + 900_000;
    const poll = () => (log.includes('(serving') ? done(true) : child.exitCode !== null || Date.now() > end ? done(false) : setTimeout(poll, 100));
    poll();
  });
  return { child, log: () => log, listening };
}
const stop = async (child) => { if (child?.exitCode === null) { const ended = new Promise((r) => child.once('exit', r)); child.kill('SIGTERM'); await ended; } };

test('serve builds a changed setting again beside its sources, and refuses a copied build', async () => {
  const root = mkdtempSync(resolve(import.meta.dir, '../../../target/serve-settings-'));
  let server;
  try {
    const { app, env } = await settingApp(root);
    delete env.CARGO_TARGET_DIR; // its own target/web-dist, which a suite's shared target would move
    const built = spawnSync(process.execPath, [build, app], { env: { ...env, API_BASE_URL: 'http://a.example' }, encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    const dist = join(app, 'target/web-dist'), copy = join(root, 'copy'), port = freePort();
    cpSync(dist, copy, { recursive: true });
    // Copied away from its sources: refused before it listens.
    const refused = spawnSync(process.execPath, [serve, String(port), '--loopback', '--origin', copy], { env: { ...env, API_BASE_URL: 'http://b.example' }, encoding: 'utf8', timeout: 60_000 });
    expect(refused.status).toBe(2);
    expect(refused.stderr).toContain(`${changed}, and the app's sources are not beside this build`);
    // Beside them: built again with the new value, then served.
    server = await served(dist, port, { ...env, API_BASE_URL: 'http://b.example' });
    expect(server.listening, server.log()).toBe(true);
    expect(server.log()).toContain(`${changed}: building it again`);
    const module = await (await fetch(`http://127.0.0.1:${port}/app.js`)).text();
    expect(module).toContain('http://b.example');
    expect(module).not.toContain('http://a.example');
  } finally {
    await stop(server?.child);
    rmSync(root, { recursive: true, force: true });
  }
}, 600_000);

// A build on the wasm target says no target in its marker, and takes its settings from its paired module's receipt.
// Delivery's bake (`--bake`) writes the same marker without linking the wasm.
test('serve refuses a copied wasm-target build whose setting the environment changed', async () => {
  const root = mkdtempSync(resolve(import.meta.dir, '../../../target/serve-settings-'));
  try {
    const { app, env } = await settingApp(root);
    const dist = join(root, 'baked');
    const built = spawnSync(process.execPath, [build, app, '--bake'], { env: { ...env, EXACT_WEB_DIST: dist, API_BASE_URL: 'http://a.example' }, encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    const marker = JSON.parse(readFileSync(join(dist, '.exact-build.json'), 'utf8'));
    expect([marker.target ?? null, marker.env]).toEqual([null, { API_BASE_URL: { value: 'http://a.example', source: 'environment' } }]);
    const refused = spawnSync(process.execPath, [serve, String(freePort()), '--loopback', '--origin', dist], { env: { ...env, API_BASE_URL: 'http://b.example' }, encoding: 'utf8', timeout: 60_000 });
    expect(refused.status).toBe(2);
    expect(refused.stderr).toContain(`${changed}, and the app's sources are not beside this build`);
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 1_800_000);

// Why the wasm target cannot link here, or null: the pinned nightly's own linker (WEB_TOOLCHAIN, scripts/app.mjs).
function wasmLinker() {
  const sysroot = spawnSync('rustup', ['run', WEB_TOOLCHAIN, 'rustc', '--print', 'sysroot'], { encoding: 'utf8' });
  if (sysroot.status !== 0) return `the ${WEB_TOOLCHAIN} toolchain: ${(sysroot.stderr || sysroot.error?.message || '').trim().split('\n')[0]}`;
  const host = spawnSync('rustc', ['-vV'], { encoding: 'utf8' }).stdout?.match(/^host: (.*)$/m)?.[1];
  const lld = spawnSync(resolve(sysroot.stdout.trim(), 'lib/rustlib', host ?? '', 'bin/rust-lld'), ['-flavor', 'wasm', '--version'], { encoding: 'utf8' });
  return lld.status === 0 ? null : `${WEB_TOOLCHAIN}'s rust-lld does not run: ${(lld.stderr || lld.error?.message || '').trim().split('\n')[0]}`;
}

need('the web toolchain', wasmLinker())('serve builds a wasm-target build\'s changed setting again on the wasm target', async () => {
  const root = mkdtempSync(resolve(import.meta.dir, '../../../target/serve-settings-'));
  let server;
  try {
    const { app, env } = await settingApp(root);
    const dist = join(root, 'wasm');
    const built = spawnSync(process.execPath, [build, app, '--wasm'], { env: { ...env, EXACT_WEB_DIST: dist, API_BASE_URL: 'http://a.example' }, encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    const port = freePort();
    server = await served(dist, port, { ...env, EXACT_WEB_DIST: dist, API_BASE_URL: 'http://b.example' });
    expect(server.listening, server.log()).toBe(true);
    expect(server.log()).toContain(`${changed}: building it again`);
    // Still the wasm target, now with the new value in its paired module.
    expect(existsSync(join(dist, 'app.wasm'))).toBe(true);
    expect(JSON.parse(readFileSync(join(dist, '.exact-build.json'), 'utf8')).target ?? null).toBe(null);
    expect(await (await fetch(`http://127.0.0.1:${port}/app.js`)).text()).toContain('http://b.example');
  } finally {
    await stop(server?.child);
    rmSync(root, { recursive: true, force: true });
  }
}, 1_800_000);
