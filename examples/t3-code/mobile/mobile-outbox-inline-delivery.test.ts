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
  mobileOutboxInlineRetire, mobileOutboxInlineSend, type MobileOutboxInlineReceipt } from './mobile-outbox-inline-delivery';

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

function settled(input = data().receipt, state: MobileOutboxInlineReceipt['state'] = 'acknowledged'): MobileOutboxInlineReceipt {
  const previous = input.state === 'issued' ? 'uncertain' : input.state as 'reserved' | 'uncertain' | 'rejected';
  const { result: _result, error: _error, ...base } = clone(input);
  return { ...base, state, revision: input.revision + 2, attemptRevision: input.revision + 1,
    attemptPreviousState: previous, payloadDigest: input.payloadDigest ?? 'c'.repeat(64),
    ...(state === 'acknowledged' ? { result: { attachments: input.template.inline.map(binding => {
      const local = input.record.attachments[binding.index]!;
      return { type: 'image', id: 'persisted-image', name: local.name, mimeType: local.mimeType, sizeBytes: local.sizeBytes };
    }) } } : { error: { kind: 'Transport', message: 'No confirmed reply.' } }) };
}
test('inline send carries only saved identity and preserves queue edits and shared Pending', async () => {
  const f = await fixture(), saved = settled(f.receipt, 'uncertain'); await f.refresh('edited');
  const before = mobileOutboxSnapshot(f.client), reply = settled(saved); f.answer({ operation: reply, durable: true });
  const pending = mobileOutboxInlineSend(f.client, f.native, saved);
  saved.template.commandTemplate.payload.text = 'caller changed'; saved.payloadDigest = 'a'.repeat(64);
  expect((await pending).operation).toEqual(reply);
  expect(f.calls).toEqual([{ op: 'mobileOutboxInline', action: 'send', expectedOrigin: f.record.origin,
    expectedEnvironmentId: 'env', operationId: id, revision: 3, ownerEpoch: 'epoch', generation: 7 }]);
  expect(mobileOutboxSnapshot(f.client)).toEqual(before); expect(f.client.local.pending).toEqual({});
});
test('inline attempt states preserve previous uncertainty and require explicit rejected retry', async () => {
  for (const previous of ['reserved', 'uncertain', 'issued', 'rejected'] as const) {
    for (const state of ['acknowledged', 'uncertain', 'rejected', 'reserved'] as const) {
      const f = await fixture();
      const saved = previous === 'reserved' ? f.receipt : settled(f.receipt, previous === 'issued' ? 'uncertain' : previous);
      if (previous === 'issued') { saved.state = 'issued'; saved.revision = saved.attemptRevision!; delete saved.error; }
      const output = settled(saved, state); f.answer({ operation: output, durable: true });
      const permitted = ['uncertain', 'issued'].includes(previous) ? ['acknowledged', 'uncertain'].includes(state)
        : previous === 'rejected' ? state !== 'reserved' : true;
      const promise = mobileOutboxInlineSend(f.client, f.native, saved, previous === 'rejected');
      if (permitted) expect((await promise).operation).toEqual(output);
      else await expect(promise).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
      expect(f.calls).toHaveLength(1);
    }
  }
  const f = await fixture();
  await expect(mobileOutboxInlineSend(f.client, f.native, settled(f.receipt, 'rejected'))).rejects.toMatchObject({ kind: 'outbox-stale' });
  await expect(mobileOutboxInlineSend(f.client, f.native, { ...f.receipt, state: 'retired', revision: 2 })).rejects.toMatchObject({ kind: 'outbox-stale' });
  expect(f.calls).toHaveLength(0);
});
test('ACK replay is exact and recovery never reconstructs bytes or advances attempts', async () => {
  const f = await fixture(), saved = settled(); f.answer({ operation: saved, durable: true });
  expect((await mobileOutboxInlineSend(f.client, f.native, saved)).operation).toEqual(saved);
  expect((await mobileOutboxInlineRecover(f.native, saved)).operation).toEqual(saved);
  for (const change of [(v: MobileOutboxInlineReceipt) => { v.result!.attachments[0]!.id = 'another-valid-id'; },
    (v: MobileOutboxInlineReceipt) => { v.payloadDigest = 'd'.repeat(64); },
    (v: MobileOutboxInlineReceipt) => { v.attemptRevision! += 2; v.revision += 2; }]) {
    const output = clone(saved); change(output); f.answer({ operation: output, durable: true });
    await expect(mobileOutboxInlineSend(f.client, f.native, saved)).rejects.toMatchObject({ kind: 'protocol' });
  }
  expect(f.calls.every(call => !('template' in call) && !('payload' in call) && !('payloadDigest' in call))).toBe(true);
});
test('settled reply cannot replace immutable digest/template or skip the next attempt', async () => {
  for (const mode of ['digest', 'template', 'file-digest', 'attempt', 'durability', 'missing'] as const) {
    const f = await fixture(), saved = settled(f.receipt, 'uncertain'), reply = settled(saved);
    if (mode === 'digest') reply.payloadDigest = 'b'.repeat(64);
    if (mode === 'template') reply.template.commandTemplate.payload.text = 'other';
    if (mode === 'file-digest') reply.template.inline[0]!.sha256 = 'b'.repeat(64);
    if (mode === 'attempt') { reply.attemptRevision! += 2; reply.revision += 2; }
    f.answer({ operation: mode === 'missing' ? null : reply, durable: mode !== 'durability' });
    await expect(mobileOutboxInlineSend(f.client, f.native, saved)).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
  }
});
test('ACK descriptors are closed and match the original inline subset', () => {
  for (const change of [(v: Obj) => { v.attachments = []; }, (v: Obj) => { v.extra = true; },
    (v: Obj) => { (v.attachments as Obj[])[0]!.name = 'another.png'; },
    (v: Obj) => { (v.attachments as Obj[])[0]!.mimeType = 'image/jpeg'; },
    (v: Obj) => { (v.attachments as Obj[])[0]!.sizeBytes = 99; },
    (v: Obj) => { (v.attachments as Obj[])[0]!.dataUrl = 'data:image/png;base64,YWJj'; },
    (v: Obj) => { (v.attachments as Obj[])[0]!.id = 'bad/id'; },
    (v: Obj) => { (v.attachments as Obj[])[0]!.source = {}; }]) {
    const receipt = settled(); change(receipt.result!); expect(() => mobileOutboxInlineDecode(receipt, id)).toThrow(ClientError);
  }
  for (const mode of ['digest', 'previous', 'result-and-error', 'fraction', 'overflow'] as const) {
    const value = settled() as unknown as Obj;
    if (mode === 'digest') delete value.payloadDigest;
    if (mode === 'previous') value.attemptPreviousState = 'acknowledged';
    if (mode === 'result-and-error') value.error = {};
    if (mode === 'fraction') value.attemptRevision = 2.5;
    if (mode === 'overflow') { value.attemptRevision = Number.MAX_SAFE_INTEGER; value.revision = Number.MAX_SAFE_INTEGER; }
    expect(() => mobileOutboxInlineDecode(value, id)).toThrow(ClientError);
  }
});
test('known-unsent reservation retires at its current attempt revision', async () => {
  const f = await fixture(), saved = settled(f.receipt, 'reserved'), retired = { ...saved, state: 'retired' as const, revision: saved.revision + 1 };
  f.answer({ operation: retired, durable: true });
  expect((await mobileOutboxInlineRetire(f.native, saved)).operation).toEqual(retired);
  expect((await mobileOutboxInlineRetire(f.native, retired)).operation).toEqual(retired);
  await expect(mobileOutboxInlineRetire(f.native, settled(f.receipt, 'uncertain'))).rejects.toMatchObject({ kind: 'outbox-stale' });
  expect(f.calls).toHaveLength(2);
});
test('send cancellation and endpoint replacement preserve the saved native owner without retry', async () => {
  for (const mode of ['cancel', 'generation', 'environment', 'offline'] as const) {
    const f = await fixture(), before = mobileOutboxSnapshot(f.client); f.answer({ operation: settled(), durable: true });
    if (mode === 'environment') f.client.environmentId = 'other';
    if (mode === 'offline') f.client.connection = 'disconnected';
    f.intercept(() => { if (mode === 'cancel') throw { name: 'FetchError', kind: 'Aborted' }; f.client.generation++; });
    let failure: unknown; try { await mobileOutboxInlineSend(f.client, f.native, f.receipt); } catch (error) { failure = error; }
    if (mode === 'cancel') expect(letGo(failure)).toBe(true);
    else expect(failure).toMatchObject({ kind: mode === 'generation' ? 'stale' : 'outbox-stale' });
    expect(f.calls).toHaveLength(['offline', 'environment'].includes(mode) ? 0 : 1);
    expect(mobileOutboxSnapshot(f.client)).toEqual(before);
  }
});
