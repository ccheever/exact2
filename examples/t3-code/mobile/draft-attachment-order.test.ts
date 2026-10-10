import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles, mobileAdoptRecoveredDraft } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftBind, mobileNewTaskDraftPresentation } from './mobile-new-task-drafts';
import { mobileComposerAttachmentAction, mobileComposerAttachments } from './composer-attachments';
import { mobileDraftAttachmentIds, mobileDraftAttachmentRecord, mobileDraftAttachmentsForSend } from './draft-attachment-order';
import { draftFiles } from './shared/composer-editor-files';
import { arr, obj, str, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';

const image = { kind: 'image', id: '11111111-1111-4111-a111-111111111111', name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 50 };
const file = { kind: 'file', id: '22222222-2222-4222-a222-222222222222', name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 50 };
const secondFile = { ...file, id: '33333333-3333-4333-a333-333333333333', name: 'last.txt' };
async function fixture(document: Obj = { version: 1 }, independent = true) {
  const client = new MobileDraftClient(); let disk = JSON.stringify(document), picked: Obj[] = [], fail = false;
  const calls: Obj[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; }, async atomicWriteFile(_path, bytes) {
    if (fail) throw new Error('disk failure'); disk = new TextDecoder().decode(bytes);
  } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'request') return { ok: false, generation: client.generation, error: { kind: 'transport', message: 'reply lost', uncertain: true } };
    return { ok: true, generation: client.generation, value: request.op === 'composerAttachPick' ? { files: picked.map(value => ({ ...value })) }
      : request.op === 'status' ? { phase: 'disconnected' } : { applied: false } };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { environmentId: 'env', projectId: 'project', origin: 'https://draft.test', threadId: independent ? '' : 'thread' });
  if (independent) {
    if (!obj(obj(document.mobileNewTaskDrafts).records)['new-task:order']) mobileNewTaskDraftCreate(client,
      { id: 'order', environmentId: 'env', projectId: 'project', origin: client.origin, createdAt: '2026-10-08T00:00:00Z' });
    mobileNewTaskDraftBind(client, 'new-task:order', 'flow');
  }
  return { client, native, storage, calls, disk: () => obj(JSON.parse(disk)), fail(value: boolean) { fail = value; },
    async pick(...values: Obj[]) { picked = values; return mobileComposerAttachmentAction('files', '', native, storage, client); },
    remove: (kind: string, id: string) => mobileComposerAttachmentAction(`remove-${kind}`, id, native, storage, client) };
}
const names = (client: MobileDraftClient) => mobileComposerAttachments(client).items.map(item => item.name);

test('mixed picker batches and separate picks retain insertion order in ordinary and independent drafts', async () => {
  for (const independent of [false, true]) for (const batch of [false, true]) for (const picks of [[file, image], [image, file]]) {
    const f = await fixture(undefined, independent);
    if (batch) expect((await f.pick(...picks)).message).toBe('');
    else for (const pick of picks) expect((await f.pick(pick)).message).toBe('');
    expect(names(f.client)).toEqual(picks.map(item => item.name));
    expect(obj(f.disk().mobileAttachmentOrder)[f.client.draftKey]).toEqual(picks.map(item => item.id));
    if (independent) expect(mobileNewTaskDraftPresentation(f.client, f.client.draftKey)?.attachmentIds).toEqual(picks.map(item => item.id));
    const restarted = await fixture(f.disk(), independent);
    expect(names(restarted.client)).toEqual(picks.map(item => item.name));
  }
});

test('failed removal restores original position; successful removal and reinsertion append', async () => {
  const f = await fixture(); await f.pick(file, image, secondFile);
  f.fail(true); expect((await f.remove('image', image.id)).message).toContain('disk failure');
  expect(names(f.client)).toEqual([file.name, image.name, secondFile.name]);
  f.fail(false); expect((await f.remove('image', image.id)).message).toBe('');
  expect(names(f.client)).toEqual([file.name, secondFile.name]);
  expect((await f.pick(image)).message).toBe('');
  expect(names(f.client)).toEqual([file.name, secondFile.name, image.name]);
  expect((await fixture(f.disk())).client.local.snapshotDrafts['new-task:order']).toHaveLength(1);
});

