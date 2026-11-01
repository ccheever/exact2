import { replArguments } from './tool-activity';
import { commandOutputView, commandSpans } from './command-output';
import { terminalOutputPlainText } from './terminal-output';
import { toolImages } from './tool-images';
import { secretCard } from './secret-card';
import { focusDiffFind } from '../diff/diff-find';
import { threadFindAction, threadFindExpanded, threadFindView } from './thread-find-runtime';
// Message presentation adapted from T3 Code 9c161462c9 (MIT; see LICENSE-T3):
// apps/web/src/components/chat/MessagesTimeline.tsx and packages/client-runtime/src/userMessage.ts.
import { copyPromptLink, promptLinkFor } from '../prompt-links/copy-prompt-link';
import { notePreviewHover } from '../pull-requests/pages-pr-links';
import { openThreadLinkTarget, relabelThreadLinks } from './thread-links';
import { renderCodexInlineDirectivesAsMarkdown } from '../highlight/codex-directives.js';
import { markdownThreadGlyphs } from './markdown-thread-glyphs';
import { shellCommand } from '../highlight/shell-command';
import { openPullRequest } from '../pull-requests/pr-chip-open';
import { runnableShellCommands } from '../terminal/terminal-integrations';
import { runTerminalCommand } from '../terminal/terminal-drawer-view';
import { toolActivityIconSources } from './timeline-tool-icons';
import { setTurnItemOpen, turnItemDetailView, turnItemIsOpen } from './timeline-item-fetch';
import { turnItemHasDetail, turnItemNeedsDetailFetch } from './timeline-item-detail';
import { arr, checkpointFileTotals, num, obj, str, type Activity, type Message, type Obj } from '../client/domain';
import { ClientError, activeRun, type Files, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { latestUnheldRun } from './worktree-card-run';
import { availableRevertTurnCounts, deriveRows, findRevertCheckpoint, rowKey, runTurnEnded, turnFoldRunIds, type Row, type RowInput } from './timeline-rows';
import { displayFailed, entryDisplayLabel, entryIcon, inspectorDetail, isToolLike, liveThoughtLine, thoughtText, threadReadLabel, workspaceRelativePath, type WorkEntry } from './timeline-worklog';
import { eventRow } from './timeline-events';
import { codeTokens, messageCodeBlocks } from '../highlight/timeline-highlight';
import { planAction, planBody, planCollapsible, planSaveView, proposedPlanTitle } from './timeline-plan';
import { treeRows } from './timeline-tree';
import { openInlineDocumentImage, cachedAttachmentUrl, forgetAttachmentUrl, forgetMediaPreviewUrl, imagePreviewAction, imagePreviewView, isSentSvg, messageAttachments, noteChipPreviewFailed, svgPreviewFailed, userMessageShape, withoutSoundReferences } from './timeline-attachments';
import { revealCitation } from './citation-source';
import { minimapItems, type MinimapRow } from './timeline-minimap';
import { diagramPreviewAction, diagramPreviewView, messageDiagrams, noteMermaidFrame, retryMermaid } from './timeline-mermaid';
import { mathFrames, noteMathFrame } from './markdown-math';
import { agentStarted, setupView, threadWorktreeSetup, worktreeDetailsOpen, worktreeSetupAction } from './timeline-worktree';
import { asFileChip, chippedAttachmentIds, markdownChipInks, markdownEnv, messageChips, shouldCollapseUserMessage } from './message-chips';
import { hasQuestionAnswer, notificationChildThreadId, notificationSubagent, plainOutput, questionAnswerPreview, questionHistory, questionTextPreview, runlessWorkStartedAt, type InspectorCode } from './timeline-inspect';
import { gitChatLocal } from '../details/git-route';
import { openFileSurface, surfaceLocal } from '../panel/panel-surfaces';
import { mediaLocal } from '../media/media-views'; // the media menu, a video's error and Retry
import { closeTableMenu, tableMenuAction } from './table-menu'; // the table Copy popup
import { noteRefScrollEnd } from '../composer/ref-paging';
import { isCompacting } from '../composer/composer-controls';
import { workflowOpening, workflowOpeningMessage, type WorkflowOpening } from './timeline-workflow-opening';
import { preparationFailureRunId, retryableActivities } from './workspace-retry'; // Retry a failed workspace preparation
import { numericDateFormatter, timestampFormatter } from '../shared/timestamp-format'; // the host's locale
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { CHATGPT_WORKSPACE_ADMIN_REQUEST } from './timeline-errors';
import { autoDirection } from '../sidebar/title-direction';
import { pullRequestLinkPick } from '../panel/context-menu-actions';
import { prGuideLocalFor, prCodeLocalFor, prLocalWrite, prUiLocal } from '../pull-requests/pages-pr-detail';
import { stackLayerTarget } from '../pull-requests/pages-pr-stack';
import { openLinkFromUi } from '../browser/browser-links';
import { chatExternalLinkPick } from './external-link-menu';
import { withFileLinkMenus } from './file-link-menu';
import { assistantCanFork, inspectorExtras, NO_INSPECTOR_EXTRAS } from './timeline-work-rows';
import { completeSave } from '../media/save-file';
import { inlineDocumentKey, inlineDocumentView, inlineDocumentEvent } from './inline-documents';
import { keptThread } from '../client/keep-alive';
import { foldedMessageSummary, plainContextReferences, userMessagePresentation } from './user-message';
import { liveEnvironment } from '../client/live-streams';
import { noteAudioPlaying } from './timeline-kept-rows';
import { sleep } from '../client/command-waits';
import { timelinePendingApprovals, type TimelineApprovalView } from './timeline-approvals';

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

interface ViewState { open: Set<string>; copies: Map<string, { nonce: number }>; nonce: number; codeCopy: { threadId: string; text: string; nonce: number };
  historyError: Map<string, string>; revert: { threadId: string; messageId: string; turnCount: number } | null;
  treeAll: Set<string>; treeDirs: Map<string, boolean>; lastRows: { threadId: string; rows: MinimapRow[] };
  /** The View source placing its answer (citation-source.ts), else 0: ChatColumn restores no remembered position meanwhile. A
   *  newer View source lets the older one go where it waits, on its own timer, so each one clears only its own. */
  citing: number; citeFlows: number;
  /** The answer row the last View source asked ChatColumn to place, on its thread; each request a new nonce. */
  citeJump: { threadId: string; key: string; nonce: number };
  /** The thread whose folded messages `open` holds: another thread's start closed, as their rows' state is. */
  foldsThread: string;
  /** The chat header's rename field's text on its thread, while it is edited (headerRenameDir). */
  renameDraft: { threadId: string; text: string } | null }
const views = new WeakMap<T3Client, ViewState>();
export function timelineView(client: T3Client): ViewState {
  let view = views.get(client);
  if (!view) { view = { open: new Set(), copies: new Map(), nonce: 0, codeCopy: { threadId: '', text: '', nonce: 0 }, historyError: new Map(), revert: null, treeAll: new Set(), treeDirs: new Map(), lastRows: { threadId: '', rows: [] }, citing: 0, citeFlows: 0, citeJump: { threadId: '', key: '', nonce: 0 }, foldsThread: '', renameDraft: null }; views.set(client, view); }
  return view;
}
const scoped = (client: T3Client, kind: string, id: string) => `${client.threadId}\u0000${kind}\u0000${id}`;
/** ChatHeader's rename field is `dir="auto"`: the direction of the text being typed, else of the title it starts from. */
export function headerRenameDir(client: T3Client, title: string): string {
  const draft = timelineView(client).renameDraft;
  return autoDirection(draft && draft.threadId === client.threadId ? draft.text : title) ?? 'ltr';
}

/**
 * Local timeline actions: T3 keeps fold, attempt and work-group disclosure in
 * view state (expandedRunIds, expandedAttemptIds, expandedWorkGroupIds), plus
 * Copy message / Copy code with their anchored feedback.
 */
export async function chatLocal(client: T3Client, native: Native, op: string, id: string, value: string, storage?: Files): Promise<string> {
  if (op === 'save-complete') { completeSave(client, Number(id)); return ''; }
  if (op.startsWith('inline-doc-')) { await inlineDocumentEvent(client, id, op.slice(11), value, native, storage, image => openInlineDocumentImage(client, image.src, image.name, `inline-frame-${id}`)); return ''; }
  if (op === 'run-terminal' && storage) { await runTerminalCommand(client, native, storage, value.trim(), '', 'preserve'); return ''; }
  if (op.startsWith('surface-')) return surfaceLocal(client, native, op.slice(8), id, value); // the right panel's surfaces (window chatLocal)
  if (op === 'link-open') return openLinkFromUi(client, native, id, value); // "Open links in" (browser-links.ts)
  if (op === 'ref-scroll-end') { noteRefScrollEnd(client, id); return ''; } // a ref list scrolled toward its end (ref-paging.contract)
  if (op === 'link-pick') return chatExternalLinkPick(client, native, storage, value, id === 'page'); // a row of a reply's web link menu (external-link-menu.ts)
  if (op === 'pr-link-pick') { await pullRequestLinkPick(client, native, value); return ''; } // the detail header's number's menu choice
  if (op === 'pr-preview') { notePreviewHover(client, id, value === '1'); return ''; } // a pull request link's hover card
  if (op.startsWith('prw-')) return prLocalWrite(client, native, op.slice(4), id, value); // the composer's drafts (pages-pr-detail.ts)
  if (op === 'pr-ui-select') return surfaceLocal(client, native, 'linked-pr-open', stackLayerTarget(value), ''); // a stack layer beside a thread
  if (op.startsWith('pr-ui-')) return prUiLocal(client, op.slice(6), id, value); // the panel header's menu choices
  if (op.startsWith('pr-guide-')) return prGuideLocalFor(client, native, op.slice(9), id, value);
  if (op === 'pr-code-find-focus') { focusDiffFind(client, 'pr', id); return ''; }
  if (op.startsWith('pr-code-')) return prCodeLocalFor(client, native, op.slice(8), id, value); // the Code tab's presses (pages-pr-code.ts)
  if (op.startsWith('git-')) return gitChatLocal(client, native, op.slice(4), id, value); // git-route.ts
  if (op.startsWith('media-')) return mediaLocal(client, native, op.slice(6), id, value, { urlOf: attachmentId => cachedAttachmentUrl(client, attachmentId),
    openFile: relativePath => openFileSurface(client, native, relativePath, 0), forgetAttachment: attachmentId => { if (!forgetMediaPreviewUrl(client, attachmentId)) forgetAttachmentUrl(client, attachmentId); } });
  const view = timelineView(client);
  if (op === 'find-focus') { focusDiffFind(client, null); return ''; }
  if (op === 'diff-find-focus') { focusDiffFind(client, 'panel'); return ''; }
  if (op.startsWith('diff-find-')) { client.diffState.find.action(op.slice(10), value); return ''; }
  if (op.startsWith('find-')) return threadFindAction(client, op.slice(5), value);
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
  if (op === 'image-open' && isSentSvg(client, id)) return surfaceLocal(client, native, 'linked-attachment', id, value); // SvgFileChip's onOpen
  if (op === 'chip-preview-failed') { noteChipPreviewFailed(client, id, value); return ''; } // a chip's thumbnail at `value` reported an error
  if (op.startsWith('image-')) return imagePreviewAction(client, op, id, value);
  if (op === 'revert') return askRevert(client, view, id, Number(value));
  if (op === 'revert-answer') return answerRevert(client, native, view, value, storage);
  if (op === 'diagram-retry') return retryMermaid(value, id); // id: the theme whose page failed
  if (op === 'diagram-frame') return noteMermaidFrame(value); // a Mermaid page's report (assets/mermaid/render.js)
  if (op === 'math-frame') return noteMathFrame(value); // a math page's report (assets/math/render.js)
  if (op === 'audio-playing') { noteAudioPlaying(client, id, value); return ''; } // a sound started or stopped in a row (timeline-kept-rows.ts)
  if (op === 'setup-details' || op === 'setup-cancel' || op === 'setup-terminal') return worktreeSetupAction(client, native, op);
  if (op === 'diagram-open' || op === 'diagram-close') return diagramPreviewAction(client, op, value);
  // A table's Copy as Markdown / CSV (the check morph, no toast) and a thread chip's open.
  if (op === 'table-menu' || op === 'table-menu-close') return tableMenuAction(client, op, id, value);
  if (op === 'copy-table') {
    closeTableMenu(client);
    if (!value) throw new ClientError('That table is unavailable.');
    await chatLocal(client, native, 'copy-code', id, value, storage);
    return '';
  }
  // View source opens the cited thread and has the chat column place its answer (citation-source.ts).
  if (op === 'chip-citation') {
    const flow = view.citing = ++view.citeFlows;
    try {
      return await revealCitation(client, native, id, value, () => view.lastRows.threadId === client.threadId ? view.lastRows.rows : [],
        key => { view.citeJump = { threadId: client.threadId, key, nonce: view.citeJump.nonce + 1 }; }, messageId => openCitedFold(client, messageId));
    } finally { if (view.citing === flow) view.citing = 0; }
  }
  if (op === 'chip-pr') { if (!id) throw new ClientError('That pull request is no longer available.'); await openPullRequest(client, native, id); return ''; } // a sent pull request chip (pr-chip-open.ts)
  if (op === 'chip-thread' && value === 'link') { await openThreadLinkTarget(client, native, id); return 'focus:composer'; } // a thread link's chip (message-chips.ts)
  if (op === 'chip-thread') {
    if (!id || !client.shell.threads.some(thread => thread.id === id)) throw new ClientError('Thread no longer available');
    await client.openSelected(native, id);
    return '';
  }
  if (op === 'copy-prompt-link') { await copyPromptLink(client, native, promptLinkFor(copyText(client, id))); return ''; }
  // The header's rename field: its text as typed (`id` its thread; none once the field closes), for its direction.
  if (op === 'rename-input') { view.renameDraft = id ? { threadId: id, text: value } : null; return ''; }
  // ThreadErrorBanner's Copy request for admin, for a ChatGPT Business workspace limit.
  if (op === 'admin-request') {
    try { await client.restAccess(native).call({ op: 'copyText', text: CHATGPT_WORKSPACE_ADMIN_REQUEST }); }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Could not copy the request' }); return ''; }
    pushToast(client, { kind: 'success', title: 'Request copied', description: 'Send it to your workspace admin.' });
    return '';
  }
  if (op === 'copy-code' || op === 'copy') {
    const text = op === 'copy-code' ? value : messageCopyText(client, id);
    if (!text || text.length > 1_000_000) throw new ClientError(op === 'copy-code' ? 'That code block is unavailable.' : 'That message is no longer available.');
    const key = op === 'copy' ? id : `code:${id}`;
    try { await client.restAccess(native).call({ op: 'copyText', text }); }
    catch (error) {
      // showAnchoredCopyErrorToast: a refused message copy is an error toast with the reason, at the normal timeout; the
      // button keeps its copy glyph.
      if (op !== 'copy' || letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'Failed to copy', description: error instanceof Error ? error.message : String(error) });
      return '';
    }
    view.copies.set(key, { nonce: ++view.nonce });
    if (op === 'copy-code') view.codeCopy = { threadId: client.threadId, text, nonce: view.nonce };
    return op === 'copy' ? 'Copied message' : 'Copied code';
  }
  if (!['fold', 'group', 'attempt', 'message'].includes(op)) throw new ClientError(`Unknown timeline action: ${op}`);
  const key = scoped(client, op, id);
  if (view.open.has(key)) view.open.delete(key); else view.open.add(key);
  return '';
}
function copyText(client: T3Client, id: string): string {
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === id);
  const item = obj(row?.item);
  // A user message copies what it shows: an older automation prompt without its schedule prefix (MessageCopyButton's `text`).
  const text = item.type === 'proposed_plan' ? str(item.markdown) : item.type === 'user_message' ? userMessagePresentation(item).text : item.type === 'assistant_message' ? str(item.text) : '';
  return relabelThreadLinks(renderCodexInlineDirectivesAsMarkdown(text), client.shell.threads, client.environmentId);
}
/** A sent message's Copy: its words, or for media sent without words, the media's references (`copyText` there). */
function messageCopyText(client: T3Client, id: string): string {
  const text = copyText(client, id);
  if (text) return text;
  const item = obj(arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === id)?.item);
  if (item.type !== 'user_message') return '';
  const attached = messageAttachments(item), sender = userMessagePresentation(item);
  return sender.attribution === 'automation' || sender.attribution === 'agent' ? '' : userMessageShape(item, '', attached, new Set(), false).copy;
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
  const checkpoint = findRevertCheckpoint(arr(client.projection.checkpoints), pending.turnCount);
  if (!checkpoint) throw new ClientError(`Checkpoint for run ordinal ${pending.turnCount} is unavailable.`);
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === pending.messageId);
  if (!row) throw new ClientError('The message to rewind is no longer available.');
  const text = str(obj(row.item).text), threadId = client.threadId;
  // waitForRevertedMessage: what shows the rewind landed is read before it is asked for.
  const witness = rollbackWitness(client.projection, pending.turnCount);
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.dispatch(storage, { type: 'checkpoint.rollback', commandId, threadId, scopeId: str(checkpoint.scopeId),
    checkpointId: str(checkpoint.id), restoreFiles: choice === 'files' }, 'Edit from here');
  // The prompt returns only once the rewind landed; the thread's rollbackFailure for this command becomes the thread error.
  await settleRollback(client, native, storage, threadId, String(commandId), witness, pending.turnCount);
  const current = client.local.drafts[client.draftKey] ?? '';
  const restored = text.trim() ? text : '';
  if (restored) client.local.drafts[client.draftKey] = current ? `${current}\n\n${restored}` : restored;
  return choice === 'files' ? 'Reverted files and conversation' : 'Reverted conversation';
}

