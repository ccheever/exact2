// App-owned queued edit memory and notice lifetime, below native/editor adapters.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { ClientError } from './shared/protocol';
import { mobileQueuedEditOrigin } from './queued-edit-origin';

export interface MobileQueuedEditAttachment {
  id: string; name: string; mimeType: string; sizeBytes: number;
  kind: 'image' | 'file'; uploadId: string; status: 'staged' | 'uploading' | 'ready' | 'error';
}
export interface MobileQueuedEditSession {
  owner: string; draftKey: string; origin: string; environmentId: string; threadId: string; projectId: string;
  generation: number; session: string; revision: number; runId: string; messageId: string; text: string;
  recoveryKey?: string; context?: Obj; attachments: MobileQueuedEditAttachment[]; existingAttachments: Obj[]; saving: boolean;
}
export interface MobileQueuedEditOperation {
  operationId: string; owner: string; revision: number; state: string; environmentId: string; origin: string;
  method: string; payload: Obj; editorRevision: number; error?: unknown;
}
export interface MobileQueuedEditNoticeOwner {
  origin: string; environmentId: string; threadId: string; messageId: string | null;
}
export interface QueuedEditState {
  sessions: Map<string, MobileQueuedEditSession>; active: Map<string, string>;
  recoveries: Map<string, MobileQueuedEditSession>; operationEpoch: number; readSerial: number;
  operations: Map<string, MobileQueuedEditOperation>; globalNotice: { readonly message: string } | null;
  notices: Map<string, { message: string; messageId: string | null; serial: number }>; noticeSerial: number; hydrated: boolean;
}
const states = new WeakMap<T3Client, QueuedEditState>();
export function queuedEditState(client: T3Client): QueuedEditState {
  let state = states.get(client);
  if (!state) { state = { sessions: new Map(), active: new Map(), recoveries: new Map(), operationEpoch: 0, readSerial: 0, operations: new Map(), globalNotice: null, notices: new Map(), noticeSerial: 0, hydrated: false }; states.set(client, state); }
  return state;
}
const noticeKey = (owner: MobileQueuedEditNoticeOwner) => JSON.stringify([owner.origin, owner.environmentId, owner.threadId]);
/** Pinned thread-composer-error keeps errors on their thread through navigation
 * and typing. The optional message identity prevents an older acknowledgment
 * from clearing a newer message's error. Journal-wide failures have no thread. */
export function queuedEditNoticeOwner(client: T3Client, messageId: string | null = null): MobileQueuedEditNoticeOwner | null {
  return client.environmentId && client.threadId ? { origin: mobileQueuedEditOrigin(client), environmentId: client.environmentId,
    threadId: client.threadId, messageId } : null;
}
export function queuedEditSessionNoticeOwner(edit: MobileQueuedEditSession, messageId: string | null = edit.messageId): MobileQueuedEditNoticeOwner {
  return { origin: edit.origin, environmentId: edit.environmentId, threadId: edit.threadId, messageId };
}
export function queuedEditOperationNoticeOwner(operation: MobileQueuedEditOperation, client: T3Client, records: Obj[] = []): MobileQueuedEditNoticeOwner | null {
  const edit = queuedEditState(client).sessions.get(operation.owner) ?? obj(records.find(record => record.owner === operation.owner)?.record);
  const threadId = str(operation.payload.threadId);
  if (!operation.origin || !operation.environmentId || !threadId) return null;
  return { origin: operation.origin, environmentId: operation.environmentId, threadId,
    messageId: edit.origin === operation.origin && edit.environmentId === operation.environmentId && edit.threadId === threadId ? str(edit.messageId) || null : null };
}
export function queuedEditSetNotice(client: T3Client, message: string, owner: MobileQueuedEditNoticeOwner | null): void {
  const state = queuedEditState(client);
  if (!owner) state.globalNotice = message ? { message } : null;
  else {
    const key = noticeKey(owner);
    if (message) {
      if (!Number.isSafeInteger(state.noticeSerial + 1)) throw new ClientError('Reopen this view before reporting another composer error.');
      state.notices.set(key, { message, messageId: owner.messageId, serial: ++state.noticeSerial });
    }
    else if (state.notices.get(key)?.messageId === owner.messageId) state.notices.delete(key);
  }
  client.revision++;
}
export function mobileQueuedEditNotice(client: T3Client): string {
  const state = queuedEditState(client), owner = queuedEditNoticeOwner(client);
  return state.globalNotice?.message || (owner ? state.notices.get(noticeKey(owner))?.message : '') || '';
}
/** Exact presented identity, including same-text replacement. Global journal
 * failures have no scoped Dismiss and retire only after a complete refresh. */
export function queuedEditNoticeKey(client: T3Client, owner = queuedEditNoticeOwner(client)): string {
  const notice = owner ? queuedEditState(client).notices.get(noticeKey(owner)) : null;
  return notice && owner ? JSON.stringify([owner.origin, owner.environmentId, owner.threadId, notice.serial]) : '';
}
export function mobileQueuedEditDismissKey(client: T3Client): string {
  return queuedEditState(client).globalNotice ? '' : queuedEditNoticeKey(client);
}
export function queuedEditClearCapturedNotice(client: T3Client, owner: MobileQueuedEditNoticeOwner | null, expectedKey: string): boolean {
  if (!owner || !expectedKey || queuedEditNoticeKey(client, owner) !== expectedKey) return false;
  queuedEditState(client).notices.delete(noticeKey(owner)); client.revision++; return true;
}
export function mobileQueuedEditDismissNotice(key: string, client: T3Client): boolean {
  return !!key && mobileQueuedEditDismissKey(client) === key && queuedEditClearCapturedNotice(client, queuedEditNoticeOwner(client), key);
}
/** Removal owns the environment, including errors reported while it awaited.
 * A different saved canonical identity protects only that replacement's errors. */
export function queuedEditClearEnvironmentNotices(client: T3Client, environmentId: string, replacementOrigin = ''): void {
  let changed = false;
  for (const key of queuedEditState(client).notices.keys()) {
    const [origin, environment] = JSON.parse(key) as string[];
    if (environment === environmentId && (!replacementOrigin || origin !== replacementOrigin)) {
      queuedEditState(client).notices.delete(key); changed = true;
    }
  }
  if (changed) client.revision++;
}
