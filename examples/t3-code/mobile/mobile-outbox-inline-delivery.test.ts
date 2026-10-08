import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileOutboxRead, mobileOutboxCapture, mobileOutboxSnapshot } from './mobile-outbox';
import { mobileOutboxMessagePlan } from './mobile-outbox-wire';
import { mobileOutboxCompactInline } from './mobile-outbox-inline';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxInlineDecode, mobileOutboxInlineReserve, mobileOutboxInlineStatus, mobileOutboxInlineRecover,
  mobileOutboxInlineRetire, type MobileOutboxInlineReceipt } from './mobile-outbox-inline-delivery';

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const id = 'aaaaaaaa-1111-4000-a000-000000000001';
function data() {
  const record: MobileOutboxRecord = { schemaVersion: 1, origin: 'https://home.test', environmentId: 'env', threadId: 'thread',
    messageId: 'message', commandId: 'command', text: 'photo', createdAt: '2026-10-08T00:00:00.000Z',
    modelSelection: { instanceId: 'p', model: 'm' }, attachments: [{ kind: 'image', id: 'BBBBBBBB-2222-4000-A000-000000000001',
      name: 'photo.png', mimeType: 'image/png', sizeBytes: 3, status: 'staged', uploadId: '' }] };
  const plan = mobileOutboxMessagePlan(record, { origin: record.origin, environmentId: 'env', config: { providers: [],
    environment: { environmentId: 'env', capabilities: { attachmentUploads: false, inlineMessageContext: true, serverResolvedCommandContext: true } } },
    attachments: [{ localId: record.attachments[0]!.id, kind: 'inline-image',
      attachment: { type: 'image', name: 'photo.png', mimeType: 'image/png', sizeBytes: 3, dataUrl: 'data:image/png;base64,YWJj' } }] },
    { origin: record.origin, environmentId: 'env', threadId: 'thread', modelSelection: record.modelSelection!, runtimeMode: 'full-access', interactionMode: 'default' });
  if (plan.status !== 'needs-inline-persistence') throw Error(JSON.stringify(plan));
  const compact = mobileOutboxCompactInline(record, plan.value);
  if (compact.status !== 'ready') throw Error(JSON.stringify(compact));
  const receipt: MobileOutboxInlineReceipt = { kind: 'outbox-inline', operationId: id, revision: 1,
    origin: record.origin, environmentId: 'env', messageId: 'message', threadId: 'thread', rowToken: 'epoch:1', rowRevision: 1,
    record, template: { ...compact.value, inline: compact.value.inline.map(binding => ({ ...binding,
      sha256: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad' })) },
    attachmentIDs: record.attachments.map(file => file.id), state: 'reserved' };
  return { record, template: compact.value, receipt };
}
async function fixture() {
  const f = data(), client = new T3Client(), calls: Obj[] = [];
  Object.assign(client, { origin: 'https://relay.test', environmentId: 'env', generation: 7, connection: 'connected' });
  let record = clone(f.record), held = false, complete = true, revision = 1;
  let answer: unknown = { operation: f.receipt, durable: true }, intercept: (() => void) | undefined;
  const native: Native = { available: true, watch() { throw Error('No subscription'); }, async later(raw) {
    const request = obj(raw); calls.push(request);
    if (request.op === 'mobileOutbox') return { ok: true, generation: 0, value: { ownerEpoch: 'epoch', sequenceFloor: revision,
      complete, errors: [], records: [{ record: clone(record), revision, token: `epoch:${revision}`, pending: false, held }],
      revisions: { message: revision }, tokens: { message: `epoch:${revision}` }, outcomes: [], mutations: [], transfers: [] } };
    intercept?.(); return { ok: true, generation: request.generation ?? 0, value: clone(answer) };
  } };
  await mobileOutboxRead(client, native); calls.length = 0;
  return { ...f, client, native, calls, capture: () => mobileOutboxCapture(client, 'message')!,
    answer(value: unknown) { answer = value; }, intercept(value: () => void) { intercept = value; },
    async refresh(mode: 'held' | 'incomplete' | 'edited') {
      if (mode === 'held') held = true;
      if (mode === 'incomplete') complete = false;
      if (mode === 'edited') { record.text = 'changed'; revision++; }
      await mobileOutboxRead(client, native); calls.length = 0;
    } };
}
test('inline reservation uses compact source-produced metadata and preserves captured uppercase file UUID', async () => {
  const f = await fixture(), capture = f.capture(), before = mobileOutboxSnapshot(f.client);
  const pending = mobileOutboxInlineReserve(f.client, f.native, capture, id, f.template);
  expect(f.calls).toHaveLength(1);
  expect(f.calls[0]).toMatchObject({ op: 'mobileOutboxInline', action: 'reserve', operationId: id, generation: 7,
    expectedOrigin: f.record.origin, expectedEnvironmentId: 'env', expectedToken: 'epoch:1', expectedRevision: 1, ownerEpoch: 'epoch' });
  expect(JSON.stringify(f.calls[0])).not.toContain('dataUrl'); expect(JSON.stringify(f.calls[0])).not.toContain('sha256');
  f.template.commandTemplate.payload.text = 'caller changed'; capture.token = 'other';
  expect((await pending).operation?.template.commandTemplate.payload.text).toBe('photo');
  expect(mobileOutboxSnapshot(f.client)).toEqual(before); expect(f.client.local.pending).toEqual({});
});
test('stale rows, holds, incomplete inventory and foreign connection refuse before native admission', async () => {
  for (const mode of ['held', 'incomplete', 'edited', 'environment', 'offline', 'template'] as const) {
    const f = await fixture(), capture = f.capture();
    if (['held', 'incomplete', 'edited'].includes(mode)) await f.refresh(mode as 'held' | 'incomplete' | 'edited');
    if (mode === 'environment') f.client.environmentId = 'other';
    if (mode === 'offline') f.client.connection = 'disconnected';
    if (mode === 'template') f.template.owner.commandId = 'other';
    await expect(mobileOutboxInlineReserve(f.client, f.native, capture, id, f.template)).rejects.toBeInstanceOf(ClientError);
    expect(f.calls).toHaveLength(0);
  }
});
test('reserve reply must describe the captured row and exact template', async () => {
  for (const mode of ['record', 'template', 'token', 'rowRevision', 'missing', 'durability'] as const) {
    const f = await fixture(), receipt = clone(f.receipt);
    if (mode === 'record') receipt.record.text = 'other';
    if (mode === 'template') receipt.template.commandTemplate.payload.text = 'other';
    if (mode === 'token') receipt.rowToken = 'epoch:2';
    if (mode === 'rowRevision') receipt.rowRevision++;
    f.answer({ operation: mode === 'missing' ? null : receipt, durable: mode !== 'durability' });
    await expect(mobileOutboxInlineReserve(f.client, f.native, f.capture(), id, f.template)).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
  }
});
test('offline status and recovery preserve exact native byte digest and lifecycle without rereading files', async () => {
  const f = await fixture(); f.client.connection = 'disconnected';
  f.answer({ operation: f.receipt, durable: false });
  const visible = await mobileOutboxInlineStatus(f.native, id); expect(visible.durable).toBe(false);
  f.answer({ operation: f.receipt, durable: true });
  expect((await mobileOutboxInlineRecover(f.native, visible.operation!)).durable).toBe(true);
  expect(f.calls.map(call => call.action)).toEqual(['status', 'recover']);
  expect(f.calls[1]).toEqual({ op: 'mobileOutboxInline', action: 'recover', operationId: id, revision: 1 });
  const changed = clone(f.receipt); changed.template.inline[0]!.sha256 = '0'.repeat(64);
  f.answer({ operation: changed, durable: true });
  await expect(mobileOutboxInlineRecover(f.native, f.receipt)).rejects.toMatchObject({ kind: 'protocol' });
});
test('retirement requires exact never-issued transition and exact retry', async () => {
  const f = await fixture();
  await expect(mobileOutboxInlineRetire(f.native, f.receipt)).rejects.toMatchObject({ kind: 'protocol' });
  const retired = { ...f.receipt, state: 'retired' as const, revision: 2 };
  f.answer({ operation: retired, durable: true });
  expect((await mobileOutboxInlineRetire(f.native, f.receipt)).operation).toEqual(retired);
  expect((await mobileOutboxInlineRetire(f.native, retired)).operation).toEqual(retired);
  f.answer({ operation: null, durable: false });
  expect(await mobileOutboxInlineStatus(f.native, id)).toEqual({ operation: null, durable: false });
  await expect(mobileOutboxInlineRecover(f.native, retired)).rejects.toMatchObject({ kind: 'protocol' });
});
test('invalid native metadata cannot become a saved byte owner or sendable operation', () => {
  for (const change of [(v: Obj) => { v.state = 'issued'; }, (v: Obj) => { v.revision = true; },
    (v: Obj) => { v.payload = {}; }, (v: Obj) => { v.kind = 'outbox'; }, (v: Obj) => { v.attachmentIDs = []; },
    (v: Obj) => { v.origin = 'https://other.test'; }, (v: Obj) => { obj(v.template).inline = []; },
    (v: Obj) => { (obj(v.template).inline as Obj[])[0]!.sha256 = 'x'.repeat(64); },
    (v: Obj) => { (obj(v.template).inline as Obj[])[0]!.index = -1; },
    (v: Obj) => { obj(obj(obj(v.template).commandTemplate).payload).attachments = [null]; },
    (v: Obj) => { v.extra = Infinity; }]) {
    const value = clone(data().receipt) as unknown as Obj; change(value);
    expect(() => mobileOutboxInlineDecode(value, id)).toThrow(ClientError);
  }
  const value = data().receipt, decoded = mobileOutboxInlineDecode(value, id);
  decoded.template.inline[0]!.sha256 = '0'.repeat(64); expect(value.template.inline[0]!.sha256).not.toBe('0'.repeat(64));
});
test('answer cancellation and generation replacement do not trigger another call or adopt queue state', async () => {
  for (const mode of ['cancel', 'generation'] as const) {
    const f = await fixture(), before = mobileOutboxSnapshot(f.client);
    f.intercept(() => { if (mode === 'cancel') throw { name: 'FetchError', kind: 'Aborted' }; f.client.generation++; });
    let caught: unknown;
    try { await mobileOutboxInlineReserve(f.client, f.native, f.capture(), id, f.template); } catch (error) { caught = error; }
    if (mode === 'cancel') expect(letGo(caught)).toBe(true); else expect(caught).toMatchObject({ kind: 'stale', uncertain: true });
    expect(f.calls).toHaveLength(1); expect(mobileOutboxSnapshot(f.client)).toEqual(before);
  }
});
