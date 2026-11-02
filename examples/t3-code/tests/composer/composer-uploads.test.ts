// A draft image uploads as it joins the draft, three at once, each in an upload resource whose stream tells the bytes it
// has sent (composer-uploads.ts; attachmentUploadQueue.ts at T3 Code 7c3b3b3605ef).
import { afterAll, afterEach, beforeAll, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { adoptAppFiles } from '../../src/media/save-file';
import { RETRY_FAILED_UPLOADS, deferSend, forgetMissingAttachments, forgetUnsentUploads, readySend, retryDraftUpload, runDraftUploads, settleUploads, stagedImages, takeWaitingSend, uploadBlock,
  uploadSource, uploadTable, waitingSend, type UploadEvent } from '../../src/composer/composer-uploads';
import { questionDraftKey } from '../../src/composer/question-attachments';
import { beginQueuedEdit, queuedEdit } from '../../src/composer/composer-controls-queue';
import type { Native } from '../../src/client/protocol';
import { contractComponent, contractFns } from '../support/contract-action-fixture';
import { opened } from '../support/composer-controls-fixture';
import { memoryFiles, png } from '../support/app-files-fixture';
import { snapshot } from '../../src/client/presentation';
import { arr, obj, str } from '../../src/client/domain';
import { toasts } from '../../src/shell/toast';
import { T3Client } from '../../src/client/client';
import { storage } from '../support/client-fixture';

type Obj = Record<string, unknown>;
type Fake = T3Client & { shown: string; local: { snapshotDrafts: Record<string, Obj[]> }; requests: Array<[string, Obj]> };
const A = 'env-1:thread-A', B = 'env-1:thread-B';
const native = {} as Native;

const images = (count: number, prefix = 'img'): Obj[] => Array.from({ length: count }, (_, index) => ({ id: `${prefix}-${index}`, name: `${prefix}-${index}.png`, mimeType: 'image/png', sizeBytes: 1 }));
/** Draft files of `size` bytes, as the upload's body reads them (snapshot-upload.ts). */
const draftFiles = (size: number) => adoptAppFiles({ fs: { readFile: async () => new ArrayBuffer(0), writeFile: async () => undefined, mkdir: async () => undefined, stat: async () => ({ size }) } } as never);

/** A connected client on machine env-1, showing draft `shown` of `drafts`, whose server stages uploads and mints their URLs. */
function fixture(drafts: Record<string, Obj[]>, shown = A, { supported = true, mintLimit = Infinity } = {}): Fake {
  const requests: Array<[string, Obj]> = [];
  let minted = 0;
  const client = {
    environmentId: 'env-1', origin: 'http://127.0.0.1:16555', connection: 'connected', generation: 1, shown, requests,
    scopes: ['orchestration:read', 'orchestration:operate'],
    get writable() { return this.connection === 'connected' && this.scopes.includes('orchestration:operate'); },
    get attachmentKey() { return this.shown; },
    config: { environment: { capabilities: { attachmentUploads: supported } } },
    local: { snapshotDrafts: drafts },
    get snapshotDrafts() { return this.local.snapshotDrafts[this.attachmentKey] ?? []; },
    request: async (_native: Native, method: string, payload: Obj) => {
      requests.push([method, payload]);
      if (method !== 'attachments.createUploadUrl') return {};
      if (Number(payload.sizeBytes) > mintLimit) throw new Error('Attachment exceeds the upload limit.');
      minted += 1;
      return { attachmentId: `att-${minted}`, relativeUrl: `/api/attachments/upload/att-${minted}?token=t${minted}` };
    },
  };
  draftFiles(2_000_000);
  return client as unknown as Fake;
}
const mints = (client: Fake) => client.requests.filter(([method]) => method === 'attachments.createUploadUrl').length;
/** A kept press on machine env-1 naming `imageIds` of draft `draftKey`. */
const press = (draftKey: string, imageIds: string[]) => ({ draftKey, environmentId: 'env-1', imageIds, resume: async () => '' });
const deleted = (client: Fake) => client.requests.filter(([method]) => method === 'attachments.delete').map(([, payload]) => payload.attachmentId);

/** The fetch an upload resource makes: its URL and init, and its mapper. This file's tests alone see the stand-in. */
const fetched: Array<{ url: string; init: Obj }> = [];
const realFetch = globalThis.fetch;
beforeAll(() => { globalThis.fetch = (async (url: string, init: Obj) => { fetched.push({ url, init }); return new Promise(() => {}); }) as unknown as typeof fetch; });
afterAll(() => { globalThis.fetch = realFetch; });
afterEach(() => { fetched.length = 0; });
/** Asks slot `slot`'s resource as the root would, with what the table says it carries. */
const stream = (slot: number, client: T3Client) => {
  const table = uploadTable(client), at = [table.u0, table.u1, table.u2, table.u3, table.u4, table.u5][slot]!;
  uploadSource(client, slot, at.id, at.attachmentId);
  const call = fetched.at(-1)!;
  return { ...call, event: (event: UploadEvent) => (call.init.exactStream as (e: UploadEvent) => Obj)(event) };
};
/** The root's pump, then each slot's upload landing, `rounds` times. */
async function pumpAndLand(client: Fake, rounds: number) {
  for (let round = 0; round < rounds; round++) {
    await runDraftUploads(client, native);
    const table = uploadTable(client);
    [table.u0, table.u1, table.u2].forEach((slot, index) => { if (slot.id) stream(index, client).event({ type: 'load', status: 201 }); });
  }
  await runDraftUploads(client, native);
}

describe('a draft image uploads as it joins the draft', () => {
  test('nothing waits while the server stages no uploads', () => {
    const client = fixture({ [A]: images(2) }, A, { supported: false });
    expect(uploadTable(client)).toMatchObject({ supported: false, pending: '', images: [] });
    expect(uploadBlock(client)).toBe('');
  });

  test('three at once: each free resource takes the next image, minting its upload URL; the rest wait at 0%', async () => {
    const client = fixture({ [A]: images(4) });
    expect(uploadTable(client).pending).toBe('img-0 img-1 img-2 img-3');
    await runDraftUploads(client, native);
    expect(client.requests.map(([method, payload]) => [method, payload.type, payload.mimeType, payload.sizeBytes])).toEqual(Array(3).fill(['attachments.createUploadUrl', 'image', 'image/png', 2_000_000]));
    const table = uploadTable(client);
    expect([table.u0, table.u1, table.u2]).toEqual([{ id: 'img-0', attachmentId: 'att-1' }, { id: 'img-1', attachmentId: 'att-2' }, { id: 'img-2', attachmentId: 'att-3' }]);
    // No resource is free, so the root asks for none until one is.
    expect(table.pending).toBe('');
    expect(table.images.map(image => [image.id, image.status])).toEqual([['img-0', 'slot'], ['img-1', 'slot'], ['img-2', 'slot'], ['img-3', 'queued']]);
    expect(uploadBlock(client)).toBe('Attachments still uploading');
  });

  test('the stream posts the draft file and tells its progress from its first report, then in twentieths; its reply lands the attachment', async () => {
    const client = fixture({ [A]: images(2) }), [first] = client.local.snapshotDrafts[A]!;
    await runDraftUploads(client, native);
    const upload = stream(0, client);
    expect(upload.url).toBe('http://127.0.0.1:16555/api/attachments/upload/att-1?token=t1');
    expect(upload.init).toMatchObject({ method: 'POST', exactBodyFrom: 'app:/data/snapshots/drafts/img-0.png', exactUploadProgress: true, exactTimeout: 300000,
      headers: { 'Content-Type': 'image/png' } });
    // The first report shows (the tile reads 1%), as uploadBytes' lastStep starts below every step.
    expect(upload.event({ type: 'progress', loaded: 20_000, total: 2_000_000 })).toMatchObject({ id: 'img-0', status: 'uploading', progress: 0.01 });
    expect(upload.event({ type: 'progress', loaded: 512_000, total: 2_000_000 })).toMatchObject({ progress: 0.256 });
    // A step of less than 5% keeps the answer the reference would show.
    expect(upload.event({ type: 'progress', loaded: 540_000, total: 2_000_000 })).toMatchObject({ progress: 0.256 });
    expect(upload.event({ type: 'load', status: 201 })).toMatchObject({ status: 'ready', progress: 1, attachmentId: 'att-1' });
    // Landed, it no longer holds the send, and the root's next run settles its resource.
    expect(uploadBlock(client)).toBe('Attachment still uploading');
    expect(uploadTable(client)).toMatchObject({ u0: { id: 'img-0', attachmentId: 'att-1' }, settle: 1 });
    expect(first!.uploadId).toBeUndefined();
    await runDraftUploads(client, native);
    expect(first!.uploadId).toBe('att-1');
    expect(uploadTable(client)).toMatchObject({ u0: { id: '', attachmentId: '' }, settle: 0 });
    expect(uploadTable(client).images.map(image => image.id)).toEqual(['img-1']);
  });

  test('a failed upload keeps its reason for the tile\'s Retry, its attachment is deleted, and Retry mints another', async () => {
    const client = fixture({ [A]: images(1) });
    await runDraftUploads(client, native);
    expect(stream(0, client).event({ type: 'load', status: 500 })).toMatchObject({ status: 'failed', reason: 'Upload rejected (500)' });
    await runDraftUploads(client, native);
    expect(deleted(client)).toEqual(['att-1']);
    expect(uploadTable(client)).toMatchObject({ pending: '', operate: true, images: [{ id: 'img-0', status: 'failed', reason: 'Upload rejected (500)' }] });
    // A send retries a failed upload, so it does not wait for one (attachmentUploadBlockReason).
    expect(uploadBlock(client)).toBe('');
    retryDraftUpload(client, 'img-0');
    expect(uploadTable(client).pending).toBe('img-0');
    await runDraftUploads(client, native);
    expect(uploadTable(client).u0).toEqual({ id: 'img-0', attachmentId: 'att-2' });
  });

  test('a failure is asked again once the connection is replaced, and Retry does not act on a connection that may not operate', async () => {
    const client = fixture({ [A]: images(1) });
    await runDraftUploads(client, native);
    stream(0, client).event({ type: 'error' });
    await runDraftUploads(client, native);
    expect(uploadTable(client).images).toMatchObject([{ status: 'failed', reason: 'Upload failed' }]);
    client.connection = 'reconnecting';
    expect(uploadTable(client).images).toMatchObject([{ status: 'failed' }]);
    // The environment connected again (attachmentUploadQueue's reconnect watch): the upload starts over.
    Object.assign(client, { connection: 'connected', generation: 2 });
    expect(uploadTable(client)).toMatchObject({ pending: 'img-0', images: [{ status: 'queued' }] });
    await runDraftUploads(client, native);
    expect(uploadTable(client).u0).toEqual({ id: 'img-0', attachmentId: 'att-2' });
    stream(0, client).event({ type: 'load', status: 503 });
    client.scopes = ['orchestration:read'];
    expect(uploadTable(client)).toMatchObject({ operate: false, images: [{ status: 'failed', reason: 'Upload rejected (503)' }] });
  });

  test('a timeout, and an upload cut off with no answer, fail with the reference\'s words', async () => {
    const client = fixture({ [A]: images(2) });
    await runDraftUploads(client, native);
    expect(stream(0, client).event({ type: 'error', kind: 'Timeout' })).toMatchObject({ status: 'failed', reason: 'Upload timed out' });
    await runDraftUploads(client, native, [1]);
    expect(uploadTable(client).images.map(image => [image.status, image.reason])).toEqual([['failed', 'Upload timed out'], ['failed', 'Upload failed']]);
  });

  test('an image removed while it uploads lets its upload go: the resource carries nothing, and its attachment is deleted', async () => {
    const client = fixture({ [A]: images(2) });
    await runDraftUploads(client, native);
    client.local.snapshotDrafts[A] = client.local.snapshotDrafts[A]!.filter(image => image.id !== 'img-0');
    expect(uploadTable(client).settle).toBe(1);
    settleUploads(client);
    expect(uploadTable(client).u0).toEqual({ id: '', attachmentId: '' });
    expect(uploadSource(client, 0, 'img-0', 'att-1')).toMatchObject({ id: '', status: '' });
    await runDraftUploads(client, native);
    expect(deleted(client)).toEqual(['att-1']);
  });

  test('an image removed after its upload landed, before the pump settled it, has its attachment deleted (releaseAttachmentUpload)', async () => {
    const client = fixture({ [A]: images(1) });
    await runDraftUploads(client, native);
    stream(0, client).event({ type: 'load', status: 201 });
    client.local.snapshotDrafts[A] = [];
    await runDraftUploads(client, native);
    expect(deleted(client)).toEqual(['att-1']);
    expect(uploadTable(client)).toMatchObject({ u0: { id: '' }, settle: 0 });
  });

  test('a type the provider does not take, a mint the server refuses (an image over its limit), and a draft file that is gone fail the image', async () => {
    const heic = fixture({ [A]: [{ id: 'img-0', name: 'photo.heic', mimeType: 'image/heic', sizeBytes: 1 }] });
    await runDraftUploads(heic, native);
    expect(uploadTable(heic).images).toEqual([{ id: 'img-0', attachmentId: '', status: 'failed', progress: 0, reason: 'Unsupported image type' }]);
    expect(mints(heic)).toBe(0);
    const large = fixture({ [A]: images(1) }, A, { mintLimit: 1_000_000 });
    await runDraftUploads(large, native);
    expect(uploadTable(large).images[0]).toMatchObject({ status: 'failed', reason: 'Upload could not start' });
    const gone = fixture({ [A]: images(1) });
    adoptAppFiles({ fs: { readFile: async () => new ArrayBuffer(0), writeFile: async () => undefined, mkdir: async () => undefined, stat: async () => { throw new Error('missing'); } } } as never);
    await runDraftUploads(gone, native);
    expect(uploadTable(gone).images[0]).toMatchObject({ status: 'failed', reason: 'Original file is no longer available' });
    expect(mints(gone)).toBe(0);
  });
});

describe('an upload belongs to its draft entry', () => {
  // adoptPastedChips copies the image into the other draft with its id, and without its attachment.
  const source = () => ({ id: 'img-x', name: 'x.png', mimeType: 'image/png', sizeBytes: 1, uploadId: 'att-0' });
  const pasted = () => ({ id: 'img-x', name: 'x.png', mimeType: 'image/png', sizeBytes: 1 });

  test('a chip pasted into another thread uploads once there, and the draft it came from keeps its own attachment', async () => {
    const from = source(), into: Obj = pasted(), client = fixture({ [A]: [from], [B]: [into] }, B);
    await pumpAndLand(client, 4);
    expect(mints(client)).toBe(1);
    expect([from.uploadId, into.uploadId]).toEqual(['att-0', 'att-1']);
    expect(uploadTable(client)).toMatchObject({ pending: '', settle: 0, images: [] });
    expect(uploadBlock(client)).toBe('');
  });

  test('a chip pasted into a question\'s answer uploads once for that answer', async () => {
    const answer = questionDraftKey('env-1', 'thread-A', 'request-1', 'question-1');
    const from = source(), into: Obj = pasted(), client = fixture({ [A]: [from], [answer]: [into] }, answer);
    await pumpAndLand(client, 4);
    expect(mints(client)).toBe(1);
    expect([from.uploadId, into.uploadId]).toEqual(['att-0', 'att-1']);
    expect(uploadBlock(client)).toBe('');
  });

  test('a failure in one thread does not mark the same image pasted into another', async () => {
    const client = fixture({ [A]: [source()], [B]: [pasted()] }, A);
    delete client.local.snapshotDrafts[A]![0]!.uploadId;
    await runDraftUploads(client, native);
    stream(0, client).event({ type: 'load', status: 500 });
    await runDraftUploads(client, native);
    expect(uploadTable(client).images).toMatchObject([{ id: 'img-x', status: 'failed' }]);
    client.shown = B;
    expect(uploadTable(client)).toMatchObject({ pending: 'img-x', images: [{ id: 'img-x', status: 'queued' }] });
  });

  test('the queue goes on while another draft is shown: an image waiting for a slot starts once one is free', async () => {
    const client = fixture({ [A]: images(4), [B]: [] });
    await runDraftUploads(client, native);
    client.shown = B;
    expect(uploadTable(client)).toMatchObject({ pending: '', images: [] });
    stream(1, client).event({ type: 'load', status: 201 });
    expect(uploadTable(client).settle).toBe(1);
    await runDraftUploads(client, native);
    expect(client.local.snapshotDrafts[A]![1]!.uploadId).toBe('att-2');
    expect(uploadTable(client).u1).toEqual({ id: 'img-3', attachmentId: 'att-4' });
  });

  test('a queued message\'s edit sets the thread\'s images aside, and their uploads go on and land in them', async () => {
    const stash = images(1), client = fixture({ [A]: stash });
    await runDraftUploads(client, native);
    Object.assign(client, { threadId: 'thread-A', draftKey: A, local: { ...client.local, drafts: {} },
      projection: { runs: [{ id: 'q1', ordinal: 2, status: 'queued', queuePosition: 1, userMessageId: 'm1' }], messages: [{ id: 'm1', text: 'Queued', attachments: [] }] } });
    // beginEdit: the draft shows the edit's own images; the thread's wait in the edit's stash.
    beginQueuedEdit(client, 'q1');
    expect([client.local.snapshotDrafts[A], queuedEdit(client)?.stashImages]).toEqual([[], stash]);
    expect(uploadTable(client)).toMatchObject({ u0: { id: 'img-0', attachmentId: 'att-1' }, settle: 0 });
    stream(0, client).event({ type: 'load', status: 201 });
    await runDraftUploads(client, native);
    expect([stash[0]!.uploadId, deleted(client)]).toEqual(['att-1', []]);
  });

  test('an upload of another machine\'s draft lands while that machine is not focused; its next image and its deletions wait for it', async () => {
    const client = fixture({ [A]: images(4) });
    await runDraftUploads(client, native);
    Object.assign(client, { environmentId: 'env-2', shown: 'env-2:thread-C' });
    client.local.snapshotDrafts['env-2:thread-C'] = [];
    stream(0, client).event({ type: 'load', status: 201 });
    client.local.snapshotDrafts[A] = client.local.snapshotDrafts[A]!.filter(image => image.id !== 'img-1');
    await runDraftUploads(client, native);
    expect(client.local.snapshotDrafts[A]![0]!.uploadId).toBe('att-1');
    expect([mints(client), deleted(client), uploadTable(client).settle]).toEqual([3, [], 0]);
    Object.assign(client, { environmentId: 'env-1', shown: A });
    expect(uploadTable(client).settle).toBe(1);
    await runDraftUploads(client, native);
    expect(deleted(client)).toEqual(['att-2']);
    expect(uploadTable(client).u0).toEqual({ id: 'img-3', attachmentId: 'att-4' });
  });
});

describe('a send and its draft\'s uploads: no send waits on its own timer', () => {
  test('a press whose images have not all landed is kept, its failed ones go back to the queue, and it goes once none is on its way', async () => {
    const client = fixture({ [A]: images(2) });
    await runDraftUploads(client, native);
    stream(1, client).event({ type: 'error', kind: 'Timeout' });
    await runDraftUploads(client, native);
    const drafted = client.local.snapshotDrafts[A]!;
    expect(() => stagedImages(client, A, drafted, 2, false)).toThrow(expect.objectContaining({ kind: 'deferred' }));
    // The failed one is asked again, through the queue and its limit.
    expect(uploadTable(client).images.map(image => [image.id, image.status])).toEqual([['img-0', 'slot'], ['img-1', 'queued']]);
    deferSend(client, press(A, ['img-0', 'img-1']));
    const token = waitingSend(client)!.token;
    expect([token.startsWith('send-'), readySend(client)]).toEqual([true, '']);
    stream(0, client).event({ type: 'load', status: 201 });
    await runDraftUploads(client, native);
    // The image asked again took the resource img-0 left.
    expect([uploadTable(client).u0.id, readySend(client)]).toEqual(['img-1', '']);
    stream(0, client).event({ type: 'load', status: 201 });
    expect(readySend(client)).toBe(token);
    expect(stagedImages(client, A, drafted, 2, true).map(image => image.uploadId)).toEqual(['att-1', 'att-3']);
    expect([takeWaitingSend(client, 'send-0'), takeWaitingSend(client, token)?.imageIds, waitingSend(client)]).toEqual([undefined, ['img-0', 'img-1'], undefined]);
  });

  test('the press sent again says to retry or remove an image that failed again, or left the draft', async () => {
    const client = fixture({ [A]: images(2) });
    await runDraftUploads(client, native);
    deferSend(client, press(A, ['img-0', 'img-1']));
    stream(0, client).event({ type: 'load', status: 500 });
    stream(1, client).event({ type: 'load', status: 201 });
    expect(readySend(client)).toBe(waitingSend(client)!.token);
    expect(() => stagedImages(client, A, client.local.snapshotDrafts[A]!, 2, true)).toThrow(RETRY_FAILED_UPLOADS);
    expect(() => stagedImages(client, A, client.local.snapshotDrafts[A]!.slice(1), 2, true)).toThrow(RETRY_FAILED_UPLOADS);
  });

  test('each draft keeps its own press; the one the root sends is on the focused machine', async () => {
    const client = fixture({ [A]: images(1), [B]: images(1, 'b'), 'env-2:thread-C': [] });
    await runDraftUploads(client, native);
    deferSend(client, press(A, ['img-0']));
    const first = waitingSend(client)!.token;
    client.shown = B;
    // Another thread's composer is free: its own press is kept beside the first.
    expect(waitingSend(client)).toBeUndefined();
    deferSend(client, press(B, ['b-0']));
    const second = waitingSend(client)!.token;
    stream(0, client).event({ type: 'load', status: 201 });
    expect(readySend(client)).toBe(first);
    // Not while the connection may not write: the root sends it once it can, rather than a send that fails at once.
    client.connection = 'reconnecting';
    expect(readySend(client)).toBe('');
    client.connection = 'connected';
    Object.assign(client, { environmentId: 'env-2', shown: 'env-2:thread-C' });
    expect(readySend(client)).toBe('');
    Object.assign(client, { environmentId: 'env-1', shown: B });
    takeWaitingSend(client, first);
    expect([readySend(client), waitingSend(client)?.token]).toEqual(['', second]);
  });

  test('three uploads at once on each machine: another machine focused meanwhile starts its own', async () => {
    const client = fixture({ [A]: images(4) });
    await runDraftUploads(client, native);
    expect(mints(client)).toBe(3);
    Object.assign(client, { environmentId: 'env-2', shown: 'env-2:thread-C' });
    client.local.snapshotDrafts['env-2:thread-C'] = images(2, 'c');
    expect(uploadTable(client).pending).toBe('c-0 c-1');
    await runDraftUploads(client, native);
    const table = uploadTable(client);
    expect([table.u0, table.u1, table.u2, table.u3, table.u4].map(slot => slot.id)).toEqual(['img-0', 'img-1', 'img-2', 'c-0', 'c-1']);
    // The first machine's fourth waits for one of its own to end.
    expect(table.u5).toEqual({ id: '', attachmentId: '' });
  });

  test('a send refused for an attachment the server no longer has clears that image\'s id on this machine, so it uploads again', () => {
    const client = fixture({ [A]: [{ ...images(1)[0], uploadId: 'att-old' }, { id: 'img-9', name: 'other.png', uploadId: 'att-9' }], 'env-2:t': [{ id: 'img-0', name: 'img-0.png', uploadId: 'att-x' }] });
    expect(forgetMissingAttachments(client, new Error('offline'))).toBe(false);
    expect(forgetMissingAttachments(client, new Error("Attachment 'img-0.png' cannot be sent: attachment not found (removed or expired)."))).toBe(true);
    expect(client.local.snapshotDrafts[A]!.map(image => image.uploadId)).toEqual([undefined, 'att-9']);
    expect(client.local.snapshotDrafts['env-2:t']![0]!.uploadId).toBe('att-x');
    expect(uploadTable(client).pending).toBe('img-0');
  });

  test('a relaunch reads the drafts back without their attachment ids, but those a submission in doubt names', async () => {
    const uuid = (n: number) => `cccccccc-cccc-4ccc-8ccc-${String(n).padStart(12, '0')}`;
    const image = (n: number) => ({ id: uuid(n), name: `${n}.png`, mimeType: 'image/png', sizeBytes: 10, uploadId: `att-${n}` });
    const { files } = storage({ version: 1, snapshotDrafts: { 'env1:t1': [image(1), image(2)] }, pending: { env1: { method: 'orchestration.dispatchCommand',
      payload: { commandId: 'c1', attachments: [{ type: 'image', id: 'att-2' }] }, description: 'Send', threadId: 't1', text: '' } } });
    const client = new T3Client();
    await client.load(files);
    expect(client.local.snapshotDrafts['env1:t1']!.map(entry => entry.uploadId)).toEqual([undefined, 'att-2']);
  });

  test('at launch the ids are gone, but those a submission still in doubt names', () => {
    const local = { snapshotDrafts: { [A]: [{ id: 'img-0', uploadId: 'att-1' }, { id: 'img-1', uploadId: 'att-2' }], [B]: [{ id: 'img-2', uploadId: 'att-3' }] } as Record<string, Obj[]>,
      pending: { 'env-1': { payload: { attachments: [{ type: 'image', id: 'att-2' }] } } }, launches: { l: { payload: { initialMessage: { attachments: [{ id: 'att-3' }] } } } } };
    forgetUnsentUploads(local);
    expect(Object.values(local.snapshotDrafts).flat()).toEqual([{ id: 'img-0' }, { id: 'img-1', uploadId: 'att-2' }, { id: 'img-2', uploadId: 'att-3' }]);
  });
});

describe('the tile draws its upload, and the root pumps the uploads', () => {
  const fns = contractFns('src/composer/composer-editor.contract', { floor: Math.floor, max: Math.max, min: Math.min }) as Record<string, (...args: unknown[]) => unknown>;
  const image = (patch: Obj) => ({ id: 'img-0', attachmentId: 'att-1', status: 'slot', progress: 0, reason: '', ...patch });
  const live = (patch: Obj) => ({ slot: 0, id: 'img-0', attachmentId: 'att-1', status: 'uploading', progress: 0, reason: '', ...patch });
  test('the percentage, from 0% while it waits and 1% at the first report, and the Retry\'s reason once it failed', () => {
    expect(fns.uploadPercent!(0.01)).toBe('1%');
    expect(fns.uploadPercent!(0.256)).toBe('25%');
    expect(fns.uploadPercent!(1.4)).toBe('100%');
    expect(fns.uploadLabel!(image({ status: 'queued', attachmentId: '' }), [])).toBe('0%');
    // In a resource that has not answered for this attachment yet (its answer is still the last image's).
    expect(fns.uploadLabel!(image({}), [live({ id: 'img-9', attachmentId: 'att-0', status: 'ready' })])).toBe('0%');
    expect(fns.uploadLabel!(image({}), [live({ progress: 0.01 })])).toBe('1%');
    expect(fns.uploadLabel!(image({}), [live({ status: 'ready', progress: 1 })])).toBe('');
    expect([fns.uploadLabel!(image({}), [live({ status: 'failed', reason: 'Upload timed out' })]), fns.uploadFailure!(image({}), [live({ status: 'failed', reason: 'Upload timed out' })])])
      .toEqual(['', 'Upload timed out']);
    expect(fns.uploadFailure!(image({ status: 'failed', attachmentId: '', reason: 'Upload rejected (500)' }), [])).toBe('Upload rejected (500)');
    expect(fns.uploadShows!([image({})], [live({ progress: 0.5 })])).toBe('uploading');
    expect(fns.uploadShows!([], [])).toBe('');
  });

  test('an image\'s chip in the prompt reads its tile\'s upload at each report, and "upload failed" once it failed (uploadStatusSuffix)', () => {
    const status = (patch: Obj, answers: unknown[], id = 'img-0') => fns.uploadChipStatus!(id, [image(patch)], answers);
    expect(status({ status: 'queued', attachmentId: '' }, [])).toBe('0%');
    const reports = [0.01, 0.24, 0.36, 0.5, 0.99, 1];
    expect(reports.map(progress => status({}, [live({ progress })]))).toEqual(['1%', '24%', '36%', '50%', '99%', '100%']);
    expect(reports.map(progress => status({}, [live({ progress })]))).toEqual(reports.map(progress => fns.uploadLabel!(image({}), [live({ progress })])));
    expect(status({}, [live({ status: 'ready', progress: 1 })])).toBe('');
    expect(status({}, [live({ status: 'failed', reason: 'Upload timed out' })])).toBe('upload failed');
    expect(status({ status: 'failed', attachmentId: '', reason: 'Upload rejected (500)' }, [])).toBe('upload failed');
    // A chip of another kind (no image id) or of another image shows none.
    expect([status({}, [live({ progress: 0.5 })], ''), status({}, [live({ progress: 0.5 })], 'img-1')]).toEqual(['', '']);
  });

  test('the prompt\'s chips take their images\' upload states from the draft\'s uploads and the resources\' answers', () => {
    const card = contractComponent('composer.contract', 'ComposerCard');
    const data = { canSend: true, snapshotDrafts: [], composer: { uploadBlock: '', compactTokens: '', canResume: false, queueEditing: '', queueEditAttachments: [], uploads: { images: [image({})] } } };
    const scope = { data, text: '', richComposer: { chips: [{ face: { imageId: 'img-0' } }, { face: { imageId: '' } }] }, composerUploads: [live({ progress: 0.6 })],
      composerAtomFace: (chip: Obj, upload: string) => [obj(chip.face).imageId, upload], uploadChipStatus: fns.uploadChipStatus };
    expect(card.derives({}, scope).faces).toEqual([['img-0', '60%'], ['', '']]);
  });

  test('Retry acts only where the connection may operate the thread', () => {
    const meter = contractComponent('composer-editor.contract', 'UploadMeter');
    const scope = (operate: boolean) => ({ upload: image({ status: 'failed', attachmentId: '', reason: 'Upload failed' }), live: [], name: 'shot.png', command: () => ({}), operate, ...fns });
    expect([meter.attribute('retry-upload-', 'disabled', {}, scope(true)), meter.attribute('retry-upload-', 'disabled', {}, scope(false))]).toEqual([false, true]);
  });

  test('each tile of the strip gets its own image\'s uploads, the resources\' answers and whether Retry acts', () => {
    const strip = contractComponent('snapshot.contract', 'SnapshotDrafts');
    const uploads = { supported: true, operate: false, images: [image({ id: 'img-0' }), image({ id: 'img-1', status: 'queued' })] }, answers = [live({})];
    const scope = { images: [], videos: [], disabled: false, remove: () => ({}), ask: () => ({}), command: () => ({}), uploads, live: answers, i: 1, draftFocusAfter: () => '' };
    for (const [marker, image] of [['ImageDraftTile(', { id: 'img-1', name: 'b.png', referenced: false }], ['SnapshotTile(', { id: 'img-1', name: 'b.png', snapshot: true }]] as const) {
      expect(strip.callArguments(marker, {}, { ...scope, image })).toMatchObject({ uploads: [uploads.images[1]], live: answers, operate: false });
    }
  });

  test('the pump names the resources that failed with no answer, and the tick hears it start and end', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const sent: unknown[][] = [];
    root.call('composerUploadsNow', {}, { composerUploadsCut: '1', command: (...args: unknown[]) => { sent.push(args); return {}; } }, []);
    expect(sent).toEqual([['cclocal:uploads', '', '1', 0]]);
  });
});

