// What an Apple build writes and checks (host/apple/build.mjs): Hermes bundles, asset catalogs, plists and entitlements, the
// SDK, and signing.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { delimiter, resolve, toNamespacedPath } from 'node:path';
import { HERMES_INSTALLER, hermesBundle, hermesLeanSysRoots } from './app.mjs';
import { checkHermesBundles, hermesCrossTargets, hermesSetupTargets } from './exact.mjs';
import { iosAssets, entitlements, infoPlist, macInfoPlist, documentTypes, importedTypes, exportedTypes, writeUsageStrings, designCompatible, COMPATIBLE_SDK, moduleTripleFor } from '../host/apple/build.mjs';
import { useXcode } from '../host/apple/devices.mjs';
import { macReleaseEntitlements, launchImageInventory } from '../host/apple/assets.mjs';

test('the Apple build rejects --phone before selecting a simulator or resolving an app', () => {
  const result = spawnSync(process.execPath, ['host/apple/build.mjs', '--ios', '--phone', 'other-lane', 'no-such-app'], { encoding: 'utf8' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /--phone needs --device; for a simulator use --sim/);
});

// These cases run cargo (the filesystem tool, bakes, locks). A shell whose PATH
// omits rustup's bin directory still finds it there; without cargo, say so.
const cargoBin = resolve(process.env.CARGO_HOME ?? resolve(homedir(), '.cargo'), 'bin');
if (!(process.env.PATH ?? '').split(delimiter).includes(cargoBin)) process.env.PATH = `${process.env.PATH ?? ''}${delimiter}${cargoBin}`;
if (!Bun.which('cargo', { PATH: process.env.PATH })) throw new Error(`these tests need cargo: put it on PATH or in ${cargoBin}`);
// The fixtures name their apps; a caller's EXACT_APP_DIR would redirect every one.
delete process.env.EXACT_APP_DIR;

for (const [sdk, platform, ios, device, tv] of [
  ['macosx', 'MACOS', false, false, false],
  ['iphoneos', 'IOS', true, true, false],
  ['iphonesimulator', 'IOSSIMULATOR', true, false, false],
  ['appletvos', 'TVOS', true, true, true],
  ['appletvsimulator', 'TVOSSIMULATOR', true, false, true],
]) test(`app module deployment target follows minimumOS: ${sdk}`, () => {
  if (process.platform !== 'darwin') return;
  useXcode();
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-module-target-'));
  try {
    const source = resolve(dir, 'Module.swift'), dylib = resolve(dir, 'Module.dylib');
    for (const raised of [false, true]) {
      const minimum = raised ? '26.0' : ios ? '17.0' : '14.0';
      const app = { manifest: raised ? { host: { macos: { minimumOS: '26.0' }, ios: { minimumOS: '26.0' } } } : {} };
      writeFileSync(source, `@available(macOS ${minimum}, iOS ${minimum}, tvOS ${minimum}, *)\nfunc requiredAPI() -> Int { 26 }\npublic func moduleValue() -> Int { requiredAPI() }\n`);
      const built = spawnSync('xcrun', ['--sdk', sdk, 'swiftc', '-parse-as-library', '-emit-library', '-target', moduleTripleFor(app, ios, device, tv), source, '-o', dylib], { encoding: 'utf8' });
      assert.equal(built.status, 0, `${sdk} minimumOS ${minimum}: ${built.stderr}`);
      const load = spawnSync('xcrun', ['vtool', '-show-build', dylib], { encoding: 'utf8' });
      assert.equal(load.status, 0, load.stderr);
      assert.match(load.stdout, new RegExp(`platform ${platform}\\s`));
      assert.match(load.stdout, new RegExp(`minos ${minimum.replace('.', '\\.')}\\s`));
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 60_000);

test('iOS Hermes preflight selects the pinned digest and exact installer command', () => {
  const home = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-hermes-home-')));
  try {
    const env = { HOME: home };
    const simulator = hermesBundle('aarch64-apple-ios-sim', env);
    const intelSimulator = hermesBundle('x86_64-apple-ios', env);
    assert.equal(simulator.installed, false);
    assert.equal(simulator.root, intelSimulator.root, 'both simulator triples select the universal bundle');
    assert.equal(simulator.fix, `cargo run --manifest-path ${HERMES_INSTALLER} -- --target aarch64-apple-ios-sim`);
  } finally { rmSync(home, { recursive: true, force: true }); }
});

test('setup check uses the installer result and preserves its missing-bundle message', () => {
  const calls = [], missingMessage = 'Hermes bundle installation or check failed: install with cargo run --manifest-path installer -- --target aarch64-apple-darwin';
  const execute = (command, args, options) => {
    calls.push({command, args, options});
    return calls.length === 1
      ? {status: 1, stdout: 'Checking pinned Hermes bundles', stderr: missingMessage}
      : {status: 0, stdout: 'Verified every installed bundle', stderr: ''};
  };
  const env = {HOME:'/empty-cargo-home', CARGO_HOME:'/empty-cargo-home'};
  const missing = checkHermesBundles(env, execute, 'darwin', 'arm64');
  assert.equal(missing.ok, false);
  assert.ok(missing.message.includes(missingMessage));
  // The required check is the host bundle alone, locked and offline.
  assert.ok(calls[0].args.includes('--check'));
  assert.ok(calls[0].args.includes('--locked'));
  assert.ok(calls[0].args.includes('--offline'));
  assert.ok(!calls[0].args.includes('--target'), 'cross targets are separate, optional rows');
  assert.equal(calls[0].options.env.HERMES_LEAN_SYS_OFFLINE, '1');
  assert.doesNotMatch(missing.fix, /--check/);
  const installed = checkHermesBundles(env, execute, 'darwin', 'arm64');
  assert.equal(installed.ok, true);
  assert.match(installed.message, /Verified every installed bundle/);
});

test('each Apple TypeScript destination checks its own Hermes bundle, device included', () => {
  assert.deepEqual(hermesCrossTargets('darwin', 'arm64'), [
    { target: 'aarch64-apple-ios-sim', need: 'iOS Simulator TypeScript' },
    { target: 'aarch64-apple-ios', need: 'iOS device TypeScript' },
    { target: 'aarch64-apple-tvos-sim', need: 'tvOS Simulator TypeScript' },
  ]);
  assert.equal(hermesCrossTargets('darwin', 'x64')[0].target, 'x86_64-apple-ios');
  assert.deepEqual(hermesCrossTargets('linux', 'x64'), []);
  assert.deepEqual([false, true].map(sdk => hermesSetupTargets('darwin', 'arm64', sdk)).concat([hermesSetupTargets('linux', 'x64', true)]), [['aarch64-apple-ios-sim', 'aarch64-apple-tvos-sim'], ['aarch64-apple-ios-sim', 'aarch64-apple-ios', 'aarch64-apple-tvos-sim'], []]); // the device's with the iphoneos SDK
  const calls = [];
  const execute = (command, args) => { calls.push(args); return {status: 0, stdout: 'Verified', stderr: ''}; };
  checkHermesBundles({}, execute, 'darwin', 'arm64', ['aarch64-apple-ios']);
  assert.deepEqual(calls[0].slice(-3), ['--check', '--target', 'aarch64-apple-ios']);
  const install = checkHermesBundles({}, execute, 'darwin', 'arm64', ['aarch64-apple-ios']).fix;
  assert.match(install, /--locked/);
  assert.doesNotMatch(install, /--offline|--check/);
});

test('bake receipts recognize the install-once Hermes cache and explicit installs', () => {
  const home = resolve(tmpdir(), 'exact-hermes-receipt-home');
  const receiptRoots = path => [...new Set(process.platform === 'win32' ? [path, toNamespacedPath(path)] : [path])];
  assert.deepEqual(hermesLeanSysRoots({ HOME: home }), receiptRoots(resolve(home, '.cargo/hermes-lean-sys')));
  const selected = resolve(home, 'selected engine');
  assert.deepEqual(hermesLeanSysRoots({ HERMES_LEAN_SYS_DIR: selected }), receiptRoots(selected));
});

// Real actool: separate compiles into the same bundle silently replace Assets.car.
test.skipIf(process.platform !== 'darwin')('iOS distribution assets retain the icon and both launch appearances', () => {
  useXcode();
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ios-assets-'));
  try {
    writeFileSync(resolve(dir, 'icon.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=', 'base64'));
    const bundle = resolve(dir, 'Fixture.app');
    mkdirSync(bundle);
    const app = { dir, name: 'fixture', manifest: { icons: [{ src: 'icon.png', sizes: '1024x1024' }], background_color: '#fff', background_color_dark: '#123456',
      host: { ios: { launchImage: { src: 'icon.png', width: 40 } } } } };
    const keys = iosAssets(app, bundle, true, { catalog: true });
    assert.equal(keys.CFBundleIcons.CFBundlePrimaryIcon.CFBundleIconName, 'AppIcon');
    const { UIColorName: colour, UIImageName: image } = keys.UILaunchScreen;
    assert.match(colour, /^ExactLaunch-[0-9a-f]{8}$/);
    assert.match(image, /^ExactLaunchImage-[0-9a-f]{8}$/, 'the picture the launch screen centres on its colour');
    assert.equal(keys.ExactLaunchImageSource, 'icon.png', 'an image of the picture decodes from the catalog');
    assert.equal(keys.ExactLaunchImageSHA256, launchImageInventory(app).sha256, 'only for the bytes the catalog was made from');
    const result = spawnSync('xcrun', ['assetutil', '--info', resolve(bundle, 'Assets.car')], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const assets = JSON.parse(result.stdout);
    assert.ok(assets.some(asset => asset.Name === 'AppIcon'), 'Assets.car must retain AppIcon');
    assert.equal(assets.filter(asset => asset.Name === colour).length, 2);
    const launch = assets.filter(asset => asset.Name === image);
    assert.ok(launch.length >= 1, 'Assets.car holds the launch image');
    assert.ok(launch.every(asset => asset.PixelWidth === 40 * asset.Scale), `40 points wide at each scale: ${JSON.stringify(launch)}`);
    // The launch image is a binary input: packaging draws the bytes the bake
    // recorded, and refuses a picture replaced since, or one the receipt lacks.
    const baked = launchImageInventory(app);
    assert.equal(baked.sha256.length, 64);
    const again = resolve(dir, 'Again.app'); mkdirSync(again);
    assert.deepEqual(iosAssets(app, again, true, { expectedLaunchImage: baked }).UILaunchScreen, keys.UILaunchScreen, 'the same content, the same names');
    // iOS draws the launch screen it kept for unchanged `UILaunchScreen` keys (the euxy port's white launch):
    // another colour or picture is another name.
    const darker = { ...app, manifest: { ...app.manifest, background_color: '#08080a' } };
    assert.equal(iosAssets(darker, again, true).UILaunchScreen.UIImageName, image);
    assert.notEqual(iosAssets(darker, again, true).UILaunchScreen.UIColorName, colour);
    const wider = { ...app, manifest: { ...app.manifest, host: { ios: { launchImage: { src: 'icon.png', width: 41 } } } } };
    assert.notEqual(iosAssets(wider, again, true).UILaunchScreen.UIImageName, image);
    assert.throws(() => iosAssets(app, again, true, { expectedLaunchImage: null }), /launch image changed after the bake/);
    writeFileSync(resolve(dir, 'icon.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64'));
    assert.notEqual(launchImageInventory(app).sha256, baked.sha256);
    assert.throws(() => iosAssets(app, again, true, { expectedLaunchImage: baked }), /launch image changed after the bake/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 60000);

test('device grants derive the plists, their translations and the release entitlements (LLP 1069.008)', async () => {
  const { readManifest } = await import('./app.mjs');
  // The bake's `reach` for `device.microphone purpose.mic` and
  // `device.speech-recognition purpose.speech`, with en and fr tables.
  const reach = { base: 'en', locales: ['en', 'fr'], rows: [], entitlements: ['com.apple.security.device.audio-input'],
    usage: { NSMicrophoneUsageDescription: { en: 'Records "takes".', fr: 'Enregistre.' }, NSSpeechRecognitionUsageDescription: { en: 'Transcribes.', fr: 'Transcrit.' } } };
  const app = { id: 'com.example.fixture', displayName: 'Fixture', manifest: { host: {} } };
  assert.equal(macReleaseEntitlements(null), null);
  assert.equal(macReleaseEntitlements({ reach: { ...reach, entitlements: [] } }), null);
  const entitled = macReleaseEntitlements({ reach });
  assert.deepEqual([...entitled.matchAll(/<key>([^<]+)<\/key><true\/>/g)].map((m) => m[1]), ['com.apple.security.device.audio-input']);
  for (const plist of [infoPlist(app, false, { reach }), macInfoPlist(app, { reach })]) {
    assert.match(plist, /<key>NSMicrophoneUsageDescription<\/key><string>Records "takes".<\/string>/);
    assert.match(plist, /<key>NSSpeechRecognitionUsageDescription<\/key><string>Transcribes.<\/string>/);
    assert.match(plist, /<key>CFBundleLocalizations<\/key><array><string>en<\/string><string>fr<\/string><\/array>/);
  }
  // No device grant, no key: the plists are what they were.
  for (const plist of [infoPlist(app, false), macInfoPlist(app, { reach: { ...reach, usage: {} } })]) assert.doesNotMatch(plist, /UsageDescription|CFBundleLocalizations/);
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-usage-'));
  try {
    writeUsageStrings(reach, dir);
    const fr = readFileSync(resolve(dir, 'fr.lproj/InfoPlist.strings'), 'utf8');
    assert.match(fr, /<key>NSMicrophoneUsageDescription<\/key><string>Enregistre.<\/string>/);
    assert.ok(existsSync(resolve(dir, 'en.lproj/InfoPlist.strings')));
    if (process.platform === 'darwin') {
      writeFileSync(resolve(dir, 'entitlements.plist'), entitled);
      for (const file of ['entitlements.plist', 'fr.lproj/InfoPlist.strings']) assert.equal(spawnSync('plutil', ['-lint', resolve(dir, file)]).status, 0, file);
    }
    // The hand-written keys are gone, and the refusal names the grant form.
    for (const platform of ['ios', 'macos']) {
      writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ app: { id: 'com.example.fixture', name: 'Fixture' }, host: { [platform]: { permissions: { NSMicrophoneUsageDescription: 'x' } } } }));
      assert.throws(() => readManifest(dir, 'fixture'), new RegExp(`host\\.${platform}\\.permissions: deleted \\(LLP 1069\\.008\\); declare the device .*device\\.microphone purpose\\.microphone`));
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 60_000);

test('`device.local-network` writes the Local Network prompt on both Apple hosts, over the dev client\'s default (LLP 1069.008 D2, D4)', () => {
  const reach = { base: 'en', locales: ['en', 'fr'], usage: { NSLocalNetworkUsageDescription: { en: 'Finds servers nearby.', fr: 'Trouve les serveurs.' } } };
  const app = (host) => ({ id: 'com.example.fixture', displayName: 'Fixture', manifest: { host } });
  const flat = (plist) => plist.replace(/>\s+</g, '><');
  const prompt = /<key>NSLocalNetworkUsageDescription<\/key><string>Finds servers nearby.<\/string>/;
  for (const plist of [macInfoPlist(app({}), { reach }), infoPlist(app({}), false, { reach })]) assert.match(plist, prompt);
  // The dev client's reach keeps its ATS key; its prompt gives way to the grant's.
  for (const localNetworking of [true, 'Reaches the dev server.']) {
    const ios = flat(infoPlist(app({ ios: { localNetworking } }), false, { reach }));
    assert.match(ios, prompt);
    assert.match(ios, /<key>NSAllowsLocalNetworking<\/key><true\/>/);
    assert.equal(ios.match(/NSLocalNetworkUsageDescription/g).length, 1);
  }
  assert.match(infoPlist(app({ ios: { localNetworking: true } })), /Connects to your dev server on the local network./);
  assert.doesNotMatch(macInfoPlist(app({ ios: { localNetworking: true } })), /NSLocalNetworkUsageDescription/);
});

test('`host.macos.oneTimeCodeAutofill: "marked"` keeps codes to marked fields; absent, the platform\'s default', async () => {
  const { readManifest } = await import('./app.mjs');
  const app = (host) => ({ id: 'com.example.fixture', displayName: 'Fixture', manifest: { host } });
  assert.match(macInfoPlist(app({ macos: { oneTimeCodeAutofill: 'marked' } })).replace(/>\s+</g, '><'), /<key>NSAutoFillRequiresTextContentTypeForOneTimeCodeOnMac<\/key><true\/>/);
  for (const plist of [macInfoPlist(app({})), infoPlist(app({ macos: { oneTimeCodeAutofill: 'marked' } }))]) assert.doesNotMatch(plist, /OneTimeCode/);
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-otc-'));
  try {
    const save = (value) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Fixture', app: { id: 'com.example.fixture', name: 'Fixture' }, host: { macos: { oneTimeCodeAutofill: value } } }));
    save('marked');
    assert.equal(readManifest(dir, 'fixture').host.macos.oneTimeCodeAutofill, 'marked');
    for (const bad of ['any', true]) { save(bad); assert.throws(() => readManifest(dir, 'fixture'), /oneTimeCodeAutofill/); }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('`appTransportSecurity` writes ATS for web content only; absent, no key (#106)', async () => {
  const { readManifest } = await import('./app.mjs');
  const ats = { appTransportSecurity: { allowsArbitraryLoadsInWebContent: true } };
  const app = (host) => ({ id: 'com.example.fixture', displayName: 'Fixture', manifest: { host } });
  const flat = (plist) => plist.replace(/>\s+</g, '><');
  const web = /<key>NSAppTransportSecurity<\/key><dict><key>NSAllowsArbitraryLoadsInWebContent<\/key><true\/><\/dict>/;
  assert.match(flat(macInfoPlist(app({ macos: ats }))), web);
  assert.match(flat(infoPlist(app({ ios: ats }))), web);
  // Beside the dev client's local networking, one dictionary with both keys.
  assert.match(flat(infoPlist(app({ ios: { ...ats, localNetworking: true } }))), /<key>NSAppTransportSecurity<\/key><dict><key>NSAllowsArbitraryLoadsInWebContent<\/key><true\/><key>NSAllowsLocalNetworking<\/key><true\/><\/dict>/);
  assert.match(flat(infoPlist(app({ ios: { localNetworking: true } }))), /<key>NSAppTransportSecurity<\/key><dict><key>NSAllowsLocalNetworking<\/key><true\/><\/dict>/);
  // Absent or false, on tvOS (no WebKit), and one platform's field never reaches the other's plist.
  assert.doesNotMatch(infoPlist(app({ ios: ats }), false, { tv: true }), /NSAppTransportSecurity/);
  for (const plist of [macInfoPlist(app({})), macInfoPlist(app({ ios: ats })), infoPlist(app({ macos: ats })), macInfoPlist(app({ macos: { appTransportSecurity: { allowsArbitraryLoadsInWebContent: false } } }))]) assert.doesNotMatch(plist, /NSAppTransportSecurity/);
  // A typed field, not a pass-through: any other key fails the manifest.
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ats-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'F', app: { id: 'com.example.f', name: 'F' }, host: { macos: { appTransportSecurity: { NSAllowsArbitraryLoads: true } } } }));
    assert.throws(() => readManifest(dir, 'f'), /host\.macos\.appTransportSecurity/);
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'F', app: { id: 'com.example.f', name: 'F' }, host: { macos: ats, ios: ats } }));
    assert.equal(readManifest(dir, 'f').host.macos.appTransportSecurity.allowsArbitraryLoadsInWebContent, true);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('`allowsArbitraryLoads` turns ATS off on macOS, iOS and tvOS, written alone so ATS does not ignore it', async () => {
  const { readManifest } = await import('./app.mjs');
  const app = (host) => ({ id: 'com.example.fixture', displayName: 'Fixture', manifest: { host } });
  const flat = (plist) => plist.replace(/>\s+</g, '><');
  const off = /<key>NSAppTransportSecurity<\/key><dict><key>NSAllowsArbitraryLoads<\/key><true\/><\/dict>/;
  const reach = { localNetworking: true }; // what a `net.fetch local-network` line derives
  const loads = { allowsArbitraryLoads: true };
  // Alone, beside the web-content key, and beside local networking from the manifest or a grant line: the one key.
  for (const ats of [loads, { ...loads, allowsArbitraryLoadsInWebContent: true }]) {
    for (const plist of [macInfoPlist(app({ macos: { appTransportSecurity: ats } })), macInfoPlist(app({ macos: { appTransportSecurity: ats } }), { reach })]) assert.match(flat(plist), off);
    for (const tv of [false, true]) {
      for (const [ios, built] of [[{ appTransportSecurity: ats }, {}], [{ appTransportSecurity: ats, localNetworking: true }, {}], [{ appTransportSecurity: ats }, { reach }]]) {
        assert.match(flat(infoPlist(app({ ios }), false, { tv, ...built })), off, JSON.stringify({ tv, ios, built }));
      }
    }
  }
  // The dev client's prompt is not ATS's and stays.
  assert.match(infoPlist(app({ ios: { appTransportSecurity: loads, localNetworking: true } })), /NSLocalNetworkUsageDescription/);
  // False is absent, and leaves the other keys as they were.
  assert.doesNotMatch(macInfoPlist(app({ macos: { appTransportSecurity: { allowsArbitraryLoads: false } } })), /NSAppTransportSecurity/);
  assert.match(flat(infoPlist(app({ ios: { appTransportSecurity: { allowsArbitraryLoads: false, allowsArbitraryLoadsInWebContent: true }, localNetworking: true } }))), /<key>NSAppTransportSecurity<\/key><dict><key>NSAllowsArbitraryLoadsInWebContent<\/key><true\/><key>NSAllowsLocalNetworking<\/key><true\/><\/dict>/);
  // One platform's field never reaches the other's plist.
  for (const plist of [macInfoPlist(app({ ios: { appTransportSecurity: loads } })), infoPlist(app({ macos: { appTransportSecurity: loads } }))]) assert.doesNotMatch(plist, /NSAppTransportSecurity/);
  // The manifest takes the key on both platforms, as a boolean.
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ats-'));
  try {
    const save = (value) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'F', app: { id: 'com.example.f', name: 'F' }, host: { macos: { appTransportSecurity: { allowsArbitraryLoads: value } }, ios: { appTransportSecurity: { allowsArbitraryLoads: value } } } }));
    save(true);
    const host = readManifest(dir, 'f').host;
    assert.equal(host.macos.appTransportSecurity.allowsArbitraryLoads, true);
    assert.equal(host.ios.appTransportSecurity.allowsArbitraryLoads, true);
    save('yes');
    assert.throws(() => readManifest(dir, 'f'), /host\.macos\.appTransportSecurity\.allowsArbitraryLoads/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('a manifest `start_url` other than the root bakes `ExactStartURL` on iOS and macOS (W3C App Manifest)', () => {
  const app = (manifest) => ({ id: 'com.example.fixture', displayName: 'Fixture', name: 'fixture', manifest: { host: {}, ...manifest } });
  for (const plist of [infoPlist(app({ start_url: '/today' }), false), macInfoPlist(app({ start_url: '/today' }))]) {
    assert.match(plist, /<key>ExactStartURL<\/key><string>\/today<\/string>/);
  }
  // Resolved against the manifest's URL, as `deploy.mjs` resolves it for the web.
  for (const [start_url, want] of [['today', '/today'], ['./today', '/today'], ['/today?tab=2', '/today?tab=2']]) {
    assert.ok(infoPlist(app({ start_url }), false).includes(`<key>ExactStartURL</key><string>${want}</string>`), start_url);
  }
  for (const start_url of [undefined, '', '/', './', '//elsewhere/today', 'https://example.com/today']) {
    assert.doesNotMatch(infoPlist(app({ start_url }), false), /ExactStartURL/, String(start_url));
  }
  // On the app's declared origin an absolute or scheme-relative URL is a place in the app; on another, none.
  const at = (start_url) => infoPlist({ ...app({ start_url }), origin: 'https://example.com' }, false);
  for (const start_url of ['https://example.com/today', '//example.com/today', '/today']) {
    assert.ok(at(start_url).includes('<key>ExactStartURL</key><string>/today</string>'), start_url);
  }
  for (const start_url of ['https://elsewhere.com/today', 'http://example.com/today', 'https://example.com/']) {
    assert.doesNotMatch(at(start_url), /ExactStartURL/, start_url);
  }
});

test('the launch handler bakes `ExactLaunchMode` with or without documents (LLP 1069.010 D4)', async () => {
  const { readManifest } = await import('./app.mjs');
  const app = (manifest) => ({ id: 'com.example.fixture', displayName: 'Fixture', name: 'fixture', manifest: { host: {}, ...manifest } });
  // An app that opens nothing still gets File ▸ New Window from `navigate-new`.
  const windows = macInfoPlist(app({ launch_handler: { client_mode: 'navigate-new' } }));
  assert.match(windows, /<key>ExactLaunchMode<\/key><string>navigate-new<\/string>/);
  assert.doesNotMatch(windows, /CFBundleDocumentTypes/);
  // With documents, both keys; the W3C list's first mode after `auto` wins.
  const documents = macInfoPlist(app({ file_handlers: [{ action: '/', accept: { 'text/markdown': ['.md'] } }], launch_handler: { client_mode: ['auto', 'navigate-new'] } }));
  assert.match(documents, /<key>CFBundleDocumentTypes<\/key>/);
  assert.match(documents, /<key>ExactLaunchMode<\/key><string>navigate-new<\/string>/);
  // No launch handler: the mode is still written, as the host's default `navigate-existing`; no document types.
  const plain = macInfoPlist(app({}));
  assert.match(plain, /<key>ExactLaunchMode<\/key><string>navigate-existing<\/string>/);
  assert.doesNotMatch(plain, /CFBundleDocumentTypes/);
  // The common document and image types each name their system type (ledger
  // diary F9: CSV); only Markdown, which iOS does not declare, is imported.
  const common = { 'text/plain': 'public.plain-text', 'application/json': 'public.json', 'text/csv': 'public.comma-separated-values-text', 'text/tab-separated-values': 'public.tab-separated-values-text', 'text/markdown': 'net.daringfireball.markdown', 'text/html': 'public.html', 'application/pdf': 'com.adobe.pdf', 'image/png': 'public.png', 'image/jpeg': 'public.jpeg', 'image/gif': 'com.compuserve.gif', 'image/webp': 'org.webmproject.webp', 'image/svg+xml': 'public.svg-image', 'application/zip': 'public.zip-archive' };
  const every = app({ file_handlers: [{ action: '/', accept: Object.fromEntries(Object.keys(common).map(m => [m, []])) }] });
  assert.deepEqual(documentTypes(every)[0].LSItemContentTypes, Object.values(common));
  assert.deepEqual(importedTypes(every).map(t => t.UTTypeIdentifier), ['net.daringfireball.markdown']);
  // An unmapped type is refused when any build reads the manifest, the
  // web's included (files diary F12); IANA's generic binary is mapped.
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-types-'));
  try {
    const write = (accept) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Types', app: { id: 'com.example.types', name: 'Types' }, file_handlers: [{ action: '/', accept }] }));
    write({ 'audio/mpeg': ['.mp3'] });
    assert.throws(() => readManifest(dir, 'types'), /file_handlers\[0\]\.accept: audio\/mpeg names no type the Apple hosts map \(they map .*text\/csv/);
    write({ 'application/octet-stream': ['.bin', '.dat'] });
    assert.deepEqual(documentTypes(app(readManifest(dir, 'types')))[0].LSItemContentTypes, ['public.data']);
    // An app's own format (studio diary R13): a vendor or `x-` type is
    // exported as the app's, its editor and owner, beside the system types
    // it only views; `+json` makes it JSON. It must name its extension.
    write({ 'application/x-studio-board+json': ['.board'], 'application/json': ['.json'] });
    const own = { ...app(readManifest(dir, 'types')), id: 'com.example.types' };
    assert.deepEqual(documentTypes(own).map(t => [t.LSItemContentTypes, t.CFBundleTypeRole, t.LSHandlerRank, t.CFBundleTypeExtensions]),
      [[['com.example.types.studio-board'], 'Editor', 'Owner', ['board']], [['public.json'], 'Viewer', 'Alternate', ['json']]]);
    assert.deepEqual(exportedTypes(own), [{ UTTypeIdentifier: 'com.example.types.studio-board', UTTypeDescription: 'application/x-studio-board+json',
      UTTypeConformsTo: ['public.json', 'public.content'], UTTypeTagSpecification: { 'public.filename-extension': ['board'], 'public.mime-type': ['application/x-studio-board+json'] } }]);
    assert.match(macInfoPlist(own), /<key>UTExportedTypeDeclarations<\/key>/);
    write({ 'application/vnd.example.sheet': [] });
    assert.throws(() => readManifest(dir, 'types'), /application\/vnd\.example\.sheet, the app's own type, names no extension/);
    // So are a launch colour iOS cannot draw and the Apple icon's missing file.
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Types', app: { id: 'com.example.types', name: 'Types' }, background_color: 'white', icons: [{ src: 'icon.png', sizes: '1024x1024' }] }));
    assert.throws(() => readManifest(dir, 'types'), /background_color: "white" does not match/);
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Types', app: { id: 'com.example.types', name: 'Types' }, background_color: '#fff', icons: [{ src: 'icon.png', sizes: '1024x1024' }] }));
    assert.throws(() => readManifest(dir, 'types'), /icons: icon\.png, the square icon the Apple bundles are drawn from, does not exist/);
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Types', app: { id: 'com.example.types', name: 'Types' }, host: { ios: { launchImage: { src: 'splash.png', width: 276 } } } }));
    assert.throws(() => readManifest(dir, 'types'), /host\.ios\.launchImage: splash\.png, the launch screen's picture, does not exist/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('an Apple app records the SDK it is built with unless its manifest keeps the design before 26', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-design-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Design', app: { id: 'com.example.design', name: 'Design' }, host: { macos: { minimumOS: '14.0', designRequiresCompatibility: true }, ios: { designRequiresCompatibility: true } } }));
    const manifest = readManifest(dir, 'design');
    const app = (host) => ({ id: 'com.example.design', manifest: { host } });
    assert.equal(designCompatible({ id: 'com.example.design', manifest }, 'macos'), true, 'the manifest field is read');
    assert.equal(designCompatible({ id: 'com.example.design', manifest }, 'ios'), true);
    assert.equal(designCompatible(app({ macos: { minimumOS: '14.0' } }), 'macos'), false, 'absent: the SDK the app is built with');
    assert.equal(designCompatible(app({ macos: { designRequiresCompatibility: true } }), 'ios'), false, 'per platform');
    assert.deepEqual(COMPATIBLE_SDK, { ios: '18.0', macos: '15.0' }, 'the last SDKs before the 26 design');
    // An app that needs 26 has no earlier design to keep.
    assert.throws(() => designCompatible(app({ macos: { minimumOS: '26.0', designRequiresCompatibility: true } }), 'macos'), /no earlier design/);
    assert.throws(() => designCompatible(app({ ios: { minimumOS: '26.0', designRequiresCompatibility: true } }), 'ios'), /no earlier design/);
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Design', app: { id: 'com.example.design', name: 'Design' }, host: { macos: { designRequiresCompatibility: 'yes' } } }));
    assert.throws(() => readManifest(dir, 'design'), /designRequiresCompatibility/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('simulator signing gives each app and embedded host a distinct Keychain identity', async () => {
  const {entitlements} = await import('../host/apple/build.mjs');
  const app = {id:'com.exact.test',manifest:{host:{ios:{}}}};
  const simulator = {simulator:true};
  const sim = entitlements(app, null, true, null, simulator), host = entitlements({...app,id:app.id+'.host'}, null, true, null, simulator);
  // A Team ID's ten characters before the id (LLP 1069.008.000 D3), the bare id it had kept as a Keychain group.
  assert.match(sim, /<key>application-identifier<\/key><string>SIMULATORX.com.exact.test<\/string>/);
  assert.match(sim, /<key>keychain-access-groups<\/key><array><string>SIMULATORX.com.exact.test<\/string><string>com.exact.test<\/string><\/array>/);
  assert.match(host, /<key>application-identifier<\/key><string>SIMULATORX.com.exact.test.host<\/string>/);
  assert.doesNotMatch(sim, /com.apple.developer.team-identifier/);
  const device = entitlements(app, 'TEAM', false);
  assert.match(device, /TEAM.com.exact.test/);
  assert.doesNotMatch(device, /keychain-access-groups|SIMULATORX/);
  assert.match(device, /<key>get-task-allow<\/key><false\/>/);
});

test('an App Clip is its parent\'s app under a second identity: NSAppClip, the parent identifier and appclips: domains (LLP 1126 D12)', async () => {
  const { appClip, entitlements, infoPlist } = await import('../host/apple/build.mjs');
  const flat = (s) => s.replace(/\s+/g, '');
  const base = { id: 'com.exact.test', displayName: 'Test', manifest: { host: { ios: { associatedDomains: ['applinks:exact.test', 'webcredentials:exact.test'], urlSchemes: ['exacttest'] } } } };
  assert.equal(appClip(base), null);
  const app = { ...base, manifest: { host: { ios: { ...base.manifest.host.ios, appClip: {} } } } };
  assert.deepEqual(appClip(app), { id: 'com.exact.test.Clip', name: 'Test', executable: 'Test' });
  assert.deepEqual(appClip({ ...app, manifest: { host: { ios: { appClip: { id: 'com.exact.clip', name: 'Test: Clip' } } } } }), { id: 'com.exact.clip', name: 'Test: Clip', executable: 'Test- Clip' });
  const clip = appClip(app);
  // The clip's plist: its own id and name, NSAppClip, no URL scheme and no dev link; the parent's keeps both.
  const plist = flat(infoPlist(app, false, { id: clip.id, name: clip.name, executable: clip.executable, clip: true, development: { scheme: 'exact-dev', origins: ['http://localhost:4000'], token: 'a'.repeat(64) } }));
  assert.match(plist, /<key>CFBundleIdentifier<\/key><string>com.exact.test.Clip<\/string>/);
  assert.match(plist, /<key>CFBundleExecutable<\/key><string>Test<\/string>/);
  assert.match(plist, /<key>NSAppClip<\/key><dict><key>NSAppClipRequestEphemeralUserNotification<\/key><false\/><key>NSAppClipRequestLocationConfirmation<\/key><false\/><\/dict>/);
  assert.doesNotMatch(plist, /CFBundleURLTypes|ExactDevelopmentURLScheme/);
  assert.match(flat(infoPlist(app, false, { development: { scheme: 'exact-dev', origins: [], token: '' } })), /<key>CFBundleURLSchemes<\/key><array><string>exacttest<\/string><string>exact-dev<\/string><\/array>/);
  assert.doesNotMatch(infoPlist(app, false), /NSAppClip/);
  // Entitlements: the clip signs its parent's identifier and each applinks: host as appclips:, in place of the
  // applinks:; the parent lists the appclips: beside its applinks:. A simulator build prefixes both as the app's.
  const sim = flat(entitlements({ ...app, id: clip.id, clipParent: app.id }, null, true, null, { simulator: true }));
  assert.match(sim, /<key>application-identifier<\/key><string>SIMULATORX.com.exact.test.Clip<\/string>/);
  assert.match(sim, /<key>com.apple.developer.parent-application-identifiers<\/key><array><string>SIMULATORX.com.exact.test<\/string><\/array>/);
  assert.match(sim, /<key>com.apple.developer.associated-domains<\/key><array><string>webcredentials:exact.test<\/string><string>appclips:exact.test<\/string><\/array>/);
  const device = flat(entitlements({ ...app, id: clip.id, clipParent: app.id }, 'TEAM', true, null, { prefix: 'PREFIX' }));
  assert.match(device, /<key>application-identifier<\/key><string>PREFIX.com.exact.test.Clip<\/string>/);
  assert.match(device, /<key>com.apple.developer.parent-application-identifiers<\/key><array><string>PREFIX.com.exact.test<\/string><\/array>/);
  const parent = flat(entitlements(app, 'TEAM', true));
  assert.match(parent, /<key>com.apple.developer.associated-domains<\/key><array><string>applinks:exact.test<\/string><string>webcredentials:exact.test<\/string><string>appclips:exact.test<\/string><\/array>/);
  assert.doesNotMatch(parent, /parent-application-identifiers/);
  assert.doesNotMatch(flat(entitlements(base, 'TEAM', true)), /appclips:/);
  // The declared origin's applinks: becomes the clip's appclips: too; a tvOS build of the app has neither.
  const origin = { ...app, origin: 'https://exact.test', manifest: { host: { ios: { associatedDomains: true, appClip: {} } } } };
  assert.match(flat(entitlements({ ...origin, id: clip.id, clipParent: app.id }, 'TEAM', true)), /<array><string>appclips:exact.test<\/string><\/array>/);
  assert.doesNotMatch(entitlements({ ...origin, widgetTV: true }, 'TEAM', true), /appclips:/);
  // With widgets, the clip shares the parent's App Group and Keychain group, and takes no push environment.
  const entries = { ...app, manifest: { ...app.manifest, live_activities: [{ kind: 'departure', push: true }] } };
  const grouped = flat(entitlements({ ...entries, id: clip.id, clipParent: app.id }, 'TEAM', true, null, { prefix: 'PREFIX' }));
  assert.match(grouped, /<key>com.apple.security.application-groups<\/key><array><string>group.com.exact.test<\/string><\/array>/);
  assert.match(grouped, /<key>keychain-access-groups<\/key><array><string>PREFIX.com.exact.test<\/string><\/array>/);
  assert.doesNotMatch(grouped, /aps-environment/);
});

test('widgets share the owner Keychain groups and leave the host sample outside the group', async () => {
  const {entitlements, infoPlist} = await import('../host/apple/build.mjs');
  const app = {id:'com.exact.test',displayName:'Test',manifest:{widgets:[{kind:'next-trains',families:['systemSmall']}],host:{ios:{}}}};
  for (const target of [app, {...app,id:app.id+'.widgets',widgetAppId:app.id}]) {
    const ent = entitlements(target,null,true,null,{simulator:true});
    assert.match(ent, /<key>keychain-access-groups<\/key><array><string>SIMULATORX.com.exact.test<\/string><string>com.exact.test<\/string><\/array>/);
    assert.match(ent, /group.com.exact.test/);
  }
  const host = entitlements({...app,id:app.id+'.host',widgetHost:true},null,true,null,{simulator:true});
  assert.doesNotMatch(host, /application-groups/);
  assert.match(host, /SIMULATORX.com.exact.test.host/);
  assert.doesNotMatch(infoPlist(app,false,{id:app.id+'.host',executable:'ExactHostIOS'}), /ExactAppGroup|ExactAppIdentifier|ExactWidgetKinds/);
  assert.match(infoPlist(app,false,{executable:'Test'}), /ExactAppGroup/);
  assert.doesNotMatch(infoPlist(app,false,{executable:'Test'}), /NSSupportsLiveActivities/);
  assert.doesNotMatch(entitlements(app,null,true,null,{simulator:true}), /aps-environment/);
});

test('Live Activities alone gate the group, the Keychain group and the plist keys, and a pushed one signs aps-environment (LLP 1126.001 WP13)', async () => {
  const {entitlements, infoPlist} = await import('../host/apple/build.mjs');
  const {hasEntries} = await import('../host/apple/widgets.mjs');
  const app = {id:'com.exact.test',displayName:'Test',manifest:{live_activities:[{kind:'departure',push:true},{kind:'alert',frequent:true}],host:{ios:{}}}};
  assert.ok(hasEntries(app) && !hasEntries({manifest:{}}) && hasEntries({manifest:{widgets:[{kind:'w',families:['systemSmall']}]}}));
  const plist = infoPlist(app,false,{executable:'Test'});
  assert.match(plist, /<key>ExactWidgetKinds<\/key><array><\/array>/, 'the app is the group store\'s writer with no widget kind');
  assert.match(plist, /<key>NSSupportsLiveActivities<\/key><true\/>/);
  assert.match(plist, /<key>NSSupportsLiveActivitiesFrequentUpdates<\/key><true\/>/);
  assert.doesNotMatch(infoPlist({...app,manifest:{...app.manifest,live_activities:[{kind:'departure'}]}},false,{executable:'Test'}), /FrequentUpdates/);
  assert.doesNotMatch(infoPlist(app,false,{executable:'Test',tv:true}), /NSSupportsLiveActivities|ExactAppGroup/);
  const sim = entitlements(app,null,true,null,{simulator:true});
  assert.match(sim, /group.com.exact.test/);
  assert.match(sim, /<key>aps-environment<\/key><string>development<\/string>/);
  assert.match(entitlements(app,'TEAM',false), /<key>aps-environment<\/key><string>production<\/string>/, 'a distribution profile signs production');
  assert.doesNotMatch(entitlements({...app,id:app.id+'.widgets',widgetAppId:app.id},'TEAM',true), /aps-environment/, 'the extension reads no token');
  assert.doesNotMatch(entitlements({...app,manifest:{...app.manifest,live_activities:[{kind:'departure'}]}},'TEAM',true), /aps-environment/);
});

test('a device build with entries takes the app\'s and the extension\'s profiles that grant the shared groups, or names what each close profile lacked or failed (LLP 1126.001 WP13)', async () => {
  const {entryProfiles} = await import('../host/apple/widgets.mjs');
  const app = {id:'com.exact.test',manifest:{live_activities:[{kind:'departure',push:true}],host:{ios:{}}}};
  const day = 8.64e7;
  // A decoded profile as devices.mjs reads one; the real predicates (`profileMisfit`, `profilesFor`) judge it. The
  // Keychain grant is the one group both sign, `TEAM.com.exact.test`, here without the team wildcard.
  const make = (name, appId, extra = {}) => {
    const p = {path:`/p/${name}`, name, team:'TEAM', prefix:'TEAM', appId, devices:['PHONE'], dev:true, expires:new Date(Date.now()+day), entitlements:['application-identifier','get-task-allow','keychain-access-groups'], groups:[], keychain:['TEAM.com.exact.test','com.apple.token'], aps:null, enterprise:false, ...extra};
    p.method ??= p.dev ? 'development' : p.devices.length ? 'adhoc' : p.enterprise ? 'enterprise' : 'appstore';
    return p;
  };
  const wildcard = make('Team wildcard', 'TEAM.*', {keychain:['TEAM.*','com.apple.token']}), appOnly = make('App', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'development'});
  const extension = make('Extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test']});
  const pair = entryProfiles({app, udid:'PHONE', installed:[wildcard, appOnly, extension], named:null});
  assert.equal(pair.app.name, 'App'); assert.equal(pair.extension.name, 'Extension');
  const refusal = (installed, options = {}) => { try { entryProfiles({app, udid:'PHONE', installed, named:null, ...options}); } catch (e) { return e.message; } assert.fail('refused'); };
  const wildcardOnly = refusal([wildcard]);
  assert.match(wildcardOnly, /com.exact.test: "Team wildcard" covers it but none grants the App Group group.com.exact.test, aps-environment \(development on a development profile/);
  assert.match(wildcardOnly, /com.exact.test.widgets: "Team wildcard" covers it but none grants the App Group group.com.exact.test\n/);
  assert.match(wildcardOnly, /register the App ID com.exact.test with App Groups \(group.com.exact.test\) and Push Notifications, and the App ID com.exact.test.widgets with App Groups \(group.com.exact.test\); then make one development profile for each App ID that includes this phone/);
  assert.match(refusal([appOnly]), /com.exact.test: provisioned\n  com.exact.test.widgets: no profile covers it\nIn the Apple Developer portal, register the App ID com.exact.test.widgets with App Groups \(group.com.exact.test\) and Keychain Sharing \(the group <prefix>.com.exact.test, or the team's <prefix>.\*\); then make one development profile for each App ID that includes this phone/);
  // The grants: the push environment agrees with the method; HealthKit is the app's alone; the Keychain group is
  // the one both sign (the extension's own App ID's group is no grant of it), or the team wildcard.
  assert.match(refusal([make('App', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'production'}), extension]), /"App" covers it but none grants aps-environment/);
  assert.match(refusal([appOnly, extension], {required:['com.apple.developer.healthkit']}), /com.exact.test: "App" covers it but none grants com.apple.developer.healthkit\n  com.exact.test.widgets: provisioned/);
  assert.equal(entryProfiles({app, udid:'PHONE', installed:[appOnly, make('Wildcard extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], keychain:['TEAM.*']})], named:null}).extension.name, 'Wildcard extension');
  const ownGroup = refusal([appOnly, make('Own group', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], keychain:['TEAM.com.exact.test.widgets']})]);
  assert.match(ownGroup, /com.exact.test.widgets: "Own group" covers it but none grants the Keychain group TEAM.com.exact.test\n/);
  assert.match(ownGroup, /register the App ID com.exact.test.widgets with Keychain Sharing \(the group TEAM.com.exact.test, or the team's TEAM.\*\)/);
  // The conditions, each named: expiry, this phone, the team, the prefix, the method (before the phone).
  const expired = refusal([wildcard, make('App', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'development', expires:new Date(Date.now()-day)}), extension]);
  assert.match(expired, /com.exact.test: "Team wildcard" covers it but none grants the App Group group.com.exact.test, aps-environment[\s\S]*?pushes\); "App" is for it but expired on \d{4}-\d\d-\d\d/);
  assert.match(expired, /com.exact.test.widgets: provisioned/);
  assert.match(refusal([make('App', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'development', devices:['OTHER']}), extension]), /"App" is for it but does not include this phone/);
  assert.match(refusal([appOnly, make('Extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], team:'OTHER'})]), /"Extension" is for it but is team OTHER's, not TEAM's/);
  assert.match(refusal([appOnly, make('Extension', 'OLD.com.exact.test.widgets', {groups:['group.com.exact.test'], prefix:'OLD', keychain:['OLD.*']})]), /"Extension" is for it but has the App ID prefix OLD, not TEAM/);
  assert.match(refusal([appOnly, make('Store extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], dev:false, devices:[]})]), /"Store extension" is for it but is App Store, not development/);
  assert.match(refusal([make('Ad hoc app', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'production', dev:false, devices:['OTHER']}), extension]), /"Ad hoc app" is for it but is Ad Hoc, not development/);
  // EXACT_PROFILE is the app's, under the same conditions: on a phone, a development profile for this app.
  assert.equal(entryProfiles({app, udid:'PHONE', installed:[wildcard, extension], named:appOnly}).app, appOnly);
  assert.match(refusal([extension], {named:{...appOnly, expires:new Date(Date.now()-day)}}), /com.exact.test: EXACT_PROFILE \(App\) is for it but expired on/);
  assert.match(refusal([extension], {named:make('Other', 'TEAM.com.other', {groups:['group.com.exact.test'], aps:'development'})}), /EXACT_PROFILE \(Other\) is for it but is for TEAM.com.other, not com.exact.test/);
  assert.match(refusal([extension], {named:make('Store', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'production', dev:false, devices:[]})}), /EXACT_PROFILE \(Store\) is for it but is App Store, not development/);
  const adHoc = make('Ad hoc app', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'production', dev:false, devices:['A','B','PHONE']});
  const adHocExtension = make('Ad hoc extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], dev:false, devices:['A','B','PHONE']});
  const namedAdHoc = refusal([extension, adHocExtension], {named:adHoc});
  assert.match(namedAdHoc, /com.exact.test: EXACT_PROFILE \(Ad hoc app\) is for it but is Ad Hoc, not development\n  com.exact.test.widgets: provisioned\nMake one development profile for each App ID that includes this phone/, 'the development extension profile fits a phone build; the named Ad Hoc one does not');
  // An archive (no phone): distribution profiles, the extension's of the named one's method, an Ad Hoc pair of
  // its devices; a development pair is refused, and an unfit named profile keeps the extension on distribution.
  assert.equal(entryProfiles({app, udid:undefined, installed:[extension, adHocExtension], named:adHoc}).extension, adHocExtension);
  const archive = refusal([extension, make('Store extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], dev:false, devices:[]}), {...adHocExtension, devices:['A']}], {udid:undefined, named:adHoc});
  assert.match(archive, /embeds one Ad Hoc profile in the app and one in its extension[^\n]*On this Mac, for an archive:\n  com.exact.test: provisioned\n  com.exact.test.widgets: "Extension" is for it but is development, not Ad Hoc; "Store extension" is for it but is App Store, not Ad Hoc; "Ad hoc extension" is for it but does not include 2 of the 3 devices the app's profile names\nMake one Ad Hoc profile for each App ID naming the same devices/);
  const developmentArchive = refusal([extension], {udid:undefined, named:appOnly});
  assert.match(developmentArchive, /embeds one distribution profile[^\n]*for an archive:\n  com.exact.test: EXACT_PROFILE \(App\) is for it but is development; an archive takes a distribution profile\n  com.exact.test.widgets: "Extension" is for it but is development; an archive takes a distribution profile\nMake one distribution profile for each App ID; install both/);
  const store = make('Store app', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'production', dev:false, devices:[]});
  assert.equal(entryProfiles({app, udid:undefined, installed:[adHocExtension, make('Store extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], dev:false, devices:[]})], named:store}).extension.name, 'Store extension');
  const enterprise = make('Enterprise app', 'TEAM.com.exact.test', {groups:['group.com.exact.test'], aps:'production', dev:false, devices:[], enterprise:true});
  assert.equal(enterprise.method, 'enterprise');
  assert.equal(entryProfiles({app, udid:undefined, installed:[make('Store extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], dev:false, devices:[]}), make('Enterprise extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], dev:false, devices:[], enterprise:true})], named:enterprise}).extension.name, 'Enterprise extension');
  assert.match(refusal([make('Store extension', 'TEAM.com.exact.test.widgets', {groups:['group.com.exact.test'], dev:false, devices:[]})], {udid:undefined, named:enterprise}), /"Store extension" is for it but is App Store, not enterprise/);
});

test('a device build takes the App Clip\'s profile for its own App ID, of the app\'s profile\'s team, prefix and method, granting the parent identifier, or names what each profile for it lacked', async () => {
  const { embeddedProfile } = await import('../host/apple/devices.mjs');
  const day = 8.64e7, parent = 'com.apple.developer.parent-application-identifiers';
  const make = (name, appId, extra = {}) => {
    const p = { path: `/p/${name}`, name, team: 'TEAM', prefix: 'TEAM', appId, devices: ['PHONE'], dev: true, expires: new Date(Date.now() + day), entitlements: ['application-identifier', 'get-task-allow'], groups: [], keychain: [], aps: null, enterprise: false, ...extra };
    p.method ??= p.dev ? 'development' : p.devices.length ? 'adhoc' : p.enterprise ? 'enterprise' : 'appstore';
    return p;
  };
  const app = make('App', 'TEAM.com.exact.test'), wildcard = make('Team wildcard', 'TEAM.*');
  const clip = make('Clip', 'TEAM.com.exact.test.Clip', { entitlements: [parent] });
  assert.equal(embeddedProfile('PHONE', 'com.exact.test.Clip', app, [parent], [wildcard, clip]), clip);
  const longer = make('Clip later', 'TEAM.com.exact.test.Clip', { entitlements: [parent], expires: new Date(Date.now() + 2 * day) });
  assert.equal(embeddedProfile('PHONE', 'com.exact.test.Clip', app, [parent], [clip, longer]), longer, 'the longest-lived of the fits');
  const refusal = (installed, like = app, udid = 'PHONE') => { try { embeddedProfile(udid, 'com.exact.test.Clip', like, [parent], installed); } catch (e) { return e.message; } assert.fail('refused'); };
  assert.match(refusal([]), /no development profile on this Mac covers com.exact.test.Clip as the app's \(App\) does: none is for it\. In the Apple Developer portal, register the App ID com.exact.test.Clip with com.apple.developer.parent-application-identifiers, make a development profile for it of team TEAM that includes this phone, and install it\./);
  // A team wildcard never grants the App Clip capability; the conditions the extension's profile meets, each named.
  assert.match(refusal([wildcard]), /"Team wildcard" does not allow com.apple.developer.parent-application-identifiers/);
  assert.match(refusal([make('Other team', 'OTHER.com.exact.test.Clip', { team: 'OTHER', prefix: 'OTHER', entitlements: [parent] })]), /"Other team" is team OTHER's, not TEAM's/);
  assert.match(refusal([make('Elsewhere', 'TEAM.com.exact.test.Clip', { devices: ['OTHER'], entitlements: [parent] })]), /"Elsewhere" does not include this phone/);
  assert.match(refusal([make('Store clip', 'TEAM.com.exact.test.Clip', { dev: false, devices: [], entitlements: [parent] })]), /"Store clip" is App Store, not development/);
  // An archive: the clip's profile names every device the app's does.
  const adHoc = make('Ad hoc app', 'TEAM.com.exact.test', { dev: false, devices: ['A', 'B'] });
  assert.equal(embeddedProfile(null, 'com.exact.test.Clip', adHoc, [parent], [make('Ad hoc clip', 'TEAM.com.exact.test.Clip', { dev: false, devices: ['A', 'B'], entitlements: [parent] })]).name, 'Ad hoc clip');
  assert.match(refusal([make('Ad hoc clip', 'TEAM.com.exact.test.Clip', { dev: false, devices: ['A'], entitlements: [parent] })], adHoc, null), /"Ad hoc clip" does not include 1 of the 2 devices the app's profile names/);
});

test('a HealthKit grant signs with its entitlement, tvOS drops it, and a device build takes only a profile that allows it', async () => {
  // LLP 1069.008.000 D2, D4, D5.
  const {entitlements, tvReach} = await import('../host/apple/build.mjs');
  const {allows} = await import('../host/apple/devices.mjs');
  const app = {id:'com.exact.test',manifest:{host:{ios:{}}}};
  const health = 'com.apple.developer.healthkit';
  const reach = {base:'en', locales:['en'], entitlements:[health], tvOmits:['NSHealthShareUsageDescription','NSHealthUpdateUsageDescription',health],
    usage:{NSHealthShareUsageDescription:{en:'Reads.'}, NSHealthUpdateUsageDescription:{en:'Reads.'}, NSMicrophoneUsageDescription:{en:'Hears.'}}};
  for (const plist of [entitlements(app, null, true, reach, {simulator:true}), entitlements(app, 'TEAM', true, reach)]) assert.match(plist, /<key>com.apple.developer.healthkit<\/key><true\/>/);
  const tv = tvReach(reach);
  assert.deepEqual(tv.entitlements, []);
  assert.deepEqual(Object.keys(tv.usage), ['NSMicrophoneUsageDescription']);
  assert.doesNotMatch(entitlements(app, null, true, tv, {simulator:true}), /healthkit/);
  assert.equal(tvReach({usage:{A:{en:'x'}}}).usage.A.en, 'x', 'nothing to omit leaves reach alone');
  const wildcard = {entitlements:['application-identifier','get-task-allow','keychain-access-groups']}, explicit = {entitlements:[...wildcard.entitlements, health]};
  assert.ok(allows(wildcard, []) && allows(explicit, [health]));
  assert.ok(!allows(wildcard, [health]) && !allows(null, []));
});

test('Swift under modules/apple is the app module even when it has no views', async () => {
  const { mkdtempSync, mkdirSync, writeFileSync, rmSync, realpathSync } = await import('node:fs');
  const { resolve } = await import('node:path');
  const { tmpdir } = await import('node:os');
  const { resolveApp } = await import('./app.mjs');
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-module-no-views-'))), saved = process.env.EXACT_APP_DIR;
  try {
    mkdirSync(resolve(dir, 'modules/apple'), { recursive: true });
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    text "calls"\n');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Calls', app: { id: 'com.exact.calls', name: 'Calls' }, rust: false, deploy: { store: { web: '0' } } }));
    writeFileSync(resolve(dir, 'modules/apple/Calls.swift'), '');
    process.env.EXACT_APP_DIR = dir;
    const modules = resolveApp().modules;
    assert.deepEqual(modules.tags, []);
    assert.deepEqual(modules.apple, [resolve(dir, 'modules/apple/Calls.swift')], 'native.call and native.later need the module with no views');
  } finally {
    if (saved === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = saved;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('widget reload artifact is cached by source and target triple', async () => {
  const { widgetReload } = await import('../host/apple/widgets.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-widget-reload-')), source = resolve(dir, 'reload.swift');
  try {
    writeFileSync(source, 'first');
    let builds = 0;
    const options = { directory: dir, source, triple: 'arm64-apple-ios17.0-simulator', sdk: '/sdk', runApple: (_, args) => { builds++; writeFileSync(args.at(-1), 'binary'); } };
    widgetReload(options); widgetReload(options); assert.equal(builds, 1);
    writeFileSync(source, 'second'); widgetReload(options); assert.equal(builds, 2);
    widgetReload({ ...options, triple: 'arm64-apple-ios17.0' }); assert.equal(builds, 3);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('watch entries keep accessory families and omit TypeScript pending the Hermes gate', async () => {
  const { platformWidgetEntries } = await import('../host/apple/widgets.mjs');
  const { watchEntries } = await import('../host/apple/watch.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-watch-entry-'));
  try {
    const app = { dir, manifest: { widgets: [
      { kind: 'home', families: ['systemSmall'] },
      { kind: 'train', refresh: 'nextTrains', families: ['systemSmall', 'accessoryCircular', 'accessoryRectangular', 'accessoryInline', 'accessoryCorner'] },
    ] } };
    assert.deepEqual(platformWidgetEntries(app).map(e => e.families), [['systemSmall'], ['systemSmall', 'accessoryCircular', 'accessoryRectangular', 'accessoryInline']]);
    assert.deepEqual(watchEntries(app).map(e => [e.kind, e.families]), [['train', ['accessoryCircular', 'accessoryRectangular', 'accessoryInline', 'accessoryCorner']]]);
    writeFileSync(resolve(dir, 'app.ts'), '');
    assert.deepEqual(watchEntries(app), []);
    assert.deepEqual(watchEntries(app), [], 'mixed apps remain omitted until the watch Hermes gate');
    const caltrain = { dir, manifest: { widgets: [{kind:'train', families:['systemSmall', 'accessoryRectangular'], watchFamilies:['accessoryInline','accessoryCircular','accessoryCorner']}] } };
    assert.deepEqual(platformWidgetEntries(caltrain).map(e => e.families), [['systemSmall','accessoryRectangular']]);
    assert.deepEqual(platformWidgetEntries(caltrain, 'watchos').map(e => e.families), [['accessoryInline','accessoryCircular','accessoryCorner']]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('Mac widgets omit accessory-only entries and require a provisioned pair for shared secrets (LLP 1126.001 WP16)', async () => {
  const { platformWidgetEntries, macWidgetGroup, macWidgetProfiles, macWidgetEntitlements } = await import('../host/apple/widgets.mjs');
  const app = { id: 'com.exact.test', manifest: { widgets: [
    { kind: 'home', families: ['systemSmall', 'systemMedium', 'systemLarge', 'systemExtraLarge', 'accessoryRectangular'] },
    { kind: 'lock', families: ['accessoryInline'] },
  ] } };
  assert.deepEqual(platformWidgetEntries(app, 'macos').map(e => e.families), [['systemSmall', 'systemMedium', 'systemLarge', 'systemExtraLarge']]);
  assert.deepEqual(platformWidgetEntries({ manifest: { widgets: [app.manifest.widgets[1]], live_activities: [{ kind: 'activity' }] } }, 'macos'), []);
  assert.equal(macWidgetGroup(app, 'TEAM'), 'TEAM.com.exact.test.widgets');
  const make = (id, extra = {}) => ({ appId: `PREFIX.${id}`, team: 'TEAM', prefix: 'PREFIX', platforms: ['OSX'],
    expires: new Date(Date.now() + 86400000), method: 'development', dev: true, devices: ['MAC'],
    groups: ['TEAM.*'], keychain: ['PREFIX.com.exact.test'], ...extra });
  const owner = make(app.id), extension = make(`${app.id}.widgets`);
  const choose = installed => macWidgetProfiles({ app, team: 'TEAM', installed, named: null, udid: 'MAC' });
  assert.equal(choose([]), null);
  const profiles = choose([owner, extension]);
  assert.equal(profiles.app, owner); assert.equal(profiles.extension, extension);
  for (const changed of [{ platforms: ['iOS'] }, { team: 'OTHER' }, { keychain: [] }, { groups: [] }, { devices: ['OTHER'] }, { expires: new Date(0) }, { method: 'appstore' }, { prefix: 'OTHER', appId: `OTHER.${app.id}.widgets` }]) {
    assert.equal(choose([owner, { ...extension, ...changed }]), null, JSON.stringify(changed));
  }
  assert.throws(() => macWidgetProfiles({ app, team: 'TEAM', installed: [], named: owner }), /matching macOS profile/);
  const plain = macWidgetEntitlements({ app, team: 'TEAM' });
  assert.match(plain, /TEAM.com.exact.test.widgets/);
  assert.doesNotMatch(plain, /keychain-access-groups|application-identifier|app-sandbox|network.client|aps-environment/);
  const sandbox = macWidgetEntitlements({ app, team: 'TEAM', extension: true });
  assert.match(sandbox, /com.apple.security.app-sandbox<\/key><true\/>/);
  assert.match(sandbox, /com.apple.security.network.client<\/key><true\/>/);
  assert.doesNotMatch(sandbox, /keychain-access-groups|application-identifier/);
  for (const isExtension of [false, true]) {
    const signed = macWidgetEntitlements({ app, team: 'TEAM', profiles, extension: isExtension });
    assert.match(signed, /<key>keychain-access-groups<\/key><array><string>PREFIX.com.exact.test<\/string><\/array>/);
    assert.match(signed, new RegExp(`<key>com.apple.application-identifier</key><string>PREFIX.com.exact.test${isExtension ? '.widgets' : ''}</string>`));
  }
});


test('corner-only entries give iOS no widget group or publishing kinds', () => {
  const app = {id:'com.exact.corner',displayName:'Corner',manifest:{widgets:[{kind:'corner',families:['accessoryCorner']}],host:{ios:{}}}};
  assert.doesNotMatch(infoPlist(app,false), /ExactAppGroup|ExactWidgetKinds|ExactAppIdentifier/);
  assert.doesNotMatch(entitlements(app,null,true,null,{simulator:true}), /application-groups/);
  const activity = {...app, manifest:{...app.manifest,live_activities:[{kind:'departure'}]}};
  assert.match(entitlements(activity,null,true,null,{simulator:true}), /application-groups/);
  const plist = infoPlist(activity,false);
  assert.match(plist, /<key>ExactWidgetKinds<\/key><array><\/array>/);
  assert.match(plist, /<key>NSSupportsLiveActivities<\/key><true\/>/);
});

test('--watch-sim requires an id before the parent build can consume flags', async () => {
  const { watchSimulator } = await import('../host/apple/watch.mjs');
  for (const args of [['--watch-sim'], ['--watch-sim','--release'], ['--watch-sim','s1126-wp17']]) assert.throws(() => watchSimulator(args), /requires a UDID/);
  const id = '114F2B20-7CA4-46F3-A1AA-22784759F58D';
  assert.equal(watchSimulator(['--watch-sim',id,'--release']), id);
});

test('watch owns the build before the parent starts and reports a skipped TypeScript run', async () => {
  const { buildWatch } = await import('../host/apple/watch.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-watch-lock-')), logs = [], events = [], log = console.log;
  try {
    writeFileSync(resolve(dir, 'app.ts'), '');
    const app = {dir, id:'com.exact.watch-test', displayName:'Watch Test', manifest:{widgets:[{kind:'question',refresh:'question',families:['accessoryInline']}]} };
    console.log = message => logs.push(message);
    await buildWatch(app, ['--watchos', '--run'], {
      appleBuildLock: () => { events.push('lock'); return () => events.push('release'); },
      buildParent: async args => { events.push('parent'); assert.ok(args.includes('--no-install')); },
      appleArtifacts: () => ({bundle:resolve(dir,'Parent.app')}),
    });
    assert.deepEqual(events, ['lock','parent','release']);
    assert.ok(logs.some(line => line.includes('omits entry question')));
    assert.ok(logs.some(line => line.includes('--run skipped')));
  } finally { console.log = log; rmSync(dir, {recursive:true,force:true}); }
});


test('a refused parent prevents use of a stale companion bundle', async () => {
  const { buildWatch } = await import('../host/apple/watch.mjs');
  const before = process.exitCode ?? 0, events = [];
  try {
    await assert.rejects(buildWatch({id:'fixture'}, ['--watchos'], {
      appleBuildLock: () => { events.push('lock'); return () => events.push('release'); },
      buildParent: async () => { events.push('parent'); process.exitCode = 1; },
      appleArtifacts: () => { throw new Error('stale artifact accessed'); },
    }), /iOS companion build refused/);
    assert.deepEqual(events, ['lock','parent','release']);
  } finally { process.exitCode = before; }
});


test('Mac signing preflight falls back without a development identity and reads hardware only for Mac profiles', async () => {
  const { macWidgetSigning, widgetSigningTeam, macHardwareID, macWidgetProfiles } = await import('../host/apple/widgets.mjs');
  const { profile } = await import('../host/apple/devices.mjs');
  const app = { id: 'com.exact.test', manifest: { widgets: [{ kind: 'public', families: ['systemSmall'] }] } };
  const messages = [];
  assert.equal(macWidgetSigning(app, null, { log: text => messages.push(text) }), null);
  assert.equal(messages.length, 1); assert.match(messages[0], /EXACT_IDENTITY/);
  const certificates = [{ subject: 'CN=Apple Development: Fixture\nOU=0123456789', fingerprint: 'AA' },
    { subject: 'CN=Developer ID Application: Fixture\nOU=0123456789', fingerprint: 'BB' }];
  assert.equal(widgetSigningTeam('AA', { certificates }), '0123456789');
  assert.equal(widgetSigningTeam('Apple Development: Fixture', { certificates }), '0123456789');
  assert.equal(macWidgetSigning(app, 'BB', { certificates, log: text => messages.push(text) }), null);
  assert.match(messages.at(-1), /Developer ID.*ad hoc.*EXACT_IDENTITY.*Apple Development/);
  assert.equal(macWidgetSigning(app, null, { distribution: true, log: text => messages.push(text) }), null);
  assert.match(messages.at(-1), /EXACT_IDENTITY.*Developer ID Application/);
  assert.doesNotMatch(messages.at(-1), /Apple Development/);
  assert.throws(() => macWidgetSigning(app, 'AA', { certificates, distribution: true }), /Developer ID Application.*EXACT_IDENTITY names Apple Development/);
  assert.equal(widgetSigningTeam('BB', { certificates, distribution: true }), '0123456789');
  // Captured SPHardwareDataType property list; hardware and provisioning ids redacted separately.
  const capture = readFileSync(new URL('../host/apple/tests/fixtures/mac-hardware.json', import.meta.url), 'utf8');
  assert.equal(macHardwareID(command => { assert.equal(command, 'system_profiler'); return capture; }), '00006000-0011223344556677');
  assert.equal(macHardwareID(command => command === 'system_profiler' ? '{}' : '"IOPlatformUUID" = "HARDWARE"'), 'HARDWARE');
  const base = { name: 'Mac distribution', team: '0123456789', prefix: 'PREFIX', platforms: ['OSX'], expires: new Date(Date.now() + 86400000),
    method: 'enterprise', dev: false, devices: [], groups: ['0123456789.*'], keychain: ['PREFIX.com.exact.test'], entitlements: [] };
  const installed = [app.id, app.id + '.widgets'].map(id => ({ ...base, appId: 'PREFIX.' + id }));
  const noHardware = () => { throw new Error('hardware query without a relevant profile'); };
  assert.ok(macWidgetProfiles({ app, team: base.team, installed, named: null, distribution: true, hardwareID: noHardware }));
  const ios = { ...base, name: 'iPhone development', platforms: ['iOS'] };
  for (const installed of [[], [ios], [{ ...base, team: 'OTHER' }]]) {
    assert.equal(macWidgetProfiles({ app, team: base.team, installed, named: ios, hardwareID: noHardware, log: text => messages.push(text) }), null);
    assert.match(messages.at(-1), /ignoring EXACT_PROFILE \(iPhone development\).*iOS.*§5 Q5/);
  }
  let reads = 0;
  const development = installed.map(p => ({ ...p, dev: true, method: 'development', devices: ['MAC'] }));
  assert.ok(macWidgetProfiles({ app, team: base.team, installed: development, named: ios, hardwareID: () => { ++reads; return 'MAC'; }, log: text => messages.push(text) }));
  assert.equal(reads, 1);
  assert.equal(macWidgetProfiles({ app, team: base.team, installed, named: null, udid: 'MAC' }), null);
  assert.throws(() => profile('PHONE', app.id, [], { installed, named: installed[0] }), /EXACT_PROFILE \(Mac distribution\) is not an iOS profile/);
});

test('Mac versioned runtime publication serves both bare products (LLP 1126.001 WP16)', async () => {
  if (process.platform !== 'darwin') return;
  const { wrapFramework } = await import('../host/apple/assets.mjs');
  const { publishMacFramework } = await import('../host/apple/build.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-mac-widget-runtime-'));
  const run = (command, args) => { const r = spawnSync(command, args, { encoding: 'utf8' }); assert.equal(r.status, 0, r.stderr); return r.stdout; };
  try {
    const frameworks = resolve(dir, 'built'), namespace = resolve(dir, 'products'); mkdirSync(frameworks);
    writeFileSync(resolve(dir, 'core.c'), 'int exact_value(void) { return 42; }');
    writeFileSync(resolve(dir, 'app.c'), 'extern int exact_value(void); int main(void) { return exact_value() == 42 ? 0 : 1; }');
    run('xcrun', ['clang', '-dynamiclib', resolve(dir, 'core.c'), '-o', resolve(frameworks, 'libcore.dylib')]);
    wrapFramework(frameworks, 'libcore.dylib', 'ExactCore', { id: 'com.exact.test', manifest: {} }, 'MacOSX');
    publishMacFramework(resolve(frameworks, 'ExactCore.framework'), namespace, '-');
    for (const product of ['standalone', 'host']) {
      const path = resolve(namespace, product, 'Fixture'); mkdirSync(resolve(namespace, product));
      run('xcrun', ['clang', resolve(dir, 'app.c'), '-F', frameworks, '-framework', 'ExactCore', '-Wl,-rpath,@executable_path/../Frameworks', '-o', path]);
      assert.match(run('otool', ['-L', path]), /ExactCore.framework\/Versions\/A\/ExactCore/);
      run(path, []);
    }
    run('codesign', ['--verify', '--strict', resolve(namespace, 'Frameworks/ExactCore.framework')]);
    assert.equal(realpathSync(resolve(namespace, 'Frameworks/ExactCore.framework/ExactCore')), realpathSync(resolve(namespace, 'Frameworks/ExactCore.framework/Versions/A/ExactCore')));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});


test('Mac shared-core rebuild replaces an existing versioned XCFramework slice', async () => {
  if (process.platform !== 'darwin') return;
  const { sharedWidgetCore } = await import('../host/apple/widgets.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-core-recopy-'));
  const run = (command, args) => { const r = spawnSync(command, args, { encoding: 'utf8' }); assert.equal(r.status, 0, r.stderr); return r; };
  try {
    const bin = resolve(dir, 'bin'), base = resolve(dir, 'core'); mkdirSync(bin);
    const prepared = { base, args: [], platform: 'macos', framework: resolve(base, 'ExactCore.framework'), xcframework: resolve(base, 'ExactCore.xcframework') };
    const source = resolve(dir, 'core.c'), archive = resolve(dir, 'libfixture.a');
    const runApple = (command, args) => {
      if (command !== 'swift') return run(command, args);
      if (args.includes('--show-bin-path')) return { stdout: bin };
      if (args.includes('ExactCore')) run('xcrun', ['clang', '-dynamiclib', source, '-o', resolve(bin, 'libExactCore.dylib'), ...args.flatMap((arg,i)=>arg==='-Xlinker'?['-Xlinker',args[i+1]]:[])]);
      return { stdout: '' };
    };
    const slice = resolve(prepared.xcframework, `macos-${process.arch === 'arm64' ? 'arm64' : 'x86_64'}/ExactCore.framework`);
    for (const version of [1, 2]) {
      writeFileSync(source, `int exact_value(void) { return ${version}; }`);
      writeFileSync(archive, `archive ${version}`);
      for (const module of ['ExactCore', 'ExactSnapshot']) {
        const path = resolve(bin, module + '.swiftmodule'); mkdirSync(path, { recursive: true });
        writeFileSync(resolve(path, 'fixture'), `module ${version}`);
      }
      sharedWidgetCore({ app: { id: 'com.exact.test', manifest: {} }, prepared, libDir: dir, crate: 'fixture', env: process.env, runApple });
      assert.equal(realpathSync(resolve(slice, 'ExactCore')), realpathSync(resolve(slice, 'Versions/A/ExactCore')));
      assert.equal(realpathSync(resolve(slice, 'Resources')), realpathSync(resolve(slice, 'Versions/A/Resources')));
      assert.equal(readFileSync(resolve(slice, 'Modules/ExactCore.swiftmodule/fixture'), 'utf8'), `module ${version}`);
      assert.deepEqual(readFileSync(resolve(slice, 'ExactCore')), readFileSync(resolve(prepared.framework, 'ExactCore')));
      writeFileSync(resolve(dir, 'app.c'), `extern int exact_value(void); int main(void) { return exact_value() == ${version} ? 0 : 1; }`);
      const executable = resolve(dir, `Fixture${version}`), frameworks = resolve(slice, '..');
      run('xcrun', ['clang', resolve(dir, 'app.c'), '-F', frameworks, '-framework', 'ExactCore', '-Wl,-rpath,' + frameworks, '-o', executable]);
      run(executable, []);
    }
    // A plan edit relinks the shared image even when its Rust archive stays.
    const planDir=resolve(dir,'plan'); mkdirSync(planDir);
    writeFileSync(resolve(planDir,'compat.json'),'{}');
    writeFileSync(resolve(planDir,'app.hbc'),'');
    writeFileSync(source, `#include <mach-o/getsect.h>
      extern const struct mach_header_64 __dso_handle;
      const unsigned char *exact_plan_bytes(unsigned long *size) {
        return getsectiondata(&__dso_handle,"__TEXT","__exact_plan",size);
      }`);
    for (const text of ['first plan','second plan']) {
      writeFileSync(resolve(planDir,'app.plan'),text);
      sharedWidgetCore({app:{id:'com.exact.test',manifest:{}},prepared,libDir:dir,crate:'fixture',env:process.env,runApple,linkedPlan:{dir:planDir,digest:text}});
      const executable=resolve(dir,'ReadPlan');
      writeFileSync(resolve(dir,'read.c'), `#include <string.h>
        extern const unsigned char *exact_plan_bytes(unsigned long *size);
        int main(void) { unsigned long size=0; const unsigned char *bytes=exact_plan_bytes(&size);
          return bytes && size==${text.length} && !memcmp(bytes,"${text}",size) ? 0 : 1; }`);
      run('xcrun',['clang',resolve(dir,'read.c'),'-F',resolve(slice,'..'),'-framework','ExactCore','-Wl,-rpath,'+resolve(slice,'..'),'-o',executable]);
      run(executable,[]);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
