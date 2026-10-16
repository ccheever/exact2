// `exact release` signs every nested Mach-O file and bundle, innermost first (#119); `exact contract build`
// refreshes an app's types (LLP 1116.001 D4.5).
// `bun test ./scripts/exact.test.mjs`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, linkSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, relative, resolve } from 'node:path';
import { signingOrder, signRelease, signedGpuProduct, assertArchiveFrameworks } from '../host/apple/assets.mjs';
import { appleEmbedOnly, copyAppleGpuProducts, captureAppleProduct, stripForDistribution } from '../host/apple/build.mjs';
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
  put('Contents/Resources/assets/plain.bundle/note.txt', 'not a code bundle\n');
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
test('release refuses code in content-addressed assets before signing changes its digest', () => inDir(dir => {
  const app = resolve(dir, 'Fixture.app'), helper = resolve(app, 'Contents/Resources/assets/helper');
  mkdirSync(dirname(helper), { recursive: true });
  const bytes = Buffer.from([0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1]);
  writeFileSync(helper, bytes);
  const compat = { embedded: { assets: [{ name: 'assets/helper', sha256: createHash('sha256').update(bytes).digest('hex') }] } };
  assert.throws(() => signingOrder(app, compat), /assets\/helper.*host.macos.resources/);
}));

test.skipIf(!tools)('release preserves baked GPU signatures and rejects changed products', () => inDir(dir => {
  const app = resolve(dir, 'Fixture.app'), executable = resolve(app, 'Contents/MacOS/Fixture');
  mkdirSync(dirname(executable), { recursive: true });
  writeFileSync(resolve(dir, 'main.c'), 'int main(void){return 0;}\n');
  assert.equal(spawnSync('clang', [resolve(dir, 'main.c'), '-o', executable]).status, 0);
  writeFileSync(resolve(app, 'Contents/Info.plist'), plist('Fixture'));
  const product = resolve(dir, 'gpu.dylib');
  assert.equal(spawnSync('clang', ['-dynamiclib', resolve(dir, 'main.c'), '-o', product]).status, 0);
  const gpu = resolve(app, 'Contents/MacOS/libexact_gpu.dylib');
  cpSync(signedGpuProduct(product, { hardened: true }), gpu);
  const digest = createHash('sha256').update(readFileSync(gpu)).digest('hex');
  const named = resolve(app, 'Contents/MacOS/libexact_gpu_title_sky.dylib');
  cpSync(gpu, named);
  const rust = resolve(app, 'Contents/Resources/rust/module.dylib');
  mkdirSync(dirname(rust), { recursive: true }); cpSync(gpu, rust);
  const compat = { embedded: { assets: [{ name: 'rust/module.dylib', sha256: digest }], gpu: { sha256: digest }, gpuModules: { 'title-sky': { sha256: digest } } } };
  assert.deepEqual(signingOrder(app, compat), [app]);
  for (const path of signingOrder(app, compat)) assert.equal(spawnSync('codesign', ['--force', '--sign', '-', '--options', 'runtime', '--timestamp=none', path]).status, 0);
  assert.equal(spawnSync('codesign', ['--verify', '--deep', '--strict', app]).status, 0);
  assert.equal(createHash('sha256').update(readFileSync(gpu)).digest('hex'), digest);
  assert.match(spawnSync('codesign', ['-dv', gpu], { encoding: 'utf8' }).stderr, /runtime/);
  const development = signedGpuProduct(product);
  assert.doesNotMatch(spawnSync('codesign', ['-dv', development], { encoding: 'utf8' }).stderr, /runtime/);
  assert.notEqual(createHash('sha256').update(readFileSync(development)).digest('hex'), digest, 'the signing mode invalidates the cached copy');
  writeFileSync(gpu, Buffer.concat([readFileSync(gpu), Buffer.from('changed')]));
  assert.throws(() => signingOrder(app, compat), /baked code.*digest/);
}), 60000);

test('--archive --embed does not stop after embed staging; plain --embed builds no Swift', () => {
  assert.equal(appleEmbedOnly(['--device', '--embed']), true);
  for (const extra of [['--archive', 'out.ipa'], ['--run'], ['--host']])
    assert.equal(appleEmbedOnly(['--device', '--embed', ...extra]), false);
  assert.equal(appleEmbedOnly(['--device', '--archive', 'out.ipa']), false);
});

