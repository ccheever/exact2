import { notePreviewHover } from './pages-pr-links'; // pr-links-previews-and-routing
import { runnableShellCommands } from './terminal-integrations';
import { runTerminalCommand } from './terminal-drawer-view';
import { toolActivityIconSources } from './timeline-tool-icons';
import { setTurnItemOpen, turnItemDetailView, turnItemIsOpen } from './timeline-item-fetch';
import { turnItemHasDetail, turnItemNeedsDetailFetch } from './timeline-item-detail';
import { arr, num, obj, str, type Activity, type Message, type Obj } from './domain';
import { ClientError, activeRun, type Files, type Native } from './protocol';
import type { T3Client } from './client';
import { deriveRows, type Row } from './timeline-rows';
import { displayFailed, entryDisplayLabel, entryIcon, inspectorDetail, isToolLike, thoughtText, workspaceRelativePath, type WorkEntry } from './timeline-worklog';
import { eventRow } from './timeline-events';
import { messageCodeBlocks } from './timeline-highlight';
import { planAction, planBody, planCollapsible, planSaveView, proposedPlanTitle } from './timeline-plan';
import { treeRows } from './timeline-tree';
import { cachedAttachmentUrl, forgetAttachmentUrl, forgetMediaPreviewUrl, imagePreviewAction, imagePreviewView, messageAttachments } from './timeline-attachments';
import { revealCitation } from './r5-composer-citation';
import { jumpToTurn, minimapCurrent, minimapItems, nativeTurns, type MinimapRow } from './timeline-minimap';
import { diagramPreviewAction, diagramPreviewView, messageDiagrams, retryMermaid } from './timeline-mermaid';
import { setupView, threadWorktreeSetup, worktreeDetailsOpen, worktreeSetupAction } from './timeline-worktree';
import { chippedAttachmentIds, markdownEnv, messageChips } from './r4-timeline-chips';
import { hasQuestionAnswer, notificationSubagent, plainOutput, questionAnswerPreview, questionHistory, questionTextPreview, runlessWorkStartedAt, type InspectorCode } from './timeline-inspect';
import { gitChatLocal } from './r4-git-route';
import { openFileSurface, surfaceLocal } from './r4-surfaces-panel';
import { mediaLocal } from './media-views'; // media-actions: the media menu, a video's error and Retry
import { closeTableMenu, tableMenuAction } from './r8-keys-table-menu'; // lane r8-keys: the table Copy popup
import { preparationFailureRunId, retryableActivities } from './r11-upstream-retry'; // lane r11-upstream: Retry a failed workspace preparation
import { numericDateFormatter, timestampFormatter } from './timestamp-format'; // desktop-shell-details: the host's locale
import { letGo } from './let-go';
import { pullRequestLinkMenu } from './context-menu-actions'; // context-menu-gaps
import { prCodeLocalFor, prLocalWrite, prUiLocal } from './pages-pr-detail'; // pr-writing-and-metadata, pr-header-actions-and-stacks, pr-code-tab
import { stackLayerTarget } from './pages-pr-stack';
import { openLinkFromUi } from './browser-links';

/** formatShortTimestamp: the wall-clock time alone, in the selected format. */
export function shortTime(value: unknown, format: string): string {
  const date = new Date(typeof value === 'string' ? value : '');
  if (!Number.isFinite(date.getTime())) return '';
  return timestampFormatter(format).format(date);
}

/** formatChatTimestampTooltip: "2:20 PM, 4th October 2026". */
export function timestampTooltip(value: unknown, format: string): string {
  const date = new Date(typeof value === 'string' ? value : '');
  if (!Number.isFinite(date.getTime())) return '';
  const day = date.getDate(), lastTwo = day % 100;
  const suffix = lastTwo >= 11 && lastTwo <= 13 ? 'th' : day % 10 === 1 ? 'st' : day % 10 === 2 ? 'nd' : day % 10 === 3 ? 'rd' : 'th';
  return `${shortTime(value, format)}, ${day}${suffix} ${new Intl.DateTimeFormat('en-US', { month: 'long' }).format(date)} ${date.getFullYear()}`;
}

