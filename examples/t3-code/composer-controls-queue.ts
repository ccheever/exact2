// The queued-messages control above the composer (lane composer-controls),
// adapted from T3 Code (MIT); see LICENSE-T3. Sources: chat/QueuedRunsControl.tsx,
// packages/client-runtime/src/state/threadWorkflows.ts (deriveThreadQueueWorkflowState,
// getUserQueuedThreadRuns) and ChatView.tsx (beginEditingQueuedRun,
// cancelEditingQueuedRun, the queued-edit save path and its lost-run recovery).
import { arr, obj, str, type Obj } from './domain';
import { ClientError, activeRun, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';

export type QueuedRow = { runId: string; text: string; images: number; index: number; editing: boolean; last: boolean };

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
      return { run: entry, text: message ? str(message.text) : 'Queued message', images: arr(message?.attachments).filter(attachment => attachment.type === 'image').length };
    }),
    activeRunId: str(run?.id),
    canReorder: turns.supportsQueuedMessages === true,
    canSteer: steerable && (turns.supportsActiveSteering === true || turns.supportsSteeringByInterruptRestart === true),
  };
}

// The edit draft borrows the thread's composer; the thread's own draft waits here.
type Editing = { key: string; runId: string; originalText: string; stash: string };
const editing = new WeakMap<T3Client, Editing>();
const threadKey = (client: T3Client) => `${client.environmentId}:${client.threadId}`;
export function queuedEdit(client: T3Client): Editing | undefined {
  const entry = editing.get(client);
  return entry && entry.key === threadKey(client) ? entry : undefined;
}

/** The control's rows plus whether edit mode still holds a live queued run (else it ends, keeping or dropping the edit). */
export function queuedView(client: T3Client) {
  const state = client.threadId ? queueState(client.projection) : { queued: [], activeRunId: '', canReorder: false, canSteer: false };
  const current = queuedEdit(client);
  if (current && !state.queued.some(entry => entry.run.id === current.runId)) endLostEdit(client, current);
  const edit = queuedEdit(client);
  const rows: QueuedRow[] = state.queued.map((entry, index) => ({ runId: str(entry.run.id), text: entry.text.replace(/\s+/g, ' ').trim(), images: entry.images,
    index, editing: edit?.runId === entry.run.id, last: index === state.queued.length - 1 }));
  return { queued: rows, queueReorder: state.canReorder, queueSteer: state.canSteer, queueSteerReason: state.activeRunId ? '' : 'There is no active run to steer',
    queueEditing: edit?.runId ?? '', queueDropSeq: drops.get(client) ?? 0 };
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

/** recoverQueuedMessageEdit: a dirty edit moves into an empty thread draft; otherwise it is dropped. */
function endLostEdit(client: T3Client, current: Editing): void {
  editing.delete(client);
  const edited = client.local.drafts[client.draftKey] ?? '';
  const dirty = edited.trim() !== '' && edited !== current.originalText;
  if (dirty && !current.stash.trim()) {
    pushToast(client, { kind: 'info', title: 'Queued message is no longer queued', description: 'Your unsaved edit was kept in the composer.' });
    return;
  }
  client.local.drafts[client.draftKey] = current.stash;
  if (dirty) pushToast(client, { kind: 'warning', title: 'Queued message is no longer queued', description: 'Your edit was discarded because the composer already has a draft.' });
}

export function beginQueuedEdit(client: T3Client, runId: string): void {
  const entry = queueState(client.projection).queued.find(candidate => candidate.run.id === runId);
  if (!entry) throw new ClientError('That queued message already started or was removed.');
  const previous = queuedEdit(client);
  const stash = previous ? previous.stash : client.local.drafts[client.draftKey] ?? '';
  editing.set(client, { key: threadKey(client), runId, originalText: entry.text, stash });
  client.local.drafts[client.draftKey] = entry.text;
}
export function cancelQueuedEdit(client: T3Client): void {
  const current = queuedEdit(client);
  if (!current) return;
  editing.delete(client);
  client.local.drafts[client.draftKey] = current.stash;
}
/** The send button saves the edit (queued-run.edit) and gives the composer back its draft. */
export async function saveQueuedEdit(client: T3Client, native: Native, storage: Files, value: string): Promise<void> {
  const current = queuedEdit(client);
  if (!current) throw new ClientError('There is no queued message being edited.');
  const text = value || client.local.drafts[client.draftKey] || '';
  if (!text.trim()) throw new ClientError('Write the queued message, or remove it from the queue.');
  if (!queueState(client.projection).queued.some(entry => entry.run.id === current.runId)) throw new ClientError('That queued message already started or was removed.');
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.dispatch(storage, { type: 'queued-run.edit', commandId, threadId: client.threadId, runId: current.runId, text }, 'Update queued message');
  editing.delete(client);
  client.local.drafts[client.draftKey] = current.stash;
}
