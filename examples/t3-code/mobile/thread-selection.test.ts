import { afterEach, expect, test } from 'bun:test';
import { mobileThreadSelection } from './thread-selection';
import { T3Client } from './shared/client';
import { fleet } from './shared/settings-b-fleet';
import { initialShell, obj, str, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';

afterEach(() => fleet.entries.clear());

function fixture() {
  const client = new T3Client(), calls: Obj[] = [];
  client.adoptStatus({ state: 'connected', origin: 'https://focused.invalid', environmentId: 'focused', message: '' }, 1);
  client.shell = { ...initialShell(), projects: [{ id: 'project', title: 'Project' }],
    threads: [{ id: 'thread', projectId: 'project' }] };
  client.projectId = 'project';
  const projection: Obj = { thread: client.shell.threads[0] };
  for (const key of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns',
    'runtimeRequests', 'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[key] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'connect') return { ok: true, generation: 2,
      value: { state: 'connected', origin: 'https://remote.invalid', environmentId: 'remote', message: '' } };
    return { ok: true, generation: client.generation, value: request.op === 'http'
      ? { snapshotSequence: 1, projection } : request.op === 'subscribe' ? { id: 'thread-sub' } : {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; }, async atomicWriteFile() {} } };
  return { client, native, storage, calls };
}

test('a stale fleet route opens the now-focused thread without reconnecting', async () => {
  const f = fixture();
  const result = await mobileThreadSelection(f.client, 'fleet:focused:thread', f.native, f.storage);
  expect(result.message).toBe('');
  expect(f.client.threadId).toBe('thread');
  expect(obj(f.client.thread?.projection.thread).id).toBe('thread');
  expect(f.calls.some(call => call.path === '/api/orchestration/threads/thread/bounded')).toBe(true);
  expect(f.calls.filter(call => ['connect', 'fleetStop'].includes(str(call.op)))).toEqual([]);
});

test('a different environment retains its identity and uses the real fleet focus', async () => {
  const f = fixture(), key = 'https://remote.invalid\nremote';
  fleet.entries.set(key, { key, origin: 'https://remote.invalid', environmentId: 'remote', phase: 'connected', message: '',
    traceId: '', generation: 4, synchronized: 4, lastEvent: 0, subscriptions: {}, config: {},
    shell: f.client.shell, scopes: ['orchestration:operate'], error: '', requested: true });
  const result = await mobileThreadSelection(f.client, 'fleet:remote:thread', f.native, f.storage);
  expect(result.message).toBe('');
  expect(f.client.environmentId).toBe('remote');
  expect(f.client.threadId).toBe('thread');
  expect(f.calls.filter(call => call.op === 'connect')).toEqual([{ op: 'connect', origin: 'https://remote.invalid', credential: '' }]);
  expect(f.calls.filter(call => call.op === 'fleetStop')).toEqual([{ op: 'fleetStop', fleet: key }]);
  expect(fleet.entries.has(key)).toBe(false);
  // The native focus has completed, but its shell has not loaded yet. A route
  // computed from the previous projection can repeat that exact fleet target.
  expect(f.client.shell.threads).toEqual([]);
  f.calls.length = 0;
  expect((await mobileThreadSelection(f.client, 'fleet:remote:thread', f.native, f.storage)).message).toBe('');
  expect(f.client.threadId).toBe('thread');
  expect(f.client.projectId).toBe('project');
  expect(f.calls).toEqual([]);
});

test('a missing different environment cannot open a same-named local thread', async () => {
  const f = fixture();
  const result = await mobileThreadSelection(f.client, 'fleet:missing:thread', f.native, f.storage);
  expect(result.message).toBe('That environment is no longer connected.');
  expect(f.client.threadId).toBe('');
  expect(f.client.environmentId).toBe('focused');
  expect(f.calls.some(call => call.op === 'http' || call.op === 'connect')).toBe(false);
});

test('normal local selection and real read failures keep their existing behavior', async () => {
  const f = fixture(), original = f.native.later;
  f.native.later = async input => obj(input).op === 'http'
    ? { ok: false, generation: 1, error: { kind: 'Permission', message: 'Thread read denied.', uncertain: false } }
    : original(input);
  expect((await mobileThreadSelection(f.client, 'fleet:focused:thread', f.native, f.storage)).message).toBe('Thread read denied.');
  expect(f.client.thread).toBeNull();
  f.native.later = original;
  expect((await mobileThreadSelection(f.client, 'thread', f.native, f.storage)).message).toBe('');
  expect(obj(f.client.thread?.projection.thread).id).toBe('thread');
});