/** MessagesTimeline's day-aware wall-clock label for the selected timestamp format. */
export function messageTime(value: unknown, now: number, format: string): string {
  const date = new Date(typeof value === 'string' ? value : '');
  if (!Number.isFinite(date.getTime())) return '';
  const time = timestampFormatter(format).format(date);
  const today = new Date(now);
  const dayDiff = Math.round((new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime()
    - new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()) / 86_400_000);
  if (dayDiff <= 0) return time;
  if (dayDiff === 1) return `yesterday at ${time}`;
  return `${numericDateFormatter(date.getFullYear() !== today.getFullYear()).format(date)} ${time}`;
}

interface ViewState { open: Set<string>; copies: Map<string, { nonce: number; ok: boolean }>; nonce: number; codeCopy: { threadId: string; text: string; nonce: number };
  historyError: Map<string, string>; revert: { threadId: string; messageId: string; turnCount: number } | null;
  treeAll: Set<string>; treeDirs: Map<string, boolean>; lastRows: { threadId: string; rows: MinimapRow[] } }
const views = new WeakMap<T3Client, ViewState>();
export function timelineView(client: T3Client): ViewState {
  let view = views.get(client);
  if (!view) { view = { open: new Set(), copies: new Map(), nonce: 0, codeCopy: { threadId: '', text: '', nonce: 0 }, historyError: new Map(), revert: null, treeAll: new Set(), treeDirs: new Map(), lastRows: { threadId: '', rows: [] } }; views.set(client, view); }
  return view;
}
const scoped = (client: T3Client, kind: string, id: string) => `${client.threadId}\u0000${kind}\u0000${id}`;

/**
 * Local timeline actions: T3 keeps fold, attempt and work-group disclosure in
 * view state (expandedRunIds, expandedAttemptIds, expandedWorkGroupIds), plus
 * Copy message / Copy code with their anchored feedback.
 */
