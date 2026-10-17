// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { letGo } from './shared/let-go';
import type { Native } from './shared/protocol';
import { mobileCacheRemove } from './mobile-client-cache';

interface Deletion { environmentId: string; threadId: string }
const deletions = new WeakMap<T3Client, Map<string, Deletion>>();
const observers = new WeakSet<T3Client>();
// Projection identity is weak: a failed disk removal must not retain an entire
// deleted transcript. The pending map below needs only its two cache key fields.
const observed = new WeakMap<T3Client, WeakMap<object, string>>();
const key = (environmentId: string, threadId: string) => JSON.stringify([environmentId, threadId]);
/** Called only after the shared reducer adopts a projection. Merely losing a row
 * from the active shell is not deletion: the thread could have been archived. */
export function mobileCacheObserveThread(client: T3Client): void {
  const projection = client.thread?.projection, thread = obj(projection?.thread);
  if (client.connection !== 'connected' || !client.environmentId || !client.threadId
    || !projection || thread.id !== client.threadId || !str(thread.deletedAt)) return;
  const id = key(client.environmentId, client.threadId);
  if (observed.get(client)?.get(projection) === id) return;
  let pending = deletions.get(client);
  if (!pending) { pending = new Map(); deletions.set(client, pending); }
  if (!pending.has(id)) pending.set(id, { environmentId: client.environmentId, threadId: client.threadId });
  let seen = observed.get(client);
  if (!seen) { seen = new WeakMap(); observed.set(client, seen); }
  // Observation suppresses duplicate admission only. Failed removal remains in
  // pending independently and is retried by each later awaited flush.
  seen.set(projection, id);
}
/** Prepared app-only observation of the shared client's public data property.
 * The selection-update method is private, so it cannot be overridden by the
 * mobile subtype. Observe the actual assignment synchronously, before the next
 * event may clear selection. This remains the client's single thread value.
 * No transport, projection copy, native handle or asynchronous work lives here. */
export function mobileCacheObserveAdoption(client: T3Client): void {
  if (observers.has(client)) return;
  let current = client.thread;
  Object.defineProperty(client, 'thread', {
    configurable: true, enumerable: true,
    get: () => current,
    set: (value: T3Client['thread']) => { current = value; mobileCacheObserveThread(client); },
  });
  observers.add(client);
  mobileCacheObserveThread(client);
}
export function mobileCacheThreadDeleted(client: T3Client, environmentId: string, threadId: string): boolean {
  return deletions.get(client)?.has(key(environmentId, threadId)) === true;
}
/** The answer owns every awaited cleanup. Keep unsuccessful deletions blocked
 * until a later answer can remove them; no promise or native handle is retained. */
export async function mobileCacheFlushDeletes(client: T3Client, native: Native): Promise<void> {
  const pending = deletions.get(client);
  if (!pending) return;
  for (const [id, deletion] of [...pending]) {
    try {
      await mobileCacheRemove(native, { environmentId: deletion.environmentId, kind: 'thread', key: deletion.threadId });
      if (pending.get(id) === deletion) pending.delete(id);
    } catch (error) {
      if (letGo(error)) throw error;
      // Removal failure must not turn a valid live refresh into a failure.
    }
  }
}
