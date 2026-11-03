// `exact share`'s decisions: which link a share publishes to, and when it refuses or rebuilds nothing.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { branchOf, mismatch, parseLink, plan, sourceOf } from './share.mjs';

const url = 'https://appdrop.app/d/k3m9x2p7qa';
const record = { url, branch: 'main', source: 'a b', revision: 4 };

test('the first share, and --new-link, create a link', () => {
  expect(plan(null, { branch: 'main', source: 'a b' }).action).toBe('create');
  expect(plan(record, { branch: 'main', source: 'c d', newLink: true }).action).toBe('create');
});

test('a share from the link\'s branch updates it, or rebuilds nothing when the source is unchanged', () => {
  expect(plan(record, { branch: 'main', source: 'c d' })).toMatchObject({ action: 'update', url, id: 'k3m9x2p7qa' });
  expect(plan(record, { branch: 'main', source: 'a b' })).toMatchObject({ action: 'current', revision: 4 });
  // A dirty tree has no source: always rebuilt.
  expect(plan(record, { branch: 'main', source: null }).action).toBe('update');
});

test('a share from another branch refuses, unless --force publishes over it', () => {
  const refused = plan(record, { branch: 'feat/onboarding', source: 'a b' });
  expect(refused).toMatchObject({ action: 'refuse', url, from: 'main' });
  expect(mismatch(refused, 'feat/onboarding')).toBe(`This app's AppDrop link, ${url}, was last shared from main; this checkout is on feat/onboarding. `
    + "Pass --new-link to give feat/onboarding its own link, or --force to publish over main's.");
  // Forced, it publishes even when the source matches: the branch the link records changes.
  expect(plan(record, { branch: 'feat/onboarding', source: 'a b', force: true }).action).toBe('update');
});

test('only a share link is a link; its update key never is', () => {
  expect(parseLink(`${url}/`)).toEqual({ url, origin: 'https://appdrop.app', id: 'k3m9x2p7qa' });
  expect(parseLink(`${url}/manage#secret`)).toBeNull();
  expect(parseLink('http://appdrop.app/d/k3m9x2p7qa')).toBeNull();
});

test('the branch and source come from git', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-share-'));
  try {
    const git = (...args) => spawnSync('git', ['-C', dir, '-c', 'user.name=t', '-c', 'user.email=t@t', ...args], { encoding: 'utf8' });
    expect(branchOf(dir)).toBeNull();
    git('init', '-q', '-b', 'feat/x');
    writeFileSync(join(dir, 'app.json'), '{}');
    git('add', '.'); git('commit', '-qm', 'one');
    expect(branchOf(dir)).toBe('feat/x');
    const head = git('rev-parse', 'HEAD').stdout.trim();
    expect(sourceOf(dir, dir)).toBe(`${head} ${head}`);
    writeFileSync(join(dir, 'app.json'), '{"changed":true}');
    expect(sourceOf(dir, dir)).toBeNull();
    git('checkout', '-q', '--detach');
    expect(branchOf(dir)).toBe(head.slice(0, branchOf(dir).length));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
