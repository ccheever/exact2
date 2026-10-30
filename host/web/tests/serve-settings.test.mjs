// serve.mjs serves a build as this environment's settings make it (LLP 1153 D4): a setting the environment changes is
// built again when the app's sources are beside the build, and refused, naming both values, when they are not.
import { test, expect } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';

const serve = resolve(import.meta.dir, '../serve.mjs'), build = resolve(import.meta.dir, '../build.mjs');

test('serve builds a changed setting again beside its sources, and refuses a copied build', async () => {
  const { createApp } = await import('../../../game/new.mjs');
  const root = mkdtempSync(resolve(import.meta.dir, '../../../target/serve-settings-')), app = join(root, 'app');
  let child;
  try {
    createApp(app);
    const manifest = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8'));
    writeFileSync(join(app, 'app.json'), JSON.stringify({ ...manifest, env: ['API_BASE_URL'] }));
    writeFileSync(join(app, 'app.contract'), 'component App\n  resource origin = origin() as shape string else empty()\n  view\n    text origin\n');
    writeFileSync(join(app, 'app.ts'), `import type { Answer } from './app.contract.d.ts';\nexport const appId = ${JSON.stringify(manifest.app.id)};\nconst API = import.meta.env.API_BASE_URL;\nexport const grants = \`net.fetch \${API}\`;\nexport const answer: Answer = () => API;\n`);
    // Its own target/web-dist, which a suite's shared dist or target would move.
    const env = { ...process.env, EXACT_APP_DIR: app };
    for (const name of ['EXACT_WEB_DIST', 'CARGO_TARGET_DIR', 'EXACT_BACKEND_ORIGIN', 'API_BASE_URL']) delete env[name];
    const built = spawnSync(process.execPath, [build, app], { env: { ...env, API_BASE_URL: 'http://a.example' }, encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    const dist = join(app, 'target/web-dist'), copy = join(root, 'copy');
    cpSync(dist, copy, { recursive: true });
    const free = Bun.serve({ port: 0, fetch: () => new Response() }), port = free.port;
    free.stop(true);
    // Copied away from its sources: refused before it listens.
    const refused = spawnSync(process.execPath, [serve, String(port), '--loopback', '--origin', copy], { env: { ...env, API_BASE_URL: 'http://b.example' }, encoding: 'utf8', timeout: 60_000 });
    expect(refused.status).toBe(2);
    expect(refused.stderr).toContain('import.meta.env.API_BASE_URL was built as "http://a.example" (environment) and is "http://b.example" (environment) here, and the app\'s sources are not beside this build');
    // Beside them: built again with the new value, then served.
    let log = '';
    child = spawn(process.execPath, [serve, String(port), '--loopback', '--origin', dist], { env: { ...env, API_BASE_URL: 'http://b.example' }, stdio: ['ignore', 'pipe', 'pipe'] });
    for (const stream of [child.stdout, child.stderr]) stream.on('data', (b) => { log += b; });
    const listening = await new Promise((done) => {
      const end = Date.now() + 240_000;
      const poll = () => (log.includes('(serving') ? done(true) : child.exitCode !== null || Date.now() > end ? done(false) : setTimeout(poll, 100));
      poll();
    });
    expect(listening, log).toBe(true);
    expect(log).toContain('import.meta.env.API_BASE_URL was built as "http://a.example" (environment) and is "http://b.example" (environment) here: building it again');
    const module = await (await fetch(`http://127.0.0.1:${port}/app.js`)).text();
    expect(module).toContain('http://b.example');
    expect(module).not.toContain('http://a.example');
  } finally {
    if (child?.exitCode === null) { const ended = new Promise((r) => child.once('exit', r)); child.kill('SIGTERM'); await ended; }
    rmSync(root, { recursive: true, force: true });
  }
}, 600_000);
