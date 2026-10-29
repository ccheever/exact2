// A draft image uploads as it joins the draft, three at once, each in an upload resource whose stream tells the bytes it
// has sent (composer-uploads.ts; attachmentUploadQueue.ts at T3 Code 7c3b3b3605ef).
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { adoptAppFiles } from '../../src/media/save-file';
import { retryDraftUpload, runDraftUploads, settleUploads, uploadBlock, uploadSource, uploadTable, type UploadEvent } from '../../src/composer/composer-uploads';
import type { Native } from '../../src/client/protocol';
import { contractComponent, contractFns } from '../support/contract-action-fixture';

type Obj = Record<string, unknown>;
const KEY = 'env-1:thread-1';
const native = {} as Native;

/** A connected client whose server stages uploads, with `count` draft images and a scripted server. */
function fixture(count: number, supported = true) {
  const requests: Array<[string, Obj]> = [];
  let minted = 0;
  const images: Obj[] = Array.from({ length: count }, (_, index) => ({ id: `img-${index}`, name: `shot-${index}.png`, mimeType: 'image/png', sizeBytes: 1 }));
  const client = {
    writable: true, environmentId: 'env-1', origin: 'http://127.0.0.1:16555', attachmentUploads: {} as Record<string, Obj>,
    config: { environment: { capabilities: { attachmentUploads: supported } } },
    local: { snapshotDrafts: { [KEY]: images } as Record<string, Obj[]> },
    get snapshotDrafts() { return this.local.snapshotDrafts[KEY] ?? []; },
    request: async (_native: Native, method: string, payload: Obj) => {
      requests.push([method, payload]);
      if (method === 'attachments.createUploadUrl') { minted += 1; return { attachmentId: `att-${minted}`, relativeUrl: `/api/attachments/upload/att-${minted}?token=t${minted}` }; }
      return {};
    },
  } as unknown as T3Client & { local: { snapshotDrafts: Record<string, Obj[]> } };
  adoptAppFiles({ fs: { readFile: async () => new ArrayBuffer(0), writeFile: async () => undefined, mkdir: async () => undefined, stat: async () => ({ size: 2_000_000 }) } });
  return { client, images, requests };
}

/** The fetch an upload resource makes: its URL and init, and its mapper. */
const fetched: Array<{ url: string; init: Obj }> = [];
const realFetch = globalThis.fetch;
globalThis.fetch = (async (url: string, init: Obj) => { fetched.push({ url, init }); return new Promise(() => {}); }) as unknown as typeof fetch;
afterEach(() => { fetched.length = 0; });
process.on('exit', () => { globalThis.fetch = realFetch; });
const stream = (slot: number, client: T3Client, table = uploadTable(client)) => {
  const at = [table.u0, table.u1, table.u2][slot]!;
  uploadSource(client, slot, at.id, at.attachmentId);
  const call = fetched.at(-1)!;
  return { ...call, event: (event: UploadEvent) => (call.init.exactStream as (e: UploadEvent) => Obj)(event) };
};