test.skipIf(!tools)('GPU archives hash the signed framework binary, including declared modules', () => inDir(dir => {
  const app = { id: 'test.gpu', manifest: {} };
  const digests = new Map();
  const source = resolve(dir, 'gpu.c');
  writeFileSync(source, 'int gpu_fixture(void){return 42;}\n');
  for (const name of ['ExactGpu', 'ExactGpu_title_sky']) {
    const product = resolve(dir, `${name}.dylib`);
    const cc = spawnSync('xcrun', ['--sdk', 'iphonesimulator', 'clang', '-target', 'arm64-apple-ios17.0-simulator', '-dynamiclib', source, '-o', product], { encoding: 'utf8' });
    assert.equal(cc.status, 0, cc.stderr);
    const options = { app, framework: name, platform: 'iPhoneSimulator' };
    const signed = signedGpuProduct(product, options);
    assert.equal(signed, resolve(dir, 'signed/frameworks', `${name}.dylib`, `${name}.framework`, name));
    const hash = () => createHash('sha256').update(readFileSync(signed)).digest('hex');
    const digest = hash();
    digests.set(name, digest);
    const info = spawnSync('plutil', ['-extract', 'CFBundleIdentifier', 'raw', '-o', '-', resolve(dirname(signed), 'Info.plist')], { encoding: 'utf8' });
    assert.equal(info.stdout.trim(), `test.gpu.${name.toLowerCase().replaceAll('_', '-')}`);
    const installName = spawnSync('otool', ['-D', signed], { encoding: 'utf8' });
    assert.equal(installName.status, 0, installName.stderr);
    assert.ok(installName.stdout.includes(`@rpath/${name}.framework/${name}`));
    const verify = spawnSync('codesign', ['--verify', '--deep', '--strict', dirname(signed)], { encoding: 'utf8' });
    assert.equal(verify.status, 0, verify.stderr);
    assert.equal(signedGpuProduct(product, options), signed);
    assert.equal(hash(), digest, 'a warm build preserves the baked bytes');
    const otherProduct = resolve(dir, `other-${name}.dylib`);
    cpSync(product, otherProduct);
    const other = signedGpuProduct(otherProduct, { ...options, app: { ...app, id: 'test.other' } });
    assert.notEqual(other, signed, 'apps sharing a Cargo output directory keep separate signed frameworks');
    assert.equal(hash(), digest);
    const packaged = resolve(dir, 'Payload', 'Fixture.app', 'Frameworks');
    mkdirSync(packaged, { recursive: true });
    const capture = resolve(dir, 'capture');
    const captured = resolve(capture, `${name}.framework`, name);
    cpSync(dirname(signed), dirname(captured), { recursive: true });
    rmSync(captured);
    captureAppleProduct({ products: [{ path: signed, bytes: statSync(signed).size, sha256: digest }] }, signed, captured);
    const artifact = { built: `${name}.dylib`, load: name === 'ExactGpu' ? 'libexact_gpu.dylib' : 'libexact_gpu_title_sky.dylib', framework: name };
    const embed = resolve(dir, 'embed');
    mkdirSync(embed, { recursive: true });
    copyAppleGpuProducts(capture, embed, [artifact], true);
    assert.equal(createHash('sha256').update(readFileSync(resolve(embed, `${name}.framework`, name))).digest('hex'), digest);
    assert.equal(spawnSync('codesign', ['--verify', '--deep', '--strict', resolve(embed, `${name}.framework`)]).status, 0);
    assertArchiveFrameworks(embed);
    copyAppleGpuProducts(capture, packaged, [artifact], true);
    assertArchiveFrameworks(packaged);
    assert.equal(createHash('sha256').update(readFileSync(resolve(packaged, `${name}.framework`, name))).digest('hex'), digest);
    const copied = spawnSync('codesign', ['--verify', '--deep', '--strict', resolve(packaged, `${name}.framework`)], { encoding: 'utf8' });
    assert.equal(copied.status, 0, copied.stderr);
    const development = signedGpuProduct(product);
    assert.equal(development, resolve(dir, 'signed', `${name}.dylib`));
    assert.notEqual(createHash('sha256').update(readFileSync(development)).digest('hex'), digest);
    const developmentCapture = resolve(dir, 'development-capture'), developmentEmbed = resolve(dir, 'development-embed');
    mkdirSync(developmentCapture, { recursive: true }); mkdirSync(developmentEmbed, { recursive: true });
    cpSync(development, resolve(developmentCapture, artifact.built));
    copyAppleGpuProducts(developmentCapture, developmentEmbed, [artifact]);
    assert.deepEqual(readFileSync(resolve(developmentEmbed, artifact.load)), readFileSync(development));
    cpSync(development, resolve(packaged, 'libexact_gpu.dylib'));
    assert.throws(() => assertArchiveFrameworks(packaged), /loose libraries.*libexact_gpu.dylib/);
    rmSync(resolve(packaged, 'libexact_gpu.dylib'));
  }
  const bundle = resolve(dir, 'Payload/Fixture.app');
  writeFileSync(resolve(dir, 'main.c'), 'int main(void){return 0;}\n');
  const cc = spawnSync('xcrun', ['--sdk', 'iphonesimulator', 'clang', '-target', 'arm64-apple-ios17.0-simulator', resolve(dir, 'main.c'), '-o', resolve(bundle, 'Fixture')], { encoding: 'utf8' });
  assert.equal(cc.status, 0, cc.stderr);
  writeFileSync(resolve(bundle, 'Info.plist'), plist('Fixture').replace('</dict>', '<key>CFBundleIdentifier</key><string>test.gpu</string><key>CFBundlePackageType</key><string>APPL</string></dict>'));
  assert.equal(spawnSync('codesign', ['--force', '--sign', '-', '--timestamp=none', bundle]).status, 0);
  const verify = spawnSync('codesign', ['--verify', '--deep', '--strict', bundle], { encoding: 'utf8' });
  assert.equal(verify.status, 0, verify.stderr);
  for (const [name, digest] of digests)
    assert.equal(createHash('sha256').update(readFileSync(resolve(bundle, 'Frameworks', `${name}.framework`, name))).digest('hex'), digest, 'the final app seal preserves the baked GPU bytes');
}), 60000);

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

