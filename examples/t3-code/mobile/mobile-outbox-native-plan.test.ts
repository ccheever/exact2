// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { expect, test } from 'bun:test';
import { obj, type Obj } from './shared/domain';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxLaunchNativePlan, mobileOutboxMessageNativePlan, mobileOutboxLaunchPlan, mobileOutboxMessagePlan,
  mobileOutboxLaunchRequest, mobileOutboxMessageRequest, type MobileOutboxNativeWireFacts, type MobileOutboxWireFacts,
  type MobileOutboxThreadFacts } from './mobile-outbox-wire';
import { mobileOutboxCompactInline, mobileOutboxMaterializeInline } from './mobile-outbox-inline';
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function fixture(launch = false, inlineContext = true) {
  const record: MobileOutboxRecord = { schemaVersion: 1, origin: 'https://home.test', environmentId: 'env', threadId: 'thread',
    messageId: 'message', commandId: 'command', text: '  [Photo](t3-context://v1/image/photo)  ', createdAt: '2026-10-08T00:00:00.000Z',
    modelSelection: { instanceId: 'p', model: 'm' }, attachments: [
      { kind: 'image', id: '11111111-1111-4111-a111-111111111111', name: 'first.png', mimeType: 'image/png', sizeBytes: 1, status: 'staged', uploadId: '' },
      { kind: 'file', id: '22222222-2222-4222-a222-222222222222', name: 'file.txt', mimeType: 'text/plain', sizeBytes: 2, status: 'ready', uploadId: 'remote-file', uploadEnvironmentId: 'env', contextId: 'file', source: 'file' },
      { kind: 'image', id: '33333333-3333-4333-a333-333333333333', name: 'last.png', mimeType: 'image/png', sizeBytes: 2, status: 'staged', uploadId: '' }],
    context: { version: 1, records: [{ version: 1, kind: 'image', contextId: 'photo', label: 'Photo', attachmentId: '11111111-1111-4111-a111-111111111111' }] } };
  if (launch) record.creation = { projectId: 'project', projectCwd: '/repo', workspaceMode: 'worktree', branch: 'main', worktreePath: null, startFromOrigin: true };
  const facts: MobileOutboxNativeWireFacts = { origin: record.origin, environmentId: 'env', config: { providers: [], environment: {
    environmentId: 'env', capabilities: { attachmentUploads: false, inlineMessageContext: inlineContext, serverResolvedCommandContext: true } } },
    attachments: record.attachments.map(local => local.kind === 'image'
      ? { localId: local.id, kind: 'inline-image-metadata', attachment: { type: 'image', name: local.name, mimeType: local.mimeType, sizeBytes: local.sizeBytes } }
      : { localId: local.id, kind: 'reference', attachment: { type: 'file', id: local.uploadId, name: local.name, mimeType: local.mimeType, sizeBytes: local.sizeBytes } }) };
  const thread: MobileOutboxThreadFacts = { origin: record.origin, environmentId: 'env', threadId: 'thread', modelSelection: record.modelSelection!, runtimeMode: 'full-access', interactionMode: 'default' };
  const run = () => launch ? mobileOutboxLaunchNativePlan(record, facts, 't3/frozen') : mobileOutboxMessageNativePlan(record, facts, thread);
  return { record, facts, thread, run };
}
test('metadata plans equal existing source-byte compaction while containing no bytes or fabricated placeholders', () => {
  for (const launch of [false, true]) for (const inlineContext of [false, true]) {
    const f = fixture(launch, inlineContext), before = copy(f.facts), result = f.run();
    const byteFacts: MobileOutboxWireFacts = { ...f.facts, attachments: f.facts.attachments.map((entry, index) => entry.kind === 'reference' ? entry :
      { localId: entry.localId, kind: 'inline-image', attachment: { ...entry.attachment, dataUrl: `data:image/png;base64,${index === 0 ? 'YQ==' : 'YmM='}` } }) };
    const bytePlan = launch ? mobileOutboxLaunchPlan(f.record, byteFacts, 't3/frozen') : mobileOutboxMessagePlan(f.record, byteFacts, f.thread);
    expect(result.status).toBe('needs-inline-reservation'); expect(bytePlan.status).toBe('needs-inline-persistence');
    if (result.status !== 'needs-inline-reservation' || bytePlan.status !== 'needs-inline-persistence') throw Error('fixture');
    const compact = mobileOutboxCompactInline(f.record, bytePlan.value); expect(compact.status).toBe('ready');
    if (compact.status !== 'ready') throw Error('fixture'); expect(result.value).toEqual(compact.value);
    expect(result.value.inline).toEqual([{ index: 0, localId: f.record.attachments[0]!.id }, { index: 2, localId: f.record.attachments[2]!.id }]);
    expect(JSON.stringify(result)).not.toContain('dataUrl'); expect(JSON.stringify(result)).not.toContain('base64'); expect(f.facts).toEqual(before);
    const content = launch ? obj(result.value.commandTemplate.payload.initialMessage) : result.value.commandTemplate.payload;
    if (inlineContext) expect((obj(content.context).records as Obj[])[0]!.attachmentId).toBe(f.record.attachments[0]!.id);
    else expect(content.context).toBeUndefined();
  }
});
test('native metadata rejects byte inputs, extra descriptor fields, owner and ordering drift', () => {
  for (const mode of ['bytes', 'dataUrl', 'id', 'source', 'extra', 'name', 'mime', 'size', 'order', 'owner', 'capability', 'file'] as const) {
    const f = fixture(), first = f.facts.attachments[0]! as unknown as Obj, descriptor = obj(first.attachment);
    if (mode === 'bytes') { first.kind = 'inline-image'; descriptor.dataUrl = 'data:image/png;base64,YQ=='; }
    if (['dataUrl', 'id', 'source', 'extra'].includes(mode)) descriptor[mode] = mode === 'source' ? {} : 'unexpected';
    if (mode === 'name') descriptor.name = 'other.png';
    if (mode === 'mime') descriptor.mimeType = 'image/jpeg';
    if (mode === 'size') descriptor.sizeBytes = 999;
    if (mode === 'order') f.facts.attachments = [...f.facts.attachments].reverse();
    if (mode === 'owner') f.facts.environmentId = 'other';
    if (mode === 'capability') obj(obj(f.facts.config.environment).capabilities).attachmentUploads = true;
    if (mode === 'file') f.record.attachments[0] = { ...f.record.attachments[0]!, kind: 'file', contextId: 'file', source: 'file' };
    expect(f.run().status).toBe('blocked');
  }
});
test('ordinary requests and source-byte plans refuse metadata even when passed through untyped callers', () => {
  for (const launch of [false, true]) {
    const f = fixture(launch), incompatible = f.facts as unknown as MobileOutboxWireFacts;
    expect((launch ? mobileOutboxLaunchRequest(f.record, incompatible, 'branch') : mobileOutboxMessageRequest(f.record, incompatible, f.thread)).status).toBe('blocked');
    expect((launch ? mobileOutboxLaunchPlan(f.record, incompatible, 'branch') : mobileOutboxMessagePlan(f.record, incompatible, f.thread)).status).toBe('blocked');
  }
});
test('pure modern native planning produces the unchanged reference-only request', () => {
  const f = fixture(); f.record.attachments = [f.record.attachments[1]!]; f.facts.attachments = [f.facts.attachments[1]!];
  expect(f.run()).toEqual(mobileOutboxMessageRequest(f.record, f.facts as MobileOutboxWireFacts, f.thread)); expect(f.run().status).toBe('ready');
});
test('projection and workspace requirements remain source-owned and final ACK preserves mixed slot order', () => {
  const f = fixture(); f.record.dispatchMode = 'auto'; obj(obj(f.facts.config.environment).capabilities).serverResolvedCommandContext = false;
  expect(f.run().status).toBe('needs-projection');
  const projection = { thread: { id: 'thread' }, runs: [{ id: 'run', status: 'running', providerThreadId: 'provider-thread' }],
    providerThreads: [{ id: 'provider-thread', providerSessionId: 'session' }], providerSessions: [{ id: 'session', capabilities: { turns: { supportsActiveSteering: true } } }], messages: [] };
  const result = mobileOutboxMessageNativePlan(f.record, f.facts, f.thread, projection);
  expect(result.status).toBe('needs-inline-reservation'); if (result.status !== 'needs-inline-reservation') throw Error('fixture');
  expect(result.value.commandTemplate.payload.dispatchMode).toEqual({ type: 'steer_active', targetRunId: 'run' });
  const response = { attachments: [0, 2].map(index => { const local = f.record.attachments[index]!;
    return { type: 'image', id: `asset-${index}`, name: local.name, mimeType: local.mimeType, sizeBytes: local.sizeBytes }; }) };
  const materialized = mobileOutboxMaterializeInline(result.value, response); expect(materialized.status).toBe('ready');
  if (materialized.status !== 'ready') throw Error('fixture'); expect((materialized.value.payload.attachments as Obj[]).map(value => value.id)).toEqual(['asset-0', 'remote-file', 'asset-2']);
  const launch = fixture(true); expect(mobileOutboxLaunchNativePlan(launch.record, launch.facts).status).toBe('blocked');
});
