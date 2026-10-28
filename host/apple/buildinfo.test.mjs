// The build stamp (`buildinfo.mjs`): `bun test host/apple/buildinfo.test.mjs`;
// the async lane runs it (`tests/it/development.rs`).
import { test, expect } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { buildInfo, commitOf, releaseNotes, NOTES_MAX } from './buildinfo.mjs';
import { nativeManifestMetadata, pendingBuildInputs } from '../../scripts/app.mjs';
import { iosPrivacyManifest, writeIOSPrivacyManifests, plistFile } from './assets.mjs';

const root = resolve(import.meta.dir, '../..');
const now = new Date('2026-10-08T10:00:00Z');

test('iOS privacy declarations follow their binaries and changed source invalidates the bake', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-privacy-'));
  try {
    const src = 'App.xcprivacy', path = resolve(dir, src), bundle = resolve(dir, 'Test.app');
    const app = { dir, manifest: { host: { ios: { privacyManifest: src } } } };
    const declared = { NSPrivacyTracking: true, NSPrivacyTrackingDomains: ['example.test'], NSPrivacyCollectedDataTypes: [], NSPrivacyAccessedAPITypes: [{ NSPrivacyAccessedAPIType: 'NSPrivacyAccessedAPICategoryFileTimestamp', NSPrivacyAccessedAPITypeReasons: ['C617.1', 'DDA9.1'] }] };
    writeFileSync(path, plistFile(declared));
    const inventory = iosPrivacyManifest(app);
    expect(inventory.configuration).toEqual(declared);
    const build = { binary: { metadata: {}, inputs: [{ path, name: src, sha256: inventory.sha256 }], missing: [], directories: [] } };
    expect(pendingBuildInputs(build)).toEqual([]);
    for (const name of ['ExactCore', 'ExactModules', 'ExactMIDI', 'ExactWeb']) mkdirSync(resolve(bundle, `Frameworks/${name}.framework`), { recursive: true });
    writeIOSPrivacyManifests(bundle, inventory);
    const read = directory => JSON.parse(spawnSync('plutil', ['-convert', 'json', '-o', '-', resolve(directory, 'PrivacyInfo.xcprivacy')], { encoding: 'utf8' }).stdout);
    const combined = read(bundle), core = read(resolve(bundle, 'Frameworks/ExactCore.framework'));
    expect(combined.NSPrivacyTrackingDomains).toEqual(['example.test']);
    expect(combined.NSPrivacyAccessedAPITypes.find(item => item.NSPrivacyAccessedAPIType.endsWith('FileTimestamp')).NSPrivacyAccessedAPITypeReasons).toEqual(['C617.1', '3B52.1', 'DDA9.1']);
    expect(core.NSPrivacyTracking).toBe(false);
    expect(core.NSPrivacyAccessedAPITypes.find(item => item.NSPrivacyAccessedAPIType.endsWith('ActiveKeyboards')).NSPrivacyAccessedAPITypeReasons).toEqual(['54BD.1']);
    for (const name of ['ExactMIDI', 'ExactWeb']) expect(read(resolve(bundle, `Frameworks/${name}.framework`))).toEqual({ NSPrivacyTracking: false, NSPrivacyTrackingDomains: [], NSPrivacyCollectedDataTypes: [], NSPrivacyAccessedAPITypes: [{ NSPrivacyAccessedAPIType: 'NSPrivacyAccessedAPICategorySystemBootTime', NSPrivacyAccessedAPITypeReasons: ['35F9.1'] }] });
    expect(core.NSPrivacyAccessedAPITypes.map(item => item.NSPrivacyAccessedAPIType)).not.toContain('NSPrivacyAccessedAPICategoryDiskSpace');
    expect(read(resolve(bundle, 'Frameworks/ExactModules.framework'))).toEqual(declared);
    const widget = resolve(bundle, 'PlugIns/Widgets.appex');
    mkdirSync(widget, { recursive: true });
    writeIOSPrivacyManifests(widget);
    expect(read(widget)).toEqual(core);
    writeFileSync(path, plistFile({ ...declared, NSPrivacyTracking: false }));
    expect(iosPrivacyManifest(app).sha256).not.toBe(inventory.sha256);
    expect(pendingBuildInputs(build)).toEqual([src]);
    for (const invalid of [[], { NSPrivacyTracking: 'false' }, { NSPrivacyAccessedAPITypes: [{ NSPrivacyAccessedAPIType: 'missing reasons' }] }, { NSPrivacyCollectedDataTypes: [{}] }]) {
      writeFileSync(path, plistFile(invalid));
      expect(() => iosPrivacyManifest(app)).toThrow(/host.ios.privacyManifest/);
    }
    symlinkSync(resolve(root, 'package.json'), resolve(dir, 'Outside.xcprivacy'));
    expect(() => iosPrivacyManifest({ ...app, manifest: { host: { ios: { privacyManifest: 'Outside.xcprivacy' } } } })).toThrow(/inside the app/);
    expect(iosPrivacyManifest({ dir, manifest: {} })).toBeNull();
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('iOS privacy manifests reject unknown identifiers and reasons for another category', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-privacy-invalid-'));
  try {
    const src = 'App.xcprivacy', path = resolve(dir, src);
    const app = { dir, manifest: { host: { ios: { privacyManifest: src } } } };
    const api = { NSPrivacyAccessedAPIType: 'NSPrivacyAccessedAPICategoryDiskSpace', NSPrivacyAccessedAPITypeReasons: ['E174.1'] };
    const data = { NSPrivacyCollectedDataType: 'NSPrivacyCollectedDataTypeEmailAddress', NSPrivacyCollectedDataTypeLinked: true, NSPrivacyCollectedDataTypeTracking: false, NSPrivacyCollectedDataTypePurposes: ['NSPrivacyCollectedDataTypePurposeAppFunctionality'] };
    const declared = { NSPrivacyAccessedAPITypes: [api], NSPrivacyCollectedDataTypes: [data] };
    writeFileSync(path, plistFile(declared));
    expect(iosPrivacyManifest(app).configuration).toEqual(declared);
    const cases = [
      [{ NSPrivacyAccessedAPITypes: [{ ...api, NSPrivacyAccessedAPIType: '' }] }, /NSPrivacyAccessedAPITypes\[0\].NSPrivacyAccessedAPIType.*unknown category/],
      [{ NSPrivacyAccessedAPITypes: [{ ...api, NSPrivacyAccessedAPIType: 'NSPrivacyAccessedAPICategoryDiskSpaec' }] }, /unknown category.*DiskSpaec/],
      [{ NSPrivacyAccessedAPITypes: [{ ...api, NSPrivacyAccessedAPITypeReasons: ['CA92.1'] }] }, /CA92\.1.*not approved for.*DiskSpace/],
      [{ NSPrivacyAccessedAPITypes: [{ ...api, NSPrivacyAccessedAPITypeReasons: ['E174.1', '0000.1'] }] }, /0000\.1.*not approved for.*DiskSpace/],
      [{ NSPrivacyCollectedDataTypes: [{ ...data, NSPrivacyCollectedDataType: '' }] }, /NSPrivacyCollectedDataTypes\[0\].NSPrivacyCollectedDataType.*unknown data type/],
      [{ NSPrivacyCollectedDataTypes: [{ ...data, NSPrivacyCollectedDataType: 'NSPrivacyCollectedDataTypeEmailAdress' }] }, /unknown data type.*EmailAdress/],
      [{ NSPrivacyCollectedDataTypes: [{ ...data, NSPrivacyCollectedDataTypePurposes: ['AppFunctionality'] }] }, /NSPrivacyCollectedDataTypePurposes.*unknown purpose.*AppFunctionality/],
      [{ NSPrivacyCollectedDataTypes: [{ ...data, NSPrivacyCollectedDataTypePurposes: [] }] }, /NSPrivacyCollectedDataTypePurposes.*nonempty array/],
    ];
    for (const [invalid, diagnostic] of cases) {
      writeFileSync(path, plistFile({ ...declared, ...invalid }));
      expect(() => iosPrivacyManifest(app)).toThrow(diagnostic);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('native binary identity includes protocol handler registration and templates', () => {
  const manifest = { app: { id: 'com.exact.links' }, protocol_handlers: [{ protocol: 'exacttest', url: '/open?u=%s' }] };
  const first = nativeManifestMetadata(manifest, 'macos');
  expect(first.protocol_handlers).toEqual(manifest.protocol_handlers);
  for (const platform of ['macos', 'ios']) expect(nativeManifestMetadata({ ...manifest, protocol_handlers: [{ ...manifest.protocol_handlers[0], distribution_only: true }] }, platform)).not.toEqual(nativeManifestMetadata(manifest, platform));
  expect(nativeManifestMetadata({ ...manifest, protocol_handlers: [{ protocol: 'exacttest', url: '/callback?u=%s' }] }, 'macos')).not.toEqual(first);
});

test('the orientation lock is part of an iPhone and Android binary\'s identity, not a Mac\'s', () => {
  const manifest = { app: { id: 'com.exact.turns' } };
  for (const platform of ['ios', 'android']) {
    expect(nativeManifestMetadata({ ...manifest, orientation: 'portrait' }, platform)).not.toEqual(nativeManifestMetadata(manifest, platform));
  }
  expect(nativeManifestMetadata({ ...manifest, orientation: 'portrait' }, 'macos')).toEqual(nativeManifestMetadata(manifest, 'macos'));
});

test('an app inside exact2 carries exact2\'s commit alone, and the kind', () => {
  const info = buildInfo({ dir: resolve(root, 'apps/caltrain') }, { root, now });
  expect(info.ExactBuildTime).toBe('2026-10-08T10:00:00.000Z');
  expect(info.ExactBuildKind).toBe('debug');
  expect(info.ExactCommit).toMatch(/^[0-9a-f]{40}$/);
  expect(typeof info.ExactCommitDirty).toBe('boolean');
  expect(info.ExactAppCommit).toBeUndefined();
  expect(info.ExactAppBranch).toBeUndefined();
  expect(info.ExactBuildHost).toBeTruthy();
  expect(buildInfo({ dir: root }, { root, now, sdk: 'iphoneos', sdkVersion: '18.0' }).ExactBuildXcode).toMatch(/iphoneos 18\.0$/);
  expect(info.ExactReleaseNotes).toBeUndefined();
  expect(buildInfo({ dir: root }, { root, now, env: { EXACT_DISTRIBUTION_REVISION: '7' } }).ExactDistributionRevision).toBe('7');
  expect(buildInfo({ dir: root }, { root, production: true, now }).ExactBuildKind).toBe('release');
  expect(buildInfo({ dir: root }, { root, archive: true, production: true, now }).ExactBuildKind).toBe('archive');
  expect(buildInfo({ dir: root }, { root, now, env: {} }).ExactDevMenu).toBeUndefined();
  expect(buildInfo({ dir: root }, { root, now, env: { EXACT_DEV_MENU: '0' } }).ExactDevMenu).toBe(false);
  expect(buildInfo({ dir: root }, { root, production: true, now, env: { EXACT_DEV_MENU: '1' } }).ExactDevMenu).toBe(true);
});

test('an app in its own repository carries its commit and whether it was dirty', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-buildinfo-'));
  const git = (...args) => spawnSync('git', ['-C', dir, ...args], { encoding: 'utf8' });
  git('init', '-q');
  writeFileSync(resolve(dir, 'app.contract'), 'component A\n');
  git('add', '.');
  git('-c', 'user.email=t@t', '-c', 'user.name=t', 'commit', '-qm', 'one');
  const clean = buildInfo({ dir }, { root, now });
  expect(clean.ExactAppCommit).toBe(commitOf(dir).sha);
  expect(clean.ExactAppCommitDirty).toBe(false);
  expect(clean.ExactAppBranch).toBe(git('rev-parse', '--abbrev-ref', 'HEAD').stdout.trim());
  writeFileSync(resolve(dir, 'release-notes.md'), '\n- one\n- two\n\n');
  git('add', '.');
  git('-c', 'user.email=t@t', '-c', 'user.name=t', 'commit', '-qm', 'notes');
  expect(buildInfo({ dir }, { root, now }).ExactReleaseNotes).toBe('- one\n- two');
  git('config', 'status.showUntrackedFiles', 'no');
  writeFileSync(resolve(dir, 'new.contract'), 'component N\n');
  expect(buildInfo({ dir }, { root, now }).ExactAppCommitDirty).toBe(true);
  expect(buildInfo({ dir: tmpdir() }, { root, now }).ExactAppCommit).toBeUndefined();
});

test('release notes: none or blank is no section, long ones are cut at a character, and non-UTF-8 fails the build', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-notes-'));
  expect(releaseNotes(dir)).toBe('');
  writeFileSync(resolve(dir, 'release-notes.md'), '  \n\n');
  expect(releaseNotes(dir)).toBe('');
  writeFileSync(resolve(dir, 'release-notes.md'), 'a'.repeat(NOTES_MAX - 1) + '🙂 and more');
  const cut = releaseNotes(dir);
  expect(cut.endsWith('\n…')).toBe(true);
  expect(cut).not.toContain('\uFFFD');
  expect(cut.startsWith('a'.repeat(NOTES_MAX - 1))).toBe(true);
  for (const bytes of [Buffer.from([0x68, 0xe9, 0x6c, 0x6c, 0x6f]), Buffer.alloc(NOTES_MAX + 5, 0x80), Buffer.concat([Buffer.alloc(NOTES_MAX, 0x61), Buffer.from([0x80])])]) {
    writeFileSync(resolve(dir, 'release-notes.md'), bytes);
    expect(() => releaseNotes(dir)).toThrow(/not UTF-8/);
  }
});
