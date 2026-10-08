import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { mobileOutboxRead, mobileOutboxCapture, mobileOutboxSnapshot, mobileOutboxStatus } from './mobile-outbox';
import { mobileOutboxPrepareAttachments } from './mobile-outbox-upload';
import type { MobileOutboxAttachment, MobileOutboxRecord } from './mobile-outbox-model';

const image = (digit = '1'): MobileOutboxAttachment => ({ kind: 'image', id: `${digit.repeat(8)}-${digit.repeat(4)}-4${digit.repeat(3)}-a${digit.repeat(3)}-${digit.repeat(12)}`,
  name: 'image.png', mimeType: 'image/png', sizeBytes: 3, uploadId: '', status: 'staged' });
const file = (): MobileOutboxAttachment => ({ ...image('2'), kind: 'file', name: 'notes.txt', mimeType: 'text/plain;charset=utf-8',
  contextId: 'file_notes', source: 'pasted-text' });

async function fixture(attachments: MobileOutboxAttachment[] = [image()]) {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://active.test', environmentId: 'env', generation: 4, connection: 'connected', configLive: true });
  client.scopes = ['orchestration:operate'];
  client.config = { environment: { capabilities: { attachmentUploads: true, fileAttachments: { maxUploadBytes: 50 * 1024 * 1024 } } } };
  let record: MobileOutboxRecord = { schemaVersion: 1, origin: 'https://home.test', environmentId: 'env', threadId: 'thread',
    messageId: 'message', commandId: 'command', text: 'send', attachments, createdAt: '2026-10-08T00:00:00.000Z' };
  let token = 'epoch:1', revision = 1, floor = 1, held = false, minted = 0;
  let mutationMode = 'committed', session: Obj = { authenticated: true, permissions: ['orchestration:operate'] };
  let after: ((input: Obj) => void | Promise<void>) | null = null;
  const calls: Obj[] = [], errors = new Map<string, unknown>(), outcomes: Obj[] = [];
  const row = () => ({ record, revision, token, pending: false, held });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input), operation = String(request.method || request.action || request.op); calls.push(request);
    if (errors.has(operation)) throw errors.get(operation);
    let value: unknown;
    if (operation === 'read') value = { ownerEpoch: 'epoch', sequenceFloor: floor, complete: true, errors: [], records: [row()],
      revisions: { message: revision }, tokens: { message: token }, outcomes, mutations: [], transfers: [] };
    else if (operation === 'confirmQueued') value = { current: !held && request.token === token && request.expectedRevision === revision, revision };
    else if (operation === 'status' && request.op === 'mobileOutbox') value = outcomes.find(value => value.mutationId === request.mutationId);
    else if (operation === 'status') value = { origin: client.origin, environmentId: client.environmentId, homeOrigin: 'https://home.test' };
    else if (operation === 'http') value = session;
    else if (['snapshotDraftRead', 'composerAttachRead'].includes(operation)) value = { base64: 'YWJj', sizeBytes: 3 };
    else if (operation === 'assets.createUrl') value = { url: '/assets/existing' };
    else if (operation === 'attachments.createUploadUrl') { minted++; value = { attachmentId: `upload-${minted}`, relativeUrl: `/api/attachments/upload/${minted}` }; }
    else if (['uploadAttachment', 'attachments.delete'].includes(operation)) value = {};
    else if (operation === 'mutate') {
      await after?.(request);
      floor = Number(String(request.mutationId).split(':')[1]);
      const admitted = request.expectedToken === token && request.expectedRevision === revision && (!request.requireUnheld || !held);
      const status = !admitted ? 'stale' : mutationMode === 'lost' ? 'committed' : mutationMode;
      if (status === 'committed') { record = request.record as MobileOutboxRecord; token = String(request.mutationId); revision++; }
      const outcome = { mutationId: request.mutationId, messageId: 'message', status, revision, ownerEpoch: 'epoch', sequenceFloor: floor,
        record: status === 'committed' ? record : null, removed: null, message: '', current: row() };
      outcomes.push(outcome);
      if (mutationMode === 'lost') throw new Error('Lost committed reply');
      return { ok: true, generation: 4, value: outcome };
    } else throw new Error(`Unexpected ${operation}`);
    await after?.(request);
    return { ok: true, generation: 4, value };
  } };
  expect(await mobileOutboxRead(client, native)).toBe(true);
  const run = () => mobileOutboxPrepareAttachments(client, native, mobileOutboxCapture(client, 'message')!);
  return { client, native, run, calls, errors, disk: () => record, setHeld: (value: boolean) => { held = value; },
    setMode: (value: string) => { mutationMode = value; }, setSession: (value: Obj) => { session = value; },
    setAfter: (value: typeof after) => { after = value; }, replace: () => { record = { ...record, text: 'newer' }; token = 'epoch:9'; revision++; } };
}
const ops = (f: Awaited<ReturnType<typeof fixture>>) => f.calls.map(value => value.method || value.action || value.op);

