// LLP 1154 D3: a package's prebuilt library, linked into an app `exact new` made outside the repo and carried with
// it, found where the product is at run time. The library is a cdylib built here from a few lines of Rust whose one
// function answers a number nothing else knows; the package is a copy of @fixture/tally whose Linux implementation
// shows that number in its status, and whose exact.json declares the library for this machine's target by a file:
// URL and its hash. The tests share one app, and each builds it for its host, which takes minutes, so none runs in a
// lane. Run the file by name:
//   bun test scripts/package-libraries-link.test.mjs
//
// - The Linux host (on a Mac, the Linux host built for the Mac's target, as `bun exact.mjs linux` builds it): the
//   library sits beside the executable, which loads it by its run path alone; with the cache's copy removed, a drive
//   bound to the implementation shows the library's number.
// - The web: the package's declared asset, captured with the package app.ts imports, is in the build at
//   /assets/<package>/<path> with the package's bytes, and serve.mjs serves it there; delivery's wasm-target bake of
//   the same app links no library and succeeds.
// - The macOS app (a Mac only): the framework is in the bundle's Frameworks, signed, loaded by the executable through
//   `@rpath` and a run path inside the bundle; launched with the cache's copy removed, dyld loads it from there. Its
//   `--embed` output carries the framework and names it in capabilities.json.
// - The iOS app on a simulator of the test's own (a Mac only): the same, from the installed app's container.
import { afterAll, expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { LIBRARY_CACHE } from './package-libraries.mjs';

const root = resolve(import.meta.dir, '..'), tally = resolve(root, 'packages/fixtures/tally');
const scratch = realpathSync(mkdtempSync(join(tmpdir(), 'exact-package-library-')));
afterAll(() => rmSync(scratch, { recursive: true, force: true }), 300_000);
const ANSWER = 4217, mac = process.platform === 'darwin';
const target = /^host: (.+)$/m.exec(spawnSync('rustc', ['-vV'], { encoding: 'utf8' }).stdout)[1];

const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} (exit ${r.status}):\n${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** The library for this machine's target: on a Mac a zipped xcframework of a macOS framework and an iOS simulator's,
 * declared for both targets, else `libvault.so`. Returns the declaration's `native.library`. */
function buildLibrary() {
  const dir = join(scratch, 'library');
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'lib.rs'), `#[no_mangle]\npub extern "C" fn vaultlib_answer() -> i32 {\n    ${ANSWER}\n}\n`);
  const cdylib = (out, ...flags) => run('rustc', ['--edition', '2021', '--crate-type', 'cdylib', '-O', '--target', target, 'lib.rs', '-o', out, ...flags], dir);
  const plist = (keys = '') => `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>vault</string><key>CFBundleIdentifier</key><string>dev.exact.fixture.vault</string><key>CFBundlePackageType</key><string>FMWK</string><key>CFBundleVersion</key><string>1</string>${keys}</dict></plist>
`;
  let file;
  if (mac) {
    const fw = join(dir, 'mac/vault.framework');
    mkdirSync(join(fw, 'Versions/A/Resources'), { recursive: true });
    cdylib(join(fw, 'Versions/A/vault'), '-C', 'link-arg=-Wl,-install_name,@rpath/vault.framework/Versions/A/vault');
    symlinkSync('A', join(fw, 'Versions/Current'));
    symlinkSync('Versions/Current/vault', join(fw, 'vault'));
    symlinkSync('Versions/Current/Resources', join(fw, 'Resources'));
    writeFileSync(join(fw, 'Versions/A/Resources/Info.plist'), plist());
    // An iOS framework is flat (Apple's "Bundle Programming Guide", "Anatomy of Framework Bundles").
    const sim = join(dir, 'sim/vault.framework');
    mkdirSync(sim, { recursive: true });
    run('rustc', ['--edition', '2021', '--crate-type', 'cdylib', '-O', '--target', 'aarch64-apple-ios-sim', 'lib.rs', '-o', join(sim, 'vault'), '-C', 'link-arg=-Wl,-install_name,@rpath/vault.framework/vault'], dir);
    writeFileSync(join(sim, 'Info.plist'), plist('<key>MinimumOSVersion</key><string>17.0</string>'));
    for (const f of [fw, sim]) run('codesign', ['--force', '--sign', '-', f], dir);
    run('xcodebuild', ['-create-xcframework', '-framework', fw, '-framework', sim, '-output', join(dir, 'vault.xcframework')], dir);
    file = join(dir, 'vault.xcframework.zip');
    run('ditto', ['-c', '-k', '--keepParent', 'vault.xcframework', file], dir);
  } else cdylib(file = join(dir, 'libvault.so'), '-C', 'link-arg=-Wl,-soname,libvault.so');
  const entry = { url: pathToFileURL(file).href, sha256: createHash('sha256').update(readFileSync(file)).digest('hex') };
  return { [target]: entry, ...(mac ? { 'aarch64-apple-ios-sim': entry } : {}) };
}