export async function chatLocal(client: T3Client, native: Native, op: string, id: string, value: string, storage?: Files): Promise<string> {
  if (op === 'run-terminal' && storage) { await runTerminalCommand(client, native, storage, value.trim()); return ''; }
  if (op.startsWith('surface-')) return surfaceLocal(client, native, op.slice(8), id, value); // r4-surfaces: the right panel's surfaces (window chatLocal)
  if (op === 'link-open') return openLinkFromUi(client, native, id, value); // browser-surface part 5: "Open links in" (browser-links.ts)
  if (op === 'pr-link-menu') { await pullRequestLinkMenu(client, native, value); return ''; } // context-menu-gaps: the detail header's number
  if (op === 'pr-preview') { notePreviewHover(client, id, value === '1'); return ''; } // pr-links-previews-and-routing: a pull request link's hover card
  if (op.startsWith('prw-')) return prLocalWrite(client, native, op.slice(4), id, value); // pr-writing-and-metadata: the composer's drafts (pages-pr-detail.ts)
  if (op === 'pr-ui-select') return surfaceLocal(client, native, 'r5-pr-open', stackLayerTarget(value), ''); // pr-header-actions-and-stacks: a stack layer beside a thread
  if (op.startsWith('pr-ui-')) return prUiLocal(client, op.slice(6), id, value); // pr-header-actions-and-stacks: the panel header's menu choices
  if (op.startsWith('pr-code-')) return prCodeLocalFor(client, native, op.slice(8), id, value); // pr-code-tab: the Code tab's presses (pages-pr-code.ts)
  if (op.startsWith('git-')) return gitChatLocal(client, native, op.slice(4), id, value); // lane r4-git (r4-git-route.ts)
  if (op.startsWith('media-')) return mediaLocal(client, native, op.slice(6), id, value, { urlOf: attachmentId => cachedAttachmentUrl(client, attachmentId), // media-actions (media-views.ts)
    openFile: relativePath => openFileSurface(client, native, relativePath, 0), forgetAttachment: attachmentId => { if (!forgetMediaPreviewUrl(client, attachmentId)) forgetAttachmentUrl(client, attachmentId); } });
  const view = timelineView(client);
  if (op === 'item-detail') { setTurnItemOpen(client, id, value === 'open'); return ''; }
  if (op === 'history') {
    const threadId = client.threadId;
    view.historyError.delete(threadId);
    try { await client.history(native); }
    catch (error) { if (letGo(error)) throw error; view.historyError.set(threadId, error instanceof Error && error.message ? error.message : 'Could not load earlier turns.'); }
    return '';
  }
  if (op.startsWith('plan-')) return planAction(client, native, op, id, value);
  if (op === 'tree-all' || op === 'tree-dir') return toggleTree(client, view, op, id, value);
  if (op.startsWith('image-')) return imagePreviewAction(client, op, id, value);
  if (op === 'jump') return jumpToTurn(client, native, id, value, view.lastRows.threadId === client.threadId ? view.lastRows.rows : []);
  if (op === 'revert') return askRevert(client, view, id, Number(value));
  if (op === 'revert-answer') return answerRevert(client, native, view, value, storage);
  if (op === 'diagram-retry') return retryMermaid(value);
  if (op === 'setup-details' || op === 'setup-cancel' || op === 'setup-terminal') return worktreeSetupAction(client, native, op);
  if (op === 'diagram-open' || op === 'diagram-close') return diagramPreviewAction(client, op, value);
  // r4-timeline: a table's Copy as Markdown / CSV (the check morph, no toast) and a thread chip's open.
  if (op === 'table-menu' || op === 'table-menu-close') return tableMenuAction(client, op, id, value, native);
  if (op === 'copy-table') {
    closeTableMenu(client);
    if (!value) throw new ClientError('That table is unavailable.');
    await chatLocal(client, native, 'copy-code', id, value, storage);
    return '';
  }
  // r5-composer: View source opens the cited thread and scrolls its answer into place (r5-composer-citation.ts).
  if (op === 'chip-citation') return revealCitation(client, native, id, value, () => view.lastRows.threadId === client.threadId ? view.lastRows.rows : []);
  if (op === 'chip-thread') {
    if (!id || !client.shell.threads.some(thread => thread.id === id)) throw new ClientError('Thread no longer available');
    await client.openSelected(native, id);
    return '';
  }
  if (op === 'copy-code' || op === 'copy') {
    const text = op === 'copy-code' ? value : copyText(client, id);
    if (!text || text.length > 1_000_000) throw new ClientError(op === 'copy-code' ? 'That code block is unavailable.' : 'That message is no longer available.');
    const key = op === 'copy' ? id : `code:${id}`;
    try { await client.restAccess(native).call({ op: 'copyText', text }); }
    catch (error) { if (!letGo(error)) view.copies.set(key, { nonce: ++view.nonce, ok: false }); throw error; }
    view.copies.set(key, { nonce: ++view.nonce, ok: true });
    if (op === 'copy-code') view.codeCopy = { threadId: client.threadId, text, nonce: view.nonce };
    return op === 'copy' ? 'Copied message' : 'Copied code';
  }
  if (!['fold', 'group', 'attempt'].includes(op)) throw new ClientError(`Unknown timeline action: ${op}`);
  const key = scoped(client, op, id);
  if (view.open.has(key)) view.open.delete(key); else view.open.add(key);
  return '';
}
function copyText(client: T3Client, id: string): string {
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === id);
  const item = obj(row?.item);
  return item.type === 'proposed_plan' ? str(item.markdown) : ['user_message', 'assistant_message'].includes(str(item.type)) ? str(item.text) : '';
}

/**
 * ChangedFilesCard: "Expand all folders" flips the card (per thread and run)
 * and forgets folder overrides; a folder's own toggle overrides that state.
 */
function toggleTree(client: T3Client, view: ViewState, op: string, runKey: string, path: string): string {
  if (!runKey) throw new ClientError('Those changes are no longer available.');
  const key = scoped(client, 'tree', runKey);
  if (op === 'tree-all') {
    if (view.treeAll.has(key)) view.treeAll.delete(key); else view.treeAll.add(key);
    for (const dir of [...view.treeDirs.keys()]) if (dir.startsWith(`${key}\u0000`)) view.treeDirs.delete(dir);
    return '';
  }
  if (!path) throw new ClientError('That folder is no longer available.');
  const dir = `${key}\u0000${path}`;
  view.treeDirs.set(dir, !(view.treeDirs.get(dir) ?? view.treeAll.has(key)));
  return '';
}
function treeState(client: T3Client, runKey: string): { all: boolean; overrides: Map<string, boolean> } {
  const view = timelineView(client), key = scoped(client, 'tree', runKey), overrides = new Map<string, boolean>();
  for (const [dir, open] of view.treeDirs) if (dir.startsWith(`${key}\u0000`)) overrides.set(dir.slice(key.length + 1), open);
  return { all: view.treeAll.has(key), overrides };
}

