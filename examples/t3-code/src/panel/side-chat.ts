// Side chats beside the main thread (T3 Code 2359643cff, MIT, see LICENSE-T3: components/chat/SideChatPane.tsx,
// chat/useSideChat.ts, lib/sideChatPane.ts, rightPanelStore.ts openSideChat, RightPanelTabs.tsx's side chat tab,
// hooks/useRightPanelSurfaceClose.ts, ChatView.tsx sideChatsOffered and sideChatDisabledReason, composer-logic.ts
// parseSideChatCommand). A side chat is a hidden fork of the open thread's latest stable point
// (`thread.fork` with `sideChat: true`), shown as a right-panel tab of its parent: `side-chat:<thread id>`, its thread id
// kept in the surface's `path`. The tab's body is the side chat's title, Bring back, Delete and Close over its
// conversation and a field that sends to it. The reference embeds a whole ChatView there; the clone's timeline and
// composer belong to the window's one thread (presentation.ts snapshot), so the pane draws the side chat's own messages
// as Markdown, its pending approvals and its first open question, from its thread stream (pinned while the tab shows,
// lineage-parent.ts pinThreads).
import type { T3Client } from '../client/client';
import { arr, messages, obj, str, type Obj } from '../client/domain';
import { activeRun, ClientError, sendPayload, type Native } from '../client/protocol';
import { can } from '../connections/command-permissions';
import { keptThread } from '../client/keep-alive';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { launchTitle } from '../composer/composer-editor-title';
import { pendingRequests } from '../composer/requests';
import { markdownEnv } from '../timeline/message-chips';
import { pendingApproval, pendingInput } from '../sidebar/sidebar-model';
import { pendingSubagentRequest } from '../sidebar/subagent-requests';
import { latestMergeBackRun } from '../details/shell-lineage';
import type { MarkdownSource } from './files-surface';
import type { MarkdownEnv } from '../timeline/message-chips';

export const SIDE_CHAT_PREFIX = 'side-chat:';
export const sideChatSurfaceId = (threadId: string) => `${SIDE_CHAT_PREFIX}${threadId}`;
const ACTIVE = ['preparing', 'starting', 'running', 'waiting'];

const capabilities = (client: T3Client): Obj => obj(obj(client.config.environment).capabilities);
const shellThread = (client: T3Client, threadId: string): Obj | undefined => client.shell.threads.find(thread => thread.id === threadId);
export const isSideChat = (thread: Obj | null | undefined): boolean => obj(thread?.lineage).sideChat === true;
/** threadRuntimeIsActive over a shell's run. */
export const threadActive = (thread: Obj | null | undefined): boolean => !!thread && (!!thread.activeRunId || ACTIVE.includes(str(thread.status)));
const canOperate = (client: T3Client): boolean => can(client, 'orchestration:operate') && client.writable !== false;

/** `/side [prompt]`, or its alias `/btw [prompt]`, as the whole message (composer-logic.ts parseSideChatCommand). */
export function parseSideChatCommand(text: string): { prompt: string } | null {
  const match = /^\/(?:side|btw)(?:\s+([\s\S]*))?$/i.exec(text.trim());
  return match ? { prompt: match[1]?.trim() ?? '' } : null;
}

/** ChatView sideChatsOffered: side chats do not nest or start from subagents, and older servers would fork a listed thread. */
export function sideChatsOffered(client: T3Client): boolean {
  const thread = shellThread(client, client.threadId);
  return canOperate(client) && !isSideChat(thread) && obj(thread?.lineage).relationshipToParent !== 'subagent'
    && capabilities(client).sideChats === true;
}
const isServerThread = (client: T3Client) => !!client.threadId && !!shellThread(client, client.threadId);

/** ChatView sideChatDisabledReason: the reason main-thread surfaces show, and `/side` sent anyway toasts. */
export function sideChatDisabledReason(client: T3Client): string | null {
  const thread = shellThread(client, client.threadId);
  if (isSideChat(thread)) return 'Side chats cannot start side chats.';
  if (obj(thread?.lineage).relationshipToParent === 'subagent') return 'Subagent threads cannot start side chats.';
  if (!client.ready) return 'Side chats are not ready yet. Try again in a moment.';
  if (capabilities(client).sideChats !== true) return "Update this environment's T3 Code server to use side chats.";
  const offered = sideChatsOffered(client);
  if (offered && !isServerThread(client)) return 'Side chats start after the first message.';
  if (offered && worktreePending(str(thread?.worktreePath) || null, arr(client.projection.runs))) return "Available once the thread's worktree is set up.";
  return null;
}
/** orchestrationV2WorktreePending: a worktree still being prepared (or whose preparation failed) for a thread without one. */
export function worktreePending(worktreePath: string | null, runs: Obj[]): boolean {
  return worktreePath === null && runs.some(run => ['preparing', 'failed', 'interrupted'].includes(str(run.status)) && obj(run.workspacePreparation).type === 'worktree');
}

