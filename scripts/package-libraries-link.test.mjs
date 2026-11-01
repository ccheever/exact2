// LLP 1154 D3: a package's prebuilt library, linked into an app `exact new` made outside the repo and carried with
// it, found where the product is at run time: how each product links and carries it. The package is
// @fixture/vault built with its library (packages/fixtures/vault/test/library.mjs); what the app then does with it, on
// each host, is that fixture's own suites' (packages/fixtures/vault/test/linux.mjs, apple.mjs). The tests share one
// app, and each builds it for its host, which takes minutes, so none runs in a lane. Run the file by name:
//   bun test scripts/package-libraries-link.test.mjs
//
// - The Linux host (on a Mac, the Linux host built for the Mac's target, as `bun exact.mjs linux` builds it): the
//   library sits beside the executable, which names it by its run path alone.
// - The web: the package's declared asset, captured with the package app.ts imports, is in the build with the
//   package's bytes; delivery's wasm-target bake of the same app links no library and succeeds.
// - The macOS app (a Mac only): the framework is in the bundle's Frameworks, signed, loaded by the executable through
//   `@rpath` and a run path inside the bundle; launched with the cache's copy removed, dyld loads it from there. Its
//   `--embed` output carries the framework and names it in capabilities.json.
// - The iOS app on a simulator of the test's own (a Mac only): the same, from the installed app's container.
import { afterAll, expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { forget, makeApp } from '../packages/fixtures/vault/test/fixture.mjs';
import { buildPackage } from '../packages/fixtures/vault/test/library.mjs';

const root = resolve(import.meta.dir, '..');
const scratch = realpathSync(mkdtempSync(join(tmpdir(), 'exact-package-library-')));
afterAll(() => rmSync(scratch, { recursive: true, force: true }), 300_000);
const mac = process.platform === 'darwin';
const target = /^host: (.+)$/m.exec(spawnSync('rustc', ['-vV'], { encoding: 'utf8' }).stdout)[1];

const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} (exit ${r.status}):\n${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

let made;
const fixture = () => {
  if (made) return made;
  const { pkg, library } = buildPackage(join(scratch, 'build'));
  return made = { pkg, library, app: makeApp(join(scratch, 'vaultfixture'), pkg) };
};
const rpaths = (binary) => [...run('otool', ['-l', binary], scratch).matchAll(/cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (\S+)/g)].map(m => m[1]);

test('the Linux host links the library and carries it beside its executable, which finds it by its run path', () => {
  const { app } = fixture();
  const built = run('bun', ['exact.mjs', 'linux'], app).trim().split('\n');
  const binary = built.at(-2);
  if (mac) {
    expect(existsSync(join(dirname(binary), 'vault.framework/Versions/A/vault'))).toBe(true);
    expect(run('otool', ['-L', binary], scratch)).toContain('@rpath/vault.framework/Versions/A/vault');
    expect(rpaths(binary)).toEqual(['@executable_path']);
  } else {
    expect(existsSync(join(dirname(binary), 'libvault.so'))).toBe(true);
    const dynamic = run('readelf', ['-d', binary], scratch);
    expect(dynamic).toContain('Shared library: [libvault.so]');
    expect(dynamic).toMatch(/R(UN)?PATH.*\[\$ORIGIN\]/);
  }
}, 3_600_000);

test('the web build carries the package\'s declared asset, and delivery\'s web bake links no library', () => {
  const { app, pkg } = fixture();
  run('bun', ['exact.mjs', 'web-build', '--out', 'dist'], app);
  expect(readFileSync(join(app, 'dist/assets/@fixture/vault/web/device.wasm'))).toEqual(readFileSync(join(pkg, 'web/device.wasm')));
  // Delivery's bake of the web crate (scripts/deploy.mjs `bake`) links no native library, whatever the package declares.
  const baked = join(scratch, 'web-bake');
  run('bun', [resolve(root, 'host/web/build.mjs'), 'vaultfixture-web', '--bake'], root, { ...process.env, EXACT_APP_DIR: app, EXACT_WEB_DIST: baked });
  expect(existsSync(join(baked, 'exact.json'))).toBe(true);
}, 3_600_000);

test.skipIf(!mac)('the macOS app embeds the framework, signed, and dyld loads it from the bundle', async () => {
  const { app, library } = fixture();
  const said = run('bun', ['exact.mjs', 'mac'], app);
  const bundle = /^local client: (.+\.app)$/m.exec(said)?.[1];
  expect(bundle).toBeTruthy();
  const framework = join(bundle, 'Contents/Frameworks/vault.framework'), executable = join(bundle, 'Contents/MacOS', run('plutil', ['-extract', 'CFBundleExecutable', 'raw', join(bundle, 'Contents/Info.plist')], scratch).trim());
  run('codesign', ['--verify', '--strict', framework], scratch);
  run('codesign', ['--verify', '--deep', '--strict', bundle], scratch);
  expect(run('otool', ['-L', executable], scratch)).toContain('@rpath/vault.framework/Versions/A/vault');
  // A development link also names SwiftPM's and the toolchain's directories; none of them holds the framework.
  expect(rpaths(executable).filter(path => !path.startsWith('@') && existsSync(join(path, 'vault.framework')))).toEqual([]);
  expect(rpaths(executable)).toContain('@executable_path/../Frameworks');
  forget(library);
  // Launched as Launch Services would, with dyld saying what it loads; stopped once the framework is among them.
  const child = spawn(executable, [], { env: { ...process.env, DYLD_PRINT_LIBRARIES: '1' }, stdio: ['ignore', 'ignore', 'pipe'] });
  let printed = '';
  const loaded = await new Promise((done) => {
    const bound = setTimeout(() => done(null), 60_000);
    child.stderr.on('data', (chunk) => {
      printed += chunk;
      const line = printed.split('\n').find(l => l.includes('vault.framework'));
      if (line) { clearTimeout(bound); done(line); }
    });
    child.on('exit', () => { clearTimeout(bound); done(printed.split('\n').find(l => l.includes('vault.framework')) ?? null); });
  });
  child.kill();
  expect(loaded?.split(' ').at(-1)).toBe(join(realpathSync(framework), 'Versions/A/vault'));
  // What an embedding app links (`--embed`) carries the framework and says how to link it.
  const embedded = run('bun', [resolve(root, 'host/apple/build.mjs'), 'vaultfixture-apple', '--embed'], root, { ...process.env, EXACT_APP_DIR: app });
  const embed = /^host\/apple: (\S+) — /m.exec(embedded)?.[1];
  const at = embed.startsWith('/') ? embed : resolve(root, embed);
  expect(existsSync(join(at, 'Frameworks/vault.framework/Versions/A/vault'))).toBe(true);
  expect(JSON.parse(readFileSync(join(at, 'capabilities.json'), 'utf8')).frameworks).toEqual([{ path: 'Frameworks/vault.framework', package: '@fixture/vault', sha256: library[target].sha256, linkerFlags: ['-F', 'Frameworks', '-framework', 'vault', '-Xlinker', '-rpath', '-Xlinker', '@executable_path/../Frameworks'], embed: true }]);
}, 3_600_000);

test.skipIf(!mac)('the iOS app on a simulator embeds the framework, signed, and dyld loads it from the bundle', async () => {
  const { app, library } = fixture();
  // A simulator of the test's own (docs/testing.md: another person may share the Mac).
  const udid = run('xcrun', ['simctl', 'create', 'exact-package-library', 'com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro'], scratch).trim();
  try {
    const said = run('bun', ['exact.mjs', 'ios'], app, { ...process.env, EXACT_SIM: udid, EXACT_SHOW_SIMULATOR: '0' });
    const bundle = /^host\/apple: (\S+\.app) on /m.exec(said)?.[1];
    expect(bundle).toBeTruthy();
    const info = (key) => run('plutil', ['-extract', key, 'raw', join(bundle, 'Info.plist')], scratch).trim();
    const framework = join(bundle, 'Frameworks/vault.framework'), executable = join(bundle, info('CFBundleExecutable')), id = info('CFBundleIdentifier');
    run('codesign', ['--verify', '--strict', framework], scratch);
    expect(run('otool', ['-L', executable], scratch)).toContain('@rpath/vault.framework/vault');
    expect(rpaths(executable).filter(path => !path.startsWith('@') && existsSync(join(path, 'vault.framework')))).toEqual([]);
    expect(rpaths(executable)).toContain('@executable_path/Frameworks');
    forget(library);
    const container = run('xcrun', ['simctl', 'get_app_container', udid, id], scratch).trim();
    const child = spawn('xcrun', ['simctl', 'launch', '--console-pty', '--terminate-running-process', udid, id], { env: { ...process.env, SIMCTL_CHILD_DYLD_PRINT_LIBRARIES: '1' }, stdio: ['ignore', 'pipe', 'pipe'] });
    let printed = '';
    const loaded = await new Promise((done) => {
      const bound = setTimeout(() => done(null), 120_000);
      const heard = (chunk) => {
        printed += chunk;
        const line = printed.split('\n').find(l => l.includes('vault.framework'));
        if (line) { clearTimeout(bound); done(line.trim()); }
      };
      child.stdout.on('data', heard);
      child.stderr.on('data', heard);
      child.on('exit', () => { clearTimeout(bound); done(printed.split('\n').find(l => l.includes('vault.framework'))?.trim() ?? null); });
    });
    child.kill();
    spawnSync('xcrun', ['simctl', 'terminate', udid, id]);
    expect(loaded?.split(' ').at(-1)).toBe(join(realpathSync(container), 'Frameworks/vault.framework/vault'));
  } finally {
    spawnSync('xcrun', ['simctl', 'shutdown', udid]);
    spawnSync('xcrun', ['simctl', 'delete', udid]);
  }
}, 3_600_000);