/** ChatView onRevertToTurnCount: refuse while working or unsupported, otherwise ask first. */
function askRevert(client: T3Client, view: ViewState, messageId: string, turnCount: number): string {
  if (!Number.isInteger(turnCount) || turnCount < 0) throw new ClientError('That message can no longer be edited.');
  const provider = arr(client.config.providers).find(provider => provider.instanceId === obj(obj(client.projection.thread).modelSelection).instanceId);
  if (provider?.supportsConversationRollback === false) throw new ClientError('This provider does not support reverting conversation history. Start a new thread instead.');
  if (activeRun(client.projection)) throw new ClientError('Interrupt the current turn before reverting checkpoints.');
  view.revert = { threadId: client.threadId, messageId, turnCount };
  return '';
}
/** "Revert files too" or "Revert and keep changes": roll back to the turn's checkpoint, then return the prompt to the composer. */
async function answerRevert(client: T3Client, native: Native, view: ViewState, choice: string, storage?: Files): Promise<string> {
  const pending = view.revert;
  view.revert = null;
  if (choice === 'cancel' || !pending) return '';
  if (pending.threadId !== client.threadId) throw new ClientError('That thread is no longer selected.');
  if (activeRun(client.projection)) throw new ClientError('Interrupt the current turn before reverting checkpoints.');
  if (!storage) throw new ClientError('Edit from here is unavailable.');
  const checkpoints = arr(client.projection.checkpoints);
  const checkpoint = [...checkpoints].reverse().find(candidate => pending.turnCount === 0
    ? candidate.ordinalWithinScope === 0 && (candidate.appRunOrdinal === null || candidate.appRunOrdinal === undefined) : candidate.appRunOrdinal === pending.turnCount);
  if (!checkpoint || checkpoint.status !== 'ready') throw new ClientError(`Checkpoint for run ordinal ${pending.turnCount} is unavailable.`);
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === pending.messageId);
  if (!row) throw new ClientError('The message to rewind is no longer available.');
  const text = str(obj(row.item).text), messageRunId = str(obj(row.item).runId), threadId = client.threadId;
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.dispatch(storage, { type: 'checkpoint.rollback', commandId, threadId, scopeId: str(checkpoint.scopeId),
    checkpointId: str(checkpoint.id), restoreFiles: choice === 'files' }, 'Edit from here');
  // waitForRevertedMessage: the prompt returns only once the run is rolled back;
  // the thread's rollbackFailure for this command becomes the thread error.
  await settleRollback(client, native, storage, threadId, String(commandId), messageRunId, pending.turnCount);
  const current = client.local.drafts[client.draftKey] ?? '';
  const restored = text.trim() ? text : '';
  if (restored) client.local.drafts[client.draftKey] = current ? `${current}\n\n${restored}` : restored;
  return choice === 'files' ? 'Reverted files and conversation' : 'Reverted conversation';
}

/** Polls the projection for the rollback's outcome, pulling events every third pause, two minutes at most. */
export async function settleRollback(client: T3Client, native: Native, storage: Files, threadId: string, commandId: string, runId: string, turnCount: number): Promise<void> {
  for (let attempt = 0; attempt < 600; attempt++) {
    if (attempt % 3 === 2) await client.refresh(native, storage);
    if (client.threadId !== threadId) throw new ClientError('That thread is no longer selected.');
    const failure = obj(obj(client.projection.thread).rollbackFailure);
    if (str(failure.requestId) === commandId) throw new ClientError(str(failure.message, 'Failed to revert thread state.'));
    if (arr(client.projection.runs).some(run => run.id === runId && Number(run.ordinal) > turnCount && run.status === 'rolled_back')) return;
    await client.restAccess(native).call({ op: 'timelineSleep', ms: 200 });
  }
  throw new ClientError('Timed out waiting for the thread to rewind.');
}

/** Timeline state beside its rows: the history control's error and the Edit from here dialog. */
export function timelineSnapshot(client: T3Client) {
  const view = timelineView(client);
  const native = nativeTurns(client), rows = view.lastRows.threadId === client.threadId ? view.lastRows.rows : [];
  const minimap = minimapItems(rows, native.inView);
  return { historyError: view.historyError.get(client.threadId) ?? '',
    revertPending: view.revert?.threadId === client.threadId ? view.revert.messageId : '', ...planSaveView(client),
    minimap: minimap.map(({ row: _row, ...item }) => item), minimapCurrent: minimapCurrent(minimap, native.above), ...imagePreviewView(client), ...diagramPreviewView(client) };
}

