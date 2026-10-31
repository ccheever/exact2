// The site pattern (LLP 1153 D8, 1153.001 D5) at an API port of its own, set in .env.local: its tests pass there, a fault
// on `/planets` following the port; its list page is built ahead; and `serve` renders a planet's page for each request
// with the API's answer then, a change made after the build included. Its API is server.ts, run as `test` runs it
// (scripts/verify.mjs `startServer`), under this runner's NODE_ENV=test, with which Bun alone would skip .env.local.
import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';

const root = resolve(import.meta.dir, '../../..');
const build = resolve(root, 'host/web/build.mjs'), serve = resolve(root, 'host/web/serve.mjs');
const freePort = () => { const free = Bun.serve({ port: 0, fetch: () => new Response() }), { port } = free; free.stop(true); return port; };

test('the site pattern: tests at its API\'s port, a list built ahead, a planet rendered per request with the API as it is', async () => {
  const dir = mkdtempSync(resolve(root, 'target/site-pattern-'));
  let api, served;
  try {
    const { createApp, applyPattern } = await import('../../../game/new.mjs');
    const app = join(dir, 'site');
    createApp(app);
    applyPattern(app, 'site');
    const API = `http://127.0.0.1:${freePort()}`, port = freePort();
    writeFileSync(join(app, '.env.local'), `API_BASE_URL=${API}\n`);
    const env = { ...process.env, EXACT_BUN_LOADED: '' }; // a value this test gives is the environment's
    for (const name of ['EXACT_APP_DIR', 'EXACT_WEB_DIST', 'EXACT_BACKEND_ORIGIN', 'API_BASE_URL', 'CARGO_TARGET_DIR']) delete env[name];
    const tested = spawnSync(process.execPath, [join(app, 'exact.mjs'), 'test', 'web'], { cwd: app, env, encoding: 'utf8' });
    const said = tested.stdout + tested.stderr;
    expect(tested.status, said).toBe(0);
    expect(said).toContain(`test: bun server.ts at ${API}, without a database`);
    expect(said).toContain('4 passed, 0 failed');

    const dist = join(app, 'dist');
    const built = spawnSync(process.execPath, [build, app, '--out', dist], { env: { ...env, EXACT_APP_DIR: app }, encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    const home = readFileSync(join(dist, 'index.html'), 'utf8');
    expect(home).toContain('aria-sort="ascending"');
    expect(home).toContain('Neptune');

    const { appServer, startServer } = await import('../../../scripts/verify.mjs');
    const found = appServer(app, env);
    expect(found.url).toBe(API);
    api = await startServer(app, found, 'site-pattern');
    let log = '';
    served = spawn(process.execPath, [serve, '--port', String(port), '--loopback', dist], { env: { ...env, EXACT_APP_DIR: app }, stdio: ['ignore', 'pipe', 'pipe'] });
    for (const stream of [served.stdout, served.stderr]) stream.on('data', (b) => { log += b; });
    for (const end = Date.now() + 300_000; !log.includes('(serving');) {
      if (served.exitCode !== null || Date.now() > end) throw new Error(`serve did not listen: ${log}`);
      await Bun.sleep(100);
    }
    // The page as `serve` sends it, before any script on it runs.
    const page = async () => { const r = await fetch(`http://127.0.0.1:${port}/planet/7`, { headers: { accept: 'text/html' } }); return [r.status, await r.text()]; };
    let [status, html] = await page();
    expect(status).toBe(200);
    expect(html).toContain('<title>Uranus</title>');
    expect(html).toContain('>28<');
    const patched = await fetch(`${API}/planets/7`, { method: 'PATCH', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ moons: 29 }) });
    expect(patched.status).toBe(200);
    [status, html] = await page();
    expect(status).toBe(200);
    expect(html).toContain('>29<');
    expect(html).not.toContain('>28<');
  } finally {
    if (served?.exitCode === null) { const ended = new Promise((r) => served.once('exit', r)); served.kill('SIGTERM'); await ended; }
    await api?.stop();
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_800_000);
