import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileOutboxRead, mobileOutboxCapture, mobileOutboxSnapshot } from './mobile-outbox';
import { mobileOutboxDeliveryDecode, mobileOutboxDeliveryStatus, mobileOutboxDeliveryReserve, mobileOutboxDeliverySend,
  mobileOutboxDeliveryReserveInline, mobileOutboxDeliveryRecover, mobileOutboxDeliveryRetire, mobileOutboxDeliveryComplete, type MobileOutboxDeliveryReceipt } from './mobile-outbox-delivery';
import { mobileOutboxMessagePlan, mobileOutboxLaunchPlan, type MobileOutboxWireRequest, type MobileOutboxWireFacts } from './mobile-outbox-wire';
import { mobileOutboxCompactInline, mobileOutboxMaterializeInline } from './mobile-outbox-inline';
import type { MobileOutboxInlineReceipt } from './mobile-outbox-inline-delivery';
import type { MobileOutboxRecord } from './mobile-outbox-model';

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const record = (): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://home.test', environmentId: 'env',
  threadId: 'thread', messageId: 'message', commandId: 'command', text: 'original', attachments: [], createdAt: '2026-10-08T00:00:00.000Z' });
const command = (): MobileOutboxWireRequest => ({ owner: { origin: 'https://home.test', environmentId: 'env', threadId: 'thread',
  messageId: 'message', commandId: 'command' }, stage: 'start-turn', method: 'orchestration.dispatchCommand',
  payload: { type: 'message.dispatch', commandId: 'command', threadId: 'thread', messageId: 'message', text: 'original',
    attachments: [], createdBy: 'user', creationSource: 'mobile', dispatchMode: { type: 'start_immediately' },
    modelSelection: { instanceId: 'codex', model: 'gpt-6' } } });
const receipt = (): MobileOutboxDeliveryReceipt => ({ kind: 'outbox', operationId: 'command', revision: 1,
  origin: 'https://home.test', environmentId: 'env', messageId: 'message', threadId: 'thread', rowToken: 'epoch:1', rowRevision: 1,
  record: record(), stage: 'start-turn', method: 'orchestration.dispatchCommand', payload: command().payload,
  attachmentIDs: [], state: 'reserved' });
const ack = (): MobileOutboxDeliveryReceipt => ({ ...receipt(), state: 'acknowledged', revision: 3, attemptRevision: 2,
  attemptPreviousState: 'reserved', result: { sequence: 42 } });
async function fixture() {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://relay.test', environmentId: 'env', generation: 7, connection: 'connected' });
  const calls: Obj[] = [];
  let row = record(), held = false, complete = true, token = 'epoch:1', revision = 1;
  let answer: unknown = { operation: receipt(), durable: true };
  let intercept: ((request: Obj) => void | Promise<void>) | undefined;
  const native: Native = { available: true, watch() { throw Error('No subscription'); }, async later(raw) {
    const request = obj(raw); calls.push(request);
    if (request.op === 'mobileOutbox') return { ok: true, generation: 0, value: { ownerEpoch: 'epoch', sequenceFloor: revision,
      complete, errors: [], records: [{ record: clone(row), revision, token, pending: false, held }],
      revisions: { message: revision }, tokens: { message: token }, outcomes: [], mutations: [], transfers: [] } };
    await intercept?.(request);
    return { ok: true, generation: request.generation ?? 0, value: clone(answer) };
  } };
  await mobileOutboxRead(client, native); calls.length = 0;
  return { client, native, calls, capture: () => mobileOutboxCapture(client, 'message')!,
    answer(value: unknown) { answer = value; }, intercept(value: typeof intercept) { intercept = value; },
    async refresh(options: { held?: boolean; complete?: boolean; edited?: boolean }) {
      held = options.held ?? held; complete = options.complete ?? complete;
      if (options.edited) { row = { ...row, text: 'newer' }; revision++; token = `epoch:${revision}`; }
      await mobileOutboxRead(client, native); calls.length = 0;
    } };
}

