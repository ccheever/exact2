import { describe, expect, test } from 'bun:test';
import { mobileOutboxDecode, mobileOutboxEncode, mobileOutboxGroup, mobileOutboxFlatten, mobileOutboxRetryDelay,
  mobileOutboxDeliveryAction, mobileOutboxDispatchStep, mobileOutboxCreationSendable, mobileOutboxResolveSettings,
  mobileOutboxModelsEqual, mobileOutboxShouldRetry, mobileOutboxFailureAction, type MobileOutboxRecord } from './mobile-outbox-model';
const base = (): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://example.test', environmentId: 'env', threadId: 'thread',
  messageId: 'message', commandId: 'command', text: 'hello', attachments: [], createdAt: '2026-10-08T12:00:00.000Z' });
describe('durable mobile outbox model', () => {
  test('round trip preserves restoration metadata, future context and ordered options without aliasing', () => {
    const record: MobileOutboxRecord = { ...base(), attachments: [{ id: '12345678-1234-1234-1234-123456789012',
      kind: 'file', name: 'movie.mp4', mimeType: 'video/mp4', sizeBytes: 42, status: 'ready', uploadId: 'uploaded',
      uploadEnvironmentId: 'env', contextId: 'file_1', source: 'attached', videoWidth: 1920, videoHeight: 1080 }],
      context: { version: 1, records: [{ version: 1, kind: 'future-kind', contextId: 'future_1', label: 'Future', payload: { deeply: ['kept', 1, false] } }] },
      modelSelection: { instanceId: 'provider', model: 'model', options: [{ id: 'reasoning', value: 'high' }, { id: 'fast', value: false }] },
      creation: { projectId: 'project', projectTitle: 'Title', projectCwd: '/repo', workspaceMode: 'worktree', branch: 'main', worktreePath: null, startFromOrigin: false } };
    const encoded = mobileOutboxEncode(record); expect(encoded).toEqual(record); expect(encoded).not.toBe(record);
    encoded.attachments[0]!.name = 'changed'; expect(record.attachments[0]!.name).toBe('movie.mp4');
    expect(mobileOutboxDecode(JSON.parse(JSON.stringify(record)))).toEqual({ ok: true, record });
  });
  test('optional nested fields encode like JSON while undefined array entries are refused', () => {
    const record: MobileOutboxRecord = { ...base(), modelSelection: { instanceId: 'p', model: 'm', options: undefined },
      creation: { projectId: 'p', projectTitle: undefined, workspaceMode: 'local', branch: null, worktreePath: null } };
    expect(mobileOutboxEncode(record)).toEqual(JSON.parse(JSON.stringify(record)));
    expect(mobileOutboxDecode({ ...record, context: { version: 1, records: [undefined] } }).ok).toBe(false);
    expect(mobileOutboxDecode({ ...record, extra: [undefined] }).ok).toBe(false);
  });
  test('corrupt and future records retain unknown ownership, never empty inventory', () => {
    const invalid: unknown[] = [null, [], { ...base(), schemaVersion: 2 }, { ...base(), origin: 'https://example.test/' },
      { ...base(), origin: 'https://user:secret@example.test' }, { ...base(), createdAt: '2026-02-30T00:00:00.000Z' },
      { ...base(), createdAt: '2026-10-08T12:00:00Z' }, { ...base(), messageId: ' ' },
      { ...base(), modelSelection: { instanceId: 'p', model: 'm', options: { fast: true } } },
      { ...base(), attachments: [{ id: '../bytes' }] }, { ...base(), text: 1 },
      { ...base(), context: { version: 2, records: [] } }, { ...base(), runtimeMode: 'legacy' }];
    for (const raw of invalid) { const result = mobileOutboxDecode(raw); expect(result.ok).toBe(false);
      if (!result.ok) { expect(result.ownership).toBe('unknown'); expect(result.raw).toBe(raw); } }
    const cycle: Record<string, unknown> = { ...base() }; cycle.extra = cycle;
    expect(mobileOutboxDecode(cycle).ok).toBe(false);
    expect(() => mobileOutboxEncode({ ...base(), origin: 'bad' })).toThrow();
  });
  test('uploaded attachment is bound to captured environment; duplicate ownership is refused', () => {
    const file = { id: '12345678-1234-1234-1234-123456789012', kind: 'image' as const, name: 'image.png', mimeType: 'image/png',
      sizeBytes: 3, uploadId: 'upload', status: 'ready' as const, source: { kind: 'snapshot', rect: { x: 1 } } };
    expect(mobileOutboxDecode({ ...base(), attachments: [file] }).ok).toBe(false);
    expect(mobileOutboxDecode({ ...base(), attachments: [{ ...file, uploadEnvironmentId: 'other' }] }).ok).toBe(false);
    expect(mobileOutboxDecode({ ...base(), attachments: [{ ...file, uploadEnvironmentId: 'env' }] }).ok).toBe(true);
    const staged = { ...file, id: 'abcdefab-abcd-abcd-abcd-abcdefabcdef', uploadId: '', status: 'staged' };
    expect(mobileOutboxDecode({ ...base(), attachments: [staged, { ...staged, id: staged.id.toUpperCase() }] }).ok).toBe(false);
    expect(mobileOutboxDecode({ ...base(), attachments: [staged, staged] }).ok).toBe(false);
  });
  test('last duplicate message wins, thread queues are stable oldest first, flatten preserves groups', () => {
    const old = { ...base(), text: 'old' }, replacement = { ...old, text: 'replacement' };
    const first = { ...base(), messageId: 'first', createdAt: '2026-10-07T12:00:00.000Z' };
    const other = { ...base(), messageId: 'other', environmentId: 'second' };
    const queues = mobileOutboxGroup([old, other, first, replacement]);
    expect(queues['env:thread']?.map(item => item.text)).toEqual(['hello', 'replacement']);
    expect(mobileOutboxFlatten(queues).map(item => item.messageId)).toEqual(['first', 'message', 'other']);
    expect(mobileOutboxGroup([old, { ...replacement, environmentId: 'second' }])['env:thread']).toBeUndefined();
  });
  test('creation dedupe requires live shell before send, existing busy turns may enqueue', () => {
    for (const isCreation of [false, true]) for (const threadExists of [false, true]) for (const shellStatus of ['idle', 'loading', 'live', 'error'])
      for (const environmentConnected of [false, true]) {
        const input = { isCreation, threadExists, shellStatus, environmentConnected, threadBusy: false };
        const expected = isCreation ? threadExists ? 'remove' : environmentConnected && shellStatus === 'live' ? 'send' : 'wait' :
          !threadExists ? shellStatus === 'live' ? 'remove' : 'wait' : environmentConnected ? 'send' : 'wait';
        expect(mobileOutboxDeliveryAction(input)).toBe(expected);
        expect(mobileOutboxDeliveryAction({ ...input, threadBusy: true })).toBe(expected);
      }
  });
  test('config waits do not block cleanup; file admission reports source limits', () => {
    expect(mobileOutboxDispatchStep({ deliveryAction: 'remove', fileAttachments: [], serverConfig: null })).toEqual({ step: 'remove' });
    expect(mobileOutboxDispatchStep({ deliveryAction: 'send', fileAttachments: [], serverConfig: null })).toEqual({ step: 'retry' });
    expect(mobileOutboxDispatchStep({ deliveryAction: 'send', fileAttachments: [], serverConfig: { maxFileUploadBytes: undefined } })).toEqual({ step: 'send' });
    const fileAttachments = [{ name: 'file', sizeBytes: 50 * 1024 * 1024 + 1 }];
    expect(mobileOutboxDispatchStep({ deliveryAction: 'send', fileAttachments, serverConfig: { maxFileUploadBytes: undefined } })).toEqual({ step: 'restore', reason: 'This server does not support file attachments.' });
    for (const [maxFileUploadBytes, limit] of [[100 * 1024 * 1024, '50 MB'], [1024, '1 KB'], [1, '1 byte'], [3, '3 bytes']] as const)
      expect(mobileOutboxDispatchStep({ deliveryAction: 'send', fileAttachments, serverConfig: { maxFileUploadBytes } })).toEqual({ step: 'restore', reason: `'file' exceeds the ${limit} attachment limit.` });
  });
  test('creation waits for content, model and worktree base; settings follow captured provider', () => {
    const modelSelection = { instanceId: 'p', model: 'm' }, creation = { projectId: 'project', workspaceMode: 'worktree' as const, branch: null, worktreePath: null };
    expect(mobileOutboxCreationSendable(base())).toBe(false);
    expect(mobileOutboxCreationSendable({ ...base(), modelSelection, creation })).toBe(false);
    expect(mobileOutboxCreationSendable({ ...base(), modelSelection, creation: { ...creation, branch: 'main' } })).toBe(true);
    expect(mobileOutboxCreationSendable({ ...base(), text: ' ', modelSelection, creation: { ...creation, workspaceMode: 'local' } })).toBe(false);
    const thread = { modelSelection, runtimeMode: 'auto' as const, interactionMode: 'plan' as const };
    expect(mobileOutboxResolveSettings(base(), thread)).toEqual(thread);
    expect(mobileOutboxResolveSettings({ ...base(), runtimeMode: 'full-access' }, thread, [{ instanceId: 'p', showInteractionModeToggle: false }])).toEqual({ ...thread, runtimeMode: 'full-access', interactionMode: 'default' });
    expect(mobileOutboxModelsEqual(modelSelection, { ...modelSelection, options: [] })).toBe(false);
    expect(mobileOutboxModelsEqual(modelSelection, { ...modelSelection })).toBe(true);
  });
  test('source typed failure policy, interruption and capped retry delay', () => {
    expect([-1, 0, 1, 2, 3, 4, 5, 30].map(mobileOutboxRetryDelay)).toEqual([1000, 1000, 1000, 2000, 4000, 8000, 16000, 16000]);
    for (const _tag of ['ConnectionTransientError', 'RpcClientError', 'EnvironmentRpcUnavailableError', 'EnvironmentNotRegisteredError']) expect(mobileOutboxShouldRetry({ _tag, message: 'arbitrary' })).toBe(true);
    for (const _tag of ['OrchestrationDispatchCommandError', 'EnvironmentAuthorizationError']) expect(mobileOutboxShouldRetry({ _tag, message: 'SocketCloseError' })).toBe(false);
    for (const error of ['SocketCloseError', new Error('server disconnected.'), { message: 'ping timeout' }]) expect(mobileOutboxShouldRetry(error)).toBe(true);
    for (const error of [null, {}, 'disconnected. Additional business detail.', 'model invalid']) expect(mobileOutboxShouldRetry(error)).toBe(false);
    expect(mobileOutboxFailureAction({ stage: 'settings-sync', interrupted: false, error: 'model invalid' })).toBe('retry');
    expect(mobileOutboxFailureAction({ stage: 'start-turn', interrupted: true, error: 'model invalid' })).toBe('retry');
    expect(mobileOutboxFailureAction({ stage: 'start-turn', interrupted: false, error: 'model invalid' })).toBe('restore');
  });
});
