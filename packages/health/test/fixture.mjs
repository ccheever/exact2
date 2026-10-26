// The conformance fixture (LLP 1133.001 D7), as every platform's suite makes
// it: an app `exact new` made outside the repo, its files replaced by
// `fixture/`'s, depending on @exact/health, @exact/pedometer and
// @exact/altimeter packed as a registry would carry them (`bun pm pack`) and
// installed, so each build is LLP 1133.001 D8's out-of-tree one. It names a privacy policy, which
// Health on Android requires (D4), and carries the agent's Health stores as
// its assets under `health-fixture/`, where the Android implementation reads
// them.
import { cpSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** The fixture app's directory and its bundle id. */
export function makeFixture() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-health-')), app = join(dir, 'healthfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const json = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8'));
  json.privacy_policy = 'https://example.com/privacy';
  writeFileSync(join(app, 'app.json'), JSON.stringify(json, null, 2));
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', json.app.id));
  mkdirSync(join(app, 'assets/health-fixture'), { recursive: true });
  for (const f of ['health.json', 'other-health.json']) cpSync(join(pkg, 'fixture', f), join(app, 'assets/health-fixture', f));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  const tarball = (from) => {
    const into = mkdtempSync(join(dir, 'pack-'));
    run('bun', ['pm', 'pack', '--destination', into, '--quiet'], from);
    return `file:${join(into, readdirSync(into).find((f) => f.endsWith('.tgz')))}`;
  };
  manifest.dependencies = {
    ...manifest.dependencies,
    '@exact/health': tarball(pkg),
    '@exact/pedometer': tarball(resolve(root, 'packages/pedometer')),
    '@exact/altimeter': tarball(resolve(root, 'packages/altimeter')),
  };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id: json.app.id };
}

/** Each testId's text in a `tree` reply. */
export function texts(tree) {
  const nodes = tree?.nodes ?? [], byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => (n ? (n.props?.text ?? (n.children ?? []).map((c) => text(byId.get(c))).join('')) : '');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
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