/** The working root that path labels are made relative to (filePathDisplay). */
export function workspaceRoot(client: T3Client): string {
  const thread = obj(client.projection.thread);
  const project = client.shell.projects.find(project => project.id === (thread.projectId ?? client.projectId));
  return str(thread.worktreePath) || str(project?.workspaceRoot);
}

function activity(entry: WorkEntry, root: string, client: T3Client, label?: string): Activity {
  const fetches = turnItemNeedsDetailFetch(entry.item);
  const detail = turnItemDetailView(client, entry.item, undefined, entry.id);
  entry = { ...entry, item: detail.item };
  const item = entry.item, warning = entry.sourceActivityKind === 'runtime.warning';
  const failedError = item.type === 'error' && item.status === 'failed';
  const failed = !warning && displayFailed(entry);
  const severe = failed && (entry.itemType === 'error' || !isToolLike(entry));
  const plain = plainOutput(entry, root);
  const inspect = plain === undefined ? inspectorDetail(entry, root) : { input: '', result: '', ok: true };
  const reasoning = entry.itemType === 'reasoning';
  const usage = failedError && obj(item.failure).class === 'usage_limit';
  // An answered question leads with what was asked (even over a lone tool's label) and trails its answer.
  const question = entry.questionAnswer ? questionTextPreview(entry.questionAnswer) : '';
  const title = failedError ? usage ? 'Usage limit reached.' : entry.label
    : reasoning ? thoughtText(entry.detail ?? '') || (entry.status === 'inProgress' ? 'Thinking' : 'Thought') : question || (label ?? entryDisplayLabel(entry, root));
  // Reads and skills expand to plain text (output) instead of the inspector (body).
  return { id: entry.id, label: title, body: entry.questionAnswer ? '' : plain !== undefined && fetches ? plain ?? '' : inspect.input, icon: failedError || warning || severe ? 'circle-alert' : entryIcon(entry),
    output: reasoning ? str(entry.detail) : fetches ? detail.text : plain ?? detail.text, outputState: detail.state, detailOpen: turnItemIsOpen(client, entry.id), ...toolActivityIconSources(client, item), result: inspect.result, failed, timestamp: entry.createdAt,
    tone: failedError ? usage ? 'provider-warning' : 'provider-error' : warning ? 'warning' : severe ? 'error' : failed ? 'failed' : '',
    ok: inspect.ok, reasoning, expandable: !failedError && (!!entry.questionAnswer || (plain !== undefined ? !!plain || entry.item.outputOmitted === true || !!detail.text : turnItemHasDetail(entry.item))),
    detail: failedError && !usage ? str(obj(item.failure).message) : '', status: entry.status === 'inProgress' ? 'Thinking' : 'Thought',
    targetId: entry.itemType === 'thread_created' ? str(item.targetThreadId) : '', retryRunId: preparationFailureRunId(item), // lane r11-upstream (737993303d)
    answer: entry.questionAnswer && hasQuestionAnswer(entry.questionAnswer) ? questionAnswerPreview(entry.questionAnswer) : '' };
}

/** T3's timeline rows for the selected thread (MessagesTimeline.logic deriveMessagesTimelineRows). */
export function transcriptRows(client: T3Client): Message[] {
  const projection = client.projection, view = timelineView(client), root = workspaceRoot(client);
  const runs = arr(projection.runs), running = activeRun(projection), runless = runlessWorkStartedAt(projection);
  const setup = threadWorktreeSetup(client);
  const open = (kind: string) => new Set([...view.open].filter(key => key.startsWith(`${client.threadId}\u0000${kind}\u0000`)).map(key => key.split('\u0000')[2]!));
  const provider = arr(client.config.providers).find(provider => provider.instanceId === obj(obj(projection.thread).modelSelection).instanceId);
  const rows = deriveRows({ rows: client.thread ? arr(projection.visibleTurnItems) : [], runs, attempts: arr(projection.attempts), nodes: arr(projection.nodes),
    checkpoints: arr(projection.checkpoints), isWorking: !!running || runless !== '', runningRunId: str(running?.id), latestRun: runs[runs.length - 1] ?? null,
    // orchestrationV2RunWorkStartedAt: a wake run keeps the start of the work it continues.
    activeStartedAt: running ? str(running.workStartedAt ?? running.startedAt ?? running.requestedAt) : runless, runlessWorkActive: runless !== '', worktreeSetup: setup.snapshot,
    expandedRuns: open('fold'), expandedAttempts: open('attempt'), expandedGroups: open('group'), root, rollback: provider?.supportsConversationRollback !== false });
  const context: PresentContext = { client, root, view, subagents: arr(projection.subagents), format: client.local.deviceSettings.timestampFormat,
    preparing: setup.preparing, detailsOpen: worktreeDetailsOpen(client), threads: client.shell.threads };
  return rows.map(row => present(row, context));
}

