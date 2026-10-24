// The conformance fixtures of @exact/contacts and @exact/calendar (LLP
// 1133.001 D7, LLP 1133.018 §10), as every platform's suite makes them: an
// app `exact new` made outside the repo, its files replaced by a package's
// `test/fixture/`, depending on that package by `file:` and installed.
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const root = resolve(import.meta.dir, '../../..');
export { checker } from '../../camera/test/fixture.mjs';

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 26 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};

/** The fixture app of package `name` (`contacts` or `calendar`): its
 * directory, name and bundle id. `grants` replaces the fixture's grant lines. */
export function makeFixture(name, { grants } = {}) {
  const pkg = resolve(root, 'packages', name), here = join(pkg, 'test/fixture');
  const dir = mkdtempSync(join(tmpdir(), `exact-${name}-`)), app = join(dir, `${name}fixture`);
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  let ts = readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id);
  if (grants) ts = ts.replace(/export const grants = '[^']*';/, `export const grants = '${grants}';`);
  writeFileSync(join(app, 'app.ts'), ts);
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, [`@exact/${name}`]: `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, name: basename(app), id, remove: () => rmSync(dir, { recursive: true, force: true }) };
}

/** Builds `fixture` for `host` (`web`, `macos`, `ios`). */
export function build(fixture, host, extra = []) {
  const env = { ...process.env, EXACT_APP_DIR: fixture.app, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' };
  if (host === 'web') return run('bun', [resolve(root, 'host/web/build.mjs'), `${fixture.name}-web`], root, env);
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios'] : []), ...extra, `${fixture.name}-apple`], root, env);
}

/** One launch under the agent: each op's JSON reply, by its op. */
export function drive(fixture, host, flags, ops) {
  const args = [resolve(root, 'scripts/agent.mjs'), host, '--json', '--app', fixture.name, '--storage', `${fixture.name}-suite`, '--reset-storage',
    '--epoch', '2026-09-16T12:00:00Z', '--time-zone', 'UTC', ...flags, ...ops];
  const env = { ...process.env, EXACT_APP_DIR: fixture.app, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' };
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', env, maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => l.startsWith('{') || l.startsWith('[') || l.startsWith('"') || /^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
  return replies;
}

/** Each testId's text in a `tree` reply. */
export function texts(tree) {
  const nodes = tree?.nodes ?? [], byId = new Map(nodes.map(n => [n.id, n]));
  const text = n => n ? (n.props?.text ?? (n.children ?? []).map(c => text(byId.get(c))).join('')) : '';
  return Object.fromEntries(nodes.filter(n => n.props?.testId).map(n => [n.props.testId, text(n)]));
}

/** The held ticket for `capability` in a `state pending` reply. */
export const ticket = (pending, capability) => Array.isArray(pending) ? pending.find(p => p.device?.capability === capability) : undefined;