/** The right panel's "+" menu and launcher row (RightPanelTabs `onAddSideChat`): none where side chats are not offered. */
export function sideChatSurfaceRows(client: T3Client): { id: string; label: string; icon: string; shortcut: string; letter: string; available: boolean; reason: string }[] {
  if (!sideChatsOffered(client)) return [];
  const reason = sideChatDisabledReason(client);
  const available = isServerThread(client) && reason === null;
  return [{ id: 'side-chat', label: 'Side chat', icon: 'messages-square', shortcut: 'S', letter: 's', available, reason: available ? '' : reason ?? 'Available after the first message.' }];
}

type Store = { starting: Set<string>; busy: Map<string, string>; pending: boolean };
const stores = new WeakMap<T3Client, Store>();
export function sideChatStore(client: T3Client): Store {
  let store = stores.get(client);
  if (!store) stores.set(client, store = { starting: new Set(), busy: new Map(), pending: false });
  return store;
}

const errorMessage = (error: unknown, fallback: string) => error instanceof Error && error.message ? error.message : fallback;
async function dispatch(client: T3Client, native: Native, payload: Obj): Promise<void> {
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.request('orchestration.dispatchCommand', { ...payload, commandId }, true);
}
/** thread.delete; `onlyIfUnused`: the server keeps a side chat with a run or a message. */
export async function deleteSideChatThread(client: T3Client, native: Native, threadId: string, onlyIfUnused: boolean): Promise<void> {
  await dispatch(client, native, { type: 'thread.delete', threadId, ...(onlyIfUnused ? { onlyIfUnused: true } : {}) });
}

/**
 * useSideChat.open: forks the open thread's latest stable point, sends `prompt` there, and shows the side chat's tab.
 * Resolves false when it did not start, or another one is still starting; a failure goes on the thread's error banner.
 * `openTab` adds and activates the tab on the parent's panel, also when the window has moved to another thread since
 * (panel-surfaces.ts openNewSideChat): an empty side chat is deleted only when its tab closes.
 */
export async function startSideChat(client: T3Client, native: Native, prompt: string, openTab: (threadId: string) => void): Promise<boolean> {
  const store = sideChatStore(client);
  if (!sideChatsOffered(client) || !isServerThread(client) || store.pending) return false;
  const parentId = client.threadId;
  store.pending = true;
  const fail = (error: unknown, fallback: string) => { if (letGo(error)) throw error; client.error = errorMessage(error, fallback); return false; };
  try {
    const [targetThreadId = ''] = await client.restAccess(native).ids(1);
    try {
      await dispatch(client, native, { type: 'thread.fork', createdBy: 'user', creationSource: 'web', sourceThreadId: parentId, targetThreadId,
        sourcePoint: { type: 'latest_stable' }, sideChat: true, title: 'Side chat' });
    } catch (error) { return fail(error, 'Could not start a side chat.'); }
    store.starting.add(targetThreadId);
    // Drops the empty side chat a failed start leaves behind; one that got its message stays.
    const discard = () => { store.starting.delete(targetThreadId); void deleteSideChatThread(client, native, targetThreadId, true).catch(() => undefined); };
    const text = prompt.trim();
    // Send before showing the tab: closing a tab whose side chat has no run deletes it. The server asks before edits on
    // a side chat's first turn, whatever mode is sent.
    if (text) {
      try {
        const [commandId = '', messageId = ''] = await client.restAccess(native).ids(2);
        await client.restAccess(native).request('orchestration.dispatchCommand', sendPayload(commandId, targetThreadId, messageId, text, [], launchTitle(text)), true);
      } catch (error) { discard(); return fail(error, 'Could not send the side chat message.'); }
    }
    openTab(targetThreadId);
    return true;
  } finally { store.pending = false; }
}

/** The side chat's own stream, while its tab shows: the open thread's when it is the side chat itself. */
function sideChatProjection(client: T3Client, threadId: string): Obj | null {
  return keptThread(client, threadId)?.projection ?? null;
}

/** SideChatPane's Bring back reason: why it is unavailable, or null. */
export function bringBackBlockedReason(input: { canOperate: boolean; parentActive: boolean; mergeRun: Obj | null }): string | null {
  if (!input.canOperate) return 'This connection cannot change threads.';
  if (input.parentActive) return 'Available when the main thread is idle.';
  if (input.mergeRun === null) return 'Available after a finished reply.';
  return null;
}

