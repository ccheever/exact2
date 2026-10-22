import { expect, test } from 'bun:test';
import { mobileOutboxLaunchRequest, mobileOutboxMessageRequest, mobileOutboxSettingsRequests, mobileOutboxMessageContent,
  mobileOutboxTitle, type MobileOutboxWireFacts, type MobileOutboxThreadFacts } from './mobile-outbox-wire';
import type { MobileOutboxRecord } from './mobile-outbox-model';
const record = (): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://server.test', environmentId: 'env',
  threadId: 'thread', messageId: 'message', commandId: 'command', text: '  first\n second  ', attachments: [],
  modelSelection: { instanceId: 'p', model: 'm', options: [] }, createdAt: '2026-10-08T00:00:00.000Z' });
const facts = (): MobileOutboxWireFacts => ({ origin: 'https://server.test', environmentId: 'env', attachments: [],
  config: { environment: { environmentId: 'env', capabilities: { inlineMessageContext: true, serverResolvedCommandContext: true } }, providers: [] } });
const thread = (): MobileOutboxThreadFacts => ({ origin: 'https://server.test', environmentId: 'env', threadId: 'thread',
  modelSelection: { instanceId: 'p', model: 'm' }, runtimeMode: 'auto', interactionMode: 'plan' });
const creation = { projectId: 'project', projectCwd: '/captured', workspaceMode: 'local' as const, branch: null, worktreePath: null };
const ready = <T>(result: { status: string; value?: T }): T => { expect(result.status).toBe('ready'); return result.value!; };