export interface RollbackWitness { runId: string; providerThreadId: string; providerInstanceId: string; providerThreadIds: Set<string>; threadId: string }
/**
 * ChatView waitForRevertedMessage (T3 Code 74541bab17): a rewind commits the whole settled suffix at once, and paging can
 * omit the message's own run, so the latest settled run after the target stands witness. With none the thread changed.
 */
export function rollbackWitness(projection: Obj, turnCount: number): RollbackWitness {
  const thread = obj(projection.thread), providerThreads = arr(projection.providerThreads);
  let witness: Obj | undefined;
  for (const run of arr(projection.runs)) {
    if (Number(run.ordinal) <= turnCount || !['completed', 'interrupted', 'failed', 'cancelled'].includes(str(run.status))) continue;
    if (!witness || Number(run.ordinal) > Number(witness.ordinal)) witness = run;
  }
  if (!witness) throw new ClientError('The thread has changed. Select a message to rewind again.');
  const providerThreadId = str(thread.activeProviderThreadId);
  return { runId: str(witness.id), providerThreadId, providerInstanceId: str(providerThreads.find(entry => entry.id === providerThreadId)?.providerInstanceId),
    providerThreadIds: new Set(providerThreads.map(entry => str(entry.id))), threadId: str(thread.id) };
}
/** The rewind landed: the provider thread's last run is the target (a fresh root session for a rewind to the start), and the witness rolled back. */
export function rollbackLanded(projection: Obj, witness: RollbackWitness, turnCount: number): boolean {
  return arr(projection.providerThreads).some(entry => (entry.id === witness.providerThreadId
    // Claude resets to a fresh root session when rewinding to the thread start.
    || turnCount === 0 && entry.ownerNodeId == null && entry.appThreadId === witness.threadId && entry.providerInstanceId === witness.providerInstanceId
      && !witness.providerThreadIds.has(str(entry.id)))
    && (entry.lastRunOrdinal ?? null) === (turnCount === 0 ? null : turnCount))
    && arr(projection.runs).some(run => run.id === witness.runId && run.status === 'rolled_back');
}
/** Polls the projection for the rollback's outcome, pulling events every third pause, two minutes at most. */
export async function settleRollback(client: T3Client, native: Native, storage: Files, threadId: string, commandId: string, witness: RollbackWitness, turnCount: number): Promise<void> {
  for (let attempt = 0; attempt < 600; attempt++) {
    if (attempt % 3 === 2) await client.refresh(native, storage);
    if (client.threadId !== threadId) throw new ClientError('That thread is no longer selected.');
    const failure = obj(obj(client.projection.thread).rollbackFailure);
    if (str(failure.requestId) === commandId) throw new ClientError(str(failure.message, 'Failed to revert thread state.'));
    if (rollbackLanded(client.projection, witness, turnCount)) return;
    await sleep(200);
  }
  throw new ClientError('Timed out waiting for the thread to rewind.');
}

