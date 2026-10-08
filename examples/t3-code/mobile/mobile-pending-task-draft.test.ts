// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobilePendingTaskDraftAdopt as adopt, mobilePendingTaskDraftBuildRecord as build,
  mobilePendingTaskDraftCleanup as cleanup, mobilePendingTaskDraftFingerprint as fingerprint,
  mobilePendingTaskDraftCleanupDocument as cleanupDocument } from './mobile-pending-task-draft';
import { mobileNewTaskDraftStore as store, mobileNewTaskDraftPresentation as presentation,
  mobileNewTaskDraftBind as bind, mobileNewTaskDraftNoteBranch as noteBranch } from './mobile-new-task-drafts';
import { draftFiles } from './shared/composer-editor-files';
import { mobileDraftAttachmentRecord } from './draft-attachment-order';
import { branchState } from './shared/r4-git-branch';
import { contextLink } from './shared/composer-editor-menu';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import { mobilePendingTaskEditorsCreate, type MobilePendingTaskMarker } from './mobile-pending-task-state';

const imageId = '11111111-1111-4111-8111-111111111111', fileId = '22222222-2222-4222-8222-222222222222';
const extraId = '33333333-3333-4333-8333-333333333333';
const key = 'new-task:pending-message-A', origin = 'https://pending.test';
function record(): MobileOutboxRecord {
  return { schemaVersion: 1, origin, environmentId: 'env', threadId: 'thread-A', messageId: 'message-A', commandId: 'command-A',
    text: 'Original task', createdAt: '2026-10-08T01:02:03.000Z', modelSelection: { instanceId: 'provider', model: 'model', options: [{ id: 'effort', value: 'high' }] },
    runtimeMode: 'full-access', interactionMode: 'plan', dispatchMode: 'queue',
    creation: { projectId: 'project', projectTitle: 'Captured title', projectCwd: '/captured/project', workspaceMode: 'worktree', branch: 'base', worktreePath: null, startFromOrigin: false },
    attachments: [{ id: fileId, kind: 'file', contextId: 'file_context', name: 'clip.mp4', mimeType: 'video/mp4', sizeBytes: 40,
      uploadId: 'remote-file', uploadEnvironmentId: 'env', status: 'ready', source: 'attached', videoWidth: 120, videoHeight: 80 },
      { id: imageId, kind: 'image', name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 20,
        uploadId: 'remote-image', uploadEnvironmentId: 'env', status: 'ready', source: { _tag: 'screenshot' } }] };
}
function marker(value = record()): MobilePendingTaskMarker {
  return { version: 1, owner: { origin: value.origin, environmentId: value.environmentId, threadId: value.threadId, messageId: value.messageId, commandId: value.commandId },
    session: 'session', revision: 1, contentRevision: 0, draftKey: `new-task:pending-${value.messageId}`,
    baseline: { record: value, token: 'token', revision: 3 }, pending: null };
}
async function fixture(document: Obj = { version: 1 }) {
  const client = new MobileDraftClient(); let disk = JSON.stringify(document);
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { disk = new TextDecoder().decode(bytes); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    return { ok: true, generation: 1, value: obj(input).op === 'status' ? { phase: 'disconnected' } : {} };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { environmentId: 'env', origin, projectId: 'project', threadId: '', generation: 1 });
  return { client, storage, disk: () => obj(JSON.parse(disk)) };
}
function ready(client: MobileDraftClient, value = marker()) {
  const result = build(client, value);
  if (result.status !== 'ready') throw new Error(result.reason);
  return result;
}

