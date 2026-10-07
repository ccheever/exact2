import { describe, test, expect } from 'bun:test';
import { T3Client } from './shared/client';
import { mobileSend, mobileSubmissionNative } from './composer-behavior';
import { mobileThreadComposer } from './thread';
import { sendIntent } from './shared/composer-editor-intent';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';

function fixture(preferred: string) {
  const client = new T3Client();
  client.environmentId = 'env'; client.projectId = 'project'; client.threadId = 'thread';
  client.origin = 'https://test.invalid'; client.connection = 'connected';
  client.configLive = true; client.shellLive = true; client.threadLive = true; client.shellLoaded = true;
  client.scopes = ['orchestration:operate']; client.providerId = 'provider'; client.modelId = 'model';
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [
    { instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready', models: [{ slug: 'model', name: 'Model' }] }] };
  client.shell.projects = [{ id: 'project', title: 'Project', workspaceRoot: '/project' }];
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1,
    projection: { thread: { id: 'thread', projectId: 'project', modelSelection: { instanceId: 'provider', model: 'model' } },
      runs: [{ id: 'run', ordinal: 1, status: 'running', providerThreadId: 'provider-thread', activeAttemptId: 'attempt' }],
      providerThreads: [{ id: 'provider-thread', appThreadId: 'thread', providerSessionId: 'session' }],
      providerSessions: [{ id: 'session', capabilities: { turns: { supportsActiveSteering: true } } }],
      providerTurns: [{ runAttemptId: 'attempt', status: 'running' }], turnItems: [] } };
  client.local.drafts[client.draftKey] = 'Follow up';
  const calls: Obj[] = []; let next = 0;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const value = request.op === 'mobilePreferences' ? { followUpBehavior: preferred }
      : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `id-${++next}`) : {};
    return { ok: true, generation: client.generation, value };
  } };
  return { client, native, storage, calls };
}

describe('mobile composer behavior', () => {
  for (const preferred of ['queue', 'steer']) for (const alternate of [false, true]) {
    test(`${preferred} preference with ${alternate ? 'alternate' : 'normal'} submit reaches actual shared dispatch`, async () => {
      const f = fixture(preferred);
      expect(await mobileSend(f.client, alternate, f.native, f.storage)).toMatchObject({ message: '' });
      const payload = obj(f.calls.find(call => call.method === 'orchestration.dispatchCommand' && obj(call.payload).type === 'message.dispatch')?.payload);
      const expected = alternate ? preferred === 'queue' ? 'steer' : 'queue' : preferred;
      expect(payload.dispatchMode).toEqual({ type: expected === 'queue' ? 'queue_after_active' : 'start_immediately' });
      if (expected === 'steer') expect(payload.deliveryIntent).toBe('steer');
      expect(f.calls.some(call => call.op === 'composerSendIntent')).toBe(false);
    });
  }
  test('a running provider without steering queues even when the device preference or chord requests steering', async () => {
    for (const alternate of [false, true]) {
      const f = fixture('steer'); f.client.thread!.projection.providerSessions = [];
      expect(await mobileSend(f.client, alternate, f.native, f.storage)).toMatchObject({ message: '' });
      const payload = obj(f.calls.find(call => obj(call.payload).type === 'message.dispatch')?.payload);
      expect(payload.dispatchMode).toEqual({ type: 'queue_after_active' });
    }
  });
  test('an owner change during preference loading cannot submit another draft', async () => {
    const f = fixture('queue'), original = f.native.later;
    f.native.later = async request => { if (obj(request).op === 'mobilePreferences') f.client.projectId = 'other'; return original(request); };
    await expect(mobileSend(f.client, false, f.native, f.storage)).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.calls.some(call => call.op === 'request')).toBe(false);
  });
  for (const awaitPoint of ['devicePresentation', 'ids']) for (const change of ['thread', 'model']) {
    test(`a ${change} change during ${awaitPoint} cannot dispatch the new owner`, async () => {
      const f = fixture('queue'), original = f.native.later;
      f.native.later = async request => {
        if (obj(request).op === awaitPoint) {
          if (change === 'thread') {
            f.client.threadId = 'other';
            f.client.thread!.projection.thread = { ...obj(f.client.thread!.projection.thread), id: 'other' };
            f.client.local.drafts[f.client.draftKey] = 'Other owned draft';
          } else f.client.modelId = 'other-model';
        }
        return original(request);
      };
      await mobileSend(f.client, false, f.native, f.storage);
      expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
      expect(f.client.local.drafts['env:thread']).toBe('Follow up');
      if (change === 'thread') expect(f.client.local.drafts['env:other']).toBe('Other owned draft');
    });
  }
  test('resolved mobile action is independent of desktop keyboard bindings and never backgrounds a new task', async () => {
    const f = fixture('queue');
    const native = mobileSubmissionNative(f.native, true);
    const result = obj(await native.later({ op: 'composerSendIntent', generation: 3 }));
    expect(sendIntent({ keybindings: [] }, obj(result.value), true, false)).toBe('alternate');
    expect(sendIntent({}, obj(result.value), false, true)).toBe('foreground');
  });
});

