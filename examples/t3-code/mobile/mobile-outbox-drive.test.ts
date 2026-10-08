// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { expect, test } from 'bun:test';
import { mobilePendingTaskEditorsHydrate, mobilePendingTaskEditorsCreate, type MobilePendingTaskMarker } from './mobile-pending-task-state';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileHomeAction, mobileHomeActionsObserve } from './home-actions';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { mobileOutboxSnapshot } from './mobile-outbox';
import { mobileOutboxDeliverOne } from './mobile-outbox-foreground';
import { mobileOutboxDriveSnapshot as snapshot, mobileOutboxDriveRead as read, mobileOutboxDriveRun as run,
  mobileOutboxDriveCompleted as completed } from './mobile-outbox-drive';
import type { MobileOutboxRecord, MobileOutboxAttachment } from './mobile-outbox-model';
import type { MobileOutboxDeliveryReceipt } from './mobile-outbox-delivery';
import type { MobileOutboxInlineReceipt } from './mobile-outbox-inline-delivery';

const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const owner = { origin: 'https://home.test', environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command' };
const image = (digit = '1'): MobileOutboxAttachment => ({ kind: 'image', id: `${digit.repeat(8)}-${digit.repeat(4)}-4${digit.repeat(3)}-a${digit.repeat(3)}-${digit.repeat(12)}`,
  name: `image-${digit}.png`, mimeType: 'image/png', sizeBytes: 3, status: 'staged', uploadId: '' });
const queued = (): MobileOutboxRecord => ({ schemaVersion: 1, ...owner, text: 'original', attachments: [],
  createdAt: '2026-10-08T00:00:00.000Z', modelSelection: { instanceId: 'p', model: 'm' } });
function saved(record = queued()): MobileOutboxDeliveryReceipt {
  return { kind: 'outbox', operationId: owner.commandId, revision: 1, origin: owner.origin, environmentId: owner.environmentId,
    threadId: owner.threadId, messageId: owner.messageId, rowToken: 'epoch:1', rowRevision: 1, record: copy(record),
    stage: 'start-turn', method: 'orchestration.dispatchCommand', payload: { type: 'message.dispatch', commandId: owner.commandId,
      threadId: owner.threadId, messageId: owner.messageId, text: record.text, attachments: [], createdBy: 'user',
      creationSource: 'mobile', dispatchMode: { type: 'start_immediately' }, modelSelection: record.modelSelection },
    attachmentIDs: record.attachments.map(file => file.id), state: 'reserved' };
}
function settled<T extends MobileOutboxDeliveryReceipt | MobileOutboxInlineReceipt>(input: T, state = 'acknowledged'): T {
  const result = copy(input); delete result.error; delete result.result;
  Object.assign(result, { state, revision: input.revision + 2, attemptRevision: input.revision + 1,
    attemptPreviousState: input.state === 'issued' ? 'uncertain' : input.state });
  if (result.kind === 'outbox-inline') result.payloadDigest = 'b'.repeat(64);
  if (state === 'acknowledged') Object.assign(result, { result: result.kind === 'outbox-inline' ? {
    attachments: result.template.inline.map(binding => { const file = result.record.attachments[binding.index]!;
      return { type: 'image', id: `asset-${binding.index}`, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes }; }) } : { sequence: 42 } });
  else Object.assign(result, { error: { kind: 'transport', message: 'Lost reply' } });
  return result;
}
function fixture(input = queued()) {
  const makeClient = () => {
    const result = new T3Client();
    mobilePendingTaskEditorsHydrate(result, {});
    Object.assign(result, { origin: 'https://relay.test', environmentId: 'env', generation: 7, connection: 'connected', configLive: true, shellLive: true });
    result.scopes = ['orchestration:operate'];
    result.config = { providers: [{ instanceId: 'p', showInteractionModeToggle: true }], environment: { environmentId: 'env', capabilities: {
      serverResolvedCommandContext: true, inlineMessageContext: true, attachmentUploads: true, fileAttachments: { maxUploadBytes: 50000000 } } } };
    result.shell = { sequence: 1, projects: [{ id: 'project', workspaceRoot: '/repo' }], threads: input.creation ? [] : [{ id: 'thread', projectId: 'project',
      modelSelection: { instanceId: 'p', model: 'm' }, runtimeMode: 'full-access', interactionMode: 'default' }] };
    return result;
  };
  let client = makeClient(), record: MobileOutboxRecord | null = copy(input), revision = 1, token = 'epoch:1', floor = 1;
  let held = false, complete = true, cold = false, minted = 0, ids = 0, mutationMode = 'committed';
  let cleanupFails = false, sendState = 'acknowledged';
  let extras: MobileOutboxRecord[] = [];
  let session: Obj = { authenticated: true, permissions: ['orchestration:operate'] };
  let intercept: ((request: Obj) => void | Promise<void>) | undefined;
  const calls: Obj[] = [], outcomes: Obj[] = [];
  const deliveries = new Map<string, MobileOutboxDeliveryReceipt>(), inlines = new Map<string, MobileOutboxInlineReceipt>();
  const failures = new Map<string, unknown>();
  const key = (r: Obj) => `${r.op}:${r.action ?? r.method ?? ''}:${r.operationId ?? ''}`;
  const row = () => ({ record: copy(record), revision, token, pending: false, held });
  const wrapper = (operation: unknown) => ({ operation: copy(operation), durable: !!operation && !cold });
  function reserve(r: Obj): MobileOutboxDeliveryReceipt {
    const source = r.record as MobileOutboxRecord;
    return { kind: 'outbox', operationId: String(obj(r.payload).commandId), revision: Number(r.expectedRetiredRevision ?? 0) + 1,
      ...(r.expectedRetiredRevision === undefined ? {} : { retiredRevision: Number(r.expectedRetiredRevision) }),
      origin: source.origin, environmentId: source.environmentId, threadId: source.threadId, messageId: source.messageId,
      rowToken: String(r.expectedToken), rowRevision: Number(r.expectedRevision), record: copy(source), stage: r.stage as 'start-turn',
      method: r.method as 'orchestration.dispatchCommand', payload: copy(obj(r.payload)), attachmentIDs: source.attachments.map(file => file.id), state: 'reserved' };
  }
  const native: Native = { available: true, watch() { throw Error('Foreground delivery cannot subscribe'); }, async later(raw) {
    const r = obj(raw); calls.push(copy(r));
    const fail = failures.get(key(r)); if (fail) throw fail;
    let value: unknown;
    if (r.op === 'mobileOutbox') {
      if (r.action === 'read') value = { ownerEpoch: 'epoch', sequenceFloor: floor, complete, errors: [], records: [...(record ? [row()] : []), ...extras.map(record => ({ record, revision: 1, token: 'epoch:1', pending: false, held: false }))],
        revisions: { message: revision, ...Object.fromEntries(extras.map(record => [record.messageId, 1])) },
        tokens: { message: token, ...Object.fromEntries(extras.map(record => [record.messageId, 'epoch:1'])) }, outcomes, mutations: [], transfers: [] };
      else if (r.action === 'confirmQueued') value = { current: !!record && !held && r.token === token && r.expectedRevision === revision, revision };
      else if (r.action === 'acknowledge') {
        const index = outcomes.findIndex(item => item.mutationId === r.mutationId);
        value = { acknowledged: index >= 0 }; if (index >= 0) outcomes.splice(index, 1);
      } else if (r.action === 'mutate') {
        floor = Number(String(r.mutationId).split(':')[1]);
        const status = held || r.expectedToken !== token || r.expectedRevision !== revision ? 'stale' : mutationMode;
        if (status === 'committed') { record = copy(r.record as MobileOutboxRecord); revision++; token = String(r.mutationId); }
        value = { mutationId: r.mutationId, messageId: owner.messageId, status, revision, ownerEpoch: 'epoch', sequenceFloor: floor,
          record: status === 'committed' ? record : null, removed: null, message: '', current: row() }; outcomes.push(copy(value as Obj));
      } else throw Error(`Unexpected ${key(r)}`);
    } else if (r.op === 'mobileOutboxDelivery') {
      let operation = deliveries.get(String(r.operationId));
      if (r.action === 'status') value = wrapper(operation ?? null);
      else if (r.action === 'recover') { cold = false; value = wrapper(operation); }
      else if (r.action === 'reserve' || r.action === 'reserveInline') {
        if (r.action === 'reserve') operation = reserve(r);
        else {
          const source = inlines.get(String(r.inlineOperationId))!;
          const command = copy(source.template.commandTemplate), content = command.method === 'orchestration.launchThread' ? obj(command.payload.initialMessage) : command.payload;
          const attachments = content.attachments as Obj[];
          source.template.inline.forEach((binding, index) => { attachments[binding.index] = copy(source.result!.attachments[index]!); });
          operation = reserve({ record: source.record, expectedToken: source.rowToken, expectedRevision: source.rowRevision, ...command });
          operation.inlineSource = { operationId: source.operationId, ackRevision: source.revision, payloadDigest: source.payloadDigest! };
        }
        deliveries.set(operation.operationId, operation); value = wrapper(operation);
      } else if (r.action === 'send') { operation = settled(operation!, sendState); deliveries.set(operation.operationId, operation); value = wrapper(operation); }
      else if (r.action === 'complete') {
        if (!operation) throw Error('Missing saved delivery');
        if (!operation.cleanup || r.retryCleanupRevision !== undefined) {
          const before = operation.revision, phase = operation.stage === 'settings-sync' ? 'settings' : cleanupFails ? 'failed' : held || JSON.stringify(record) !== JSON.stringify(operation.record) ? 'edited' : 'removed';
          let outcome = null;
          if (phase !== 'settings') {
            floor = Number(String(r.mutationId).split(':')[1]); revision++; token = String(r.mutationId);
            const removed = phase === 'removed' ? copy(record) : null; if (removed) record = null;
            outcome = { mutationId: String(r.mutationId), messageId: owner.messageId, status: phase === 'removed' ? 'committed' : phase === 'failed' ? 'failed' : 'stale',
              message: '', ownerEpoch: 'epoch', sequenceFloor: floor, revision, record: null, removed, current: row() };
            outcomes.push(copy(outcome));
          }
          operation = { ...operation, revision: before + (phase === 'settings' ? 1 : 2), cleanup: {
            ackRevision: operation.cleanup?.ackRevision ?? before, intentRevision: before + 1, phase, outcome,
            ...(phase === 'settings' ? {} : { mutationId: String(r.mutationId), ownerEpoch: 'epoch' }) } };
          deliveries.set(operation.operationId, operation);
        }
        value = { ...wrapper(operation), cleanup: operation.cleanup!.phase, outcome: operation.cleanup!.outcome };
      } else throw Error(`Unexpected ${key(r)}`);
    } else if (r.op === 'mobileOutboxInline') {
      let operation = inlines.get(String(r.operationId));
      if (r.action === 'lookup') value = { operations: [...inlines.values()].map(wrapper) };
      else if (r.action === 'status') value = wrapper(operation ?? null);
      else if (r.action === 'recover') { cold = false; value = wrapper(operation); }
      else if (r.action === 'reserve') {
        const source = r.record as MobileOutboxRecord, template = copy(r.template as MobileOutboxInlineReceipt['template']);
        template.inline.forEach(binding => { binding.sha256 = 'a'.repeat(64); });
        operation = { kind: 'outbox-inline', operationId: String(r.operationId), revision: 1, origin: source.origin,
          environmentId: source.environmentId, threadId: source.threadId, messageId: source.messageId, rowToken: String(r.expectedToken),
          rowRevision: Number(r.expectedRevision), record: copy(source), attachmentIDs: source.attachments.map(file => file.id), template, state: 'reserved' };
        inlines.set(operation.operationId, operation); value = wrapper(operation);
      } else if (r.action === 'send') { operation = settled(operation!); inlines.set(operation.operationId, operation); value = wrapper(operation); }
      else throw Error(`Unexpected ${key(r)}`);
    } else if (r.op === 'status') value = { origin: 'https://relay.test', homeOrigin: owner.origin, environmentId: owner.environmentId };
    else if (r.op === 'http') value = session;
    else if (r.op === 'ids') value = Array.from({ length: Number(r.count) }, () => `aaaaaaaa-0000-4000-a000-${String(++ids).padStart(12, '0')}`);
    else if (r.op === 'snapshotDraftRead') value = { base64: 'YWJj', sizeBytes: 3 };
    else if (r.op === 'uploadAttachment' || r.method === 'attachments.delete') value = {};
    else if (r.method === 'attachments.createUploadUrl') { minted++; value = { attachmentId: `upload-${minted}`, relativeUrl: `/api/attachments/upload/${minted}` }; }
    else if (r.method === 'assets.createUrl') value = { url: '/assets/saved' };
    else if (r.method === 'orchestration.getThreadProjection') value = { thread: { id: 'thread' }, messages: [],
      runs: [{ id: 'run', status: 'running', providerThreadId: 'provider-thread' }],
      providerThreads: [{ id: 'provider-thread', providerSessionId: 'session' }],
      providerSessions: [{ id: 'session', capabilities: { turns: { supportsActiveSteering: true } } }] };
    else throw Error(`Unexpected ${key(r)}`);
    await intercept?.(r);
    return { ok: true, generation: r.generation ?? 0, value: copy(value) };
  } };
  return { get client() { return client; }, native, calls, deliveries, inlines, failures,
    run: (options?: { recover?: boolean; retryCleanupRevision?: number }) => mobileOutboxDeliverOne(client, native, owner, options),
    disk: () => record, setHeld(value = true) { held = value; }, setSendState(value: string) { sendState = value; }, setExtras(value: MobileOutboxRecord[]) { extras = value; }, setComplete(value: boolean) { complete = value; },
    setSession(value: Obj) { session = value; }, setMutation(value: string) { mutationMode = value; }, setCleanupFailure(value: boolean) { cleanupFails = value; },
    intercept(value: typeof intercept) { intercept = value; }, edit() { record = { ...record!, text: 'newer' }; revision++; token = `epoch:${++floor}`; },
    restart() { client = makeClient(); cold = true; calls.length = 0; },
  };
}
const stages = (f: ReturnType<typeof fixture>) => f.calls.filter(r => ['reserve', 'send', 'complete', 'reserveInline'].includes(String(r.action)))
  .map(r => `${r.op}:${r.action}:${r.operationId ?? obj(r.payload).commandId ?? r.inlineOperationId}`);
const writes = (f: ReturnType<typeof fixture>) => f.calls.filter(r => ['reserve', 'send', 'reserveInline', 'mutate'].includes(String(r.action)) || r.op === 'uploadAttachment');

const now = Date.parse('2026-10-08T06:00:00Z');
const second = (threadId = 'thread-2'): MobileOutboxRecord => ({ ...queued(), threadId,
  messageId: 'message-2', commandId: 'command-2', createdAt: '2026-10-08T01:00:00.000Z' });
async function loaded(f: ReturnType<typeof fixture>) { await read(f.client, f.native); f.calls.length = 0; return snapshot(f.client, now); }
function gate() { let resolve!: () => void; const promise = new Promise<void>(done => { resolve = done; }); return { promise, resolve }; }

test('real delivery removes its row through native receipt cleanup and exposes no second request', async () => {
  const f = fixture(), first = await loaded(f);
  expect(first).toMatchObject({ count: 1, complete: true, busy: false, delay: 1 });
  await run(f.client, f.native, first.next, now);
  expect(snapshot(f.client, now)).toMatchObject({ count: 0, next: '', busy: false });
  expect(stages(f)).toEqual(['mobileOutboxDelivery:reserve:command', 'mobileOutboxDelivery:send:command', 'mobileOutboxDelivery:complete:command']);
});
test('old scheduled row signature cannot read or issue after a newer edit is adopted', async () => {
  const f = fixture(), first = await loaded(f); f.edit(); await read(f.client, f.native); f.calls.length = 0;
  await run(f.client, f.native, first.next, now);
  expect(f.calls).toEqual([]); expect(snapshot(f.client, now).next).not.toBe(first.next);
});
test('partial inventory and held row never schedule, and stale key is refused after partial read', async () => {
  const f = fixture(), first = await loaded(f); f.setComplete(false); await read(f.client, f.native); f.calls.length = 0;
  expect(snapshot(f.client, now).next).toBe('');
  await run(f.client, f.native, first.next, now); expect(writes(f)).toEqual([]);
  f.setComplete(true); f.setHeld(); await read(f.client, f.native);
  expect(snapshot(f.client, now)).toMatchObject({ next: '', items: [{ held: true }] });
});
test('foreign environment has no scheduled or manual request authority', async () => {
  const f = fixture(); const first = await loaded(f); f.client.environmentId = 'other';
  expect(snapshot(f.client, now).next).toBe('');
  await run(f.client, f.native, first.next, now);
  await run(f.client, f.native, first.items[0]!.owner, now, true);
  expect(f.calls).toEqual([]);
});
test('malformed scheduled values cannot throw or issue native calls', async () => {
  const f = fixture(); await loaded(f);
  for (const key of ['null', '[]', '1', '"text"', '{}', '{', '{"owner":null}']) {
    expect(await run(f.client, f.native, key, now)).toHaveProperty('revision');
  }
  expect(f.calls).toEqual([]);
});
test('concurrent calls use one live pass and expose a busy snapshot', async () => {
  const f = fixture(), first = await loaded(f), entered = gate(), release = gate();
  f.intercept(async request => { if (request.op === 'mobileOutbox' && request.action === 'read') { entered.resolve(); await release.promise; } });
  const sending = run(f.client, f.native, first.next, now); await entered.promise;
  expect(snapshot(f.client, now)).toMatchObject({ busy: true, next: '', items: [{ status: 'sending', canRetry: false }] });
  const calls = f.calls.length; await run(f.client, f.native, first.next, now); expect(f.calls.length).toBe(calls);
  release.resolve(); await sending; expect(snapshot(f.client, now).busy).toBe(false);
});
test('uncertain retry uses Contract time, old keys expire and exact payload is reused', async () => {
  const f = fixture(), first = await loaded(f); f.setSendState('uncertain');
  await run(f.client, f.native, first.next, now);
  const retry = snapshot(f.client, now); expect(retry.items[0]!.status).toBe('retry'); expect(retry.delay).toBe(1000);
  const before = f.calls.length; await run(f.client, f.native, first.next, now + 1000); await run(f.client, f.native, retry.next, now + 999);
  expect(f.calls.length).toBe(before);
  await run(f.client, f.native, retry.next, now + 1000);
  expect(snapshot(f.client, now + 1000).delay).toBe(2000);
  expect(f.calls.filter(call => call.action === 'reserve')).toHaveLength(1);
  expect(f.calls.filter(call => call.action === 'send').map(call => call.operationId)).toEqual(['command', 'command']);
});
test('ready independent thread runs before older retry deadline', async () => {
  const f = fixture(), first = await loaded(f); f.setSendState('uncertain');
  await run(f.client, f.native, first.next, now); f.setExtras([second()]); await read(f.client, f.native);
  const next = snapshot(f.client, now);
  expect(JSON.parse(next.next).owner.messageId).toBe('message-2'); expect(next.delay).toBe(1);
});
test('held oldest row blocks its own thread without blocking another thread', async () => {
  const f = fixture(); f.setHeld(); f.setExtras([second('thread'), { ...second(), messageId: 'message-3', commandId: 'command-3' }]);
  const first = await loaded(f); expect(JSON.parse(first.next).owner.messageId).toBe('message-3');
});
test('waiting row holds its thread until connection facts change', async () => {
  const f = fixture(); f.client.connection = 'disconnected'; f.setExtras([second('thread')]);
  const first = await loaded(f); await run(f.client, f.native, first.next, now);
  expect(snapshot(f.client, now)).toMatchObject({ next: '', items: [{ status: 'waiting' }, {}] });
  f.client.connection = 'connected';
  expect(JSON.parse(snapshot(f.client, now + 1000).next).owner.messageId).toBe('message');
});
test('snapshot during durable upload adoption keeps terminal recovery on the live attempt', async () => {
  const f = fixture({ ...queued(), attachments: [image()] });
  const first = await loaded(f);
  let sawAdoption = false;
  f.intercept(request => {
    if (request.op === 'mobileOutboxDelivery' && request.action === 'reserve') {
      sawAdoption = true; expect(snapshot(f.client, now).busy).toBe(true);
      throw Error('Reservation unavailable');
    }
  });
  await run(f.client, f.native, first.next, now);
  expect(sawAdoption).toBe(true);
  expect(snapshot(f.client, now)).toMatchObject({ next: '', items: [{ status: 'recovery-required', reason: 'Reservation unavailable' }] });
});
test('manual cleanup retry names the exact ACK revision and acknowledges its saved outcome', async () => {
  const f = fixture(), first = await loaded(f); f.setCleanupFailure(true);
  await run(f.client, f.native, first.next, now);
  const failed = snapshot(f.client, now), revision = f.deliveries.get('command')!.revision;
  expect(failed).toMatchObject({ next: '', items: [{ status: 'cleanup-pending', canRetry: true }] });
  f.setCleanupFailure(false); f.calls.length = 0;
  await run(f.client, f.native, failed.items[0]!.owner, now + 1, true);
  expect(f.calls.find(call => call.action === 'complete')).toMatchObject({ retryCleanupRevision: revision });
  expect(f.calls.findIndex(call => call.action === 'acknowledge')).toBeLessThan(f.calls.findIndex(call => call.action === 'complete'));
  expect(f.calls.some(call => call.action === 'send' || call.op === 'http')).toBe(false);
  expect(snapshot(f.client, now).count).toBe(0);
});
test('let-go propagates while releasing the plain busy latch with no follow-up request', async () => {
  const f = fixture(), first = await loaded(f);
  f.failures.set('mobileOutbox:read:', { name: 'FetchError', kind: 'Aborted', message: 'The answer was released' });
  let failure: unknown;
  try { await run(f.client, f.native, first.next, now); } catch (error) { failure = error; }
  expect(letGo(failure)).toBe(true); expect(f.calls).toHaveLength(1); expect(snapshot(f.client, now).busy).toBe(false);
});

test('creation ACK bridge retains removed prompt until its exact environment and thread appear', async () => {
  const record = { ...queued(), runtimeMode: 'full-access' as const, interactionMode: 'default' as const,
    creation: { projectId: 'project', projectCwd: '/repo', workspaceMode: 'local' as const, branch: null, worktreePath: null } };
  const f = fixture(record), first = await loaded(f);
  expect(completed(f.client)).toEqual([]);
  await run(f.client, f.native, first.next, now);
  expect(f.disk()).toBeNull(); expect(snapshot(f.client, now).count).toBe(0);
  expect(completed(f.client)).toEqual([record]);
  mobileHomeActionsObserve('completed-home', true, false, f.client);
  const countBeforeOpen = f.calls.length;
  expect((await mobileHomeAction('completed-home', record.environmentId, record.threadId, 'pending-open', JSON.stringify(owner), now,
    f.native, f.client, new EnvironmentFleet())).nextLocation).toBe('/threads/env/thread');
  expect(f.calls).toHaveLength(countBeforeOpen);
  const presented = completed(f.client); presented[0]!.text = 'UI copy changed';
  expect(completed(f.client)[0]!.text).toBe('original');
  f.client.shell.threads = [{ id: 'unrelated', projectId: 'project' }];
  expect(completed(f.client)).toEqual([record]);
  f.client.environmentId = 'another-environment';
  f.client.shell.threads = [{ id: record.threadId, projectId: 'project' }];
  expect(completed(f.client)).toEqual([record]);
  f.client.environmentId = record.environmentId;
  expect(completed(f.client)).toEqual([]);
  f.client.shell.threads = [];
  expect(completed(f.client)).toEqual([]);
  f.client.environmentId = 'another-environment';
  expect(completed(f.client)).toEqual([]);
});
test('existing-thread completion and failed creation cleanup do not fabricate a removed-creation bridge', async () => {
  const existing = fixture(), initial = await loaded(existing);
  await run(existing.client, existing.native, initial.next, now);
  expect(completed(existing.client)).toEqual([]);
  const creation = fixture({ ...queued(), runtimeMode: 'full-access', interactionMode: 'default',
    creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } });
  const first = await loaded(creation); creation.setCleanupFailure(true);
  await run(creation.client, creation.native, first.next, now);
  expect(snapshot(creation.client, now).items[0]!.status).toBe('cleanup-pending');
  expect(completed(creation.client)).toEqual([]);
});