test('adoption uses actual owners, preserves file/image chronology and original creation, with defensive input/output copies', async () => {
  const f = await fixture(), source = record();
  expect(adopt(f.client, source)).toEqual({ draftKey: key, adopted: true, reason: '' });
  expect(f.client.local.drafts[key]).toBe(source.text);
  expect(draftFiles(f.client.local)[0]).toMatchObject({ id: fileId, draftKey: key, environmentId: 'env', attachmentId: 'remote-file', videoWidth: 120 });
  expect(presentation(f.client, key)?.attachmentIds).toEqual([fileId, imageId]);
  expect(ready(f.client).record).toEqual(source);
  source.text = 'mutated input'; source.attachments[0].name = 'mutated file'; source.creation!.projectCwd = '/mutated';
  const capture = ready(f.client); capture.record.attachments[0].name = 'mutated output';
  expect(ready(f.client).record).toEqual(record());
  expect(f.client.threadId).toBe(''); expect(f.client.draftKey).not.toBe(key);
});
test('reopen retains even emptied newer editor content and rejects conflicting or orphaned owners', async () => {
  const f = await fixture(); adopt(f.client, record()); f.client.local.drafts[key] = '';
  expect(adopt(f.client, record())).toEqual({ draftKey: key, adopted: false, reason: '' });
  expect(f.client.local.drafts[key]).toBe(''); expect(build(f.client, marker())).toMatchObject({ status: 'blocked' });
  const conflict = record(); conflict.environmentId = 'another'; conflict.attachments = [];
  expect(adopt(f.client, conflict).reason).toContain('different');
  const orphan = await fixture(); orphan.client.local.drafts[key] = 'keep';
  expect(adopt(orphan.client, record()).reason).toContain('unresolved'); expect(orphan.client.local.drafts[key]).toBe('keep');
  expect(store(orphan.client).records[key]).toBeUndefined();
  const cold = new MobileDraftClient(); expect(adopt(cold, record()).reason).toContain('Read saved');
});
test('editing recaptures stored picks under original IDs despite unrelated active client defaults', async () => {
  const f = await fixture(); adopt(f.client, record());
  f.client.local.drafts[key] = '  Revised task  ';
  const draft = store(f.client).records[key]; draft.choices = { providerId: 'new-provider', modelId: 'new-model', modelOptions: [], runtimeMode: 'auto', interactionMode: 'default' };
  draft.branchChoice = { kind: 'explicit', envMode: 'local', branch: 'feature', worktreePath: '/worktrees/feature', startFromOrigin: true };
  f.client.local.composerControls.contexts[key] = { envMode: 'local', branch: 'feature', worktreePath: '/worktrees/feature' };
  branchState(f.client).origin.set(key, true);
  Object.assign(f.client, { projectId: 'unrelated', modelId: 'unrelated', environmentId: 'unrelated', threadId: 'unrelated' });
  expect(ready(f.client).record).toMatchObject({ origin, environmentId: 'env', threadId: 'thread-A', messageId: 'message-A', commandId: 'command-A',
    createdAt: record().createdAt, text: 'Revised task', modelSelection: { instanceId: 'new-provider', model: 'new-model' },
    runtimeMode: 'auto', interactionMode: 'default', creation: { projectId: 'project', projectTitle: 'Captured title', projectCwd: '/captured/project',
      branch: 'feature', workspaceMode: 'local', worktreePath: '/worktrees/feature', startFromOrigin: true } });
});
test('actual preferences JSON replay retains owners, order, context and explicit origin toggle', async () => {
  const f = await fixture(), original = record();
  original.text += ` ${contextLink('thread', 'related_thread', 'Related')}`;
  original.context = { version: 1, records: [{ version: 1, contextId: 'related_thread', kind: 'thread', label: 'Related', title: 'Related task', environmentId: 'env', threadId: 'related' }] };
  adopt(f.client, original);
  expect(bind(f.client, key, 'editor')).toBe(true); branchState(f.client).origin.set(key, true); noteBranch(f.client, 'explicit');
  await f.client.persist(f.storage);
  const cold = await fixture(f.disk()); expect(branchState(cold.client).origin.has(key)).toBe(false);
  expect(presentation(cold.client, key)?.attachmentIds).toEqual([fileId, imageId]);
  expect(ready(cold.client, marker(original)).record).toEqual({ ...original, creation: { ...original.creation!, startFromOrigin: true } });
  expect(adopt(cold.client, original)).toEqual({ draftKey: key, adopted: false, reason: '' });
  expect(branchState(cold.client).origin.get(key)).toBe(true);
  const saved = store(cold.client).records[key]; saved.branchChoice!.startFromOrigin = false;
  await cold.client.persist(cold.storage); const again = await fixture(cold.disk());
  expect(ready(again.client).record.creation?.startFromOrigin).toBe(false);
});
test('added local attachments and changed order are captured; forged upload IDs and cross-environment file owners block', async () => {
  const f = await fixture(); adopt(f.client, record());
  f.client.local.snapshotDrafts[key].push({ id: extraId, name: 'new.png', mimeType: 'image/png', sizeBytes: 5 });
  mobileDraftAttachmentRecord(f.client, key, [extraId, imageId, fileId]);
  expect(ready(f.client).record.attachments.map(file => [file.id, file.uploadId])).toEqual([[extraId, ''], [imageId, 'remote-image'], [fileId, 'remote-file']]);
  f.client.local.snapshotDrafts[key][1].uploadId = 'unproven';
  expect(build(f.client, marker())).toMatchObject({ status: 'blocked', reason: expect.stringContaining('upload owner') });
  delete f.client.local.snapshotDrafts[key][1].uploadId;
  draftFiles(f.client.local)[0].environmentId = 'other';
  expect(build(f.client, marker())).toMatchObject({ status: 'blocked', reason: expect.stringContaining('different draft') });
});
test('owner changes, invalid workspace provenance, settings and future/raw context fail closed without dropping content', async () => {
  const f = await fixture(); adopt(f.client, record());
  const stale = marker(); stale.owner.commandId = 'other'; expect(build(f.client, stale).status).toBe('blocked');
  const draft = store(f.client).records[key]; draft.context = { version: 7, records: [] };
  const invalidStamp = fingerprint(f.client, key); expect(build(f.client, marker()).status).toBe('blocked');
  expect(fingerprint(f.client, key)).toBe(invalidStamp); expect(draft.context).toEqual({ version: 7, records: [] });
  delete draft.context; draft.branchChoice = undefined;
  expect(build(f.client, marker()).status).toBe('ready'); // worktree's actual selected base is explicit in existing capture policy
  f.client.local.composerControls.contexts[key].envMode = 'local';
  expect(build(f.client, marker())).toMatchObject({ status: 'blocked', reason: expect.stringContaining('Confirm') });
  f.client.local.composerControls.contexts[key].branch = ''; draft.choices!.runtimeMode = 'invalid';
  expect(build(f.client, marker()).status).toBe('blocked');
});
test('cleanup guards raw context, metadata revisions, whitespace, attachment order and origin changes; removes only its exact owners', async () => {
  const f = await fixture(); adopt(f.client, record());
  f.client.local.drafts.other = 'keep'; f.client.local.snapshotDrafts.other = [{ id: extraId }];
  const first = ready(f.client).fingerprint; store(f.client).records[key].context = undefined;
  expect(cleanup(f.client, key, first)).toBe(false); delete store(f.client).records[key].context;
  expect(fingerprint(f.client, key)).toBe(first);
  store(f.client).records[key].revision++; expect(cleanup(f.client, key, first)).toBe(false);
  let guard = fingerprint(f.client, key)!; f.client.local.drafts[key] += ' '; expect(cleanup(f.client, key, guard)).toBe(false);
  guard = fingerprint(f.client, key)!; mobileDraftAttachmentRecord(f.client, key, [imageId, fileId]); expect(cleanup(f.client, key, guard)).toBe(false);
  guard = fingerprint(f.client, key)!; branchState(f.client).origin.set(key, true); expect(cleanup(f.client, key, guard)).toBe(false);
  const current = fingerprint(f.client, key)!; expect(cleanup(f.client, key, current)).toBe(true); expect(cleanup(f.client, key, current)).toBe(false);
  expect(presentation(f.client, key)).toBeNull(); expect(draftFiles(f.client.local)).toEqual([]);
  expect(f.client.local.snapshotReleases).toEqual([imageId]); expect(store(f.client).fileReleases).toEqual([fileId]);
  expect(f.client.local.drafts.other).toBe('keep'); expect(f.client.local.snapshotDrafts.other).toEqual([{ id: extraId }]);
});
test('cleanup does not clear a launch claimant; unreadable cyclic context blocks capture and cleanup', async () => {
  const f = await fixture(); adopt(f.client, record()); const guard = fingerprint(f.client, key)!;
  store(f.client).claims.capture = key; expect(cleanup(f.client, key, guard)).toBe(false); delete store(f.client).claims.capture;
  const cycle: Obj = {}; cycle.self = cycle; store(f.client).records[key].context = cycle;
  expect(fingerprint(f.client, key)).toBeNull(); expect(build(f.client, marker()).status).toBe('blocked'); expect(cleanup(f.client, key, guard)).toBe(false);
});
test('raw malformed values that JSON normalizes produce distinct cleanup guards', async () => {
  const f = await fixture(); adopt(f.client, record()); const draft = store(f.client).records[key];
  draft.context = { version: 99, records: [undefined] }; const undefinedStamp = fingerprint(f.client, key)!;
  draft.context = { version: 99, records: [null] }; expect(cleanup(f.client, key, undefinedStamp)).toBe(false);
  draft.context = { version: 99, records: [NaN] }; const nanStamp = fingerprint(f.client, key)!;
  draft.context = { version: 99, records: [null] }; expect(cleanup(f.client, key, nanStamp)).toBe(false);
});


