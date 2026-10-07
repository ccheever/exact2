// `exact release` signs every nested Mach-O file and bundle, innermost first (#119).
// `bun test ./scripts/exact.test.mjs`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, relative, resolve } from 'node:path';
import { signingOrder } from '../host/apple/assets.mjs';
import { stripForDistribution } from '../host/apple/build.mjs';
import { assertLinkedSdk } from '../host/apple/link.mjs';

const plist = (executable) => `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>${executable}</string></dict></plist>\n`;

function inDir(body) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-release-'));
  try { return body(dir); } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('the signing order holds every Mach-O by its magic and each nested bundle after its contents', () => inDir((dir) => {
  const app = resolve(dir, 'Fixture.app');
  const put = (path, bytes) => { mkdirSync(dirname(resolve(app, path)), { recursive: true }); writeFileSync(resolve(app, path), bytes); };
  const word = (...words) => Buffer.concat(words.map((w) => { const b = Buffer.alloc(4); b.writeUInt32BE(w >>> 0); return b; }));
  put('Contents/Info.plist', plist('ExactMac'));
  put('Contents/MacOS/ExactMac', word(0xcffaedfe, 0x0c000001)); // the main executable: the bundle's own signature
  put('Contents/MacOS/libexact_web.dylib', word(0xcffaedfe, 0x0c000001));
  put('Contents/MacOS/spawn-helper', word(0xcafebabe, 2)); // universal, two architectures
  put('Contents/Resources/assets/helper', word(0xcffaedfe, 0x0c000001)); // no extension to go by
  put('Contents/Resources/assets/addon.node', word(0xcefaedfe, 7)); // 32-bit
  put('Contents/Resources/assets/Big.class', word(0xcafebabe, 0x00000034)); // Java: the fat magic, a class version
  put('Contents/Resources/assets/notes.dylib', 'not code\n'); // a name is not a format
  put('Contents/Resources/assets/short', Buffer.from([0xcf, 0xfa]));
  put('Contents/Helpers/Inner.app/Contents/Info.plist', plist('Inner'));
  put('Contents/Helpers/Inner.app/Contents/MacOS/Inner', word(0xfeedfacf, 0x0100000c));
  put('Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/A/Kit', word(0xcffaedfe, 1)); // the framework's own
  put('Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/A/Resources/Info.plist', plist('Kit'));
  put('Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/A/XPCServices/Svc.xpc/Contents/Info.plist', plist('Svc'));
  put('Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/A/XPCServices/Svc.xpc/Contents/MacOS/Svc', word(0xcffaedfe, 2));
  symlinkSync('A', resolve(app, 'Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/Current'));
  symlinkSync('Versions/Current/Kit', resolve(app, 'Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Kit'));

  const order = signingOrder(app).map((path) => relative(dir, path));
  assert.deepEqual([...order].sort(), [
    'Fixture.app',
    'Fixture.app/Contents/Helpers/Inner.app',
    'Fixture.app/Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework',
    'Fixture.app/Contents/Helpers/Inner.app/Contents/Frameworks/Kit.framework/Versions/A/XPCServices/Svc.xpc',
    'Fixture.app/Contents/MacOS/libexact_web.dylib',
    'Fixture.app/Contents/MacOS/spawn-helper',
    'Fixture.app/Contents/Resources/assets/addon.node',
    'Fixture.app/Contents/Resources/assets/helper',
  ]);
  // Innermost first: everything a bundle holds is signed before the bundle.
  for (const [i, path] of order.entries()) {
    for (const later of order.slice(i + 1)) assert.ok(!later.startsWith(`${path}/`), `${later} is signed after its container ${path}`);
  }
  assert.equal(order.at(-1), 'Fixture.app');
}));