test('uploads in mixed order and adopts references through one unheld revision CAS before ready', async () => {
  const f = await fixture([image(), file(), image('3')]), result = await f.run();
  expect(result.status).toBe('ready');
  if (result.status !== 'ready') throw new Error(result.reason);
  expect(result.pendingAttachmentIds).toEqual(['upload-1', 'upload-2', 'upload-3']);
  expect(result.attachments.map(value => value.attachment.type)).toEqual(['image', 'file', 'image']);
  expect(result.attachments[1]!.attachment.source).toEqual({ _tag: 'pasted-text' });
  expect(f.calls.filter(value => value.op === 'uploadAttachment')).toHaveLength(3);
  for (const call of f.calls.filter(value => value.op === 'uploadAttachment')) expect(call).toMatchObject({ generation: 4, expectedOrigin: 'https://home.test', expectedEnvironmentId: 'env' });
  expect(f.calls.find(value => value.action === 'mutate')).toMatchObject({ operation: 'update', expectedToken: 'epoch:1', expectedRevision: 1, requireUnheld: true });
  expect(f.disk().attachments.map(value => value.uploadEnvironmentId)).toEqual(['env', 'env', 'env']);
  expect(ops(f).at(-1)).toBe('confirmQueued');
  expect(ops(f)).not.toContain('orchestration.dispatchCommand');
});
test('saved references are freshly verified without reading bytes or minting', async () => {
  const f = await fixture([{ ...image(), status: 'ready', uploadId: 'saved', uploadEnvironmentId: 'env' }]);
  const result = await f.run(); expect(result.status).toBe('ready');
  expect(ops(f)).toContain('assets.createUrl'); expect(ops(f)).not.toContain('snapshotDraftRead'); expect(ops(f)).not.toContain('mutate');
});
test('only a missing asset permits reupload; auth and transport errors preserve references', async () => {
  for (const kind of ['AssetAttachmentNotFoundError', 'Unauthorized', 'transport']) {
    const f = await fixture([{ ...image(), status: 'ready', uploadId: 'saved', uploadEnvironmentId: 'env' }]);
    f.errors.set('assets.createUrl', new ClientError('Asset lookup failed', kind));
    expect((await f.run()).status).toBe(kind === 'AssetAttachmentNotFoundError' ? 'ready' : 'blocked');
    expect(ops(f).includes('attachments.createUploadUrl')).toBe(kind === 'AssetAttachmentNotFoundError');
    expect(ops(f)).not.toContain('attachments.delete');
  }
});
test('older-server images remain inline with no adoption or early persistChatAttachments', async () => {
  const f = await fixture(); obj(f.client.config.environment).capabilities = {};
  const result = await f.run(); expect(result.status).toBe('ready');
  if (result.status !== 'ready') throw new Error(result.reason);
  expect(result.attachments).toEqual([{ localId: image().id, kind: 'inline-image', attachment: {
    type: 'image', name: 'image.png', mimeType: 'image/png', sizeBytes: 3, dataUrl: 'data:image/png;base64,YWJj' } }]);
  expect(result.pendingAttachmentIds).toEqual([]); expect(f.disk().attachments[0]!.uploadId).toBe('');
  expect(ops(f)).not.toContain('mutate'); expect(ops(f)).not.toContain('assets.persistChatAttachments');
});
test('Files-selected image requires file capability before wire promotion', async () => {
  const f = await fixture([{ ...file(), name: 'photo.png', mimeType: 'image/png' }]);
  obj(f.client.config.environment).capabilities = { attachmentUploads: true };
  expect((await f.run()).status).toBe('blocked'); expect(ops(f)).not.toContain('composerAttachRead');
});
test('explicit empty grant prevents any upload', async () => {
  const f = await fixture(); f.setSession({ authenticated: true, permissions: [], scopes: ['orchestration:operate'] });
  expect((await f.run()).status).toBe('blocked'); expect(ops(f)).not.toContain('snapshotDraftRead');
});
test('connection change after mint never posts to replacement and leaves orphan for server expiry', async () => {
  const f = await fixture(); f.setAfter(request => { if (request.method === 'attachments.createUploadUrl') { f.client.generation++; f.client.origin = 'https://replacement.test'; } });
  expect((await f.run()).status).toBe('blocked'); expect(ops(f)).not.toContain('uploadAttachment');
  expect(ops(f)).not.toContain('attachments.delete'); expect(f.disk().attachments[0]!.uploadId).toBe('');
});
test('editor hold acquired after POST refuses adoption and cleans only newly minted IDs', async () => {
  const f = await fixture([{ ...image(), status: 'ready', uploadId: 'saved', uploadEnvironmentId: 'env' }, image('3')]);
  f.setAfter(request => { if (request.op === 'uploadAttachment') f.setHeld(true); });
  expect((await f.run()).status).toBe('abandoned');
  expect(f.calls.filter(value => value.method === 'attachments.delete').map(value => obj(value.payload).attachmentId)).toEqual(['upload-1']);
  expect(f.disk().attachments[1]!.uploadId).toBe('');
});
test('hold racing native adoption CAS refuses update', async () => {
  const f = await fixture(); f.setAfter(request => { if (request.action === 'mutate') f.setHeld(true); });
  expect((await f.run()).status).toBe('abandoned'); expect(f.disk().attachments[0]!.uploadId).toBe('');
  expect(ops(f)).toContain('attachments.delete');
});
test('replacement racing adoption preserves its payload', async () => {
  const f = await fixture(); f.setAfter(request => { if (request.action === 'mutate') f.replace(); });
  expect((await f.run()).status).toBe('abandoned'); expect(f.disk().text).toBe('newer'); expect(ops(f)).toContain('attachments.delete');
});
test('lost committed adoption keeps uploaded bytes and recovers through existing mutation status', async () => {
  const f = await fixture(); f.setMode('lost'); const result = await f.run(); expect(result.status).toBe('uncertain');
  if (result.status !== 'uncertain') throw new Error('Expected uncertain');
  expect(f.disk().attachments[0]!.uploadId).toBe('upload-1'); expect(ops(f)).not.toContain('attachments.delete');
  expect((await f.run()).status).toBe('abandoned');
  expect((await mobileOutboxStatus(f.client, f.native, 'message', result.mutationId)).status).toBe('committed');
  expect(mobileOutboxSnapshot(f.client).rows[0]!.record.attachments[0]!.uploadId).toBe('upload-1');
});
test('uncertain and failed saves have different cleanup ownership', async () => {
  for (const mode of ['uncertain', 'failed']) {
    const f = await fixture(); f.setMode(mode);
    expect((await f.run()).status).toBe(mode === 'uncertain' ? 'uncertain' : 'abandoned');
    expect(ops(f).includes('attachments.delete')).toBe(mode === 'failed');
  }
});
test('LetGo after POST never invents adoption or starts cleanup calls outside the answer', async () => {
  const f = await fixture(); f.errors.set('uploadAttachment', { name: 'FetchError', kind: 'Aborted' });
  expect((await f.run()).status).toBe('abandoned'); expect(ops(f)).not.toContain('mutate'); expect(ops(f)).not.toContain('attachments.delete');
});
test('config change during upload retains local bytes and never adopts the stale preparation', async () => {
  const f = await fixture(); f.setAfter(request => { if (request.op === 'uploadAttachment') f.client.config = {}; });
  expect((await f.run()).status).toBe('abandoned'); expect(ops(f)).not.toContain('mutate');
  expect(ops(f)).toContain('attachments.delete'); expect(f.disk().attachments[0]!.uploadId).toBe('');
});
test('legacy inline preparation preserves original MIME without modern upload normalization', async () => {
  const f = await fixture([{ ...image(), name: 'photo.heic', mimeType: 'image/heic' }]);
  obj(f.client.config.environment).capabilities = {};
  const result = await f.run(); expect(result.status).toBe('ready');
  if (result.status === 'ready') expect(result.attachments[0]!.attachment.mimeType).toBe('image/heic');
});
test('invalid retained wire IDs are refused before verification or adoption', async () => {
  for (const uploadId of ['remote/id', 'remote.id', 'x'.repeat(129)]) {
    const f = await fixture([{ ...image(), status: 'ready', uploadId, uploadEnvironmentId: 'env' }]);
    expect((await f.run()).status).toBe('blocked'); expect(ops(f)).not.toContain('assets.createUrl');
    expect(ops(f)).not.toContain('mutate');
  }
});