describe('a draft image uploads as it joins the draft', () => {
  test('nothing waits while the server stages no uploads', () => {
    const { client } = fixture(2, false);
    expect(uploadTable(client)).toMatchObject({ supported: false, pending: '', images: [] });
    expect(uploadBlock(client)).toBe('');
  });

  test('three at once: each free resource takes the next image, minting its upload URL; the rest wait at 0%', async () => {
    const { client, requests } = fixture(4);
    expect(uploadTable(client).pending).toBe('img-0 img-1 img-2 img-3');
    await runDraftUploads(client, native);
    expect(requests.map(([method, payload]) => [method, payload.type, payload.mimeType, payload.sizeBytes])).toEqual(Array(3).fill(['attachments.createUploadUrl', 'image', 'image/png', 2_000_000]));
    const table = uploadTable(client);
    expect([table.u0, table.u1, table.u2]).toEqual([{ id: 'img-0', attachmentId: 'att-1' }, { id: 'img-1', attachmentId: 'att-2' }, { id: 'img-2', attachmentId: 'att-3' }]);
    // No resource is free, so the root asks for none until one is.
    expect(table.pending).toBe('');
    expect(table.images.map(image => [image.id, image.status])).toEqual([['img-0', 'slot'], ['img-1', 'slot'], ['img-2', 'slot'], ['img-3', 'queued']]);
    expect(uploadBlock(client)).toBe('Attachments still uploading');
  });

  test('the stream posts the draft file and tells its progress in twentieths; its reply lands the attachment in the draft', async () => {
    const { client, images } = fixture(2);
    await runDraftUploads(client, native);
    const upload = stream(0, client);
    expect(upload.url).toBe('http://127.0.0.1:16555/api/attachments/upload/att-1?token=t1');
    expect(upload.init).toMatchObject({ method: 'POST', exactBodyFrom: 'app:/data/snapshots/drafts/img-0.png', exactUploadProgress: true, exactTimeout: 300000,
      headers: { 'Content-Type': 'image/png' } });
    expect(upload.event({ type: 'progress', loaded: 512_000, total: 2_000_000 })).toMatchObject({ id: 'img-0', status: 'uploading', progress: 0.256 });
    // A step of less than 5% keeps the answer the reference would show.
    expect(upload.event({ type: 'progress', loaded: 540_000, total: 2_000_000 })).toMatchObject({ progress: 0.256 });
    expect(upload.event({ type: 'load', status: 201 })).toMatchObject({ status: 'ready', progress: 1, attachmentId: 'att-1' });
    // Landed, it no longer holds the send, and the root's next run settles its resource.
    expect(uploadBlock(client)).toBe('Attachment still uploading');
    expect(uploadTable(client)).toMatchObject({ u0: { id: 'img-0', attachmentId: 'att-1' }, settle: 1 });
    expect(images[0]!.uploadId).toBeUndefined();
    await runDraftUploads(client, native);
    expect(images[0]!.uploadId).toBe('att-1');
    expect(uploadTable(client)).toMatchObject({ u0: { id: '', attachmentId: '' }, settle: 0 });
    expect(uploadTable(client).images.map(image => image.id)).toEqual(['img-1']);
  });

  test('a failed upload keeps its reason for the tile\'s Retry, its attachment is deleted, and Retry mints another', async () => {
    const { client, requests } = fixture(1);
    await runDraftUploads(client, native);
    expect(stream(0, client).event({ type: 'load', status: 500 })).toMatchObject({ status: 'failed', reason: 'Upload rejected (500)' });
    await runDraftUploads(client, native);
    expect(requests.filter(([method]) => method === 'attachments.delete')).toEqual([['attachments.delete', { attachmentId: 'att-1' }]]);
    expect(uploadTable(client)).toMatchObject({ pending: '', images: [{ id: 'img-0', status: 'failed', reason: 'Upload rejected (500)' }] });
    // A send retries a failed upload, so it does not wait for one (attachmentUploadBlockReason).
    expect(uploadBlock(client)).toBe('');
    retryDraftUpload(client, 'img-0');
    expect(uploadTable(client).pending).toBe('img-0');
    await runDraftUploads(client, native);
    expect(uploadTable(client).u0).toEqual({ id: 'img-0', attachmentId: 'att-2' });
  });

  test('a timeout, and an upload cut off with no answer, fail with the reference\'s words', async () => {
    const { client } = fixture(2);
    await runDraftUploads(client, native);
    expect(stream(0, client).event({ type: 'error', kind: 'Timeout' })).toMatchObject({ status: 'failed', reason: 'Upload timed out' });
    await runDraftUploads(client, native, [1]);
    expect(uploadTable(client).images.map(image => [image.status, image.reason])).toEqual([['failed', 'Upload timed out'], ['failed', 'Upload failed']]);
  });

  test('an image removed while it uploads lets its upload go: the resource carries nothing, and its attachment is deleted', async () => {
    const { client, requests } = fixture(2);
    await runDraftUploads(client, native);
    client.local.snapshotDrafts[KEY] = client.local.snapshotDrafts[KEY]!.filter(image => image.id !== 'img-0');
    expect(uploadTable(client).settle).toBe(1);
    settleUploads(client);
    expect(uploadTable(client).u0).toEqual({ id: '', attachmentId: '' });
    expect(uploadSource(client, 0, 'img-0', 'att-1')).toMatchObject({ id: '', status: '' });
    await runDraftUploads(client, native);
    expect(requests.filter(([method]) => method === 'attachments.delete')).toEqual([['attachments.delete', { attachmentId: 'att-1' }]]);
  });

  test('a mint that fails, or a draft file that is gone, fails the image without a resource', async () => {
    const { client } = fixture(1);
    (client as unknown as { request: () => Promise<never> }).request = async () => { throw new Error('offline'); };
    await runDraftUploads(client, native);
    expect(uploadTable(client).images).toEqual([{ id: 'img-0', attachmentId: '', status: 'failed', progress: 0, reason: 'Upload could not start' }]);
    const gone = fixture(1);
    adoptAppFiles({ fs: { readFile: async () => new ArrayBuffer(0), writeFile: async () => undefined, mkdir: async () => undefined, stat: async () => { throw new Error('missing'); } } });
    await runDraftUploads(gone.client, native);
    expect(uploadTable(gone.client).images[0]).toMatchObject({ status: 'failed', reason: 'Original file is no longer available' });
  });
});

