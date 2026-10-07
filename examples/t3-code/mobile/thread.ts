import { mobileQueuedEditPresentation } from './queued-edit';
// Mobile ThreadFeed/ThreadComposer at upstream365aa87982; shared transport and V2 reducers stay authoritative.
// @ref llp/1106.000-mobile-app-layout.decision.md#shared-typescript
// @ref llp/1106.003-pairing-and-transport.decision.md#mobile-adaptations
import { mobileComposerTarget, mobileComposerTargetText } from './composer-target';
import { mobileAnswerFilesRequest, mobilePrepareAnswerFiles } from './thread-answer-files';
import { mobileClient, mobileCommand, mobileNative } from './client';
import { mobileProviderIconURL } from './environment-detail';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { activeRun, providerAvailable, type Files, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { transcriptRows, timelineView } from './shared/timeline-presentation';
import { refreshTimelineReads, timelineReadsNeeded } from './shared/timeline-prepare';
import { attachmentUrls, cachedAttachmentUrl } from './shared/timeline-attachments';
import { syncWorktreeSetup } from './shared/timeline-worktree';
import { threadPhase } from './shared/composer-presentation';
import { primaryAction } from './shared/composer-controls-view';
import { followUpBehavior } from './shared/composer-controls';
import { queueState } from './shared/composer-controls-queue';
import { requestPresentation } from './shared/requests';
import { mobileCodeTokens, type ThreadCodeToken } from './thread-highlight';
import { mobileThreadActivity } from './thread-work';
import type { ThreadActivity } from './thread-work';
export type { ThreadActivity } from './thread-work';

export interface ThreadBlock { id: string; kind: string; text: string; language: string; tokens: ThreadCodeToken[] }
export interface ThreadMedia { id: string; name: string; kind: string; url: string }
export interface ThreadRow { id: string; kind: string; title: string; body: string; blocks: ThreadBlock[]; user: boolean;
  timestamp: string; showMeta: boolean; streaming: boolean; attribution: string; intent: string; copied: boolean;
  expanded: boolean; toggleOp: string; toggleId: string; failed: boolean; live: boolean; activities: ThreadActivity[];
  media: ThreadMedia[]; first: boolean; last: boolean; }
export interface ThreadApprovalOption { id: string; label: string; tone: string; warning: string }
export interface ThreadApproval { id: string; title: string; detail: string; disabled: boolean; reason: string; options: ThreadApprovalOption[] }
export interface ThreadComposerState { editing: boolean; saving: boolean; canCancel: boolean; editNotice: string; editPendingId: string; canRetryEdit: boolean; contentOwner: string; draft: string; placeholder: string; canSend: boolean; canStop: boolean; showStop: boolean;
  canOperate: boolean; showReadOnlyNotice: boolean; sendLabel: string; sendSymbol: string; blockedReason: string; modelLabel: string; providerDriver: string;
  providerIconURL: string; modelUnavailable: boolean; running: boolean; queueCount: number; }
export interface ThreadSnapshot { revision: number; environmentId: string; threadId: string; title: string; loaded: boolean; loading: boolean; rows: ThreadRow[];
  emptyTitle: string; emptyDetail: string; error: string; uncertain: boolean; hasMore: boolean; historyLoading: boolean; historyError: string;
  readsNeeded: boolean; answerFilesOwner: string; answerFilesRequest: string; approvals: ThreadApproval[]; composer: ThreadComposerState; }
const timeFormatter = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' });
export function mobileMessageTime(value: unknown): string { const stamp = Date.parse(str(value)); return Number.isFinite(stamp) ? timeFormatter.format(stamp) : ''; }

/** Keep rich prose on the portable Markdown reader; fenced blocks use the pinned mobile Shiki theme. */
export function mobileThreadBlocks(body: string, dark: boolean): ThreadBlock[] {
  const blocks: ThreadBlock[] = [];
  const fence = /^( {0,3})(`{3,}|~{3,})([^\n`]*)\n([\s\S]*?)\n? {0,3}\2[`~]*[ \t]*(?:\n|$)/gm;
  let position = 0;
  const prose = (text: string) => { if (text.trim()) blocks.push({ id: String(blocks.length), kind: 'markdown', text, language: '', tokens: [] }); };
  for (const match of body.matchAll(fence)) {
    prose(body.slice(position, match.index));
    const indent = match[1]!.length, language = match[3]!.trim().split(/\s+/)[0] || 'text';
    const text = (indent ? match[4]!.split('\n').map(line => line.replace(new RegExp(`^ {0,${indent}}`), '')).join('\n') : match[4]!).replace(/\n$/, '');
    blocks.push({ id: String(blocks.length), kind: 'code', text, language, tokens: mobileCodeTokens(text, language, dark) });
    position = match.index! + match[0].length;
  }
  prose(body.slice(position)); return blocks;
}
/** Adapts shared derived row types, preserving their ids and disclosure ownership. */
export function mobileThreadRows(client: T3Client, now: number, dark = false): ThreadRow[] {
  const source = transcriptRows(client), projected = new Map(arr(client.projection.visibleTurnItems).map(row => [JSON.stringify([row.sourceThreadId, row.sourceItemId]), row]));
  const raw = new Map([...projected].map(([key, row]) => [key, obj(row.item)]));
  const view = timelineView(client), rows: ThreadRow[] = [];
  for (const message of source) {
    const item = raw.get(message.id), user = message.kind === 'user';
    // Shared desktop renders '(empty response)'; mobile deliberately skips blank assistant rows.
    const body = message.kind === 'assistant' && item ? str(item.text) : message.body;
    const media = arr(item?.attachments).filter(attachment => str(attachment.id)).map(attachment => ({ id: str(attachment.id), name: str(attachment.name),
      kind: attachment.type === 'image' ? 'image' : /^video\//.test(str(attachment.mimeType)) ? 'video' : 'file',
      url: cachedAttachmentUrl(client, str(attachment.id)) ?? '' }));
    if (message.kind === 'assistant' && !body.trim() && !media.length) continue;
    const toggleOp = message.kind === 'work' ? 'chatlocal:fold' : message.kind === 'attempt' ? 'chatlocal:attempt'
      : ['group', 'live', 'thinking'].includes(message.kind) && message.groupId ? 'chatlocal:group' : '';
    const showMeta = user || ['assistant', 'plan'].includes(message.kind) && message.meta !== false && message.completed === true && message.streaming !== true;
    const title = message.kind === 'working' && message.startedMs ? `${message.title} ${elapsed(now - message.startedMs)}` : message.title;
    rows.push({ id: message.id, kind: message.kind, title, body, blocks: mobileThreadBlocks(body, dark), user,
      timestamp: mobileMessageTime(user ? item?.startedAt ?? message.createdAt : item?.updatedAt ?? message.createdAt), showMeta,
      streaming: message.streaming === true, attribution: message.attribution === 'automation' ? 'Sent by automation' : message.attribution === 'agent' ? 'Sent by agent' : '',
      intent: message.intent ?? '', copied: (view.copies.get(message.id)?.nonce ?? 0) > 0 && view.copies.get(message.id)?.ok !== false,
      expanded: message.expanded === true, toggleOp, toggleId: message.groupId ?? message.runId ?? '', failed: message.failed === true,
      live: message.live === true, media, first: false, last: false,
      activities: (['group', 'live'].includes(message.kind) ? [] : message.activities ?? []).map(activity => {
        const shown = mobileThreadActivity(activity, projected.get(activity.id), client, now, dark, mobileMessageTime(activity.timestamp));
        // Parse only visible reasoning; tools keep literal command/result output.
        return { ...shown, reasoningBlocks: shown.reasoning && shown.expanded ? mobileThreadBlocks(shown.output, dark) : [] };
      }) });
  }
  rows.forEach((row, index) => { row.first = index === 0; row.last = index === rows.length - 1; }); return rows;
}
function elapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return seconds < 60 ? `${seconds}s` : seconds < 3600 ? `${Math.floor(seconds / 60)}m ${seconds % 60}s` : `${Math.floor(seconds / 3600)}h ${Math.floor(seconds % 3600 / 60)}m`;
}

export function mobileThreadComposer(client: T3Client): ThreadComposerState {
  const run = activeRun(client.projection), running = !!run, queue = queueState(client.projection), edit = mobileQueuedEditPresentation(client), target = mobileComposerTarget(client);
  const provider = arr(client.config.providers).find(provider => provider.instanceId === client.providerId);
  const model = arr(provider?.models).find(model => model.slug === client.modelId), modelReady = !!provider && providerAvailable(provider) && !!model;
  const modelUnavailable = client.connection === 'connected' && !modelReady;
  const requests = requestPresentation(client), followUp = running && !queue.canSteer ? 'queue' : followUpBehavior(client);
  const action = primaryAction(client, threadPhase(client.projection));
  const blockedReason = edit.saving ? 'Saving…' : edit.uncertain ? 'Resolve the pending queued edit before sending.' : client.pending?.uncertain ? 'Check the synchronized thread before retrying.'
    : edit.editing ? '' : requests.approvals.length ? 'Resolve this approval request to continue.' : requests.questions.length ? 'Answer the pending question to continue.'
    : action.sendStatus;
  const canOperate = client.writable, canStop = canOperate && !client.pending && !client.busy && running;
  // Offline-outbox admission is not the desktop transport's contract; preserve its real refusal until the mobile outbox exists.
  const canSend = edit.editing ? edit.canSave && modelReady : canOperate && !client.pending && !client.busy && modelReady && !!client.projectId && !blockedReason && (!!client.draft.trim() || client.snapshotDrafts.length > 0);
  const sendLabel = edit.editing ? 'Update queued message' : running ? followUp === 'steer' ? 'Steer' : 'Queue' : 'Send';
  return { editing: edit.editing, saving: edit.saving, canCancel: edit.canCancel, editNotice: edit.error, editPendingId: edit.pendingId, canRetryEdit: edit.canRetry, contentOwner: target.owner, draft: mobileComposerTargetText(client, target) ?? '', placeholder: 'Ask the repo agent, or run a command…', canSend, canStop, showStop: !client.draft.trim() && client.snapshotDrafts.length === 0 && canStop && !edit.editing,
    canOperate, showReadOnlyNotice: client.connection === 'connected' && !canOperate, sendLabel, sendSymbol: edit.editing ? 'checkmark' : running ? followUp === 'steer' ? 'arrow.turn.left.up' : 'list.number' : 'arrow.up',
    blockedReason, modelLabel: str(model?.name, client.modelId), providerDriver: str(provider?.driver), providerIconURL: mobileProviderIconURL(provider?.iconUrl),
    modelUnavailable, running, queueCount: queue.queued.length };
}

/** Read after mobileSnapshot refresh; supplied time owns all duration labels. */
export function mobileThread(now: number, dark = false, client: T3Client = mobileClient): ThreadSnapshot {
  const rows = mobileThreadRows(client, now, dark), loaded = !!client.thread;
  const loading = !loaded && ['connected', 'connecting', 'reconnecting'].includes(client.connection);
  const requests = requestPresentation(client), view = timelineView(client);
  const answerFiles = mobileAnswerFilesRequest(client, visibleAnswerFiles(rows), now);
  return { answerFilesOwner: answerFiles.owner, answerFilesRequest: answerFiles.request, revision: client.revision, environmentId: client.environmentId, threadId: client.threadId,
    title: str(obj(client.projection.thread).title), loaded, loading, rows,
    emptyTitle: loaded ? 'No conversation yet' : loading ? '' : 'Messages not cached',
    emptyDetail: loaded ? 'Ask the agent to inspect the repo, run a command, or continue the active thread.' : loading ? '' : 'Reconnect this environment to load the conversation.',
    error: requests.error, uncertain: client.pending?.uncertain === true, hasMore: client.thread?.hasMore === true,
    historyLoading: client.historyLoading, historyError: view.historyError.get(client.threadId) ?? '', readsNeeded: timelineReadsNeeded(client),
    approvals: requests.approvals.map(approval => {
      const item = arr(client.projection.turnItems).slice().reverse().find(item => item.requestId === approval.id);
      const request = arr(client.projection.runtimeRequests).find(request => request.id === approval.id);
      // Mobile preserves server option order and has its own three-option fallback.
      const options = Array.isArray(item?.options) ? arr(item.options) : [
        { decision: 'accept', label: 'Allow once' }, { decision: 'acceptForSession', label: 'Allow session' }, { decision: 'decline', label: 'Decline' }];
      return { id: approval.id, title: approval.appName || str(item?.requestKind, str(request?.kind)), detail: approval.detail,
        disabled: !approval.canRespond || approval.responding, reason: approval.responseReason,
        options: options.map(option => ({ id: str(option.decision), label: str(option.label), warning: str(option.warning),
          tone: option.decision === 'accept' ? 'primary' : option.decision === 'decline' ? 'danger' : 'secondary' })) };
    }), composer: mobileThreadComposer(client) };
}

/** Root owns this awaited resource/mutation; native handles are never saved between answers. */
export async function mobileThreadPrepare(nativeInput: Native | null | undefined, now: number) {
  const native = nativeInput?.available ? letGoAware(mobileNative(nativeInput)) : nativeInput;
  if (native?.available && mobileClient.ready && mobileClient.threadId) {
    await syncWorktreeSetup(mobileClient, native);
    await refreshTimelineReads(mobileClient, native);
    await attachmentUrls(mobileClient, native, now);
  }
  return { revision: mobileClient.revision, pending: timelineReadsNeeded(mobileClient) };
}

/** Same real command seam as root; no manufactured success or alternate reducer. */
export function mobileThreadAction(args: unknown[], native: Native | null | undefined, storage: Files) {
  return mobileCommand(args, native, storage);
}

function visibleAnswerFiles(rows: ThreadRow[]): string[] {
  return rows.flatMap(row => row.activities.flatMap(activity => activity.expanded
    ? activity.answerHistory.flatMap(question => question.files.map(file => file.id)) : []));
}
/** A dedicated root mutation owns this request; transcript rendering stays pure. */
export async function mobileThreadAnswerFilesPrepare(owner: string, request: string, now: number,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const native = nativeInput?.available ? letGoAware(mobileNative(nativeInput)) : nativeInput;
  await mobilePrepareAnswerFiles(client, native, now, owner, request, visibleAnswerFiles(mobileThreadRows(client, now)));
  return { revision: client.revision };
}
