// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileOutboxSnapshot } from './mobile-outbox';
import { mobileOutboxDeliverOne } from './mobile-outbox-foreground';
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
  let cleanupFails = false;
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
      if (r.action === 'read') value = { ownerEpoch: 'epoch', sequenceFloor: floor, complete, errors: [], records: record ? [row()] : [],
        revisions: { message: revision }, tokens: { message: token }, outcomes, mutations: [], transfers: [] };
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
      } else if (r.action === 'send') { operation = settled(operation!); deliveries.set(operation.operationId, operation); value = wrapper(operation); }
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
    disk: () => record, setHeld(value = true) { held = value; }, setComplete(value: boolean) { complete = value; },
    setSession(value: Obj) { session = value; }, setMutation(value: string) { mutationMode = value; }, setCleanupFailure(value: boolean) { cleanupFails = value; },
    intercept(value: typeof intercept) { intercept = value; }, edit() { record = { ...record!, text: 'newer' }; revision++; token = `epoch:${++floor}`; },
    restart() { client = makeClient(); cold = true; calls.length = 0; },
  };
}
const stages = (f: ReturnType<typeof fixture>) => f.calls.filter(r => ['reserve', 'send', 'complete', 'reserveInline'].includes(String(r.action)))
  .map(r => `${r.op}:${r.action}:${r.operationId ?? obj(r.payload).commandId ?? r.inlineOperationId}`);
const writes = (f: ReturnType<typeof fixture>) => f.calls.filter(r => ['reserve', 'send', 'reserveInline', 'mutate'].includes(String(r.action)) || r.op === 'uploadAttachment');