/** Timeline state beside its rows: the history control's error and the Edit from here dialog. */
export function timelineSnapshot(client: T3Client) {
  const view = timelineView(client);
  const rows = view.lastRows.threadId === client.threadId ? view.lastRows.rows : [];
  return { historyError: view.historyError.get(client.threadId) ?? '', codeCopyNonce: view.codeCopy.threadId === client.threadId ? view.codeCopy.nonce : 0,
    revertPending: view.revert?.threadId === client.threadId ? view.revert.messageId : '', ...planSaveView(client),
    minimap: minimapItems(rows), citing: view.citing > 0,
    citeJump: view.citeJump.threadId === client.threadId ? { key: view.citeJump.key, nonce: view.citeJump.nonce } : { key: '', nonce: 0 },
    ...imagePreviewView(client), ...diagramPreviewView(client), mathFrames: mathFrames() };
}

/** The automation's task title from the environment's live task list (scheduledTasksLive), '' until it arrives. */
export function scheduledTaskTitle(client: T3Client, taskId: string): string {
  if (!taskId) return '';
  return str(liveEnvironment(client, null, client.environmentId)?.tasks.value?.find(task => task.id === taskId)?.title);
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
  const argumentsView = entry.itemType === 'dynamic_tool' ? replArguments(entry.item.toolName, entry.item.input) : [];
  const item = entry.item, warning = entry.sourceActivityKind === 'runtime.warning';
  const failedError = item.type === 'error' && item.status === 'failed';
  const failed = !warning && displayFailed(entry);
  const severe = failed && (entry.itemType === 'error' || !isToolLike(entry));
  const plain = plainOutput(entry, root);
  const inspect = plain === undefined ? inspectorDetail(entry, root) : { input: '', result: '', ok: true };
  // V2ItemInspector draws search results and file changes as their own parts, not as tool output.
  const extras = plain === undefined ? inspectorExtras(item, path => workspaceRelativePath(path, root)) : NO_INSPECTOR_EXTRAS;
  const structured = item.type === 'file_search' || item.type === 'web_search';
  const terminal = commandOutputView(client, entry.id, item), terminalText = terminal?.value?.output.text ?? '';
  const terminalState = terminal?.value ? terminalText ? '' : terminal.value.running ? '' : 'empty' : terminal?.error ? 'error' : 'loading';
  const reasoning = entry.itemType === 'reasoning';
  const usage = failedError && obj(item.failure).class === 'usage_limit';
  // An answered question leads with what was asked (even over a lone tool's label) and trails its answer.
  const question = entry.questionAnswer ? questionTextPreview(entry.questionAnswer) : '';
  const title = failedError ? usage ? 'Usage limit reached.' : entry.label
    : reasoning ? thoughtText(entry.detail ?? '') || (entry.status === 'inProgress' ? 'Thinking' : 'Thought') : question || (label ?? entryDisplayLabel(entry, root));
  // A thread read names the thread by its live title, linked to it (ThreadReadLabel).
  const read = failedError || reasoning ? null : threadReadLabel(entry, title, client.shell.threads);
  // Reads and skills expand to plain text (output) instead of the inspector (body).
  return { id: entry.id, label: read?.text ?? title, linkPrefix: read?.prefix ?? '', linkTitle: read?.title ?? '', linkThreadId: read?.threadId ?? '', replArguments: argumentsView, body: entry.questionAnswer || argumentsView.length ? '' : plain !== undefined && fetches ? plain ?? '' : inspect.input, icon: failedError || warning || severe ? 'circle-alert' : entryIcon(entry),
    terminal: terminal !== null, terminalSpans: commandSpans(terminalText), terminalTruncated: terminal?.value?.output.truncated === true, toolImages: toolImages(item).map(({ id, name, source }) => ({ id, name, source })),
    output: terminal ? terminalText ? terminalOutputPlainText(terminalText) : terminal.error || (terminal.value?.running === false ? 'No output.' : terminal.value ? '' : 'Loading output…') : reasoning ? str(entry.detail) : fetches ? detail.text : structured ? '' : plain ?? detail.text, outputState: terminal ? terminalState : detail.state, ...extras, detailOpen: turnItemIsOpen(client, entry.id), ...toolActivityIconSources(client, item), result: inspect.result, failed, timestamp: entry.createdAt,
    tone: failedError ? usage ? 'provider-warning' : 'provider-error' : warning ? 'warning' : severe ? 'error' : failed ? 'failed' : '',
    ok: inspect.ok, reasoning, expandable: !failedError && (!!entry.questionAnswer || (plain !== undefined ? !!plain || entry.item.outputOmitted === true || !!detail.text : turnItemHasDetail(entry.item))),
    detail: failedError && !usage ? str(obj(item.failure).message) : '', status: entry.status === 'inProgress' ? 'Thinking' : 'Thought',
    // A created thread's Open chat; a subagent notification the parent has no record of keeps Open subagent.
    targetId: entry.itemType === 'thread_created' ? str(item.targetThreadId) : notificationChildThreadId(item), targetLabel: entry.itemType === 'thread_created' ? 'Open chat' : 'Open subagent',
    retryRunId: preparationFailureRunId(item), // upstream 737993303d
    answer: entry.questionAnswer && hasQuestionAnswer(entry.questionAnswer) ? questionAnswerPreview(entry.questionAnswer) : '' };
}

