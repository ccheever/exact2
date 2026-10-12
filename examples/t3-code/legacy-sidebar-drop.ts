// Files dropped on a legacy sidebar row (T3 Code 1e2ecbd975; MIT, see LICENSE-T3): LegacySidebar.tsx
// SidebarThreadRow's makeWorkspaceFileDropHandlers and handleThreadFileDrop, sidebarPendingFileDropStore.ts,
// ChatView.tsx's consumePendingFileDrop effect and ChatComposer addDroppedFiles. The row's native drop target
// (T3RowDrop.swift, the `t3-row-drop` hatch) rings the row while files are dragged over it and queues the dropped
// files under the row's thread key; a row that is not the open thread is pressed, which opens the thread (the open
// thread's own row only queues). Once that thread is the open one, each refresh takes its queued files (oldest
// first) and adds them as the composer's Attach does: images to the shelf, other files as chips, refusals named in
// one error. handleThreadFileDrop lets a drop go when its navigation fails or lands elsewhere: here a failed open
// clears the pressed row's drops (client-ops-threads.ts), and a thread change that lands on another thread clears
// every drop queued before it for a thread that is still not open (its press found the app busy, or another thread
// opened first).
import { arr, obj, str } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { attachStagingLimit } from './composer-editor-attach';
import { addAttachedFiles } from './composer-controls-attach';
import { fleetThreadId } from './settings-b-fleet';

/** The keys a drop for the open thread was queued under: its own id, and the `fleet:` key it had as a background row. */
export function openThreadDropKeys(client: T3Client): string[] {
  return client.threadId && client.environmentId ? [client.threadId, fleetThreadId(client.environmentId, client.threadId)] : [];
}
const queuedKeys = (client: T3Client): string[] => { const listed = obj(client.presentation).rowDrops; return Array.isArray(listed) ? listed.map(entry => str(entry)) : []; };

/** clearPendingFileDrop: the queued drops under these keys go (a no-op for keys with none). */
export async function clearRowDrops(client: T3Client, native: Native, keys: string[]): Promise<void> {
  const queued = queuedKeys(client), going = [...new Set(keys)].filter(key => key && queued.includes(key));
  if (!going.length) return;
  try { await client.restAccess(native).call({ op: 'rowDropClear', keys: going }); } catch (error) { if (letGo(error)) throw error; }
}

/** The open thread and the keys queued at the last refresh, per client: a later refresh that finds another thread open lets those go. */
const seen = new WeakMap<T3Client, { open: string; keys: string[] }>();
/** consumePendingFileDrop for the open thread, when the module reports a queued drop for it. */
export async function takeRowDrops(client: T3Client, native: Native): Promise<boolean> {
  const queued = queuedKeys(client), keys = openThreadDropKeys(client), open = `${client.environmentId}\n${client.threadId}`;
  // Only a thread that is open counts as where a navigation landed (not a draft, nor an environment switch underway).
  if (client.ready && client.threadId) {
    const last = seen.get(client);
    const stale = last && last.open !== open ? queued.filter(key => last.keys.includes(key) && !keys.includes(key)) : [];
    seen.set(client, { open, keys: queued.filter(key => !stale.includes(key) && !keys.includes(key)) });
    if (stale.length) await clearRowDrops(client, native, stale);
  }
  if (!keys.some(key => queued.includes(key))) return false;
  try {
    const reply = obj(await client.restAccess(native).call({ op: 'rowDropTake', keys, fileLimit: attachStagingLimit(client) }));
    const files = arr(reply.files);
    if (!files.length) return false;
    await addAttachedFiles(client, native, files, undefined, client.snapshotOwner);
    return true;
  } catch (error) { if (letGo(error)) throw error; return false; }
}
