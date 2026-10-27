// One draft image's upload, which the reference starts when the image is attached (T3 Code 7c3b3b3605ef, MIT, see
// LICENSE-T3: apps/web/src/lib/attachmentUploadQueue.ts, attachmentUploadState.ts): `attachmentUploads` says how it
// goes, and Send reuses the attachment it made. The host reports no upload progress, so it is 0 until the reply.
import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { memoryFiles, png } from '../support/app-files-fixture';

const id = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
const draft = `app:/data/snapshots/drafts/${id}.png`;

function fixture(refuseUpload = false) {
  const client = new T3Client();
  Object.assign(client, { generation: 1, environmentId: 'env', projectId: 'p', threadId: 't', connection: 'connected', scopes: ['orchestration:read', 'orchestration:operate'] });
  memoryFiles({ [draft]: png(4, 3, 20) });
  const calls: Obj[] = [];
  const seen: string[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'uploadAttachment') {
      seen.push(client.attachmentUploads[id]?.status ?? '');
      if (refuseUpload) return { ok: false, generation: 1, error: { kind: 'HTTP', message: 'Upload rejected', uncertain: false } };
    }
    const value = request.method === 'attachments.createUploadUrl' ? { attachmentId: 'att-1', relativeUrl: '/api/attachments/upload/att-1.signed', expiresAt: 999999 } : {};
    return { ok: true, generation: 1, value };
  } };
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  const image: Obj = { id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 53, source: {} };
  client.local.snapshotDrafts[client.draftKey] = [image];
  return { client, native, files, image, calls, seen };
}

test('an image uploads at once, reading uploading then ready, and Send reuses its attachment', async () => {
  const f = fixture();
  expect(await f.client.uploadDraftImage(f.native, f.files, f.image)).toBe('att-1');
  expect(f.seen).toEqual(['uploading']);
  expect(f.client.attachmentUploads[id]).toEqual({ status: 'ready', progress: 1, reason: '', attachmentId: 'att-1' });
  expect(f.image.uploadId).toBe('att-1');
  const before = f.calls.length;
  expect(await f.client.uploadSnapshots(f.native, f.files)).toMatchObject([{ type: 'image', id: 'att-1' }]);
  expect(f.calls.slice(before).filter(call => call.op === 'uploadAttachment')).toEqual([]);
});

test('a refused upload reads failed with its reason, and its attachment is deleted', async () => {
  const f = fixture(true);
  await expect(f.client.uploadDraftImage(f.native, f.files, f.image)).rejects.toThrow('Upload rejected');
  expect(f.client.attachmentUploads[id]).toEqual({ status: 'failed', progress: 0, reason: 'Upload rejected', attachmentId: '' });
  expect(f.calls.at(-1)).toMatchObject({ op: 'request', method: 'attachments.delete', payload: { attachmentId: 'att-1' } });
  expect(f.image.uploadId).toBeUndefined();
});