test('reservation binds confirmed capture and canonical home, enters native without another request', async () => {
  const f = await fixture(), input = command(), capture = f.capture();
  const pending = mobileOutboxDeliveryReserve(f.client, f.native, capture, input);
  expect(f.calls).toHaveLength(1);
  expect(f.calls[0]).toMatchObject({ op: 'mobileOutboxDelivery', action: 'reserve', generation: 7,
    expectedOrigin: 'https://home.test', expectedEnvironmentId: 'env', expectedToken: 'epoch:1', expectedRevision: 1, ownerEpoch: 'epoch' });
  input.payload.text = 'caller edited'; capture.token = 'wrong';
  expect(f.calls[0]!.payload).toMatchObject({ text: 'original' });
  const result = await pending; expect(result.durable).toBe(true); expect(result.operation?.state).toBe('reserved');
  expect(f.client.local.pending).toEqual({});
});
test('stale, held, incomplete and wrong-owner reservations never enter native', async () => {
  for (const mode of ['stale', 'held', 'incomplete', 'owner', 'endpoint'] as const) {
    const f = await fixture(), capture = f.capture(), input = command();
    if (mode === 'stale') await f.refresh({ edited: true });
    if (mode === 'held') await f.refresh({ held: true });
    if (mode === 'incomplete') await f.refresh({ complete: false });
    if (mode === 'owner') input.owner.commandId = 'other';
    if (mode === 'endpoint') f.client.environmentId = 'other';
    await expect(mobileOutboxDeliveryReserve(f.client, f.native, capture, input)).rejects.toBeInstanceOf(ClientError);
    expect(f.calls).toHaveLength(0);
  }
});
test('reservation rejects a mismatched native record or command even under matching IDs', async () => {
  for (const target of ['record', 'payload'] as const) {
    const f = await fixture(), bad = receipt(); bad[target].text = 'different';
    f.answer({ operation: bad, durable: true });
    await expect(mobileOutboxDeliveryReserve(f.client, f.native, f.capture(), command())).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
  }
});
test('retry sends only receipt identity and preserves edited row and ordinary Pending', async () => {
  const f = await fixture(), saved = { ...receipt(), state: 'uncertain' as const, revision: 3, attemptRevision: 2,
    attemptPreviousState: 'reserved' as const, error: { kind: 'transport', message: 'lost' } };
  await f.refresh({ edited: true });
  f.answer({ operation: { ...saved, state: 'acknowledged', revision: 5, attemptRevision: 4, attemptPreviousState: 'uncertain', error: undefined, result: {} }, durable: true });
  const result = await mobileOutboxDeliverySend(f.client, f.native, saved);
  expect(result.operation?.state).toBe('acknowledged');
  expect(f.calls[0]).toEqual({ op: 'mobileOutboxDelivery', action: 'send', generation: 7, expectedOrigin: 'https://home.test',
    expectedEnvironmentId: 'env', operationId: 'command', revision: 3, ownerEpoch: 'epoch' });
  expect(mobileOutboxSnapshot(f.client).rows[0]!.record.text).toBe('newer'); expect(f.client.local.pending).toEqual({});
});
test('offline cold ACK status and recovery need no client or send', async () => {
  const f = await fixture(); f.client.connection = 'disconnected'; f.answer({ operation: ack(), durable: false });
  const visible = await mobileOutboxDeliveryStatus(f.native, 'command'); expect(visible.durable).toBe(false);
  f.answer({ operation: ack(), durable: true });
  const durable = await mobileOutboxDeliveryRecover(f.native, visible.operation!); expect(durable.durable).toBe(true);
  expect(durable.operation?.result).toEqual({ sequence: 42 });
  expect(f.calls.map(item => item.action)).toEqual(['status', 'recover']);
});
test('missing status stays missing, while recover and send cannot accept missing receipts', async () => {
  const f = await fixture(); f.answer({ operation: null, durable: false });
  expect(await mobileOutboxDeliveryStatus(f.native, 'command')).toEqual({ operation: null, durable: false });
  await expect(mobileOutboxDeliveryRecover(f.native, receipt())).rejects.toMatchObject({ kind: 'protocol' });
  await expect(mobileOutboxDeliverySend(f.client, f.native, receipt())).rejects.toMatchObject({ kind: 'protocol' });
  f.answer({ operation: null, durable: true });
  await expect(mobileOutboxDeliveryStatus(f.native, 'command')).rejects.toMatchObject({ kind: 'protocol' });
});
test('let-go propagates without a follow-up request or shared state mutation', async () => {
  const f = await fixture(), before = mobileOutboxSnapshot(f.client);
  f.intercept(() => { throw { name: 'FetchError', kind: 'Aborted' }; });
  let caught: unknown;
  try { await mobileOutboxDeliverySend(f.client, f.native, receipt()); } catch (error) { caught = error; }
  expect(letGo(caught)).toBe(true); expect(f.calls).toHaveLength(1);
  expect(mobileOutboxSnapshot(f.client)).toEqual(before); expect(f.client.local.pending).toEqual({});
});
test('a generation change while awaiting delivery leaves recovery to the native receipt', async () => {
  const f = await fixture(); f.answer({ operation: ack(), durable: true }); f.intercept(() => { f.client.generation++; });
  await expect(mobileOutboxDeliverySend(f.client, f.native, receipt())).rejects.toMatchObject({ kind: 'stale', uncertain: true });
  expect(f.calls).toHaveLength(1);
});
test('known-unsent retirement retains identity and issued work cannot retire', async () => {
  const f = await fixture(); f.answer({ operation: { ...receipt(), state: 'retired', revision: 2 }, durable: true, releases: [] });
  expect((await mobileOutboxDeliveryRetire(f.native, receipt())).operation?.state).toBe('retired');
  await expect(mobileOutboxDeliveryRetire(f.native, ack())).rejects.toMatchObject({ kind: 'outbox-stale' });
  expect(f.calls).toHaveLength(1);
});
test('reply lifecycle rejects bad owners, attempt lineage and non-JSON results', () => {
  for (const patch of [{ origin: 'https://wrong.test' }, { revision: true }, { attemptRevision: 1 }, { rowRevision: 0 },
    { state: 'rejected', attemptPreviousState: 'uncertain', result: undefined, error: {} }, { result: Infinity },
    { result: undefined }, { attachmentIDs: ['unknown'] }, { extra: true }, { state: 'issued' }, { attemptPreviousState: 'rejected' }]) {
    expect(() => mobileOutboxDeliveryDecode({ ...ack(), ...patch }, 'command')).toThrow(ClientError);
  }
  expect(() => mobileOutboxDeliveryDecode(ack(), 'other')).toThrow(ClientError);
  const saved = ack(), decoded = mobileOutboxDeliveryDecode(saved, 'command'); decoded.payload.text = 'changed';
  expect(saved.payload.text).toBe('original');
});
test('ACK send replay and durability recovery cannot change a saved result or lifecycle', async () => {
  for (const action of ['send', 'recover'] as const) for (const replacement of [
    { ...ack(), result: { sequence: 99 } },
    { ...ack(), state: 'uncertain', revision: 5, attemptRevision: 4, result: undefined, error: { kind: 'transport' } },
  ]) {
    const f = await fixture(); f.answer({ operation: replacement, durable: true });
    const call = action === 'send' ? mobileOutboxDeliverySend(f.client, f.native, ack()) : mobileOutboxDeliveryRecover(f.native, ack());
    await expect(call).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
  }
});
test('retirement must advance to retired and attempt replies must settle the requested attempt', async () => {
  const f = await fixture();
  await expect(mobileOutboxDeliveryRetire(f.native, receipt())).rejects.toMatchObject({ kind: 'protocol' });
  for (const result of [receipt(), { ...ack(), revision: 5, attemptRevision: 4 }, { ...ack(), state: 'issued', revision: 2, result: undefined }]) {
    f.answer({ operation: result, durable: true });
    await expect(mobileOutboxDeliverySend(f.client, f.native, receipt())).rejects.toMatchObject({ kind: 'protocol' });
  }
});

