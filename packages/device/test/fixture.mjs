// The conformance fixture (LLP 1133.001 D7), as every platform's suite makes
// it: an app `exact new` made outside the repo, its files replaced by
// `fixture/`'s, depending on this package by `file:` and installed.
//
// usage: bun packages/device/test/fixture.mjs <dir>   (makes it at <dir>)
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

/** The fixture app's directory, made at `app` (a new temporary one by default), and its bundle id. `device` is the
 * package's dependency spec: this checkout's, or `EXACT_DEVICE_PACKAGE` (the seam suite's packed tarball). */
export function makeFixture(app = join(mkdtempSync(join(tmpdir(), 'exact-device-')), 'devicefixture'), { device = process.env.EXACT_DEVICE_PACKAGE ?? `file:${pkg}` } = {}) {
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/device': device };
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

/** Each testId's text in a session's tree. */
export async function texts(session) {
  const { nodes } = await session.tree(), byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? (n.children ?? []).map((c) => text(byId.get(c))).join('');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
}

if (import.meta.main) console.log(makeFixture(resolve(process.argv[2])).app);