test('launch captures IDs and workspace, trims text and keeps empty model options', () => {
  const input = { ...record(), creation }, current = facts(), before = JSON.stringify({ input, current });
  const result = ready(mobileOutboxLaunchRequest(input, current));
  expect(result.owner).toEqual({ origin: input.origin, environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command' });
  expect(result.payload).toEqual({ commandId: 'command', threadId: 'thread', projectId: 'project', title: 'first second',
    generateTitle: true, creationSource: 'mobile', modelSelection: input.modelSelection, runtimeMode: 'full-access', interactionMode: 'default',
    workspaceStrategy: { type: 'root' }, initialMessage: { messageId: 'message', text: 'first\n second', attachments: [] } });
  expect(JSON.stringify({ input, current })).toBe(before);
  expect(ready(mobileOutboxLaunchRequest({ ...input, creation: { ...creation, branch: '', worktreePath: '/tree' } }, current)).payload.workspaceStrategy)
    .toEqual({ type: 'existing_worktree', branch: '', worktreePath: '/tree' });
  expect(ready(mobileOutboxLaunchRequest({ ...input, creation: { ...creation, workspaceMode: 'worktree', branch: 'base', worktreePath: '/stale', startFromOrigin: true } }, current, 't3/12345678')).payload.workspaceStrategy)
    .toEqual({ type: 'worktree', baseRef: 'base', branch: 't3/12345678', startFromOrigin: true });
});
test('endpoint and target mismatches block, no foreground fallback', () => {
  expect(mobileOutboxLaunchRequest({ ...record(), creation }, { ...facts(), environmentId: 'other' }).status).toBe('blocked');
  expect(mobileOutboxLaunchRequest({ ...record(), creation }, { ...facts(), origin: 'https://other.test' }).status).toBe('blocked');
  expect(mobileOutboxMessageRequest(record(), facts(), { ...thread(), threadId: 'other' }).status).toBe('blocked');
  expect(mobileOutboxMessageRequest(record(), facts(), thread(), { thread: { id: 'other' } }).status).toBe('blocked');
  expect(mobileOutboxLaunchRequest({ ...record(), creation: { ...creation, workspaceMode: 'worktree', branch: 'base' } }, facts()).status).toBe('blocked');
});
test('settings commands use derived stable IDs and source provider-forced Build', () => {
  const current = facts(); current.config.providers = [{ instanceId: 'p', showInteractionModeToggle: false }];
  const commands = ready(mobileOutboxSettingsRequests({ ...record(), runtimeMode: 'full-access', interactionMode: 'plan' }, current, thread()));
  expect(commands.map(command => command.payload)).toEqual([
    { type: 'thread.runtime-mode.set', commandId: 'command:runtime-mode', threadId: 'thread', runtimeMode: 'full-access' },
    { type: 'thread.interaction-mode.set', commandId: 'command:interaction-mode', threadId: 'thread', interactionMode: 'default' },
  ]);
  expect(ready(mobileOutboxSettingsRequests(record(), facts(), thread()))).toEqual([]);
});
test('absent mode keeps source start without title seed or delivery intent', () => {
  const message = ready(mobileOutboxMessageRequest(record(), facts(), thread()));
  expect(message.payload).toEqual({ type: 'message.dispatch', commandId: 'command', threadId: 'thread', messageId: 'message',
    text: record().text, attachments: [], createdBy: 'user', creationSource: 'mobile', dispatchMode: { type: 'start_immediately' }, modelSelection: record().modelSelection });
  for (const mode of ['auto', 'queue', 'steer', 'restart'] as const) {
    const payload = ready(mobileOutboxMessageRequest({ ...record(), dispatchMode: mode }, facts(), thread())).payload;
    expect(payload.titleSeed).toBe('first second'); expect(payload.deliveryIntent).toBe(mode === 'queue' ? undefined : mode);
    expect(payload.dispatchMode).toEqual({ type: mode === 'queue' ? 'queue_after_active' : 'start_immediately' });
  }
});
test('older server requests a target projection and resolves last active run capabilities', () => {
  const current = facts(); current.config.environment = { environmentId: 'env', capabilities: { inlineMessageContext: true } };
  const input = { ...record(), dispatchMode: 'auto' as const };
  expect(mobileOutboxMessageRequest(input, current, thread())).toEqual({ status: 'needs-projection' });
  const projection = { thread: { id: 'thread' }, runs: [{ id: 'old', status: 'running', providerThreadId: 'old' }, { id: 'latest', status: 'waiting', providerThreadId: 'provider-thread' }],
    providerThreads: [{ id: 'provider-thread', providerSessionId: 'session' }], providerSessions: [{ id: 'session', capabilities: { turns: { supportsSteeringByInterruptRestart: true } } }], messages: [{ id: 'prior' }] };
  const payload = ready(mobileOutboxMessageRequest(input, current, thread(), projection)).payload;
  expect(payload.dispatchMode).toEqual({ type: 'restart_active', targetRunId: 'latest' }); expect(payload.titleSeed).toBeUndefined();
  expect(ready(mobileOutboxMessageRequest(record(), current, thread())).payload.dispatchMode).toEqual({ type: 'start_immediately' });
});
test('source Antigravity admission does not replace missing other-provider models', () => {
  const input = { ...record(), creation }, current = facts();
  expect(mobileOutboxLaunchRequest(input, current).status).toBe('ready');
  current.config.settings = { providerInstances: { p: { driver: 'antigravity' } } };
  expect(mobileOutboxLaunchRequest(input, current)).toEqual({ status: 'blocked', reason: 'Antigravity model unavailable. Set it up on web or desktop, or choose another model.' });
});
test('context remaps exact adopted attachment IDs and retains metadata without mutating capture', () => {
  const input: MobileOutboxRecord = { ...record(), text: '[video](t3-context://v1/file/file_1)', attachments: [{ kind: 'file', id: 'local', name: 'video.mov', mimeType: 'video/quicktime',
    sizeBytes: 3, uploadId: 'remote', uploadEnvironmentId: 'env', status: 'ready', contextId: 'file_1', source: 'attached', videoWidth: 20, videoHeight: 30 }],
    context: { version: 1, records: [{ version: 1, kind: 'file', contextId: 'file_1', label: 'video', attachmentId: 'local', source: 'attached' },
      { version: 1, kind: 'preview-annotation', contextId: 'annotation', label: 'preview', screenshotContextId: 'image_1' }] } };
  const prepared = [{ localId: 'local', attachment: { type: 'file', id: 'remote', name: 'video.mov', mimeType: 'video/quicktime', sizeBytes: 3, source: 'attached', videoWidth: 20, videoHeight: 30 } }];
  const before = JSON.stringify(input), content = mobileOutboxMessageContent(input, prepared, true);
  expect(content.context?.records).toEqual([{ ...input.context!.records![0], attachmentId: 'remote' }, input.context!.records![1]]);
  expect(content.attachments).toEqual(prepared.map(value => value.attachment)); expect(JSON.stringify(input)).toBe(before);
  expect(() => mobileOutboxMessageContent(input, [], true)).toThrow();
  expect(() => mobileOutboxMessageContent(input, [{ ...prepared[0]!, localId: 'wrong' }], true)).toThrow();
  expect(() => mobileOutboxMessageContent(input, [{ ...prepared[0]!, attachment: { ...prepared[0]!.attachment, id: 'uncommitted' } }], true)).toThrow();
});
test('legacy context expands repeated terminal references and preserves orphan links', () => {
  const input: MobileOutboxRecord = { ...record(), text: '[log](t3-context://v1/terminal/t) [again](t3-context://v1/terminal/t) [orphan](t3-context://v1/file/nope)  ',
    context: { version: 1, records: [{ version: 1, kind: 'terminal', contextId: 't', label: 'log', terminalLabel: 'Dev Server', lineStart: 2, lineEnd: 3, text: 'one\ntwo\n' }] } };
  expect(mobileOutboxMessageContent(input, [], false)).toEqual({ text: '@dev-server:2-3 @dev-server:2-3 [orphan](t3-context://v1/file/nope)\n\n<terminal_context>\n- Dev Server lines 2-3:\n  2 | one\n  3 | two\n</terminal_context>', attachments: [] });
  expect(mobileOutboxTitle('', [{ name: 'notes.txt' }])).toBe('Image: notes.txt');
  expect(mobileOutboxTitle('[chip](t3-context://v1/file/file_1)', [])).toBe('[chip](t3-context://v1/file/file_1)');
});

test('legacy preview retains source field prefixes', () => {
  const input: MobileOutboxRecord = { ...record(), text: '', context: { version: 1, records: [{ version: 1, kind: 'preview-annotation', contextId: 'p', label: 'Preview', annotationId: 'id', pageTitle: 'Page', comment: 'Red', targetSummary: 'Button', styleChanges: [] }] } };
  expect(mobileOutboxMessageContent(input, [], false).text).toBe('<preview_annotation>\nId: id\nPage: Page\nComment: Red\nTargets: Button\n</preview_annotation>');
});

test('distinct local attachments may share an adopted remote asset', () => {
  const input: MobileOutboxRecord = { ...record(), attachments: ['00000000-0000-4000-8000-000000000001', '00000000-0000-4000-8000-000000000002'].map((id, index) => ({
    kind: 'file', id, contextId: `file_${index}`, name: 'image.png', mimeType: 'image/png', sizeBytes: 3,
    uploadId: 'shared-asset', uploadEnvironmentId: 'env', status: 'ready', source: 'attached',
  })), context: { version: 1, records: [0, 1].map(index => ({ version: 1, kind: 'file', contextId: `file_${index}`, label: 'image',
    attachmentId: `00000000-0000-4000-8000-00000000000${index + 1}` })) } };
  const prepared = input.attachments.map(file => ({ localId: file.id, attachment: {
    type: 'image', id: 'shared-asset', name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes,
  } }));
  const content = mobileOutboxMessageContent(input, prepared, true);
  expect(content.attachments.map(file => file.id)).toEqual(['shared-asset', 'shared-asset']);
  expect(content.context?.records).toEqual([0, 1].map(index => ({ version: 1, kind: 'file', contextId: `file_${index}`, label: 'image', attachmentId: 'shared-asset' })));
  expect(ready(mobileOutboxMessageRequest(input, { ...facts(), attachments: prepared }, thread())).payload.attachments).toEqual(content.attachments);
});
