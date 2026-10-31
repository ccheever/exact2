// @fixture/vault in the macOS app and the iOS app on a simulator (LLP 1154 §6 M5): the package built with its
// library (library.mjs) and the fixture app depending on it (fixture.mjs). A Mac only; never a device.
//
// - A copy of the package that declares a wrong hash is refused by the macOS and the iOS build, both hashes named.
// - Each app bundle carries the framework in its Frameworks.
// - With the fetched library deleted, each drive (fixture.mjs STEPS) gets the library's answer, an ungranted path
//   refused with the grant named, the file opened by the library's SQLite with its row count, and the app's own open
//   of it refused while the library holds it and admitted after. The drive's own build check fetches the library
//   again (host/apple/build.mjs fetches before every build), so what the app loads is shown otherwise: its
//   executable names the framework by `@rpath`, the one run path that holds one is the bundle's, and launched, dyld
//   loads the bundle's copy.
//
// The simulator is the suite's own (docs/testing.md: another person may share the Mac), deleted after.
//
// usage: bun packages/fixtures/vault/test/apple.mjs [<dir>]   (a directory a run made is reused, its apps built again)
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, realpathSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { checker, drive, forget, makeFixture, refused, run, workDir, wrongHash } from './fixture.mjs';

/** Whether the executable at `binary` can load `framework` (its install name) only from the bundle's `frameworks`
 * directory: it names it by `@rpath`, and every run path that holds a vault.framework, `@executable_path/…` and
 * `@loader_path/…` resolved from the executable's directory, is that one. */
function loadsFromBundle(binary, framework, frameworks) {
  const from = dirname(binary);
  const rpaths = [...run('otool', ['-l', binary], '/').matchAll(/cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (\S+)/g)]
    .map(m => resolve(m[1].replace(/^@(executable|loader)_path(?=\/|$)/, from)));
  const holding = rpaths.filter(path => existsSync(join(path, 'vault.framework')));
  return run('otool', ['-L', binary], '/').includes(framework) && holding.length > 0 && holding.every(path => path === resolve(frameworks));
}

/** The path dyld printed for vault.framework's binary when `command` launched the app (`DYLD_PRINT_LIBRARIES`), or
 * null; the launch is stopped once it is printed. */
async function dyldLoaded(command, args, env) {
  const child = spawn(command, args, { env, stdio: ['ignore', 'pipe', 'pipe'] });
  const line = await new Promise((done) => {
    const bound = setTimeout(() => done(null), 120_000);
    const found = (lines) => lines.find(l => l.includes('vault.framework'));
    // Each stream's text after its last newline, which a later chunk completes.
    const pending = { stdout: '', stderr: '' };
    const heard = (stream) => (chunk) => {
      const lines = (pending[stream] + chunk).split('\n');
      pending[stream] = lines.pop();
      const line = found(lines);
      if (line) { clearTimeout(bound); done(line.trim()); }
    };
    child.stdout.on('data', heard('stdout'));
    child.stderr.on('data', heard('stderr'));
    // At exit each stream's last text is a whole line.
    child.on('exit', () => { clearTimeout(bound); done(found(Object.values(pending))?.trim() ?? null); });
  });
  child.kill();
  return line?.split(' ').at(-1) ?? null;
}
const plistValue = (plist, key) => run('plutil', ['-extract', key, 'raw', plist], '/').trim();

if (process.platform !== 'darwin') { console.error('apple.mjs runs on a Mac'); process.exit(1); }
const given = process.argv[2], dir = workDir(given);
const { library, wrong: wrongLibrary, app, wrongApp } = makeFixture(dir);
const { check, failed } = checker();

// The macOS app.
let wrong = refused('bun', ['exact.mjs', 'mac'], wrongApp) ?? '';
check('macos', 'a declared hash the library does not have is refused, both named', wrong.includes(wrongHash(library, wrongLibrary, 'aarch64-apple-darwin')), wrong.slice(0, 2000));
const bundle = /^local client: (.+\.app)$/m.exec(run('bun', ['exact.mjs', 'mac'], app))?.[1];
check('macos', 'the bundle carries the framework in Contents/Frameworks', !!bundle && existsSync(join(bundle, 'Contents/Frameworks/vault.framework/Versions/A/vault')), bundle);
forget(library);
drive(check, 'macos', app);
const macExecutable = bundle && join(bundle, 'Contents/MacOS', plistValue(join(bundle, 'Contents/Info.plist'), 'CFBundleExecutable'));
check('macos', 'the executable can load the framework from the bundle alone', !!bundle && loadsFromBundle(macExecutable, '@rpath/vault.framework/Versions/A/vault', join(bundle, 'Contents/Frameworks')), macExecutable);
const macLoaded = bundle && await dyldLoaded(macExecutable, [], { ...process.env, DYLD_PRINT_LIBRARIES: '1' });
check('macos', 'launched, dyld loads the bundle\'s framework', !!bundle && macLoaded === join(realpathSync(bundle), 'Contents/Frameworks/vault.framework/Versions/A/vault'), macLoaded);

// The iOS app, on a simulator of the suite's own.
const udid = run('xcrun', ['simctl', 'create', 'exact-vault', 'com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro'], dir).trim();
const env = { ...process.env, EXACT_SIM: udid, EXACT_SHOW_SIMULATOR: '0' };
try {
  wrong = refused('bun', ['exact.mjs', 'ios'], wrongApp, env) ?? '';
  check('ios', 'a declared hash the library does not have is refused, both named', wrong.includes(wrongHash(library, wrongLibrary, 'aarch64-apple-ios-sim')), wrong.slice(0, 2000));
  const ios = /^host\/apple: (\S+\.app) on /m.exec(run('bun', ['exact.mjs', 'ios'], app, env))?.[1];
  check('ios', 'the bundle carries the framework in Frameworks', !!ios && existsSync(join(ios, 'Frameworks/vault.framework/vault')), ios);
  forget(library);
  drive(check, 'ios', app, ['--sim', udid], env);
  const iosExecutable = ios && join(ios, plistValue(join(ios, 'Info.plist'), 'CFBundleExecutable'));
  check('ios', 'the executable can load the framework from the bundle alone', !!ios && loadsFromBundle(iosExecutable, '@rpath/vault.framework/vault', join(ios, 'Frameworks')), iosExecutable);
  // The installed app, launched from its container.
  const id = ios && plistValue(join(ios, 'Info.plist'), 'CFBundleIdentifier');
  const container = id && realpathSync(run('xcrun', ['simctl', 'get_app_container', udid, id], dir).trim());
  const iosLoaded = id && await dyldLoaded('xcrun', ['simctl', 'launch', '--console-pty', '--terminate-running-process', udid, id], { ...process.env, SIMCTL_CHILD_DYLD_PRINT_LIBRARIES: '1' });
  spawnSync('xcrun', ['simctl', 'terminate', udid, id]);
  check('ios', 'launched, dyld loads the installed bundle\'s framework', !!id && iosLoaded === join(container, 'Frameworks/vault.framework/vault'), `${iosLoaded} (container ${container})`);
} finally {
  spawnSync('xcrun', ['simctl', 'shutdown', udid]);
  spawnSync('xcrun', ['simctl', 'delete', udid]);
}

if (!given) rmSync(dir, { recursive: true, force: true });
process.exit(failed() ? 1 : 0);
