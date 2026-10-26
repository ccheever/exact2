// The design server's commit session (scripts/tamber-commit.mjs), a file whose text is HEAD's again (the third
// hands-on pass's #3; theme2: re-baseline it, and keep the refusal while tool edits are pending): committed by hand,
// the next write is a new baseline and commits; a commit asked then has nothing to say but that; a write in flight
// keeps its record; an edit still pending when HEAD moves stays refused.
import { afterEach, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { designSession } from './tamber-commit.mjs';

const sha = text => createHash('sha256').update(text).digest('hex');
let dir;
afterEach(() => { if (dir) rmSync(dir, { recursive: true, force: true }); dir = undefined; });
const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: dir, encoding: 'utf8' });
function repo() {
  dir = mkdtempSync(join(tmpdir(), 'exact-commit-session-'));
  git('init', '-q'); git('config', 'user.email', 't@example.test'); git('config', 'user.name', 't'); writeFileSync(join(dir, 'app.tamber'), 'v1\n'); git('add', '.'); git('commit', '-q', '-m', 'start');
  return designSession(dir);
}
/** A write the server made of app.tamber, from `from` to `to`, as the server reports it to the session. */
function write(s, from, to, n) {
  s.prepare(['app.tamber']);
  writeFileSync(join(dir, 'app.tamber'), to);
  s.wrote('app.tamber', { before: sha(from), after: sha(to), client: 'agent', tokens: ['ink'], write: n });
}

test('committed by hand, the next write is a new baseline and the session commits it', () => {
  const s = repo();
  write(s, 'v1\n', 'v2\n', 1);
  git('commit', '-q', '-am', 'by hand');
  write(s, 'v2\n', 'v3\n', 2);
  const done = s.commit({ client: 'person', message: 'ink' });
  expect(done.error, JSON.stringify(done)).toBeUndefined();
  expect(git('log', '-1', '--format=%s').stdout.trim()).toBe('ink');
});

test('committed by hand, then a commit asked: nothing of the session\'s is left, said so, refusing nothing', () => {
  const s = repo();
  write(s, 'v1\n', 'v2\n', 1);
  git('commit', '-q', '-am', 'by hand');
  const done = s.commit({ client: 'person' });
  expect(done.error.refused).toBeUndefined();
  expect(done.error.message).toContain('nothing to commit');
});

test('a write in flight keeps its record: an undo on disk before its reply is no foreign change', () => {
  const s = repo();
  write(s, 'v1\n', 'v2\n', 1);
  // The undo, prepared and on disk (the file HEAD's again), its reply not yet in.
  s.prepare(['app.tamber']);
  writeFileSync(join(dir, 'app.tamber'), 'v1\n');
  // Another write prepared meanwhile, which writes nothing.
  const held = s.prepare(['app.tamber']);
  s.release(held);
  s.wrote('app.tamber', { before: sha('v2\n'), after: sha('v1\n'), client: 'agent', tokens: ['ink'], write: 2 });
  expect(s.state()['app.tamber']).toMatchObject({ foreign: false, dirty: null });
  // The next write, the file HEAD's and nothing in flight, is a new baseline, and commits.
  write(s, 'v1\n', 'v3\n', 3);
  const done = s.commit({ client: 'person' });
  expect(done.error, JSON.stringify(done)).toBeUndefined();
});

test('an edit still pending when HEAD moves stays refused', () => {
  const s = repo();
  write(s, 'v1\n', 'v2\n', 1);
  git('commit', '-q', '--allow-empty', '-m', 'elsewhere');
  expect(s.commit({ client: 'person' }).error.refused).toEqual([{ file: 'app.tamber', why: 'HEAD has moved since the session first wrote it' }]);
});