function completed(input = ack(), mutationId = 'epoch:2', phase: 'removed' | 'edited' | 'uncertain' = 'removed'): MobileOutboxDeliveryReceipt {
  const edited = { ...record(), text: 'newer' };
  const outcome = { mutationId, messageId: 'message', status: phase === 'removed' ? 'committed' : phase === 'edited' ? 'stale' : 'uncertain',
    message: '', ownerEpoch: 'epoch', sequenceFloor: 2, revision: 2, record: null,
    removed: phase === 'edited' ? null : record(),
    current: { record: phase === 'edited' ? edited : null, revision: 2, token: 'epoch:2', pending: false } };
  return { ...input, revision: input.revision + 2, cleanup: { ackRevision: 3, intentRevision: input.revision + 1,
    mutationId, ownerEpoch: mutationId.slice(0, mutationId.lastIndexOf(':')), phase, outcome } };
}
function completionAnswer(input: MobileOutboxDeliveryReceipt) {
  return { operation: input, durable: true, cleanup: input.cleanup!.phase, outcome: input.cleanup!.outcome };
}
test('offline completion adopts the exact native removal through the existing outbox owner', async () => {
  const f = await fixture(); f.client.connection = 'disconnected'; f.answer(completionAnswer(completed()));
  const result = await mobileOutboxDeliveryComplete(f.client, f.native, ack());
  expect(result.cleanup).toBe('removed'); expect(result.operation.result).toEqual({ sequence: 42 });
  expect(f.calls).toEqual([{ op: 'mobileOutboxDelivery', action: 'complete', operationId: 'command', revision: 3, ownerEpoch: 'epoch', mutationId: 'epoch:2' }]);
  expect(mobileOutboxSnapshot(f.client).rows).toHaveLength(0); expect(f.client.local.pending).toEqual({});
});
test('edited and uncertain cleanup outcomes remain distinct from delivery ACK', async () => {
  for (const phase of ['edited', 'uncertain'] as const) {
    const f = await fixture(), saved = completed(ack(), 'epoch:2', phase); f.answer(completionAnswer(saved));
    const result = await mobileOutboxDeliveryComplete(f.client, f.native, ack());
    expect(result.operation.state).toBe('acknowledged'); expect(result.cleanup).toBe(phase);
    if (phase === 'edited') expect(mobileOutboxSnapshot(f.client).rows[0]?.record.text).toBe('newer');
    else expect(result.outcome?.status).toBe('uncertain');
  }
});
test('settings completion retains the queue and makes no network request', async () => {
  const f = await fixture(), input: MobileOutboxDeliveryReceipt = { ...ack(), operationId: 'command:runtime-mode', stage: 'settings-sync',
    payload: { type: 'thread.runtime-mode.set', commandId: 'command:runtime-mode', threadId: 'thread', runtimeMode: 'full-access' } };
  const saved: MobileOutboxDeliveryReceipt = { ...input, revision: 4, cleanup: { ackRevision: 3, intentRevision: 4, phase: 'settings', outcome: null } };
  f.answer(completionAnswer(saved)); expect((await mobileOutboxDeliveryComplete(f.client, f.native, input)).cleanup).toBe('settings');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.record).toEqual(record()); expect(f.calls).toHaveLength(1);
});
test('unknown completion identity requires status recovery before any queue adoption', async () => {
  const f = await fixture(), saved = completed(ack(), 'old-epoch:7'), before = mobileOutboxSnapshot(f.client);
  f.answer(completionAnswer(saved));
  await expect(mobileOutboxDeliveryComplete(f.client, f.native, ack())).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
  expect(mobileOutboxSnapshot(f.client)).toEqual(before);
  const latest = await mobileOutboxDeliveryStatus(f.native, 'command');
  expect((await mobileOutboxDeliveryComplete(f.client, f.native, latest.operation!)).cleanup).toBe('removed');
  expect(f.calls.map(call => call.action)).toEqual(['complete', 'status', 'complete']);
});
test('a saved cleanup outcome cannot erase a newer projected row', async () => {
  const f = await fixture(), saved = completed();
  await f.refresh({ edited: true }); await f.refresh({ edited: true });
  f.answer(completionAnswer(saved));
  await mobileOutboxDeliveryComplete(f.client, f.native, saved);
  expect(mobileOutboxSnapshot(f.client).rows[0]?.record.text).toBe('newer');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.nativeRevision).toBe(3);
});
test('cleanup validation rejects mismatched outcomes and changed saved cleanup before adoption', async () => {
  for (const mode of ['outer', 'removed', 'mutation', 'identity', 'terminal', 'intent', 'result'] as const) {
    const f = await fixture(), saved = completed(), before = mobileOutboxSnapshot(f.client);
    const input = mode === 'identity' || mode === 'terminal' ? clone(saved) : ack();
    const raw = completionAnswer(saved);
    if (mode === 'outer') raw.outcome = { ...saved.cleanup!.outcome, message: 'different' };
    if (mode === 'removed') saved.cleanup!.outcome!.removed = { ...record(), text: 'different' };
    if (mode === 'mutation') saved.cleanup!.outcome!.mutationId = 'epoch:999';
    if (mode === 'identity') { saved.cleanup!.mutationId = 'epoch:999'; saved.cleanup!.outcome!.mutationId = 'epoch:999'; }
    if (mode === 'terminal') { saved.cleanup!.phase = 'uncertain'; saved.cleanup!.outcome!.status = 'uncertain'; raw.cleanup = 'uncertain'; }
    if (mode === 'intent') saved.cleanup!.intentRevision = 5;
    if (mode === 'result') saved.result = { sequence: 99 };
    f.answer(raw);
    await expect(mobileOutboxDeliveryComplete(f.client, f.native, input)).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
    expect(mobileOutboxSnapshot(f.client)).toEqual(before);
  }
});
test('explicit cleanup retry binds the new identity and advances the existing sequence', async () => {
  const f = await fixture(), input = completed(ack(), 'old:2', 'edited');
  const next = completed(input); f.answer(completionAnswer(next));
  expect((await mobileOutboxDeliveryComplete(f.client, f.native, input, 5)).cleanup).toBe('removed');
  expect(f.calls[0]).toMatchObject({ mutationId: 'epoch:2', revision: 5, retryCleanupRevision: 5 });
  await expect(mobileOutboxDeliveryComplete(f.client, f.native, next, next.revision)).rejects.toMatchObject({ kind: 'outbox-stale' });
  expect(f.calls).toHaveLength(1);
});
test('lost completion reply leaves state intact and requires explicit saved status', async () => {
  const f = await fixture(), before = mobileOutboxSnapshot(f.client);
  f.intercept(() => { throw { name: 'FetchError', kind: 'Aborted' }; });
  let caught: unknown;
  try { await mobileOutboxDeliveryComplete(f.client, f.native, ack()); } catch (error) { caught = error; }
  expect(letGo(caught)).toBe(true); expect(f.calls).toHaveLength(1); expect(mobileOutboxSnapshot(f.client)).toEqual(before);
});