/** The selected thread's row input (deriveRows, turnFoldRunIds). */
function rowInput(client: T3Client, setup = threadWorktreeSetup(client), approvals = timelinePendingApprovals(client)): RowInput {
  const projection = client.projection, view = timelineView(client), root = workspaceRoot(client);
  const runs = arr(projection.runs), running = activeRun(projection), runless = runlessWorkStartedAt(projection);
  const open = (kind: string) => new Set([...view.open].filter(key => key.startsWith(`${client.threadId}\u0000${kind}\u0000`)).map(key => key.split('\u0000')[2]!));
  const provider = arr(client.config.providers).find(provider => provider.instanceId === obj(obj(projection.thread).modelSelection).instanceId);
  const finding = threadFindExpanded(client);
  const input: RowInput = { rows: client.thread ? arr(projection.visibleTurnItems) : [], runs, attempts: arr(projection.attempts), nodes: arr(projection.nodes),
    checkpoints: arr(projection.checkpoints), isWorking: !!running || runless !== '', runningRunId: str(running?.id), latestRun: runs[runs.length - 1] ?? null,
    latestRunTurnEnded: runTurnEnded(projection, latestUnheldRun(projection)), // deriveLatestThreadRun's run, without the usage-limit presentation
    // orchestrationV2RunWorkStartedAt: a wake run keeps the start of the work it continues.
    activeStartedAt: running ? str(running.workStartedAt ?? running.startedAt ?? running.requestedAt) : runless, runlessWorkActive: runless !== '', worktreeSetup: setup.snapshot,
    expandedRuns: finding ? new Set(runs.map(run => str(run.id))) : open('fold'), expandedAttempts: finding ? new Set(arr(projection.attempts).map(attempt => str(attempt.id))) : open('attempt'), expandedGroups: open('group'), root, rollback: provider?.supportsConversationRollback !== false,
    threadStartCheckpointReady: !client.thread?.hasMore && findRevertCheckpoint(arr(projection.checkpoints), 0) !== undefined,
    availableRevertTurnCounts: availableRevertTurnCounts(projection), pendingApprovalRequestIds: new Set(approvals.keys()) };
  // Find opens every fold, the synthetic keys of runless turns and steer segments too.
  if (finding) input.expandedRuns = new Set([...input.expandedRuns, ...turnFoldRunIds(input).values()]);
  return input;
}

