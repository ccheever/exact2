// @ref llp/1109.005-composer-and-transcript.decision.md#pending-task-editor-save-and-restart-recovery
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobilePendingTaskEditorsHydrate as hydrate, mobilePendingTaskEditorsCreate as create,
  mobilePendingTaskEditorsReplace as replace, mobilePendingTaskEditorsRemove as remove,
  type MobilePendingTaskMarker } from './mobile-pending-task-state';
import { mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import type { MobileOutboxAttachment } from './mobile-outbox-model';
import type { Files, Native } from './shared/protocol';
import { obj, type Obj } from './shared/domain';
const image = '11111111-1111-4111-8111-111111111111', file = '22222222-2222-4222-8222-222222222222';
const attachment = (id: string, kind: 'image' | 'file'): MobileOutboxAttachment => ({ id, kind, name: 'saved', mimeType: 'text/plain',
  sizeBytes: 3, uploadId: '', status: 'staged', ...(kind === 'file' ? { contextId: 'context-file', source: 'attached' } : {}) });
function setup() {
  const client = new MobileDraftClient(); hydrate(client, {});
  const owner = { origin: 'https://one.test', environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command' };
  const baseline: MobilePendingTaskMarker = { version: 1, owner, session: 'editor', revision: 1, draftKey: 'new-task:pending-message', contentRevision: 0,
    baseline: { token: 'old:1', revision: 1, record: { schemaVersion: 1, ...owner, text: 'baseline', attachments: [],
      createdAt: '2026-10-08T12:00:00.000Z', creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } } }, pending: null };
  expect(create(client, baseline)).not.toBeNull();
  const marker: MobilePendingTaskMarker = { ...baseline, revision: 2, contentRevision: 1, pending: { mutationId: 'old:2', contentRevision: 1,
    request: { ownerEpoch: 'old', mutationId: 'old:2', messageId: 'message', operation: 'update', expectedToken: 'old:1', expectedRevision: 1,
      requireUnheld: false, record: { ...baseline.baseline.record, attachments: [attachment(image, 'image'), attachment(file, 'file')] } } } };
  expect(replace(client, baseline, marker)).not.toBeNull();
  const calls: Obj[] = []; let fail = false;
  const native: Native = { available: true, watch: () => '', async later(input) { calls.push(obj(input)); return { ok: true, generation: client.generation, value: { removed: true } }; } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new Uint8Array(); }, async atomicWriteFile() { if (fail) throw new Error('disk full'); } } };
  return { client, marker, calls, native, storage, fail: (value: boolean) => { fail = value; } };
}
test('failed first capture write still blocks direct command removals of both captured byte kinds', async () => {
  const f = setup(); f.fail(true); await expect(f.client.persist(f.storage)).rejects.toThrow('disk full');
  const handle = mobileDraftRecoveryHandles(f.client, f.native, f.storage).native!;
  for (const [op, id] of [['snapshotDraftRemove', image.toUpperCase()], ['composerAttachRemove', file]])
    await expect(handle.later({ op, id })).rejects.toThrow('retained by a pending editor');
  expect(f.calls).toHaveLength(0);
  expect(f.client.local.snapshotReleases).toEqual([image]);
  expect(mobileNewTaskDraftStore(f.client).fileReleases).toEqual([file]);
  await handle.later({ op: 'composerAttachRead', id: file }); expect(f.calls.map(value => value.op)).toEqual(['composerAttachRead']);
});
test('ordinary flush keeps retry IDs for removed chips until exact pending ownership is resolved', async () => {
  const f = setup(); f.client.local.snapshotReleases = [image]; mobileNewTaskDraftStore(f.client).fileReleases = [file];
  await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(f.calls).toHaveLength(0); expect(f.client.local.snapshotReleases).toEqual([image]);
  expect(mobileNewTaskDraftStore(f.client).fileReleases).toEqual([file]);
  const cleared = { ...f.marker, revision: 3, pending: null }; expect(replace(f.client, f.marker, cleared)).not.toBeNull();
  expect(remove(f.client, cleared)).toBe(true);
  await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(f.calls.map(value => value.op)).toEqual(['snapshotDraftRemove', 'composerAttachRemove']);
  expect(f.client.local.snapshotReleases).toEqual([]); expect(mobileNewTaskDraftStore(f.client).fileReleases).toEqual([]);
});
test('unread or malformed pending ownership cannot fall through to native byte removal', async () => {
  for (const malformed of [undefined, { version: 1, markers: { broken: {} } }]) {
    const f = setup(), client = new MobileDraftClient();
    if (malformed) hydrate(client, { mobilePendingTaskEditors: malformed });
    const handle = mobileDraftRecoveryHandles(client, f.native, f.storage).native!;
    await expect(handle.later({ op: 'snapshotDraftRemove', id: image })).rejects.toThrow('retained by a pending editor');
    expect(f.calls).toHaveLength(0);
  }
});
test('the saved baseline continues owning bytes after its pending receipt is resolved', async () => {
  const f = setup(), resolved: MobilePendingTaskMarker = { ...f.marker, revision: 3,
    baseline: { record: f.marker.pending!.request.record, token: 'old:2', revision: 2 }, pending: null };
  expect(replace(f.client, f.marker, resolved)).not.toBeNull();
  const handle = mobileDraftRecoveryHandles(f.client, f.native, f.storage).native!;
  await expect(handle.later({ op: 'composerAttachRemove', id: file })).rejects.toThrow('retained by a pending editor');
  expect(f.calls).toHaveLength(0);
  expect(remove(f.client, resolved)).toBe(true);
  await handle.later({ op: 'composerAttachRemove', id: file }); expect(f.calls).toHaveLength(1);
});