interface PresentContext { client: T3Client; root: string; view: ViewState; subagents: Obj[]; format: string; preparing: boolean; detailsOpen: boolean; threads: Obj[] }
/** A changing one-line label keyed by its text, so the row redraws it instead of reusing the old string. */
const keyedLabel = (row: Row, label: string): Activity => ({ id: `${row.id}\u0000${label}`, label, body: '', icon: '', output: '', result: '', failed: false, timestamp: '' });
const entryCode = (entries: WorkEntry[]): InspectorCode[] => entries.flatMap(entry => {
  if (entry.questionAnswer) return [questionHistory(entry.id, entry.questionAnswer)];
  return [];
});

function present(row: Row, context: PresentContext): Message {
  const { root, view } = context;
  const base = { id: row.id, createdAt: row.createdAt, title: '', body: '' };
  switch (row.kind) {
    case 'fold': return { ...base, kind: 'work', title: row.label, runId: row.runId, groupId: row.runId, expanded: row.expanded };
    // While a worktree is prepared the header reads "Setting up worktree…" and the activity row keeps its height empty.
    case 'working': return { ...base, kind: 'working', title: context.preparing ? 'Setting up worktree…' : 'Working for', live: context.preparing, startedMs: Date.parse(row.createdAt) || 0 };
    case 'setup': return { ...base, kind: 'worktree-setup', title: 'Worktree setup', setup: [setupView(row.snapshot, row.embedded, context.detailsOpen)] };
    case 'thinking': return { ...base, kind: 'thinking', title: context.preparing ? '' : 'Thinking', icon: 'brain', live: true, continues: row.continues === true,
      groupId: row.groupId ?? '', expanded: row.expanded === true };
    case 'live': return { ...base, kind: 'live', title: row.label, icon: entryIcon(row.entry), failed: displayFailed(row.entry), live: row.active,
      expanded: row.expanded, groupId: row.groupId, continues: row.continues === true, runId: row.entry.runId, activities: [{ ...keyedLabel(row, row.label), ...toolActivityIconSources(context.client, row.entry.item) }] };
    case 'group': return { ...base, kind: 'group', title: row.summary, icon: row.icon, failed: row.failed, expanded: row.expanded, groupId: row.groupId,
      runId: row.runId, continues: row.continues === true, activities: [keyedLabel(row, row.summary)] };
    case 'details': {
      const activities = row.entries.map(entry => activity(entry, root, context.client));
      return { ...base, kind: 'details', activities, continues: row.continues === true, code: entryCode(row.entries) };
    }
    case 'entry': {
      const single = row.entries.length === 1 ? row.entries[0]! : null;
      const card = single?.itemType === 'notification' ? notificationSubagent(single.item, context.subagents, single.createdAt, shortTime(single.createdAt, context.format)) : null;
      if (card) return { ...base, kind: 'subagent', title: card.label, activities: [card], runId: single!.runId };
      const activities = row.entries.map(entry => activity(entry, root, context.client, row.entries.length === 1 ? row.label : undefined));
      return { ...base, kind: 'entry', activities, continues: row.continues === true, runId: row.entries[0]?.runId ?? '', code: entryCode(row.entries) };
    }
    case 'attempt': return { ...base, kind: 'attempt', title: 'Superseded attempt', detail: 'Partial output retained', expanded: row.expanded, groupId: row.attemptId, runId: row.runId };
    case 'compaction': return { ...base, kind: 'compaction', title: row.label, live: row.active, icon: 'minimize', activities: [keyedLabel(row, row.label)] };
    case 'plan': {
      const markdown = str(row.entry.item.markdown);
      return { ...base, kind: 'plan', title: proposedPlanTitle(markdown) ?? 'Proposed plan', body: planBody(markdown), runId: str(row.entry.item.runId),
        completed: row.entry.item.streaming !== true, streaming: row.entry.item.streaming === true, sourceThreadId: str(row.entry.row.sourceThreadId), meta: false, collapsible: planCollapsible(markdown), code: messageCodeBlocks(markdown),
        diagrams: messageDiagrams(markdown, row.entry.item.streaming === true),
        copied: view.copies.get(row.id)?.nonce ?? 0 };
    }
    case 'checkpoint': return { ...base, kind: 'checkpoint', title: 'Changes', checkpointId: str(row.entry.item.checkpointId), runId: str(row.entry.item.runId),
      sourceThreadId: str(row.entry.row.sourceThreadId), files: arr(row.entry.item.files).map(file => ({ path: str(file.path), additions: num(file.additions), deletions: num(file.deletions) })) };
    case 'event': return { ...base, ...eventRow(row.entry.item, row.subagents, row.entry.row) };
    case 'meta': return { ...base, kind: 'meta', body: str(row.entry.item.text), runId: str(row.entry.item.runId), completed: true,
      actionsId: row.entry.id, copied: view.copies.get(row.entry.id)?.nonce ?? 0, copyFailed: view.copies.get(row.entry.id)?.ok === false };
    case 'message': {
      const item = row.entry.item, copy = view.copies.get(row.id);
      const common = { ...base, runId: str(item.runId), sourceThreadId: str(row.entry.row.sourceThreadId), completed: item.status === 'completed',
        copied: copy?.nonce ?? 0, copyFailed: copy?.ok === false };
      if (row.entry.role === 'assistant') {
        const text = str(item.text);
        return { ...common, kind: 'assistant', title: 'Assistant', body: text || (item.streaming === true ? '' : '(empty response)'), chips: messageChips(item, context.root, context.threads, row.id),
          meta: row.meta, streaming: item.streaming === true || row.inProgress, code: messageCodeBlocks(text),
          diagrams: messageDiagrams(text, item.streaming === true || row.inProgress) };
      }
      const intent = str(item.inputIntent);
      const text = str(item.text), chips = messageChips(item, context.root, context.threads, row.id), attached = messageAttachments(item), chipped = chippedAttachmentIds(chips);
      // A file with a chip in the prose needs no standalone row (images always show).
      return { ...common, kind: 'user', title: 'You', body: text, revert: row.revert, images: attached.images, attachFiles: attached.attachFiles.filter(file => !chipped.has(file.id)), chips, code: messageCodeBlocks(text), diagrams: messageDiagrams(text, false),
        collapsible: text.trim().length > 0 && (text.length > 600 || text.split('\n').length > 8),
        intent: intent === 'queued_turn' ? 'Queued' : intent === 'steer' || intent === 'promoted_queued_to_steer' ? 'Steer' : '',
        intentTip: intent === 'queued_turn' ? 'Queued behind the active turn' : intent === 'promoted_queued_to_steer'
          ? 'Originally queued, then promoted to steer the active turn' : intent === 'steer' ? 'Steered the active turn' : '',
        attribution: item.createdBy === 'agent' ? 'agent' : item.createdBy === 'automation' || item.creationSource === 'scheduled_task' || item.scheduledTaskId ? 'automation' : '',
        targetId: str(item.senderThreadId), detail: str(item.scheduledTaskId),
        status: ['completed', 'pending', 'waiting'].includes(str(item.status)) ? '' : str(item.status) };
    }
  }
}

