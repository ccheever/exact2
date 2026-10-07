import { describe, test, expect } from 'bun:test';
import { T3Client } from './shared/client';
import { mobileSend, mobileSubmissionNative } from './composer-behavior';
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
