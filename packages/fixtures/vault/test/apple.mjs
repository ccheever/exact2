// @fixture/vault in the macOS app and the iOS app on a simulator (LLP 1154 §6 M5): the package built with its
// library (library.mjs) and the fixture app depending on it (fixture.mjs). A Mac only; never a device.
//
// - A copy of the package that declares a wrong hash is refused by the macOS and the iOS build, both hashes named.
// - Each app bundle carries the framework in its Frameworks.
// - With the fetched library deleted, each drive (fixture.mjs STEPS) gets the library's answer, an ungranted path
//   refused with the grant named, the file opened by the library's SQLite with its row count, and the app's own open
//   of it refused while the library holds it and admitted after. The drive's own build check fetches the library
//   again (host/apple/build.mjs fetches before every build), so what the app loads is shown by its executable: it
//   names the framework by `@rpath`, and no run path outside the bundle holds one.
//
// The simulator is the suite's own (docs/testing.md: another person may share the Mac), deleted after.
//
// usage: bun packages/fixtures/vault/test/apple.mjs [<dir>]   (a directory a run made is reused, its apps built again)
import { spawnSync } from 'node:child_process';
import { existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { checker, drive, forget, makeFixture, refused, run, workDir, wrongHash } from './fixture.mjs';

/** Whether the executable at `binary` loads `framework` (its install name) only from `within`, a run path relative to
 * the executable: it names it by `@rpath`, its run paths include `within`, and none that is a directory on this
 * machine holds a vault.framework. */
function loadsFromBundle(binary, framework, within) {
  const rpaths = [...run('otool', ['-l', binary], '/').matchAll(/cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (\S+)/g)].map(m => m[1]);
  return run('otool', ['-L', binary], '/').includes(framework) && rpaths.includes(within)
    && !rpaths.some(path => !path.startsWith('@') && existsSync(join(path, 'vault.framework')));
}
const plistValue = (plist, key) => run('plutil', ['-extract', key, 'raw', plist], '/').trim();

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
const macExecutable = bundle && join(bundle, 'Contents/MacOS', plistValue(join(bundle, 'Contents/Info.plist'), 'CFBundleExecutable'));
check('macos', 'the executable loads the framework from the bundle alone', !!bundle && loadsFromBundle(macExecutable, '@rpath/vault.framework/Versions/A/vault', '@executable_path/../Frameworks'), macExecutable);

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
  const iosExecutable = ios && join(ios, plistValue(join(ios, 'Info.plist'), 'CFBundleExecutable'));
  check('ios', 'the executable loads the framework from the bundle alone', !!ios && loadsFromBundle(iosExecutable, '@rpath/vault.framework/vault', '@executable_path/Frameworks'), iosExecutable);
} finally {
  spawnSync('xcrun', ['simctl', 'shutdown', udid]);
  spawnSync('xcrun', ['simctl', 'delete', udid]);
}

if (!given) rmSync(dir, { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
