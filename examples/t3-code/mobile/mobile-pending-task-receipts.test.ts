import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileOutboxRead, mobileOutboxSnapshot } from './mobile-outbox';
import { mobilePendingTaskReceiptsRead as read, mobilePendingTaskReceiptsPrepare as prepare } from './mobile-pending-task-receipts';
import type { MobileOutboxDeliveryReceipt, MobileOutboxDeliveryStatus } from './mobile-outbox-delivery';
import type { MobileOutboxInlineReceipt, MobileOutboxInlineStatus } from './mobile-outbox-inline-delivery';
import { mobileOutboxMessagePlan } from './mobile-outbox-wire';
import { mobileOutboxCompactInline } from './mobile-outbox-inline';
import type { MobileOutboxRecord } from './mobile-outbox-model';
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const owner = { origin: 'https://home.test', environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command' };
const record = (): MobileOutboxRecord => ({ schemaVersion: 1, ...owner, text: 'original', attachments: [], createdAt: '2026-10-08T00:00:00.000Z' });
function receipt(stage: 'final' | 'runtime-mode' | 'interaction-mode' = 'final'): MobileOutboxDeliveryReceipt {
  const commandId = stage === 'final' ? owner.commandId : owner.commandId + ':' + stage;
  return { kind: 'outbox', operationId: commandId, revision: 1, origin: owner.origin, environmentId: owner.environmentId,
    messageId: owner.messageId, threadId: owner.threadId, rowToken: 'epoch:1', rowRevision: 1, record: record(),
    stage: stage === 'final' ? 'start-turn' : 'settings-sync', method: 'orchestration.dispatchCommand',
    payload: stage === 'final' ? { type: 'message.dispatch', commandId, threadId: owner.threadId, messageId: owner.messageId,
      text: 'original', attachments: [], createdBy: 'user', creationSource: 'mobile', dispatchMode: { type: 'start_immediately' }, modelSelection: { instanceId: 'p', model: 'm' } }
      : { type: stage === 'runtime-mode' ? 'thread.runtime-mode.set' : 'thread.interaction-mode.set', commandId, threadId: owner.threadId,
        ...(stage === 'runtime-mode' ? { runtimeMode: 'full-access' } : { interactionMode: 'default' }) },
    attachmentIDs: [], state: 'reserved' };
}
function terminal<T extends MobileOutboxDeliveryReceipt | MobileOutboxInlineReceipt>(base: T, state: 'issued' | 'uncertain' | 'acknowledged' | 'rejected'): T {
  return { ...copy(base), state, revision: state === 'issued' ? 2 : 3, attemptRevision: 2, attemptPreviousState: 'reserved',
    ...(base.kind === 'outbox-inline' ? { payloadDigest: 'a'.repeat(64) } : {}),
    ...(state === 'acknowledged' ? { result: base.kind === 'outbox-inline' ? { attachments: base.record.attachments.map(a => ({ type: 'image', id: 'remote-image', name: a.name, mimeType: a.mimeType, sizeBytes: a.sizeBytes })) } : { sequence: 1 } } : state === 'issued' ? {} : { error: { kind: 'transport', message: 'saved outcome' } }) };
}
function inline(): MobileOutboxInlineReceipt {
  const r = record(); r.attachments = [{ kind: 'image', id: 'bbbbbbbb-2222-4000-a000-000000000001', name: 'photo.png', mimeType: 'image/png', sizeBytes: 3, status: 'staged', uploadId: '' }];
  r.modelSelection = { instanceId: 'p', model: 'm' };
  const planned = mobileOutboxMessagePlan(r, { origin: owner.origin, environmentId: 'env', config: { providers: [],
    environment: { environmentId: 'env', capabilities: { attachmentUploads: false, inlineMessageContext: true, serverResolvedCommandContext: true } } },
    attachments: [{ localId: r.attachments[0]!.id, kind: 'inline-image', attachment: { type: 'image', name: 'photo.png', mimeType: 'image/png', sizeBytes: 3, dataUrl: 'data:image/png;base64,YWJj' } }] },
    { origin: owner.origin, environmentId: 'env', threadId: 'thread', modelSelection: r.modelSelection, runtimeMode: 'full-access', interactionMode: 'default' });
  if (planned.status !== 'needs-inline-persistence') throw Error('missing inline plan');
  const compact = mobileOutboxCompactInline(r, planned.value); if (compact.status !== 'ready') throw Error('missing compact template');
  return { kind: 'outbox-inline', operationId: 'aaaaaaaa-1111-4000-a000-000000000001', revision: 1,
    origin: owner.origin, environmentId: owner.environmentId, messageId: owner.messageId, threadId: owner.threadId,
    rowToken: 'epoch:1', rowRevision: 1, record: r, template: { ...compact.value, inline: compact.value.inline.map(binding => ({ ...binding,
      sha256: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad' })) }, attachmentIDs: r.attachments.map(a => a.id), state: 'reserved' };
}
async function fixture() {
  const client = new T3Client(), calls: Obj[] = [], deliveries = new Map<string, MobileOutboxDeliveryStatus>(), images = new Map<string, MobileOutboxInlineStatus>();
  Object.assign(client, { origin: 'https://relay.test', environmentId: 'env', generation: 7, connection: 'disconnected' });
  let row = record(), token = 'epoch:1', revision = 1, complete = true, held = true, active = true, removed = false;
  const hooks = new Map<string, (r: Obj) => void | Promise<void>>();
  const native: Native = { available: true, watch() { throw Error('No watch'); }, async later(raw) {
    const r = obj(raw); calls.push(copy(r));
    const hook = hooks.get(String(r.action)); if (hook) await hook(r);
    let value: unknown;
    if (r.op === 'mobileOutbox' && r.action === 'read') value = { ownerEpoch: 'epoch', sequenceFloor: revision, complete, errors: [],
      records: removed ? [] : [{ record: copy(row), token, revision, pending: false, held }], revisions: { message: revision }, tokens: { message: token }, outcomes: [], mutations: [], transfers: [] };
    else if (r.op === 'mobileOutboxInline' && r.action === 'lookup') value = { operations: [...images.values()].map(copy) };
    else if (['mobileOutboxDelivery', 'mobileOutboxInline'].includes(String(r.op)) && ['status', 'recover', 'retire'].includes(String(r.action))) {
      const map = r.op === 'mobileOutboxDelivery' ? deliveries : images;
      const saved = map.get(String(r.operationId));
      if (r.action === 'status') value = saved ?? { operation: null, durable: false };
      else {
        if (!saved?.operation || saved.operation.revision !== r.revision) return { ok: false, error: { kind: 'stale', message: 'Receipt revision changed' } };
        if (r.action === 'retire') {
          if (!['reserved','retired'].includes(saved.operation.state)) return { ok: false, error: { kind: 'stale', message: 'Original attempt owns the wire' } };
          if (saved.operation.state === 'reserved') { saved.operation.state = 'retired'; saved.operation.revision++; }
        }
        saved.durable = true; value = saved;
      }
    } else throw Error('No network or mutation fallback: ' + JSON.stringify(r));
    return { ok: true, generation: 0, value: copy(value) };
  } };
  await mobileOutboxRead(client, native); calls.length = 0;
  const input = { owner: copy(owner), row: mobileOutboxSnapshot(client).rows[0]!, current: () => active };
  return { client, native, calls, deliveries, images, hooks, input,
    delivery(r: MobileOutboxDeliveryReceipt, durable = true) { deliveries.set(r.operationId, { operation: r, durable }); },
    image(r: MobileOutboxInlineReceipt, durable = true) { images.set(r.operationId, { operation: r, durable }); },
    stale() { active = false; }, unhold() { held = false; }, partial() { complete = false; }, remove() { removed = true; },
    edit() { row = { ...row, text: 'winning newer queue content' }; revision++; token = 'epoch:' + revision; },
  };
}
const changes = (f: Awaited<ReturnType<typeof fixture>>) => f.calls.filter(c => ['recover','retire'].includes(String(c.action)));
test('offline held row with no original receipts is editable without network, reservations or native retention', async () => {
  const f = await fixture();
  expect(await read(f.client, f.native, f.input)).toMatchObject({ status: 'editable', final: { operation: null }, settings: [{ operation: null }, { operation: null }], inline: [] });
  expect(changes(f)).toEqual([]); expect(f.client.local.pending).toEqual({});
});
test('Read never retires reservations; Prepare recovers and retires exact final/settings/inline revisions', async () => {
  const f = await fixture(); for (const kind of ['final','runtime-mode','interaction-mode'] as const) f.delivery(receipt(kind), false); f.image(inline(), false);
  expect((await read(f.client, f.native, f.input)).status).toBe('prepare-required'); expect(changes(f)).toEqual([]);
  const result = await prepare(f.client, f.native, f.input);
  expect(result.status).toBe('editable'); expect(result.final.operation?.state).toBe('retired');
  expect(result.settings.every(s => s.operation?.state === 'retired')).toBe(true); expect(result.inline[0]?.operation?.state).toBe('retired');
  expect(changes(f).map(c => c.action)).toEqual(['recover','recover','recover','recover','retire','retire','retire','retire']);
  expect(changes(f).every(c => c.revision === 1)).toBe(true);
});
test('all stages are inspected before any retirement, even when final itself is harmless', async () => {
  const f = await fixture(); f.delivery(receipt()); f.image(terminal(inline(),'uncertain'));
  expect((await prepare(f.client,f.native,f.input)).status).toBe('recovery-required'); expect(changes(f)).toEqual([]);
  expect(f.deliveries.get('command')?.operation?.state).toBe('reserved');
});
test('issued and uncertain final authority blocks editing despite a held row', async () => {
  for (const state of ['issued','uncertain'] as const) {
    const f = await fixture(); f.delivery(terminal(receipt(),state));
    expect((await prepare(f.client,f.native,f.input)).status).toBe('recovery-required'); expect(changes(f)).toEqual([]);
  }
});
test('cold final ACK and rejection recover only durability and return original result instead of editable', async () => {
  for (const state of ['acknowledged','rejected'] as const) {
    const f = await fixture(); f.delivery(terminal(receipt(),state),false);
    expect((await read(f.client,f.native,f.input)).status).toBe('prepare-required');
    const result = await prepare(f.client,f.native,f.input);
    expect(result.status).toBe(state==='acknowledged'?'original-accepted':'original-rejected');
    expect(result.final).toMatchObject({ durable: true, operation: { state, operationId:'command' } });
    expect(changes(f).map(c=>c.action)).toEqual(['recover']);
  }
});
test('settings terminal and inline terminal receipts retain original authority and require new-identity recovery', async () => {
  for (const stage of ['settings','inline'] as const) for (const state of ['acknowledged','rejected'] as const) {
    const f = await fixture();
    if(stage==='settings') f.delivery(terminal(receipt('runtime-mode'),state)); else {
      const saved = terminal(inline(),state);
      f.image(saved);
    }
    const result=await prepare(f.client,f.native,f.input);
    expect(result.status).toBe('recovery-required'); expect(result.reason).toContain('already been attempted'); expect(changes(f)).toEqual([]);
  }
});
test('foreign original receipt cannot grant editing or retirement for this held row',async()=>{
  const f=await fixture(), r=receipt();r.record.threadId='foreign';r.threadId='foreign';r.payload.threadId='foreign';f.delivery(r);
  expect((await prepare(f.client,f.native,f.input)).status).toBe('stale');expect(changes(f)).toEqual([]);
});
test('partial inventory, missing row, changed row and released hold refuse every receipt mutation',async()=>{
  for(const kind of ['partial','remove','edit','unhold'] as const){
    const f=await fixture();f.delivery(receipt());f[kind]();
    expect((await prepare(f.client,f.native,f.input)).status).toBe('stale');expect(changes(f)).toEqual([]);
  }
});
test('native attempt that wins retirement race remains original recovery authority',async()=>{
  const f=await fixture();f.delivery(receipt());f.hooks.set('retire',()=>{ f.hooks.delete('retire');f.delivery(terminal(receipt(),'issued')); });
  expect((await prepare(f.client,f.native,f.input)).status).toBe('recovery-required');
  expect(f.deliveries.get('command')?.operation?.state).toBe('issued');expect(changes(f)).toHaveLength(1);
});
test('a new receipt discovered after retire prevents editable success without an automatic second pass',async()=>{
  const f=await fixture();f.delivery(receipt());let retired=false;
  f.hooks.set('retire',()=>{retired=true;});f.hooks.set('status',r=>{if(retired&&r.operationId==='command:runtime-mode')f.delivery(terminal(receipt('runtime-mode'),'uncertain'));});
  expect((await prepare(f.client,f.native,f.input)).status).toBe('recovery-required');expect(changes(f).map(c=>c.action)).toEqual(['retire']);
});
test('stale route before or during receipt work propagates and admits no follow-up retirement',async()=>{
  for(const when of ['before','during']){
    const f=await fixture();f.delivery(receipt());if(when==='before')f.stale();else f.hooks.set('status',()=>f.stale());
    await expect(prepare(f.client,f.native,f.input)).rejects.toMatchObject({kind:'superseded'});expect(changes(f)).toEqual([]);
  }
});
test('queue mutation during receipt inspection fails the final fresh baseline check',async()=>{
  const f=await fixture();f.hooks.set('lookup',()=>f.edit());
  expect((await read(f.client,f.native,f.input)).status).toBe('stale');expect(changes(f)).toEqual([]);
});