function editorMarker(record: MobileOutboxRecord): MobilePendingTaskMarker {
  const originalOwner = { origin: record.origin, environmentId: record.environmentId, threadId: record.threadId,
    messageId: record.messageId, commandId: record.commandId };
  return { version: 1, owner: originalOwner, session: 'editor-session', revision: 1,
    draftKey: `new-task:pending-${record.messageId}`, contentRevision: 1,
    baseline: { record, token: 'epoch:1', revision: 1 }, pending: null };
}
const asCreation = (record: MobileOutboxRecord): MobileOutboxRecord => ({ ...record,
  creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } });
test('unhydrated and unreadable pending-editor documents block both captured schedule and manual drive without native calls', async () => {
  for (const document of [undefined, null, { version: 2, markers: {} }, { version: 1, markers: { broken: {} } }]) {
    const f = fixture(), scheduled = await loaded(f);
    f.client.local = new T3Client().local;
    if (document !== undefined) mobilePendingTaskEditorsHydrate(f.client, { mobilePendingTaskEditors: document });
    expect(snapshot(f.client, now)).toMatchObject({ next: '', items: [{ canRetry: false, reason: 'Read saved pending edits before sending.' }] });
    await run(f.client, f.native, scheduled.next, now);
    await run(f.client, f.native, JSON.stringify(owner), now, true);
    expect(f.calls).toEqual([]); expect(f.disk()?.text).toBe('original');
  }
});
test('exact persisted editor marker blocks schedule and manual delivery even though native row has no live hold', async () => {
  const original = asCreation(queued()), f = fixture(original), scheduled = await loaded(f);
  expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(original))).not.toBeNull();
  expect(mobileOutboxSnapshot(f.client).rows[0]?.held).toBe(false);
  expect(snapshot(f.client, now)).toMatchObject({ next: '', items: [{ status: 'editing', held: true, canRetry: false }] });
  await run(f.client, f.native, scheduled.next, now);
  await run(f.client, f.native, JSON.stringify(owner), now, true);
  expect(f.calls).toEqual([]); expect(f.deliveries.size).toBe(0);
});
test('editor-held first row stops that thread order while a different thread can actually deliver', async () => {
  const main = asCreation({ ...queued(), createdAt: '2026-10-08T01:00:00.000Z' });
  const held = asCreation({ ...second(), createdAt: '2026-10-08T00:00:00.000Z' });
  const follower = { ...second(), messageId: 'message-3', commandId: 'command-3', createdAt: '2026-10-08T02:00:00.000Z' };
  const f = fixture(main); f.setExtras([held, follower]);
  expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(held))).not.toBeNull();
  const scheduled = await loaded(f);
  expect(JSON.parse(scheduled.next).owner).toEqual(owner);
  expect(scheduled.items[0]).toMatchObject({ messageId: 'message-2', held: true, status: 'editing' });
  await run(f.client, f.native, scheduled.next, now);
  expect(stages(f)).toEqual(['mobileOutboxDelivery:reserve:command', 'mobileOutboxDelivery:send:command', 'mobileOutboxDelivery:complete:command']);
  expect(snapshot(f.client, now)).toMatchObject({ next: '', count: 2 });
  expect(f.calls.some(call => call.operationId === 'command-3')).toBe(false);
});