describe('the tile draws its upload, and the root pumps the uploads', () => {
  const fns = contractFns('src/composer/composer-editor.contract', { floor: Math.floor, max: Math.max, min: Math.min }) as Record<string, (...args: unknown[]) => unknown>;
  const image = (patch: Obj) => ({ id: 'img-0', attachmentId: 'att-1', status: 'slot', progress: 0, reason: '', ...patch });
  const live = (patch: Obj) => ({ slot: 0, id: 'img-0', attachmentId: 'att-1', status: 'uploading', progress: 0, reason: '', ...patch });
  test('the percentage, from 0% while it waits, and the Retry\'s reason once it failed', () => {
    expect(fns.uploadPercent!(0.256)).toBe('25%');
    expect(fns.uploadPercent!(1.4)).toBe('100%');
    expect(fns.uploadLabel!(image({ status: 'queued', attachmentId: '' }), [])).toBe('0%');
    // In a resource that has not answered for this attachment yet (its answer is still the last image's).
    expect(fns.uploadLabel!(image({}), [live({ id: 'img-9', attachmentId: 'att-0', status: 'ready' })])).toBe('0%');
    expect(fns.uploadLabel!(image({}), [live({ progress: 0.456 })])).toBe('45%');
    expect(fns.uploadLabel!(image({}), [live({ status: 'ready', progress: 1 })])).toBe('');
    expect([fns.uploadLabel!(image({}), [live({ status: 'failed', reason: 'Upload timed out' })]), fns.uploadFailure!(image({}), [live({ status: 'failed', reason: 'Upload timed out' })])])
      .toEqual(['', 'Upload timed out']);
    expect(fns.uploadFailure!(image({ status: 'failed', attachmentId: '', reason: 'Upload rejected (500)' }), [])).toBe('Upload rejected (500)');
    expect(fns.uploadShows!([image({})], [live({ progress: 0.5 })])).toBe('uploading');
    expect(fns.uploadShows!([], [])).toBe('');
  });

  test('the pump names the resources that failed with no answer', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const sent: unknown[][] = [];
    root.call('composerUploadsNow', {}, { composerUploadsCut: '1', command: (...args: unknown[]) => { sent.push(args); return {}; } }, []);
    expect(sent).toEqual([['cclocal:uploads', '', '1', 0]]);
  });
});
