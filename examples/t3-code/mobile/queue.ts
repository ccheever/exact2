// Pinned365aa87982 ThreadQueueControl / threadQueueControlPresentation (MIT).
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { queueState } from './shared/composer-controls-queue';
import { composerCommand } from './shared/composer-controls-commands';
import { mobileSessionGrants } from './environment-detail';
import { queueImageSnapshot } from './queue-images';
import { mobileQueuedEditCurrent } from './queued-edit-state';
import { mobileQueuedEditBegin } from './queued-edit';

export interface MobileQueueAttachment { id: string; name: string; mimeType: string; image: boolean; url: string }
export interface MobileQueueRow {
  id: string; actionId: string; menuId: string; title: string; text: string; index: number;
  editing: boolean; busy: boolean; canEdit: boolean; canSteer: boolean; canMoveUp: boolean; canMoveDown: boolean; canRemove: boolean;
  attachments: MobileQueueAttachment[]; attachmentOverflow: string;
}
export interface MobileQueueSnapshot {
  owner: string; visit: string; environmentId: string; threadId: string; thumbnailRequest: string; actionId: string; rows: MobileQueueRow[]; count: number; held: boolean; busy: boolean; error: string;
  canResume: boolean; showSteer: boolean; dismiss: boolean; editingUnavailable: boolean;
}
interface State { owner: string; visit: string; active: boolean; hadRows: boolean; editDismiss: boolean; busy: string; error: string }
const states = new WeakMap<T3Client, State>();
export const mobileQueueOwner = (client: T3Client) => JSON.stringify([client.origin, client.environmentId, client.projectId, client.threadId, client.generation, client.threadEpoch]);
function stateFor(client: T3Client) {
  const owner = mobileQueueOwner(client); let state = states.get(client);
  if (!state || state.owner !== owner) { state = { owner, visit: '', active: false, hadRows: false, editDismiss: false, busy: '', error: '' }; states.set(client, state); }
  return state;
}
export const mobileQueueCurrent = (owner: string, visit: string, client: T3Client) => {
  const state = states.get(client); return !!state && state.active && state.owner === owner && state.visit === visit && mobileQueueOwner(client) === owner;
};
const orderOf = (client: T3Client) => queueState(client.projection).queued.map(entry => str(entry.run.id));
const held = (client: T3Client) => arr(client.projection.runs).some(run => run.status === 'queued' && run.queueHeld === true);
const loaded = (client: T3Client) => !!client.threadId && str(obj(client.projection.thread).id) === client.threadId;
const actionId = (state: State, runId: string, order: string[]) => JSON.stringify({ owner: state.owner, visit: state.visit, runId, order });

/** Sheet visit is explicit: an initially empty queue does not auto-close. Only
 * a queue seen nonempty in this visit closes when its last row leaves. */
export function mobileQueueSnapshot(visit: string, active: boolean, now: number, client: T3Client = mobileClient): MobileQueueSnapshot {
  const state = stateFor(client);
  if (state.visit !== visit || active && !state.active) { state.visit = visit; state.hadRows = false; state.editDismiss = false; state.error = ''; }
  state.active = active;
  const queue = queueState(client.projection), order = queue.queued.map(entry => str(entry.run.id));
  if (active && queue.queued.length) state.hadRows = true;
  const busy = !!state.busy || !!client.pending || client.busy, enabled = active && loaded(client) && client.writable && !busy;
  const editing = mobileQueuedEditCurrent(client);
  const rows = queue.queued.map((entry, index) => {
    const message = arr(client.projection.messages).find(message => message.id === entry.run.userMessageId);
    const attachments = arr(message?.attachments).map(attachment => ({ id: str(attachment.id), name: str(attachment.name),
      mimeType: str(attachment.mimeType), image: str(attachment.mimeType).startsWith('image/'), url: '' }));
    const images = attachments.filter(attachment => attachment.image).slice(0, 3), overflow = attachments.length - images.length;
    return { id: str(entry.run.id), actionId: actionId(state, str(entry.run.id), order), menuId: `queue-menu-${index}`,
      title: entry.text || (attachments.length ? 'Attachments' : 'Queued message'), text: entry.text, index: index + 1,
      editing: editing?.runId === entry.run.id, busy: state.busy === entry.run.id,
      canEdit: active && loaded(client) && !busy && !editing?.saving && editing?.runId !== entry.run.id,
      canSteer: enabled && queue.canSteer && editing?.runId !== entry.run.id,
      canMoveUp: enabled && queue.canReorder && index > 0, canMoveDown: enabled && queue.canReorder && index < order.length - 1,
      canRemove: enabled, attachments: images, attachmentOverflow: overflow > 0 ? `${images.length ? '+' : ''}${overflow}` : '' };
  });
  const thumbnails = queueImageSnapshot(client, state.owner, visit, active, rows.flatMap(row => row.attachments), now);
  for (const row of rows) for (const attachment of row.attachments) attachment.url = thumbnails.url(attachment);
  return { owner: state.owner, visit, environmentId: client.environmentId, threadId: client.threadId, thumbnailRequest: thumbnails.request, actionId: actionId(state, '', order), rows, count: rows.length, held: held(client), busy,
    error: state.error, canResume: enabled && held(client) && rows.length > 0, showSteer: queue.canSteer,
    dismiss: active && !busy && (state.editDismiss || state.hadRows && rows.length === 0), editingUnavailable: false };
}

