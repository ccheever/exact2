import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { type Files, type Native } from './shared/protocol';
import { mobileComposerAttachmentUploadRetry, mobileComposerAttachmentUploadsStart, mobileComposerAttachmentUploadSnapshot } from './composer-attachment-uploads';

const imageId = '11111111-1111-4111-a111-111111111111';
const secondImageId = '22222222-2222-4222-a222-222222222222';
const thirdImageId = '33333333-3333-4333-a333-333333333333';
const fourthImageId = '44444444-4444-4444-a444-444444444444';
const imageIds = [imageId, secondImageId, thirdImageId, fourthImageId];
const turn = () => new Promise<void>(resolve => setTimeout(resolve, 0));

function fixture(count = 1, holdUploads = false) {
  const client = new T3Client();
  client.origin = 'https://example.test'; client.environmentId = 'env'; client.projectId = 'project'; client.connection = 'connected';
  client.scopes = ['orchestration:operate'];
  client.config = { environment: { capabilities: { attachmentUploads: true } } };
  const key = client.draftKey, ids = imageIds.slice(0, count);
  client.local.snapshotDrafts[key] = ids.map((id, index) => ({ id, name: `${index}.png`, mimeType: 'image/png', sizeBytes: 3 }));
  const calls: Obj[] = [], writes: string[] = [], failed = new Set<string>(), waiters: Array<() => void> = [];
  let activeUploads = 0, peakUploads = 0, persistFailure = false;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile(path) { if (persistFailure) throw new Error('disk full'); writes.push(path); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'snapshotDraftRead') return { ok: true, generation: client.generation, value: { base64: 'aGV5', sizeBytes: 3 } };
    if (request.op === 'request' && request.method === 'attachments.createUploadUrl') {
      const n = calls.filter(call => call.method === 'attachments.createUploadUrl').length;
      return { ok: true, generation: client.generation, value: { attachmentId: `upload-${n}`, relativeUrl: `/api/attachments/upload/${n}` } };
    }
    if (request.op === 'uploadAttachment') {
      const id = String(request.path).split('/').pop()!;
      if (failed.has(id)) return { ok: false, generation: client.generation, error: { kind: 'HTTP', message: 'Upload failed', uncertain: false } };
      activeUploads++; peakUploads = Math.max(peakUploads, activeUploads);
      if (holdUploads) return new Promise(resolve => waiters.push(() => { activeUploads--; resolve({ ok: true, generation: client.generation, value: {} }); }));
      activeUploads--;
    }
    return { ok: true, generation: client.generation, value: {} };
  } };
  return { client, native, storage, calls, writes, failed, key, waiters, peakUploads: () => peakUploads, setPersistFailure(value: boolean) { persistFailure = value; } };
}

describe('composer attachment upload queue', () => {
  test('adopts a successful upload before Send and records explicit provenance', async () => {
    const f = fixture();
    mobileComposerAttachmentUploadsStart(f.client, f.native, f.storage);
    for (let i = 0; i < 20 && !f.client.local.snapshotDrafts[f.key]![0]!.uploadId; i++) await turn();
    expect(f.client.local.snapshotDrafts[f.key]![0]).toMatchObject({ uploadId: 'upload-1', uploadEnvironmentId: 'env' });
    expect(mobileComposerAttachmentUploadSnapshot(f.client)).toMatchObject({ states: { [imageId]: 'ready' }, owners: { [imageId]: { origin: 'https://example.test', environmentId: 'env' } } });
    expect(f.writes.length).toBeGreaterThan(0);
    expect(f.calls.some(call => call.op === 'uploadAttachment')).toBe(true);
  });

  test('failed upload exposes Retry and a retry adopts the same draft', async () => {
    const f = fixture(); f.failed.add('1');
    mobileComposerAttachmentUploadsStart(f.client, f.native, f.storage);
    for (let i = 0; i < 20 && mobileComposerAttachmentUploadSnapshot(f.client).states[imageId] !== 'failed'; i++) await turn();
    expect(mobileComposerAttachmentUploadSnapshot(f.client)).toMatchObject({ states: { [imageId]: 'failed' }, errors: { [imageId]: 'Upload failed' } });
    expect(f.calls.some(call => call.op === 'request' && call.method === 'attachments.delete')).toBe(true);
    f.failed.clear();
    expect(mobileComposerAttachmentUploadRetry(f.client, f.native, f.storage, imageId)).toBe(true);
    for (let i = 0; i < 20 && mobileComposerAttachmentUploadSnapshot(f.client).states[imageId] !== 'ready'; i++) await turn();
    expect(f.client.local.snapshotDrafts[f.key]![0]!.uploadId).toBe('upload-2');
    expect(mobileComposerAttachmentUploadSnapshot(f.client).states[imageId]).toBe('ready');
  });

  test('does not invent upload ownership for an existing remote ID', () => {
    const f = fixture(); f.client.local.snapshotDrafts[f.key]![0]!.uploadId = 'legacy-id';
    mobileComposerAttachmentUploadsStart(f.client, f.native, f.storage);
    expect(mobileComposerAttachmentUploadSnapshot(f.client).states[imageId]).toBeUndefined();
    expect(mobileComposerAttachmentUploadSnapshot(f.client).owners[imageId]).toBeUndefined();
  });

  test('keeps three uploads active and drains the fourth when a worker settles', async () => {
    const f = fixture(4, true);
    mobileComposerAttachmentUploadsStart(f.client, f.native, f.storage);
    for (let i = 0; i < 20 && f.waiters.length < 3; i++) await turn();
    expect(f.waiters).toHaveLength(3);
    expect(f.peakUploads()).toBe(3);
    f.waiters.shift()!();
    for (let i = 0; i < 20 && f.waiters.length < 3; i++) await turn();
    expect(f.waiters).toHaveLength(3);
    expect(f.peakUploads()).toBe(3);
    while (f.waiters.length) f.waiters.shift()!();
    for (let i = 0; i < 20 && Object.values(mobileComposerAttachmentUploadSnapshot(f.client).states).some(state => state !== 'ready'); i++) await turn();
    expect(mobileComposerAttachmentUploadSnapshot(f.client).states).toEqual(Object.fromEntries(imageIds.map(id => [id, 'ready'])));
  });

  test('keeps a server ID after an uncertain local write and retries the durable adoption', async () => {
    const f = fixture(); f.setPersistFailure(true);
    mobileComposerAttachmentUploadsStart(f.client, f.native, f.storage);
    for (let i = 0; i < 20 && mobileComposerAttachmentUploadSnapshot(f.client).states[imageId] !== 'failed'; i++) await turn();
    expect(f.client.local.snapshotDrafts[f.key]![0]).toMatchObject({ uploadId: 'upload-1', uploadEnvironmentId: 'env' });
    f.setPersistFailure(false);
    expect(mobileComposerAttachmentUploadRetry(f.client, f.native, f.storage, imageId)).toBe(true);
    for (let i = 0; i < 20 && mobileComposerAttachmentUploadSnapshot(f.client).states[imageId] !== 'ready'; i++) await turn();
    expect(mobileComposerAttachmentUploadSnapshot(f.client).states[imageId]).toBe('ready');
    expect(f.client.local.snapshotDrafts[f.key]![0]!.uploadId).toBe('upload-1');
  });
});
