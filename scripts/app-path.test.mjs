// `--app <path>` (the lead's 08:39Z; x2-perf4's stage 10 C2 found a crate/ in
// apps/theme-fixture): a path names an app as EXACT_APP_DIR does, so its name
// is its directory's and nothing built from the name (the JS build's module
// crate, the dev loop's GPU side root, the JS build's GPU cache) lands in the
// app's own directory.
import { afterEach, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative, resolve } from 'node:path';
import { authoredTests, resolveApp } from './app.mjs';

const ROOT = resolve(import.meta.dir, '..');
const before = process.env.EXACT_APP_DIR;
afterEach(() => { if (before === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = before; });

test('an absolute or relative path to an app resolves to that app, named by its directory', () => {
  const dir = realpathSync(resolve(ROOT, 'apps/theme-fixture'));
  for (const given of [dir, relative(process.cwd(), dir) || '.', `${dir}/`]) {
    delete process.env.EXACT_APP_DIR;
    const app = resolveApp(given);
    expect([given, app.name, app.dir]).toEqual([given, 'theme-fixture', dir]);
    expect(app.crate('web')).toBe('theme-fixture-web');
    // What the dev loop and the JS build make from the name stays in the target.
    for (const made of [resolve(app.target, 'dev-gpu', app.name), resolve(app.target, 'web-js-gpu', app.name)]) {
      expect([made.startsWith(`${app.target}/`), made.startsWith(`${dir}/`)]).toEqual([true, false]);
    }
    // The processes it starts get the same app.
    expect(process.env.EXACT_APP_DIR).toBe(resolve(String(given)));
  }
});

test('a name still resolves under apps/, as before', () => {
  delete process.env.EXACT_APP_DIR;
  const app = resolveApp('theme-fixture');
  expect([app.name, app.dir]).toEqual(['theme-fixture', realpathSync(resolve(ROOT, 'apps/theme-fixture'))]);
  expect(process.env.EXACT_APP_DIR).toBe(undefined);
  expect(resolveApp('theme-fixture-web').name).toBe('theme-fixture');
});

test('the JS build given a path writes its module crate under the target, none in the app', () => {
  const dir = realpathSync(resolve(ROOT, 'apps/theme-fixture'));
  const out = mkdtempSync(join(tmpdir(), 'exact-app-path-'));
  try {
    const env = { ...process.env };
    delete env.EXACT_APP_DIR;
    const built = spawnSync(process.execPath, ['host/web-js/build.mjs', dir, '--out', out, '--render', 'none'], { cwd: ROOT, env, encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    expect(existsSync(resolve(dir, 'crate'))).toBe(false);
    expect(existsSync(resolve(resolveApp('theme-fixture').target, 'web-js-modules/theme-fixture/crate'))).toBe(true);
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 600_000);

test('an app\'s authored tests are app.test.contract, then each other *.test.contract beside it by name', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-authored-tests-'));
  try {
    expect(authoredTests(dir)).toEqual([]);
    mkdirSync(join(dir, 'nested'));
    for (const f of ['b.test.contract', 'app.contract', 'nested/c.test.contract', 'a.test.contract']) writeFileSync(join(dir, f), '');
    expect(authoredTests(dir)).toEqual([join(dir, 'a.test.contract'), join(dir, 'b.test.contract')]);
    writeFileSync(join(dir, 'app.test.contract'), '');
    expect(authoredTests(dir)).toEqual(['app', 'a', 'b'].map((n) => join(dir, `${n}.test.contract`)));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
