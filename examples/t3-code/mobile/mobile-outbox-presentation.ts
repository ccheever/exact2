// Source365aa87982 pending-new-tasks-model and pending-thread-feed.
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { arr, obj } from './shared/domain';
import { mobileOutboxRootFailure } from './mobile-outbox-root';
import { mobileClient } from './client';
import type { T3Client } from './shared/client';
import { mobileOutboxSnapshot } from './mobile-outbox';
import { mobileOutboxCreationShell, mobileOutboxDriveCompleted, mobileOutboxDriveSnapshot } from './mobile-outbox-drive';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import { homeDraftTitle, type HomeDraftOptions } from './home-drafts';
import { mobileMessageTime, mobileThreadBlocks, mobileThreadRows, type ThreadSnapshot } from './thread';

export interface MobilePendingTask {
  owner: string; record: MobileOutboxRecord; status: string; reason: string; canRetry: boolean;
}
export function mobileOutboxOwner(record: MobileOutboxRecord): string {
  return JSON.stringify({ origin: record.origin, environmentId: record.environmentId, threadId: record.threadId,
    messageId: record.messageId, commandId: record.commandId });
}
export function mobileOutboxPendingTasks(client: T3Client, now: number): MobilePendingTask[] {
  const drive = mobileOutboxDriveSnapshot(client, now);
  const result = mobileOutboxSnapshot(client).rows.filter(row => row.record.creation).map(row => {
    const owner = mobileOutboxOwner(row.record), item = drive.items.find(item => item.owner === owner);
    return { owner, record: row.record, status: item?.status ?? 'queued', reason: item?.reason ?? '', canRetry: item?.canRetry ?? false };
  });
  for (const record of mobileOutboxDriveCompleted(client)) {
    if (record.creation && !result.some(item => item.owner === mobileOutboxOwner(record))) result.push({ owner: mobileOutboxOwner(record),
      record, status: 'delivered', reason: 'Waiting for the task to synchronize.', canRetry: false });
  }
  return result;
}
export function mobileOutboxStatus(status: string): string {
  return ({ sending: 'Sending…', delivered: 'Starting task…', waiting: 'Waiting to send', retry: 'Waiting to retry',
    'recovery-required': 'Needs attention', 'edited-after-ack': 'Saved changes need attention',
    'cleanup-pending': 'Finishing send…', editing: 'Being edited' } as Record<string, string>)[status] ?? 'Sends on reconnect';
}
const previous = new WeakMap<T3Client, { key: string; item: MobilePendingTask }>();
/** Same prompt-and-turn handoff as source resolvePendingThreadCreation. */
export function mobileOutboxCreationDetailReady(record: MobileOutboxRecord, detail: unknown): boolean {
  if (!detail || typeof detail !== 'object') return false;
  const projection = obj(detail), run = arr(projection.runs).at(-1), status = run?.status ?? obj(projection.session).status;
  if (['failed', 'error', 'cancelled', 'stopped', 'interrupted'].includes(String(status))) return true;
  return (run !== undefined || projection.latestTurn != null) && arr(projection.messages).some(message => message.id === record.messageId);
}
/** A mounted route keeps its last creation through shell/outcome collection.
 * It may load the real detail while its preparing presentation stays visible. */
export function mobileOutboxThread(environmentId: string, threadId: string, now: number, dark = false,
  client: T3Client = mobileClient): ThreadSnapshot | null {
  const key = JSON.stringify([environmentId, threadId]);
  if (previous.get(client)?.key !== key || !environmentId || !threadId) previous.delete(client);
  if (!environmentId || !threadId) return null;
  const failure = mobileOutboxRootFailure(client, environmentId, threadId);
  const pending = mobileOutboxPendingTasks(client, now).filter(item => item.record.environmentId === environmentId
    && item.record.threadId === threadId).sort((a, b) => a.record.createdAt.localeCompare(b.record.createdAt))[0];
  const item = pending ?? previous.get(client)?.item;
  if (!item && !failure) return null;
  const record = failure?.record ?? item!.record, shell = mobileOutboxCreationShell(client, record);
  const detail = client.environmentId === environmentId && client.threadId === threadId
    && obj(client.thread?.projection.thread).id === threadId ? client.thread?.projection : null;
  if (failure && shell || !failure && mobileOutboxCreationDetailReady(record, detail)) {
    previous.delete(client); return null;
  }
  if (!failure) previous.set(client, { key, item: { ...item!, record: JSON.parse(JSON.stringify(record)) } });
  const status = failure ? 'Could not start task' : mobileOutboxStatus(item!.status);
  const rows = detail ? mobileThreadRows(client, now, dark) : [];
  return { revision: client.revision, environmentId, threadId, title: homeDraftTitle(record.text, record.attachments.length),
    queuedCanSelect: !!shell && !failure, queued: true, queuedOwner: mobileOutboxOwner(record), queuedStatus: status, queuedReason: failure?.reason ?? item!.reason,
    queuedCanRetry: !failure && item!.canRetry, queuedFailed: !!failure, queuedCanEdit: failure?.editable ?? false,
    loaded: true, loading: false, emptyTitle: '', emptyDetail: '', error: '', uncertain: false, hasMore: false,
    historyLoading: false, historyError: '', readsNeeded: false, answerFilesOwner: '', answerFilesRequest: '', approvals: [],
    rows: [...rows, ...(detail && arr(detail.messages).some(message => message.id === record.messageId) ? [] : [{ id: record.messageId, kind: 'pending', title: '', body: record.text, blocks: mobileThreadBlocks(record.text, dark),
      user: true, timestamp: mobileMessageTime(record.createdAt), showMeta: true, streaming: false, attribution: '', intent: status,
      copied: false, expanded: false, toggleOp: '', toggleId: '', failed: false, live: false, activities: [],
      media: [], first: rows.length === 0, last: true, canFork: false, forkKey: '', forkBusy: false }])].map((row, index, all) => ({ ...row, first: index === 0, last: index === all.length - 1 })),
    composer: { editing: false, saving: false, canCancel: false, editNotice: '', editPendingId: '', canRetryEdit: false,
      contentOwner: '', draft: '', placeholder: 'Waiting for this task to start…', canSend: false, canStop: false, showStop: false,
      canOperate: false, showReadOnlyNotice: false, sendLabel: 'Send', sendSymbol: 'arrow.up', blockedReason: status,
      modelLabel: record.modelSelection?.model ?? '', providerDriver: '', providerIconURL: '', modelUnavailable: false,
      running: false, queueCount: 0 } };
}
/** Same pending-task slot as source: drafts lead, then queued creations newest first. */
export function projectHomePending(tasks: readonly MobilePendingTask[], options: HomeDraftOptions = {}) {
  const query = options.query?.trim().toLocaleLowerCase() ?? '';
  return tasks.filter(item => {
    const record = item.record, creation = record.creation;
    return creation && (!options.environmentId || record.environmentId === options.environmentId)
      && (options.projectRefs == null || options.projectRefs.some(project => project.environmentId === record.environmentId && project.projectId === creation.projectId))
      && (!query || homeDraftTitle(record.text, record.attachments.length).toLocaleLowerCase().includes(query));
  }).sort((a, b) => b.record.createdAt.localeCompare(a.record.createdAt) || a.record.messageId.localeCompare(b.record.messageId));
}