/** T3's timeline rows for the selected thread (MessagesTimeline.logic deriveMessagesTimelineRows). */
export function transcriptRows(client: T3Client): Message[] {
  const projection = client.projection, view = timelineView(client), root = workspaceRoot(client);
  const setup = threadWorktreeSetup(client), approvals = timelinePendingApprovals(client), input = rowInput(client, setup, approvals);
  const rows = deriveRows(input);
  resetFolds(client, view);
  // MessagesTimeline backgroundWorktreeSetup: a setup still running after the agent's turn started.
  const background = setup.snapshot?.phase === 'running' && agentStarted(setup.snapshot) && input.latestRun?.startedAt != null ? setup.snapshot : null;
  const context: PresentContext = { client, root, view, subagents: arr(projection.subagents), providers: arr(client.config.providers), format: client.local.deviceSettings.timestampFormat,
    preparing: setup.preparing, compacting: isCompacting(client) && !rows.some(row => row.kind === 'compaction' && row.active || row.kind === 'working' && row.compacting === true), detailsOpen: worktreeDetailsOpen(client), threads: client.shell.threads,
    workflow: workflowOpening(client), background, findEntry: threadFindExpanded(client) ? threadFindView(client).entry : '', approvals };
  const messages = rows.map(row => present(row, context));
  // An approval whose item is not among the rows (not loaded yet) still answers, after them.
  const drawn = new Set(messages.flatMap(message => message.approval?.map(approval => approval.id) ?? []));
  for (const approval of approvals.values()) if (!drawn.has(approval.id)) messages.push({ id: `pending-approval:${approval.id}`, kind: 'entry', title: '', body: '', activities: [], approval: [approval] });
  return messages;
}

/**
 * A folded message's `expanded` is its row's own state in the reference (AutomationMessageRow, UserTimelineRow), which a
 * thread opened again starts afresh: switching threads closes every fold.
 */
function resetFolds(client: T3Client, view: ViewState): void {
  if (view.foldsThread === client.threadId) return;
  view.foldsThread = client.threadId;
  for (const key of [...view.open]) if (key.split('\u0000')[1] === 'message') view.open.delete(key);
}

/**
 * useAssistantCitationTarget's onExpandTurn: opens the turn fold that hides the cited assistant answer, by the
 * fold's own key (an imported or runless turn's is not its answer's run id). True when it opened one.
 */
export function openCitedFold(client: T3Client, messageId: string): boolean {
  const bare = (id: string) => id.replace(/^(?:turn-item|message):/, '');
  const wanted = bare(messageId);
  const row = arr(client.projection.visibleTurnItems).find(row => obj(row.item).type === 'assistant_message' && bare(str(row.sourceItemId)) === wanted);
  const runId = row && wanted ? turnFoldRunIds(rowInput(client)).get(rowKey(row)) : undefined;
  if (!runId) return false;
  const view = timelineView(client), key = scoped(client, 'fold', runId);
  if (view.open.has(key)) return false;
  view.open.add(key);
  return true;
}

/**
 * useV2ItemSupport reads an item's own thread: this thread, or an inherited row's source while its
 * detail is live (a kept running thread); none, and the fork stays available.
 */
function forkable(client: T3Client, row: Obj, item: Obj): boolean {
  const source = str(row.sourceThreadId, client.threadId);
  const projection = source === client.threadId ? client.projection : keptThread(client, source)?.projection;
  return assistantCanFork(projection ?? {}, item);
}

interface PresentContext { client: T3Client; root: string; view: ViewState; subagents: Obj[]; providers: Obj[]; format: string; preparing: boolean; compacting: boolean; detailsOpen: boolean; threads: Obj[];
  /** A workflow coordinator's opening, drawn in place of its script's message (timeline-workflow-opening.ts). */
  workflow: WorkflowOpening | null;
  /** A worktree setup still running after the agent started: the Working row's Background setup chip. */
  background: Obj | null;
  /** The row holding find's active match, while find shows matches. */
  findEntry: string;
  /** The approvals answerable on their rows, by request id. */
  approvals: Map<string, TimelineApprovalView> }
/** A changing one-line label keyed by its text, so the row redraws it instead of reusing the old string. */
const keyedLabel = (row: Row, label: string): Activity => ({ id: `${row.id}\u0000${label}`, label, body: '', icon: '', output: '', result: '', failed: false, timestamp: '' });
const entryCode = (entries: WorkEntry[], client: T3Client): InspectorCode[] => entries.flatMap(entry => {
  if (entry.questionAnswer) return [questionHistory(entry.id, entry.questionAnswer)];
  if (entry.itemType === 'command_execution' && turnItemIsOpen(client, entry.id)) return [{ id: entry.id, code: '', icon: 'shell',
    tokens: codeTokens(shellCommand(str(entry.item.input), 'data')) }];
  // An open thought's trace is ChatMarkdown, whose fences are highlighted as a message's are.
  if (entry.itemType === 'reasoning' && turnItemIsOpen(client, entry.id)) return messageCodeBlocks(str(entry.detail), 'data', `reasoning:${entry.id}`).map(block => ({ ...block, id: `${entry.id}:${block.id}` }));
  return [];
});