const blankActivity = (activity: Activity): Activity & Required<Pick<Activity, 'tone' | 'ok' | 'reasoning' | 'expandable' | 'detail' | 'status' | 'targetId' | 'answer' | 'retryRunId' | 'outputState' | 'detailOpen' | 'iconLight' | 'iconDark'>> & { timeTip: string } => ({
  tone: '', ok: true, reasoning: false, expandable: true, detail: '', status: '', targetId: '', answer: '', retryRunId: '', timeTip: '', ...activity,
  outputState: activity.outputState ?? '', detailOpen: activity.detailOpen ?? false, iconLight: activity.iconLight ?? '', iconDark: activity.iconDark ?? '' });

/**
 * Timeline rows as Contract draws them, every Message field present. T3 shows
 * a turn's changed files inside the final assistant row, above that row's
 * copy/fork/time actions; the client keeps checkpoints as their own rows, so an
 * attached checkpoint carries the preceding answer's actions and the answer omits its own.
 */
export function timelineMessages(client: T3Client, transcript: Message[], now: number) {
  const format = client.local.deviceSettings.timestampFormat, md = markdownEnv(client);
  const copy = timelineView(client).codeCopy, codeCopy = copy.threadId === client.threadId ? { codeCopied: copy.text, codeCopyNonce: copy.nonce } : { codeCopied: '', codeCopyNonce: 0 };
  const rows = transcript.filter(message => message.kind !== 'checkpoint' || !message.sourceThreadId || message.sourceThreadId === client.threadId).map(message => {
    const checkpoint = arr(client.projection.checkpoints).find(checkpoint => checkpoint.id === message.checkpointId);
    const files = message.files ?? [], tree = treeState(client, message.runId || message.checkpointId || '');
    return { id: message.id, kind: message.kind, title: message.title, body: message.body,
      checkpointId: message.checkpointId ?? '', checkpointOrdinal: num(checkpoint?.appRunOrdinal, -1),
      additions: files.reduce((sum, file) => sum + file.additions, 0), deletions: files.reduce((sum, file) => sum + file.deletions, 0),
      files: message.kind === 'checkpoint' ? treeRows(files, tree.all, tree.overrides) : [],
      runId: message.runId ?? '', sourceThreadId: message.sourceThreadId ?? '',
      canFork: (message.kind === 'assistant' || message.kind === 'meta') && message.completed === true && message.streaming !== true && message.local !== true,
      completed: message.completed === true, createdAt: message.createdAt ?? '', timestamp: messageTime(message.createdAt, now, format),
      timeTip: timestampTooltip(message.createdAt, format), actionsTimeTip: '', ...codeCopy,
      activities: retryableActivities(client, (message.activities ?? []).map(activity => ({ ...blankActivity({ ...activity, timestamp: messageTime(activity.timestamp, now, format) }), timeTip: timestampTooltip(activity.timestamp, format) }))),
      first: false, attached: false, hoverKey: message.id, actionsId: '', actionsFork: false, actionsTime: '', expanded: message.kind === 'checkpoint' ? tree.all : message.expanded === true, last: false,
      icon: message.icon ?? '', tone: message.tone ?? '', failed: message.failed === true, live: message.live === true,
      startedMs: message.startedMs ?? 0, detail: message.detail ?? '', status: message.status ?? '', groupId: message.groupId ?? '',
      continues: message.continues === true, revert: message.revert ?? -1, intent: message.intent ?? '', intentTip: message.intentTip ?? '',
      attribution: message.attribution ?? '', targetId: message.kind === 'checkpoint' ? files[0]?.path ?? '' : message.targetId ?? '', actionLabel: message.actionLabel ?? '',
      copied: message.copied ?? 0, copyFailed: message.copyFailed === true, meta: message.meta !== false,
      collapsible: message.collapsible === true, code: message.code ?? [], diagrams: message.diagrams ?? [], setup: message.setup ?? [], fileCount: message.kind === 'checkpoint' ? files.length : 0,
      images: message.images ?? [], attachFiles: message.attachFiles ?? [], md: { ...md, chips: message.chips ?? [], runCommands: client.projectId ? runnableShellCommands(str(message.body), message.streaming === true) : [] } };
  });
  timelineView(client).lastRows = { threadId: client.threadId, rows: rows.map(row => ({ id: row.id, kind: row.kind, body: row.body })) };
  rows.forEach((row, index) => {
    row.first = index === 0;
    row.last = index === rows.length - 1;
    if (row.kind === 'meta' || (row.kind === 'assistant' || row.kind === 'plan') && row.meta && row.completed) {
      row.actionsId = row.kind === 'meta' ? transcript.find(message => message.id === row.id)?.actionsId ?? row.id : row.id;
      row.actionsFork = row.canFork; row.actionsTime = row.timestamp; row.actionsTimeTip = row.timeTip;
    }
    const previous = rows[index - 1];
    if (row.kind === 'checkpoint' && row.fileCount > 0 && previous && (previous.kind === 'assistant' || previous.kind === 'plan')
      && (!row.runId || !previous.runId || row.runId === previous.runId)) {
      row.attached = true; row.hoverKey = previous.hoverKey;
      row.actionsId = previous.actionsId; row.actionsFork = previous.actionsFork; row.actionsTime = previous.actionsTime; row.actionsTimeTip = previous.actionsTimeTip;
      row.copied = previous.copied; row.copyFailed = previous.copyFailed;
      previous.actionsId = ''; previous.actionsFork = false; previous.actionsTime = '';
    }
  });
  return rows;
}

export { workspaceRelativePath };
