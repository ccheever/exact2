// A draft image's upload at send (client.ts uploadDraftImage), for a server that stages no uploads, so none started as
// the image was attached (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: apps/web/src/lib/attachmentUploadQueue.ts): a
// later send reuses the attachment it made. A staging server's uploads are composer-uploads.test.ts's.
import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { initialShell, obj, str, type Obj } from '../../src/client/domain';
import { fleet } from '../../src/connections/settings-b-fleet';
import type { Files, Native } from '../../src/client/protocol';
import { memoryFiles, png } from '../support/app-files-fixture';

const id = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
const draft = `app:/data/snapshots/drafts/${id}.png`;

function fixture(refuseUpload = false) {
  const client = new T3Client();
  Object.assign(client, { generation: 1, environmentId: 'env', projectId: 'p', threadId: 't', connection: 'connected', scopes: ['orchestration:read', 'orchestration:operate'] });
  memoryFiles({ [draft]: png(4, 3, 20) });
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'uploadAttachment' && refuseUpload) return { ok: false, generation: 1, error: { kind: 'HTTP', message: 'Upload rejected', uncertain: false } };
    const value = request.method === 'attachments.createUploadUrl' ? { attachmentId: 'att-1', relativeUrl: '/api/attachments/upload/att-1.signed', expiresAt: 999999 } : {};
    return { ok: true, generation: 1, value };
  } };
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  const image: Obj = { id, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 53, source: {} };
  client.local.snapshotDrafts[client.draftKey] = [image];
  return { client, native, files, image, calls };
}

test('an image uploads, and Send reuses its attachment', async () => {
  const f = fixture();
  expect(await f.client.uploadDraftImage(f.native, f.files, f.image)).toBe('att-1');
  expect(f.image.uploadId).toBe('att-1');
  const before = f.calls.length;
  expect(await f.client.uploadSnapshots(f.native, f.files)).toMatchObject([{ type: 'image', id: 'att-1' }]);
  expect(f.calls.slice(before).filter(call => call.op === 'uploadAttachment')).toEqual([]);
});

test('a refused upload fails with its reason, and its attachment is deleted', async () => {
  const f = fixture(true);
  await expect(f.client.uploadDraftImage(f.native, f.files, f.image)).rejects.toThrow('Upload rejected');
  expect(f.calls.at(-1)).toMatchObject({ op: 'request', method: 'attachments.delete', payload: { attachmentId: 'att-1' } });
  expect(f.image.uploadId).toBeUndefined();
});

test('a second ask for an image whose upload runs joins that upload (attachmentUploadQueue.ts jobsByImageId)', async () => {
  const f = fixture();
  const [first, second] = await Promise.all([f.client.uploadDraftImage(f.native, f.files, f.image), f.client.uploadDraftImage(f.native, f.files, f.image)]);
  expect([first, second]).toEqual(['att-1', 'att-1']);
  expect(f.calls.filter(call => call.op === 'uploadAttachment' || call.method === 'attachments.createUploadUrl')).toHaveLength(2);
});

// startAttachmentUpload and cancelAttachmentUpload: the image's upload belongs to one environment.
test('an image that moves to another environment uploads there; the old upload stops, and its attachment is deleted where it was minted', async () => {
  const f = fixture();
  fleet.entries.set('old', { key: 'old', origin: 'http://old.example', environmentId: 'env', phase: 'connected', message: '', traceId: '', generation: 1, synchronized: 1,
    lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: ['orchestration:read', 'orchestration:operate'], error: '', requested: true });
  try {
    let release: () => void = () => {};
    const held = new Promise<void>(resolve => { release = resolve; });
    const later = f.native.later.bind(f.native);
    let minted = 0;
    f.native.later = async (input: unknown) => {
      const request = obj(input);
      if (request.method === 'attachments.createUploadUrl') {
        f.calls.push(request); const attachmentId = `att-${++minted}`;
        return { ok: true, generation: 1, value: { attachmentId, relativeUrl: `/api/attachments/upload/${attachmentId}.signed`, expiresAt: 999999 } };
      }
      if (request.op === 'uploadAttachment' && str(request.path).includes('att-1')) { f.calls.push(request); await held; return { ok: true, generation: 1, value: {} }; }
      return later(input);
    };
    const moving = f.client.uploadDraftImage(f.native, f.files, f.image).catch((error: unknown) => error);
    for (let turn = 0; turn < 20 && !f.calls.some(call => call.op === 'uploadAttachment'); turn++) await Promise.resolve();
    f.client.environmentId = 'env-2';
    expect(await f.client.uploadDraftImage(f.native, f.files, f.image)).toBe('att-2');
    release();
    expect(await moving).toMatchObject({ kind: 'superseded' });
    // Deleted once, through the old environment's own connection; the image keeps the new upload.
    expect(f.calls.filter(call => call.method === 'attachments.delete').map(call => [obj(call.payload).attachmentId, call.fleet ?? 'focused'])).toEqual([['att-1', 'old']]);
    expect(f.image.uploadId).toBe('att-2');
  } finally { fleet.entries.delete('old'); }
});
