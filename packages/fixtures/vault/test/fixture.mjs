// The fixture app, as each suite makes it: an app `exact new` made outside
// the repo, its view replaced by `fixture/app.contract`, depending on this
// package by `file:` and installed (LLP 1154 §3 item 4).
//
// usage: bun packages/fixtures/vault/test/fixture.mjs <dir>   (makes it at <dir>)
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { checker, run } from '../../tally/test/fixture.mjs';

export { checker, run };
export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../../..');

/** The fixture app's directory, made at `app` (a new temporary one by default). */
export function makeFixture(app = join(mkdtempSync(join(tmpdir(), 'exact-vault-')), 'vaultfixture')) {
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@fixture/vault': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return app;
}

if (import.meta.main) console.log(makeFixture(resolve(process.argv[2])));
