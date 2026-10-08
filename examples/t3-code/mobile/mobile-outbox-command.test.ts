import { expect, test } from 'bun:test';
import { obj, arr } from './shared/domain';
import { mobileOutboxLaunchPlan, mobileOutboxLaunchRequest, mobileOutboxMessagePlan, mobileOutboxMessageRequest,
  type MobileOutboxWireFacts, type MobileOutboxThreadFacts } from './mobile-outbox-wire';
import { mobileOutboxMaterializeCommand, type MobileOutboxInlinePersistence } from './mobile-outbox-command';
import type { MobileOutboxRecord } from './mobile-outbox-model';
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function fixture() {
  const record: MobileOutboxRecord = { schemaVersion: 1, origin: 'https://server.test', environmentId: 'env',
    threadId: 'thread', messageId: 'message', commandId: 'command', text: '  [photo](t3-context://v1/image/photo)  ',
    dispatchMode: 'auto', modelSelection: { instanceId: 'p', model: 'm', options: [] }, createdAt: '2026-10-08T00:00:00.000Z',
    attachments: [{ kind: 'image', id: 'local-photo', name: 'photo.png', mimeType: 'image/png', sizeBytes: 1, status: 'ready' },
      { kind: 'file', id: 'local-file', name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 2, status: 'ready',
        uploadId: 'stored-file', uploadEnvironmentId: 'env', contextId: 'file', source: 'attached' },
      { kind: 'image', id: 'local-other', name: 'other.png', mimeType: 'image/png', sizeBytes: 1, status: 'ready' }],
    context: { version: 1, records: [{ version: 1, kind: 'image', contextId: 'photo', label: 'photo', attachmentId: 'local-photo' },
      { version: 1, kind: 'file', contextId: 'file', label: 'notes', attachmentId: 'local-file' }] } };
  const facts: MobileOutboxWireFacts = { origin: record.origin, environmentId: record.environmentId,
    config: { environment: { environmentId: 'env', capabilities: { attachmentUploads: false, inlineMessageContext: true,
      serverResolvedCommandContext: true } }, providers: [] }, attachments: record.attachments.map(local => ({ localId: local.id,
      kind: local.kind === 'image' ? 'inline-image' : 'reference', attachment: { type: local.kind, name: local.name,
        mimeType: local.mimeType, sizeBytes: local.sizeBytes, ...(local.kind === 'image' ? { dataUrl: 'data:image/png;base64,YQ==' } : { id: local.uploadId }) } })) };
  const thread: MobileOutboxThreadFacts = { origin: record.origin, environmentId: 'env', threadId: 'thread',
    modelSelection: record.modelSelection!, runtimeMode: 'full-access', interactionMode: 'default' };
  const response = { attachments: ['photo', 'other'].map(name => ({ type: 'image', id: `persisted-${name}`, name: `${name}.png`, mimeType: 'image/png', sizeBytes: 1 })) };
  return { record, facts, thread, response };
}
function inlinePlan(): { plan: MobileOutboxInlinePersistence; fixture: ReturnType<typeof fixture> } {
  const value = fixture(), result = mobileOutboxMessagePlan(value.record, value.facts, value.thread);
  if (result.status !== 'needs-inline-persistence') throw new Error(`Expected persistence, got ${result.status}`);
  return { plan: result.value, fixture: value };
}
test('persist only inline slots under captured IDs, restore mixed order and source idless context', () => {
  const { plan, fixture: value } = inlinePlan(), before = JSON.stringify({ plan, value });
  expect(plan.request).toEqual({ method: 'assets.persistChatAttachments', payload: { threadId: 'thread', messageId: 'message',
    attachments: [value.facts.attachments[0]!.attachment, value.facts.attachments[2]!.attachment] } });
  const result = mobileOutboxMaterializeCommand(plan, value.response);
  expect(result.status).toBe('ready'); if (result.status !== 'ready') return;
  expect(result.value.payload.attachments).toEqual([value.response.attachments[0], value.facts.attachments[1]!.attachment, value.response.attachments[1]]);
  expect(arr(obj(result.value.payload.context).records).map(entry => entry.attachmentId)).toEqual(['local-photo', 'stored-file']);
  expect(result.value.payload.commandId).toBe('command'); expect(result.value.payload.messageId).toBe('message');
  expect(JSON.stringify({ plan, value })).toBe(before);
  obj(arr(result.value.payload.attachments)[0]).name = 'edited';
  expect(value.response.attachments[0]!.name).toBe('photo.png');
});
test('launch freezes worktree and title before persistence', () => {
  const { record, facts, response } = fixture();
  record.creation = { projectId: 'project', projectCwd: '/repo', workspaceMode: 'worktree', branch: 'main', worktreePath: null, startFromOrigin: true };
  const plan = mobileOutboxLaunchPlan(record, facts, 't3/frozen');
  expect(plan.status).toBe('needs-inline-persistence'); if (plan.status !== 'needs-inline-persistence') return;
  const result = mobileOutboxMaterializeCommand(plan.value, response);
  expect(result.status).toBe('ready'); if (result.status !== 'ready') return;
  expect(result.value.method).toBe('orchestration.launchThread');
  expect(result.value.payload.workspaceStrategy).toEqual({ type: 'worktree', baseRef: 'main', branch: 't3/frozen', startFromOrigin: true });
  expect(result.value.payload.title).toBe('[photo](t3-context://v1/image/photo)');
  expect(obj(result.value.payload.initialMessage).text).toBe(record.text.trim());
  expect(obj(result.value.payload.initialMessage).messageId).toBe(record.messageId);
  expect(mobileOutboxLaunchRequest(record, facts, 't3/frozen').status).toBe('blocked');
});
test('legacy context stays serialized and ordinary request API never exposes inline bytes', () => {
  const { record, facts, thread, response } = fixture();
  obj(obj(facts.config.environment).capabilities).inlineMessageContext = false;
  const plan = mobileOutboxMessagePlan(record, facts, thread);
  expect(plan.status).toBe('needs-inline-persistence'); if (plan.status !== 'needs-inline-persistence') return;
  const result = mobileOutboxMaterializeCommand(plan.value, response);
  expect(result.status).toBe('ready'); if (result.status !== 'ready') return;
  expect(result.value.payload.text).toBe('  photo'); expect(result.value.payload.context).toBeUndefined();
  expect(result.value.payload.titleSeed).toBe('[photo](t3-context://v1/image/photo)');
  expect(mobileOutboxMessageRequest(record, facts, thread).status).toBe('blocked');
});
test('modern reference-only request is unchanged by planning and detached', () => {
  const { record, facts, thread } = fixture(); record.attachments = record.attachments.slice(1, 2); facts.attachments = facts.attachments.slice(1, 2);
  const old = mobileOutboxMessageRequest(record, facts, thread), plan = mobileOutboxMessagePlan(record, facts, thread);
  expect(plan).toEqual(old); if (plan.status !== 'ready') throw new Error('reference plan blocked');
  arr(plan.value.payload.attachments)[0]!.name = 'changed'; expect(facts.attachments[0]!.attachment.name).toBe('notes.txt');
});
test('inline capability, kind, ownership, order and metadata remain admission checks', () => {
  const initial = fixture();
  const cases = [
    (v: typeof initial) => { obj(obj(v.facts.config.environment).capabilities).attachmentUploads = true; },
    (v: typeof initial) => { v.facts.origin = 'https://other.test'; },
    (v: typeof initial) => { v.facts.attachments = [...v.facts.attachments].reverse(); },
    (v: typeof initial) => { v.facts.attachments[0]!.attachment.id = 'not-idless'; },
    (v: typeof initial) => { v.facts.attachments[0]!.attachment.sizeBytes = 2; },
    (v: typeof initial) => { v.facts.attachments[0]!.attachment.dataUrl = 'data:image/jpeg;base64,YQ=='; },
    (v: typeof initial) => { v.facts.attachments[1]!.kind = 'inline-image'; },
  ];
  for (const change of cases) { const value = copy(initial); change(value); expect(mobileOutboxMessagePlan(value.record, value.facts, value.thread).status).toBe('blocked'); }
  const value = copy(initial); obj(obj(value.facts.config.environment).capabilities).serverResolvedCommandContext = false;
  expect(mobileOutboxMessagePlan(value.record, value.facts, value.thread)).toEqual({ status: 'needs-projection' });
});
test('reject partial, extra, malformed and unrelated persisted descriptors', () => {
  const { plan, fixture: { response } } = inlinePlan();
  const badReplies: unknown[] = [null, {}, { attachments: {} }, { attachments: [] }, { attachments: response.attachments.slice(1) },
    { attachments: [...response.attachments, response.attachments[0]] }];
  for (const edit of [{ type: 'file' }, { name: 'different.png' }, { mimeType: 'IMAGE/PNG' }, { sizeBytes: 2 }, { dataUrl: 'data:image/png;base64,YQ==' }])
    badReplies.push({ attachments: [{ ...response.attachments[0], ...edit }, response.attachments[1]] });
  for (const reply of badReplies) expect(mobileOutboxMaterializeCommand(plan, reply).status).toBe('blocked');
});
test('persisted IDs admit exact wire vocabulary and reject trailing line terminators', () => {
  const { plan, fixture: { response } } = inlinePlan();
  for (const id of ['', 'a'.repeat(129), 'asset\n', 'asset\r', 'asset\u2028', 'asset\u2029', ' asset', 'asset/id', 'é'])
    expect(mobileOutboxMaterializeCommand(plan, { attachments: [{ ...response.attachments[0], id }, response.attachments[1]] }).status).toBe('blocked');
  for (const id of ['A-0_z', 'a'.repeat(128)])
    expect(mobileOutboxMaterializeCommand(plan, { attachments: [{ ...response.attachments[0], id }, response.attachments[1]] }).status).toBe('ready');
});
test('refuse a response if the captured plan request or command owner changed', () => {
  const { plan, fixture: { response } } = inlinePlan();
  for (const change of [
    (v: MobileOutboxInlinePersistence) => { v.request.payload.messageId = 'other'; },
    (v: MobileOutboxInlinePersistence) => { v.request.payload.attachments.reverse(); },
    (v: MobileOutboxInlinePersistence) => { v.commandTemplate.owner.environmentId = 'other'; },
    (v: MobileOutboxInlinePersistence) => { v.commandTemplate.payload.messageId = 'other'; },
  ]) { const value = copy(plan); change(value); expect(mobileOutboxMaterializeCommand(value, response).status).toBe('blocked'); }
});