const WASM = Buffer.concat([Buffer.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]), Buffer.from(`only the package has these bytes ${ANSWER}`)]);
/** @fixture/tally with the library declared and its Linux status naming the library's answer, and a module app.ts
 * imports beside a declared web asset. */
function makePackage(library) {
  const pkg = join(scratch, 'tally');
  cpSync(tally, pkg, { recursive: true, filter: (from) => !from.includes(`${tally}/test`) });
  const declaration = JSON.parse(readFileSync(join(pkg, 'exact.json'), 'utf8'));
  declaration.native = { link: 'vault', library };
  declaration.web = { assets: ['web/device.wasm'] };
  mkdirSync(join(pkg, 'web'));
  writeFileSync(join(pkg, 'web/device.wasm'), WASM);
  writeFileSync(join(pkg, 'driver.js'), "export const driver = 'tally driver';\n");
  writeFileSync(join(pkg, 'driver.d.ts'), 'export declare const driver: string;\n');
  const manifest = JSON.parse(readFileSync(join(pkg, 'package.json'), 'utf8'));
  manifest.exports['./driver'] = { types: './driver.d.ts', default: './driver.js' };
  writeFileSync(join(pkg, 'package.json'), JSON.stringify(manifest, null, 2));
  writeFileSync(join(pkg, 'exact.json'), JSON.stringify(declaration, null, 2));
  const source = readFileSync(join(pkg, 'linux/lib.rs'), 'utf8');
  writeFileSync(join(pkg, 'linux/lib.rs'), source.replace('"platform": std::env::consts::OS,', '"platform": format!("{} {}", std::env::consts::OS, unsafe { vaultlib_answer() }),')
    + '\nextern "C" {\n    fn vaultlib_answer() -> i32;\n}\n');
  return pkg;
}

/** The fixture app, as packages/fixtures/tally/test/fixture.mjs makes it, depending on `pkg`. */
function makeApp(pkg) {
  const app = join(scratch, 'vaultfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(tally, 'test/fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), `import { driver } from '@fixture/tally/driver';\nvoid driver;\n` + readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@fixture/tally': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return app;
}

let made;
const fixture = () => {
  if (made) return made;
  const library = buildLibrary();
  return made = { library, app: makeApp(makePackage(library)) };
};
const forget = (library) => rmSync(join(LIBRARY_CACHE, library[target].sha256), { recursive: true, force: true });
const rpaths = (binary) => [...run('otool', ['-l', binary], scratch).matchAll(/cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (\S+)/g)].map(m => m[1]);

test('the Linux host links the library, carries it beside its executable, and answers with it', () => {
  const { app, library } = fixture();
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
  // Only the copy beside the executable is left to load.
  forget(library);
  const lines = run('bun', ['exact.mjs', 'agent', 'linux', '--reset-storage', '--json', '--bind', 'tally', 'clock settle', 'tree'], app).trim().split('\n').map(l => JSON.parse(l));
  const status = lines[1].nodes.find(n => n.props?.testId === 'status')?.props.text;
  expect(status).toBe(`true ${{ darwin: 'macos', linux: 'linux' }[process.platform]} ${ANSWER} 0 true`);
}, 3_600_000);

test('the web build serves the package\'s declared asset at /assets/<package>/<path>', async () => {
  const { app } = fixture();
  run('bun', ['exact.mjs', 'web-build', '--out', 'dist'], app);
  // Delivery's bake of the web crate (scripts/deploy.mjs `bake`) links no native library, whatever the package declares.
  const baked = join(scratch, 'web-bake');
  run('bun', [resolve(root, 'host/web/build.mjs'), 'vaultfixture-web', '--bake'], root, { ...process.env, EXACT_APP_DIR: app, EXACT_WEB_DIST: baked });
  expect(existsSync(join(baked, 'exact.json'))).toBe(true);
  expect(readFileSync(join(app, 'dist/assets/@fixture/tally/web/device.wasm'))).toEqual(WASM);
  const port = 20000 + Math.floor(Math.random() * 20000);
  const server = spawn('bun', [resolve(root, 'host/web/serve.mjs'), join(app, 'dist'), '--port', String(port), '--loopback'], { stdio: 'ignore' });
  try {
    let response;
    for (let i = 0; i < 200 && !response; i++) {
      response = await fetch(`http://127.0.0.1:${port}/assets/@fixture/tally/web/device.wasm`).catch(() => null);
      if (!response) await Bun.sleep(50);
    }
    expect(response?.status).toBe(200);
    expect(response.headers.get('content-type')).toBe('application/wasm');
    expect(Buffer.from(await response.arrayBuffer())).toEqual(WASM);
  } finally { server.kill(); }
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
  expect(JSON.parse(readFileSync(join(at, 'capabilities.json'), 'utf8')).frameworks).toEqual([{ path: 'Frameworks/vault.framework', package: '@fixture/tally', sha256: library[target].sha256, linkerFlags: ['-F', 'Frameworks', '-framework', 'vault', '-Xlinker', '-rpath', '-Xlinker', '@executable_path/../Frameworks'], embed: true }]);
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