// LLP 1069.008 D9: a runtime grant's entitlement reaches the one executable it names.
test.skipIf(!tools)('release signs each helper a runtime grant names with exactly its exceptions, and other code with none', () => inDir((dir) => {
  const app = resolve(dir, 'Fixture.app');
  // Exits 0 when the hardened runtime lets it map JIT memory, 1 when it refuses.
  writeFileSync(resolve(dir, 'jit.c'), '#include <sys/mman.h>\nint main(void){return mmap(0,16384,PROT_READ|PROT_WRITE|PROT_EXEC,MAP_PRIVATE|MAP_ANON|MAP_JIT,-1,0)==MAP_FAILED;}\n');
  const cc = (out, ...flags) => {
    mkdirSync(dirname(resolve(app, out)), { recursive: true });
    const r = spawnSync('clang', [...flags, '-o', resolve(app, out), resolve(dir, 'jit.c')], { encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
  };
  cc('Contents/MacOS/Fixture');
  writeFileSync(resolve(app, 'Contents/Info.plist'), plist('Fixture'));
  cc('Contents/Helpers/node/bin/node', '-Wl,-no_adhoc_codesign');
  cc('Contents/Helpers/node/bin/npx', '-Wl,-no_adhoc_codesign');
  cc('Contents/Helpers/node/lib/addon.dylib', '-dynamiclib', '-Wl,-no_adhoc_codesign');
  cc('Contents/Helpers/Tool.app/Contents/MacOS/Tool', '-Wl,-no_adhoc_codesign');
  writeFileSync(resolve(app, 'Contents/Helpers/Tool.app/Contents/Info.plist'), plist('Tool'));
  cc('Contents/Helpers/Tool.app/Contents/MacOS/sub', '-Wl,-no_adhoc_codesign'); // a second executable in a granted bundle
  // A name the plist spells with an entity: the walk's quick read takes `A&amp;B`, Foundation and codesign `A&B`.
  cc('Contents/Helpers/Amp.app/Contents/MacOS/A&B', '-Wl,-no_adhoc_codesign');
  writeFileSync(resolve(app, 'Contents/Helpers/Amp.app/Contents/Info.plist'), plist('A&amp;B'));
  // A staging file the resources tree carries; codesign takes only code under Helpers/, so data goes under Resources/.
  mkdirSync(resolve(app, 'Contents/Resources/stamp'), { recursive: true });
  writeFileSync(resolve(app, 'Contents/Resources/stamp/channel'), 'beta\n');
  const manifest = { host: { macos: { resources: [{ from: 'node', to: 'Helpers/node' }, { from: 'tool', to: 'Helpers/Tool.app' }, { from: 'stamp', to: 'Resources/stamp' }, { from: 'trick', to: 'Helpers/Trick.app' },
    { from: 'amp', to: 'Helpers/Amp.app' }, { from: 'plat', to: 'Helpers/Plat.app' }] } } };
  const outside = `is not inside a host.macos.resources destination (${manifest.host.macos.resources.map(({ to }) => to).join(', ')})`;
  const [jit, memory, library] = ['allow-jit', 'allow-unsigned-executable-memory', 'disable-library-validation'].map((name) => `com.apple.security.cs.${name}`);
  const microphone = 'com.apple.security.device.audio-input'; // the app's own, from a device grant
  const compat = { reach: { entitlements: [microphone], helperEntitlements: {
    'Helpers/node/bin/node': [jit, memory], 'Helpers/Tool.app/Contents/MacOS/Tool': [jit, library],
    'Helpers/Amp.app/Contents/MacOS/A&B': [jit] } } };
  const sign = (c) => signRelease(app, { compat: c, manifest, identity: '-', id: 'dev.exact.fixture', scratch: dir, timestamp: ['--timestamp=none'] });
  const keys = (path) => {
    const r = spawnSync('codesign', ['-d', '--entitlements', '-', '--xml', resolve(app, path)], { encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
    return [...r.stdout.matchAll(/<key>([^<]+)<\/key>/g)].map((m) => m[1]);
  };
  const runs = (path) => spawnSync(resolve(app, path)).status;

  sign(compat);
  const verify = spawnSync('codesign', ['--verify', '--deep', '--strict', app], { encoding: 'utf8' });
  assert.equal(verify.status, 0, verify.stderr);
  assert.deepEqual(keys('Contents/Helpers/node/bin/node'), [jit, memory]);
  assert.deepEqual(keys('Contents/Helpers/Tool.app'), [jit, library], 'a nested app carries its executable\'s grant');
  assert.deepEqual(keys('Contents/Helpers/Amp.app'), [jit], 'the bundle signed after its executable keeps the grant');
  assert.deepEqual(keys('.'), [microphone], 'no runtime exception reaches the app');
  for (const path of ['Contents/Helpers/node/bin/npx', 'Contents/Helpers/node/lib/addon.dylib', 'Contents/Helpers/Tool.app/Contents/MacOS/sub']) assert.deepEqual(keys(path), [], path);
  for (const path of ['Contents/Helpers/node/bin/node', 'Contents/Helpers/node/bin/npx', 'Contents/Helpers/Tool.app'])
    assert.match(spawnSync('codesign', ['-dv', resolve(app, path)], { encoding: 'utf8' }).stderr, /flags=0x10002\(adhoc,runtime\)/, path);
  // The hardened runtime itself: JIT for the granted helper and refused to its undeclared sibling.
  assert.equal(runs('Contents/Helpers/node/bin/node'), 0);
  assert.equal(runs('Contents/Helpers/Tool.app/Contents/MacOS/Tool'), 0);
  assert.equal(runs('Contents/Helpers/Amp.app/Contents/MacOS/A&B'), 0);
  assert.equal(runs('Contents/Helpers/node/bin/npx'), 1);
  assert.equal(runs('Contents/Helpers/Tool.app/Contents/MacOS/sub'), 1, 'a bundle\'s grant stays on the executable it runs');
  assert.equal(runs('Contents/MacOS/Fixture'), 1, 'the app carries no runtime exception');
  writeFileSync(resolve(app, 'Contents/Resources/stamp/channel'), 'stable\n');
  assert.notEqual(spawnSync('codesign', ['--verify', '--deep', '--strict', app]).status, 0, 'the stamp is inside the seal');

  // An app that declares none: every signature carries no entitlements, as before D9.
  sign({ reach: { entitlements: [] } });
  assert.equal(spawnSync('codesign', ['--verify', '--deep', '--strict', app]).status, 0);
  for (const path of ['.', 'Contents/Helpers/node/bin/node', 'Contents/Helpers/Tool.app']) assert.deepEqual(keys(path), [], path);
  assert.equal(runs('Contents/Helpers/node/bin/node'), 1);

  // Every target that is not a helper executable the walk signs is refused in one error, before anything is signed.
  symlinkSync('node', resolve(app, 'Contents/Helpers/node/bin/link'));
  // A bundle whose Info.plist hides a decoy name in a comment: Apple's parser reads `Other`, so a grant for `Granted` must not reach the bundle.
  cc('Contents/Helpers/Trick.app/Contents/MacOS/Granted', '-Wl,-no_adhoc_codesign');
  cc('Contents/Helpers/Trick.app/Contents/MacOS/Other', '-Wl,-no_adhoc_codesign');
  writeFileSync(resolve(app, 'Contents/Helpers/Trick.app/Contents/Info.plist'), plist('Other').replace('<dict>', '<dict><!-- <key>CFBundleExecutable</key><string>Granted</string> -->'));
  // Foundation prefers Info-macos.plist, so this bundle runs `Other` whatever Info.plist names.
  cc('Contents/Helpers/Plat.app/Contents/MacOS/Granted', '-Wl,-no_adhoc_codesign');
  cc('Contents/Helpers/Plat.app/Contents/MacOS/Other', '-Wl,-no_adhoc_codesign');
  writeFileSync(resolve(app, 'Contents/Helpers/Plat.app/Contents/Info.plist'), plist('Granted'));
  writeFileSync(resolve(app, 'Contents/Helpers/Plat.app/Contents/Info-macos.plist'), plist('Other'));
  // A signature written to one hard link is every link's.
  linkSync(resolve(app, 'Contents/Helpers/node/bin/npx'), resolve(app, 'Contents/Helpers/node/bin/npx-twin'));
  const bad = Object.fromEntries(['Helpers/node/lib/addon.dylib', 'Helpers/node/bin/missing', 'Helpers/node/bin/link', 'Frameworks/node/bin/node',
    'Helpers/Trick.app/Contents/MacOS/Granted', 'app', 'Helpers/Plat.app/Contents/MacOS/Granted', 'Helpers/node/bin/npx'].map((path) => [path, [jit]]));
  assert.throws(() => sign({ reach: { helperEntitlements: bad } }), (error) => {
    assert.deepEqual(error.message.split('\n').map((line) => line.replace(/^runtime grant: Contents\/\S+ /, '')), [
      'is not a Mach-O executable', 'is not a file in the bundle', 'is not a file in the bundle',
      outside, 'is not code the release signs', outside, 'is not code the release signs',
      'has another hard link, which its signature would reach']);
    return true;
  });
  assert.deepEqual(keys('Contents/Helpers/node/bin/node'), [], 'a refusal signs nothing');
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

// LLP 1075.003.000.001 §5: `exact hatch` writes a stub a target, wires it into a module it wrote, and declares the word.
test('exact hatch writes each target\'s stub, wires it in, and declares the word with its platforms', async () => {
  const { hatch } = await import('./exact.mjs');
  const { readFileSync } = await import('node:fs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-hatch-'));
  const was = process.env.EXACT_APP_DIR;
  try {
    for (const d of ['apple', 'web']) mkdirSync(resolve(dir, d));
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    column hatch="avatar"\n');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Scratch', app: { id: 'com.example.scratch', name: 'Scratch' }, host: { ios: {}, web: {} } }));
    process.env.EXACT_APP_DIR = dir;
    const said = [];
    const first = hatch(['avatar'], line => said.push(line));
    assert.deepEqual(first.platforms, ['ios', 'web']);
    assert.deepEqual(JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).hatches, { avatar: ['ios', 'web'] });
    const swift = readFileSync(resolve(dir, 'modules/apple/Hatches.swift'), 'utf8');
    assert.ok(swift.includes('if element.hatch == .avatar { avatarHatch(element) }') && swift.includes('if element.hatch == .avatar { avatarHatchEnded(element) }'));
    assert.ok(readFileSync(resolve(dir, 'modules/apple/AvatarHatch.swift'), 'utf8').includes('func avatarHatch(_ element: ExactElement)'));
    const page = readFileSync(resolve(dir, 'modules/web/index.js'), 'utf8');
    assert.ok(page.includes("import * as avatarHatch from './hatch-avatar.js';") && page.includes('hatches["avatar"] = avatarHatch;'));
    assert.ok(readFileSync(resolve(dir, 'modules/web/hatch-avatar.js'), 'utf8').includes('export function elementEnded(e)'));
    // A second word joins the first; the same word again changes nothing; the scopes take no word.
    hatch(['unread-dot'], () => {});
    const again = hatch(['avatar'], () => {});
    assert.deepEqual(again.wrote, []);
    assert.deepEqual(JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).hatches, { avatar: ['ios', 'web'], 'unread-dot': ['ios', 'web'] });
    assert.ok(readFileSync(resolve(dir, 'modules/apple/Hatches.swift'), 'utf8').includes('if element.hatch == .unreadDot { unreadDotHatch(element) }'));
    hatch(['--window'], () => {});
    assert.ok(readFileSync(resolve(dir, 'modules/apple/Hatches.swift'), 'utf8').includes('        windowHatch(window)\n        // exact:window\n'));
    assert.ok(readFileSync(resolve(dir, 'modules/web/index.js'), 'utf8').includes('export function window(x) { windowHatch.built(x); }'));
    assert.throws(() => hatch(['Not A Word'], () => {}), /name a word/);
    // A Linux crate gets one hatches file, each word an arm and its functions, and is told what its build.rs owes.
    mkdirSync(resolve(dir, 'linux'));
    const linux = hatch(['unread-dot'], () => {});
    assert.ok(linux.platforms.includes('linux') && linux.todo.some(line => line.includes('rust_hatch_entry')));
    const rust = readFileSync(resolve(dir, 'modules/linux/hatches.rs'), 'utf8');
    assert.ok(rust.includes('"unread-dot" => unread_dot_hatch(element, context),') && rust.includes('fn unread_dot_hatch_ended<H: Hatches>'));
    assert.deepEqual(JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).hatches['unread-dot'], ['ios', 'web', 'linux']);
    // A build.rs that already asks for the app's hatches owes nothing: the first file only makes it run again.
    rmSync(resolve(dir, 'modules/linux'), { recursive: true });
    writeFileSync(resolve(dir, 'linux/build.rs'), 'fn main() { let _ = contract::native::rust_hatch_entry; }\n');
    assert.deepEqual(hatch(['unread-dot'], () => {}).todo.filter(line => line.includes('build.rs')), []);
    // In a module of the app's own, with no marker, the lines to add are shown, not written.
    rmSync(resolve(dir, 'modules/apple'), { recursive: true });
    mkdirSync(resolve(dir, 'modules/apple'));
    writeFileSync(resolve(dir, 'modules/apple/Mine.swift'), 'final class Mine: ExactModule {}\nlet exactModule: ExactModule.Type = Mine.self\n');
    const mine = hatch(['seal'], () => {});
    assert.equal(mine.todo.length, 2);
    assert.ok(mine.todo.some(line => line.includes('if element.hatch == .seal { sealHatch(element) }')));
    assert.equal(readFileSync(resolve(dir, 'modules/apple/Mine.swift'), 'utf8').includes('sealHatch'), false);
  } finally {
    if (was === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = was;
    rmSync(dir, { recursive: true, force: true });
  }
});

// Catalogs are native inputs even when the app declares no native view tags.
test('Apple asset catalogs select platform modules and invalidate native receipts on additions and deletions', () => inDir(dir => {
  const { appleAssetCatalogInventory } = require('../host/apple/assets.mjs');
  const { pendingBuildInputs } = require('./app.mjs');
  const app = { dir, platform: 'ios', manifest: {} };
  const build = { binary: { assetCatalogApp: app, metadata: {}, inputs: [], directories: [], missing: [] } };
  const put = (path, text) => { mkdirSync(dirname(resolve(dir, path)), { recursive: true }); writeFileSync(resolve(dir, path), text); };
  assert.deepEqual(pendingBuildInputs(build), []);
  put('modules/apple/Shared.xcassets/Contents.json', '{"info":{"version":1,"author":"xcode"}}');
  assert.deepEqual(pendingBuildInputs(build), ['Apple asset catalogs']);
  const { appleRunBinary, appleArtifacts } = require('../host/apple/build.mjs');
  const launchApp = { ...app, id: 'com.exact.catalog', name: 'catalog', displayName: 'Catalog', target: resolve(dir, 'target') };
  const paths = appleArtifacts(launchApp);
  assert.equal(appleRunBinary(launchApp), resolve(paths.bundle, 'Contents/MacOS', paths.executable));
  const shared = appleAssetCatalogInventory(app, 'ios');
  assert.equal(shared[0].path, 'modules/apple/Shared.xcassets');
  build.binary.metadata.appleAssetCatalogs = shared;
  assert.deepEqual(pendingBuildInputs(build), []);
  put('modules/apple/Shared.xcassets/Contents.json', '{"info":{"version":1,"author":"exact"}}');
  assert.deepEqual(pendingBuildInputs(build), ['Apple asset catalogs']);
  put('ios/modules/Local.xcassets/Contents.json', '{}');
  assert.deepEqual(appleAssetCatalogInventory(app, 'ios').map(entry => entry.path), ['ios/modules/Local.xcassets']);
  assert.deepEqual(appleAssetCatalogInventory(app, 'macos').map(entry => entry.path), ['modules/apple/Shared.xcassets']);
  rmSync(resolve(dir, 'ios/modules'), { recursive: true });
  assert.equal(appleAssetCatalogInventory(app, 'ios')[0].path, shared[0].path);
  rmSync(resolve(dir, 'modules/apple/Shared.xcassets'), { recursive: true });
  assert.deepEqual(pendingBuildInputs(build), ['Apple asset catalogs']);
  assert.equal(appleRunBinary(launchApp), paths.binary);
  mkdirSync(resolve(dir, 'modules/apple/Linked.xcassets'));
  symlinkSync(resolve(dir, 'outside'), resolve(dir, 'modules/apple/Linked.xcassets/Contents.json'));
  assert.throws(() => appleAssetCatalogInventory(app, 'ios'), /symlinks/);
}));

test.skipIf(process.platform !== 'darwin')('the iOS launch screen is the app\'s own ground: its colours, else the grouped grey under a first route that scrolls a grouped list, else the system\'s (LLP 1116.011 D2.3)', () => inDir(dir => {
  const { useXcode } = require('../host/apple/devices.mjs');
  const { iosAssets } = require('../host/apple/assets.mjs');
  useXcode();
  let n = 0;
  const made = (manifest, options) => {
    const out = resolve(dir, `app${n++}.app`); mkdirSync(out);
    const keys = iosAssets({ dir, name: 'Launch', manifest }, out, false, options);
    const result = spawnSync('xcrun', ['assetutil', '--info', resolve(out, 'Assets.car')], { encoding: 'utf8' });
    const colours = keys.UILaunchScreen ? JSON.parse(result.stdout).filter(asset => asset.Name === 'ExactLaunch')
      .map(asset => [asset.Appearance ?? 'light', asset['Color components'].map(c => Math.round(c * 255))]) : [];
    return { keys, colours };
  };
  // Stopwatch, round 13: a first route on the grouped grey, whose launch screen was the system's white.
  const grouped = made({}, { launch: 'grouped' });
  assert.equal(grouped.keys.UILaunchScreen.UIColorName, 'ExactLaunch');
  assert.deepEqual(grouped.colours, [['light', [242, 242, 247, 255]], ['UIAppearanceDark', [0, 0, 0, 255]]]);
  // A page of the platform's own stays the system's launch screen (`systemBackground`, light and dark).
  for (const options of [{ launch: 'system' }, {}]) assert.deepEqual(made({}, options).keys, {});
  // The author's colours name it whatever the first route stands on.
  const named = made({ background_color: '#ffeedd', background_color_dark: '#102030' }, { launch: 'grouped' });
  assert.deepEqual(named.colours, [['light', [255, 238, 221, 255]], ['UIAppearanceDark', [16, 32, 48, 255]]]);
}), 60000);

test.skipIf(process.platform !== 'darwin')('Apple asset catalogs retain named images and colours, share the iOS icon pass, and reuse unchanged compiles', () => inDir(dir => {
  const { useXcode } = require('../host/apple/devices.mjs');
  const { appleAssets, iosAssets, appleAssetCatalogInventory } = require('../host/apple/assets.mjs');
  const { readFileSync, readdirSync, statSync } = require('node:fs');
  useXcode();
  const put = (path, bytes) => { mkdirSync(dirname(resolve(dir, path)), { recursive: true }); writeFileSync(resolve(dir, path), bytes); };
  const json = (path, value) => put(path, JSON.stringify(value));
  const info = { version: 1, author: 'xcode' };
  const source = 'modules/apple/Artwork.xcassets';
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=', 'base64');
  json(`${source}/Contents.json`, { info });
  put(`${source}/Badge.imageset/image.png`, png);
  json(`${source}/Badge.imageset/Contents.json`, { info, images: [{ idiom: 'universal', filename: 'image.png' }] });
  const colour = red => ({ info, colors: [{ idiom: 'universal', color: { 'color-space': 'srgb', components: { red, green: '0.0', blue: '0.0', alpha: '1.0' } } }] });
  json(`${source}/Ink.colorset/Contents.json`, colour('0.2'));
  put('icon.png', png);
  const app = { dir, name: 'Catalog', manifest: { icons: [{ src: 'icon.png', sizes: '1024x1024' }], background_color: '#fff', background_color_dark: '#123456' } };
  const cache = { dir: resolve(dir, 'cache'), stamp: 'fixture-toolchain' };
  const inventory = appleAssetCatalogInventory(app, 'ios');
  const ios = resolve(dir, 'iOS.app'); mkdirSync(ios);
  const keys = iosAssets(app, ios, false, { catalog: true, kept: cache, expected: inventory });
  assert.equal(keys.CFBundleIcons.CFBundlePrimaryIcon.CFBundleIconName, 'AppIcon');
  assert.equal(keys.UILaunchScreen.UIColorName, 'ExactLaunch');
  assert.equal(keys.NSAccentColorName, undefined, 'no AccentColor set, no accent named');
  const inspect = path => {
    const result = spawnSync('xcrun', ['assetutil', '--info', resolve(path, 'Assets.car')], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout);
  };
  const first = inspect(ios);
  for (const name of ['Badge', 'Ink', 'AppIcon', 'ExactLaunch']) assert.ok(first.some(asset => asset.Name === name), name);
  assert.equal(first.filter(asset => asset.Name === 'ExactLaunch').length, 2);
  const held = readdirSync(cache.dir).find(name => name.startsWith('assets-ios-'));
  const before = statSync(resolve(cache.dir, held, 'Assets.car'), { bigint: true }).mtimeNs;
  const second = resolve(dir, 'Second.app'); mkdirSync(second);
  assert.deepEqual(iosAssets(app, second, false, { catalog: true, kept: cache, expected: inventory }), keys);
  assert.equal(statSync(resolve(cache.dir, held, 'Assets.car'), { bigint: true }).mtimeNs, before);
  assert.deepEqual(readFileSync(resolve(second, 'Assets.car')), readFileSync(resolve(ios, 'Assets.car')));
  json(`${source}/Ink.colorset/Contents.json`, colour('0.8'));
  assert.throws(() => iosAssets(app, second, false, { kept: cache, expected: inventory }), /changed after the bake/);
  iosAssets(app, second, false, { catalog: true, kept: cache });
  assert.notEqual(readdirSync(cache.dir).find(name => name.startsWith('assets-ios-')), held);
  assert.notDeepEqual(readFileSync(resolve(second, 'Assets.car')), readFileSync(resolve(ios, 'Assets.car')));
  // An `AccentColor` set is named the app's accent, as Xcode's default
  json(`${source}/AccentColor.colorset/Contents.json`, colour('0.5'));
  const accented = resolve(dir, 'Accented.app'); mkdirSync(accented);
  assert.equal(iosAssets(app, accented, false, { catalog: true }).NSAccentColorName, 'AccentColor');
  const cached = resolve(dir, 'Cached.app'); mkdirSync(cached);
  assert.equal(iosAssets(app, cached, false, { catalog: true, kept: cache }).NSAccentColorName, 'AccentColor', 'the cached path names it too');
  assert.ok(inspect(accented).some(asset => asset.Name === 'AccentColor'));
  const accentedMac = resolve(dir, 'AccentedMac.bundle/Contents/Resources'); mkdirSync(accentedMac, { recursive: true });
  assert.equal(appleAssets(app, accentedMac, { platform: 'macos' }).NSAccentColorName, 'AccentColor', 'macOS names it too');
  rmSync(resolve(dir, source, 'AccentColor.colorset'), { recursive: true, force: true });
  const mac = resolve(dir, 'Catalog.bundle/Contents/Resources'); mkdirSync(mac, { recursive: true });
  put('Catalog.bundle/Contents/Info.plist', '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.exact.catalog.test</string></dict></plist>');
  appleAssets(app, mac, { platform: 'macos', kept: cache });
  assert.ok(inspect(mac).some(asset => asset.Name === 'Badge'));
  assert.ok(readdirSync(cache.dir).some(name => name.startsWith('assets-ios-')), 'macOS must not evict the iOS cache');
  put('Lookup.swift', 'import AppKit\nlet bundle = Bundle(path: CommandLine.arguments[1])!\nguard bundle.image(forResource: "Badge") != nil, let colour = NSColor(named: "Ink", bundle: bundle)?.usingColorSpace(.sRGB), abs(colour.redComponent - 0.8) < 0.01 else { fatalError("named asset lookup failed") }\nprint("named assets loaded")\n');
  const lookup = resolve(dir, 'lookup');
  const compile = spawnSync('xcrun', ['swiftc', resolve(dir, 'Lookup.swift'), '-o', lookup], { encoding: 'utf8' });
  assert.equal(compile.status, 0, compile.stderr);
  const loaded = spawnSync(lookup, [resolve(dir, 'Catalog.bundle')], { encoding: 'utf8' });
  assert.equal(loaded.status, 0, loaded.stderr);
  assert.match(loaded.stdout, /named assets loaded/);
  rmSync(resolve(dir, source), { recursive: true });
  appleAssets(app, mac, { platform: 'macos', kept: cache });
  assert.ok(!existsSync(resolve(mac, 'Assets.car')), 'removing all catalogs clears the previous output');
}), 60000);

// LLP 1116.001 D4.5: `exact contract build` of an app's app.contract rewrites the types its app.ts imports
// when a source's signature changed, and only then, saying so in one line on stderr; `contract types` agrees.
test('contract build regenerates app.contract.d.ts when a signature changed, and only then', () => inDir(dir => {
  const contract = (shape) => `shape Greeting\n  text: string\n${shape}\ncomponent App\n  resource greeting = greeting("x") as shape Greeting\n  view\n    text greeting.text\n`;
  writeFileSync(resolve(dir, 'app.contract'), contract(''));
  writeFileSync(resolve(dir, 'app.ts'), "export const appId = 'com.example.types';\n");
  const exact = (...args) => spawnSync(process.execPath, [resolve(import.meta.dir, 'exact.mjs'), ...args], { cwd: dir, encoding: 'utf8', timeout: 600_000 });
  const build = () => exact('contract', 'build', 'app.contract', '--json');
  const types = resolve(dir, 'app.contract.d.ts');
  let r = build();
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout.trim(), '[]', 'the build\'s reply stays JSON');
  assert.match(r.stderr, /^app\.contract\.d\.ts: written \(the types app\.ts imports\)$/m);
  assert.equal(readFileSync(types, 'utf8'), exact('contract', 'types', 'app.contract').stdout, 'what contract types prints');
  const written = statSync(types).mtimeMs;
  r = build();
  assert.doesNotMatch(r.stderr, /app\.contract\.d\.ts/, 'nothing changed, nothing said');
  assert.equal(statSync(types).mtimeMs, written, 'nor written');
  writeFileSync(resolve(dir, 'app.contract'), contract('  count: number\n'));
  r = build();
  assert.match(r.stderr, /^app\.contract\.d\.ts: regenerated, since a source's signature changed/m);
  assert.match(readFileSync(types, 'utf8'), /"count": T\d+;/);
  // A build that fails leaves the types as they were; another file than app.contract never writes them.
  writeFileSync(resolve(dir, 'app.contract'), 'not a contract');
  assert.notEqual(build().status, 0);
  assert.match(readFileSync(types, 'utf8'), /"count"/);
  writeFileSync(resolve(dir, 'other.contract'), contract(''));
  assert.doesNotMatch(exact('contract', 'build', 'other.contract').stderr, /app\.contract\.d\.ts/);
}), 600_000); // The first call may compile the compiler.