test('foreground pass discovers every identity before new work and delivers once', async () => {
  const f = fixture(); expect((await f.run()).status).toBe('delivered');
  const discovered = f.calls.filter(r => r.op === 'mobileOutboxDelivery' && r.action === 'status').map(r => r.operationId);
  expect(discovered).toEqual(['command', 'command:runtime-mode', 'command:interaction-mode']);
  const lookup = f.calls.findIndex(r => r.op === 'mobileOutboxInline' && r.action === 'lookup');
  expect(lookup).toBeGreaterThan(-1); expect(f.calls.indexOf(writes(f)[0]!)).toBeGreaterThan(lookup);
  expect(stages(f)).toEqual(['mobileOutboxDelivery:reserve:command', 'mobileOutboxDelivery:send:command', 'mobileOutboxDelivery:complete:command']);
  expect(mobileOutboxSnapshot(f.client).rows).toEqual([]); expect(f.client.local.pending).toEqual({});
});
test('runtime and interaction settings ACK before any attachment upload, then use adopted capture', async () => {
  const f = fixture({ ...queued(), runtimeMode: 'approval-required', interactionMode: 'plan', attachments: [image()] });
  expect((await f.run()).status).toBe('delivered');
  const sendIds = f.calls.filter(r => r.action === 'send').map(r => r.operationId);
  expect(sendIds).toEqual(['command:runtime-mode', 'command:interaction-mode', 'command']);
  const upload = f.calls.findIndex(r => r.method === 'attachments.createUploadUrl');
  expect(upload).toBeGreaterThan(f.calls.findIndex(r => r.action === 'send' && r.operationId === 'command:interaction-mode'));
  const final = f.calls.find(r => r.action === 'reserve' && obj(r.payload).commandId === 'command')!;
  expect(final.expectedRevision).toBe(2); expect(final.expectedToken).toBe(f.calls.find(r => r.action === 'mutate')!.mutationId);
  expect((obj(final.payload).attachments as Obj[])[0]!.id).toBe('upload-1');
});
test('creation embeds settings and allocates branch only for a new worktree command', async () => {
  for (const workspaceMode of ['local', 'worktree'] as const) {
    const f = fixture({ ...queued(), runtimeMode: 'approval-required', interactionMode: 'plan', creation: {
      projectId: 'project', projectCwd: '/repo', workspaceMode, branch: 'main', worktreePath: null } });
    expect((await f.run()).status).toBe('delivered');
    expect(f.calls.filter(r => r.action === 'send').map(r => r.operationId)).toEqual(['command']);
    const final = f.deliveries.get('command')!; expect(final.method).toBe('orchestration.launchThread');
    expect(final.payload.runtimeMode).toBe('approval-required'); expect(final.payload.interactionMode).toBe('plan');
    expect(f.calls.filter(r => r.op === 'ids')).toHaveLength(workspaceMode === 'worktree' ? 1 : 0);
  }
});
test('legacy inline images stay native and persist before linked final delivery', async () => {
  const f = fixture({ ...queued(), attachments: [image(), image('2')] });
  obj(obj(f.client.config.environment).capabilities).attachmentUploads = false;
  expect((await f.run()).status).toBe('delivered');
  expect(f.calls.some(r => r.op === 'snapshotDraftRead' || r.op === 'uploadAttachment')).toBe(false);
  expect(JSON.stringify(f.calls)).not.toContain('dataUrl');
  const final = f.deliveries.get('command')!;
  expect(final.inlineSource?.operationId).toBe([...f.inlines.keys()][0]);
  expect((final.payload.attachments as Obj[]).map(file => file.id)).toEqual(['asset-0', 'asset-1']);
  expect(f.calls.findIndex(r => r.op === 'mobileOutboxDelivery' && r.action === 'reserveInline'))
    .toBeGreaterThan(f.calls.findIndex(r => r.op === 'mobileOutboxInline' && r.action === 'send'));
});
test('durable final ACK completes offline without settings, upload, IDs or authentication', async () => {
  const f = fixture({ ...queued(), attachments: [image()] }); f.deliveries.set('command', settled(saved(f.disk()!)));
  f.client.connection = 'disconnected'; f.client.config = {}; f.client.configLive = false;
  expect((await f.run()).status).toBe('delivered'); expect(writes(f)).toEqual([]);
  expect(f.calls.some(r => ['http', 'status', 'ids', 'snapshotDraftRead'].includes(String(r.op)))).toBe(false);
  expect(stages(f)).toEqual(['mobileOutboxDelivery:complete:command']);
});
test('edited final ACK preserves newer content and never resends it', async () => {
  const f = fixture(); f.deliveries.set('command', settled(saved())); f.edit();
  expect((await f.run()).status).toBe('edited-after-ack'); expect(f.disk()?.text).toBe('newer');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.record.text).toBe('newer'); expect(writes(f)).toEqual([]);
});
test('cold restart requires explicit recovery and retains exact final payload', async () => {
  const f = fixture(); const receipt = settled(saved(), 'uncertain'); f.deliveries.set('command', receipt); f.edit(); f.restart();
  expect((await f.run()).status).toBe('recovery-required'); expect(writes(f)).toEqual([]);
  f.calls.length = 0; expect((await f.run({ recover: true })).status).toBe('edited-after-ack');
  expect(f.calls.some(r => r.action === 'recover' && r.operationId === 'command')).toBe(true);
  expect(f.calls.filter(r => r.action === 'send')).toEqual([expect.objectContaining({ op: 'mobileOutboxDelivery', operationId: 'command', revision: 3 })]);
  expect(f.deliveries.get('command')!.payload.text).toBe('original'); expect(f.calls.some(r => r.action === 'reserve')).toBe(false);
});
test('saved issued final is retried by identity without rebuilding current settings', async () => {
  const f = fixture(); const receipt = settled(saved(), 'uncertain'); receipt.state = 'issued'; receipt.revision = receipt.attemptRevision!; delete receipt.error;
  f.deliveries.set('command', receipt); f.client.config = {}; f.client.configLive = false;
  const result = await f.run(); expect(result.status).toBe('delivered');
  expect(f.calls.filter(r => r.action === 'send')[0]).toMatchObject({ operationId: 'command', revision: 2 });
  expect(f.calls.some(r => r.action === 'reserve' || r.op === 'ids')).toBe(false);
});
test('lost final send reply recovers saved ACK after restart without a second send', async () => {
  const f = fixture(); f.intercept(r => { if (r.op === 'mobileOutboxDelivery' && r.action === 'send') throw Error('Reply lost after native ACK'); });
  expect((await f.run()).status).not.toBe('delivered'); expect(f.deliveries.get('command')?.state).toBe('acknowledged');
  f.intercept(undefined); f.restart(); expect((await f.run({ recover: true })).status).toBe('delivered');
  expect(writes(f)).toEqual([]); expect(f.calls.some(r => r.op === 'ids')).toBe(false);
});
test('lost inline reservation reply is discovered after restart without new bytes or UUID', async () => {
  const f = fixture({ ...queued(), attachments: [image()] }); obj(obj(f.client.config.environment).capabilities).attachmentUploads = false;
  f.intercept(r => { if (r.op === 'mobileOutboxInline' && r.action === 'reserve') throw Error('Reply lost after reservation'); });
  expect((await f.run()).status).not.toBe('delivered'); expect(f.inlines.size).toBe(1);
  const id = [...f.inlines.keys()][0]; f.intercept(undefined); f.restart();
  expect((await f.run()).status).toBe('recovery-required'); expect(writes(f)).toEqual([]); f.calls.length = 0;
  expect((await f.run({ recover: true })).status).toBe('delivered');
  expect([...f.inlines.keys()]).toEqual([id]); expect(f.calls.some(r => r.op === 'ids' || r.op === 'snapshotDraftRead' || r.action === 'reserve')).toBe(false);
});
test('discovery denial and malformed inventory are never treated as absent receipts', async () => {
  for (const mode of ['busy', 'denied', 'incomplete'] as const) {
    const f = fixture();
    if (mode === 'incomplete') f.setComplete(false);
    else f.failures.set('mobileOutboxInline:lookup:', new ClientError('Cannot inspect receipts', mode));
    expect((await f.run()).status).not.toBe('delivered'); expect(writes(f)).toEqual([]);
  }
});
test('native authorization overrides optimistic cached scopes before any new write', async () => {
  const f = fixture({ ...queued(), attachments: [image()] }); f.setSession({ authenticated: true, permissions: [], scopes: ['orchestration:operate'] });
  expect((await f.run()).status).not.toBe('delivered'); expect(writes(f)).toEqual([]);
});
test('hold or endpoint replacement after authentication prevents reservation and upload', async () => {
  for (const mode of ['hold', 'endpoint'] as const) {
    const f = fixture({ ...queued(), attachments: [image()] });
    f.intercept(r => { if (r.op === 'http') { if (mode === 'hold') f.setHeld(); else f.client.generation++; } });
    expect((await f.run()).status).not.toBe('delivered'); expect(writes(f)).toEqual([]);
  }
});
test('uncertain upload adoption stops before final or inline reservation', async () => {
  const f = fixture({ ...queued(), attachments: [image()] }); f.setMutation('uncertain');
  expect((await f.run()).status).not.toBe('delivered');
  expect(f.calls.some(r => r.action === 'reserve' || r.action === 'send')).toBe(false);
  expect(f.calls.some(r => r.method === 'attachments.delete')).toBe(false);
});
test('lost settings ACK resumes that stage before interaction and uploads', async () => {
  const f = fixture({ ...queued(), runtimeMode: 'approval-required', interactionMode: 'plan', attachments: [image()] });
  f.intercept(r => { if (r.action === 'send' && r.operationId === 'command:runtime-mode') throw Error('Settings ACK reply lost'); });
  expect((await f.run()).status).not.toBe('delivered');
  expect(f.calls.some(r => r.method === 'attachments.createUploadUrl')).toBe(false);
  f.intercept(undefined); f.restart(); expect((await f.run({ recover: true })).status).toBe('delivered');
  expect(f.calls.filter(r => r.action === 'send').map(r => r.operationId)).toEqual(['command:interaction-mode', 'command']);
  expect(f.calls.filter(r => r.action === 'reserve').map(r => obj(r.payload).commandId)).toEqual(['command:interaction-mode', 'command']);
});
test('an earlier settings ACK cannot authorize a different payload under the same command ID', async () => {
  const record = { ...queued(), runtimeMode: 'approval-required' as const }, f = fixture(record), prior = settled(saved(record));
  prior.operationId = 'command:runtime-mode'; prior.stage = 'settings-sync';
  prior.payload = { type: 'thread.runtime-mode.set', commandId: prior.operationId, threadId: 'thread', runtimeMode: 'auto' };
  f.deliveries.set(prior.operationId, prior);
  expect((await f.run()).status).toBe('recovery-required'); expect(writes(f)).toEqual([]);
  expect(f.calls.some(r => r.op === 'snapshotDraftRead')).toBe(false);
});
test('lost cleanup reply replays saved removal after restart without any network work', async () => {
  const f = fixture(); f.intercept(r => { if (r.op === 'mobileOutboxDelivery' && r.action === 'complete') throw Error('Removal reply lost'); });
  expect((await f.run()).status).not.toBe('delivered'); expect(f.disk()).toBeNull();
  expect(f.deliveries.get('command')?.cleanup?.phase).toBe('removed');
  f.intercept(undefined); f.restart(); f.client.connection = 'disconnected';
  expect((await f.run({ recover: true })).status).toBe('delivered'); expect(writes(f)).toEqual([]);
  expect(f.calls.some(r => ['http', 'status', 'ids'].includes(String(r.op)))).toBe(false);
});
test('non-server-resolved follow-up fetches the owner projection and steers its active run', async () => {
  const f = fixture({ ...queued(), dispatchMode: 'auto' });
  obj(obj(f.client.config.environment).capabilities).serverResolvedCommandContext = false;
  f.client.thread = { projection: { thread: { id: 'unrelated-thread' } } } as T3Client['thread'];
  expect((await f.run()).status).toBe('delivered');
  expect(f.calls.filter(r => r.method === 'orchestration.getThreadProjection')).toEqual([
    expect.objectContaining({ payload: { threadId: 'thread' } }),
  ]);
  expect(f.deliveries.get('command')?.payload.dispatchMode).toEqual({ type: 'steer_active', targetRunId: 'run' });
});
test('saved settings ACK or uncertain attempt settles even when shell already reflects its desired value', async () => {
  for (const state of ['acknowledged', 'uncertain']) {
    const record = { ...queued(), runtimeMode: 'full-access' as const }, f = fixture(record), prior = settled(saved(record), state);
    prior.operationId = 'command:runtime-mode'; prior.stage = 'settings-sync';
    prior.payload = { type: 'thread.runtime-mode.set', commandId: prior.operationId, threadId: 'thread', runtimeMode: 'full-access' };
    f.deliveries.set(prior.operationId, prior);
    expect((await f.run()).status).toBe('delivered');
    expect(f.calls.filter(r => r.action === 'send').map(r => r.operationId)).toEqual(state === 'uncertain' ? ['command:runtime-mode', 'command'] : ['command']);
    expect(f.calls.filter(r => r.action === 'reserve').map(r => obj(r.payload).commandId)).toEqual(['command']);
    expect(f.calls.some(r => r.action === 'complete' && r.operationId === prior.operationId)).toBe(true);
  }
});
test('cold retired final requires recovery before exact revision rearm', async () => {
  const f = fixture(), prior = { ...saved(), state: 'retired' as const, revision: 2 }; f.deliveries.set('command', prior); f.restart();
  expect((await f.run()).status).toBe('recovery-required'); expect(writes(f)).toEqual([]);
  f.calls.length = 0; expect((await f.run({ recover: true })).status).toBe('delivered');
  const recovered = f.calls.findIndex(r => r.action === 'recover' && r.operationId === 'command');
  const rearmed = f.calls.findIndex(r => r.action === 'reserve'); expect(recovered).toBeGreaterThan(-1); expect(rearmed).toBeGreaterThan(recovered);
  expect(f.calls[rearmed]).toMatchObject({ expectedRetiredRevision: 2 });
  expect(f.deliveries.get('command')?.retiredRevision).toBe(2);
});
test('creation waits for live shell and requires live or captured project path', async () => {
  for (const mode of ['stale-shell', 'missing-path'] as const) {
    const f = fixture({ ...queued(), creation: { projectId: 'project', workspaceMode: 'local', branch: 'main', worktreePath: null } });
    if (mode === 'stale-shell') f.client.shellLive = false;
    else f.client.shell.projects = [];
    expect((await f.run()).status).toBe(mode === 'stale-shell' ? 'waiting' : 'recovery-required');
    expect(writes(f)).toEqual([]); expect(f.calls.some(r => r.op === 'ids')).toBe(false);
  }
});
test('saved settings outcome settles before current file capability requests draft recovery', async () => {
  const attachment: MobileOutboxAttachment = { ...image(), kind: 'file', contextId: 'file', source: 'file', name: 'notes.txt', mimeType: 'text/plain' };
  const record = { ...queued(), runtimeMode: 'full-access' as const, attachments: [attachment] }, f = fixture(record), prior = settled(saved(record), 'uncertain');
  prior.operationId = 'command:runtime-mode'; prior.stage = 'settings-sync';
  prior.payload = { type: 'thread.runtime-mode.set', commandId: prior.operationId, threadId: 'thread', runtimeMode: 'full-access' };
  f.deliveries.set(prior.operationId, prior); delete obj(obj(f.client.config.environment).capabilities).fileAttachments;
  expect((await f.run()).status).toBe('recovery-required');
  expect(f.calls.filter(r => r.action === 'send').map(r => r.operationId)).toEqual(['command:runtime-mode']);
  expect(f.calls.some(r => r.action === 'complete' && r.operationId === prior.operationId)).toBe(true);
  expect(f.calls.some(r => r.action === 'reserve' || r.op === 'snapshotDraftRead' || r.method === 'attachments.createUploadUrl')).toBe(false);
});
test('never-issued final refuses config or thread model changes during actual auth', async () => {
  for (const mode of ['config', 'model'] as const) {
    const record = queued(); if (mode === 'model') delete record.modelSelection;
    const f = fixture(record), receipt = saved(record); receipt.payload.modelSelection = { instanceId: 'p', model: 'm' };
    f.deliveries.set('command', receipt);
    f.intercept(r => { if (r.op === 'http') {
      if (mode === 'config') f.client.config = { ...f.client.config, changed: true };
      else f.client.shell.threads[0]!.modelSelection = { instanceId: 'other', model: 'other' };
    } });
    expect((await f.run()).status).not.toBe('delivered'); expect(writes(f)).toEqual([]);
  }
});
test('saved reserved inline and inline ACK refuse config changes before issue or final binding', async () => {
  for (const state of ['reserved', 'acknowledged'] as const) {
    const f = fixture({ ...queued(), attachments: [image()] }); obj(obj(f.client.config.environment).capabilities).attachmentUploads = false;
    f.intercept(r => { if (r.op === 'mobileOutboxInline' && r.action === (state === 'reserved' ? 'reserve' : 'send')) throw Error('Saved reply lost'); });
    expect((await f.run()).status).not.toBe('delivered'); expect([...f.inlines.values()][0]?.state).toBe(state);
    f.calls.length = 0;
    f.intercept(r => { if (r.op === 'http') f.client.config = { ...f.client.config, changed: true }; });
    expect((await f.run()).status).not.toBe('delivered'); expect(writes(f)).toEqual([]); expect(f.deliveries.size).toBe(0);
  }
});
test('exact failed cleanup retry acknowledges its prior outcome then removes without network', async () => {
  const f = fixture(); f.setCleanupFailure(true);
  expect((await f.run()).status).toBe('cleanup-pending'); const receipt = f.deliveries.get('command')!;
  expect(receipt.cleanup?.phase).toBe('failed'); expect(f.disk()?.text).toBe('original');
  f.setCleanupFailure(false); f.calls.length = 0; f.client.connection = 'disconnected';
  expect((await f.run({ retryCleanupRevision: receipt.revision })).status).toBe('delivered');
  const acknowledgement = f.calls.findIndex(r => r.op === 'mobileOutbox' && r.action === 'acknowledge');
  const completion = f.calls.findIndex(r => r.action === 'complete');
  expect(acknowledgement).toBeGreaterThan(-1); expect(completion).toBeGreaterThan(acknowledgement);
  expect(f.calls[acknowledgement]!.mutationId).toBe(receipt.cleanup!.mutationId);
  expect(f.calls[completion]!.retryCleanupRevision).toBe(receipt.revision); expect(writes(f)).toEqual([]);
  expect(f.calls.some(r => ['http', 'status', 'ids'].includes(String(r.op)))).toBe(false);
});
test('cleanup retry refuses absent or non-ACK receipts without starting new delivery', async () => {
  for (const hasReceipt of [false, true]) {
    const f = fixture(); if (hasReceipt) f.deliveries.set('command', saved());
    expect((await f.run({ retryCleanupRevision: 1 })).status).toBe('cleanup-pending'); expect(writes(f)).toEqual([]);
  }
});
test('let-go after native send makes no follow-on call and propagates lifetime cancellation', async () => {
  const f = fixture(); f.intercept(r => { if (r.op === 'mobileOutboxDelivery' && r.action === 'send') throw { name: 'FetchError', kind: 'Aborted' }; });
  let error: unknown; try { await f.run(); } catch (caught) { error = caught; }
  expect(letGo(error)).toBe(true); expect(f.calls.at(-1)).toMatchObject({ op: 'mobileOutboxDelivery', action: 'send' });
  expect(f.deliveries.get('command')?.state).toBe('acknowledged'); expect(f.disk()?.text).toBe('original');
});
