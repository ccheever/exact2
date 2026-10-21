// The conformance fixture (LLP 1133.001 D7), as every platform's suite makes
// it: an app `exact new` made outside the repo, its files replaced by
// `fixture/`'s, depending on this package by `file:` and installed.
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** The fixture app's directory and its bundle id. `dependencies` are added to
 * its package.json beside this package (`file:` by default); `approximate`
 * grants the approximate line (D7) in place of the precise one. */
export function makeFixture({ geolocation = `file:${pkg}`, dependencies = {}, approximate = false } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-geolocation-')), app = join(dir, 'geofixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  let ts = readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id);
  if (approximate) ts = ts.replace("'device.geolocation ", "'device.geolocation-approximate ");
  writeFileSync(join(app, 'app.ts'), ts);
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/geolocation': geolocation, ...dependencies };
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