function present(row: Row, context: PresentContext): Message {
  const { root, view } = context;
  const base = { id: row.id, createdAt: row.createdAt, title: '', body: '' };
  switch (row.kind) {
    case 'fold': return { ...base, kind: 'work', title: row.label, runId: row.runId, groupId: row.runId, expanded: row.expanded };
    // While a worktree is prepared the header reads "Setting up worktree…" and the activity row keeps its height empty.
    // A compaction reads "Compacting for 12s", timed from the row's start, or "Compacting…" without one
    // (WorkingTimelineRow's CompactingLabel).
    case 'working': {
      const compacting = row.compacting === true || context.compacting, startedMs = Date.parse(row.createdAt) || 0;
      return { ...base, kind: 'working', title: context.preparing ? 'Setting up worktree…' : compacting ? startedMs > 0 ? 'Compacting for' : 'Compacting…' : 'Working for',
        live: context.preparing || compacting, icon: !context.preparing && compacting ? 'minimize' : '', startedMs,
        ...(context.background ? { setup: [setupView(context.background, true, context.detailsOpen)], detail: str(obj(context.background.setupScript).name, 'Setup script') } : {}) };
    }
    case 'setup': return { ...base, kind: 'worktree-setup', title: 'Worktree setup', setup: [setupView(row.snapshot, row.embedded, context.detailsOpen)] };
    // A failed group keeps this disclosure while an approval waits, but nothing is thinking then.
    case 'thinking': return { ...base, kind: 'thinking', title: context.preparing || context.compacting ? '' : 'Thinking', icon: 'brain', live: context.approvals.size === 0, continues: row.continues === true,
      groupId: row.groupId ?? '', expanded: row.expanded === true };
    case 'live': {
      // The latest thought's first sentence sits above the status line (the open group lists it already);
      // while that thought is itself the live entry, the status line says Thinking.
      const thought = row.thought && !row.expanded ? liveThoughtLine(row.thought.detail ?? '') : '';
      const label = thought && row.entry.id === row.thought?.id ? row.active ? 'Thinking' : 'Thought'
        : threadReadLabel(row.entry, row.label, context.threads)?.text ?? row.label;
      return { ...base, kind: 'live', title: label, detail: thought, icon: entryIcon(row.entry), failed: displayFailed(row.entry), live: row.active,
        expanded: row.expanded, groupId: row.groupId, continues: row.continues === true, runId: row.entry.runId, activities: [{ ...keyedLabel(row, label), ...toolActivityIconSources(context.client, row.entry.item) }] };
    }
    // The group's image icon: the primary tool source's, else the last entry's.
    case 'group': return { ...base, kind: 'group', title: row.summary, icon: row.icon, failed: row.failed, expanded: row.expanded, groupId: row.groupId,
      runId: row.runId, continues: row.continues === true, activities: [{ ...keyedLabel(row, row.summary), ...(row.toolIcon ? toolActivityIconSources(context.client, { toolIcon: row.toolIcon }) : {}) }] };
    case 'details': {
      const activities = row.entries.map(entry => activity(entry, root, context.client));
      return { ...base, kind: 'details', activities, continues: row.continues === true, code: entryCode(row.entries, context.client) };
    }
    case 'entry': {
      const single = row.entries.length === 1 ? row.entries[0]! : null;
      const card = single?.itemType === 'notification' ? notificationSubagent(single.item, context.subagents, single.createdAt, shortTime(single.createdAt, context.format), context.providers) : null;
      if (card) return { ...base, kind: 'subagent', title: card.label, activities: [card], runId: single!.runId };
      const approval = single?.item.type === 'approval_request' ? context.approvals.get(str(single.item.requestId)) : undefined;
      if (approval) return { ...base, kind: 'entry', activities: [], approval: [approval], continues: row.continues === true, runId: single!.runId };
      const activities = row.entries.map(entry => activity(entry, root, context.client, row.entries.length === 1 ? row.label : undefined));
      return { ...base, kind: 'entry', activities, continues: row.continues === true, runId: row.entries[0]?.runId ?? '', code: entryCode(row.entries, context.client) };
    }
    case 'attempt': return { ...base, kind: 'attempt', title: 'Superseded attempt', detail: 'Cut off by a steer', expanded: row.expanded, groupId: row.attemptId, runId: row.runId };
    // ContextCompactionTimelineRow: T3 Code's own compaction's title leads in place of "Context compacted"; a failed one
    // reads in the destructive colour with its reason after a middle dot (`output`).
    case 'compaction': {
      const label = row.title && row.label.startsWith('Context compacted') ? row.title + row.label.slice('Context compacted'.length) : row.label;
      return { ...base, kind: 'compaction', title: row.failure ? `${label}: ${row.failure}` : label, live: row.active, icon: 'minimize', failed: row.failed === true,
        activities: [{ ...keyedLabel(row, label), output: row.failure ?? '',
          ...(row.tokenCounts ? { body: row.title ?? 'Context compacted', detail: row.tokenCounts.before, result: row.tokenCounts.after } : {}) }] };
    }
    case 'document': {
      const entry = row.entry, ref = entry.document;
      const key = inlineDocumentKey(context.client, str(entry.row.sourceThreadId), str(entry.item.id), ref);
      return { ...base, kind: 'inline-document', runId: str(entry.item.runId), inlineDocuments: [inlineDocumentView(context.client, key, ref)] };
    }
    case 'plan': {
      const markdown = relabelThreadLinks(renderCodexInlineDirectivesAsMarkdown(str(row.entry.item.markdown)), context.threads, context.client.environmentId);
      return { ...base, kind: 'plan', title: proposedPlanTitle(markdown) ?? 'Proposed plan', body: planBody(markdown), runId: str(row.entry.item.runId),
        completed: row.entry.item.streaming !== true, streaming: row.entry.item.streaming === true, sourceThreadId: str(row.entry.row.sourceThreadId), meta: false, collapsible: planCollapsible(markdown), code: messageCodeBlocks(markdown, 'data', `plan:${row.id}`), chips: withFileLinkMenus(context.client, messageChips({ ...row.entry.item, text: markdown }, context.root, context.threads, row.id)),
        diagrams: messageDiagrams(markdown),
        copied: view.copies.get(row.id)?.nonce ?? 0 };
    }
    case 'checkpoint': {
      const filesTotal = checkpointFileTotals(row.entry.item.filesTotal);
      return { ...base, kind: 'checkpoint', title: 'Changes', checkpointId: str(row.entry.item.checkpointId), runId: str(row.entry.item.runId),
        sourceThreadId: str(row.entry.row.sourceThreadId), files: arr(row.entry.item.files).map(file => ({ path: str(file.path), additions: num(file.additions), deletions: num(file.deletions) })),
        ...(filesTotal ? { filesTotal } : {}) };
    }
    case 'event': if (row.entry.item.type === 'secret_request') return { ...base, kind: 'secret', secret: secretCard(context.client, row.entry.item, row.entry.row.visibility) };
    {
      const event = eventRow(row.entry.item, row.subagents, row.entry.row, { agents: context.subagents, providers: context.providers, runs: arr(context.client.projection.runs), threads: context.threads });
      if (event.kind !== 'subagents') return { ...base, ...event };
      // A group of subagents opens and closes as a work group does (the `group` toggle, kept per thread).
      const groupId = `subagent-group:${row.id}`;
      return { ...base, ...event, groupId, expanded: view.open.has(scoped(context.client, 'group', groupId)) };
    }

    case 'meta': return { ...base, kind: 'meta', body: str(row.entry.item.text), runId: str(row.entry.item.runId), completed: true, canFork: forkable(context.client, row.entry.row, row.entry.item),
      actionsId: row.entry.id, copied: view.copies.get(row.entry.id)?.nonce ?? 0 };
    case 'message': {
      const item = row.entry.item, copy = view.copies.get(row.id);
      const common = { ...base, runId: str(item.runId), sourceThreadId: str(row.entry.row.sourceThreadId), completed: item.status === 'completed',
        copied: copy?.nonce ?? 0 };
      if (row.entry.role === 'assistant') {
        const text = relabelThreadLinks(renderCodexInlineDirectivesAsMarkdown(str(item.text)), context.threads, context.client.environmentId);
        return { ...common, kind: 'assistant', title: 'Assistant', rawText: str(item.text), canFork: forkable(context.client, row.entry.row, item), body: text || (item.streaming === true ? '' : '(empty response)'), chips: withFileLinkMenus(context.client, messageChips({ ...item, text }, context.root, context.threads, row.id, {}, context.client)),
          meta: row.meta, streaming: item.streaming === true || row.inProgress, code: messageCodeBlocks(text, 'data', `message:${row.id}`),
          diagrams: messageDiagrams(text) };
      }
      if (context.workflow && context.workflow.messageId === str(item.messageId)) {
        const opening = workflowOpeningMessage(context.workflow, { agents: [], providers: context.providers, threads: context.threads }, threadFindView(context.client).highlight);
        return { ...common, ...opening, code: messageCodeBlocks(opening.body, 'data', `opening:${row.id}`) };
      }
      const intent = str(item.inputIntent), sender = userMessagePresentation(item);
      // An automation's prompt reads its context references as their labels (AutomationMessageRow's UserMessageBody).
      const relabeled = relabelThreadLinks(renderCodexInlineDirectivesAsMarkdown(sender.text), context.threads, context.client.environmentId);
      const text = sender.attribution === 'automation' ? plainContextReferences(relabeled) : withoutSoundReferences(relabeled, item), chips = withFileLinkMenus(context.client, messageChips({ ...item, text }, context.root, context.threads, row.id, {}, context.client))
        .map(chip => chip.kind === 'image' && svgPreviewFailed(context.client, chip.target) ? asFileChip(chip) : chip), attached = messageAttachments(item), chipped = chippedAttachmentIds(chips, item);
      // Automation and other agents' messages fold to one line, their author (the sending thread's title) as the heading.
      const automation = sender.attribution === 'automation', agent = sender.attribution === 'agent';
      const senderTitle = agent ? str(context.threads.find(thread => thread.id === item.senderThreadId)?.title).trim() || 'Agent' : '';
      const summary = automation || agent ? foldedMessageSummary(sender.attribution, automation ? sender.text : text, automation ? scheduledTaskTitle(context.client, sender.scheduledTaskId) : '') : '';
      // findRevealRef: find's painter reveals the fold on every paint while its active match is inside (revealFolded), so
      // the chevron cannot close it then; it stays open after the match moves on.
      const foldKey = scoped(context.client, 'message', row.id);
      if ((automation || agent) && context.findEntry === row.id) view.open.add(foldKey);
      // A file with a chip in the prose needs no standalone row (images always show); an automation's prompt shows none.
      const shaped = userMessageShape(item, text, attached, chipped, automation || agent);
      return { ...common, kind: 'user', title: automation ? 'Automation' : agent ? senderTitle : 'You', summary, body: text, revert: row.revert,
        images: automation ? [] : attached.images, attachFiles: automation ? [] : shaped.files, audio: automation ? [] : attached.audio, chips, code: messageCodeBlocks(text, 'data', `message:${row.id}`), diagrams: messageDiagrams(text),
        shape: shaped.shape, mediaAbove: shaped.mediaAbove, mediaLayout: shaped.layout, copyable: sender.text !== '' || shaped.copy !== '',
        collapsible: !automation && !agent && shouldCollapseUserMessage(text),
        // A queued message that was sent reads as any other; only a steer keeps its marker.
        intent: intent === 'steer' || intent === 'promoted_queued_to_steer' ? 'Steer' : '',
        intentTip: intent === 'promoted_queued_to_steer' ? 'Originally queued, then promoted to steer the active turn' : intent === 'steer' ? 'Steered the active turn' : '',
        attribution: sender.attribution, targetId: str(item.senderThreadId), detail: sender.scheduledTaskId, expanded: view.open.has(foldKey),
        status: ['completed', 'pending', 'waiting'].includes(str(item.status)) ? '' : str(item.status) };
    }
  }
}

