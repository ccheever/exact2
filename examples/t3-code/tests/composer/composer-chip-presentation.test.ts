import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { composerChipFaces } from '../../src/composer/composer-chip-presentation';
import { composerChips } from '../../src/composer/composer-source-chips';
import { refreshFileMetadata } from '../../src/shared/file-metadata';
import type { Native } from '../../src/client/protocol';

test('filesystem metadata distinguishes dotted folders and extensionless files in composer chip faces', async () => {
  const value = '@release.v1 @capture ', client = new T3Client();
  Object.defineProperties(client, { draft: { get: () => value }, draftKey: { get: () => 'env:t1' }, ready: { get: () => true }, scopes: { get: () => ['filesystem:read'] },
    restAccess: { value: () => ({ request: async () => ({ entries: [{ kind: 'directory' }, { kind: 'file', mimeType: 'image/png' }] }) }) } });
  client.projectId = 'p1';
  client.shell.projects = [{ id: 'p1', workspaceRoot: '/repo' }];
  await refreshFileMetadata(client, { available: true } as Native);
  const faces = composerChipFaces(client, value, composerChips(value));
  expect(faces[0]).toMatchObject({ icon: 'folder', fileToken: '', interactive: true, tip: 'release.v1' });
  expect(faces[1]).toMatchObject({ icon: 'at-sign', fileToken: 'image', interactive: true });
});

test('context availability and pull-request state use existing product records', () => {
  const client = new T3Client(), value = '[PR #7](t3-context://v1/review-comment/review-comment_p) [gone](t3-context://v1/terminal/terminal_missing)';
  Object.assign(client.local, { reviewComments: { 'review-comment_p': { kind: 'review-comment', contextId: 'review-comment_p', pullRequest: { state: 'merged', number: 7 } } } });
  const faces = composerChipFaces(client, value, composerChips(value));
  expect(faces[0]).toMatchObject({ kind: 'pr-merged', icon: 'git-merge', available: true, interactive: true });
  expect(faces[1]).toMatchObject({ kind: 'neutral', icon: 'circle-dashed', available: false, interactive: false, tip: 'Unavailable context, gone' });
});

test('preview skills have their own presentation and no context actions', () => {
  const client = new T3Client(), value = '$frontend-design @file.ts [Thread](t3-context://v1/thread/thread_t1)';
  const faces = composerChipFaces(client, value, composerChips(value), true);
  expect(faces[0]).toMatchObject({ label: 'Frontend Design', kind: 'skill', interactive: true });
  expect(faces[1]?.interactive).toBe(false);
  expect(faces[2]?.available).toBe(false);
});

test('a thread chip leads with its project icon, MessagesSquare without a known project (ThreadContextChip, #22621)', () => {
  const client = new T3Client(), value = '[Plan](t3-context://v1/thread/thread_t1) [Loose](t3-context://v1/thread/thread_t2)';
  client.shell.projects = [{ id: 'p1', title: 'Clone', projectIcon: { kind: 'emoji', emoji: '🧪' } }];
  client.shell.threads = [{ id: 't1', title: 'Plan', projectId: 'p1' }, { id: 't2', title: 'Loose', projectId: 'gone' }];
  const faces = composerChipFaces(client, value, composerChips(value));
  expect(faces[0]).toMatchObject({ kind: 'thread', icon: 'messages-square', glyph: { kind: 'emoji', emoji: '🧪' } });
  expect(faces[1]).toMatchObject({ kind: 'thread', icon: 'messages-square', glyph: { kind: '' } });
});
