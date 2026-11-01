// app.json keys the web manifest standard defines (scripts/app.schema.json).
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

test('each painted host admits ink, carries it in native metadata and refuses gpu', async () => {
  const { readManifest, nativeManifestMetadata } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-painter-'));
  const base = { name: 'Painter', app: { id: 'dev.exact.painter', name: 'Painter' } };
  const read = host => {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...base, host }));
    return readManifest(dir, 'painter');
  };
  try {
    for (const platform of ['windows', 'linux', 'android']) {
      const manifest = read({ [platform]: { painter: 'ink' } });
      assert.equal(nativeManifestMetadata(manifest, platform).host.painter, 'ink');
      for (const other of ['windows', 'linux', 'android'].filter(p => p !== platform)) {
        assert.equal(nativeManifestMetadata(manifest, other).host.painter, undefined);
      }
      assert.equal(nativeManifestMetadata(read({ [platform]: {} }), platform).host.painter, undefined);
      assert.throws(() => read({ [platform]: { painter: 'gpu' } }), /painter/);
      if (platform === 'windows') {
        assert.equal(nativeManifestMetadata(read({ windows: { painter: 'cpu' } }), platform).host.painter, 'cpu');
      } else {
        assert.throws(() => read({ [platform]: { painter: 'cpu' } }), /painter/);
      }
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('readManifest takes the web manifest\'s description', async () => {
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-description-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Tips', app: { id: 'dev.exact.tips', name: 'Tips' }, description: 'Splits a bill and its tip.' }));
    assert.equal(readManifest(dir, 'tips').description, 'Splits a bill and its tip.');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('protocol handlers require one in-app placeholder and distinct schemes', async () => {
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-protocol-'));
  const base = { name: 'Links', app: { id: 'dev.exact.links', name: 'Links' } };
  const save = protocol_handlers => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...base, protocol_handlers }));
  try {
    save([{ protocol: 'web+links', url: '/open?u=%s' }]);
    assert.deepEqual(readManifest(dir, 'links').protocol_handlers, [{ protocol: 'web+links', url: '/open?u=%s' }]);
    for (const distribution_only of [true, false]) {
      save([{ protocol: 'web+links', url: '/open?u=%s', distribution_only }]);
      assert.equal(readManifest(dir, 'links').protocol_handlers[0].distribution_only, distribution_only);
    }
    for (const bad of [
      [{ protocol: 'web+links', url: '/open?u=%s', distribution_only: 'true' }],
      [{ protocol: 'web+links', url: '/open' }],
      [{ protocol: 'web+links', url: '/open?u=%s&again=%s' }],
      [{ protocol: 'web+links', url: '//other/open?u=%s' }],
      [{ protocol: 'web+links', url: '/open?u=%s#fragment' }],
      [{ protocol: 'web+links', url: '/open?u=%s' }, { protocol: 'WEB+LINKS', url: '/other?u=%s' }],
    ]) {
      save(bad);
      assert.throws(() => readManifest(dir, 'links'), /protocol_handlers/);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('Apple bundles register declared protocol schemes and keep their templates', async () => {
  const { macInfoPlist, infoPlist } = await import('../host/apple/build.mjs');
  const app = { id: 'dev.exact.links', displayName: 'Links', manifest: {
    protocol_handlers: [{ protocol: 'web+links', url: '/open?u=%s' }],
    host: { macos: { urlSchemes: ['other'] }, ios: { urlSchemes: ['other'] } },
  } };
  for (const plist of [macInfoPlist(app), infoPlist(app)]) {
    assert.match(plist, /<key>ExactProtocolHandlers<\/key>/);
    assert.match(plist, /<string>web\+links<\/string>/);
    assert.match(plist, /<string>other<\/string>/);
    assert.match(plist, /<string>\/open\?u=%s<\/string>/);
  }
});

test('distribution-only handlers stay out of both Apple registration keys in local builds', async () => {
  const { macInfoPlist, infoPlist } = await import('../host/apple/build.mjs');
  const handler = { protocol: 'packaged', url: '/packaged?u=%s', distribution_only: true };
  const app = { id: 'dev.exact.links', displayName: 'Links', manifest: {
    protocol_handlers: [handler, { protocol: 'ordinary', url: '/ordinary?u=%s', distribution_only: false }],
  } };
  const development = { scheme: 'exact-dev-test', origins: ['http://localhost:8080'], token: 'abc' };
  for (const generate of [options => macInfoPlist(app, options), options => infoPlist(app, true, options)]) {
    const local = generate({ development });
    assert.doesNotMatch(local, /packaged/);
    assert.match(local, /<string>ordinary<\/string>/);
    assert.match(local, /<string>exact-dev-test<\/string>/);
    const shipped = generate({ distributed: true });
    const compact = shipped.replace(/\s+/g, '');
    assert.match(compact, /<key>CFBundleURLSchemes<\/key><array><string>packaged<\/string><string>ordinary<\/string><\/array>/);
    assert.match(compact, /<key>ExactProtocolHandlers<\/key><array><dict><key>protocol<\/key><string>packaged<\/string><key>url<\/key><string>\/packaged\?u=%s<\/string><\/dict>/);
    // Session.swift reads [[String: String]], so a build-only Boolean cannot reach this dictionary.
    assert.doesNotMatch(shipped, /distribution_only/);
  }
  const only = { ...app, manifest: { protocol_handlers: [handler] } };
  for (const local of [macInfoPlist(only), infoPlist(only)]) assert.doesNotMatch(local, /CFBundleURLTypes|ExactProtocolHandlers/);
  assert.match(infoPlist(app, true, { distribution: { ExactBuildKind: 'release' } }), /<string>release<\/string>/);
  assert.doesNotMatch(infoPlist(app, true, { distribution: { ExactBuildKind: 'release' } }), /packaged/);
});

test('http and https come as a pair: a Mac web browser, refused on iOS and dropped from the web manifest (LLP 1030 D2)', async () => {
  const { macInfoPlist, infoPlist } = await import('../host/apple/build.mjs');
  const { readManifest, webAppManifest } = await import('./app.mjs');
  const flat = (plist) => plist.replace(/\s+/g, '');
  const pair = (extra = {}) => [{ protocol: 'https', url: '/open?u=%s', ...extra }, { protocol: 'http', url: '/open?u=%s', ...extra }];
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-web-'));
  const save = (protocol_handlers) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Links', app: { id: 'dev.exact.links', name: 'Links' }, protocol_handlers }));
  try {
    save([...pair(), { protocol: 'links', url: '/open?u=%s' }]);
    assert.equal(readManifest(dir, 'links').protocol_handlers.length, 3);
    for (const bad of [[pair()[0]], [pair()[0], { protocol: 'HTTP', url: '/other?u=%s' }], [pair()[0], { ...pair()[1], distribution_only: true }]]) {
      save(bad);
      assert.throws(() => readManifest(dir, 'links'), /http and https come together/);
    }
    for (const platform of ['ios', 'macos']) {
      writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Links', app: { id: 'dev.exact.links', name: 'Links' }, host: { [platform]: { urlSchemes: ['links', 'HTTPS'] } } }));
      assert.throws(() => readManifest(dir, 'links'), new RegExp(`host\\.${platform}\\.urlSchemes: HTTPS is the web's own scheme; declare http and https together in protocol_handlers`));
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }

  const app = { id: 'dev.exact.links', displayName: 'Links', manifest: { protocol_handlers: [...pair(), { protocol: 'links', url: '/l?u=%s' }], host: { macos: { urlSchemes: ['other'] } } } };
  const mac = flat(macInfoPlist(app));
  assert.match(mac, /<key>CFBundleURLTypes<\/key><array><dict><key>CFBundleURLName<\/key><string>dev.exact.links<\/string><key>CFBundleURLSchemes<\/key><array><string>other<\/string><string>links<\/string><\/array><\/dict><dict><key>CFBundleURLName<\/key><string>WebsiteURL<\/string><key>CFBundleTypeRole<\/key><string>Viewer<\/string><key>CFBundleURLSchemes<\/key><array><string>https<\/string><string>http<\/string><\/array><\/dict><\/array>/);
  assert.match(mac, /<key>CFBundleDocumentTypes<\/key><array><dict><key>CFBundleTypeName<\/key><string>Webpage<\/string><key>CFBundleTypeRole<\/key><string>Viewer<\/string><key>LSHandlerRank<\/key><string>Alternate<\/string><key>LSItemContentTypes<\/key><array><string>public.html<\/string><string>public.xhtml<\/string><\/array><key>ExactWebPage<\/key><true\/><\/dict><\/array>/);
  assert.match(mac, /<key>protocol<\/key><string>https<\/string><key>url<\/key><string>\/open\?u=%s<\/string>/);
  // Beside the app's own documents, after them.
  const documents = flat(macInfoPlist({ ...app, manifest: { ...app.manifest, file_handlers: [{ action: '/', accept: { 'text/markdown': ['.md'] } }] } }));
  assert.match(documents, /net.daringfireball.markdown<\/string><\/array>.*<string>Webpage<\/string>/);
  // iOS registers neither, nor their template; the app's own scheme stays.
  const ios = infoPlist(app);
  assert.doesNotMatch(ios, /<string>https?<\/string>|Web site URL|Web page/);
  assert.match(ios, /<string>links<\/string>/);
  // The iOS and tvOS builds say so, but only when the pair would have been registered.
  const { webRefusal } = await import('../host/apple/documents.mjs');
  assert.match(webRefusal(app, false, false), /^host\/apple: protocol_handlers http and https: iOS registers neither/);
  assert.match(webRefusal(app, false, true), /tvOS registers neither/);
  assert.equal(webRefusal({ ...app, manifest: { protocol_handlers: [{ protocol: 'links', url: '/l?u=%s' }] } }, true, false), null);
  // A distribution-only pair: a local Mac build is no browser, and a local iOS build refuses nothing.
  const packaged = { ...app, manifest: { protocol_handlers: pair({ distribution_only: true }) } };
  assert.doesNotMatch(macInfoPlist(packaged), /https|Web page|CFBundleDocumentTypes/);
  assert.match(macInfoPlist(packaged, { distributed: true }), /Web page/);
  assert.equal(webRefusal(packaged, false, false), null);
  assert.match(webRefusal(packaged, true, false), /iOS registers neither/);
  // The web registers neither (HTML's safelist): the pair goes, the rest stays.
  assert.deepEqual(webAppManifest(app.manifest, 'Links').protocol_handlers, [{ protocol: 'links', url: '/l?u=%s' }]);
  assert.equal(webAppManifest({ protocol_handlers: pair() }, 'Links').protocol_handlers, undefined);
});

test('an iOS app lists the schemes canOpenURL may ask about, and a malformed or 51st one is refused', async () => {
  const { infoPlist } = await import('../host/apple/build.mjs');
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const plist = infoPlist({ id: 'dev.exact.maps', displayName: 'Maps', manifest: { host: { ios: { queriesSchemes: ['comgooglemaps', 'waze'] } } } });
  assert.match(plist.replace(/\s+/g, ''), /<key>LSApplicationQueriesSchemes<\/key><array><string>comgooglemaps<\/string><string>waze<\/string><\/array>/);
  assert.doesNotMatch(infoPlist({ id: 'dev.exact.maps', displayName: 'Maps', manifest: {} }), /LSApplicationQueriesSchemes/);
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-queries-'));
  try {
    const save = (queriesSchemes) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Maps', app: { id: 'dev.exact.maps', name: 'Maps' }, host: { ios: { queriesSchemes } } }));
    save(['comgooglemaps', 'waze']);
    assert.deepEqual(readManifest(dir, 'maps').host.ios.queriesSchemes, ['comgooglemaps', 'waze']);
    for (const [schemes, says] of [[['waze://'], /does not match/], [Array.from({ length: 51 }, (_, i) => `s${i}`), /more than 50 items/]]) {
      save(schemes);
      assert.throws(() => readManifest(dir, 'maps'), says);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('readManifest admits the app\'s design, `tamber` (LLP 1121 D2)', async () => {
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'tamber-manifest-'));
  try {
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    text "a"\n');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'D', app: { id: 'com.example.d', name: 'D' }, tamber: 'design/app.tamber' }));
    assert.equal(readManifest(dir, 'd').tamber, 'design/app.tamber');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('manifest colours follow the web and retired launch and alias keys are refused', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-manifest-'));
  const manifest = { name: 'Manifest', app: { id: 'test.manifest', name: 'Manifest' }, background_color: '#fff', background_color_dark: '#123456' };
  const read = value => { writeFileSync(resolve(dir, 'app.json'), JSON.stringify(value)); return readManifest(dir, 'fixture'); };
  try {
    assert.equal(read(manifest).background_color_dark, '#123456');
    assert.throws(() => read({ ...manifest, launch: { background: '#fff' } }), /launch/);
    assert.throws(() => read({ ...manifest, typescript: { aliases: { '@/*': './*' } } }), /aliases/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('readManifest names the game.presentation rename instead of an unknown key', async () => {
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'render-rename-manifest-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), { recursive: true });
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ game: { presentation: { crate: 'island-presentation', type: 'Hooks' } } }));
    assert.throws(() => readManifest(dir, 'island'), /game\.presentation is now game\.render/);
    assert.throws(() => readManifest(dir, 'island'), (e) => !/not a known key/.test(e.message));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('the manifest orientation locks the iPhone, and an unknown one is refused', async () => {
  const { infoPlist } = await import('../host/apple/build.mjs');
  const { phoneOrientations } = await import('../host/apple/devices.mjs');
  const plist = (orientation) => infoPlist({ id: 'dev.exact.o', displayName: 'O', manifest: orientation ? { orientation } : {} }).replace(/\s+/g, '');
  assert.match(plist('portrait'), /<key>UISupportedInterfaceOrientations<\/key><array><string>UIInterfaceOrientationPortrait<\/string><\/array>/);
  // iPad keeps all four, as its multitasking requires, whatever the iPhone is locked to
  assert.match(plist('portrait'), /<key>UISupportedInterfaceOrientations~ipad<\/key><array>(<string>UIInterfaceOrientation[A-Za-z]+<\/string>){4}<\/array>/);
  assert.match(plist('landscape-primary'), /<key>UISupportedInterfaceOrientations<\/key><array><string>UIInterfaceOrientationLandscapeRight<\/string><\/array>/);
  for (const free of [undefined, 'any']) assert.doesNotMatch(plist(free), /UISupportedInterfaceOrientations</);
  assert.deepEqual(phoneOrientations('landscape'), ['UIInterfaceOrientationLandscapeLeft', 'UIInterfaceOrientationLandscapeRight']);
  assert.deepEqual(phoneOrientations('portrait-secondary'), ['UIInterfaceOrientationPortrait']);
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-orientation-'));
  try {
    const save = (orientation) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'O', app: { id: 'dev.exact.o', name: 'O' }, orientation }));
    save('portrait');
    assert.equal(readManifest(dir, 'o').orientation, 'portrait');
    save('upright');
    assert.throws(() => readManifest(dir, 'o'), /is not one of/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('document handlers and TypeScript source mounts accept their manifest shapes and reject guessed ones', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-author-shapes-'));
  const base = { name: 'Reader', app: { id: 'dev.exact.reader', name: 'Reader' } };
  const read = extra => { writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...base, ...extra })); return readManifest(dir, 'reader'); };
  const file_handlers = [{ action: '/', accept: { 'text/markdown': ['.md'] } }];
  const sources = { backend: '../generated', snapback4: '../runtime' };
  try {
    const manifest = read({ file_handlers, typescript: { sources } });
    assert.deepEqual(manifest.file_handlers, file_handlers);
    assert.deepEqual(manifest.typescript.sources, sources);
    assert.throws(() => read({ file_handlers: { 'text/markdown': ['.md'] } }), /file_handlers:.*array/);
    assert.throws(() => read({ file_handlers: [{ action: '/', accept: ['text/markdown'] }] }), /file_handlers\[0\].accept:.*object/);
    assert.throws(() => read({ typescript: { sources: ['../generated'] } }), /typescript.sources:.*object/);
    assert.throws(() => read({ typescript: { sources: { backend: { path: '../generated' } } } }), /typescript.sources.backend:.*string/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});


test('watchFamilies admits only the four accessory families', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-watch-families-'));
  try {
    const write = watchFamilies => writeFileSync(resolve(dir,'app.json'), JSON.stringify({name:'Watch', app:{id:'com.exact.watch', name:'Watch'}, widgets:[{kind:'train',families:['systemSmall'],watchFamilies,component:'Card',props:'Props',refresh:'refresh',preview:{}}]}));
    const families = ['accessoryCircular','accessoryRectangular','accessoryInline','accessoryCorner'];
    write(families); assert.deepEqual(readManifest(dir,'watch').widgets[0].watchFamilies, families);
    for (const family of ['systemSmall','systemMedium','systemLarge','systemExtraLarge']) {
      write([family]); assert.throws(() => readManifest(dir,'watch'), /watchFamilies/);
    }
  } finally { rmSync(dir,{recursive:true,force:true}); }
});

test('activity labels reject whitespace and control characters in schema validation', async () => {
  const {validate} = await import('./app.mjs');
  const schema = await Bun.file(new URL('./app.schema.json', import.meta.url)).json();
  const activity = schema.properties.live_activities.items;
  for (const key of ['title', 'description']) {
    const label = activity.properties[key];
    assert.deepEqual(validate('Next train 🚆', label, key, schema), []);
    for (const blank of ['', ' ', '   ', '\u00a0', '\u2003']) {
      assert.ok(validate(blank, label, key, schema).length, `${key}: blank label`);
    }
    for (const code of [...Array.from({length:32}, (_, i) => i), ...Array.from({length:33}, (_, i) => i + 127), 0x2028, 0x2029]) {
      for (const value of [`bad${String.fromCodePoint(code)}label`, `label${String.fromCodePoint(code)}`]) {
        assert.ok(validate(value, label, key, schema).length, `${key}: ${code}`);
      }
    }
  }
});


test('requiresSecrets scopes the account placeholder to its widget entry', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-widget-account-'));
  try {
    const widget = { kind: 'card', families: ['systemSmall'], component: 'Card', props: 'Props', refresh: 'refresh', preview: {} };
    const write = requiresSecrets => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Account', app: { id: 'com.exact.account', name: 'Account' }, widgets: [{ ...widget, requiresSecrets }, { ...widget, kind: 'public' }] }));
    write(true); assert.equal(readManifest(dir, 'account').widgets[0].requiresSecrets, true);
    assert.equal(readManifest(dir, 'account').widgets[1].requiresSecrets, undefined);
    write('yes'); assert.throws(() => readManifest(dir, 'account'), /requiresSecrets/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('host.ios.appClip is an object of an id and a name, and an unknown key or a malformed id is refused', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-clip-'));
  try {
    const save = (appClip) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Shop', app: { id: 'dev.exact.shop', name: 'Shop' }, host: { ios: { appClip } } }));
    save({});
    assert.deepEqual(readManifest(dir, 'shop').host.ios.appClip, {});
    save({ id: 'dev.exact.shop.clip', name: 'Shop Clip' });
    assert.deepEqual(readManifest(dir, 'shop').host.ios.appClip, { id: 'dev.exact.shop.clip', name: 'Shop Clip' });
    for (const [appClip, says] of [[{ domains: ['shop.example'] }, /appClip\.domains: not a known key/], [{ id: 'clip' }, /appClip\.id: "clip" does not match/], [true, /appClip/]]) {
      save(appClip);
      assert.throws(() => readManifest(dir, 'shop'), says);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('display_override is the web manifest\'s, written into manifest.json and baked for the Mac', async () => {
  const { readManifest, webAppManifest } = await import('./app.mjs');
  const { macInfoPlist } = await import('../host/apple/build.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-display-override-'));
  const save = display_override => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Row', app: { id: 'dev.exact.row', name: 'Row' }, display: 'standalone', display_override }));
  try {
    save(['window-controls-overlay', 'standalone']);
    const manifest = readManifest(dir, 'row');
    assert.deepEqual(manifest.display_override, ['window-controls-overlay', 'standalone']);
    assert.deepEqual(webAppManifest(manifest, 'Row').display_override, ['window-controls-overlay', 'standalone']);
    const plist = macInfoPlist({ id: 'dev.exact.row', displayName: 'Row', manifest });
    assert.match(plist, /<key>ExactDisplayOverride<\/key>\s*<array>\s*<string>window-controls-overlay<\/string>/);
    save(['borderless']);
    assert.throws(() => readManifest(dir, 'row'), /display_override/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('open_url declares the schemes openURL may hand to the system, and refuses the ones the hosts keep', async () => {
  const { macInfoPlist, infoPlist } = await import('../host/apple/build.mjs');
  const { readManifest } = await import('./app.mjs');
  const app = { id: 'dev.exact.editors', displayName: 'Editors', manifest: { open_url: { schemes: ['vscode', 'zed', 'jetbrains'] } } };
  for (const plist of [macInfoPlist(app), infoPlist(app)]) {
    assert.match(plist.replace(/\s+/g, ''), /<key>ExactOpenURLSchemes<\/key><array><string>vscode<\/string><string>zed<\/string><string>jetbrains<\/string><\/array>/);
  }
  assert.doesNotMatch(infoPlist({ id: 'dev.exact.editors', displayName: 'Editors', manifest: {} }), /ExactOpenURLSchemes/);
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-open-url-'));
  try {
    const save = (open_url) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Editors', app: { id: 'dev.exact.editors', name: 'Editors' }, open_url }));
    save({ schemes: ['vscode-insiders', 'cursor', 'sms'] });
    assert.deepEqual(readManifest(dir, 'editors').open_url.schemes, ['vscode-insiders', 'cursor', 'sms']);
    for (const [open_url, says] of [[{ schemes: ['file'] }, /does not match/], [{ schemes: ['javascript'] }, /does not match/], [{ schemes: ['https'] }, /does not match/], [{ schemes: ['vbscript'] }, /does not match/], [{ schemes: ['view-source'] }, /does not match/], [{ schemes: ['intent'] }, /does not match/],
      [{ schemes: ['vscode://'] }, /does not match/], [{ schemes: [] }, /fewer than 1/], [{ apps: ['vscode'] }, /not a known key/]]) {
      save(open_url);
      assert.throws(() => readManifest(dir, 'editors'), says);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('foreground update intervals are positive seconds and reach both Apple bundles', async () => {
  const { readManifest } = await import('./app.mjs');
  const { macInfoPlist, infoPlist } = await import('../host/apple/build.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-checks-'));
  const manifest = { name: 'Checks', app: { id: 'test.checks', name: 'Checks' }, deploy: { activate: 'app-decides', check: { interval: 3 } } };
  const save = () => writeFileSync(resolve(dir, 'app.json'), JSON.stringify(manifest));
  try {
    save();
    assert.equal(readManifest(dir, 'checks').deploy.check.interval, 3);
    const app = { id: 'test.checks', displayName: 'Checks', manifest };
    for (const plist of [macInfoPlist(app), infoPlist(app)]) {
      assert.match(plist, /<key>ExactUpdateCheckInterval<\/key>\s*<integer>3<\/integer>/);
      assert.match(plist, /<key>ExactUpdateActivate<\/key>\s*<string>app-decides<\/string>/);
    }
    for (const interval of [0, -1, '3']) {
      manifest.deploy.check.interval = interval;
      save();
      assert.throws(() => readManifest(dir, 'checks'), /interval/);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('shortcuts reach manifest.json without the native bakes\' words, and a remote icon is refused', async () => {
  const { readManifest, webAppManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-shortcuts-'));
  const save = (shortcuts) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Shelf', app: { id: 'dev.exact.shelf', name: 'Shelf' }, shortcuts }));
  try {
    save([{ name: 'Add Book', url: '/add', apple: { symbol: 'plus' }, android: { rank: 0 } }]);
    const manifest = readManifest(dir, 'shelf');
    assert.deepEqual(manifest.shortcuts[0].apple, { symbol: 'plus' });
    assert.deepEqual(webAppManifest(manifest, 'Shelf').shortcuts, [{ name: 'Add Book', url: '/add' }]);
    assert.equal(webAppManifest({ shortcuts: [] }, 'Shelf').shortcuts, undefined);
    for (const bad of [{ name: 'Add', url: 'https://example.com/add' }, { name: '', url: '/add' }, { name: 'Add', url: '/add', icons: [{ src: 'https://example.com/i.png' }] }]) {
      save([bad]);
      assert.throws(() => readManifest(dir, 'shelf'), /shortcuts\[0\]/);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('iOS browser opt-in requires a web-scheme pair and preserves ordinary apps', async () => {
  const { readManifest } = await import('./app.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-ios-browser-'));
  const base = { name: 'Browser', app: { id: 'dev.exact.browser', name: 'Browser' } };
  const pair = ['http', 'https'].map(protocol => ({ protocol, url: '/open?u=%s' }));
  const save = (value) => writeFileSync(resolve(dir, 'app.json'), JSON.stringify(value));
  try {
    for (const flag of [true, false]) {
      save({ ...base, host: { ios: { webBrowser: flag } }, protocol_handlers: pair });
      assert.equal(readManifest(dir, 'browser').host.ios.webBrowser, flag);
    }
    save({ ...base, host: { ios: { webBrowser: true } } });
    assert.throws(() => readManifest(dir, 'browser'), /host.ios.webBrowser: declare http and https/);
    save({ ...base, host: { ios: { webBrowser: 'true' } }, protocol_handlers: pair });
    assert.throws(() => readManifest(dir, 'browser'), /host.ios.webBrowser: expected boolean, got string/);
    save({ ...base, host: { ios: { webBrowser: false } } });
    assert.equal(readManifest(dir, 'browser').host.ios.webBrowser, false);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('Windows Ink builds require fxc from PATH or the SDK; CPU builds do not', async () => {
  const { prepareShaders } = await import('../host/windows/build.mjs');
  const { mkdirSync } = await import('node:fs');
  const { delimiter } = await import('node:path');
  const dir = mkdtempSync(resolve(tmpdir(), 'windows-fxc-'));
  const env = () => ({ PATH: '', 'ProgramFiles(x86)': dir, EXACT_PAINTER: 'cpu' });
  try {
    for (const painter of [undefined, 'ink']) {
      assert.throws(() => prepareShaders({ host: { windows: { painter } } }, env()), /install the Windows SDK, or select host.windows.painter: "cpu"/);
    }
    assert.doesNotThrow(() => prepareShaders({ host: { windows: { painter: 'cpu' } } }, env()));
    const path = resolve(dir, 'tools'); mkdirSync(path); writeFileSync(resolve(path, 'fxc.exe'), '');
    const onPath = { ...env(), PATH: path }; prepareShaders({}, onPath);
    assert.equal(onPath.PATH, path);
    for (const version of ['10.0.9.0', '10.0.10.0']) {
      const sdk = resolve(dir, 'Windows Kits', '10', 'bin', version, 'x64');
      mkdirSync(sdk, { recursive: true }); writeFileSync(resolve(sdk, 'fxc.exe'), '');
    }
    const sdk = env(); prepareShaders({}, sdk);
    assert.equal(sdk.PATH.split(delimiter)[0], resolve(dir, 'Windows Kits', '10', 'bin', '10.0.10.0', 'x64'));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('source identities match Cargo roots for Windows canonical drive and UNC paths', async () => {
  const { sourcePath } = await import('./cargo-dep-info.mjs');
  const { win32 } = await import('node:path');
  for (const root of [String.raw`C:\app\packages`, String.raw`\\server\share\packages`]) {
    const path = win32.join(root, 'dock', 'windows', 'lib.rs');
    const canonical = win32.resolve(win32.toNamespacedPath(path));
    assert.equal(sourcePath(canonical, 'win32'), path);
    assert.equal(win32.relative(root, sourcePath(canonical, 'win32')), String.raw`dock\windows\lib.rs`);
    assert.equal(sourcePath(path, 'win32'), path);
    assert.equal(sourcePath(canonical, 'darwin'), canonical);
  }
});
