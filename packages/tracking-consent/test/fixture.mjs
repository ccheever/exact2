// The fixture apps of LLP 1133.028's three packages (§8.3, §8.4 drive 11),
// made as every package's is: an app `exact new` made outside the repo, its
// files replaced by the package's `test/fixture/`, depending on the package
// by `file:` and installed.
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { root, run } from '../../contacts/test/fixture.mjs';

export { root, run };

/** Package `name`'s fixture app (`tracking-consent`, `attestation`, `age-range`): its directory, name and bundle id. */
export function makeFixture(name = 'tracking-consent') {
  const pkg = resolve(root, 'packages', name), here = join(pkg, 'test/fixture');
  const short = name.replace(/-.*/, '');
  const dir = mkdtempSync(join(tmpdir(), `exact-${short}-`)), app = join(dir, `${short}fixture`);
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) if (existsSync(join(here, f))) cpSync(join(here, f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, [`@exact/${name}`]: `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, name: basename(app), id, remove: () => rmSync(dir, { recursive: true, force: true }) };
}

const env = (fixture) => ({ ...process.env, EXACT_APP_DIR: fixture.app, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });

/** Builds the fixture for `host`: `web`, `macos`, `ios`, `tvos` or `linux`. */
export function build(fixture, host, extra = []) {
  if (host === 'web') return run('bun', [resolve(root, 'host/web/build.mjs'), `${fixture.name}-web`], root, env(fixture));
  if (host === 'linux') return run('bun', [resolve(root, 'scripts/build-linux.mjs'), fixture.name], root, env(fixture));
  const flag = { ios: ['--ios'], tvos: ['--tvos'], macos: [] }[host];
  return run('bun', [resolve(root, 'host/apple/build.mjs'), ...flag, ...extra, `${fixture.name}-apple`], root, env(fixture));
}

/** Opens the fixture under the agent on `host`. */
export async function launch(fixture, host, options = {}) {
  process.env.EXACT_APP_DIR = fixture.app;
  const { open } = await import(resolve(root, 'scripts/agent.mjs'));
  return open({ host, app: fixture.name, storage: `${fixture.name}-${Date.now().toString(36)}`, ...options });
}
