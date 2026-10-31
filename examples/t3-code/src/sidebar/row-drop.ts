// Files dropped on a sidebar row (T3 Code 6d450a4284; MIT, see LICENSE-T3): SidebarThreadRow's
// makeWorkspaceFileDropHandlers and handleThreadFileDrop (Sidebar.tsx), sidebarPendingFileDropStore.ts,
// ChatView.tsx's consumePendingFileDrop effect and ChatComposer addDroppedFiles. The row's `drop` (sidebar-row.contract
// ThreadRow, which rings the row between `dragenter` and `dragleave`) queues the dropped files under the row's thread
// key; a row that is not the open thread's then opens its thread (the open thread's own row only queues). A drop of
// folders alone queues and opens nothing (the row's addFolders does nothing). Once that thread is the open one, each
// refresh takes its queued files (oldest first) and adds them as the composer's Attach does: images to the shelf, other
// files as chips, refusals named in one error. handleThreadFileDrop lets a drop go when its navigation fails or lands
// elsewhere: here a failed open clears the row's drops (client-ops-threads.ts), and a thread change that lands on
// another thread clears every drop queued before it for a thread that is still not open (its open found the app busy,
// or another thread opened first).
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { addAttachedFiles, questionRefuses, releaseStaged, stageFiles, type IncomingFiles } from '../composer/composer-controls-attach';
import { handleSource, statSource } from '../composer/composer-attach-stage';
import { fleetThreadId, parseFleetThreadId } from '../connections/settings-b-fleet';

/** Rows keep their environment-qualified drop key when that environment becomes focused. */
function openThreadDropKeys(client: T3Client): string[] {
  return client.threadId && client.environmentId ? [fleetThreadId(client.environmentId, client.threadId)] : [];
}

/** sidebarPendingFileDropStore: drops queue up (a second drop before the thread opens keeps both), oldest first. */
const queues = new WeakMap<T3Client, { key: string; files: string[] }[]>();
const queueOf = (client: T3Client) => { let queue = queues.get(client); if (!queue) { queue = []; queues.set(client, queue); } return queue; };
/** The keys with a queued drop, oldest first. */
export const rowDropKeys = (client: T3Client): string[] => queueOf(client).map(entry => entry.key);

/**
 * queuePendingFileDrop for a row's drop (`target`: "<1 when the row is the open thread's>\t<thread id>\t<drop key>";
 * with no key, the thread id's on the focused environment). The answer "open:<thread id>" opens a row that is not the
 * open thread's (app.contract filesAddedDone).
 */
export async function queueRowDrop(client: T3Client, target: string, incoming: IncomingFiles): Promise<string> {
  const [open = '', threadId = '', given = ''] = target.split('\t');
  const key = given || (threadId ? (parseFleetThreadId(threadId) ? threadId : fleetThreadId(client.environmentId, threadId)) : '');
  if (!key || !incoming.files.length) return '';
  const files: string[] = [];
  for (const path of incoming.files) {
    const stat = await statSource(handleSource(path));
    if (stat && !stat.folder) files.push(path); // splitDroppedItems: the files of a drop, without its folders
  }
  if (!files.length) return '';
  queueOf(client).push({ key, files });
  return open === '1' ? '' : `open:${threadId}`;
}

/** clearPendingFileDrop: the queued drops under these keys go (a no-op for keys with none). */
export async function clearRowDrops(client: T3Client, _native: Native, keys: string[]): Promise<void> {
  const going = new Set(keys.filter(Boolean));
  if (!going.size) return;
  queues.set(client, queueOf(client).filter(entry => !going.has(entry.key)));
}

/** The open thread and the keys queued at the last refresh, per client: a later refresh that finds another thread open lets those go. */
const seen = new WeakMap<T3Client, { open: string; keys: string[] }>();
/** consumePendingFileDrop for the open thread, when a drop is queued for it. */
export async function takeRowDrops(client: T3Client, native: Native): Promise<boolean> {
  if (!client.ready || !client.threadId) return false;
  const queued = rowDropKeys(client), keys = openThreadDropKeys(client), open = `${client.environmentId}\n${client.threadId}`, owner = client.snapshotOwner;
  // Only a thread that is open counts as where a navigation landed (not a draft, nor an environment switch underway).
  const last = seen.get(client);
  const stale = last && last.open !== open ? queued.filter(key => last.keys.includes(key) && !keys.includes(key)) : [];
  // Keep the destination while clearing: an interrupted navigation must still discard its queue.
  seen.set(client, { open, keys: queued.filter(key => !stale.includes(key)) });
  if (stale.length) await clearRowDrops(client, native, stale);
  const matched = queueOf(client).filter(entry => keys.includes(entry.key));
  if (!matched.length) return false;
  queues.set(client, queueOf(client).filter(entry => !matched.includes(entry)));
  // consumePendingFileDrop takes the drop, and addDroppedFiles refuses it as any attachment is refused.
  if (questionRefuses(client)) return false;
  try {
    const files = await stageFiles(client, native, matched.flatMap(entry => entry.files).map(handleSource));
    if (!files.length) return false;
    if (!client.ready || owner !== client.snapshotOwner) { await releaseStaged(client, native, files); return false; }
    await addAttachedFiles(client, native, files, undefined, owner);
    return true;
  } catch (error) { if (letGo(error)) throw error; return false; }
}