const older = (): MobileOutboxRecord => asCreation({ ...second('thread'), createdAt: '2026-10-07T23:00:00.000Z' });
test('manual and stale scheduled admission cannot overtake an earlier persisted editor on the same thread', async () => {
  const f = fixture(asCreation(queued())), scheduled = await loaded(f), prior = older();
  f.setExtras([prior]); await read(f.client, f.native);
  expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(prior))).not.toBeNull(); f.calls.length = 0;
  expect(snapshot(f.client, now).next).toBe('');
  for (const [key, manual] of [[scheduled.next, false], [JSON.stringify(owner), true]] as const)
    expect((await run(f.client, f.native, key, now, manual)).message).toContain('earlier pending message');
  expect(f.calls).toEqual([]); expect(f.disk()?.messageId).toBe('message'); expect(f.deliveries.size).toBe(0);
});
test('ordering admission scopes predecessors by canonical origin and environment, not thread id alone', async () => {
  for (const patch of [{ origin: 'https://different.test' }, { environmentId: 'another-environment' }]) {
    const f = fixture(asCreation(queued())), prior = { ...older(), ...patch }; f.setExtras([prior]);
    expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(prior))).not.toBeNull();
    const scheduled = await loaded(f); expect(JSON.parse(scheduled.next).owner).toEqual(owner);
    await run(f.client, f.native, JSON.stringify(owner), now, true);
    expect(stages(f)).toEqual(['mobileOutboxDelivery:reserve:command', 'mobileOutboxDelivery:send:command', 'mobileOutboxDelivery:complete:command']);
  }
});
test('an editor predecessor adopted during status awaits stops wire admission and removal wakes the waiting task', async () => {
  const f = fixture(asCreation(queued())), scheduled = await loaded(f), prior = older(); let adopted = false;
  f.intercept(async request => {
    if (!adopted && request.op === 'mobileOutboxDelivery' && request.action === 'status') {
      adopted = true; f.setExtras([prior]); await read(f.client, f.native);
      expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(prior))).not.toBeNull();
    }
  });
  await run(f.client, f.native, scheduled.next, now);
  expect(adopted).toBe(true); expect(writes(f)).toEqual([]);
  expect(snapshot(f.client, now).items.find(item => item.messageId === 'message')?.status).toBe('waiting');
  expect(snapshot(f.client, now).next).toBe('');
  f.setExtras([]);
  // Native removal includes its revision tombstone; absence alone is not proof
  // that a previously observed row can be forgotten by the projection.
  const removedNative: Native = { available: true, watch: topic => f.native.watch(topic), async later(request) {
    const reply = await f.native.later(request), inventory = obj(obj(reply).value);
    if (obj(request).op === 'mobileOutbox' && obj(request).action === 'read') {
      obj(inventory.revisions)[prior.messageId] = 2; obj(inventory.tokens)[prior.messageId] = 'epoch:2';
    }
    return reply;
  } };
  await read(f.client, removedNative); f.calls.length = 0;
  const retry = snapshot(f.client, now + 10000); expect(JSON.parse(retry.next).owner).toEqual(owner);
  await run(f.client, f.native, retry.next, now + 10000);
  expect(f.calls.filter(call => call.action === 'send')).toHaveLength(1);
});
test('a later acknowledged row can finish exact local cleanup without overtaking an earlier editor on the wire', async () => {
  const f = fixture(), scheduled = await loaded(f); f.setCleanupFailure(true);
  await run(f.client, f.native, scheduled.next, now); const revision = f.deliveries.get('command')!.revision;
  const prior = older(); f.setExtras([prior]); await read(f.client, f.native);
  expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(prior))).not.toBeNull();
  f.setCleanupFailure(false); f.calls.length = 0;
  expect(snapshot(f.client, now).items.find(item => item.messageId === 'message')?.canRetry).toBe(true);
  await run(f.client, f.native, JSON.stringify(owner), now + 1, true);
  expect(f.calls.find(call => call.action === 'complete')).toMatchObject({ retryCleanupRevision: revision });
  expect(f.calls.some(call => call.op === 'http' || call.op === 'status' || ['send', 'reserve'].includes(String(call.action)))).toBe(false);
  expect(f.disk()).toBeNull(); expect(snapshot(f.client, now)).toMatchObject({ count: 1, next: '' });
});
test('cached ACK admission never permits replacement wire work if its saved native receipt is absent', async () => {
  const f = fixture(), scheduled = await loaded(f); f.setCleanupFailure(true);
  await run(f.client, f.native, scheduled.next, now);
  const prior = older(); f.setExtras([prior]); await read(f.client, f.native);
  expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(prior))).not.toBeNull();
  f.deliveries.clear(); f.calls.length = 0;
  await run(f.client, f.native, JSON.stringify(owner), now + 1, true);
  expect(writes(f)).toEqual([]); expect(f.calls.some(call => call.op === 'http')).toBe(false);
  expect(f.disk()).not.toBeNull();
});

