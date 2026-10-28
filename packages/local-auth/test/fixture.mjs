// The conformance fixture (LLP 1133.001 D7), as every platform's suite makes
// it: an app `exact new` made outside the repo, its files replaced by
// `fixture/`'s, depending on this package by `file:` and installed.
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** The fixture app's directory and its bundle id. */
export function makeFixture() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-local-auth-')), app = join(dir, 'lockfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/local-auth': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id };
}

/** A suite's PASS or FAIL line; `failed()` counts the FAILs. */
export function checker() {
  let failures = 0;
  const check = (where, name, ok, detail) => {
    if (!ok) failures++;
    console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
  };
  return { check, failed: () => failures };
}

/** Excord's fixture server, started on the port a development build of this checkout's Excord bakes
 * (`fixtureBackendSelection`, js/bake/src/typescript.mjs): its origin, its port, and `stop()`. */
export async function excordFixture() {
  const { backendOrigin } = await import(resolve(root, 'js/bake/src/typescript.mjs'));
  const origin = backendOrigin(resolve(root, 'apps/excord')), port = new URL(origin).port;
  const server = spawn('bun', [resolve(root, 'apps/excord/fixture/server.ts'), '--world', 'transcript', '--reset-on-identify', '--port', port], { stdio: ['ignore', 'pipe', 'inherit'] });
  await new Promise((ok, fail) => {
    server.stdout.once('data', ok);
    server.once('exit', (code) => fail(new Error(`the fixture server exited (${code}); is ${port} taken?`)));
  });
  return { origin, port, stop: () => server.kill() };
}