test('cleanup document projects actual serialized owners while preserving live content, marker and unrelated fields', async () => {
  const f = await fixture(); adopt(f.client, record());
  expect(mobilePendingTaskEditorsCreate(f.client, marker())).not.toBeNull();
  f.client.local.drafts.other = 'unrelated'; f.client.local.snapshotDrafts.other = [{ id: extraId, name: 'keep' }];
  f.client.local.composerControls.contexts.other = { envMode: 'local', branch: 'keep', worktreePath: '' };
  f.client.local.snapshotReleases.push(imageId); store(f.client).fileReleases.push(fileId);
  await f.client.persist(f.storage);
  const document = f.disk(); document.futureExtension = { fields: ['preserve', 1] };
  const guard = fingerprint(f.client, key)!, before = JSON.stringify(document), liveBefore = JSON.stringify(f.client.local);
  const projected = cleanupDocument(f.client, key, guard, document);
  expect(projected).not.toBeNull(); if (!projected) throw new Error('Expected cleanup document');
  expect(JSON.stringify(document)).toBe(before); expect(JSON.stringify(f.client.local)).toBe(liveBefore);
  expect(fingerprint(f.client, key)).toBe(guard);
  expect(obj(projected.drafts)[key]).toBeUndefined(); expect(obj(projected.snapshotDrafts)[key]).toBeUndefined();
  expect(obj(obj(projected.mobileNewTaskDrafts).records)[key]).toBeUndefined();
  expect(obj(projected.mobileAttachmentOrder)[key]).toBeUndefined(); expect(obj(obj(projected.composerControls).contexts)[key]).toBeUndefined();
  expect(projected.composerFiles).toEqual([]); expect(projected.snapshotReleases).toEqual([imageId]);
  expect(obj(projected.mobileNewTaskDrafts).fileReleases).toEqual([fileId]);
  expect(projected.mobilePendingTaskEditors).toEqual(document.mobilePendingTaskEditors);
  expect(projected.futureExtension).toEqual(document.futureExtension);
  expect(obj(projected.drafts).other).toBe('unrelated'); expect(obj(projected.snapshotDrafts).other).toEqual([{ id: extraId, name: 'keep' }]);
  expect(obj(obj(projected.composerControls).contexts).other).toEqual(obj(obj(document.composerControls).contexts).other);
  expect(cleanup(f.client, key, guard)).toBe(true); await f.client.persist(f.storage);
  delete projected.futureExtension; expect(f.disk()).toEqual(projected);
});

