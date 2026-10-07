import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { type Native, type Files } from './shared/protocol';
import { mobileComposerAttachmentAction, mobileComposerAttachments, mobileComposerAttachmentPreviews } from './composer-attachments';

const imageId = '11111111-1111-4111-a111-111111111111';
const fileId = '22222222-2222-4222-a222-222222222222';
function fixture(files: Obj[] = []) {
  const client = new T3Client(); client.environmentId = 'env'; client.projectId = 'project'; client.origin = 'http://example.test';
  const calls: Obj[] = [], writes: string[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile(path) { writes.push(path); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const value = request.op === 'composerAttachPick' ? { files } : request.op === 'mobileAttachmentPreview' ? { dataUrl: 'data:image/jpeg;base64,thumbnail' } : { applied: false };
    return { ok: true, generation: client.generation, value };
  } };
  return { client, calls, writes, native, storage };
}
const pick = (f: ReturnType<typeof fixture>, source = 'photos', id = '') => mobileComposerAttachmentAction(source, id, f.native, f.storage, f.client);

describe('mobile native attachment bridge over shared draft ownership', () => {
  test('preserves actual JPEG metadata and shared file chip ownership, persists and previews real ids', async () => {
    const f = fixture([{ kind: 'image', id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 123 },
      { kind: 'file', id: fileId, name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 17 }]);
    expect(await pick(f)).toMatchObject({ message: '' });
    expect(f.client.snapshotDrafts[0]).toMatchObject({ id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg' });
    expect(f.client.draft).toContain('t3-context://v1/file/');
    expect(mobileComposerAttachments(f.client).items.map(item => item.kind)).toEqual(['image', 'file']);
    expect(f.writes.length).toBeGreaterThan(0);
    expect(f.calls.find(call => call.op === 'composerAttachPick')).toMatchObject({ source: 'photos', remaining: 100, fileLimit: 50 * 1024 * 1024 });
    await mobileComposerAttachmentPreviews(f.native, f.client);
    expect(mobileComposerAttachments(f.client).items[0]?.preview).toBe('data:image/jpeg;base64,thumbnail');
    expect(f.calls.filter(call => call.op === 'mobileAttachmentPreview').map(call => call.id)).toEqual([imageId]);
  });
  test('draft switch during native picker releases staged bytes and does not attach to either draft', async () => {
    const f = fixture([{ kind: 'image', id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 123 }]);
    const original = f.native.later;
    f.native.later = async input => { const response = await original(input); if (obj(input).op === 'composerAttachPick') f.client.projectId = 'other'; return response; };
    expect((await pick(f)).message).toContain('draft changed');
    expect(f.client.local.snapshotDrafts['env:new:project'] ?? []).toHaveLength(0);
    expect(f.client.snapshotDrafts).toHaveLength(0);
    expect(f.calls.some(call => call.op === 'snapshotDraftRemove' && call.id === imageId)).toBe(true);
  });
  test('100 attachment and pending submission guards do not open native picker', async () => {
    const f = fixture(); f.client.local.snapshotDrafts[f.client.draftKey] = Array.from({ length: 100 }, (_, n) => ({ id: String(n) }));
    expect(mobileComposerAttachments(f.client).canPick).toBe(false);
    expect((await pick(f)).message).toContain('100'); expect(f.calls).toHaveLength(0);
    f.client.local.snapshotDrafts[f.client.draftKey] = []; f.client.busy = true;
    expect((await pick(f)).message).toContain('submission'); expect(f.calls).toHaveLength(0);
  });
  test('unsupported image metadata is refused and its owned bytes released', async () => {
    const f = fixture([{ kind: 'image', id: imageId, name: 'bad.svg', mimeType: 'image/svg+xml', sizeBytes: 123 }]);
    expect((await pick(f)).message).toContain('not a supported image type');
    expect(f.client.snapshotDrafts).toHaveLength(0);
    expect(f.calls.some(call => call.op === 'snapshotDraftRemove' && call.id === imageId)).toBe(true);
  });
  test('capability changes reject staged files and release their unreferenced bytes', async () => {
    const f = fixture([{ kind: 'file', id: fileId, name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 17 }]);
    const original = f.native.later;
    f.native.later = async input => {
      const response = await original(input);
      if (obj(input).op === 'composerAttachPick') f.client.config = { environment: { capabilities: { attachmentUploads: false } } };
      return response;
    };
    expect((await pick(f, 'files')).message).toContain('does not support');
    expect(mobileComposerAttachments(f.client).items).toHaveLength(0);
    expect(f.calls.some(call => call.op === 'composerAttachRemove' && call.id === fileId)).toBe(true);
  });
  test('persistence failure reports the error and preserves accepted bytes and editable draft ownership', async () => {
    const f = fixture([{ kind: 'image', id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 123 }]);
    f.storage.fs.atomicWriteFile = async () => { throw new Error('Disk write failed'); };
    expect((await pick(f)).message).toContain('Disk write failed');
    expect(f.client.snapshotDrafts[0]?.id).toBe(imageId);
    expect(f.calls.some(call => call.op === 'snapshotDraftRemove')).toBe(false);
  });
  test('removal uses shared durable removal then native byte cleanup', async () => {
    const f = fixture([{ kind: 'image', id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 123 }]);
    await pick(f); expect((await pick(f, 'remove-image', imageId)).message).toBe('');
    expect(f.client.snapshotDrafts).toHaveLength(0);
    expect(f.calls.some(call => call.op === 'snapshotDraftRemove' && call.id === imageId)).toBe(true);
  });
});

test('ordinary removal refuses a changed target during shared command preamble', async () => {
  const f = fixture([{ kind: 'image', id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 123 }]);
  await pick(f);
  const priorKey = f.client.draftKey, nextKey = 'env:new:other';
  f.client.local.snapshotDrafts[nextKey] = f.client.snapshotDrafts.map(image => ({ ...image }));
  const original = f.native.later;
  f.native.later = async input => {
    const reply = await original(input);
    if (obj(input).op === 'devicePresentation') f.client.projectId = 'other';
    return reply;
  };
  await pick(f, 'remove-image', imageId);
  expect(f.client.local.snapshotDrafts[priorKey]).toHaveLength(1);
  expect(f.client.local.snapshotDrafts[nextKey]).toHaveLength(1);
  expect(f.calls.some(call => call.op === 'snapshotDraftRemove' || call.method === 'attachments.delete')).toBe(false);
});
