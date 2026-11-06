// A sent draft that is still starting its thread (MIT reference, see LICENSE-T3, T3 Code 74541bab17
// components/ChatView.tsx onSend and composerDraftStore.ts restoreFailedDraftThread): the send marks the
// draft promoted at once ("Keep new-thread flows from reusing this draft while attachments and worktree
// setup settle"), so the new-thread shortcut opens a fresh draft while the launch, its worktree among it,
// is still in flight. A launch that then succeeds stays out of view; one that fails restores the draft
// and says "Could not start thread", with Open draft.
//
// Here a draft's launch is the window's pending `orchestration.launchThread` write, and the project's
// new-thread composer is its one draft. When a new-thread flow leaves the draft while that write is in
// flight, the launch is detached: it moves from the window's one pending write to `local.launches`, so
// the fresh draft can send; the composer loses the sent text and images (kept on the launch), and the
// draft's reserved thread id is let go. The write's reply, which reaches the send's superseded command,
// is settled here (client.ts `write`); a reply let go meanwhile leaves the outcome to the shell, as a
// relaunch does (client.ts `reconcilePending`).
import type { T3Client, Pending } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { pushToast } from '../shell/toast';
import { forgetDraftThreadId } from './draft-thread-id';
import { fleetThreadId } from '../connections/settings-b-fleet';

/** The ops that open another draft or thread while a launch is in flight (app.contract newThread's `draftLeft`). */
const LEAVING = ['new-thread', 'select-project', 'select-thread'];

/** The new-thread composer a launch was sent from. */
export const launchKey = (environmentId: string, pending: Pending): string => `${environmentId}:new:${str(pending.payload.projectId)}`;

/** A detached launch's key in `local.launches`: its environment and the thread it launches. */
export const launchId = (environmentId: string, pending: Pending): string => `${environmentId}\n${str(pending.payload.threadId)}`;

/** A launch from a draft (not an unstarted server thread's first message, which keeps its own composer). */
export function isDraftLaunch(pending: Pending | undefined): pending is Pending {
  return !!pending && pending.method === 'orchestration.launchThread' && pending.payload.reuseExistingThread !== true;
}

/** Whether the window's draft is the one whose launch is in flight: the new-thread shortcut may then open a fresh one. */
export function draftLaunching(client: T3Client): boolean {
  const pending = client.pending;
  return client.busy && isDraftLaunch(pending) && !pending.uncertain && !pending.detached && launchKey(client.environmentId, pending) === client.draftKey;
}

/**
 * markPromotedDraftThreadByRef at the moment a new-thread flow leaves the sending draft: the composer keeps none of the
 * sent content and the next send reserves a new thread id. Returns whether it detached (the write's reply is then
 * settled, not lost, so the launch is not uncertain).
 */
export function detachDraftLaunch(client: T3Client, op: string): boolean {
  const pending = client.pending;
  if (!LEAVING.includes(op) || !isDraftLaunch(pending) || pending.uncertain || pending.detached) return false;
  const key = launchKey(client.environmentId, pending);
  const sent = new Set(arr(obj(pending.payload.initialMessage).attachments).map(attachment => str(attachment.id)));
  const images = client.local.snapshotDrafts[key] ?? [];
  pending.detached = { images: images.filter(image => sent.has(str(image.uploadId))) };
  if (pending.detached.images.length) client.local.snapshotDrafts[key] = images.filter(image => !sent.has(str(image.uploadId)));
  if (pending.text && client.local.drafts[key] === pending.text) delete client.local.drafts[key];
  forgetDraftThreadId(client, key);
  // Out of the window's one pending write: the fresh draft can send, and no other write can replace this one.
  (client.local.launches ??= {})[launchId(client.environmentId, pending)] = pending;
  delete client.local.pending[client.environmentId];
  return true;
}

/**
 * The detached launch failed: its text and images return to the project's draft (after what was typed there since, as a
 * displaced answer does), and the toast offers that draft (restoreFailedDraftThread, "Could not start thread").
 */
export function restoreDetachedLaunch(client: T3Client, environmentId: string, pending: Pending, message: string): void {
  const key = launchKey(environmentId, pending), typed = client.local.drafts[key] ?? '';
  if (pending.text && typed !== pending.text) client.local.drafts[key] = typed.trim() ? `${typed.trimEnd()}\n\n${pending.text}` : pending.text;
  const images: Obj[] = pending.detached?.images ?? [];
  if (images.length) client.local.snapshotDrafts[key] = [...(client.local.snapshotDrafts[key] ?? []), ...images];
  pushToast(client, { kind: 'error', title: 'Could not start thread', description: message || 'Failed to send message.', stacked: true,
    action: { label: 'Open draft', op: 'new-thread', id: fleetThreadId(environmentId, str(pending.payload.projectId)) } });
}