test('cleanup document rejects stale live guards, stale serialized target owners, claims and malformed documents', async () => {
  const f = await fixture(); adopt(f.client, record()); await f.client.persist(f.storage); const guard = fingerprint(f.client, key)!;
  const mutations: ((document: Obj) => void)[] = [
    document => { obj(document.drafts)[key] = 'different'; },
    document => { obj(document.snapshotDrafts)[key] = []; },
    document => { document.composerFiles = []; },
    document => { obj(document.mobileAttachmentOrder)[key] = [imageId, fileId]; },
    document => { obj(obj(document.mobileNewTaskDrafts).records)[key] = {}; },
    document => { obj(obj(document.composerControls).contexts)[key] = {}; },
    document => { obj(obj(document.mobileNewTaskDrafts).claims).pending = key; },
    document => { document.snapshotReleases = null; },
    document => { document.version = 2; },
  ];
  for (const mutate of mutations) { const document = f.disk(); mutate(document); expect(cleanupDocument(f.client, key, guard, document)).toBeNull(); }
  f.client.local.drafts[key] += 'new'; expect(cleanupDocument(f.client, key, guard, f.disk())).toBeNull();
  expect(presentation(f.client, key)?.text).toBe('Original tasknew');
});

test('failed cleanup write retains actual live draft and existing durable document', async () => {
  const f = await fixture(); adopt(f.client, record()); await f.client.persist(f.storage);
  const guard = fingerprint(f.client, key)!, disk = f.disk(); let projected = false;
  const failing: Files = { fs: { ...f.storage.fs, async atomicWriteFile(_path, bytes) {
    const next = cleanupDocument(f.client, key, guard, obj(JSON.parse(new TextDecoder().decode(bytes))));
    expect(next).not.toBeNull(); projected = true; throw new Error('write failed');
  } } };
  await expect(f.client.persist(failing)).rejects.toThrow('write failed');
  expect(projected).toBe(true); expect(fingerprint(f.client, key)).toBe(guard); expect(f.disk()).toEqual(disk);
  expect(f.client.local.snapshotReleases).toEqual([]); expect(store(f.client).fileReleases).toEqual([]);
});