/** SideChatPane bringBack: merges the side chat's latest finished reply into its parent's next message. */
export async function bringBackSideChat(client: T3Client, native: Native, parentId: string, threadId: string): Promise<void> {
  const store = sideChatStore(client);
  const projection = sideChatProjection(client, threadId);
  const mergeRun = latestMergeBackRun(projection);
  if (bringBackBlockedReason({ canOperate: canOperate(client), parentActive: threadActive(shellThread(client, parentId)), mergeRun }) !== null || !mergeRun || store.busy.has(threadId)) return;
  store.busy.set(threadId, 'bring-back');
  try {
    await dispatch(client, native, { type: 'thread.merge_back', createdBy: 'user', creationSource: 'web', sourceThreadId: threadId, targetThreadId: parentId,
      sourcePoint: { type: 'run', runId: str(mergeRun.id) } });
    pushToast(client, { kind: 'success', title: 'Brought back', description: 'The main thread gets this side chat with your next message there.' });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not bring back the side chat', ...(error instanceof Error ? { description: error.message } : {}) });
  } finally { store.busy.delete(threadId); }
}

/** SideChatPane deleteSideChat (after the window's confirmation, when Confirm thread deletion is on). True when deleted. */
export async function deleteSideChat(client: T3Client, native: Native, threadId: string): Promise<boolean> {
  const store = sideChatStore(client);
  if (!canOperate(client) || !shellThread(client, threadId) || store.busy.has(threadId)) return false;
  store.busy.set(threadId, 'delete');
  try {
    await deleteSideChatThread(client, native, threadId, false);
    return true;
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not delete the side chat', ...(error instanceof Error ? { description: error.message } : {}) });
    return false;
  } finally { store.busy.delete(threadId); }
}

/** The pane's field: an answer to the side chat's open question when it has one, else a new message (queued while it runs). */
export async function sendToSideChat(client: T3Client, native: Native, threadId: string, text: string): Promise<void> {
  const message = text.trim();
  if (!message || !canOperate(client)) return;
  const projection = sideChatProjection(client, threadId);
  const question = projection ? pendingRequests(projection).inputs[0] : undefined;
  const first = question?.questions[0];
  if (question && first && question.questions.length === 1) {
    await dispatch(client, native, { type: 'runtime-request.respond', threadId, requestId: question.requestId, answers: { [first.id]: first.multiSelect ? [message] : message } });
    return;
  }
  const [commandId = '', messageId = ''] = await client.restAccess(native).ids(2);
  const payload = sendPayload(commandId, threadId, messageId, message, [], launchTitle(message));
  if (projection && activeRun(projection)) Object.assign(payload, { dispatchMode: { type: 'queue_after_active' } });
  await client.restAccess(native).request('orchestration.dispatchCommand', payload, true);
}

/** An approval's decision or a question's option, pressed in the pane. */
export async function answerSideChat(client: T3Client, native: Native, threadId: string, requestId: string, value: string): Promise<void> {
  if (!canOperate(client)) return;
  const projection = sideChatProjection(client, threadId);
  const pending = projection ? pendingRequests(projection) : { approvals: [], inputs: [] };
  const approval = pending.approvals.find(entry => entry.requestId === requestId);
  if (approval) {
    if (!approval.live) throw new ClientError('This approval is no longer live.');
    await dispatch(client, native, { type: 'runtime-request.respond', threadId, requestId, decision: value });
    return;
  }
  const input = pending.inputs.find(entry => entry.requestId === requestId), question = input?.questions[0];
  if (!input || !question) throw new ClientError('This request has already been resolved.');
  await dispatch(client, native, { type: 'runtime-request.respond', threadId, requestId, answers: { [question.id]: question.multiSelect ? [value] : value } });
}

/** The pane's Stop: interrupts the side chat's active run. */
export async function stopSideChat(client: T3Client, native: Native, threadId: string): Promise<void> {
  const run = activeRun(sideChatProjection(client, threadId) ?? {});
  if (!run || !canOperate(client)) return;
  await dispatch(client, native, { type: 'run.interrupt', threadId, runId: str(run.id) });
}

/** RightPanelTabs sideChatTabState: what the tab's dot shows and its title's accessible name says ('' for nothing). */
export function sideChatTabState(thread: Obj | null | undefined): string {
  if (!thread) return '';
  if (pendingApproval(thread) || pendingSubagentRequest(thread) === 'approval') return 'needs approval';
  if (pendingInput(thread) || pendingSubagentRequest(thread) === 'user_input') return 'needs input';
  return threadActive(thread) ? 'running' : '';
}
/** The tab's title: the side chat's own, "Side chat" until it has one. */
export const sideChatTabTitle = (client: T3Client, threadId: string): string => str(shellThread(client, threadId)?.title, 'Side chat');

