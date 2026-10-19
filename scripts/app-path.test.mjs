// `--app <path>` (the lead's 08:39Z; x2-perf4's stage 10 C2 found it): a path
// names an app as EXACT_APP_DIR does, so its name is its directory's and
// nothing built from the name (a crate, the dev loop's GPU side root, the JS
// build's GPU cache) lands in the app's own directory.
import { afterEach, expect, test } from 'bun:test';
import { realpathSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { resolveApp } from './app.mjs';

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