const tools = process.platform === 'darwin' && ['clang', 'codesign'].every((tool) => Bun.which(tool));
test.skipIf(!tools)('signing in that order seals a bundle with an unsigned helper and an unsigned nested app', () => inDir((dir) => {
  const app = resolve(dir, 'Fixture.app');
  const source = resolve(dir, 'main.c');
  writeFileSync(source, 'int main(void){return 0;}\n');
  const cc = (out, ...flags) => {
    mkdirSync(dirname(resolve(app, out)), { recursive: true });
    const r = spawnSync('clang', [...flags, '-o', resolve(app, out), source], { encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
  };
  cc('Contents/MacOS/ExactMac');
  writeFileSync(resolve(app, 'Contents/Info.plist'), plist('ExactMac'));
  cc('Contents/Resources/assets/helper', '-Wl,-no_adhoc_codesign');
  cc('Contents/Helpers/Inner.app/Contents/MacOS/Inner', '-Wl,-no_adhoc_codesign');
  writeFileSync(resolve(app, 'Contents/Helpers/Inner.app/Contents/Info.plist'), plist('Inner'));
  // A binary plist, as Xcode writes one: its executable is still the bundle's own.
  assert.equal(spawnSync('plutil', ['-convert', 'binary1', resolve(app, 'Contents/Helpers/Inner.app/Contents/Info.plist')]).status, 0);
  // A versioned framework holding an unsigned XPC service: signing the
  // framework's executable on its own would seal it before the service.
  const kit = 'Contents/Frameworks/Kit.framework';
  cc(`${kit}/Versions/A/Kit`, '-dynamiclib', '-Wl,-no_adhoc_codesign');
  mkdirSync(resolve(app, kit, 'Versions/A/Resources'), { recursive: true });
  writeFileSync(resolve(app, kit, 'Versions/A/Resources/Info.plist'), plist('Kit').replace('</dict>', '<key>CFBundleIdentifier</key><string>dev.exact.kit</string><key>CFBundlePackageType</key><string>FMWK</string></dict>'));
  cc(`${kit}/Versions/A/XPCServices/Svc.xpc/Contents/MacOS/Svc`, '-Wl,-no_adhoc_codesign');
  writeFileSync(resolve(app, kit, 'Versions/A/XPCServices/Svc.xpc/Contents/Info.plist'), plist('Svc'));
  symlinkSync('A', resolve(app, kit, 'Versions/Current'));
  for (const link of ['Kit', 'Resources', 'XPCServices']) symlinkSync(`Versions/Current/${link}`, resolve(app, kit, link));

  for (const path of signingOrder(app)) {
    const r = spawnSync('codesign', ['--force', '--sign', '-', '--options', 'runtime', path], { encoding: 'utf8' });
    assert.equal(r.status, 0, `${path}: ${r.stderr}`);
  }
  const verify = spawnSync('codesign', ['--verify', '--deep', '--strict', app], { encoding: 'utf8' });
  assert.equal(verify.status, 0, verify.stderr);
  const helper = spawnSync('codesign', ['-dv', resolve(app, 'Contents/Resources/assets/helper')], { encoding: 'utf8' });
  assert.match(helper.stderr, /flags=0x10002\(adhoc,runtime\)/);
}), 60000);

test('native resource trees keep scoped names, executable modes, links and files above the bake cap', () => inDir(dir => {
  const { chmodSync, ftruncateSync, closeSync, openSync, readFileSync, readlinkSync, statSync } = require('node:fs');
  const { copyMacResources, macResourceInventory } = require('../host/apple/assets.mjs');
  const { readManifest, pendingBuildInputs } = require('./app.mjs');
  const manifest = { name: 'Resources', app: { id: 'test.resources', name: 'Resources' }, host: { macos: { resources: [{ from: 'server', to: 'Resources/server' }] } } };
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify(manifest));
  mkdirSync(resolve(dir, 'server/node_modules/@scope/a package'), { recursive: true });
  writeFileSync(resolve(dir, 'server/node_modules/@scope/a package/index.js'), 'module.exports = 42;\n');
  writeFileSync(resolve(dir, 'server/helper'), '#!/bin/sh\necho helper ran\n');
  chmodSync(resolve(dir, 'server/helper'), 0o755);
  symlinkSync('helper', resolve(dir, 'server/helper-link'));
  const fd = openSync(resolve(dir, 'server/large.bin'), 'w'); ftruncateSync(fd, 65 * 1024 * 1024); closeSync(fd);
  const app = { dir, manifest: readManifest(dir, 'resources') };
  const contents = resolve(dir, 'Fixture.app/Contents'); mkdirSync(contents, { recursive: true });
  const before = macResourceInventory(app);
  copyMacResources(app, contents);
  assert.equal(statSync(resolve(contents, 'Resources/server/helper')).mode & 0o777, 0o755);
  assert.equal(readlinkSync(resolve(contents, 'Resources/server/helper-link')), 'helper');
  assert.equal(spawnSync(resolve(contents, 'Resources/server/helper-link'), [], { encoding: 'utf8' }).stdout, 'helper ran\n');
  assert.equal(readFileSync(resolve(contents, 'Resources/server/node_modules/@scope/a package/index.js'), 'utf8'), 'module.exports = 42;\n');
  const copied = macResourceInventory({ dir: resolve(contents, 'Resources'), manifest: { host: { macos: { resources: [{ from: 'server', to: 'Resources/server' }] } } } });
  assert.deepEqual(copied, before);
  const build = { binary: { nativeResourceApp: app, metadata: { nativeResources: before }, inputs: [], directories: [], missing: [] } };
  assert.deepEqual(pendingBuildInputs(build), []);
  chmodSync(resolve(dir, 'server/helper'), 0o744);
  assert.throws(() => copyMacResources(app, resolve(dir, 'changed'), before), /changed after the binary receipt/);
  assert.deepEqual(pendingBuildInputs(build), ['host.macos.resources']);
}));

test('native resource mappings refuse traversal, overlapping assets and escaping links', () => inDir(dir => {
  const { macResourceMappings, macResourceInventory, copyMacResources } = require('../host/apple/assets.mjs');
  const manifest = resources => ({ host: { macos: { resources } } });
  for (const from of ['../server', '/server', 'assets', 'target/server', 'server/../other'])
    assert.throws(() => macResourceMappings(manifest([{ from, to: 'Resources/server' }])));
  for (const to of ['../escape', 'MacOS/ExactMac', 'Resources/assets/anything', 'Resources/receipt.json'])
    assert.throws(() => macResourceMappings(manifest([{ from: 'server', to }])));
  assert.throws(() => macResourceMappings(manifest([{ from: 'server', to: 'Resources/server' }, { from: 'server/child', to: 'Helpers/child' }])));
  mkdirSync(resolve(dir, 'server'));
  writeFileSync(resolve(dir, 'outside'), 'outside');
  symlinkSync('../outside', resolve(dir, 'server/link'));
  const app = { dir, manifest: manifest([{ from: 'server', to: 'Resources/server' }]) };
  assert.throws(() => macResourceInventory(app), /escapes/);
  rmSync(resolve(dir, 'server/link'));
  mkdirSync(resolve(dir, 'bundle/Resources'), { recursive: true });
  symlinkSync(resolve(dir, 'server'), resolve(dir, 'bundle/Resources/server'));
  assert.throws(() => copyMacResources(app, resolve(dir, 'bundle')), /already exists/);
}));

test.skipIf(process.platform !== 'darwin')('native resources sign and execute a Mach-O helper with its dylib in the assembled bundle', () => inDir(dir => {
  const { copyMacResources } = require('../host/apple/assets.mjs');
  mkdirSync(resolve(dir, 'server'), { recursive: true });
  writeFileSync(resolve(dir, 'lib.c'), 'int answer(void) { return 42; }\n');
  writeFileSync(resolve(dir, 'main.c'), '#include <stdio.h>\nextern int answer(void); int main(void) { if(answer()!=42) return 1; puts("helper ran"); return 0; }\n');
  const command = (cmd, args) => { const r = spawnSync(cmd, args, { encoding: 'utf8' }); assert.equal(r.status, 0, `${cmd}: ${r.stderr}`); return r; };
  command('cc', ['-dynamiclib', resolve(dir, 'lib.c'), '-Wl,-install_name,@loader_path/addon.node', '-Wl,-no_adhoc_codesign', '-o', resolve(dir, 'server/addon.node')]);
  command('cc', [resolve(dir, 'main.c'), resolve(dir, 'server/addon.node'), '-Wl,-no_adhoc_codesign', '-o', resolve(dir, 'server/helper')]);
  const app = resolve(dir, 'Fixture.app'), contents = resolve(app, 'Contents');
  mkdirSync(resolve(contents, 'MacOS'), { recursive: true });
  writeFileSync(resolve(contents, 'Info.plist'), plist('ExactMac'));
  writeFileSync(resolve(dir, 'host.c'), 'int main(void) { return 0; }\n');
  command('cc', [resolve(dir, 'host.c'), '-o', resolve(contents, 'MacOS/ExactMac')]);
  copyMacResources({ dir, manifest: { host: { macos: { resources: [{ from: 'server', to: 'Resources/server' }] } } } }, contents);
  for (const path of signingOrder(app)) command('codesign', ['--force', '--sign', '-', '--timestamp=none', path]);
  command('codesign', ['--verify', '--deep', '--strict', app]);
  assert.equal(command(resolve(contents, 'Resources/server/helper'), []).stdout, 'helper ran\n');
}));

// An executable is named for its app (955b0463d), and an app may be named
// "T3 Code (Exact)": otool reads a path ending in `name(member)` as an
// archive's member, so the SDK check read nothing and refused the build (#234).
test.skipIf(!tools)('an executable whose name ends in parentheses is read as a file, its SDK checked and its release stripped', () => inDir((dir) => {
  writeFileSync(resolve(dir, 'main.c'), 'int main(void){return 0;}\n');
  const executable = resolve(dir, 'Demo (Beta)');
  // The macOS 15 SDK recorded, as a design-compatible build links it.
  const r = spawnSync('clang', ['-g', '-mmacosx-version-min=14.0', '-Wl,-platform_version,macos,14.0,15.0', '-o', executable, resolve(dir, 'main.c')], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  assertLinkedSdk(executable, '15.0');
  assert.throws(() => assertLinkedSdk(executable, '27.0'), /Demo \(Beta\) records SDK 15\.0, not 27\.0/);
  // A file the tool cannot read says so, not that it records no SDK.
  assert.throws(() => assertLinkedSdk(resolve(dir, 'Gone (Beta)'), '15.0'), /vtool -show-build failed .*Gone \(Beta\)/);
  // `exact release`'s dsymutil and strip take the same path as a file.
  const { saved } = stripForDistribution(executable, resolve(dir, 'Demo (Beta).dSYM'));
  assert.ok(existsSync(resolve(dir, 'Demo (Beta).dSYM/Contents/Resources/DWARF/Demo (Beta)')), 'dsymutil wrote the dSYM');
  assert.ok(saved > 0, 'strip took symbols off');
  assertLinkedSdk(executable, '15.0');
}));
