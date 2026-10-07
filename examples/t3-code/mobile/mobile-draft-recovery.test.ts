import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileAdoptRecoveredDraft, mobileDraftRecoveryHandles, mobileRecoveredDraftMarker,
  mobileRecoveredMessageContext, mobileRetireRecoveredDraft } from './mobile-draft-recovery';
import { obj, arr, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import type { MobileQueuedEditSession } from './queued-edit-state';
import { draftFiles } from './shared/composer-editor-files';
import { omitExpiredTerminalContexts } from './shared/terminal-integrations';
import { queuedEditRefreshOrigin } from './queued-edit-origin';
const imageId = '11111111-1111-4111-8111-111111111111';
const link = (kind: string, id: string) => `[${id}](t3-context://v1/${kind}/${id})`;
function setup(document: Obj = { version: 1 }) {
  const client = new MobileDraftClient(); Object.assign(client, { origin: 'https://recovery.test', environmentId: 'e', threadId: 't', projectId: 'p', generation: 3 });
  let disk = JSON.stringify(document); const writes: Obj[] = [], calls: Obj[] = []; let uncertain = false;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { disk = new TextDecoder().decode(bytes); writes.push(obj(JSON.parse(disk))); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'request' && uncertain) return { ok: false, generation: 3, error: { kind: 'transport', message: 'reply lost', uncertain: true } };
    return { ok: true, generation: 3, value: request.op === 'status' ? { phase: 'disconnected' } : {} };
  } };
  const load = async () => {
    const handles = mobileDraftRecoveryHandles(client, native, storage);
    await client.refresh(handles.native, handles.storage);
    Object.assign(client, { origin: 'https://recovery.test', environmentId: 'e', threadId: 't', projectId: 'p', generation: 3 });
  };
  return { client, storage, native, load, writes, calls, disk: () => obj(JSON.parse(disk)), uncertain: () => { uncertain = true; } };
}
function recovery(overrides: Partial<MobileQueuedEditSession> = {}): MobileQueuedEditSession & { recoveryKey: string } {
  return { owner: 'edit-one', draftKey: 'e:t~queued-edit~run', recoveryKey: 'e:t', origin: 'https://recovery.test', environmentId: 'e', threadId: 't',
    projectId: 'p', generation: 3, session: 'epoch-one', revision: 2, runId: 'run', messageId: 'message', text: 'Recovered',
    context: { version: 1, records: [] }, attachments: [], existingAttachments: [], saving: false, ...overrides };
}
test('actual shared load restores all supported image MIME before first native presentation or persistence', async () => {
  const images = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'].map((mimeType, i) => ({ id: `${i + 1}`.repeat(8) + '-1111-4111-8111-111111111111', mimeType, sizeBytes: 10, name: `image-${i}` }));
  const f = setup({ version: 1, snapshotDrafts: { 'e:t': images } }), original = f.native.later; let presented: unknown;
  f.native.later = async input => { if (obj(input).op === 'devicePresentation') { presented = f.client.local.snapshotDrafts['e:t']; await f.client.persist(f.storage); } return original(input); };
  await f.load(); expect(presented).toEqual(images); expect(obj(f.disk().snapshotDrafts)['e:t']).toEqual(images);
  expect(f.client.local.snapshotDrafts['e:t']).toEqual(images);
});
test('hydration rejects unsupported MIME/size/id and never replays old disk on later refresh', async () => {
  const good = { id: imageId, mimeType: 'image/jpeg', sizeBytes: 42 };
  const f = setup({ version: 1, snapshotDrafts: { 'e:t': [good, { ...good, mimeType: 'image/heic' }, { ...good, sizeBytes: 11 * 1024 * 1024 }, { ...good, id: 'bad' }] } });
  await f.load(); expect(f.client.local.snapshotDrafts['e:t']).toEqual([good]);
  f.client.local.snapshotDrafts['e:t'] = []; await f.load(); expect(f.client.local.snapshotDrafts['e:t']).toEqual([]);
});
test('source keep rule recovers unchanged nonempty text and new attachments only', () => {
  const f = setup(), edit = recovery({ attachments: [{ id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 42, kind: 'image', uploadId: '', status: 'staged' }],
    existingAttachments: [{ id: 'remote-original', type: 'image' }] });
  expect(mobileAdoptRecoveredDraft(f.client, edit)).toBe('kept'); expect(f.client.draft).toBe('Recovered');
  expect(f.client.snapshotDrafts).toEqual([{ id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 42 }]);
  expect(mobileAdoptRecoveredDraft(f.client, edit)).toBe('already-adopted');
});
test('ordinary text, image or file prevents recovery and foreign identity never writes', () => {
  for (const occupied of ['text', 'image', 'file', 'origin', 'empty-edit']) {
    const f = setup(), edit = recovery();
    if (occupied === 'text') f.client.local.drafts['e:t'] = 'Existing';
    if (occupied === 'image') f.client.local.snapshotDrafts['e:t'] = [{ id: 'existing' }];
    if (occupied === 'file') Object.assign(f.client.local, { composerFiles: [{ draftKey: 'e:t' }] });
    if (occupied === 'origin') edit.origin = 'https://other.test';
    if (occupied === 'empty-edit') edit.text = ' ';
    const before = JSON.stringify(f.client.local); expect(mobileAdoptRecoveredDraft(f.client, edit)).toBe('blocked'); expect(JSON.stringify(f.client.local)).toBe(before);
  }
});
test('durable marker prevents restart resurrection after ordinary draft is cleared', async () => {
  const f = setup(); mobileAdoptRecoveredDraft(f.client, recovery()); f.client.local.drafts['e:t'] = ''; await f.client.persist(f.storage);
  const restarted = setup(f.disk()); await restarted.load();
  expect(mobileAdoptRecoveredDraft(restarted.client, recovery())).toBe('already-adopted'); expect(restarted.client.draft).toBe('');
});
test('native retirement keeps ordinary recovered review and terminal context usable', async () => {
  const f = setup(), text = `${link('review-comment', 'review_one')} ${link('terminal', 'terminal_one')}`;
  mobileAdoptRecoveredDraft(f.client, recovery({ text, context: { version: 1, records: [
    { kind: 'review-comment', contextId: 'review_one', comment: 'Preserve this' }, { kind: 'terminal', contextId: 'terminal_one', text: 'output' }] } }));
  mobileRetireRecoveredDraft(f.client, 'edit-one'); await f.client.persist(f.storage);
  const restarted = setup(f.disk()); await restarted.load();
  expect(mobileRecoveredDraftMarker(restarted.client, 'edit-one')?.nativeRetired).toBe(true);
  expect(omitExpiredTerminalContexts(restarted.client, text, false)).toEqual({ text, empty: false });
  expect(arr(mobileRecoveredMessageContext(restarted.client, 'e:t', text, [])?.records)).toHaveLength(2);
});
test('restarted recovery send persists and sends identical rebound context, excluding retained remote and removed references', async () => {
  const f = setup(), text = [link('file', 'file_local'), link('image', 'image_local'), link('image', 'image_remote'), link('review-comment', 'review_one')].join(' ');
  const fileId = 'local-file';
  mobileAdoptRecoveredDraft(f.client, recovery({ text, attachments: [
    { id: imageId, name: 'photo.webp', mimeType: 'image/webp', sizeBytes: 42, kind: 'image', uploadId: 'image-upload', status: 'ready' },
    { id: fileId, name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 8, kind: 'file', uploadId: 'file-upload', status: 'ready' }],
    existingAttachments: [{ id: 'remote-original', type: 'image' }], context: { version: 1, records: [
      { kind: 'file', contextId: 'file_local', attachmentId: fileId }, { kind: 'image', contextId: 'image_local', attachmentId: imageId },
      { kind: 'image', contextId: 'image_remote', attachmentId: 'remote-original' }, { kind: 'review-comment', contextId: 'review_one', comment: 'old' },
      { kind: 'review-comment', contextId: 'removed', comment: 'removed' }] } }));
  await f.client.persist(f.storage); const restarted = setup(f.disk()); await restarted.load();
  expect(draftFiles(restarted.client.local)[0]?.contextId).toBe('file_local');
  const payload: Obj = { type: 'message.dispatch', commandId: 'cmd-one', threadId: 't', text,
    attachments: [{ id: 'file-upload', type: 'file' }, { id: 'image-upload', type: 'image' }],
    context: { version: 1, records: [{ kind: 'review-comment', contextId: 'review_one', comment: 'fresh' }] } };
  await restarted.client.dispatch(restarted.native, restarted.storage, payload, 'Send');
  const wire = obj(restarted.calls.find(call => call.op === 'request')?.payload);
  const persisted = obj(obj(obj(restarted.writes[0]?.pending).e).payload);
  expect(wire).toEqual(persisted);
  expect(arr(obj(wire.context).records)).toEqual([{ kind: 'file', contextId: 'file_local', attachmentId: 'file-upload' },
    { kind: 'image', contextId: 'image_local', attachmentId: 'image-upload' }, { kind: 'review-comment', contextId: 'review_one', comment: 'fresh' }]);
});
test('removed local attachment context is omitted and preview screenshot dependency is retained', () => {
  const f = setup(), text = link('preview-annotation', 'annotation');
  mobileAdoptRecoveredDraft(f.client, recovery({ text, attachments: [{ id: imageId, name: 'a.png', mimeType: 'image/png', sizeBytes: 1, kind: 'image', uploadId: 'upload', status: 'ready' }],
    context: { version: 1, records: [{ kind: 'preview-annotation', contextId: 'annotation', screenshotContextId: 'shot' }, { kind: 'image', contextId: 'shot', attachmentId: imageId }] } }));
  expect(arr(mobileRecoveredMessageContext(f.client, 'e:t', text, [{ id: 'upload' }])?.records)).toHaveLength(2);
  f.client.local.snapshotDrafts['e:t'] = [];
  expect(arr(mobileRecoveredMessageContext(f.client, 'e:t', text, [{ id: 'upload' }])?.records)).toHaveLength(1);
});
test('uncertain write and restarted retry preserve immutable original composed payload', async () => {
  const f = setup(), text = link('review-comment', 'review_one');
  mobileAdoptRecoveredDraft(f.client, recovery({ text, context: { version: 1, records: [{ kind: 'review-comment', contextId: 'review_one', comment: 'captured' }] } }));
  f.uncertain(); await expect(f.client.dispatch(f.native, f.storage, { type: 'message.dispatch', commandId: 'retry-cmd', threadId: 't', text }, 'Send')).rejects.toThrow('reply lost');
  const original = obj(f.calls.find(call => call.op === 'request')?.payload), restarted = setup(f.disk()); await restarted.load();
  obj(obj(obj(restarted.client.local).mobileRecoveredDrafts)['edit-one']).context = { version: 1, records: [{ kind: 'review-comment', contextId: 'review_one', comment: 'changed' }] };
  const pending = restarted.client.local.pending.e!; await restarted.client.write(restarted.native, restarted.storage, { ...pending, uncertain: false });
  expect(restarted.calls.filter(call => call.op === 'request').at(-1)?.payload).toEqual(original);
});