test('newer typing during awaited cleanup write survives and can be durably restored while marker stays held', async () => {
  const f = await fixture(); adopt(f.client, record()); mobilePendingTaskEditorsCreate(f.client, marker()); await f.client.persist(f.storage);
  const guard = fingerprint(f.client, key)!;
  let resume: (() => void) | undefined, entered: (() => void) | undefined;
  const wait = new Promise<void>(resolve => { resume = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  const delayed: Files = { fs: { ...f.storage.fs, async atomicWriteFile(path, bytes) {
    const next = cleanupDocument(f.client, key, guard, obj(JSON.parse(new TextDecoder().decode(bytes))));
    if (!next) throw new Error('Unexpected stale cleanup');
    expect(f.client.local.drafts[key]).toBe('Original task'); entered?.(); await wait;
    await f.storage.fs.atomicWriteFile(path, new TextEncoder().encode(JSON.stringify(next)));
  } } };
  const writing = f.client.persist(delayed); await started;
  f.client.local.drafts[key] = 'newer typing'; store(f.client).records[key].revision++;
  resume?.(); await writing;
  expect(cleanup(f.client, key, guard)).toBe(false); expect(f.client.local.drafts[key]).toBe('newer typing');
  expect(obj(f.disk().drafts)[key]).toBeUndefined(); expect(obj(f.disk().mobilePendingTaskEditors).markers).not.toEqual({});
  await f.client.persist(f.storage); expect(obj(f.disk().drafts)[key]).toBe('newer typing');
});
