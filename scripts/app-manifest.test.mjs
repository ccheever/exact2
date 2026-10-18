// app.json keys the web manifest standard defines (scripts/app.schema.json).
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

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
