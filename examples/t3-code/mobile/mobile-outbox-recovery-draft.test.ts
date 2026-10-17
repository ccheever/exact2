import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles, mobileRecoveredMessageContext } from './mobile-draft-recovery';
import { mobileOutboxRecoveryDraftPrepare as prepare, mobileOutboxRecoveryDraftAdopt as adopt,
  mobileOutboxRecoveryDraftRollback as rollback, mobileOutboxRecoveryDraftCapture as capture,
  mobileOutboxRecoveryDraftApplyChoices as applyChoices } from './mobile-outbox-recovery-draft';
import { mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileDraftAttachmentIds } from './draft-attachment-order';
import { draftFiles, setDraftFiles } from './shared/composer-editor-files';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import type { MobileOutboxRecord } from './mobile-outbox-model';
const id = (n: number) => `${n.toString().padStart(8, '0')}-1111-4111-a111-111111111111`;
const image = (n: number) => ({ id: id(n), name: `${n}.png`, mimeType: 'image/png', sizeBytes: 1, uploadId: '' });
const record = (): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://recovery.test', environmentId: 'env',
  threadId: 'thread', messageId: 'message', commandId: 'command', createdAt: '2026-10-08T00:00:00.000Z',
  text: 'queued text', attachments: [{ ...image(1), kind: 'image', status: 'staged' }],
  creation: { projectId: 'project', workspaceMode: 'worktree', branch: 'saved', worktreePath: '/saved', startFromOrigin: true } });