type RowActivity = Activity & Required<Pick<Activity, 'tone' | 'ok' | 'reasoning' | 'expandable' | 'detail' | 'status' | 'targetId' | 'answer' | 'retryRunId' | 'outputState' | 'detailOpen' | 'iconLight' | 'iconDark'
  | 'results' | 'changePath' | 'stats' | 'additions' | 'deletions' | 'startedMs' | 'initials' | 'driverIcon' | 'targetLabel' | 'modelLine' | 'effort' | 'speed' | 'speedLabel'
  | 'replArguments' | 'linkPrefix' | 'linkTitle' | 'linkThreadId' | 'badge' | 'badgeColor' | 'terminal' | 'terminalTruncated' | 'terminalSpans' | 'toolImages'>> & { timeTip: string };
const blankActivity = (activity: Activity): RowActivity => ({
  tone: '', ok: true, reasoning: false, expandable: true, detail: '', status: '', targetId: '', answer: '', retryRunId: '', timeTip: '',
  ...activity, replArguments: activity.replArguments ?? [], terminal: activity.terminal ?? false, terminalTruncated: activity.terminalTruncated ?? false, terminalSpans: activity.terminalSpans ?? [], toolImages: activity.toolImages ?? [], outputState: activity.outputState ?? '', detailOpen: activity.detailOpen ?? false, iconLight: activity.iconLight ?? '', iconDark: activity.iconDark ?? '',
  results: activity.results ?? NO_INSPECTOR_EXTRAS.results, changePath: activity.changePath ?? '', stats: activity.stats ?? false, additions: activity.additions ?? 0,
  deletions: activity.deletions ?? 0, startedMs: activity.startedMs ?? 0,
  initials: activity.initials ?? '', driverIcon: activity.driverIcon ?? '', targetLabel: activity.targetLabel ?? '',
  modelLine: activity.modelLine ?? '', effort: activity.effort ?? '', speed: activity.speed ?? '', speedLabel: activity.speedLabel ?? '',
  linkPrefix: activity.linkPrefix ?? '', linkTitle: activity.linkTitle ?? '', linkThreadId: activity.linkThreadId ?? '', badge: activity.badge ?? '', badgeColor: activity.badgeColor ?? '' });

