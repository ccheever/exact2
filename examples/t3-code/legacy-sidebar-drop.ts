// Files dropped on a legacy sidebar row (T3 Code 1e2ecbd975; MIT, see LICENSE-T3): LegacySidebar.tsx
// SidebarThreadRow's makeWorkspaceFileDropHandlers and handleThreadFileDrop, sidebarPendingFileDropStore.ts,
// ChatView.tsx's consumePendingFileDrop effect and ChatComposer addDroppedFiles. The row's native drop target
// (T3RowDrop.swift, the `t3-row-drop` hatch) rings the row while files are dragged over it, queues the dropped
// files under the row's thread key and presses the row, which opens the thread. Once that thread is the open
// one, each refresh takes its queued files (oldest first) and adds them as the composer's Attach does:
// images to the shelf, other files as chips, refusals named in one error.
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
/** consumePendingFileDrop for the open thread, when the module reports a queued drop for it. */
export async function takeRowDrops(client: T3Client, native: Native): Promise<boolean> {
  const listed = obj(client.presentation).rowDrops, queued = Array.isArray(listed) ? listed.map(entry => str(entry)) : [];
  const keys = openThreadDropKeys(client);
  if (!keys.some(key => queued.includes(key))) return false;
  try {
    const reply = obj(await client.restAccess(native).call({ op: 'rowDropTake', keys, fileLimit: attachStagingLimit(client) }));
    const files = arr(reply.files);
    if (!files.length) return false;
    await addAttachedFiles(client, native, files, undefined, client.snapshotOwner);
    return true;
  } catch (error) { if (letGo(error)) throw error; return false; }
}
