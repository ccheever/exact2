// The build stamp (`buildinfo.mjs`): `bun test host/apple/buildinfo.test.mjs`;
// the async lane runs it (`tests/it/development.rs`).
import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { buildInfo, commitOf, releaseNotes, NOTES_MAX } from './buildinfo.mjs';
import { nativeManifestMetadata } from '../../scripts/app.mjs';

const root = resolve(import.meta.dir, '../..');
const now = new Date('2026-10-08T10:00:00Z');

test('frame health collection defaults off in production and follows the manifest independently of the menu', () => {
  const stamp = (manifest, options = {}) => buildInfo({ dir: root, manifest }, { root, now, env: {}, ...options });
  expect(stamp({}).ExactFrameHealthMode).toBe('core');
  expect(stamp({}, { production: true }).ExactFrameHealthMode).toBe('off');
  expect(stamp({}, { env: { EXACT_UPDATE_TRUST: 'production' } }).ExactFrameHealthMode).toBe('off');
  expect(stamp({ frame_health: { mode: 'off' } }).ExactFrameHealthMode).toBe('off');
  expect(stamp({ frame_health: { mode: 'core' } }, { production: true }).ExactFrameHealthMode).toBe('core');
  expect(nativeManifestMetadata({ app: { id: 'com.example.health' }, frame_health: { mode: 'off' } }, 'ios').frame_health).toEqual({ mode: 'off' });
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
