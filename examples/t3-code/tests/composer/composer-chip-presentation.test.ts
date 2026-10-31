import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { composerChipFaces } from '../../src/composer/composer-chip-presentation';
import { composerChips } from '../../src/composer/composer-source-chips';
import { refreshFileMetadata } from '../../src/shared/file-metadata';
import type { Native } from '../../src/client/protocol';
import { fileChipContexts, isSvgAttachment, setDraftFiles } from '../../src/composer/composer-editor-files';
import { composerChipOpen, composerChipTake } from '../../src/composer/composer-chip-bridge';
import { composerSourceSession } from '../../src/composer/composer-rich-state';
import { performChipAct } from '../../src/composer/composer-chip-details';
import { composerFileView } from '../../src/composer/composer-chip-file';
import { observeComposer } from '../support/composer-field-fixture';

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

test("an attached SVG's chip draws as an image chip and its press opens the file (FileChip's SvgFileChip)", async () => {
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env', threadId: 't1' });
  const file = (id: string, name: string, mimeType: string) => ({ id, contextId: `file_${id}`, draftKey: client.draftKey, environmentId: 'env', name, mimeType,
    sizeBytes: 2048, source: 'attached', attachmentId: '', status: 'staged' as const });
  setDraftFiles(client.local, [file('s', 'logo.svg', 'image/svg+xml'), file('o', 'mark.SVG', 'binary/octet-stream'), file('n', 'notes.txt', 'text/plain')]);
  const value = '[logo.svg](t3-context://v1/file/file_s) [mark.SVG](t3-context://v1/file/file_o) [notes.txt](t3-context://v1/file/file_n) ';
  expect(fileChipContexts(client, value)).toEqual({ 'file/file_s': 'file\t2 KB\timage', 'file/file_o': 'file\t2 KB\timage', 'file/file_n': 'file\t2 KB' });
  expect(composerChipFaces(client, value, composerChips(value)).map(face => [face.kind, face.icon, face.interactive])).toEqual([
    ['file', 'image', true], ['file', 'image', true], ['file', 'file', true]]);
  expect(isSvgAttachment('diagram.svg', 'application/unknown')).toBe(true);
  expect(isSvgAttachment('diagram.svg', 'text/xml')).toBe(false);
  // The press, as the field reports it: a file press, which opens the draft file's preview.
  const snapshot = observeComposer(client, value), session = composerSourceSession(client);
  const press = composerChipOpen(client, { ...snapshot, ...session.chips[0]!, surface: 'composer', comment: false, frame: { unavailable: false, x: 10, y: 20, width: 80, height: 18 } });
  expect(press).toMatchObject({ kind: 'file', act: 'file', context: 'file_s' });
  await performChipAct(client, {} as Native, composerChipTake(client, press!.seq)!);
  expect((await composerFileView(client, { available: true } as Native, 1)).file).toMatchObject({ id: 's', name: 'logo.svg', origin: 'Draft' });
});

test("each chip's accessible name is the reference chip's aria-label (contextChipParts, 7c3b3b3605ef)", () => {
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env', threadId: 't1' });
  setDraftFiles(client.local, [{ id: 'n', contextId: 'file_n', draftKey: client.draftKey, environmentId: 'env', name: 'notes.txt', mimeType: 'text/plain',
    sizeBytes: 2048, source: 'attached', attachmentId: '', status: 'staged' as const }]);
  Object.assign(client.local, { reviewComments: {
    'review-comment_p': { kind: 'review-comment', contextId: 'review-comment_p', pullRequest: { state: 'open', isDraft: false, number: 7, title: 'Fix tabs', url: 'https://gitlab.com/g/p/-/merge_requests/7' } },
  } });
  const value = '[plan.md](docs/plan.md) [PR #7](t3-context://v1/review-comment/review-comment_p) [notes.txt](t3-context://v1/file/file_n) [gone](t3-context://v1/terminal/terminal_missing) ';
  expect(composerChipFaces(client, value, composerChips(value)).map(face => face.ariaLabel)).toEqual([
    'Preview docs/plan.md', 'Open Open merge request PR #7: Fix tabs', 'File attachment, notes.txt, 2 KB', 'Unavailable context, gone']);
});
