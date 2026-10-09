import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileThreadSelection } from './thread-selection';
import { obj, str, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { mobileCacheFlushDeletes, mobileCacheObserveAdoption, mobileCacheObserveThread, mobileCacheThreadDeleted } from './mobile-client-cache-lifecycle';

// The prepared observer is installed only by these tests, not MobileDraftClient.
function observedClient() {
  const client = new T3Client(); mobileCacheObserveAdoption(client); mobileCacheObserveAdoption(client); return client;
}
const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{}').buffer; }, async atomicWriteFile() {} } };
function fixture(client: T3Client, caching = false) {
  const calls: Obj[] = [], events: Obj[] = [], subscriptions: Record<string, string> = {};
  const cache = new Map<string, Obj>();
  let connected = true;
  let sequence = 0, serial = 0;
  const selected = { id: 'thread', projectId: 'project', modelSelection: { instanceId: 'provider', model: 'model' } };
  const config = { environment: { environmentId: 'adoption-env', orchestrationProtocolVersion: 2,
    capabilities: { serverResolvedCommandContext: true } }, providers: [], shellResumeCompletionMarker: true, threadResumeCompletionMarker: true };
  const projection: Obj = { thread: selected };
  for (const family of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests',
    'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[family] = [];
  function enqueue(key: string, value: Obj) {
    events.push({ seq: ++sequence, generation: 1, key, subscriptionId: subscriptions[key], value });
  }
  const good = (value: unknown) => ({ ok: true, generation: 1, value });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'status') return good({ state: connected ? 'connected' : 'disconnected', origin: 'https://adoption.invalid', environmentId: 'adoption-env', message: '' });
    if (request.op === 'environments') return good({ saved: caching ? [{ environmentId: 'adoption-env', origin: 'https://adoption.invalid', enabled: true }] : [] });
    if (request.op === 'http') return good(request.path === '/api/auth/session' ? { authenticated: true, scopes: ['orchestration:operate'] }
      : request.path === '/api/orchestration/shell' ? { snapshotSequence: 1, projects: [{ id: 'project', title: 'Project', workspaceRoot: '/repo' }], threads: [selected] }
        : { snapshotSequence: 1, projection });
    if (request.method === 'server.getConfig') return good(config);
    if (request.op === 'subscribe') {
      const key = str(request.key), id = `subscription-${++serial}`; subscriptions[key] = id;
      enqueue(key, key === 'config' ? { type: 'snapshot', config } : { kind: 'synchronized' }); return good({ id });
    }
    if (request.op === 'events') return good({ events: events.filter(row => Number(row.seq) > Number(request.after)), latest: sequence });
    if (request.op === 'ack') { while (events.length && Number(events[0]!.seq) <= Number(request.through)) events.shift(); return good({ latest: sequence }); }
    if (request.op === 'mobileClientCache') {
      const key = JSON.stringify([request.environmentId, request.kind, request.key]);
      if (request.action === 'remove') { cache.delete(key); return good({ removed: 1 }); }
      if (request.action === 'ticket') return good({ ticket: 'ticket' });
      if (request.action === 'write') {
        cache.set(key, { environmentId: request.environmentId, kind: request.kind, key: request.key,
          schemaVersion: request.schemaVersion, payload: request.payload, updatedAt: 100 });
        return good({ written: true, stale: false });
      }
      if (request.action === 'read') return good({ record: cache.get(key) ?? null });
    }
    return good({});
  } };
  return { client, native, calls, events, cache, connection(value: boolean) { connected = value; }, async ready() {
    await client.refresh(native, storage); client.threadId = 'thread'; await client.openThread(native, 'thread'); await client.refresh(native, storage);
    expect(client.ready).toBe(true); expect(client.threadId).toBe('thread'); expect(client.error).toBe('');
  }, remove(archive = false) {
    enqueue('thread', { kind: 'event', sequence: 2, event: { type: archive ? 'thread.archived' : 'thread.deleted', threadId: 'thread',
      occurredAt: '2026-10-08T10:00:00Z', payload: { ...selected, ...(archive ? { archivedAt: '2026-10-08T10:00:00Z' } : { deletedAt: '2026-10-08T10:00:00Z' }) } } });
    enqueue('shell', archive ? { kind: 'thread.updated', sequence: 2, location: 'archive', thread: { ...selected, archivedAt: '2026-10-08T10:00:00Z' } }
      : { kind: 'thread.removed', sequence: 2, threadId: 'thread', location: 'active' });
  } };
}

