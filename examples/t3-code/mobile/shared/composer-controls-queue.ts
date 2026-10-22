// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/composer-controls-queue.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The queued-messages control above the composer (lane composer-controls),
// adapted from T3 Code (MIT); see LICENSE-T3. Sources: chat/QueuedRunsControl.tsx,
// packages/client-runtime/src/state/threadWorkflows.ts (deriveThreadQueueWorkflowState,
// getUserQueuedThreadRuns) and ChatView.tsx (beginEditingQueuedRun,
// cancelEditingQueuedRun, the queued-edit save path and its lost-run recovery).
import { arr, obj, str, type Obj } from './domain';
import { ClientError, activeRun, type Files, type Native } from './protocol';
import type { T3Client } from './client';
import { editView, endLostEdit, removeExisting, restoreThreadDraft, saveEdit, startEdit, type QueuedEditing } from './queued-edit-attachments';

export type QueuedRow = { runId: string; text: string; images: number; index: number; editing: boolean; last: boolean;
  /** composer-fidelity G12a: the row's 16pt image thumbnails (ids the window matches to signed URLs) and a send still saving. */
  thumbnails: Array<{ id: string; name: string }>; pending: boolean };

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
  return {
    queued: queued.map(entry => {
      const message = messages.find(candidate => candidate.id === entry.userMessageId);
      const images = arr(message?.attachments).filter(attachment => attachment.type === 'image');
      return { run: entry, text: message ? str(message.text) : 'Queued message', images: images.length, thumbnails: images.map(image => ({ id: str(image.id), name: str(image.name) })) };
    }),
    activeRunId: str(run?.id),
    canReorder: turns.supportsQueuedMessages === true,
    canSteer: steerable && (turns.supportsActiveSteering === true || turns.supportsSteeringByInterruptRestart === true),
  };
}

// The edit draft borrows the thread's composer; the thread's own draft waits here
// (composer-fidelity G12a: with its SnapShot images, and the message's attachments load; queued-edit-attachments.ts).
const editing = new WeakMap<T3Client, QueuedEditing>();
const threadKey = (client: T3Client) => `${client.environmentId}:${client.threadId}`;
export function queuedEdit(client: T3Client): QueuedEditing | undefined {
  const entry = editing.get(client);
  return entry && entry.key === threadKey(client) ? entry : undefined;
}

/** The control's rows plus whether edit mode still holds a live queued run (else it ends, keeping or dropping the edit). */
export function queuedView(client: T3Client) {
  const state = client.threadId ? queueState(client.projection) : { queued: [], activeRunId: '', canReorder: false, canSteer: false };
  const current = queuedEdit(client);
  if (current && !state.queued.some(entry => entry.run.id === current.runId)) { editing.delete(client); endLostEdit(client, current); }
  const edit = queuedEdit(client);
  const rows: QueuedRow[] = state.queued.map((entry, index) => ({ runId: str(entry.run.id), text: entry.text.replace(/\s+/g, ' ').trim(), images: entry.images,
    index, editing: edit?.runId === entry.run.id, last: index === state.queued.length - 1, thumbnails: entry.thumbnails, pending: false }));
  // composer-fidelity G12a: a queued send the projection has not acknowledged yet shows as a pending row (QueuedRunsControl optimisticQueued).
  const pending = pendingQueued(client);
  if (pending) rows.push({ ...pending, index: rows.length, last: true }), rows.forEach((row, index) => { row.last = index === rows.length - 1; });
  return { queued: rows, queueReorder: state.canReorder, queueSteer: state.canSteer, queueSteerReason: state.activeRunId ? '' : 'There is no active run to steer',
    queueEditing: edit?.runId ?? '', queueDropSeq: drops.get(client) ?? 0, ...editView(edit) };
}

// Each finished grip drag (QueuedRunsControl completeDrag) bumps this, so the view's lifted row settles.
const drops = new WeakMap<T3Client, number>();
/**
 * completeDrag: `value` is "<runId>|<insert index>" (0..queued length) from the
 * grip's pan release. Inserting just before or after itself is a no-op; else
 * the run moves before the run now at that index (the end when there is none).
 * Returns the run to put before ('' for the end), or null for nothing to do.
 */
export function queuedDrop(client: T3Client, value: string): { runId: string; before: string } | null {
  drops.set(client, (drops.get(client) ?? 0) + 1);
  const [runId = '', at = ''] = value.split('|'), insert = Number(at);
  const queued = queueState(client.projection).queued.map(entry => str(entry.run.id)), dragged = queued.indexOf(runId);
  if (!runId || !Number.isInteger(insert) || insert < 0 || insert > queued.length || dragged < 0) return null;
  if (insert === dragged || insert === dragged + 1) return null;
  return { runId, before: queued[insert] ?? '' };
}

export function beginQueuedEdit(client: T3Client, runId: string): void {
  const entry = queueState(client.projection).queued.find(candidate => candidate.run.id === runId);
  if (!entry) throw new ClientError('That queued message already started or was removed.');
  editing.set(client, startEdit(client, threadKey(client), entry.run, entry.text, queuedEdit(client)));
}
export function cancelQueuedEdit(client: T3Client): void {
  const current = queuedEdit(client);
  if (!current) return;
  editing.delete(client);
  restoreThreadDraft(client, current);
}
/** removeEditingQueuedAttachment: a kept attachment leaves the edit (op cclocal:queued-attachment-remove). */
export function removeQueuedEditAttachment(client: T3Client, attachmentId: string): void { removeExisting(queuedEdit(client), attachmentId); }
/** The send button saves the edit (queued-run.edit with its attachments) and gives the composer back its draft. */
export async function saveQueuedEdit(client: T3Client, native: Native, storage: Files, value: string, uploadImages: () => Promise<Obj[]> = async () => []): Promise<void> {
  const current = queuedEdit(client);
  if (!current) throw new ClientError('There is no queued message being edited.');
  const saved = await saveEdit(client, native, storage, current, value, uploadImages,
    () => queueState(client.projection).queued.some(entry => entry.run.id === current.runId));
  if (!saved) return;
  editing.delete(client);
  restoreThreadDraft(client, current);
}

/** A queued send in flight whose message the projection does not hold yet (ChatView optimistic queued_turn messages). */
function pendingQueued(client: T3Client): Omit<QueuedRow, 'index' | 'last'> | null {
  const pending = client.pending, payload = obj(pending?.payload);
  if (!client.busy || payload.type !== 'message.dispatch' || obj(payload.dispatchMode).type !== 'queue_after_active' || payload.threadId !== client.threadId) return null;
  if (arr(client.projection.messages).some(message => message.id === payload.messageId)) return null;
  return { runId: `pending:${str(payload.messageId)}`, text: str(payload.text).replace(/\s+/g, ' ').trim(), images: 0, editing: false,
    thumbnails: arr(payload.attachments).filter(attachment => attachment.type === 'image').map(attachment => ({ id: str(attachment.id), name: str(attachment.name) })), pending: true };
}