test('acceptance order is visible to reentrant persistence before editor await and stays with captured draft', async () => {
  const f = await fixture(), original = f.native.later; let savedOrder: unknown;
  f.native.later = async input => {
    if (obj(input).op === 'editorInsert') {
      await f.client.persist(f.storage); savedOrder = obj(f.disk().mobileAttachmentOrder)['new-task:order'];
      mobileNewTaskDraftCreate(f.client, { id: 'other', environmentId: 'env', projectId: 'project', origin: f.client.origin, createdAt: '2026-10-08T00:00:01Z' });
      mobileNewTaskDraftBind(f.client, 'new-task:other', 'flow-other');
    }
    return original(input);
  };
  expect((await f.pick(file, image)).message).toBe('');
  expect(savedOrder).toEqual([file.id, image.id]);
  expect(mobileNewTaskDraftPresentation(f.client, 'new-task:order')?.attachmentIds).toEqual([file.id, image.id]);
  expect(mobileDraftAttachmentIds(f.client, 'new-task:other')).toEqual([]);
  const restarted = await fixture(f.disk()); expect(names(restarted.client)).toEqual([file.name, image.name]);
});

test('fresh launch and message dispatch use local order, while uncertain retries keep captured payload', async () => {
  for (const independent of [false, true]) {
    const f = await fixture(undefined, independent); await f.pick(file, image, secondFile);
    f.client.local.snapshotDrafts[f.client.draftKey][0].uploadId = 'image-upload';
    for (const entry of draftFiles(f.client.local)) { entry.attachmentId = entry.id === file.id ? 'file-upload' : 'last-upload'; entry.status = 'ready'; }
    const attachments = [{ type: 'image', id: 'image-upload' }, { type: 'file', id: 'file-upload' }, { type: 'file', id: 'last-upload' }];
    const body = { text: f.client.draft, attachments };
    const pending = { method: independent ? 'orchestration.launchThread' : 'orchestration.dispatchCommand',
      payload: independent ? { commandId: 'command', projectId: 'project', threadId: 'thread', initialMessage: body }
        : { ...body, type: 'message.dispatch', commandId: 'command', threadId: 'thread' },
      description: 'Send', threadId: 'thread', text: f.client.draft, uncertain: false };
    await expect(f.client.write(f.native, f.storage, pending)).rejects.toThrow('reply lost');
    const first = obj(f.calls.filter(call => call.op === 'request').at(-1)?.payload);
    expect(arr(independent ? obj(first.initialMessage).attachments : first.attachments).map(item => item.id)).toEqual(['file-upload', 'image-upload', 'last-upload']);
    mobileDraftAttachmentRecord(f.client, f.client.draftKey, [secondFile.id, image.id, file.id]);
    await expect(f.client.write(f.native, f.storage, { ...f.client.local.pending.env!, uncertain: false })).rejects.toThrow('reply lost');
    expect(f.calls.filter(call => call.op === 'request').at(-1)?.payload).toEqual(first);
  }
});

test('same remote asset used twice keeps both ordered occurrences and exact payload objects', async () => {
  const f = await fixture(); await f.pick(file, image, secondFile);
  f.client.local.snapshotDrafts[f.client.draftKey][0].uploadId = 'shared';
  for (const entry of draftFiles(f.client.local)) entry.attachmentId = entry.id === file.id ? 'shared' : 'last';
  const payload = [{ type: 'image', id: 'shared', name: image.name }, { type: 'file', id: 'shared', name: file.name }, { type: 'file', id: 'last', name: secondFile.name }];
  expect(mobileDraftAttachmentsForSend(f.client, f.client.draftKey, payload)).toEqual([payload[1], payload[0], payload[2]]);
  mobileDraftAttachmentRecord(f.client, f.client.draftKey, [file.id, secondFile.id, image.id]);
  expect(mobileDraftAttachmentsForSend(f.client, f.client.draftKey, payload)).toEqual([payload[1], payload[2], payload[0]]);
});

