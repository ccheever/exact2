import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftLookup as lookup, mobileNewTaskDraftStore as store } from './mobile-new-task-drafts';
import { mobileComposerAttachmentAction as action } from './composer-attachments';
import { draftFiles } from './shared/composer-editor-files';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { mobilePendingTaskEditorsCreate, mobilePendingTaskEditorsReplace, mobilePendingTaskEditorsRemove,
  mobilePendingTaskEditorsSnapshot, type MobilePendingTaskMarker } from './mobile-pending-task-state';
const imageId = '11111111-1111-4111-a111-111111111111';
const fileId = '22222222-2222-4222-a222-222222222222';
async function fixture(kind = 'image') {
  const client = new MobileDraftClient(), writes: Obj[] = [], calls: Obj[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; },
    async atomicWriteFile(_path, bytes) { writes.push(obj(JSON.parse(new TextDecoder().decode(bytes)))); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const call = obj(input); calls.push(call);
    return { ok: true, generation: client.generation, value: call.op === 'composerAttachPick' ? { files: [
      { kind, id: kind === 'image' ? imageId : fileId, name: kind === 'image' ? 'photo.jpg' : 'notes.txt',
        mimeType: kind === 'image' ? 'image/jpeg' : 'text/plain', sizeBytes: 50 }] } : call.op === 'composerAttachRemove' || call.op === 'snapshotDraftRemove' ? { removed: true } : { applied: false } };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage);
  await client.command('dismiss-error', '', '', 0, handles.native, handles.storage);
  await handles.native!.later({ op: 'devicePresentation' });
  Object.assign(client, { environmentId: 'env', projectId: 'project', origin: 'https://draft.test' });
  for (const id of ['A', 'B']) {
    create(client, { id, environmentId: 'env', projectId: 'project', origin: client.origin, createdAt: '2026-10-08T00:00:00.000Z' });
    client.local.drafts[`new-task:${id}`] = id;
  }
  bind(client, 'new-task:A', 'flow-A'); writes.length = 0; calls.length = 0;
  const run = (source = 'photos', id = '') => action(source, id, native, storage, client);
  return { client, native, storage, writes, calls, run };
}
test('file removal owner departure retains a release record for the removed captured file', async () => {
  const f = await fixture('file'); await f.run(); const original = f.native.later;
  f.native.later = async input => {
    const reply = await original(input);
    if (obj(input).op === 'editorEdit') bind(f.client, 'new-task:B', 'flow-B');
    return reply;
  };
  await f.run('remove-file', fileId);
  expect(f.client.draft).toBe('B');
  expect(lookup(f.client, 'new-task:B')?.revision).toBe(0);
  expect(draftFiles(f.client.local).some(file => file.id === fileId)).toBe(false);
  expect(store(f.client).fileReleases).toContain(fileId);
});
test('failed removal save retains cleanup until a later durable flush', async () => {
  const f = await fixture('file'); await f.run(); f.calls.length = 0; f.writes.length = 0;
  const save = f.storage.fs.atomicWriteFile;
  f.storage.fs.atomicWriteFile = async () => { throw new Error('disk full'); };
  await f.run('remove-file', fileId);
  expect(store(f.client).fileReleases).toContain(fileId);
  expect(f.calls.some(call => call.op === 'composerAttachRemove')).toBe(false);
  f.storage.fs.atomicWriteFile = save;
  const original = f.native.later;
  f.native.later = async input => {
    if (obj(input).op === 'composerAttachRemove') {
      expect(obj(f.writes.at(-1)?.mobileNewTaskDrafts).fileReleases).toContain(fileId);
      expect(f.writes.at(-1)?.drafts).toMatchObject({ 'new-task:B': 'B' });
    }
    return original(input);
  };
  await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(f.calls.filter(call => call.op === 'composerAttachRemove')).toHaveLength(1);
  expect(store(f.client).fileReleases).toEqual([]);
  expect(obj(f.writes.at(-1)?.mobileNewTaskDrafts).fileReleases).toEqual([]);
});
test('removal flush retains bytes while another draft still references the same file', async () => {
  const f = await fixture('file'); await f.run();
  const file = draftFiles(f.client.local)[0];
  const { setDraftFiles } = await import('./shared/composer-editor-files');
  setDraftFiles(f.client.local, [file, { ...file, draftKey: 'new-task:B' }]);
  f.calls.length = 0;
  await f.run('remove-file', fileId);
  expect(f.calls.some(call => call.op === 'composerAttachRemove')).toBe(false);
  expect(store(f.client).fileReleases).toContain(fileId);
  expect(draftFiles(f.client.local).some(file => file.draftKey === 'new-task:B')).toBe(true);
});
test('pending capture guard preserves the attachment action release intent through a failed first save', async () => {
  const f = await fixture('file'); await f.run();
  const attached = draftFiles(f.client.local).find(file => file.id === fileId)!;
  expect(attached.source).toBe('attached');
  const owner = { origin: f.client.origin, environmentId: f.client.environmentId,
    threadId: 'pending-thread', messageId: 'pending-message', commandId: 'pending-command' };
  const baseline: MobilePendingTaskMarker = { version: 1, owner, session: 'pending-editor-session', revision: 1,
    draftKey: 'new-task:pending-pending-message', contentRevision: 0, pending: null,
    baseline: { token: 'old-epoch:1', revision: 1, record: { schemaVersion: 1, ...owner, text: 'Original', attachments: [],
      createdAt: '2026-10-08T00:00:00.000Z', creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } } } };
  expect(mobilePendingTaskEditorsCreate(f.client, baseline)).not.toBeNull();
  const captured: MobilePendingTaskMarker = { ...baseline, revision: 2, contentRevision: 1,
    pending: { mutationId: 'old-epoch:2', contentRevision: 1, request: { ownerEpoch: 'old-epoch', mutationId: 'old-epoch:2',
      messageId: owner.messageId, operation: 'update', expectedToken: baseline.baseline.token,
      expectedRevision: baseline.baseline.revision, requireUnheld: false,
      record: { ...baseline.baseline.record, text: f.client.draft, attachments: [{ id: attached.id, kind: 'file',
        contextId: attached.contextId, source: attached.source, name: attached.name, mimeType: attached.mimeType,
        sizeBytes: attached.sizeBytes, uploadId: '', status: 'staged' }] } } } };
  expect(mobilePendingTaskEditorsReplace(f.client, baseline, captured)).not.toBeNull();
  expect(mobilePendingTaskEditorsSnapshot(f.client)).toMatchObject({ ready: true, markers: [captured] });
  f.calls.length = 0; f.writes.length = 0;
  const save = f.storage.fs.atomicWriteFile;
  f.storage.fs.atomicWriteFile = async () => { throw new Error('disk full'); };
  await expect(f.client.persist(f.storage)).rejects.toThrow('disk full');
  // Drive the real action: shared code drops the row before asking its guarded
  // Native to release bytes, and catches that removal's retained failure.
  await f.run('remove-file', fileId);
  expect(draftFiles(f.client.local).some(file => file.id === fileId)).toBe(false);
  expect(store(f.client).fileReleases).toEqual([fileId]);
  expect(f.calls.some(call => call.op === 'composerAttachRemove')).toBe(false);
  expect(f.writes).toEqual([]);
  expect(mobilePendingTaskEditorsSnapshot(f.client).markers).toEqual([captured]);
  f.storage.fs.atomicWriteFile = save;
  await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(store(f.client).fileReleases).toEqual([fileId]);
  expect(f.calls.some(call => call.op === 'composerAttachRemove')).toBe(false);
  expect(obj(f.writes.at(-1)?.mobileNewTaskDrafts).fileReleases).toEqual([fileId]);
  // Resolve this never-admitted request through the exact marker CAS. The old
  // queue baseline owns no file, so no remaining capture may keep this retry.
  const resolved = { ...captured, revision: 3, pending: null };
  expect(mobilePendingTaskEditorsReplace(f.client, captured, resolved)).not.toBeNull();
  expect(mobilePendingTaskEditorsRemove(f.client, resolved)).toBe(true);
  const original = f.native.later;
  f.native.later = async input => {
    if (obj(input).op === 'composerAttachRemove') {
      expect(obj(f.writes.at(-1)?.mobileNewTaskDrafts).fileReleases).toEqual([fileId]);
      expect(obj(f.writes.at(-1)?.mobilePendingTaskEditors).markers).toEqual({});
    }
    return original(input);
  };
  await f.client.flushSnapshotReleases(f.native, f.storage);
  await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(f.calls.filter(call => call.op === 'composerAttachRemove')).toHaveLength(1);
  expect(store(f.client).fileReleases).toEqual([]);
  expect(obj(f.writes.at(-1)?.mobileNewTaskDrafts).fileReleases).toEqual([]);
});
