// Source365aa87982 pending-new-tasks-model and pending-thread-feed.
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { mobileClient } from './client';
import type { T3Client } from './shared/client';
import { mobileOutboxSnapshot } from './mobile-outbox';
import { mobileOutboxDriveCompleted, mobileOutboxDriveSnapshot } from './mobile-outbox-drive';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import { homeDraftTitle, type HomeDraftOptions } from './home-drafts';
import { mobileMessageTime, mobileThreadBlocks, type ThreadSnapshot } from './thread';

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
/** The route remains local until the matching shell thread exists. Never selects a shared thread. */
export function mobileOutboxThread(environmentId: string, threadId: string, now: number, dark = false,
  client: T3Client = mobileClient): ThreadSnapshot | null {
  if (client.environmentId === environmentId && client.shell.threads.some(thread => thread.id === threadId)) return null;
  const item = mobileOutboxPendingTasks(client, now).filter(item => item.record.environmentId === environmentId
    && item.record.threadId === threadId).sort((a, b) => a.record.createdAt.localeCompare(b.record.createdAt))[0];
  if (!item) return null;
  const { record } = item, status = mobileOutboxStatus(item.status);
  return { revision: client.revision, environmentId, threadId, title: homeDraftTitle(record.text, record.attachments.length),
    queued: true, queuedOwner: item.owner, queuedStatus: status, queuedReason: item.reason, queuedCanRetry: item.canRetry,
    loaded: true, loading: false, emptyTitle: '', emptyDetail: '', error: '', uncertain: false, hasMore: false,
    historyLoading: false, historyError: '', readsNeeded: false, answerFilesOwner: '', answerFilesRequest: '', approvals: [],
    rows: [{ id: record.messageId, kind: 'pending', title: '', body: record.text, blocks: mobileThreadBlocks(record.text, dark),
      user: true, timestamp: mobileMessageTime(record.createdAt), showMeta: true, streaming: false, attribution: '', intent: status,
      copied: false, expanded: false, toggleOp: '', toggleId: '', failed: false, live: false, activities: [],
      media: record.attachments.map(file => ({ id: file.id, name: file.name, kind: file.kind, url: '' })), first: true, last: true }],
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