/**
 * Timeline rows as Contract draws them, every Message field present. T3 shows
 * a turn's changed files inside the final assistant row, above that row's
 * copy/fork/time actions; the client keeps checkpoints as their own rows, so an
 * attached checkpoint carries the preceding answer's actions and the answer omits its own.
 */
export function timelineMessages(client: T3Client, transcript: Message[], now: number) {
  const format = client.local.deviceSettings.timestampFormat, md = markdownEnv(client), bubbleInks = markdownChipInks(client, true);
  const copy = timelineView(client).codeCopy, codeCopy = copy.threadId === client.threadId ? { codeCopied: copy.text, codeCopyNonce: copy.nonce } : { codeCopied: '', codeCopyNonce: 0 };
  // A checkpoint that changed no files draws nothing, so it is no row, as upstream derives none for it. A zero-height
  // row would also never settle in the virtualized transcript: each row state write puts it
  // back at the list's estimate, and once measured it leaves the window again, rebuilding the row at the window's edge.
  const rows = transcript.filter(message => message.kind !== 'checkpoint' || (message.files?.length ?? 0) > 0 && (!message.sourceThreadId || message.sourceThreadId === client.threadId)).map(message => {
    const checkpoint = arr(client.projection.checkpoints).find(checkpoint => checkpoint.id === message.checkpointId);
    const files = message.files ?? [], tree = treeState(client, message.runId || message.checkpointId || '');
    // checkpointFileTotals: a huge checkpoint sends a prefix of its files with the totals of all of them.
    const totals = message.filesTotal ?? { count: files.length, additions: files.reduce((sum, file) => sum + file.additions, 0), deletions: files.reduce((sum, file) => sum + file.deletions, 0) };
    const hidden = message.kind === 'checkpoint' ? totals.count - files.length : 0;
    return { id: message.id, kind: message.kind, title: message.title, body: message.body,
      checkpointId: message.checkpointId ?? '', checkpointOrdinal: num(checkpoint?.appRunOrdinal, -1),
      additions: totals.additions, deletions: totals.deletions,
      filesLabel: message.kind === 'checkpoint' ? `${totals.count.toLocaleString('en-US')} changed file${totals.count === 1 ? '' : 's'}` : '',
      moreFiles: hidden > 0 ? `${hidden.toLocaleString('en-US')} more file${hidden === 1 ? '' : 's'} in the full diff` : '',
      files: message.kind === 'checkpoint' ? treeRows(files, tree.all, tree.overrides) : [],
      runId: message.runId ?? '', sourceThreadId: message.sourceThreadId ?? '',
      // AssistantForkButton: a completed answer of a run whose provider can fork; never a row only this client shows.
      canFork: (message.kind === 'assistant' || message.kind === 'meta') && message.canFork === true && message.completed === true && message.streaming !== true && message.local !== true,
      completed: message.completed === true, createdAt: message.createdAt ?? '', timestamp: messageTime(message.createdAt, now, format),
      timeTip: timestampTooltip(message.createdAt, format), actionsTimeTip: '', ...codeCopy,
      activities: retryableActivities(client, (message.activities ?? []).map(activity => ({ ...blankActivity({ ...activity, timestamp: messageTime(activity.timestamp, now, format) }), timeTip: timestampTooltip(activity.timestamp, format) }))),
      first: false, attached: false, hoverKey: message.id, actionsId: '', actionsFork: false, actionsTime: '', expanded: message.kind === 'checkpoint' ? tree.all : message.expanded === true, last: false,
      icon: message.icon ?? '', tone: message.tone ?? '', failed: message.failed === true, live: message.live === true,
      startedMs: message.startedMs ?? 0, elapsed: message.elapsed ?? '', detail: message.detail ?? '', status: message.status ?? '', groupId: message.groupId ?? '',
      continues: message.continues === true, revert: message.revert ?? -1, intent: message.intent ?? '', intentTip: message.intentTip ?? '',
      attribution: message.attribution ?? '', summary: message.summary ?? '', shape: message.shape ?? '', mediaAbove: message.mediaAbove === true,
      mediaLayout: message.mediaLayout ?? '', copyable: message.copyable !== false, groupedNext: false, groupedPrevious: false, continuesSteer: false, sender: message.sender ?? '', targetId: message.kind === 'checkpoint' ? files[0]?.path ?? '' : message.targetId ?? '', actionLabel: message.actionLabel ?? '',
      copied: message.copied ?? 0, meta: message.meta !== false,
      collapsible: message.collapsible === true, code: message.code ?? [], diagrams: message.diagrams ?? [], inlineDocuments: message.inlineDocuments ?? [], setup: message.setup ?? [], fileCount: message.kind === 'checkpoint' ? totals.count : 0,
      secret: message.secret ?? [], approval: message.approval ?? [], findLines: message.findLines ?? [], images: message.images ?? [], attachFiles: message.attachFiles ?? [], audio: message.audio ?? [], md: { ...md, inks: message.kind === 'user' ? bubbleInks : md.inks, chips: message.chips ?? [], threadGlyphs: markdownThreadGlyphs(client, str(message.body), message.chips ?? []), runCommands: client.projectId ? runnableShellCommands(str(message.body), message.streaming === true) : [] } };
  });
  const rawText = new Map(transcript.map(message => [message.id, message.rawText]));
  timelineView(client).lastRows = { threadId: client.threadId, rows: rows.map(row => ({ id: row.id, kind: row.kind, body: row.body, text: rawText.get(row.id) })) };
  // deriveMessagesTimelineRows: your back-to-back messages stack as one burst, a run of steers labelled once.
  const own = (row: typeof rows[number] | undefined) => row?.kind === 'user' && row.attribution === '';
  rows.forEach((row, index) => {
    if (!own(row)) return;
    const previous = rows[index - 1];
    row.groupedNext = own(rows[index + 1]);
    row.groupedPrevious = own(previous);
    row.continuesSteer = row.groupedPrevious && row.intent === 'Steer' && previous!.intent === 'Steer';
  });
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
      row.copied = previous.copied;
      previous.actionsId = ''; previous.actionsFork = false; previous.actionsTime = '';
    }
  });
  return rows;
}

export { workspaceRelativePath };
