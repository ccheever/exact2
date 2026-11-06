import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftStore as store,
  mobileNewTaskDraftChoicesUpdate as choices, mobileNewTaskDraftDiscard as discard } from './mobile-new-task-drafts';
import { mobileNewTaskContextAttach as attach, mobileNewTaskContextGuard as guard, mobileNewTaskContextWrite as write,
  mobileNewTaskContextRead as read, type MobileNewTaskAttachmentBatch, type MobileNewTaskAttachmentResult } from './mobile-new-task-context';
import { mobileDraftAttachmentIds, mobileDraftAttachmentRecord } from './draft-attachment-order';
import { draftFiles, setDraftFiles } from './shared/composer-editor-files';
import { contextId, contextLink } from './shared/composer-editor-menu';
import type { OrdinaryInventoryAttachment } from './composer-attachment-publication';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';

const A = 'new-task:A', B = 'new-task:B', destination = { origin: 'https://batch.test', environmentId: 'env', projectId: 'project' };
const id = (n: number) => `10000000-0000-4000-a000-${String(n).padStart(12, '0')}`;
const image = (n: number): Extract<OrdinaryInventoryAttachment, { type: 'image' }> => ({ type: 'image', id: id(n),
  image: { id: id(n), name: `photo-${n}.png`, mimeType: 'image/png', sizeBytes: n + 1 } });
const file = (n: number): Extract<OrdinaryInventoryAttachment, { type: 'file' }> => ({ type: 'file', id: id(n), file: {
  id: id(n), contextId: contextId('file', id(n)), draftKey: A, environmentId: 'env', name: `file-${n}.txt`,
  mimeType: 'text/plain', sizeBytes: n + 1, source: 'attached', attachmentId: '', status: 'staged' } });
