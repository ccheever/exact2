import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileCacheObserveThread, mobileCacheThreadDeleted, mobileCacheFlushDeletes } from './mobile-client-cache-lifecycle';
import { mobileCacheReadRevision } from './mobile-client-cache';
function fixture() {
  const client = new T3Client(); client.environmentId = 'cache-delete-env'; client.threadId = 'thread-1'; client.connection = 'connected';
  client.thread = { projection: { thread: { id: client.threadId, deletedAt: '2026-10-08T10:00:00Z' } }, sequence: 10,
    historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  return client;
}
test('adopted deletion survives selection change until exact thread cleanup completes', async () => {
  const client = fixture(), requests: Obj[] = []; mobileCacheObserveThread(client);
  expect(mobileCacheThreadDeleted(client, 'cache-delete-env', 'thread-1')).toBe(true);
  client.environmentId = 'env-other'; client.threadId = 'thread-other'; client.thread = null;
  const oldRevision = mobileCacheReadRevision('cache-delete-env');
  const native: Native = { available: true, watch() {}, async later(input) { requests.push(obj(input)); return { ok: true, generation: 0, value: { removed: 1 } }; } };
  await mobileCacheFlushDeletes(client, native);
  expect(requests).toEqual([{ op: 'mobileClientCache', action: 'remove', environmentId: 'cache-delete-env', kind: 'thread', key: 'thread-1' }]);
  expect(mobileCacheThreadDeleted(client, 'cache-delete-env', 'thread-1')).toBe(false);
  expect(mobileCacheReadRevision('cache-delete-env')).not.toBe(oldRevision);
});
test('missing/archived/mismatched/offline rows do not imply server deletion', () => {
  for (const change of ['archive', 'missing', 'mismatch', 'offline']) {
    const client = fixture();
    if (change === 'archive') client.thread!.projection.thread = { id: client.threadId, archivedAt: 'today', deletedAt: null };
    if (change === 'missing') client.thread = null;
    if (change === 'mismatch') client.threadId = 'another';
    if (change === 'offline') client.connection = 'disconnected';
    mobileCacheObserveThread(client);
    expect(mobileCacheThreadDeleted(client, 'cache-delete-env', 'thread-1')).toBe(false);
  }
});
test('failed cleanup retains a local read/write block until later success', async () => {
  const client = fixture(); mobileCacheObserveThread(client); let failed = true;
  const native: Native = { available: true, watch() {}, async later() { return failed
    ? { ok: false, generation: 0, error: { kind: 'Disk', message: 'Write failed' } }
    : { ok: true, generation: 0, value: { removed: 0 } }; } };
  await mobileCacheFlushDeletes(client, native);
  expect(mobileCacheThreadDeleted(client, 'cache-delete-env', 'thread-1')).toBe(true);
  failed = false; await mobileCacheFlushDeletes(client, native);
  expect(mobileCacheThreadDeleted(client, 'cache-delete-env', 'thread-1')).toBe(false);
});
test('abandoned cleanup propagates with no subsequent request and retains recovery identity', async () => {
  const client = fixture(); mobileCacheObserveThread(client);
  client.threadId = 'thread-2'; client.thread!.projection.thread = { id: client.threadId, deletedAt: 'today' }; mobileCacheObserveThread(client);
  let calls = 0;
  const native: Native = { available: true, watch() {}, async later() { calls++; throw { name: 'FetchError', kind: 'Aborted' }; } };
  await expect(mobileCacheFlushDeletes(client, native)).rejects.toMatchObject({ kind: 'superseded' });
  expect(calls).toBe(1); expect(mobileCacheThreadDeleted(client, 'cache-delete-env', 'thread-1')).toBe(true);
  expect(mobileCacheThreadDeleted(client, 'cache-delete-env', 'thread-2')).toBe(true);
});