test('confirmed native retirement prunes cleared context on persist and stays pruned after restart', async () => {
  const f = setup(), text = link('review-comment', 'review_one');
  mobileAdoptRecoveredDraft(f.client, recovery({ text, context: { version: 1, records: [{ kind: 'review-comment', contextId: 'review_one', comment: 'kept until clear' }] } }));
  mobileRetireRecoveredDraft(f.client, 'edit-one'); await f.client.persist(f.storage);
  expect(mobileRecoveredDraftMarker(f.client, 'edit-one')).not.toBeNull();
  f.client.local.drafts['e:t'] = ''; await f.client.persist(f.storage);
  expect(mobileRecoveredDraftMarker(f.client, 'edit-one')).toBeNull();
  const restarted = setup(f.disk()); await restarted.load(); expect(mobileRecoveredDraftMarker(restarted.client, 'edit-one')).toBeNull();
});
test('successful ordinary send prunes retired context after durable payload acknowledgment', async () => {
  const f = setup(), text = link('review-comment', 'review_one');
  mobileAdoptRecoveredDraft(f.client, recovery({ text, context: { version: 1, records: [{ kind: 'review-comment', contextId: 'review_one', comment: 'send' }] } }));
  mobileRetireRecoveredDraft(f.client, 'edit-one');
  await f.client.dispatch(f.native, f.storage, { type: 'message.dispatch', commandId: 'clear-after-send', threadId: 't', text }, 'Send');
  expect(arr(obj(obj(f.calls.find(call => call.op === 'request')?.payload).context).records)).toHaveLength(1);
  expect(mobileRecoveredDraftMarker(f.client, 'edit-one')).toBeNull(); expect(f.client.draft).toBe('');
});
test('unretired anti-replay marker survives clear, send and restart until native cleanup is confirmed', async () => {
  const f = setup(), text = link('review-comment', 'review_one');
  mobileAdoptRecoveredDraft(f.client, recovery({ text, context: { version: 1, records: [{ kind: 'review-comment', contextId: 'review_one', comment: 'pending cleanup' }] } }));
  await f.client.dispatch(f.native, f.storage, { type: 'message.dispatch', commandId: 'unretired-send', threadId: 't', text }, 'Send');
  expect(mobileRecoveredDraftMarker(f.client, 'edit-one')).not.toBeNull();
  const restarted = setup(f.disk()); await restarted.load();
  expect(mobileAdoptRecoveredDraft(restarted.client, recovery({ text }))).toBe('already-adopted'); expect(restarted.client.draft).toBe('');
  mobileRetireRecoveredDraft(restarted.client, 'edit-one'); await restarted.client.persist(restarted.storage);
  expect(mobileRecoveredDraftMarker(restarted.client, 'edit-one')).toBeNull();
});

