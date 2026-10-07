// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/timeline-item-fetch.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// On-demand work-log detail, adapted from T3 Code 1e2ecbd975's turnItem atom family.
// Exact adaptation: per-client keyed cache over the existing authenticated RPC seam.
import { composerNow } from './composer-controls';
import { wakeShell } from './r10-connect-timing';
import { arr, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { turnItemDetailRevision, turnItemNeedsDetailFetch, turnItemOutputText } from './timeline-item-detail';
import { letGo } from './let-go';

const TTL = 60_000;
interface Detail { item?: Obj | null; error: string; pending?: Promise<void>; touched: number; fetched: number }
interface OpenItem { environment: string; thread: string; id: string; sourceThread: string }
interface Cache { connection: string; details: Map<string, Detail>; open: Map<string, OpenItem> }
const caches = new WeakMap<T3Client, Cache>();
function cache(client: T3Client): Cache {
  let value = caches.get(client);
  const connection = JSON.stringify([client.generation, client.origin]);
  if (!value || value.connection !== connection) { value = { connection, details: new Map(), open: value?.open ?? new Map() }; caches.set(client, value); }
  return value;
}
const identity = (environment: string, thread: string, id: string) => JSON.stringify([environment, thread, id]);
function rowFor(client: T3Client, id: string): Obj | undefined {
  return arr(client.projection.visibleTurnItems).find(row => obj(row.item).id === id || JSON.stringify([row.sourceThreadId, row.sourceItemId]) === id);
}
function keyFor(client: T3Client, item: Obj, rowId = str(item.id)): string {
  const row = rowFor(client, rowId);
  return identity(client.environmentId, str(row?.sourceThreadId) || client.threadId, str(row?.sourceItemId) || str(item.id));
}
function prune(value: Cache, now: number): void {
  for (const [key, detail] of value.details) if (!detail.pending && now - detail.touched >= TTL) value.details.delete(key);
}

/** Disclosure settles immediately; the root read mutation owns all native work. */
export function setTurnItemOpen(client: T3Client, id: string, open: boolean): void {
  const value = cache(client), scope = identity(client.environmentId, client.threadId, id);
  if (!open) { value.open.delete(scope); return; }
  const row = rowFor(client, id);
  if (!row) return;
  value.open.set(scope, { environment: client.environmentId, thread: client.threadId, id, sourceThread: str(row.sourceThreadId) || client.threadId });
}
async function fetchDetail(client: T3Client, native: Native, item: Obj, now: number, rowId: string): Promise<void> {
  if (!turnItemNeedsDetailFetch(item)) return;
  const value = cache(client); prune(value, now);
  const base = keyFor(client, item, rowId), revision = turnItemDetailRevision(item), key = `${base}\u0000${revision}`;
  const found = value.details.get(key);
  if (found) {
    found.touched = now;
    if (found.pending) return found.pending;
    if (now - found.fetched < TTL) return;
  }
  const detail: Detail = { item: found?.item, error: '', touched: now, fetched: now };
  value.details.set(key, detail);
  const row = rowFor(client, rowId);
  const itemId = str(row?.sourceItemId) || str(item.id);
  const threadId = str(row?.sourceThreadId) || client.threadId;
  detail.pending = (async () => {
    try {
      const reply = await client.rpc(native, 'orchestration.getTurnItem', { threadId, itemId, revision });
      // The captured key owns this reply even after navigation or a newer revision request.
      if (reply.item === null) detail.item = null;
      else if (typeof reply.item === 'object' && !Array.isArray(reply.item) && reply.item && str(obj(reply.item).id) === itemId && obj(reply.item).type === item.type) detail.item = obj(reply.item);
      else throw new Error('The server returned an invalid turn item.');
    } catch (error) {
      // A let-go read is asked again, never shown as an error (let-go.ts).
      if (letGo(error)) { if (value.details.get(key) === detail) value.details.delete(key); return; }
      detail.error = error instanceof Error ? error.message : String(error);
    }
    finally { detail.pending = undefined; detail.fetched = composerNow(client); await wakeShell(native); }
  })();
  await wakeShell(native);
  return detail.pending;
}

/** Each independent root read slot claims one eligible row before awaiting it. */
export async function refreshNextOpenTurnItemDetail(client: T3Client, native: Native, now = composerNow(client)): Promise<boolean> {
  const value = cache(client); prune(value, now);
  for (const [scope, open] of value.open) {
    if (open.environment !== client.environmentId || open.thread !== client.threadId) { value.open.delete(scope); continue; }
    const row = rowFor(client, open.id), item = obj(row?.item);
    if (!row) { value.open.delete(scope); continue; }
    if (!turnItemNeedsDetailFetch(item)) continue;
    const found = value.details.get(`${keyFor(client, item, open.id)}\u0000${turnItemDetailRevision(item)}`);
    if (found && (found.pending || now - found.fetched < TTL)) continue;
    // fetchDetail installs pending synchronously, so another root slot skips
    // this row. Its native request stays owned by this invocation until done.
    await fetchDetail(client, native, item, now, open.id);
    return true;
  }
  return false;
}

export interface TurnItemDetailView { item: Obj; text: string; state: '' | 'loading' | 'error' | 'empty' | 'missing' }
/** Preserve loaded content while a new revision is being fetched. */
export function turnItemDetailView(client: T3Client, item: Obj, now = composerNow(client), rowId = str(item.id)): TurnItemDetailView {
  if (!turnItemNeedsDetailFetch(item)) return { item, text: turnItemOutputText(item) ?? '', state: '' };
  const value = cache(client), base = keyFor(client, item, rowId), key = `${base}\u0000${turnItemDetailRevision(item)}`;
  const current = value.details.get(key);
  if (current) current.touched = now;
  const previous = [...value.details].reverse().find(([candidate, detail]) => candidate.startsWith(`${base}\u0000`) && detail.item)?.[1];
  const loaded = current?.item ?? (current?.item === null ? null : previous?.item);
    if (loaded) {
    const text = turnItemOutputText(loaded);
    return { item: loaded, text: text ?? 'No output.', state: text ? '' : 'empty' };
  }
  if (current?.error) return { item, text: `Couldn't load output: ${current.error}`, state: 'error' };
  if (current?.item === null) return { item, text: "Couldn't load output: Output is no longer available.", state: 'missing' };
  return { item, text: 'Loading output…', state: 'loading' };
}

/** Pure readiness for the root's separately awaited read mutation. */
export function turnItemDetailsNeeded(client: T3Client, now = composerNow(client)): boolean {
  const value = cache(client);
  for (const open of value.open.values()) {
    if (open.environment !== client.environmentId || open.thread !== client.threadId) continue;
    const row = rowFor(client, open.id), item = obj(row?.item);
    if (!row || !turnItemNeedsDetailFetch(item)) continue;
    const found = value.details.get(`${keyFor(client, item, open.id)}\u0000${turnItemDetailRevision(item)}`);
    if (!found || !found.pending && now - found.fetched >= TTL) return true;
  }
  return false;
}

/** Disclosure state lives with the keyed read so pending output can publish immediately. */
export function turnItemIsOpen(client: T3Client, id: string): boolean {
  return cache(client).open.has(identity(client.environmentId, client.threadId, id));
}