test('recovered queued edit preserves its mixed attachment order across storage reload', async () => {
  const f = await fixture(undefined, false);
  expect(mobileAdoptRecoveredDraft(f.client, { owner: 'edit', draftKey: 'env:thread~queued-edit~run', origin: f.client.origin,
    environmentId: 'env', projectId: 'project', threadId: 'thread', generation: f.client.generation, session: 'session', revision: 1,
    runId: 'run', messageId: 'message', text: `[notes](t3-context://v1/file/file_${file.id})`,
    context: { version: 1, records: [] }, existingAttachments: [], saving: false,
    attachments: [file, image].map(item => ({ ...item, kind: item.kind as 'file' | 'image', uploadId: '', status: 'staged' as const })) })).toBe('kept');
  expect(names(f.client)).toEqual([file.name, image.name]);
  await f.client.persist(f.storage); expect(names((await fixture(f.disk(), false)).client)).toEqual([file.name, image.name]);
});

test('rejected picks never enter order, and malformed order metadata cannot create byte ownership', async () => {
  const f = await fixture();
  const invalid = { ...image, id: '44444444-4444-4444-a444-444444444444', mimeType: 'image/svg+xml' };
  const tooBig = { ...secondFile, sizeBytes: 51 * 1024 * 1024 };
  expect((await f.pick(file, invalid, image, tooBig)).message).toContain('limit');
  expect(names(f.client)).toEqual([file.name, image.name]);
  const saved = f.disk(); saved.mobileAttachmentOrder = { 'new-task:order': [file.id, 'unknown', file.id, image.id], unrelated: [image.id], invalid: [false] };
  const restarted = await fixture(saved); expect(names(restarted.client)).toEqual([file.name, image.name]);
  await restarted.client.persist(restarted.storage);
  expect(restarted.disk().mobileAttachmentOrder).toEqual({ 'new-task:order': [file.id, image.id] });
  expect(restarted.calls.some(call => str(call.op).endsWith('Remove'))).toBe(false);
});

test('reordered captured file references map duplicate remote assets to their original local occurrences', async () => {
  const f = await fixture(); await f.pick(file, image, secondFile);
  for (const entry of draftFiles(f.client.local)) entry.attachmentId = 'shared';
  f.client.local.snapshotDrafts[f.client.draftKey][0].uploadId = 'image';
  const captured = `[last](t3-context://v1/file/file_${secondFile.id}) [first](t3-context://v1/file/file_${file.id})`;
  const payload = [{ type: 'image', id: 'image', name: image.name },
    { type: 'file', id: 'shared', name: secondFile.name }, { type: 'file', id: 'shared', name: file.name }];
  expect(mobileDraftAttachmentsForSend(f.client, f.client.draftKey, payload, captured)).toEqual([payload[2], payload[0], payload[1]]);
  expect(f.client.draft.indexOf(file.id)).toBeLessThan(f.client.draft.indexOf(secondFile.id));
});

test('ordinary image insertion fallback stays with the captured thread when another opens', async () => {
  const f = await fixture(undefined, false), original = f.native.later;
  f.client.local.drafts['env:thread'] = 'alpha'; f.client.local.drafts['env:other'] = 'beta';
  f.native.later = async input => { if (obj(input).op === 'editorInsert') f.client.threadId = 'other'; return original(input); };
  expect((await f.pick(image)).message).toBe('');
  expect(f.client.local.drafts['env:other']).toBe('beta');
  expect(f.client.local.drafts['env:thread']).toContain(image.id);
  expect(mobileDraftAttachmentIds(f.client, 'env:thread')).toEqual([image.id]);
  expect(mobileDraftAttachmentIds(f.client, 'env:other')).toEqual([]);
});