test('production mobile refresh persists adopted snapshots, restores cold offline data and refreshes from server on reconnect', async () => {
  const f = fixture(new MobileDraftClient(), true); await f.ready();
  expect(f.cache.size).toBe(3);
  f.connection(false);
  const cold = new MobileDraftClient();
  await cold.refresh(f.native, storage);
  expect(cold.shell.threads.map(row => row.id)).toEqual(['thread']);
  expect(cold.config.environment).toEqual(f.client.config.environment);
  f.calls.length = 0;
  expect((await mobileThreadSelection(cold, 'thread', f.native, storage)).message).toBe('');
  expect(cold.threadId).toBe('thread');
  expect(f.calls.every(row => row.op === 'mobileClientCache')).toBe(true);
  expect((await mobileThreadSelection(cold, 'missing-thread', f.native, storage)).message).toBe('This thread is unavailable offline.');
  expect(cold.threadId).toBe('thread');
  expect(obj(cold.thread?.projection.thread).id).toBe('thread');
  expect(cold.ready).toBe(false); expect(cold.writable).toBe(false);
  expect(cold.configLive).toBe(false); expect(cold.shellLive).toBe(false); expect(cold.threadLive).toBe(false);
  expect(cold.scopes).toEqual([]); expect(cold.subscriptions).toEqual({});
  const previousShell = cold.shell, previousThread = cold.thread;
  f.calls.length = 0; f.connection(true);
  await cold.refresh(f.native, storage);
  expect(cold.shell).not.toBe(previousShell); expect(cold.thread).not.toBe(previousThread);
  expect(f.calls.some(row => row.path === '/api/orchestration/shell')).toBe(true);
  expect(f.calls.some(row => row.path === '/api/orchestration/threads/thread/bounded')).toBe(true);
  expect(cold.ready).toBe(true);
});

test('real refresh drains deletion then shell removal; prepared assignment observer preserves exact cleanup identity', async () => {
  const f = fixture(observedClient()); await f.ready(); f.remove(); await f.client.refresh(f.native, storage);
  expect(f.client.threadId).toBe(''); expect(f.client.thread).toBeNull(); expect(f.client.shell.threads).toEqual([]);
  expect(f.client.error).toBe(''); expect(f.events).toEqual([]);
  expect(mobileCacheThreadDeleted(f.client, 'adoption-env', 'thread')).toBe(true);
  f.client.environmentId = 'another'; f.client.threadId = 'another-thread';
  await mobileCacheFlushDeletes(f.client, f.native);
  expect(f.calls.filter(row => row.op === 'mobileClientCache')).toEqual([
    { op: 'mobileClientCache', action: 'remove', environmentId: 'adoption-env', kind: 'thread', key: 'thread' },
  ]);
});
test('archive event and same-batch shell removal never become cache deletion', async () => {
  const f = fixture(observedClient()); await f.ready(); f.remove(true); await f.client.refresh(f.native, storage);
  expect(f.client.threadId).toBe(''); expect(f.client.thread).toBeNull(); expect(f.client.error).toBe('');
  expect(mobileCacheThreadDeleted(f.client, 'adoption-env', 'thread')).toBe(false);
  await mobileCacheFlushDeletes(f.client, f.native); expect(f.calls.filter(row => row.op === 'mobileClientCache')).toEqual([]);
});
test('end-of-refresh observation alone misses deletion erased by a later shell event', async () => {
  const f = fixture(new T3Client()); await f.ready(); f.remove(); await f.client.refresh(f.native, storage);
  mobileCacheObserveThread(f.client);
  expect(f.client.thread).toBeNull(); expect(f.client.threadId).toBe(''); expect(f.client.error).toBe('');
  expect(mobileCacheThreadDeleted(f.client, 'adoption-env', 'thread')).toBe(false);
});
test('prepared observer is idempotent and preserves the exact assigned public thread value', async () => {
  const f = fixture(observedClient()); await f.ready();
  const thread = f.client.thread; mobileCacheObserveAdoption(f.client);
  expect(f.client.thread).toBe(thread);
  expect(Object.getOwnPropertyDescriptor(f.client, 'thread')?.enumerable).toBe(true);
  expect(Object.getOwnPropertyDescriptor(f.client, 'thread')?.configurable).toBe(true);
  f.client.thread = null; expect(f.client.thread).toBeNull();
});
test('successful cleanup deduplicates the same adopted projection; failed cleanup remains retryable', async () => {
  const client = observedClient(); client.connection = 'connected'; client.environmentId = 'adoption-env'; client.threadId = 'deleted';
  const projection = { thread: { id: 'deleted', deletedAt: '2026-10-08T10:00:00Z' } };
  const thread = { projection, sequence: 2, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  client.thread = thread; let fail = true, calls = 0;
  const native: Native = { available: true, watch() {}, async later() {
    calls++; return fail ? { ok: false, generation: 0, error: { kind: 'Disk', message: 'Disk failed' } }
      : { ok: true, generation: 0, value: { removed: 1 } };
  } };
  await mobileCacheFlushDeletes(client, native); expect(mobileCacheThreadDeleted(client, 'adoption-env', 'deleted')).toBe(true);
  mobileCacheObserveThread(client); client.thread = { ...thread };
  expect(mobileCacheThreadDeleted(client, 'adoption-env', 'deleted')).toBe(true);
  fail = false; await mobileCacheFlushDeletes(client, native);
  expect(mobileCacheThreadDeleted(client, 'adoption-env', 'deleted')).toBe(false); expect(calls).toBe(2);
  client.thread = { ...thread }; mobileCacheObserveThread(client); await mobileCacheFlushDeletes(client, native);
  expect(calls).toBe(2);
  // A different adopted projection is new evidence; it may have arrived after a new cache producer.
  client.thread = { ...thread, projection: { ...projection } }; await mobileCacheFlushDeletes(client, native);
  expect(calls).toBe(3);
});
