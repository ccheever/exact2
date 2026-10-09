// The build stamp (`buildinfo.mjs`): `bun test host/apple/buildinfo.test.mjs`;
// the async lane runs it (`tests/it/development.rs`).
import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { buildInfo, commitOf } from './buildinfo.mjs';

const root = resolve(import.meta.dir, '../..');
const now = new Date('2026-10-08T10:00:00Z');

test('an app inside exact2 carries exact2\'s commit alone, and the kind', () => {
  const info = buildInfo({ dir: resolve(root, 'apps/caltrain') }, { root, now });
  expect(info.ExactBuildTime).toBe('2026-10-08T10:00:00.000Z');
  expect(info.ExactBuildKind).toBe('debug');
  expect(info.ExactCommit).toMatch(/^[0-9a-f]{40}$/);
  expect(typeof info.ExactCommitDirty).toBe('boolean');
  expect(info.ExactAppCommit).toBeUndefined();
  expect(buildInfo({ dir: root }, { root, production: true, now }).ExactBuildKind).toBe('release');
  expect(buildInfo({ dir: root }, { root, archive: true, production: true, now }).ExactBuildKind).toBe('archive');
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
  git('config', 'status.showUntrackedFiles', 'no');
  writeFileSync(resolve(dir, 'new.contract'), 'component N\n');
  expect(buildInfo({ dir }, { root, now }).ExactAppCommitDirty).toBe(true);
  expect(buildInfo({ dir: tmpdir() }, { root, now }).ExactAppCommit).toBeUndefined();
});