export type SideChatRow = { id: string; kind: string; text: string };
export type SideChatChoice = { label: string; value: string };
export type SideChatRequest = { requestId: string; kind: string; title: string; detail: string; choices: SideChatChoice[] };
export type SideChatView = {
  show: boolean; threadId: string; title: string; deleted: boolean; loading: boolean; contextFrom: string;
  bringBackDisabled: boolean; bringBackTip: string; bringingBack: boolean; deleteDisabled: boolean;
  confirmDelete: boolean; deleteTitle: string; deleteBody: string;
  rows: SideChatRow[]; working: boolean; requests: SideChatRequest[]; canSend: boolean; placeholder: string;
  markdownSource: MarkdownSource[]; md: MarkdownEnv;
};
export const emptySideChat = (): SideChatView => ({ show: false, threadId: '', title: '', deleted: false, loading: false, contextFrom: '',
  bringBackDisabled: true, bringBackTip: '', bringingBack: false, deleteDisabled: true, confirmDelete: true, deleteTitle: '', deleteBody: '',
  rows: [], working: false, requests: [], canSend: false, placeholder: '', markdownSource: [],
  md: { codeFont: 'ui-monospace', codeSize: 13, wrap: true, chips: [], threadGlyphs: [], runCommands: [], inks: [] } });

const docId = (threadId: string, rowId: string) => `side-chat:${threadId}:${rowId}`;

/** The pane of the side chat `threadId` whose parent is `parentId`. */
export function sideChatView(client: T3Client, parentId: string, threadId: string): SideChatView {
  const store = sideChatStore(client), shell = shellThread(client, threadId), parent = shellThread(client, parentId);
  if (shell) store.starting.delete(threadId);
  const title = str(shell?.title, 'Side chat');
  // useEnvironmentShellHasSnapshot: before the shell is in, or while this client started it, nothing says it is gone.
  const deleted = !shell && client.ready && !store.starting.has(threadId);
  const kept = keptThread(client, threadId), projection = kept?.projection ?? null;
  const writable = canOperate(client);
  const reason = bringBackBlockedReason({ canOperate: writable, parentActive: threadActive(parent), mergeRun: latestMergeBackRun(projection) });
  const confirm = (client.local as { clientSettings?: { confirmThreadDelete?: unknown } }).clientSettings?.confirmThreadDelete !== false;
  // A side chat's inherited rows collapse into its "Context from" link; its own rows follow.
  const own = messages(kept).filter(row => !row.sourceThreadId || row.sourceThreadId === threadId);
  const rows: SideChatRow[] = own.flatMap(row => row.kind === 'user' || row.kind === 'assistant' ? [{ id: row.id, kind: row.kind, text: row.body }]
    : row.kind === 'error' ? [{ id: row.id, kind: 'error', text: row.body || 'The turn failed.' }]
      : row.kind === 'tool' || row.kind === 'reasoning' ? [{ id: row.id, kind: 'note', text: row.title }] : []);
  const pending = projection ? pendingRequests(projection) : { approvals: [], inputs: [] };
  const requests: SideChatRequest[] = [
    ...pending.approvals.map(approval => ({ requestId: approval.requestId, kind: 'approval', title: 'Approval requested', detail: approval.detail || approval.reason,
      choices: approval.live ? approval.options.map(option => ({ label: str(option.label), value: str(option.decision) })) : [] })),
    ...pending.inputs.slice(0, 1).map(input => ({ requestId: input.requestId, kind: 'input', title: input.questions[0]?.header || 'Input requested',
      detail: input.questions.map(question => question.question).join('\n\n'),
      choices: input.questions.length === 1 && !input.questions[0]!.multiSelect ? input.questions[0]!.options.map(option => ({ label: option.label, value: option.value })) : [] })),
  ];
  return {
    show: true, threadId, title, deleted, loading: !!shell && !kept, contextFrom: str(parent?.title, 'Parent thread'),
    bringBackDisabled: reason !== null || store.busy.has(threadId), bringBackTip: reason ?? "Adds this side chat to the main thread's next message.",
    bringingBack: store.busy.get(threadId) === 'bring-back', deleteDisabled: !writable || !shell || store.busy.has(threadId),
    confirmDelete: confirm, deleteTitle: `Delete side chat "${title}"?`, deleteBody: 'This permanently clears its conversation history.',
    rows, working: !!projection && !!activeRun(projection), requests, canSend: writable && !!shell,
    placeholder: pending.inputs.length ? 'Type your answer' : 'Ask anything',
    markdownSource: rows.filter(row => row.kind === 'user' || row.kind === 'assistant').map(row => ({ id: docId(threadId, row.id), kind: row.kind === 'user' ? 'user' : 'assistant', title: '', body: row.text })),
    md: markdownEnv(client),
  };
}