test('verified same-home failover preserves recovered context while unverified or changed homes refuse it', async () => {
  const f = setup(), text = link('review-comment', 'review_one'), edit = recovery({ text,
    context: { version: 1, records: [{ kind: 'review-comment', contextId: 'review_one', comment: 'same environment' }] } });
  f.client.origin = 'https://alternate.test'; f.client.generation++;
  expect(mobileAdoptRecoveredDraft(f.client, edit)).toBe('blocked');
  const verified: Native = { available: true, watch() {}, async later() { return { ok: true, generation: f.client.generation,
    value: { origin: f.client.origin, environmentId: f.client.environmentId, homeOrigin: 'https://recovery.test' } }; } };
  await queuedEditRefreshOrigin(verified, f.client);
  expect(mobileAdoptRecoveredDraft(f.client, edit)).toBe('kept');
  expect(arr(mobileRecoveredMessageContext(f.client, 'e:t', text, [])?.records)).toHaveLength(1);
  f.client.origin = 'https://unverified.test'; f.client.generation++;
  expect(mobileRecoveredMessageContext(f.client, 'e:t', text, [])).toBeUndefined();
  const otherHome: Native = { ...verified, async later() { return { ok: true, generation: f.client.generation,
    value: { origin: f.client.origin, environmentId: f.client.environmentId, homeOrigin: 'https://different.test' } }; } };
  await queuedEditRefreshOrigin(otherHome, f.client);
  expect(mobileRecoveredMessageContext(f.client, 'e:t', text, [])).toBeUndefined();
});