test('an exact editor appearing during receipt inspection blocks further network and wire admission', async () => {
  const original = asCreation(queued()), f = fixture(original), scheduled = await loaded(f); let opened = false;
  f.intercept(request => {
    if (!opened && request.op === 'mobileOutboxDelivery' && request.action === 'status') {
      opened = true; expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(original))).not.toBeNull();
    }
  });
  await run(f.client, f.native, scheduled.next, now);
  expect(opened).toBe(true); expect(writes(f)).toEqual([]);
  expect(f.calls.some(call => call.op === 'http' || call.op === 'status')).toBe(false);
  expect(f.disk()?.messageId).toBe(original.messageId); expect(snapshot(f.client, now).next).toBe('');
});
test('unhydrated or malformed editor ownership adopted during a pass blocks every later unsafe request', async () => {
  for (const invalid of [undefined, { version: 2, markers: {} }, { version: 1, markers: { bad: {} } }]) {
    const f = fixture(), scheduled = await loaded(f); let changed = false;
    f.intercept(request => {
      if (!changed && request.op === 'mobileOutboxDelivery' && request.action === 'status') {
        changed = true; f.client.local = new T3Client().local;
        if (invalid !== undefined) mobilePendingTaskEditorsHydrate(f.client, { mobilePendingTaskEditors: invalid });
      }
    });
    await run(f.client, f.native, scheduled.next, now);
    expect(changed).toBe(true); expect(writes(f)).toEqual([]);
    expect(f.calls.some(call => call.op === 'http' || call.op === 'status')).toBe(false); expect(f.disk()).not.toBeNull();
  }
});
test('opening an editor after auth or final reservation prevents the next delivery admission', async () => {
  for (const after of ['auth', 'reserve']) {
    const original = asCreation(queued()), f = fixture(original), scheduled = await loaded(f); let opened = false;
    f.intercept(request => {
      if (!opened && (after === 'auth' ? request.op === 'http' : request.op === 'mobileOutboxDelivery' && request.action === 'reserve')) {
        opened = true; expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(original))).not.toBeNull();
      }
    });
    await run(f.client, f.native, scheduled.next, now);
    expect(opened).toBe(true); expect(f.calls.some(call => call.action === 'send')).toBe(false);
    if (after === 'auth') expect(f.calls.some(call => call.action === 'confirmQueued')).toBe(false);
    else expect(f.deliveries.get('command')?.state).toBe('reserved');
    expect(f.disk()?.text).toBe('original');
  }
});
test('opening an editor after attachment upload blocks durable row adoption and later send', async () => {
  const original = asCreation({ ...queued(), attachments: [image()] }), f = fixture(original), scheduled = await loaded(f); let opened = false;
  f.intercept(request => {
    if (!opened && request.op === 'uploadAttachment') { opened = true; expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(original))).not.toBeNull(); }
  });
  await run(f.client, f.native, scheduled.next, now);
  expect(opened).toBe(true); expect(f.calls.some(call => call.action === 'mutate' || call.action === 'send')).toBe(false);
  expect(f.disk()?.attachments[0]?.uploadId).toBe(''); expect(f.deliveries.size).toBe(0);
});
test('opening an editor after inline reservation prevents issuing its image command', async () => {
  const original = asCreation({ ...queued(), attachments: [image()] }), f = fixture(original); obj(f.client.config.environment).capabilities = {
    serverResolvedCommandContext: true, inlineMessageContext: true, attachmentUploads: false };
  const scheduled = await loaded(f); let opened = false;
  f.intercept(request => {
    if (!opened && request.op === 'mobileOutboxInline' && request.action === 'reserve') { opened = true; expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(original))).not.toBeNull(); }
  });
  await run(f.client, f.native, scheduled.next, now);
  expect(opened).toBe(true); expect([...f.inlines.values()].map(item => item.state)).toEqual(['reserved']);
  expect(f.calls.some(call => call.action === 'send' || call.action === 'reserveInline')).toBe(false);
});
test('an already admitted final ACK retains exact local cleanup after editor ownership appears', async () => {
  for (const held of [false, true]) {
    const original = asCreation(queued()), f = fixture(original), scheduled = await loaded(f); let opened = false;
    f.intercept(request => {
      if (!opened && request.op === 'mobileOutboxDelivery' && request.action === 'send') {
        opened = true; expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(original))).not.toBeNull(); f.setHeld(held);
      }
    });
    await run(f.client, f.native, scheduled.next, now);
    expect(opened).toBe(true); expect(f.calls.filter(call => call.action === 'send')).toHaveLength(1);
    expect(f.calls.filter(call => call.action === 'complete')).toHaveLength(1);
    expect(f.deliveries.get('command')?.cleanup?.phase).toBe(held ? 'edited' : 'removed');
    expect(f.disk() === null).toBe(!held);
  }
});
test('an unrelated editor arriving mid-pass does not block the original full queue owner', async () => {
  const original = asCreation(queued()), f = fixture(original), scheduled = await loaded(f); let opened = false;
  f.intercept(request => {
    if (!opened && request.op === 'mobileOutboxDelivery' && request.action === 'status') {
      opened = true; expect(mobilePendingTaskEditorsCreate(f.client, editorMarker(asCreation(second())))).not.toBeNull();
    }
  });
  await run(f.client, f.native, scheduled.next, now);
  expect(f.calls.filter(call => call.action === 'send')).toHaveLength(1); expect(f.disk()).toBeNull();
});
