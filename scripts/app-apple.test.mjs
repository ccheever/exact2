// What an Apple build writes and checks (host/apple/build.mjs): Hermes bundles, asset catalogs, plists and entitlements, the
// SDK, and signing.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { delimiter, resolve, toNamespacedPath } from 'node:path';
import { HERMES_INSTALLER, hermesBundle, hermesLeanSysRoots } from './app.mjs';
import { checkHermesBundles, hermesCrossTargets, hermesSetupTargets } from './exact.mjs';
import { iosAssets, infoPlist, macInfoPlist, documentTypes, importedTypes, exportedTypes, writeUsageStrings, designCompatible, COMPATIBLE_SDK, moduleTripleFor } from '../host/apple/build.mjs';
import { useXcode } from '../host/apple/devices.mjs';
import { macReleaseEntitlements, launchImageInventory } from '../host/apple/assets.mjs';

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
    assert.equal(keys.UILaunchScreen.UIColorName, 'ExactLaunch');
    assert.equal(keys.UILaunchScreen.UIImageName, 'ExactLaunchImage', 'the picture the launch screen centres on its colour');
    const result = spawnSync('xcrun', ['assetutil', '--info', resolve(bundle, 'Assets.car')], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const assets = JSON.parse(result.stdout);
    assert.ok(assets.some(asset => asset.Name === 'AppIcon'), 'Assets.car must retain AppIcon');
    assert.equal(assets.filter(asset => asset.Name === 'ExactLaunch').length, 2);
    const launch = assets.filter(asset => asset.Name === 'ExactLaunchImage');
    assert.ok(launch.length >= 1, 'Assets.car holds the launch image');
    assert.ok(launch.every(asset => asset.PixelWidth === 40 * asset.Scale), `40 points wide at each scale: ${JSON.stringify(launch)}`);
    // The launch image is a binary input: packaging draws the bytes the bake
    // recorded, and refuses a picture replaced since, or one the receipt lacks.
    const baked = launchImageInventory(app);
    assert.equal(baked.sha256.length, 64);
    const again = resolve(dir, 'Again.app'); mkdirSync(again);
    assert.equal(iosAssets(app, again, true, { expectedLaunchImage: baked }).UILaunchScreen.UIImageName, 'ExactLaunchImage');
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

test('widgets share the owner Keychain groups and leave the host sample outside the group', async () => {
  const {entitlements, infoPlist} = await import('../host/apple/build.mjs');
  const app = {id:'com.exact.test',displayName:'Test',manifest:{widgets:[{kind:'next-trains'}],host:{ios:{}}}};
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
