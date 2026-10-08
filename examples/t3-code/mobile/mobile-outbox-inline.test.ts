import { expect, test } from 'bun:test';
import { arr, obj, type Obj } from './shared/domain';
import { mobileOutboxMessagePlan, mobileOutboxLaunchPlan, type MobileOutboxWireFacts } from './mobile-outbox-wire';
import { mobileOutboxMaterializeCommand } from './mobile-outbox-command';
import { mobileOutboxCompactInline, mobileOutboxMaterializeInline } from './mobile-outbox-inline';
import type { MobileOutboxRecord } from './mobile-outbox-model';

function fixture(launch = false, inlineContext = true) {
  const record: MobileOutboxRecord = { schemaVersion: 1, origin: 'https://home.test', environmentId: 'env', threadId: 'thread',
    messageId: 'message', commandId: 'command', text: '[Photo](t3-context://v1/image/photo)', createdAt: '2026-10-08T00:00:00.000Z',
    modelSelection: { instanceId: 'p', model: 'm' }, attachments: [
      { kind: 'image', id: 'first-local', name: 'first.png', mimeType: 'image/png', sizeBytes: 1, status: 'ready' },
      { kind: 'file', id: 'file-local', name: 'file.txt', mimeType: 'text/plain', sizeBytes: 2, status: 'ready', uploadId: 'file-remote', uploadEnvironmentId: 'env' },
      { kind: 'image', id: 'last-local', name: 'last.png', mimeType: 'image/png', sizeBytes: 2, status: 'ready' }],
    context: { version: 1, records: [{ version: 1, kind: 'image', contextId: 'photo', label: 'Photo', attachmentId: 'first-local' }] } };
  const facts: MobileOutboxWireFacts = { origin: record.origin, environmentId: 'env', config: { providers: [], environment: { environmentId: 'env',
    capabilities: { attachmentUploads: false, inlineMessageContext: inlineContext, serverResolvedCommandContext: true } } },
    attachments: record.attachments.map((local, index) => ({ localId: local.id, kind: local.kind === 'image' ? 'inline-image' : 'reference',
      attachment: { type: local.kind, name: local.name, mimeType: local.mimeType, sizeBytes: local.sizeBytes,
        ...(local.kind === 'image' ? { dataUrl: `data:image/png;base64,${index === 0 ? 'YQ==' : 'YmM='}` } : { id: local.uploadId }) } })) };
  if (launch) record.creation = { projectId: 'project', projectCwd: '/repo', workspaceMode: 'worktree', branch: 'main', worktreePath: null };
  const result = launch ? mobileOutboxLaunchPlan(record, facts, 't3/frozen') : mobileOutboxMessagePlan(record, facts,
    { origin: record.origin, environmentId: 'env', threadId: 'thread', modelSelection: record.modelSelection!, runtimeMode: 'full-access', interactionMode: 'default' });
  if (result.status !== 'needs-inline-persistence') throw Error(JSON.stringify(result));
  const response = { attachments: [record.attachments[0]!, record.attachments[2]!].map((file, index) => ({ type: 'image', id: `persisted-${index}`,
    name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes })) };
  return { record, plan: result.value, response };
}
test('compact templates preserve complete final commands and idless context across launch and message', () => {
  for (const launch of [false, true]) for (const inline of [false, true]) {
    const f = fixture(launch, inline), before = JSON.stringify(f), result = mobileOutboxCompactInline(f.record, f.plan);
    expect(result.status).toBe('ready'); if (result.status !== 'ready') return;
    expect(result.value.inline).toEqual([{ index: 0, localId: 'first-local' }, { index: 2, localId: 'last-local' }]);
    expect(JSON.stringify(result.value)).not.toContain('dataUrl');
    expect(mobileOutboxMaterializeInline(result.value, f.response)).toEqual(mobileOutboxMaterializeCommand(f.plan, f.response));
    expect(JSON.stringify(f)).toBe(before);
  }
});
test('compact admission rejects another owner, local byte descriptor or inline order', () => {
  for (const mode of ['owner', 'name', 'mime', 'size', 'type', 'id', 'source', 'extra', 'url', 'subset', 'count'] as const) {
    const f = fixture(), first = arr(f.plan.commandTemplate.payload.attachments)[0]!;
    if (mode === 'owner') f.plan.owner.commandId = 'other';
    if (mode === 'name') first.name = 'other.png';
    if (mode === 'mime') first.mimeType = 'image/jpeg';
    if (mode === 'size') first.sizeBytes = 2;
    if (mode === 'type') f.record.attachments[0]!.kind = 'file';
    if (mode === 'id') first.id = 'unexpected';
    if (mode === 'source') first.source = {};
    if (mode === 'extra') first.unknown = true;
    if (mode === 'url') first.dataUrl = 'file:///tmp/file';
    if (mode === 'subset') f.plan.request.payload.attachments.reverse();
    if (mode === 'count') f.record.attachments.pop();
    expect(mobileOutboxCompactInline(f.record, f.plan).status).toBe('blocked');
  }
});
test('compact materialization rejects malformed binding indexes and payload slots', () => {
  for (const mode of ['reverse', 'duplicate', 'fraction', 'negative', 'outside', 'id', 'source', 'url', 'missing-reference'] as const) {
    const f = fixture(), result = mobileOutboxCompactInline(f.record, f.plan);
    if (result.status !== 'ready') throw Error('fixture');
    const value = result.value, attachments = arr(value.commandTemplate.payload.attachments);
    if (mode === 'reverse') value.inline.reverse();
    if (mode === 'duplicate') value.inline[1]!.localId = value.inline[0]!.localId;
    if (mode === 'fraction') value.inline[0]!.index = 0.5;
    if (mode === 'negative') value.inline[0]!.index = -1;
    if (mode === 'outside') value.inline[1]!.index = 3;
    if (mode === 'id') attachments[0]!.id = 'unrelated';
    if (mode === 'source') attachments[0]!.source = {};
    if (mode === 'url') attachments[0]!.dataUrl = 'data:image/png;base64,YQ==';
    if (mode === 'missing-reference') delete attachments[1]!.id;
    expect(mobileOutboxMaterializeInline(value, f.response).status).toBe('blocked');
  }
});
test('compact result is detached and malformed persistence replies still fail closed', () => {
  const f = fixture(), compact = mobileOutboxCompactInline(f.record, f.plan);
  if (compact.status !== 'ready') throw Error('fixture');
  const before = JSON.stringify(compact.value);
  for (const response of [{}, { attachments: [] }, { attachments: [...f.response.attachments, f.response.attachments[0]] },
    { attachments: [{ ...f.response.attachments[0], name: 'wrong.png' }, f.response.attachments[1]] }])
    expect(mobileOutboxMaterializeInline(compact.value, response).status).toBe('blocked');
  const result = mobileOutboxMaterializeInline(compact.value, f.response);
  if (result.status !== 'ready') throw Error('fixture');
  arr(result.value.payload.attachments)[0]!.name = 'edited'; obj(result.value.payload.context).version = 9;
  expect(JSON.stringify(compact.value)).toBe(before); expect(f.response.attachments[0]!.name).toBe('first.png');
  compact.value.commandTemplate.payload.text = 'caller edit'; expect(f.plan.commandTemplate.payload.text).not.toBe('caller edit');
});
