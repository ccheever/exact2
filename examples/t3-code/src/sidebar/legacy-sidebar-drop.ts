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
import { arr, obj, str } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { IMAGE_ATTACHMENT_MAX_EDGE } from '../composer/image-attachment-size';
import { attachStagingLimit, releaseStagedFile } from '../composer/composer-editor-attach';
import { addAttachedFiles } from '../composer/composer-controls-attach';
import { fleetThreadId } from '../connections/settings-b-fleet';

/** Rows keep their environment-qualified drop key when that environment becomes focused. */
export function openThreadDropKeys(client: T3Client): string[] {
  return client.threadId && client.environmentId ? [fleetThreadId(client.environmentId, client.threadId)] : [];
}
const queuedKeys = (client: T3Client): string[] => { const listed = obj(client.presentation).rowDrops; return Array.isArray(listed) ? listed.map(entry => str(entry)) : []; };

/** clearPendingFileDrop: the queued drops under these keys go (a no-op for keys with none). */
export async function clearRowDrops(client: T3Client, native: Native, keys: string[]): Promise<void> {
  const going = [...new Set(keys)].filter(Boolean); // The failed press can precede the status snapshot of its queue.
  if (!going.length) return;
  try { await client.restAccess(native).call({ op: 'rowDropClear', keys: going }); } catch (error) { if (letGo(error)) throw error; }
}

/** The open thread and the keys queued at the last refresh, per client: a later refresh that finds another thread open lets those go. */
const seen = new WeakMap<T3Client, { open: string; keys: string[] }>();
/** consumePendingFileDrop for the open thread, when the module reports a queued drop for it. */
export async function takeRowDrops(client: T3Client, native: Native): Promise<boolean> {
  if (!client.ready || !client.threadId) return false;
  const queued = queuedKeys(client), keys = openThreadDropKeys(client), open = `${client.environmentId}\n${client.threadId}`, owner = client.snapshotOwner;
  // Only a thread that is open counts as where a navigation landed (not a draft, nor an environment switch underway).
  if (client.ready && client.threadId) {
    const last = seen.get(client);
    const stale = last && last.open !== open ? queued.filter(key => last.keys.includes(key) && !keys.includes(key)) : [];
    // Keep the destination while clearing: an interrupted navigation must still discard its queue.
    seen.set(client, { open, keys: queued.filter(key => !stale.includes(key)) });
    if (stale.length) await clearRowDrops(client, native, stale);
  }
  if (!client.ready || owner !== client.snapshotOwner || !keys.some(key => queued.includes(key))) return false;
  try {
    const reply = obj(await client.restAccess(native).call({ op: 'rowDropTake', keys, fileLimit: attachStagingLimit(client), imageMaxEdge: IMAGE_ATTACHMENT_MAX_EDGE }));
    const files = arr(reply.files);
    if (!files.length) return false;
    if (!client.ready || owner !== client.snapshotOwner) {
      for (const file of files) {
        if (file.kind === 'image') client.local.snapshotReleases.push(str(file.id));
        await releaseStagedFile(native, file);
      }
      return false;
    }
    await addAttachedFiles(client, native, files, undefined, owner);
    return true;
  } catch (error) { if (letGo(error)) throw error; return false; }
}