function inlineFixture(launch = false, inlineContext = true) {
  const localIds = ['11111111-0000-4000-a000-000000000001', '22222222-0000-4000-a000-000000000002', '33333333-0000-4000-a000-000000000003'];
  const original: MobileOutboxRecord = { ...record(), text: '[Photo](t3-context://v1/image/photo)',
    modelSelection: { instanceId: 'p', model: 'm' }, attachments: [
      { id: localIds[0]!, kind: 'image', name: 'first.png', mimeType: 'image/png', sizeBytes: 1, status: 'staged', uploadId: '' },
      { id: localIds[1]!, kind: 'file', name: 'file.txt', mimeType: 'text/plain', sizeBytes: 2, status: 'ready',
        uploadId: 'existing-file', uploadEnvironmentId: 'env', contextId: 'file', source: 'file' },
      { id: localIds[2]!, kind: 'image', name: 'last.png', mimeType: 'image/png', sizeBytes: 2, status: 'staged', uploadId: '' }],
    context: { version: 1, records: [{ version: 1, kind: 'image', contextId: 'photo', label: 'Photo', attachmentId: localIds[0] }] } };
  if (launch) original.creation = { projectId: 'project', projectCwd: '/repo', workspaceMode: 'worktree', branch: 'main', worktreePath: null };
  const facts: MobileOutboxWireFacts = { origin: original.origin, environmentId: 'env', config: { providers: [], environment: {
    environmentId: 'env', capabilities: { attachmentUploads: false, inlineMessageContext: inlineContext, serverResolvedCommandContext: true } } },
    attachments: original.attachments.map((file, index) => ({ localId: file.id, kind: file.kind === 'image' ? 'inline-image' : 'reference',
      attachment: { type: file.kind, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes,
        ...(file.kind === 'image' ? { dataUrl: `data:image/png;base64,${index === 0 ? 'YQ==' : 'YmM='}` } : { id: file.uploadId }) } })) };
  const plan = launch ? mobileOutboxLaunchPlan(original, facts, 't3/captured') : mobileOutboxMessagePlan(original, facts,
    { origin: original.origin, environmentId: 'env', threadId: 'thread', modelSelection: original.modelSelection!, runtimeMode: 'full-access', interactionMode: 'default' });
  if (plan.status !== 'needs-inline-persistence') throw Error(JSON.stringify(plan));
  const compact = mobileOutboxCompactInline(original, plan.value); if (compact.status !== 'ready') throw Error(JSON.stringify(compact));
  const source: MobileOutboxInlineReceipt = { kind: 'outbox-inline', operationId: 'aaaaaaaa-0000-4000-a000-000000000001',
    revision: 3, state: 'acknowledged', attemptRevision: 2, attemptPreviousState: 'reserved', payloadDigest: 'b'.repeat(64),
    origin: original.origin, environmentId: 'env', messageId: original.messageId, threadId: original.threadId, rowToken: 'epoch:1', rowRevision: 1,
    record: original, attachmentIDs: localIds, template: { ...compact.value, inline: compact.value.inline.map(binding => ({ ...binding, sha256: 'c'.repeat(64) })) },
    result: { attachments: compact.value.inline.map(binding => { const file = original.attachments[binding.index]!;
      return { type: 'image', id: 'duplicate-valid-server-id', name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes }; }) } };
  const final = mobileOutboxMaterializeInline(source.template, source.result); if (final.status !== 'ready') throw Error(JSON.stringify(final));
  const linked: MobileOutboxDeliveryReceipt = { ...receipt(), record: original, payload: final.value.payload,
    method: final.value.method, attachmentIDs: localIds,
    inlineSource: { operationId: source.operationId, ackRevision: source.revision, payloadDigest: source.payloadDigest! } };
  return { source, linked, localIds };
}
test('saved inline ACK binds source launch/message commands without changing original queue or idless context', async () => {
  for (const launch of [false, true]) for (const inlineContext of [false, true]) {
    const f = await fixture(), input = inlineFixture(launch, inlineContext), before = mobileOutboxSnapshot(f.client);
    f.answer({ operation: input.linked, durable: true });
    expect((await mobileOutboxDeliveryReserveInline(f.client, f.native, input.source)).operation).toEqual(input.linked);
    expect(f.calls).toEqual([{ op: 'mobileOutboxDelivery', action: 'reserveInline', inlineOperationId: input.source.operationId,
      inlineRevision: 3, ownerEpoch: 'epoch', expectedOrigin: input.source.origin, expectedEnvironmentId: 'env', generation: 7 }]);
    const content = launch ? obj(input.linked.payload.initialMessage) : input.linked.payload;
    expect((content.attachments as Obj[]).map(value => value.id)).toEqual(['duplicate-valid-server-id', 'existing-file', 'duplicate-valid-server-id']);
    if (inlineContext) expect((obj(content.context).records as Obj[])[0]!.attachmentId).toBe(input.localIds[0]);
    else expect(content.context).toBeUndefined();
    expect(input.linked.record.attachments[0]!.uploadId).toBe(''); expect(mobileOutboxSnapshot(f.client)).toEqual(before);
  }
});
test('linked reservation freezes the ACK and rejects every different final identity', async () => {
  for (const mode of ['record', 'payload', 'source', 'row', 'token', 'durability'] as const) {
    const f = await fixture(), { source, linked } = inlineFixture(), bad = clone(linked);
    if (mode === 'record') bad.record.text = 'different';
    if (mode === 'payload') bad.payload.text = 'different';
    if (mode === 'source') bad.inlineSource!.payloadDigest = 'd'.repeat(64);
    if (mode === 'row') bad.rowRevision++;
    if (mode === 'token') bad.rowToken += ':other';
    f.answer({ operation: bad, durable: mode !== 'durability' });
    await expect(mobileOutboxDeliveryReserveInline(f.client, f.native, source)).rejects.toMatchObject({ kind: 'protocol', uncertain: true });
  }
  const f = await fixture(), { source, linked } = inlineFixture(), expected = clone(linked); f.answer({ operation: expected, durable: true });
  const pending = mobileOutboxDeliveryReserveInline(f.client, f.native, source); source.result!.attachments[0]!.id = 'caller-change'; source.record.text = 'caller-change';
  expect((await pending).operation).toEqual(expected);
});
test('linked pointer is closed, start-turn-only and immutable through existing recovery/send', async () => {
  for (const mode of ['extra', 'id', 'revision', 'digest', 'settings'] as const) {
    const { linked } = inlineFixture(), value = linked as unknown as Obj;
    if (mode === 'extra') obj(value.inlineSource).extra = true;
    if (mode === 'id') obj(value.inlineSource).operationId = 'not-a-uuid';
    if (mode === 'revision') obj(value.inlineSource).ackRevision = 2;
    if (mode === 'digest') obj(value.inlineSource).payloadDigest = 'A'.repeat(64);
    if (mode === 'settings') { value.stage = 'settings-sync'; value.operationId = 'command:runtime-mode'; value.payload = { type: 'thread.runtime-mode.set', commandId: value.operationId, threadId: 'thread', runtimeMode: 'full-access' }; }
    expect(() => mobileOutboxDeliveryDecode(value, String(value.operationId))).toThrow(ClientError);
  }
  const f = await fixture(), { linked } = inlineFixture();
  const final: MobileOutboxDeliveryReceipt = { ...linked, state: 'acknowledged', revision: 3, attemptRevision: 2, attemptPreviousState: 'reserved', result: { sequence: 3 } };
  f.answer({ operation: final, durable: true }); expect((await mobileOutboxDeliverySend(f.client, f.native, linked)).operation).toEqual(final);
  const changed = clone(final); changed.inlineSource!.operationId = 'bbbbbbbb-0000-4000-a000-000000000001'; f.answer({ operation: changed, durable: true });
  await expect(mobileOutboxDeliveryRecover(f.native, final)).rejects.toMatchObject({ kind: 'protocol' });
  await expect(mobileOutboxDeliverySend(f.client, f.native, final)).rejects.toMatchObject({ kind: 'protocol' });
});
test('explicit linked rearm retains the exact inline pair and final original record', async () => {
  const f = await fixture(), { source, linked } = inlineFixture(), rearmed = { ...linked, retiredRevision: 2, revision: 3 };
  f.answer({ operation: rearmed, durable: true });
  expect((await mobileOutboxDeliveryReserveInline(f.client, f.native, source, 2)).operation).toEqual(rearmed);
  expect(f.calls[0]!.expectedRetiredRevision).toBe(2); expect(f.calls[0]!.payload).toBeUndefined();
  await expect(mobileOutboxDeliveryReserveInline(f.client, f.native, source, 0)).rejects.toMatchObject({ kind: 'protocol' });
  expect(f.calls).toHaveLength(1);
});
test('unconfirmed inline outcomes, endpoint moves and lost replies never bind another command', async () => {
  for (const mode of ['unconfirmed', 'endpoint', 'generation', 'letgo'] as const) {
    const f = await fixture(), { source, linked } = inlineFixture(), before = mobileOutboxSnapshot(f.client); f.answer({ operation: linked, durable: true });
    if (mode === 'unconfirmed') { source.state = 'uncertain'; delete source.result; source.error = { kind: 'Transport' }; }
    if (mode === 'endpoint') f.client.environmentId = 'other';
    f.intercept(() => { if (mode === 'letgo') throw { name: 'FetchError', kind: 'Aborted' }; if (mode === 'generation') f.client.generation++; });
    let caught: unknown; try { await mobileOutboxDeliveryReserveInline(f.client, f.native, source); } catch (error) { caught = error; }
    if (mode === 'letgo') expect(letGo(caught)).toBe(true); else expect(caught).toBeInstanceOf(ClientError);
    expect(f.calls).toHaveLength(['unconfirmed', 'endpoint'].includes(mode) ? 0 : 1); expect(mobileOutboxSnapshot(f.client)).toEqual(before);
  }
});