/** Shared server queue controls and the separate mobile queued-edit content owner. */
export async function mobileQueueCommand(args: unknown[], nativeInput: Native | null | undefined, suppliedStorage: Files,
  client: T3Client = mobileClient): Promise<{ revision: number; message: string; dismiss?: boolean }> {
  const state = stateFor(client), result = (message = '') => ({ revision: client.revision, message });
  let ticket: Obj;
  try { ticket = obj(JSON.parse(str(args[1]))); } catch { return result('That queue action is no longer available.'); }
  const op = str(args[0]).replace(/^queue:/, ''), runId = str(ticket.runId);
  if (op === 'edit') {
    const answer = await mobileQueuedEditBegin(str(args[1]), nativeInput, client);
    if (answer.dismiss && mobileQueueCurrent(str(ticket.owner), str(ticket.visit), client)) state.editDismiss = true;
    return answer;
  }
  if (!['remove', 'up', 'down', 'reorder', 'steer', 'resume'].includes(op)) return result('Queued message editing is not available yet.');
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to manage the queue.');
  if (state.busy || client.pending || client.busy) return result('Wait for the current operation before changing the queue.');
  const captured = state.owner, visit = state.visit;
  const current = () => states.get(client) === state && mobileQueueOwner(client) === captured && state.active && state.visit === visit
    && ticket.owner === captured && ticket.visit === visit;
  let issued = false, commandId = '';
  const ownPending = () => !!commandId && client.pending?.payload.commandId === commandId;
  const initial = queueState(client.projection), target = initial.queued.find(entry => entry.run.id === runId);
  const messageId = str(target?.run.userMessageId), activeRunId = initial.activeRunId;
  // A known-unsent rejection must use shared write's ordinary cleanup path.
  // Once sent, retain its durable uncertain outcome; never clear another write.
  const refuse = (message: string): never => { throw new ClientError(message, !issued && ownPending() ? 'client' : 'superseded'); };
  const assertAdmitted = () => {
    if (client.pending && !ownPending()) throw new ClientError('Another operation owns this connection.', 'superseded');
    if (!current()) refuse('The selected queue changed.');
    if (issued) return;
    if (client.busy) refuse('Wait for the current operation before changing the queue.');
    if (!loaded(client) || !client.writable) throw new ClientError('This connection cannot manage the queue.');
    const queue = queueState(client.projection), live = queue.queued.find(entry => entry.run.id === runId);
    if (op === 'resume') {
      if (!held(client) || !queue.queued.length) refuse('This queue is no longer held.');
    } else {
      if (!live || str(live.run.userMessageId) !== messageId) refuse('That queued message already started or was removed.');
      if (op === 'steer' && (!queue.canSteer || queue.activeRunId !== activeRunId)) refuse('The active run changed before steering.');
      if (['up', 'down', 'reorder'].includes(op) && (!queue.canReorder || JSON.stringify(ticket.order) !== JSON.stringify(orderOf(client)))) {
        refuse('The queue order changed. Try again.');
      }
    }
  };
  const base = letGoAware(mobileNative(nativeInput));
  const native: Native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
    assertAdmitted();
    if (obj(input).op === 'request' && obj(input).method === 'orchestration.dispatchCommand') issued = true;
    const answer = await base.later(input);
    if (obj(input).op === 'ids') { const ids = obj(answer).value; if (Array.isArray(ids) && ids.length === 1) commandId = str(ids[0]); }
    assertAdmitted(); return answer;
  } };
  // Persistence is owned by this answer, not queue membership: a pre-wire
  // invalidation must still persist shared cleanup of the known-unsent command.
  const storage = client === mobileClient ? nativeFiles(base) : suppliedStorage;
  if (!current()) return result('The selected queue changed.');
  state.busy = runId || 'resume'; state.error = '';
  try {
    assertAdmitted();
    const session = await client.http(native, '/api/auth/session'); assertAdmitted();
    if (!mobileSessionGrants(session, 'orchestration:operate')) throw new ClientError('This connection cannot manage the queue.');
    if (op === 'resume') {
      // Shared cc:resume can also send a continuation prompt. Mobile's queue
      // sheet resumes only the queue, with ordinary shared durable dispatch.
      const [commandId] = await client.ids(native, 1); assertAdmitted();
      await client.dispatch(native, storage, { type: 'queue.resume', commandId, threadId: client.threadId }, 'Resume queue', assertAdmitted);
    } else {
      let before = str(args[2]);
      const order = orderOf(client), index = order.indexOf(runId);
      if (op === 'up' || op === 'down') {
        if (op === 'up' && index === 0 || op === 'down' && index === order.length - 1) return result();
        before = op === 'up' ? order[index - 1]! : order[index + 2] ?? '';
      }
      if (op === 'reorder') {
        if (before && !order.includes(before)) throw new ClientError('That queue position is no longer available.');
        if (before === runId || before === (order[index + 1] ?? '')) return result();
      }
      await composerCommand(client, native, storage, op === 'remove' ? 'queued-remove' : op === 'steer' ? 'queued-steer' : 'queued-reorder', runId, before);
    }
    return result();
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'Could not change the queue.';
    if (current()) state.error = message;
    return result(message);
  } finally { state.busy = ''; client.revision++; }
}
