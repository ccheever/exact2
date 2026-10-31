// startThreadRename for a row's double-click (Sidebar.tsx onDoubleClick, T3 Code 1e2ecbd975;
// MIT, see LICENSE-T3). The reference's first click navigates in the same task, so the row is there when the
// double-click renames it, and its title input's autoFocus comes after ChatView's composer focus. Here the first
// click's open of another environment's row moves the window to that environment, and that environment's shell, with
// the row, comes in on a later refresh. A rename asked before then is held and starts on the refresh that brings the
// row (sidebarRefreshed), so its field mounts, and takes the focus, after everything the open does.
import { str } from '../client/domain';
import type { T3Client } from '../client/client';
import { fleet, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { rowIdNow, rowThread } from './row-thread';
import { sidebarSession } from './sidebar-state';

interface Held { environmentId: string; threadId: string }
const held = new WeakMap<T3Client, Held>();

/** Whether a double-click's rename waits for its row (app.contract keeps the open's composer focus from the prompt). */
export function renameHeld(client: T3Client): boolean { return held.has(client); }

/**
 * The rename of row `id` (`fleet:<environment>:<id>` for a background environment's). It starts now when the row is
 * known; it waits when the window is moving to the row's environment and that environment's shell is not in yet.
 */
export function startRename(client: T3Client, id: string, source: EnvironmentFleet = fleet): void {
  const rowId = rowIdNow(client, id), session = sidebarSession(client), thread = rowThread(client, rowId, source)?.thread;
  held.delete(client);
  if (thread) {
    if (session.renameId !== rowId) { session.renameId = rowId; session.renameTitle = str(thread.title); }
    return;
  }
  const ref = parseFleetThreadId(rowId);
  // Still a `fleet:` id: the open's status has not named the new environment yet, and the fleet let go of it (refocus).
  const moving = ref ? !connected(source, ref.environmentId) : !client.shellLoaded;
  if (moving) held.set(client, ref ?? { environmentId: client.environmentId, threadId: rowId });
}

/** After each refresh (sidebarRefreshed): a held rename starts once its environment's shell is in, or goes. */
export function startHeldRename(client: T3Client, source: EnvironmentFleet = fleet): void {
  const wait = held.get(client);
  if (!wait) return;
  if (client.environmentId !== wait.environmentId) {
    // The row's environment is a background one again: the window is not going there.
    if (connected(source, wait.environmentId)) held.delete(client);
    return;
  }
  if (!client.shellLoaded) return;
  held.delete(client);
  const thread = client.shell.threads.find(entry => entry.id === wait.threadId), session = sidebarSession(client);
  if (thread) { session.renameId = wait.threadId; session.renameTitle = str(thread.title); }
}

/** A press on another row, or Escape, drops a held rename (as it drops a started one). */
export function dropHeldRename(client: T3Client, id = ''): void {
  const wait = held.get(client);
  if (!wait) return;
  const ref = parseFleetThreadId(id);
  const same = (ref ? ref.threadId : id) === wait.threadId && (!ref || ref.environmentId === wait.environmentId);
  if (!same) held.delete(client);
}

const connected = (source: EnvironmentFleet, environmentId: string) =>
  [...source.entries.values()].some(entry => entry.environmentId === environmentId && entry.phase === 'connected');
