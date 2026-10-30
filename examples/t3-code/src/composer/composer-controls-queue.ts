// The thread's queued messages and the queued-message edit,
// adapted from T3 Code (MIT); see LICENSE-T3. Sources at 7c3b3b3605ef, and chat/QueuedRunsControl.tsx at v0.1.5,
// packages/client-runtime/src/state/threadWorkflows.ts (deriveThreadQueueWorkflowState,
// getUserQueuedThreadRuns), packages/client-runtime/src/queuedRunEdit.ts (makeQueuedRunEditStore,
// queuedRunEditBlockedReason) and chat/useQueuedRunEdit.ts (beginEdit, cancelEdit, saveEdit).
import { applyShell, arr, initialShell, obj, str, type Obj } from '../client/domain';
import { ClientError, activeRun, type Files, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { editView, endLostEdit, removeExisting, restoreThreadDraft, saveEdit, startEdit, type QueuedEditing } from './queued-edit-attachments';
import { citationsToPlainText, replaceContextReferences } from './composer-editor-menu';
import { pendingRequests } from './requests';
import { wakeShell } from '../client/command-waits';

/** A queued message in full, as a sent turn reads: each context chip as its label, an image chip nothing when the row
 *  shows thumbnails, and each quote as its text. */
export function queuedPreviewText(text: string, thumbnails: boolean): string {
  return citationsToPlainText(replaceContextReferences(text, reference => reference.kind === 'image' && thumbnails ? '' : reference.label)).trim();
}

/** getUserQueuedThreadRuns in queue order; automatic completion/notification runs are not the user's queue. */
export function queueState(projection: Obj) {
  const messages = arr(projection.messages);
  const automatic = new Set(messages.filter(message => message.delegatedCompletion != null || message.notification != null).map(message => str(message.id)));
  const queued = arr(projection.runs).filter(run => run.status === 'queued' && !automatic.has(str(run.userMessageId)))
    .sort((a, b) => (Number(a.queuePosition ?? a.ordinal) - Number(b.queuePosition ?? b.ordinal)) || Number(a.ordinal) - Number(b.ordinal));
  const run = activeRun(projection);
  const thread = obj(projection.thread);
  const providerThreadId = str(run?.providerThreadId) || str(thread.activeProviderThreadId);
  const providerThread = arr(projection.providerThreads).find(entry => entry.id === providerThreadId)
    ?? arr(projection.providerThreads).find(entry => entry.appThreadId === thread.id && entry.providerSessionId);
  const session = providerThread ? arr(projection.providerSessions).find(entry => entry.id === providerThread.providerSessionId) : undefined;
  const turns = obj(obj(session?.capabilities).turns);
  const steerable = run?.status === 'running' && !!run.activeAttemptId
    && arr(projection.providerTurns).some(turn => turn.runAttemptId === run.activeAttemptId && turn.status === 'running');
  // isCompactionCommandMessage: the active run is a compaction, so sending a queued message stops it instead of steering.
  const activeMessage = run ? messages.find(message => message.id === run.userMessageId) : undefined;
  const promoteSkipsCompaction = !!activeMessage && activeMessage.role === 'user' && str(activeMessage.text).trim().toLowerCase() === '/compact'
    && arr(activeMessage.attachments).length === 0;
  return {
    queued: queued.map(entry => {
      const message = messages.find(candidate => candidate.id === entry.userMessageId), guest = obj(obj(message?.context).guest);
      const images = arr(message?.attachments).filter(attachment => attachment.type === 'image');
      // A queued guest message shows the guest's words; the owner can cancel it, not rewrite it.
      return { run: entry, message, text: typeof guest.text === 'string' ? guest.text : message ? str(message.text) : 'Queued message', messageId: str(entry.userMessageId),
        editable: obj(message?.context).guest === undefined, images: images.length, thumbnails: images.map(image => ({ id: str(image.id), name: str(image.name) })) };
    }),
    activeRunId: str(run?.id),
    // deriveThreadQueueWorkflowState: with no active run, a queued message starts one (queued-run.start); during a
    // compaction it stops the compaction; otherwise it steers a provider that can take one.
    canSendNow: !run && thread.archivedAt == null && thread.deletedAt == null,
    canSteer: promoteSkipsCompaction || (steerable && (turns.supportsActiveSteering === true || turns.supportsSteeringByInterruptRestart === true)),
    promoteSkipsCompaction,
  };
}

/** One queued message as the timeline's end lists it: the server's queue, then a queued send it has not acknowledged. */
export type QueuedItem = { key: string; runId: string; text: string; editable: boolean; thumbnails: Array<{ id: string; name: string }>;
  /** Its place in the server's queue (-1 for a send still saving). */
  serverIndex: number; pending: boolean;
  /** The message it will send, whose attribution it wears (QueuedMessage, resolveUserMessagePresentation); absent while saving. */
  message?: Obj };
export function queuedItems(client: T3Client): QueuedItem[] {
  if (!client.threadId) return [];
  const items: QueuedItem[] = queueState(client.projection).queued.map((entry, serverIndex) => ({ key: str(entry.run.id), runId: str(entry.run.id),
    text: queuedPreviewText(entry.text, entry.thumbnails.length > 0), editable: entry.editable, thumbnails: entry.thumbnails, serverIndex, pending: false,
    ...(entry.message ? { message: entry.message } : {}) }));
  const pending = pendingQueued(client);
  return pending ? [...items, pending] : items;
}

/** queuedRunEditBlockedReason: an open question takes the composer for its answer, so an edit would happen out of sight. */
export function queuedEditBlockedReason(client: T3Client): string {
  return client.threadId && pendingRequests(client.projection).inputs.length > 0 ? 'Answer the question to edit queued messages' : '';
}

// ── The edit store ─────────────────────────────────────────────────────────

// makeQueuedRunEditStore: one edit per thread (keyed as its draft), kept while the user is elsewhere. `saving` holds the
// threads with a save in flight: the claim outlives the edit, since the server orders saves by arrival and an
// overlapping older save could overwrite a newer one. An edit with a save in flight is `paused`: the save's result
// decides its fate.
type EditStore = { edits: Map<string, QueuedEditing>; saving: Set<string> };
const stores = new WeakMap<T3Client, EditStore>();
function editStore(client: T3Client): EditStore {
  let store = stores.get(client);
  if (!store) { store = { edits: new Map(), saving: new Set() }; stores.set(client, store); }
  return store;
}
export function queuedEdit(client: T3Client): QueuedEditing | undefined {
  return client.threadId ? editStore(client).edits.get(client.draftKey) : undefined;
}
/** The images a queued message's edit holds aside for the draft `draftKey` until it ends (their uploads go on). */
export function editStashImages(client: T3Client, draftKey: string): Obj[] {
  return editStore(client).edits.get(draftKey)?.stashImages ?? [];
}

// makeQueuedRunsAtom's deleted thread, which reads as having no runs. The server reports an archive on the active shell
// as it reports a deletion (`thread.removed`), and a fresh snapshot leaves both out, so a thread missing from the active
// shell is "missing" until the archived shell tells them apart (checkMissingEditThreads): an archived thread keeps its
// edit, since its queue stays editable. An archived one is looked at again a minute later, in case it was deleted there.
type Presence = { state: 'missing' | 'checking' | 'archived' | 'deleted'; at: number };
const presences = new WeakMap<QueuedEditing, Presence>();
const ARCHIVED_RECHECK_MS = 60_000;

/**
 * Whether the edit's run left the queue. A deleted thread did. Otherwise the open thread's projection decides, and
 * elsewhere the shell tells a run that started (the reference watches the thread's detail; this client follows one
 * thread, so a queued run that another client removes settles once its thread is open again). Unknown reads as still
 * queued.
 */
function runLeftQueue(client: T3Client, edit: QueuedEditing): boolean {
  if (client.environmentId !== edit.environmentId) return false;
  const shell = client.shell.threads.find(thread => thread.id === edit.threadId);
  if (shell) presences.delete(edit);
  else if (client.shellLoaded && !presences.has(edit)) presences.set(edit, { state: 'missing', at: 0 });
  if (shell?.deletedAt != null || presences.get(edit)?.state === 'deleted') return true;
  if (client.threadId === edit.threadId) {
    if (!client.thread || str(obj(client.projection.thread).id) !== edit.threadId) return false;
    return !arr(client.projection.runs).some(run => run.id === edit.runId && run.status === 'queued');
  }
  if (!shell) return false;
  return shell.activeRunId === edit.runId || (shell.latestRunId === edit.runId && shell.latestRunStartedAt != null);
}

/**
 * Tells archive from deletion for each edit whose thread left the active shell, by the archived shell snapshot, and
 * wakes the window when one was deleted, so the next snapshot ends its edit. `now`: the clock an archived thread's
 * next look is timed by.
 */
export async function checkMissingEditThreads(client: T3Client, native: Native, now: number): Promise<void> {
  const due = [...editStore(client).edits.values()].filter(edit => {
    const presence = presences.get(edit);
    return edit.environmentId === client.environmentId && !!presence
      && (presence.state === 'missing' || (presence.state === 'archived' && now - presence.at >= ARCHIVED_RECHECK_MS));
  });
  if (!due.length) return;
  for (const edit of due) presences.set(edit, { state: 'checking', at: now });
  let archived: Set<string>;
  try {
    const snapshot = applyShell(initialShell(), await client.restAccess(native).read('orchestration.getArchivedShellSnapshot'));
    archived = new Set(snapshot.threads.filter(thread => thread.deletedAt == null).map(thread => str(thread.id)));
  } catch {
    for (const edit of due) if (presences.get(edit)?.state === 'checking') presences.set(edit, { state: 'missing', at: now });
    return;
  }
  for (const edit of due) if (presences.get(edit)?.state === 'checking') presences.set(edit, { state: archived.has(edit.threadId) ? 'archived' : 'deleted', at: now });
  if (due.some(edit => presences.get(edit)?.state === 'deleted')) await wakeShell(native);
}
/** Each edit whose run left the queue ends once, wherever the user is: a changed edit is kept or discarded with a toast. */
export function settleQueuedEdits(client: T3Client): void {
  const { edits } = editStore(client);
  for (const [key, edit] of [...edits]) {
    if (edit.paused || !runLeftQueue(client, edit)) continue;
    edits.delete(key);
    endLostEdit(client, edit);
  }
}

export function beginQueuedEdit(client: T3Client, runId: string): void {
  if (queuedEditBlockedReason(client)) return;
  const queued = queueState(client.projection).queued, at = queued.findIndex(candidate => candidate.run.id === runId), entry = queued[at];
  if (!entry) throw new ClientError('That queued message already started or was removed.');
  if (!entry.editable) return;
  const { edits } = editStore(client);
  edits.set(client.draftKey, startEdit(client, entry.run, entry.text, edits.get(client.draftKey)));
  // editLatest shows the whole queue, so the message being edited (one the short list hides) stays in view.
  if (queued.length > QUEUE_VISIBLE_ROWS && at >= QUEUE_VISIBLE_ROWS - 1) showAllQueued(client);
}
export function cancelQueuedEdit(client: T3Client): void {
  const current = queuedEdit(client);
  if (!current) return;
  editStore(client).edits.delete(current.key);
  restoreThreadDraft(client, current);
}
/** removeEditingAttachment: a kept attachment leaves the edit (op cclocal:queued-attachment-remove). */
export function removeQueuedEditAttachment(client: T3Client, attachmentId: string): void { removeExisting(queuedEdit(client), attachmentId); }

/**
 * startSave: claims the open edit for a save, or null when there is none or a save is in flight. The returned finish
 * is false, changing nothing, when the edit was ended or begun again meanwhile; success ends the edit; failure resumes
 * its watch and says whether the edit survived it.
 */
function startSave(client: T3Client, edit: QueuedEditing): ((succeeded: boolean) => boolean) | null {
  const store = editStore(client);
  if (store.edits.get(edit.key) !== edit || store.saving.has(edit.key)) return null;
  store.saving.add(edit.key);
  edit.paused = true;
  return succeeded => {
    store.saving.delete(edit.key);
    edit.paused = false;
    if (store.edits.get(edit.key) !== edit) return false;
    if (succeeded) { store.edits.delete(edit.key); return true; }
    if (!runLeftQueue(client, edit)) return true;
    store.edits.delete(edit.key);
    endLostEdit(client, edit);
    return false;
  };
}
/** The send button saves the edit (queued-run.edit with its attachments) and gives the composer back the thread's draft. */
export async function saveQueuedEdit(client: T3Client, native: Native, storage: Files, value: string, uploadImages: () => Promise<Obj[]> = async () => []): Promise<void> {
  const current = queuedEdit(client);
  if (!current) throw new ClientError('There is no queued message being edited.');
  const finish = startSave(client, current);
  if (!finish) return;
  let saved: boolean;
  try { saved = await saveEdit(client, native, storage, current, value, uploadImages); } catch (error) { finish(false); throw error; }
  // A save that changed nothing leaves the edit open, as a failed one does.
  if (finish(saved) && saved) restoreThreadDraft(client, current);
}

// ── The queue's rows ───────────────────────────────────────────────────────

// QUEUE_VISIBLE_ROWS: a longer queue shows its first messages and a button that shows the rest.
export const QUEUE_VISIBLE_ROWS = 3;
// QueuedRunsControl showAll: the thread whose long queue shows in full. It resets once the queue is empty.
const shownInFull = new WeakMap<T3Client, string>();
export function queueShownInFull(client: T3Client): boolean { return shownInFull.get(client) === client.draftKey; }
export function showAllQueued(client: T3Client): void { shownInFull.set(client, client.draftKey); }
export function toggleQueueShownInFull(client: T3Client): void {
  if (queueShownInFull(client)) shownInFull.delete(client); else showAllQueued(client);
}
export function forgetQueueShownInFull(client: T3Client): void { shownInFull.delete(client); }

/** The composer's view of the edit: the run it edits, its kept attachments and whether its save is in flight. */
export function queuedEditView(client: T3Client) {
  const edit = queuedEdit(client);
  return { queueEditing: edit?.runId ?? '', ...editView(edit, !!edit && editStore(client).saving.has(edit.key)) };
}

/** A queued send in flight whose message the projection does not hold yet (ChatView optimistic queued_turn messages). */
function pendingQueued(client: T3Client): QueuedItem | null {
  const pending = client.pending, payload = obj(pending?.payload);
  if (!client.busy || payload.type !== 'message.dispatch' || obj(payload.dispatchMode).type !== 'queue_after_active' || payload.threadId !== client.threadId) return null;
  if (arr(client.projection.messages).some(message => message.id === payload.messageId)) return null;
  const thumbnails = arr(payload.attachments).filter(attachment => attachment.type === 'image').map(attachment => ({ id: str(attachment.id), name: str(attachment.name) }));
  return { key: `pending:${str(payload.messageId)}`, runId: '', text: queuedPreviewText(str(payload.text), thumbnails.length > 0), editable: true, thumbnails, serverIndex: -1, pending: true };
}