describe('with uploads staged, Send and Return wait for the draft\'s uploads, and a press that goes anyway is kept until they land', () => {
  const ID = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
  async function staged() {
    const context = await opened(), { client, native } = context;
    memoryFiles({ [`app:/data/snapshots/drafts/${ID}.png`]: png(4, 3, 20) });
    client.config = { ...client.config, environment: { ...obj(client.config.environment), capabilities: { ...obj(obj(client.config.environment).capabilities), attachmentUploads: true } } };
    client.local.snapshotDrafts[client.draftKey] = [{ id: ID, name: 'Capture.png', mimeType: 'image/png', sizeBytes: 53, source: {} }];
    const later = native.later.bind(native), calls: Obj[] = [], server = { expired: false };
    native.later = async input => {
      const request = obj(input); calls.push(request);
      // AttachmentClaims' refusal of an attachment the server deleted unsent.
      if (server.expired && obj(request.payload).type === 'message.dispatch') {
        return { ok: false, generation: native.generation, error: { kind: 'AttachmentClaimError', message: "Attachment 'Capture.png' cannot be sent: attachment not found.", uncertain: false } };
      }
      if (request.method === 'attachments.createUploadUrl') return native.ok({ attachmentId: 'att-1', relativeUrl: '/api/attachments/upload/att-1?token=t' });
      return later(input);
    };
    const dispatched = () => native.committed.filter(entry => entry.type === 'message.dispatch');
    /** The root's resume of the press it may send now, as its `sendResume` task does. */
    const resume = () => context.command('send-resume', snapshot(client).composer.sendResume);
    return { ...context, calls, server, dispatched, resume };
  }

  test('a press while its image uploads answers at once and is kept; once it lands the root sends it, naming its attachment', async () => {
    const { client, command, calls, dispatched, resume } = await staged();
    await command('cclocal:uploads');
    expect(snapshot(client)).toMatchObject({ canSend: true, composer: { uploadBlock: 'Attachment still uploading', uploads: { u0: { attachmentId: 'att-1' } }, sendResume: '' } });
    // No answer waits on the upload, so no newer command can let a waiting one go (LLP 1027.000 D1).
    expect((await command('send', '', 'Look at this')).message).toBe('');
    expect([dispatched(), snapshot(client).canSend, snapshot(client).composer.sendResume]).toEqual([[], false, '']);
    // A second press while it is kept sends nothing (sendInFlightRef).
    await command('send', '', 'Again');
    stream(0, client).event({ type: 'load', status: 201 });
    expect(snapshot(client).composer.sendResume).toMatch(/^send-/);
    expect((await resume()).message).toBe('');
    expect(dispatched()).toMatchObject([{ threadId: 't1', attachments: [{ type: 'image', id: 'att-1', name: 'Capture.png' }] }]);
    // The send named the upload that landed: it uploaded nothing itself.
    expect(calls.filter(call => call.op === 'uploadAttachment')).toEqual([]);
    expect(snapshot(client)).toMatchObject({ canSend: true, composer: { sendResume: '' } });
  });

  test('what the press named goes: an image attached while it waits stays in the draft, out of the message and its context', async () => {
    const { client, command, dispatched, resume } = await staged();
    await command('cclocal:uploads');
    await command('send', '', 'Just the first');
    const added = { id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', name: 'Later.png', mimeType: 'image/png', sizeBytes: 53, source: {}, uploadId: 'att-9' };
    client.local.snapshotDrafts[client.draftKey]!.push(added);
    stream(0, client).event({ type: 'load', status: 201 });
    await resume();
    const [sent] = dispatched();
    expect(arr(sent!.attachments).map(attachment => attachment.name)).toEqual(['Capture.png']);
    expect(arr(obj(sent!.context).records).map(record => record.name)).toEqual(['Capture.png']);
    expect(client.local.snapshotDrafts[client.draftKey]!.map(image => image.name)).toEqual(['Later.png']);
  });

  test('a press whose thread is left goes to that thread once its image lands, and the thread shown stays as it was', async () => {
    const { client, command, dispatched, resume } = await staged();
    await command('cclocal:uploads');
    await command('send', '', 'Sent from t1');
    await command('select-thread', 't2');
    // The thread shown has a composer of its own, free to send.
    expect([client.threadId, snapshot(client).canSend]).toEqual(['t2', true]);
    stream(0, client).event({ type: 'load', status: 201 });
    expect((await resume()).message).toBe('');
    expect(dispatched()).toMatchObject([{ threadId: 't1', attachments: [{ id: 'att-1' }] }]);
    expect([client.threadId, client.local.drafts['env1:t1'] ?? '', client.local.snapshotDrafts['env1:t1'] ?? []]).toEqual(['t2', '', []]);
  });

  test('a press that fails after its thread was left says so in a toast, not in the thread shown', async () => {
    const { client, command, dispatched, resume } = await staged();
    await command('cclocal:uploads');
    await command('send', '', 'Sent from t1');
    await command('select-thread', 't2');
    stream(0, client).event({ type: 'load', status: 500 });
    expect((await resume()).message).toBe('');
    expect(dispatched()).toEqual([]);
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Message not sent', description: 'Retry or remove failed uploads before sending.' });
    expect([client.error, client.local.drafts['env1:t1']]).toEqual(['', 'Sent from t1']);
  });

  test('a press retries a failed image through the queue; failing again, it asks to retry or remove it', async () => {
    const { client, command, dispatched, resume } = await staged();
    await command('cclocal:uploads');
    stream(0, client).event({ type: 'load', status: 500 });
    await command('cclocal:uploads');
    // A failed image does not hold Send (attachmentUploadBlockReason); the press asks it again.
    expect(snapshot(client).composer.uploadBlock).toBe('');
    expect((await command('send', '', 'Again')).message).toBe('');
    expect(snapshot(client).composer.uploads.pending).toBe(ID);
    await command('cclocal:uploads');
    stream(0, client).event({ type: 'load', status: 500 });
    expect((await resume()).message).toBe('Retry or remove failed uploads before sending.');
    expect(dispatched()).toEqual([]);
    retryDraftUpload(client, ID);
    await command('cclocal:uploads');
    stream(0, client).event({ type: 'load', status: 201 });
    await command('cclocal:uploads');
    expect((await command('send', '', 'Again')).message).toBe('');
    expect(dispatched()).toMatchObject([{ attachments: [{ type: 'image', id: 'att-1' }] }]);
  });

  test('a send refused for an attachment the server no longer has leaves the image to upload again', async () => {
    const { client, command, server } = await staged();
    const [image] = client.local.snapshotDrafts[client.draftKey]!;
    image!.uploadId = 'att-old';
    server.expired = true;
    expect((await command('send', '', 'Old')).message).toBe("Attachment 'Capture.png' cannot be sent: attachment not found.");
    expect(image!.uploadId).toBeUndefined();
    expect(snapshot(client).composer.uploads.pending).toBe(str(image!.id));
  });

  test('the root sends the kept press as a command of its own, once none is pending, and Reopen Closed Tab waits for one', () => {
    const root = contractComponent('app.contract', 'T3Code'), sent: unknown[][] = [], command = (...args: unknown[]) => { sent.push(args); return {}; };
    const slots = { commandPending: false };
    root.call('sendResumeNow', slots, { data: { composer: { sendResume: 'send-3' } }, command }, []);
    expect([sent, slots.commandPending]).toEqual([[['send-resume', 'send-3', '', 0]], true]);
    root.call('sendResumeNow', slots, { data: { composer: { sendResume: 'send-4' } }, command }, []);
    expect(sent).toHaveLength(1);
    // ⇧⌘T's send would let the command in flight go: it waits as the other presses do.
    root.call('panelUi', { rightPanelAt: '', settingsOpen: false, utilityPage: '', draftOwner: '', jumpRequest: 0, commandPending: true }, { data: { diffOpen: false, threadId: 't1', projectId: 'p1' },
      rightPanel: false, shell: { panel: { tabs: [], key: 'env:t1', count: 0, open: false }, terminalAvailable: false }, command }, ['reopen', '']);
    expect(sent).toHaveLength(1);
  });

  const root = contractComponent('app.contract', 'T3Code');
  const data = (uploadBlock: string, requestMode = '') => ({ canSend: true, requestMode, snapshotOwner: 's', snapshotDrafts: [{ id: 'img-0' }], connected: true,
    composer: { uploadBlock, compactTokens: '', canResume: false, queueEditing: '', toolbarRight: 0, sendTipWidth: 0 } });
  test('Return and the Send buttons wait while one uploads (attachmentUploadBlockReason)', () => {
    for (const block of ['', 'Attachment still uploading']) {
      const sent: unknown[][] = [];
      root.call('submit', { commandPending: false, composerTyping: true, draftOwner: '', jumpRequest: 0 }, { editor: { overLimit: false }, data: data(block), composerText: 'hi',
        command: (...args: unknown[]) => { sent.push(args); return {}; } }, ['']);
      expect(sent).toEqual(block ? [] : [['send', '', 'hi', 0]]);
      const scope = { data: data(block), text: '', busy: false, resting: false, viewportWidth: 800, max: Math.max, richComposer: { chips: [] }, composerAtomFace: (chip: unknown) => chip };
      expect([contractComponent('composer.contract', 'ComposerCard').derives({}, scope).sendable, contractComponent('composer-controls.contract', 'ComposerFooter').derives({}, scope).sendable])
        .toEqual([!block, !block]);
      // A plan's Refine and Implement, too (ComposerPrimaryActions).
      expect(contractComponent('composer-controls.contract', 'PrimaryActions').derives({}, scope).blocked).toBe(!!block);
    }
    // A question's Submit goes, and its answer says why it waits (onRespondToUserInput).
    const sent: unknown[][] = [];
    root.call('submit', { commandPending: false, composerTyping: true, draftOwner: '', jumpRequest: 0 }, { editor: { overLimit: false }, data: data('Attachment still uploading', 'question'),
      composerText: 'hi', command: (...args: unknown[]) => { sent.push(args); return {}; } }, ['']);
    expect(sent).toEqual([['send', '', 'hi', 0]]);
  });

  test('the Browser\'s annotate and send goes while the crop uploads: the send waits for it', () => {
    const sent: unknown[][] = [];
    root.call('annotationSendNow', { annotationSendHandled: 0, commandPending: false, composerTyping: true, draftOwner: '', jumpRequest: 0 }, {
      shell: { panel: { annotationSend: 1, annotationOwner: 'o', annotationSnapshot: 's', annotationGeneration: 3, annotationValue: 'v' } }, data: data('Attachment still uploading'),
      richComposerOwner: 'o', richComposer: { generation: 3 }, composerText: 'v', composerComposing: false,
      command: (...args: unknown[]) => { sent.push(args); return {}; }, chatLocal: (...args: unknown[]) => { sent.push(['held', ...args]); } }, []);
    expect(sent).toEqual([['send-annotation', '1', '', 0]]);
  });
});