const mention = (n: number): Obj => ({ version: 1, kind: 'mention', contextId: `mention_${n}`, label: `File ${n}`, path: `${n}.ts` });
const link = (row: Obj) => contextLink(String(row.kind), String(row.contextId), String(row.label));
function attachmentLink(row: OrdinaryInventoryAttachment): string {
  return row.type === 'file' ? contextLink('file', row.file.contextId, row.file.name)
    : contextLink('image', contextId('image', row.id), String(row.image.name));
}
function successful(result: MobileNewTaskAttachmentResult) {
  if (result.kind !== 'committed') throw new Error(`Batch refused: ${result.reason}`);
  return result.commit;
}
async function fixture(document: Obj = { version: 1 }) {
  const client = new MobileDraftClient(); let disk = JSON.stringify(document), failure = false;
  const calls: unknown[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { if (failure) throw new Error('disk full'); disk = new TextDecoder().decode(bytes); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    calls.push(input); return { ok: true, generation: 1, value: obj(input).op === 'status' ? { phase: 'disconnected' } : {} };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage);
  await client.refresh(handles.native, handles.storage);
  Object.assign(client, { ...destination, threadId: '', generation: 1 });
  if (!store(client).records[A]) for (const name of ['A', 'B']) create(client, { id: name, ...destination, createdAt: '2026-10-28T00:00:00.000Z' });
  bind(client, A, 'owner-A');
  const batch = (incoming: OrdinaryInventoryAttachment[], text = client.local.drafts[A] ?? '', start = text.length, end = start): MobileNewTaskAttachmentBatch => {
    const captured = guard(client, A); if (!captured) throw new Error('Missing draft');
    return { guard: captured, destination, insertion: { text, start, end }, incoming };
  };
  return { client, storage, calls, batch, disk: () => obj(JSON.parse(disk)), fail(value = true) { failure = value; } };
}
function edit(client: MobileDraftClient, text: string, added?: Obj) {
  const captured = guard(client, A); if (!captured) throw new Error('Missing draft');
  expect(write(client, captured, text, added)).toBe(true);
}
function install(client: MobileDraftClient, attachments: OrdinaryInventoryAttachment[]) {
  client.local.snapshotDrafts[A] = attachments.flatMap(row => row.type === 'image' ? [row.image] : []);
  setDraftFiles(client.local, attachments.flatMap(row => row.type === 'file' ? [row.file] : []));
  mobileDraftAttachmentRecord(client, A, attachments.map(row => row.id));
}
function unchanged(client: MobileDraftClient, action: () => MobileNewTaskAttachmentResult) {
  const before = structuredClone(client.local), revision = client.revision, record = store(client).records[A];
  const result = action();
  expect(client.local).toEqual(before); expect(client.revision).toBe(revision); expect(store(client).records[A]).toBe(record);
  return result;
}

test('one offscreen publication preserves latest record metadata, mixed inventory and files-before-images references', async () => {
  const f = await fixture(), incoming = [image(1), file(2), image(3), file(4)], record = store(f.client).records[A];
  const video = incoming[1]; if (video.type !== 'file') throw new Error('Expected file');
  Object.assign(video.file, { contextId: 'kept_file_context', mimeType: 'video/mp4', videoWidth: 1920, videoHeight: 1080, extra: { lane: 'file' } });
  Object.assign(record, { branchChoice: { kind: 'explicit', envMode: 'local', branch: 'feature', worktreePath: '' }, extra: { preserved: [1, 2] } });
  edit(f.client, 'review this'); const batch = f.batch(incoming), beforeRevision = record.revision;
  expect(choices(f.client, A, { runtimeMode: 'latest-setting' })).toBe(true);
  bind(f.client, B, 'owner-B'); f.client.local.drafts[B] = 'focused draft';
  Object.assign(f.client, { ...destination, environmentId: 'different-env', projectId: 'different-project' });
  const beforeClientRevision = f.client.revision, commit = successful(attach(f.client, batch));
  const expected = `review this ${attachmentLink(incoming[1])} ${attachmentLink(incoming[3])} ${attachmentLink(incoming[0])} ${attachmentLink(incoming[2])} `;
  expect(commit.after.text).toBe(expected); expect(commit.selection).toEqual({ start: expected.length, end: expected.length });
  expect(commit.acceptedIds).toEqual(incoming.map(row => row.id)); expect(commit.discardedIds).toEqual([]);
  expect(mobileDraftAttachmentIds(f.client, A)).toEqual(incoming.map(row => row.id));
  expect(commit.after.files[0]).toEqual(video.file); expect(commit.after.images).toEqual([obj(incoming[0].type === 'image' ? incoming[0].image : null), obj(incoming[2].type === 'image' ? incoming[2].image : null)]);
  expect(commit.after.record).toMatchObject({ choices: { runtimeMode: 'latest-setting' }, extra: { preserved: [1, 2] }, branchChoice: { branch: 'feature' } });
  expect(store(f.client).records[A]).toBe(record); expect(record.revision).toBe(beforeRevision + 1);
  expect(f.client.revision).toBe(beforeClientRevision + 1); expect(f.client.local.drafts[B]).toBe('focused draft');
  expect(read(f.client, A)).toMatchObject({ context: { records: [
    { kind: 'file', contextId: 'kept_file_context', attachmentId: id(2) }, { kind: 'file', attachmentId: id(4) },
    { kind: 'image', contextId: contextId('image', id(1)), attachmentId: id(1) }, { kind: 'image', attachmentId: id(3) }] } });
  expect(unchanged(f.client, () => attach(f.client, batch))).toEqual({ kind: 'refused', reason: 'stale' });
  await f.client.persist(f.storage);
  const savedRecord = obj(obj(obj(f.disk().mobileNewTaskDrafts).records)[A]);
  expect(savedRecord).toEqual(commit.after.record);
  const reloaded = await fixture(f.disk());
  expect(reloaded.client.local.drafts[A]).toBe(expected); expect(read(reloaded.client, A)).toEqual(read(f.client, A));
  expect(mobileDraftAttachmentIds(reloaded.client, A)).toEqual(incoming.map(row => row.id));
  expect(draftFiles(reloaded.client.local)[0]).toMatchObject({ videoWidth: 1920, videoHeight: 1080, contextId: 'kept_file_context' });
  obj(commit.after.record).extra = 'changed copy'; commit.after.images[0].name = 'changed copy';
  expect(obj(record).extra).toEqual({ preserved: [1, 2] }); expect(f.client.local.snapshotDrafts[A][0].name).toBe('photo-1.png');
});

test('empty, whitespace and reference-only captures keep images on the shelf without replacing a selection', async () => {
  for (const source of ['', ' \n ', link(mention(1))]) {
    const f = await fixture(); if (source.includes('t3-context:')) edit(f.client, source, mention(1)); else edit(f.client, source);
    const context = structuredClone(store(f.client).records[A].context), commit = successful(attach(f.client, f.batch([image(1), image(2)], source, 0, source.length)));
    expect(commit.after.text).toBe(source); expect(commit.selection).toBeNull(); expect(commit.after.record.context).toEqual(context);
    expect(commit.after.attachmentIds).toEqual([id(1), id(2)]); expect(commit.removed).toEqual([]);
  }
});

test('capture decides image policy before file insertion; a files-only reference never turns shelf images into references', async () => {
  const f = await fixture(), incoming = [image(1), file(2), image(3)];
  const commit = successful(attach(f.client, f.batch(incoming, '', 0, 0)));
  expect(commit.after.text).toBe(`${attachmentLink(incoming[1])} `);
  expect(read(f.client, A)).toMatchObject({ context: { records: [{ kind: 'file', attachmentId: id(2) }] } });
  expect(commit.after.attachmentIds).toEqual([id(1), id(2), id(3)]);
});

test('shelf-only append preserves selected legacy file references without inventing or pruning context', async () => {
  const f = await fixture(), old = file(1), text = attachmentLink(old);
  install(f.client, [old]); f.client.local.drafts[A] = text;
  const commit = successful(attach(f.client, f.batch([image(2)], text, 0, text.length)));
  expect(commit.after.text).toBe(text); expect(commit.after.record.context).toBeUndefined(); expect(commit.selection).toBeNull();
  expect(commit.after.files).toEqual([old.file]); expect(commit.after.attachmentIds).toEqual([id(1), id(2)]);
  expect(commit.removed).toEqual([]); expect(store(f.client).fileReleases).toEqual([]);
});

test('captured UTF-16 range replaces emoji, same-text ABA keeps the range and changed text appends', async () => {
  for (const current of ['a😀z', 'changed']) {
    const f = await fixture(), attachment = image(1), source = { text: 'a😀z', start: 1, end: 3 };
    edit(f.client, source.text); edit(f.client, 'intervening'); edit(f.client, current);
    Object.assign(f.client, { composerSelection: { start: 0, end: 0 } });
    const commit = successful(attach(f.client, { ...f.batch([attachment]), insertion: source }));
    const expected = current === source.text ? `a ${attachmentLink(attachment)} z` : `changed ${attachmentLink(attachment)} `;
    expect(commit.after.text).toBe(expected);
    const cursor = current === source.text ? expected.length - 1 : expected.length;
    expect(commit.selection).toEqual({ start: cursor, end: cursor });
    expect(obj(f.client).composerSelection).toEqual({ start: 0, end: 0 });
  }
  const f = await fixture(); edit(f.client, 'word');
  const commit = successful(attach(f.client, f.batch([file(2)], 'word', -9, 900)));
  expect(commit.after.text).toBe(`${attachmentLink(file(2))} `);
});

test('final replacement capacity admits a mixed prefix and queues only removed files after synthesizing legacy references', async () => {
  const f = await fixture(), oldFiles = [file(201), file(202)];
  install(f.client, [...Array.from({ length: 98 }, (_, i) => image(i + 1)), ...oldFiles]);
  const text = oldFiles.map(attachmentLink).join(' '); f.client.local.drafts[A] = text;
  const incoming = [image(301), file(302), image(303)], commit = successful(attach(f.client, f.batch(incoming, text, 0, text.length)));
  expect(commit.acceptedIds).toEqual([id(301), id(302)]); expect(commit.discardedIds).toEqual([id(303)]);
  expect(commit.after.attachmentIds).toHaveLength(100); expect(commit.after.attachmentIds.slice(-2)).toEqual([id(301), id(302)]);
  expect(commit.removed).toEqual(oldFiles); expect(store(f.client).fileReleases).toEqual(oldFiles.map(row => row.id));
  expect(commit.after.text).toBe(`${attachmentLink(incoming[1])} `); expect(f.client.local.snapshotReleases).toEqual([]);
  const calls = f.calls.length; await f.client.persist(f.storage); expect(f.calls).toHaveLength(calls);
  expect(obj(f.disk().mobileNewTaskDrafts).fileReleases).toEqual(oldFiles.map(row => row.id));
});

test('context capacity uses the projected replacement and never skips a rejected prefix row', async () => {
  const f = await fixture(), records = Array.from({ length: 200 }, (_, i) => mention(i)), text = records.map(link).join(' ');
  f.client.local.drafts[A] = text; store(f.client).records[A].context = { version: 1, records };
  const shelf = successful(attach(f.client, f.batch([image(1), file(2), image(3)], text)));
  expect(shelf.acceptedIds).toEqual([id(1)]); expect(shelf.discardedIds).toEqual([id(2), id(3)]);
  expect(shelf.after.text).toBe(text); expect(shelf.selection).toBeNull();
  const selected = link(records[0]), commit = successful(attach(f.client, f.batch([file(4), file(5)], text, 0, selected.length)));
  expect(commit.acceptedIds).toEqual([id(4)]); expect(commit.discardedIds).toEqual([id(5)]);
  const context = read(f.client, A); if (!context.ok) throw new Error(context.error);
  expect(context.context?.records).toHaveLength(200); expect(context.context?.records.some(row => row.contextId === records[0].contextId)).toBe(false);
});

test('all-rejected or empty batch leaves every store and revision unchanged', async () => {
  const f = await fixture(); install(f.client, Array.from({ length: 100 }, (_, i) => image(i + 1))); edit(f.client, '  ');
  const batch = f.batch([image(101)], '  ', 0, 2), commit = successful(unchanged(f.client, () => attach(f.client, batch)));
  expect(commit.applied).toBe(false); expect(commit.acceptedIds).toEqual([]); expect(commit.discardedIds).toEqual([id(101)]);
  expect(commit.selection).toBeNull(); expect(commit.removed).toEqual([]);
  expect(successful(unchanged(f.client, () => attach(f.client, f.batch([])))).applied).toBe(false);
  mobileDraftAttachmentRecord(f.client, A, [id(2), id(1)]);
  const empty = successful(unchanged(f.client, () => attach(f.client, f.batch([]))));
  expect(empty.after.images).toEqual(f.client.local.snapshotDrafts[A]);
  expect(empty.after.attachmentIds.slice(0, 2)).toEqual([id(2), id(1)]);
});

test('complete malformed context refuses before pruning and ordinary typing retains its raw value', async () => {
  const circular: Obj = mention(1); circular.extra = circular;
  const sparse: unknown[] = []; sparse.length = 2;
  const bad = [null, { version: 1, records: [mention(1), mention(1)] }, { version: 1, records: [{ ...mention(1), path: '' }] },
    { version: 1, records: [{ ...mention(1), extra: undefined }] }, { version: 1, records: [{ ...mention(1), extra: NaN }] },
    { version: 1, records: [circular] }, { version: 1, records: sparse }];
  for (const raw of bad) {
    const f = await fixture(); f.client.local.drafts[A] = 'kept'; const record = store(f.client).records[A]; record.context = raw;
    expect(unchanged(f.client, () => attach(f.client, f.batch([image(1)])))).toEqual({ kind: 'refused', reason: 'invalid-context' });
    expect(record.context).toBe(raw);
  }
  const f = await fixture(); store(f.client).records[A].context = { version: 3, records: [] };
  edit(f.client, 'ordinary typing'); expect(store(f.client).records[A].context).toEqual({ version: 3, records: [] });
});

test('whole raw inventory, duplicate IDs and foreign file owners refuse without partial acceptance', async () => {
  const corruptions: Array<(f: Awaited<ReturnType<typeof fixture>>) => void> = [
    f => Object.assign(f.client.local, { composerFiles: null }),
    f => Object.assign(f.client.local, { mobileAttachmentOrder: null }),
    f => Object.assign(f.client.local, { mobileAttachmentOrder: { other: [id(9), id(9)] } }),
    f => { f.client.local.snapshotDrafts[B] = [{ id: 'bad' }]; },
    f => { f.client.local.snapshotDrafts[A] = [image(1).image]; setDraftFiles(f.client.local, [file(1).file]); },
    f => Object.assign(store(f.client), { fileReleases: 'bad' }),
    f => Object.assign(store(f.client), { claims: { slot: 1 } }),
  ];
  for (const corrupt of corruptions) {
    const f = await fixture(); corrupt(f);
    expect(unchanged(f.client, () => attach(f.client, f.batch([image(10), file(11)])))).toEqual({ kind: 'refused', reason: 'invalid-inventory' });
  }
  const f = await fixture(), foreign = file(2); if (foreign.type !== 'file') throw new Error('Expected file');
  foreign.file.draftKey = B;
  for (const incoming of [[image(1), image(1)], [image(1), foreign]])
    expect(unchanged(f.client, () => attach(f.client, f.batch(incoming)))).toEqual({ kind: 'refused', reason: 'invalid-inventory' });
  install(f.client, [image(1)]);
  expect(unchanged(f.client, () => attach(f.client, f.batch([file(1)])))).toEqual({ kind: 'refused', reason: 'invalid-inventory' });
});

test('stale revision, incarnation, destination, reserved keys and exhausted revisions refuse', async () => {
  const f = await fixture(), batch = f.batch([image(1)]); edit(f.client, 'one'); edit(f.client, '');
  expect(unchanged(f.client, () => attach(f.client, batch))).toEqual({ kind: 'refused', reason: 'stale' });
  const current = f.batch([image(1)]);
  expect(unchanged(f.client, () => attach(f.client, { ...current, destination: { ...destination, origin: 'https://foreign.test' } }))).toEqual({ kind: 'refused', reason: 'stale' });
  for (const key of ['env:ordinary', 'new-task:pending-A'])
    expect(unchanged(f.client, () => attach(f.client, { ...current, guard: { ...current.guard, key } }))).toEqual({ kind: 'refused', reason: 'invalid-target' });
  discard(f.client, A); create(f.client, { id: 'A', ...destination, createdAt: '2026-10-28T00:00:01.000Z' });
  store(f.client).records[A].revision = current.guard.revision;
  expect(unchanged(f.client, () => attach(f.client, current))).toEqual({ kind: 'refused', reason: 'stale' });
  store(f.client).records[A].revision = Number.MAX_SAFE_INTEGER;
  expect(unchanged(f.client, () => attach(f.client, f.batch([image(1)])))).toEqual({ kind: 'refused', reason: 'exhausted' });
});

test('missing owner and malformed raw text never initialize or partially publish a draft', async () => {
  const client = new MobileDraftClient(), before = structuredClone(client.local), revision = client.revision;
  expect(attach(client, { guard: { key: A, createdAt: '2026-10-28T00:00:00.000Z', revision: 0 }, destination,
    insertion: { text: '', start: 0, end: 0 }, incoming: [image(1)] })).toEqual({ kind: 'refused', reason: 'invalid-inventory' });
  expect(client.local).toEqual(before); expect(client.revision).toBe(revision);
  const f = await fixture(), batch = f.batch([image(1)]);
  Object.assign(f.client.local.drafts, { [A]: null });
  expect(unchanged(f.client, () => attach(f.client, batch))).toEqual({ kind: 'refused', reason: 'invalid-context' });
});

test('binding mismatches and unsupported selected file cleanup refuse without seeding an accepted prefix', async () => {
  const f = await fixture(), old = file(1); install(f.client, [old]);
  const text = attachmentLink(old); f.client.local.drafts[A] = text;
  store(f.client).records[A].context = { version: 1, records: [{ version: 1, kind: 'file', contextId: old.file.contextId,
    label: old.file.name, attachmentId: old.id, name: 'different.txt', mimeType: old.file.mimeType, sizeBytes: old.file.sizeBytes }] };
  expect(unchanged(f.client, () => attach(f.client, f.batch([image(2), file(3)], text, 0, text.length)))).toEqual({ kind: 'refused', reason: 'invalid-context' });
  delete store(f.client).records[A].context; old.file.source = 'pasted-text';
  expect(unchanged(f.client, () => attach(f.client, f.batch([image(2), file(3)], text, 0, text.length)))).toEqual({ kind: 'refused', reason: 'unsupported' });
});

test('unreferenced mismatched context cannot remove or queue a live file during insertion', async () => {
  const f = await fixture(), old = file(1);
  install(f.client, [old]); f.client.local.drafts[A] = 'review this';
  store(f.client).records[A].context = { version: 1, records: [{ version: 1, kind: 'file', contextId: old.file.contextId,
    label: old.file.name, attachmentId: old.id, name: 'wrong.txt', mimeType: old.file.mimeType, sizeBytes: old.file.sizeBytes }] };
  expect(unchanged(f.client, () => attach(f.client, f.batch([image(2)])))).toEqual({ kind: 'refused', reason: 'invalid-context' });
  expect(draftFiles(f.client.local)).toEqual([old.file]);
  expect(store(f.client).fileReleases).toEqual([]);
});

test('batch keeps prior Undo metadata, restores historical screenshot dependencies and never restores missing bytes', async () => {
  const f = await fixture(), picture = image(1), pictureContext = { version: 1, kind: 'image', contextId: 'historic_photo',
    label: 'Photo', attachmentId: picture.id, name: 'photo-1.png', mimeType: 'image/png', sizeBytes: 2 };
  const annotation = { version: 1, kind: 'preview-annotation', contextId: 'annotation', label: 'Note', annotationId: 'n', pageUrl: '',
    pageTitle: null, comment: '', targetSummary: '', styleChanges: [], elements: [], screenshotContextId: 'historic_photo' };
  install(f.client, [picture]); edit(f.client, link(pictureContext), pictureContext);
  edit(f.client, `${link(pictureContext)} ${link(annotation)}`, annotation); edit(f.client, '');
  f.client.local.snapshotDrafts[A] = []; f.client.local.drafts[A] = link(annotation);
  const commit = successful(attach(f.client, f.batch([file(2)])));
  expect(read(f.client, A)).toMatchObject({ context: { records: [pictureContext, annotation, { kind: 'file', attachmentId: id(2) }] } });
  expect(commit.after.images).toEqual([]); expect(f.client.local.snapshotDrafts[A]).toEqual([]);
  const text = commit.after.text; edit(f.client, ''); edit(f.client, text);
  expect(read(f.client, A)).toMatchObject({ context: { records: [pictureContext, annotation, { kind: 'file', attachmentId: id(2) }] } });
});

test('refusal and all-rejected batches cannot seed payloads or advance Undo eviction order', async () => {
  for (const mode of ['refused', 'all-rejected']) {
    const f = await fixture(), a = mention(1), b = mention(2);
    store(f.client).records[A].context = { version: 1, records: [a, b] }; edit(f.client, '');
    store(f.client).records[A].context = { version: 1, records: [a] };
    if (mode === 'refused') expect(unchanged(f.client, () => attach(f.client, f.batch([image(1), image(1)]))).kind).toBe('refused');
    else {
      install(f.client, Array.from({ length: 100 }, (_, i) => image(i + 1)));
      expect(successful(unchanged(f.client, () => attach(f.client, f.batch([image(101)])))).applied).toBe(false);
    }
    store(f.client).records[A].context = { version: 1, records: Array.from({ length: 199 }, (_, i) => mention(i + 10)) };
    edit(f.client, ''); edit(f.client, link(b)); expect(read(f.client, A)).toMatchObject({ context: { records: [b] } });
    edit(f.client, link(a)); expect(read(f.client, A)).toEqual({ ok: true, context: undefined });
  }
});

test('failed persistence retains the committed batch; a save retry persists it once without replaying acceptance', async () => {
  const f = await fixture(), batch = f.batch([image(1), file(2)]), commit = successful(attach(f.client, batch));
  f.fail(); await expect(f.client.persist(f.storage)).rejects.toThrow('disk full');
  expect(mobileDraftAttachmentIds(f.client, A)).toEqual(commit.after.attachmentIds);
  expect(unchanged(f.client, () => attach(f.client, batch))).toEqual({ kind: 'refused', reason: 'stale' });
  f.fail(false); await f.client.persist(f.storage); const reloaded = await fixture(f.disk());
  expect(read(reloaded.client, A)).toEqual(read(f.client, A)); expect(guard(reloaded.client, A)?.revision).toBe(commit.after.record.revision);
  expect(mobileDraftAttachmentIds(reloaded.client, A)).toEqual([id(1), id(2)]);
});