// Sanitized Moonbase server.getConfig capture, 2026-10-07. The seeded thread
// retains gpt-5.4 while the authenticated real Codex catalog has these seven models.
const capturedCodexModels = ['gpt-6.1-sol', 'gpt-6-astra', 'gpt-6-sol', 'gpt-6-luna', 'gpt-5.6-sol', 'gpt-5.6-terra', 'gpt-5.6-luna'];
function capturedSelection(f: ReturnType<typeof fixture>, driver = 'codex') {
  f.client.providerId = 'codex'; f.client.modelId = 'gpt-5.4';
  f.client.thread!.projection.thread = { ...obj(f.client.thread!.projection.thread), modelSelection: { instanceId: 'codex', model: 'gpt-5.4' } };
  f.client.config.providers = [{ instanceId: 'codex', driver, enabled: true, installed: true, auth: { status: 'authenticated' },
    status: 'ready', models: capturedCodexModels.map(slug => ({ slug, name: slug })) }];
}
test('retained Codex selection absent from the real catalog reaches durable follow-up dispatch with its options', async () => {
  const f = fixture('queue'); capturedSelection(f);
  f.client.modelOptions = [{ id: 'effort', value: 'high' }];
  const before = JSON.stringify(f.client.config.providers);
  expect(mobileThreadComposer(f.client)).toMatchObject({ modelLabel: 'gpt-5.4', modelUnavailable: false, canSend: true });
  expect(await mobileSend(f.client, false, f.native, f.storage)).toMatchObject({ message: '' });
  const payload = obj(f.calls.find(call => call.method === 'orchestration.dispatchCommand' && obj(call.payload).type === 'message.dispatch')?.payload);
  expect(payload.modelSelection).toEqual({ instanceId: 'codex', model: 'gpt-5.4', options: [{ id: 'effort', value: 'high' }] });
  expect(payload.dispatchMode).toEqual({ type: 'queue_after_active' });
  expect(JSON.stringify(f.client.config.providers)).toBe(before);
  expect(f.client.pending).toBeUndefined();
});
test('retained Codex selection and options survive a new-task launch and server adoption', async () => {
  const f = fixture('queue'); capturedSelection(f);
  const selected = { instanceId: 'codex', model: 'gpt-5.4', options: [{ id: 'effort', value: 'high' }] };
  f.client.threadId = ''; f.client.thread = undefined;
  f.client.modelOptions = selected.options;
  f.client.local.drafts[f.client.draftKey] = 'New task';
  const original = f.native.later;
  f.native.later = async input => {
    const response = await original(input), request = obj(input);
    if (request.method === 'orchestration.launchThread') return { ok: true, generation: f.client.generation, value: { threadId: 'launched' } };
    if (request.op === 'http' && request.path === '/api/orchestration/threads/launched/bounded') {
      const projection: Obj = { thread: { id: 'launched', projectId: 'project', modelSelection: selected } };
      for (const family of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests',
        'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[family] = [];
      return { ok: true, generation: f.client.generation, value: { snapshotSequence: 1, projection } };
    }
    return response;
  };
  expect(await mobileSend(f.client, false, f.native, f.storage)).toMatchObject({ message: '' });
  const writes = f.calls.filter(call => call.method === 'orchestration.launchThread');
  expect(writes).toHaveLength(1);
  expect(obj(writes[0]!.payload).modelSelection).toEqual(selected);
  expect(obj(obj(writes[0]!.payload).initialMessage).text).toBe('New task');
  expect(f.client.threadId).toBe('launched');
  expect(f.client.modelOptions).toEqual(selected.options);
  expect(f.client.pending).toBeUndefined();
});
test('known catalog models still normalize dispatch options and add the implicit Fast-mode default', async () => {
  const f = fixture('queue');
  obj((f.client.config.providers as Obj[])[0]).models = [{ slug: 'model', name: 'Model', capabilities: { optionDescriptors: [
    { id: 'effort', type: 'select', options: [{ id: 'high', name: 'High' }] }, { id: 'fastMode', type: 'boolean' },
  ] } }];
  f.client.modelOptions = [{ id: 'effort', value: 'high' }, { id: 'obsolete', value: 'discard' }];
  expect(await mobileSend(f.client, false, f.native, f.storage)).toMatchObject({ message: '' });
  const payload = obj(f.calls.find(call => obj(call.payload).type === 'message.dispatch')?.payload);
  expect(payload.modelSelection).toEqual({ instanceId: 'provider', model: 'model', options: [{ id: 'effort', value: 'high' }, { id: 'fastMode', value: false }] });
});
test('Antigravity catalog refusal happens before attachment reads, uploads or durable dispatch', async () => {
  const f = fixture('queue'); capturedSelection(f, 'antigravity');
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: 'pending-image', name: 'pending.png', mimeType: 'image/png', sizeBytes: 3 }];
  expect(mobileThreadComposer(f.client)).toMatchObject({ modelUnavailable: true, canSend: false });
  expect((await mobileSend(f.client, false, f.native, f.storage)).message).toBe('Model unavailable. Open model settings.');
  expect(f.calls.some(call => call.op === 'snapshotDraftRead' || call.op === 'ids' || call.op === 'request')).toBe(false);
  expect(f.client.draft).toBe('Follow up');
});
test('provider authentication and readiness remain required before uploads even for retained non-catalog models', async () => {
  for (const patch of [{ auth: { status: 'unauthenticated' } }, { enabled: false }, { installed: false }, { availability: 'unavailable' }, { status: 'error' }]) {
    const f = fixture('queue'); capturedSelection(f); Object.assign(obj((f.client.config.providers as Obj[])[0]), patch);
    f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: 'pending-image', name: 'pending.png', mimeType: 'image/png', sizeBytes: 3 }];
    expect(mobileThreadComposer(f.client)).toMatchObject({ modelUnavailable: false, canSend: false });
    expect((await mobileSend(f.client, false, f.native, f.storage)).message).toBe('This provider is unavailable. Configure it in T3 Code.');
    expect(f.calls.some(call => call.op === 'snapshotDraftRead' || call.op === 'ids' || call.op === 'request')).toBe(false);
  }
});
test('a provider auth downgrade during native presentation rejects before uploads', async () => {
  const f = fixture('queue'); capturedSelection(f); const original = f.native.later;
  f.native.later = async request => {
    if (obj(request).op === 'devicePresentation') obj((f.client.config.providers as Obj[])[0]).auth = { status: 'unauthenticated' };
    return original(request);
  };
  expect((await mobileSend(f.client, false, f.native, f.storage)).message).toBe('This provider is unavailable. Configure it in T3 Code.');
  expect(f.calls.some(call => call.op === 'snapshotDraftRead' || call.op === 'ids' || call.op === 'request')).toBe(false);
});
