// @fixture/vault in the macOS app and the iOS app on a simulator (LLP 1154 §6 M5): the package built with its
// library (library.mjs) and the fixture app depending on it (fixture.mjs). A Mac only; never a device.
//
// - A copy of the package that declares a wrong hash is refused by the macOS and the iOS build, both hashes named.
// - Each app bundle carries the framework in its Frameworks.
// - With the fetched library deleted, each drive (fixture.mjs STEPS) gets the library's answer, an ungranted path
//   refused with the grant named, the file opened by the library's SQLite with its row count, and the app's own open
//   of it refused while the library holds it and admitted after; and nothing fetched the library again.
//
// The simulator is the suite's own (docs/testing.md: another person may share the Mac), deleted after.
//
// usage: bun packages/fixtures/vault/test/apple.mjs [<dir>]   (a directory a run made is reused, its apps built again)
import { spawnSync } from 'node:child_process';
import { existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { checker, drive, fetched, forget, makeFixture, refused, run, workDir, wrongHash } from './fixture.mjs';

if (process.platform !== 'darwin') { console.error('apple.mjs runs on a Mac'); process.exit(1); }
const given = process.argv[2], dir = workDir(given);
const { library, app, wrongApp } = makeFixture(dir);
const { check, failed } = checker();

// The macOS app.
let wrong = refused('bun', ['exact.mjs', 'mac'], wrongApp) ?? '';
check('macos', 'a declared hash the library does not have is refused, both named', wrong.includes(`@fixture/vault: ${wrongHash(library, 'aarch64-apple-darwin')}`), wrong.slice(0, 2000));
const bundle = /^local client: (.+\.app)$/m.exec(run('bun', ['exact.mjs', 'mac'], app))?.[1];
check('macos', 'the bundle carries the framework in Contents/Frameworks', !!bundle && existsSync(join(bundle, 'Contents/Frameworks/vault.framework/Versions/A/vault')), bundle);
forget(library);
drive(check, 'macos', app);
check('macos', 'the drive loaded the framework the bundle carries: nothing fetched it again', !fetched(library), 'the cache holds it again');

// The iOS app, on a simulator of the suite's own.
const udid = run('xcrun', ['simctl', 'create', 'exact-vault', 'com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro'], dir).trim();
const env = { ...process.env, EXACT_SIM: udid, EXACT_SHOW_SIMULATOR: '0' };
try {
  wrong = refused('bun', ['exact.mjs', 'ios'], wrongApp, env) ?? '';
  check('ios', 'a declared hash the library does not have is refused, both named', wrong.includes(`@fixture/vault: ${wrongHash(library, 'aarch64-apple-ios-sim')}`), wrong.slice(0, 2000));
  const ios = /^host\/apple: (\S+\.app) on /m.exec(run('bun', ['exact.mjs', 'ios'], app, env))?.[1];
  check('ios', 'the bundle carries the framework in Frameworks', !!ios && existsSync(join(ios, 'Frameworks/vault.framework/vault')), ios);
  forget(library);
  drive(check, 'ios', app, ['--sim', udid], env);
  check('ios', 'the drive loaded the framework the bundle carries: nothing fetched it again', !fetched(library), 'the cache holds it again');
} finally {
  spawnSync('xcrun', ['simctl', 'shutdown', udid]);
  spawnSync('xcrun', ['simctl', 'delete', udid]);
}

if (!given) rmSync(dir, { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
