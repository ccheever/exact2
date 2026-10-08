import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftPresentation as presentation,
  mobileNewTaskDraftStore as store, mobileNewTaskDraftChanged as changed } from './mobile-new-task-drafts';
import { mobileOutboxTransferApplyCleanup as apply } from './mobile-outbox-transfer-cleanup';
import type { MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import { mobileDraftAttachmentRecord, mobileDraftAttachmentIds } from './draft-attachment-order';
import { draftFiles, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { contextId, contextLink } from './shared/composer-editor-menu';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
const A = 'new-task:A', B = 'new-task:B', origin = 'https://transfer.test';
const image = { id: '11111111-1111-4111-8111-111111111111', name: 'photo.png', mimeType: 'image/png', sizeBytes: 42 };
const file: DraftFile = { id: '22222222-2222-4222-8222-222222222222', contextId: 'file_2', draftKey: A, environmentId: 'env',
  name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 3, source: 'attached', attachmentId: '', status: 'staged' };
async function fixture(document: Obj = { version: 1 }) {
  const client = new MobileDraftClient(); let disk = JSON.stringify(document), failure = '', retained = true;
  let gate: (() => Promise<void>) | null = null; const calls: Obj[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; }, async atomicWriteFile(_path, bytes) {
    if (gate) await gate(); if (failure === 'before') throw new Error('write failed'); disk = new TextDecoder().decode(bytes);
    if (failure === 'after') throw new Error('reply lost');
  } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: 1, value: request.op === 'status' ? { phase: 'disconnected' }
      : request.op === 'snapshotDraftRemove' || request.op === 'composerAttachRemove' ? retained ? { removed: false, retained: true } : { removed: true } : {} };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { origin, environmentId: 'env', projectId: 'project', threadId: '', generation: 1 });
  calls.length = 0;
  return { client, native, storage, calls, disk: () => obj(JSON.parse(disk)), fail(value: string) { failure = value; },
    retain(value: boolean) { retained = value; }, gate(value: (() => Promise<void>) | null) { gate = value; } };
}
async function populated() {
  const f = await fixture();
  for (const id of ['A', 'B']) { create(f.client, { id, origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' }); f.client.local.drafts[`new-task:${id}`] = ' original '; }
  f.client.local.snapshotDrafts[A] = [{ ...image }]; setDraftFiles(f.client.local, [{ ...file }]);
  mobileDraftAttachmentRecord(f.client, A, [file.id, image.id]);
  const claim: MobileOutboxTransferClaim = { transferId: 'message', draftKey: A, fingerprint: 'native-fingerprint', messageId: 'message',
    threadId: 'thread', commandId: 'command', mutationId: 'epoch:1', state: 'queued', capture: { version: 1, draft: presentation(f.client, A)! },
    record: { schemaVersion: 1, origin, environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command', text: 'original',
      createdAt: '2026-10-08T01:00:00.000Z', attachments: [] } };
  return { ...f, claim };
}
test('queued cleanup changes only captured key, defers bytes and persists its independent marker', async () => {
  const f = await populated(), snapshot = JSON.stringify(f.claim);
  expect(apply(f.client, f.claim)).toBe('applied'); expect(presentation(f.client, A)).toBeNull(); expect(f.client.local.drafts[B]).toBe(' original ');
  expect(f.client.local.snapshotReleases).toEqual([]); expect(store(f.client).fileReleases).toEqual([]); expect(f.calls).toEqual([]);
  expect(JSON.stringify(f.claim)).toBe(snapshot); await f.client.persist(f.storage);
  expect(obj(f.disk().mobileOutboxTransferCompletions).message).toEqual({ version: 1, draftKey: A, fingerprint: 'native-fingerprint' });
  expect(f.disk().pending).toEqual({});
});
test('marker prevents repeated cleanup even after equal content is recreated and restarted', async () => {
  const f = await populated(); apply(f.client, f.claim);
  create(f.client, { id: 'A', origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' });
  f.client.local.drafts[A] = ' original '; const revision = f.client.revision;
  expect(apply(f.client, f.claim)).toBe('already-applied'); expect(f.client.revision).toBe(revision); await f.client.persist(f.storage);
  const r = await fixture(f.disk()); expect(apply(r.client, f.claim)).toBe('already-applied'); expect(r.client.local.drafts[A]).toBe(' original ');
});
test('revision mismatch preserves content, mixed order and ABA-identical attachments', async () => {
  const f = await populated(); f.client.local.drafts[A] = 'new text'; changed(f.client, A);
  mobileDraftAttachmentRecord(f.client, A, [image.id, file.id]); const before = presentation(f.client, A);
  expect(apply(f.client, f.claim)).toBe('applied'); expect(presentation(f.client, A)).toEqual(before);
  expect(mobileDraftAttachmentIds(f.client, A)).toEqual([image.id, file.id]);
});
test('same revision removes only exact descriptors and preserves new referenced file metadata', async () => {
  const f = await populated(); f.client.local.drafts[A] = '[notes](t3-context://v1/file/file_2)';
  f.client.local.snapshotDrafts[A]![0]!.name = 'renamed.png';
  apply(f.client, f.claim); expect(draftFiles(f.client.local)).toEqual([file]);
  expect(f.client.local.snapshotDrafts[A]![0]!.name).toBe('renamed.png'); expect(f.client.local.drafts[A]).toContain('file_2');
});
test('actual image context references retain captured bytes despite unchanged content revision', async () => {
  const f = await populated();
  f.client.local.drafts[A] = contextLink('image', contextId('image', image.id), image.name);
  expect(f.client.local.snapshotDrafts[A]![0]!.contextId).toBeUndefined();
  apply(f.client, f.claim); expect(f.client.local.snapshotDrafts[A]).toEqual([image]);
  expect(draftFiles(f.client.local)).toEqual([]); await f.client.persist(f.storage);
  const r = await fixture(f.disk()); expect(presentation(r.client, A)?.images).toEqual([image]);
});
test('new whitespace remains a durable edit, while a truly empty cleared slot permits recreation', async () => {
  const f = await populated(); f.client.local.drafts[A] = ' \n\t ';
  apply(f.client, f.claim); expect(presentation(f.client, A)?.text).toBe(' \n\t ');
  await f.client.persist(f.storage); const r = await fixture(f.disk()); expect(presentation(r.client, A)?.text).toBe(' \n\t ');
  const empty = await populated(); empty.client.local.drafts[A] = '';
  apply(empty.client, empty.claim); expect(presentation(empty.client, A)).toBeNull(); expect(A in empty.client.local.drafts).toBe(false);
  expect(() => create(empty.client, { id: 'A', origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' })).not.toThrow();
});
test('new selections and workspace survive content cleanup and real hydration', async () => {
  const f = await populated(); const record = store(f.client).records[A]!;
  record.choices = { runtimeMode: 'auto', interactionMode: 'plan' };
  f.client.local.composerControls.contexts[A] = { envMode: 'worktree', branch: 'new', worktreePath: '/new' };
  record.branchChoice = { kind: 'explicit', ...f.client.local.composerControls.contexts[A]! };
  apply(f.client, f.claim); expect(f.client.local.drafts[A]).toBeUndefined(); expect(presentation(f.client, A)?.choices).toEqual(record.choices);
  await f.client.persist(f.storage); const r = await fixture(f.disk()); expect(presentation(r.client, A)?.choices).toEqual(record.choices);
  expect(presentation(r.client, A)?.workspace).toEqual(f.client.local.composerControls.contexts[A]);
  expect(presentation(r.client, A)?.branchChoice).toEqual(record.branchChoice);
});
test('retargeted, replaced and absent draft incarnations complete as no-ops', async () => {
  for (const field of ['origin', 'environmentId', 'projectId', 'createdAt'] as const) {
    const f = await populated(); store(f.client).records[A]![field] = 'different'; const before = presentation(f.client, A);
    expect(apply(f.client, f.claim)).toBe('applied'); expect(presentation(f.client, A)).toEqual(before);
  }
  const f = await populated(); delete store(f.client).records[A]; expect(apply(f.client, f.claim)).toBe('applied'); expect(f.client.local.drafts[A]).toBe(' original ');
});
test('nonqueued states and conflicting or corrupt completion markers refuse without mutation', async () => {
  const f = await populated();
  for (const state of ['prepared', 'failed', 'completed', 'released'] as const) {
    const before = JSON.stringify(f.client.local); expect(apply(f.client, { ...f.claim, state })).toBe('blocked'); expect(JSON.stringify(f.client.local)).toBe(before);
  }
  for (const marker of [null, [], 'invalid', { message: { version: 1, draftKey: A, fingerprint: 'other' } }]) {
    Object.assign(f.client.local, { mobileOutboxTransferCompletions: marker }); const before = JSON.stringify(f.client.local);
    expect(apply(f.client, f.claim)).toBe('blocked'); expect(JSON.stringify(f.client.local)).toBe(before);
    await f.client.persist(f.storage); const r = await fixture(f.disk()); expect(apply(r.client, f.claim)).toBe('blocked');
  }
});
test('save failure preserves marker in memory; actual saved document determines restart cleanup', async () => {
  for (const failure of ['before', 'after']) {
    const f = await populated(); await f.client.persist(f.storage); apply(f.client, f.claim); f.fail(failure);
    await expect(f.client.persist(f.storage)).rejects.toThrow(); expect(apply(f.client, f.claim)).toBe('already-applied');
    const r = await fixture(f.disk()); expect(apply(r.client, f.claim)).toBe(failure === 'before' ? 'applied' : 'already-applied');
  }
});
test('real inherited image/file flushes retain retries until explicit native removal', async () => {
  const f = await fixture(); f.client.local.snapshotReleases.push(image.id); store(f.client).fileReleases.push(file.id);
  await f.client.flushSnapshotReleases(f.native, f.storage); expect(f.client.local.snapshotReleases).toEqual([image.id]); expect(store(f.client).fileReleases).toEqual([file.id]);
  const r = await fixture(f.disk()); expect(r.client.local.snapshotReleases).toEqual([image.id]); expect(store(r.client).fileReleases).toEqual([file.id]);
  r.retain(false); await r.client.flushSnapshotReleases(r.native, r.storage); expect(r.client.local.snapshotReleases).toEqual([]); expect(store(r.client).fileReleases).toEqual([]);
});
test('ownership acquired while flush awaits persistence is checked by native removal response', async () => {
  const f = await fixture(); f.retain(false); f.client.local.snapshotReleases.push(image.id);
  let release!: () => void; const wait = new Promise<void>(resolve => { release = resolve; }); f.gate(() => wait);
  const flush = f.client.flushSnapshotReleases(f.native, f.storage); f.retain(true); f.gate(null); release(); await flush;
  expect(f.calls.some(call => call.op === 'snapshotDraftRemove')).toBe(true); expect(f.client.local.snapshotReleases).toEqual([image.id]);
});