async function fixture(document: Obj = { version: 1 }) {
  const client = new MobileDraftClient(); let disk = JSON.stringify(document), hook: (() => Promise<void>) | null = null;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { disk = new TextDecoder().decode(bytes); if (hook) await hook(); } } };
  const native: Native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: { phase: 'disconnected' } }; } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { origin: 'https://recovery.test', environmentId: 'env', projectId: 'project', threadId: '' });
  return { client, storage, native, disk: () => JSON.parse(disk) as Obj, hook: (value: () => Promise<void>) => { hook = value; } };
}
test('rejected creation adopts real draft owners once, preserving project/workspace/settings/context and ordering across restart', async () => {
  const f = await fixture(), input = record(); input.modelSelection = { instanceId: 'codex', model: 'saved-model' }; input.runtimeMode = 'full-access';
  input.text = '[term](t3-context://v1/terminal/term)';
  const context = { version: 1, kind: 'terminal', contextId: 'term', label: 'term', text: 'captured terminal', extra: { keep: true } };
  input.context = { version: 1, records: [context] };
  const before = JSON.stringify(f.client.local), change = prepare(f.client, input, 'rejected');
  expect(JSON.stringify(f.client.local)).toBe(before); expect(change.key).toBe('new-task:restored-message');
  expect(adopt(f.client, change)).toBe(true); expect(adopt(f.client, change)).toBe(false);
  await f.client.persist(f.storage); const cold = await fixture(f.disk());
  expect(cold.client.local.drafts[change.key]).toBe(input.text); expect(cold.client.local.snapshotDrafts[change.key]).toEqual([image(1)]);
  expect(mobileNewTaskDraftStore(cold.client).records[change.key]).toMatchObject({ projectId: 'project', createdAt: input.createdAt,
    choices: { providerId: 'codex', modelId: 'saved-model', runtimeMode: 'full-access' }, context: input.context,
    branchChoice: { kind: 'explicit', envMode: 'worktree', branch: 'saved', worktreePath: '/saved', startFromOrigin: true } });
  expect(mobileDraftAttachmentIds(cold.client, change.key)).toEqual([id(1)]);
});
test('accepted edits append to the existing thread; existing attachment metadata and absent choices stay intact', async () => {
  const f = await fixture(), key = 'env:thread', input = record();
  f.client.local.drafts[key] = 'current'; f.client.local.snapshotDrafts[key] = [{ ...image(1), name: 'existing.png' }, image(2)];
  f.client.local.composerControls.staged[key] = { providerId: 'provider', modelId: 'existing', options: [], runtimeMode: 'auto', interactionMode: 'plan' };
  const staged = structuredClone(f.client.local.composerControls.staged[key]);
  const change = prepare(f.client, input, 'accepted-edits'); expect(change.key).toBe(key); expect(adopt(f.client, change)).toBe(true);
  expect(f.client.local.drafts[key]).toBe('current\n\nqueued text'); expect(f.client.local.snapshotDrafts[key]?.map(x => x.name)).toEqual(['existing.png', '2.png']);
  expect(f.client.local.composerControls.staged[key]).toEqual(staged); expect(mobileNewTaskDraftStore(f.client).records[key]).toBeUndefined();
  expect(rollback(f.client, change)).toBe(true); expect(f.client.local.drafts[key]).toBe('current');
});
test('stale adoption and rollback preserve newer typing and foreign owners', async () => {
  const f = await fixture(), key = 'env:thread', change = prepare(f.client, record(), 'accepted-edits');
  f.client.local.drafts[key] = 'new typing'; expect(adopt(f.client, change)).toBe(false);
  const next = prepare(f.client, record(), 'accepted-edits'); expect(adopt(f.client, next)).toBe(true);
  f.client.local.drafts[key] += '!'; expect(rollback(f.client, next)).toBe(false); expect(f.client.local.drafts[key]).toBe('new typing\n\nqueued text!');
  const foreign = prepare(f.client, record(), 'accepted-edits'); f.client.environmentId = 'other'; expect(adopt(f.client, foreign)).toBe(false);
});
test('failed persistence retains live recovery and later writes preserve typing instead of replaying its capture', async () => {
  const f = await fixture(), change = prepare(f.client, record(), 'accepted-edits'); expect(adopt(f.client, change)).toBe(true);
  const fail: Files = { fs: { ...f.storage.fs, async atomicWriteFile() { throw Error('disk full'); } } };
  await expect(f.client.persist(fail)).rejects.toThrow('disk full'); expect(f.client.local.drafts[change.key]).toBe('queued text');
  f.hook(async () => { f.client.local.drafts[change.key] = 'typed while awaiting'; });
  await f.client.persist(f.storage); await f.client.persist(f.storage);
  expect(obj(f.disk().drafts)[change.key]).toBe('typed while awaiting');
});
test('accepted overflow images and more than 400 total files survive actual load and persistence', async () => {
  const f = await fixture(), key = 'env:thread'; f.client.local.snapshotDrafts[key] = Array.from({ length: 100 }, (_, n) => image(n + 2));
  const change = prepare(f.client, record(), 'accepted-edits'); expect(adopt(f.client, change)).toBe(true);
  setDraftFiles(f.client.local, Array.from({ length: 405 }, (_, n) => ({ id: id(n + 200), contextId: `file_${n}`, draftKey: `env:other-${n}`,
    environmentId: 'env', name: 'kept.txt', mimeType: 'text/plain', sizeBytes: 1, source: 'attached', attachmentId: '', status: 'staged' })));
  await f.client.persist(f.storage); const cold = await fixture(f.disk()); await cold.client.persist(cold.storage);
  expect(cold.client.local.snapshotDrafts[key]).toHaveLength(101); expect(obj(cold.disk().snapshotDrafts)[key]).toHaveLength(101);
  expect(draftFiles(cold.client.local)).toHaveLength(405); expect(cold.disk().composerFiles).toHaveLength(405);
  const ordinary = record(); delete ordinary.creation;
  expect(() => prepare(cold.client, ordinary, 'rejected')).toThrow('at most 100');
});
test('frozen recovered context wins over stale cache and cannot revive a removed attachment', async () => {
  const f = await fixture(), input = record(); input.text = '[review](t3-context://v1/review-comment/review) [image](t3-context://v1/image/shot)';
  const review = { version: 1, kind: 'review-comment', contextId: 'review', label: 'review', comment: 'frozen' };
  const shot = { version: 1, kind: 'image', contextId: 'shot', label: 'shot', attachmentId: id(1), name: '1.png', mimeType: 'image/png', sizeBytes: 1 };
  input.context = { version: 1, records: [review, shot] };
  const change = prepare(f.client, input, 'accepted-edits'); expect(adopt(f.client, change)).toBe(true);
  f.client.local.snapshotDrafts[change.key]![0]!.uploadId = 'uploaded';
  const stale = { version: 1, records: [{ ...review, comment: 'stale' }, { ...shot, attachmentId: 'old-upload' }] };
  expect(mobileRecoveredMessageContext(f.client, change.key, input.text, [{ id: 'uploaded' }], stale)?.records).toEqual([review, { ...shot, attachmentId: 'uploaded' }]);
  f.client.local.snapshotDrafts[change.key] = [];
  expect(mobileRecoveredMessageContext(f.client, change.key, input.text, [{ id: 'old-upload' }], stale)?.records).toEqual([review]);
});
test('partial recovered settings wait for real thread defaults and apply once without erasing absent picks', async () => {
  const f = await fixture(), input = record(); input.runtimeMode = 'full-access';
  expect(adopt(f.client, prepare(f.client, input, 'accepted-edits'))).toBe(true);
  f.client.threadId = 'thread'; applyChoices(f.client); expect(f.client.local.composerControls.staged['env:thread']).toBeUndefined();
  await f.client.persist(f.storage); const cold = await fixture(f.disk());
  Object.assign(cold.client, { threadId: 'thread', providerId: 'actual', modelId: 'current', modelOptions: [{ id: 'reasoning', value: 'high' }], runtimeMode: 'auto', interactionMode: 'plan' });
  applyChoices(cold.client); expect(cold.client.local.composerControls.staged['env:thread']).toEqual({ providerId: 'actual', modelId: 'current',
    options: [{ id: 'reasoning', value: 'high' }], runtimeMode: 'full-access', interactionMode: 'plan' });
  cold.client.runtimeMode = 'auto'; cold.client.local.composerControls.staged['env:thread']!.runtimeMode = 'auto'; applyChoices(cold.client);
  expect(cold.client.runtimeMode).toBe('auto');
});
test('foreign destination stamp and malformed target context leave all real owners untouched', async () => {
  const f = await fixture(), input = record(), change = prepare(f.client, input, 'rejected'); expect(adopt(f.client, change)).toBe(true);
  const metadata = mobileNewTaskDraftStore(f.client).records[change.key]!; metadata.projectId = 'different';
  const before = JSON.stringify(f.client.local); expect(() => prepare(f.client, input, 'rejected')).toThrow('another saved task'); expect(JSON.stringify(f.client.local)).toBe(before);
  metadata.projectId = 'project'; metadata.context = { version: 2, records: [] };
  expect(() => prepare(f.client, input, 'rejected')).toThrow('context is invalid');
  expect(capture(f.client, change.key).metadata).toEqual(metadata);
});

test('rollback restores the prior terminal cache only while its recovered payload is untouched', async () => {
  for (const newer of [false, true]) {
    const f = await fixture(), input = record(), original = { version: 1, kind: 'terminal', contextId: 'term', label: 'term', text: 'original' };
    Object.assign(f.client.local, { terminalContexts: { term: original } });
    input.text = '[term](t3-context://v1/terminal/term)'; input.context = { version: 1, records: [{ ...original, text: 'recovered' }] };
    const change = prepare(f.client, input, 'accepted-edits'); expect(adopt(f.client, change)).toBe(true);
    if (newer) obj(obj(f.client.local).terminalContexts).term = { ...original, text: 'other editor' };
    expect(rollback(f.client, change)).toBe(true);
    expect(obj(obj(obj(f.client.local).terminalContexts).term).text).toBe(newer ? 'other editor' : 'original');
  }
});
