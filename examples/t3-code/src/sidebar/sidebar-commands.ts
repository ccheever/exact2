// Thread lifecycle from the sidebar and the thread action menu (Sidebar.tsx
// attempt*/handleThreadContextMenu/handleMultiSelectContextMenu,
// hooks/useThreadActions.ts, hooks/showThreadUndoNotice.ts; MIT, see
// LICENSE-T3). Every action targets its own thread, dispatches the reference
// command, and reports a failure as the reference's toast.
import { arr, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { ClientError, type Files, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { canSnooze, capabilities, latestRun, orderKeyBetween, planReorder, wokeAt } from './sidebar-model';
import { bulkMenuState, canArchive, threadMenuState } from './sidebar-menu';
import { wall, adoptCommandTime, closeDialog, localDate, openSnoozeDialog, resolveCustomSnooze, sidebarPrefs, sidebarSession, undoLive, SETTLED_TAIL_PAGE_COUNT, type UndoAction } from './sidebar-state';
import { calendarLocal } from './snooze-calendar';
export { clock, undoLive } from './sidebar-state';
import { partition, projectScopes, renderedRows, searchRows } from './sidebar-view';
import { snoozePresets } from './sidebar-presentation';
import { sidebarDrop } from './sidebar-drop';
import { dismissProviderPill, scheduleProviderPill } from './sidebar-provider-pill';
import { syncFavicons } from './sidebar-glyph';
import { syncBranchPullRequests } from './sidebar-branch-pr';
import { resetSidebarWidth } from './sidebar-width';
import { sweepRelease } from './row-sweep';
import { discardDraft, draftChoice } from './draft-rows';
import { legacyCommand, legacyLocal } from './legacy-sidebar-commands'; // The "Sidebar (legacy)" gestures
import { legacyEnabled, legacyProjectOrder, threadSortOrder } from './legacy-sidebar-view';
import { archiveBackground, backgroundThreadPath, backgroundWorktreePlan, envAccess, isBackground, markUnreadBackground, openBackgroundDraft, removeBackground } from './legacy-sidebar-fleet';
import { openBranchDraft, rowThread } from './legacy-sidebar-links';
import { dropHeldRename, startHeldRename, startRename } from './sidebar-rename-wait';
import { openPendingPort, watchRowPorts } from './legacy-sidebar-ports';
import { closeThreadTerminals, detachThreadSessions, removeOrphanedWorktree, setWorktreePrompt, worktreePlan, worktreePrompt, type WorktreePrompt } from './worktree-cleanup';
import { deleteSelectedThreadEntries, getFallbackThreadIdAfterDelete } from './sidebar-delete-logic';
import { openHome } from '../pages/home-landing';
import { focusFleetThread, parseFleetThreadId } from '../connections/settings-b-fleet';
import { letGo } from '../client/let-go';

const failure = (error: unknown) => error instanceof Error ? error.message : 'An error occurred.';
const threadOf = (client: T3Client, id: string): Obj | undefined => client.shell.threads.find(thread => thread.id === id);
/** The failure toast; a let-go request is rethrown instead (let-go.ts), so nothing after it runs. */
const toast = (client: T3Client, title: string, error: unknown) => { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title, description: failure(error) }); };

/**
 * A row's thread and its environment (useThreadActions' ScopedThreadRef): the focused environment's, or a background
 * one's (`fleet:<environment>:<id>`), whose commands go through that environment's own transport with its own id.
 * `row` is the thread as the sidebar shows it (the row's id and project id).
 */
interface Target { threadId: string; environmentId: string; background: boolean; thread: Obj; row: Obj; config: Obj; threads: Obj[]; send(native: Native, payload: Obj): Promise<void> }
export function targetOf(client: T3Client, id: string): Target | null {
  const found = rowThread(client, id);
  if (!found) return null;
  const { environmentId, threadId } = found.ref, background = !found.focused, thread = found.thread;
  const row = background ? { ...thread, id, projectId: `fleet:${environmentId}:${str(thread.projectId)}` } : thread;
  const threads = background ? arr(obj(found.reads.shell).threads) : client.shell.threads;
  // The transport is chosen when a command is sent, not when the row was read: an undo can run after the focus moved,
  // so a background row's environment may since be the focused one, or the focused row's a background one.
  return { threadId, environmentId, background, thread, row, config: found.reads.config, threads, send: async (native, payload) => {
    if (environmentId === client.environmentId) return dispatch(client, native, { ...payload, threadId });
    const access = envAccess(client, native, environmentId);
    if (!access) throw new ClientError('That environment is no longer connected.');
    await access.dispatch({ ...payload, threadId });
  } };
}
const unavailable = () => new ClientError('That thread is no longer available.');

/** Session navigation the client exposes for the sidebar (select-thread / new-thread without a row gesture). */
interface Navigator { openSelected?(native: Native, id: string): Promise<void>; openDraft?(native: Native, projectId: string): Promise<void> }

async function dispatch(client: T3Client, native: Native, payload: Obj): Promise<void> {
  if (!client.writable) throw new ClientError(client.connection !== 'connected' ? 'Reconnect before making changes.' : 'Wait for synchronization and check your connection permissions.');
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.request('orchestration.dispatchCommand', { ...payload, commandId }, true);
}
async function attempt(client: T3Client, title: string, run: () => Promise<void>): Promise<boolean> {
  try { await run(); return true; } catch (error) { toast(client, title, error); return false; }
}

// ── Undo notice (showThreadUndoNotice): consecutive actions of one kind merge; 5s life.
type Undo = (native: Native) => Promise<void>;
const undos = new WeakMap<T3Client, { action: UndoAction; threadId: string; undo: Undo; failureTitle: string; commit?: () => void }[]>();
/** Upstream 95edeb753b: `commit` runs once the action can no longer be undone (expired or replaced). */
export function remember(client: T3Client, native: Native, action: UndoAction, threadId: string, failureTitle: string, undo: Undo, commit?: () => void): void {
  const now = wall(client), session = sidebarSession(client);
  const live = session.undo && now - session.undo.at < 5000 ? undos.get(client) ?? [] : [];
  for (const entry of undos.get(client) ?? []) if (!live.includes(entry) || entry.threadId === threadId) entry.commit?.();
  const kept = live.filter(entry => entry.threadId !== threadId);
  kept.push({ action, threadId, undo, failureTitle, ...(commit ? { commit } : {}) });
  undos.set(client, kept);
  const group = [] as string[];
  for (let index = kept.length - 1; index >= 0 && kept[index]!.action === action; index--) group.push(kept[index]!.threadId);
  session.undo = { action, threadIds: group, at: now };
  void native.later({ op: 'sidebarNotify', delay: 5100 }).catch(() => undefined);
}
function forget(client: T3Client, threadId: string): void {
  const session = sidebarSession(client), list = (undos.get(client) ?? []).filter(entry => entry.threadId !== threadId);
  undos.set(client, list);
  if (session.undo) { session.undo.threadIds = session.undo.threadIds.filter(id => id !== threadId); if (!session.undo.threadIds.length) session.undo = null; }
}
/** thread.undo / the inline Undo: reverts the displayed group once, with this command's module handle. */
export async function undoLatest(client: T3Client, native: Native): Promise<boolean> {
  const session = sidebarSession(client);
  if (!undoLive(client)) { session.undo = null; commitExpiredUndos(client); return false; }
  const action = session.undo!.action, list = undos.get(client) ?? [];
  const group: typeof list = [];
  for (let index = list.length - 1; index >= 0 && list[index]!.action === action; index--) group.push(list[index]!);
  undos.set(client, list.filter(entry => !group.includes(entry)));
  session.undo = null;
  await Promise.all(group.map(async entry => { try { await entry.undo(native); } catch (error) { toast(client, entry.failureTitle, error); } }));
  return true;
}

/** showThreadUndoNotice's expiry: actions past their five seconds commit (a discarded draft's uploads are released). */
export function commitExpiredUndos(client: T3Client): void {
  if (undoLive(client)) return;
  const list = undos.get(client) ?? [];
  if (!list.length) return;
  undos.set(client, []);
  for (const entry of list) entry.commit?.();
}

// ── Single-thread lifecycle (useThreadActions.ts).
function topOfPinnedRun(threads: Obj[]): string | undefined {
  let first: string | null = null;
  for (const thread of threads) if (thread.pinnedAt != null && thread.pinOrderKey != null && (first === null || str(thread.pinOrderKey) < first)) first = str(thread.pinOrderKey);
  return orderKeyBetween(null, first) ?? undefined;
}
export async function pin(client: T3Client, native: Native, id: string, orderKey?: string): Promise<void> {
  const target = targetOf(client, id), caps = capabilities(target?.config ?? client.config);
  if (!caps.pinning) throw new ClientError("This environment's server does not support pinning threads yet. Update the server to use it.");
  if (!target) throw unavailable();
  const key = caps.pinReorder ? orderKey ?? topOfPinnedRun(target.threads) : undefined;
  forget(client, id);
  await target.send(native, { type: 'thread.pin', ...(key !== undefined ? { orderKey: key } : {}) });
}
export async function unpin(client: T3Client, native: Native, id: string): Promise<void> {
  const target = targetOf(client, id);
  if (!capabilities(target?.config ?? client.config).pinning) throw new ClientError("This environment's server does not support pinning threads yet. Update the server to use it.");
  if (!target) throw unavailable();
  const key = target.thread.pinOrderKey;
  await target.send(native, { type: 'thread.unpin' });
  remember(client, native, 'Unpinned', id, 'Failed to undo unpin', later => pin(client, later, id, key == null ? undefined : str(key)));
}
function markVisited(client: T3Client, id: string, at: string): void { sidebarPrefs(client).visited[id] = at; }
/**
 * useAcknowledgeThreadWoke: clears a Woke marker by recording a visit at the
 * wake time. A visited-tracking server owns the watermark (thread.visit keeps
 * the later value, so every device clears); older servers keep the local one.
 * A failed visit is not reported, as the reference's mutation is not.
 */
export async function acknowledgeWoke(client: T3Client, native: Native, id: string, woke: string): Promise<void> {
  const target = targetOf(client, id);
  if (!target || !capabilities(target.config).visitedTracking) { markVisited(client, id, woke); return; }
  try { await target.send(native, { type: 'thread.visit', visitedAt: woke }); } catch { /* reportFailure: false */ }
}
/** settleThread: the undo notice puts back the pin and the snooze the settle dropped, on the thread's own environment. */
export async function settle(client: T3Client, native: Native, id: string): Promise<void> {
  const target = targetOf(client, id);
  if (!capabilities(target?.config ?? client.config).settlement) throw new ClientError('This server does not support settling threads.');
  if (!target) throw unavailable();
  const thread = target.thread;
  const woke = wokeAt(thread, wall(client)), wasPinned = thread.pinnedAt != null, key = thread.pinOrderKey, snoozedUntil = thread.snoozedUntil;
  forget(client, id);
  await target.send(native, { type: 'thread.settle' });
  if (woke) markVisited(client, id, woke);
  remember(client, native, 'Settled', id, 'Failed to undo settle', async later => {
    await target.send(later, { type: 'thread.unsettle', reason: 'user' });
    if (wasPinned) await pin(client, later, id, key == null ? undefined : str(key));
    if (snoozedUntil != null) await target.send(later, { type: 'thread.snooze', snoozedUntil: str(snoozedUntil) });
  });
}
export async function snooze(client: T3Client, native: Native, id: string, until: string): Promise<void> {
  const target = targetOf(client, id);
  if (!capabilities(target?.config ?? client.config).snooze) throw new ClientError('This server does not support snoozing threads.');
  if (!Number.isFinite(Date.parse(until))) throw new ClientError('Choose when to bring this thread back.');
  if (!target) throw unavailable();
  // useThreadActions snoozeThread: the server's invariant, checked before the round trip (ThreadSnoozeBlockedError).
  if (!canSnooze(target.thread, wall(client))) throw new ClientError('This thread is waiting on you. Respond to the pending request before snoozing it.');
  forget(client, id);
  await target.send(native, { type: 'thread.snooze', snoozedUntil: new Date(until).toISOString() });
  remember(client, native, 'Snoozed', id, 'Failed to wake thread', later => target.send(later, { type: 'thread.unsnooze', reason: 'user' }));
}
/** unsettleThread / the Wake action, on the thread's own environment. */
async function promote(client: T3Client, native: Native, id: string, type: 'thread.unsettle' | 'thread.unsnooze'): Promise<void> {
  const target = targetOf(client, id);
  if (!target) throw unavailable();
  await target.send(native, { type, reason: 'user' });
}
export async function archive(client: T3Client, native: Native, id: string): Promise<void> {
  if (isBackground(client, id)) return archiveBackground(client, native, id, remember);
  const thread = threadOf(client, id);
  if (!thread) return;
  if (!canArchive(thread)) throw new ClientError('Stop the running turn before archiving this thread.');
  const wasOpen = client.threadId === id;
  forget(client, id);
  await dispatch(client, native, { type: 'thread.archive', threadId: id });
  const woke = wokeAt(thread, wall(client));
  if (woke) markVisited(client, id, woke);
  remember(client, native, 'Archived', id, 'Failed to undo archive', async later => {
    await dispatch(client, later, { type: 'thread.unarchive', threadId: id });
    if (wasOpen) await (client as unknown as Navigator).openSelected?.(later, id);
  });
  if (wasOpen && client.threadId === id) {
    try { await (client as unknown as Navigator).openDraft?.(native, str(thread.projectId)); }
    catch (error) { toast(client, 'Thread archived, but navigation failed', error); }
  }
}
/**
 * useThreadActions.deleteThread for one shown thread: the worktree question
 * (asked once, then resumed with its `answer`), the session detach, the
 * delete, the fallback navigation, then the orphaned worktree's removal.
 * `deleted` holds this run's earlier successes. 'prompt' leaves the thread
 * in place while the "Delete the worktree too?" dialog is open.
 */
export async function remove(client: T3Client, native: Native, storage: Files, id: string, deleted: ReadonlySet<string> = new Set(), answer?: boolean): Promise<'deleted' | 'prompt' | 'missing'> {
  if (isBackground(client, id)) return removeBackground(client, native, id, deleted, answer); // A background environment's row
  const thread = threadOf(client, id);
  if (!thread) return 'missing';
  const plan = worktreePlan(client, id, deleted);
  if (plan && answer === undefined) return 'prompt';
  await detachThreadSessions(client, native, thread);
  await closeThreadTerminals(client, native, id);
  const wasOpen = client.threadId === id;
  const fallback = getFallbackThreadIdAfterDelete({ threads: client.shell.threads.filter(entry => !entry.archivedAt), deletedThreadId: id, deletedThreadIds: deleted, sortOrder: threadSortOrder(client) });
  await dispatch(client, native, { type: 'thread.delete', threadId: id });
  forget(client, id);
  delete client.local.drafts[`${client.environmentId}:${id}`];
  const session = sidebarSession(client);
  session.selection = session.selection.filter(selected => selected !== id);
  if (wasOpen && client.threadId === id) {
    const navigator = client as unknown as Navigator;
    if (fallback) await navigator.openSelected?.(native, fallback);
    // No thread left in the project: the home route `/` (IndexDraftLanding), a draft in the most recently active project.
    else await openHome(client, native, { ...client.shell, threads: client.shell.threads.filter(entry => entry.id !== id && !deleted.has(str(entry.id))) });
  }
  if (plan && answer === true) await removeOrphanedWorktree(client, native, storage, plan);
  return 'deleted';
}

/**
 * Deletes `ids` in order (a single delete, or the multi-select's
 * deleteSelectedThreadEntries), stopping at a worktree question and resuming
 * from `answerWorktree`. Failures end as one toast: "Failed to delete
 * thread", or "Failed to delete threads" for a selection.
 */
export async function deleteThreads(client: T3Client, native: Native, storage: Files, ids: string[], bulk: boolean, resume?: WorktreePrompt, answer?: boolean): Promise<void> {
  const session = sidebarSession(client);
  let pending = answer;
  const outcome = await deleteSelectedThreadEntries({
    entries: ids.map(threadKey => ({ threadKey })),
    start: resume ? { deletedThreadKeys: resume.deleted, firstFailure: resume.firstFailure } : undefined,
    delete: async ({ threadKey }, deletedThreadKeys) => {
      const reply = pending;
      pending = undefined;
      try {
        const result = await remove(client, native, storage, threadKey, deletedThreadKeys, reply);
        return result === 'missing' ? null : result === 'prompt' ? { paused: true } : { ok: true };
      } catch (error) { return { ok: false, error }; }
    },
  });
  if (outcome.pausedAt >= 0) {
    const threadId = ids[outcome.pausedAt]!, plan = isBackground(client, threadId) ? backgroundWorktreePlan(client, threadId, outcome.deletedThreadKeys) : worktreePlan(client, threadId, outcome.deletedThreadKeys);
    if (plan) {
      setWorktreePrompt(client, { threadId, plan, queue: ids.slice(outcome.pausedAt), deleted: outcome.deletedThreadKeys, bulk, firstFailure: outcome.firstFailure });
      session.dialog = { kind: 'delete-worktree', threadIds: [threadId], title: plan.display };
      return;
    }
  }
  setWorktreePrompt(client, undefined);
  if (outcome.firstFailure) toast(client, bulk ? 'Failed to delete threads' : 'Failed to delete thread', outcome.firstFailure.error);
  if (bulk) session.selection = session.selection.filter(id => !!targetOf(client, id) && !outcome.deletedThreadKeys.has(id));
}
/** The worktree question's answer: Confirm removes the worktree; Cancel (or closing) keeps it. Either way the thread is deleted and the run goes on. */
async function answerWorktree(client: T3Client, native: Native, storage: Files, confirmed: boolean): Promise<void> {
  const prompt = worktreePrompt(client);
  closeDialog(sidebarSession(client));
  setWorktreePrompt(client, undefined);
  if (prompt) await deleteThreads(client, native, storage, prompt.queue, prompt.bulk, prompt, confirmed);
}

/** planForwardNavigation: parking the open thread moves to the next remaining card, else a fresh draft. */
function planForward(client: T3Client, nativeHandle: Native, id: string, coParking: Set<string>): (() => Promise<void>) | null {
  if (client.threadId !== id) return null;
  const parts = partition(client, wall(client)), rows = renderedRows(client, parts);
  const keys = rows.map(entry => str(entry.thread.id)), at = keys.indexOf(id);
  const parked = new Set([...parts.settled, ...parts.snoozed].map(thread => str(thread.id)));
  const next = at < 0 ? null : [...keys.slice(at + 1), ...keys.slice(0, at)].find(key => !parked.has(key) && !coParking.has(key) && key !== id) ?? null;
  const thread = threadOf(client, id), navigator = client as unknown as Navigator;
  return async () => {
    if (client.threadId !== id) return;
    // The next card may be a background environment's: it opens there, as its row's press does (a `fleet:` id is no
    // thread of this environment's shell, so openSelected would show the empty state and fetch nothing).
    if (next && parseFleetThreadId(next)) {
      const focused = await focusFleetThread(client, nativeHandle, next);
      (client as unknown as { adoptStatus?(value: Obj, generation: number): void }).adoptStatus?.(focused.value, focused.generation);
    } else if (next) await navigator.openSelected?.(nativeHandle, next);
    else if (thread) await navigator.openDraft?.(nativeHandle, str(thread.projectId));
  };
}
/** Settle or snooze with the reference's toast and forward navigation (drag and drop's settle too). */
export function parkThread(client: T3Client, nativeHandle: Native, id: string, kind: 'settle' | 'snooze', until: string): Promise<boolean> {
  return park(client, nativeHandle, id, kind, until, new Set());
}
export async function park(client: T3Client, nativeHandle: Native, id: string, kind: 'settle' | 'snooze', until: string, coParking: Set<string>): Promise<boolean> {
  const session = sidebarSession(client), key = `${kind}:${id}`;
  if (session.busy.has(key)) return false;
  session.busy.add(key);
  try {
    const forward = planForward(client, nativeHandle, id, coParking);
    const ok = await attempt(client, kind === 'settle' ? 'Failed to settle thread' : 'Failed to snooze thread',
      () => kind === 'settle' ? settle(client, nativeHandle, id) : snooze(client, nativeHandle, id, until));
    if (ok && forward) await forward();
    return ok;
  } finally { session.busy.delete(key); }
}

export async function rename(client: T3Client, nativeHandle: Native, id: string, title: string): Promise<void> {
  const session = sidebarSession(client);
  if (session.renameId !== id) return;
  session.renameId = ''; session.renameTitle = '';
  const target = targetOf(client, id), trimmed = title.trim();
  if (!trimmed) { pushToast(client, { kind: 'warning', title: 'Thread title cannot be empty' }); return; }
  if (!target || trimmed === str(target.thread.title)) return;
  await attempt(client, 'Failed to rename thread', () => target.send(nativeHandle, { type: 'thread.metadata.update', title: trimmed }));
}
async function regenerate(client: T3Client, nativeHandle: Native, ids: string[], bulk: boolean): Promise<void> {
  const session = sidebarSession(client);
  for (const id of ids) {
    session.regenerating.add(id);
    const ok = await attempt(client, bulk ? 'Failed to regenerate thread titles' : 'Failed to regenerate thread title',
      async () => { const target = targetOf(client, id); if (!target) throw unavailable(); await target.send(nativeHandle, { type: 'thread.metadata.update', regenerateTitle: true }); });
    session.regenerating.delete(id);
    if (!ok) return;
  }
}
/**
 * useMarkThreadUnread: every Mark unread (row, multi-selection and thread menus, any environment) is fire and forget
 * with `reportFailure: false`, so a refusal, such as the server's for a thread with no completed run, shows nothing.
 */
export async function markUnread(client: T3Client, nativeHandle: Native, id: string): Promise<void> {
  try {
    if (isBackground(client, id)) { await markUnreadBackground(client, nativeHandle, id); return; }
    if (capabilities(client.config).visitedTracking) { await dispatch(client, nativeHandle, { type: 'thread.mark-unread', threadId: id }); return; }
    const completed = latestRun(threadOf(client, id) ?? {})?.completedAt;
    if (completed) sidebarPrefs(client).visited[id] = new Date(Date.parse(completed) - 1).toISOString();
  } catch (error) { if (letGo(error)) throw error; }
}
async function copy(client: T3Client, nativeHandle: Native, text: string, title: string): Promise<void> {
  try { await client.restAccess(nativeHandle).call({ op: 'copyText', text }); pushToast(client, { kind: 'success', title }); }
  catch (error) { toast(client, 'Failed to copy', error); }
}

// ── Visits (ChatView visit effect): the open thread's watermark, deduped per updatedAt, throttled 10s mid-turn.
// The effect runs whenever the open thread's shell changes, so it is asked after every refresh too: a thread
// opened on a background environment arrives by the reconnect to that environment, not by openSelected, and
// a turn completing on the open thread lands as a shell event.
//
// The throttle is for one thread's mid-turn bumps. The reference's applies across threads too, with a trailing timer
// that sends the newly opened thread's visit at the window's end; a data source has no clock to run that timer on
// (`wall` moves only with commands), so a visit for another thread than the last one sent goes at once. Dropped, it
// left the dedupe key on the thread visited before, so that thread, marked unread meanwhile, stayed unread when it
// was opened again.
const visits = new WeakMap<T3Client, { key: string; thread: string; at: number }>();
export function visitOpenThread(client: T3Client, nativeHandle: Native): void {
  const thread = threadOf(client, client.threadId);
  if (!thread || !client.ready) return;
  const updated = Date.parse(str(thread.updatedAt));
  if (!Number.isFinite(updated)) return;
  const prefs = sidebarPrefs(client), tracked = thread.lastVisitedAt !== undefined && capabilities(client.config).visitedTracking;
  const seen = Date.parse(str(tracked ? thread.lastVisitedAt : prefs.visited[str(thread.id)]));
  if (Number.isFinite(seen) && seen >= updated) return;
  if (!tracked) { prefs.visited[str(thread.id)] = str(thread.updatedAt); return; }
  // routeThreadKey: the same thread id on another environment is another thread.
  const route = `${client.environmentId}:${str(thread.id)}`, key = `${route}:${str(thread.updatedAt)}`, last = visits.get(client) ?? { key: '', thread: '', at: 0 };
  if (last.key === key || !client.writable) return;
  const completed = Date.parse(str(latestRun(thread)?.completedAt));
  const unseen = Number.isFinite(completed) && (!Number.isFinite(seen) || completed > seen);
  if (!unseen && last.thread === route && wall(client) - last.at < 10_000) return;
  visits.set(client, { key, thread: route, at: wall(client) });
  void dispatch(client, nativeHandle, { type: 'thread.visit', threadId: str(thread.id), visitedAt: str(thread.updatedAt) }).catch(() => undefined);
}

/**
 * The row press (`value` "click", "click:meta" or "click:shift": the press's own MouseEvent, sidebar-row.contract
 * `openPress`): ⌘-click toggles the multi-selection and ⇧-click extends it in sidebar order; a plain click clears it.
 * "search" opens a result and clears the search. True when the press only selected.
 */
export async function sidebarSelecting(client: T3Client, _nativeHandle: Native, id: string, value: string): Promise<boolean> {
  const session = sidebarSession(client);
  if (value === 'click' || value.startsWith('click:')) {
    const flags = { command: value === 'click:meta', shift: value === 'click:shift' };
    // ⇧-click ranges over the row's project list (rangeSelectTo(threadKey, orderedProjectThreadKeys)).
    const rows = legacyEnabled(client) ? legacyProjectOrder(client, id) : renderedRows(client, partition(client, wall(client))).map(entry => str(entry.thread.id));
    if (flags.command === true) {
      const removing = session.selection.includes(id);
      session.selection = removing ? session.selection.filter(key => key !== id) : [...session.selection, id];
      if (!removing) session.anchor = id;
      return true;
    }
    if (flags.shift === true) {
      const from = rows.indexOf(session.anchor), to = rows.indexOf(id), missing = from < 0 || to < 0;
      session.selection = [...new Set([...session.selection, ...(missing ? [id] : rows.slice(Math.min(from, to), Math.max(from, to) + 1))])];
      if (missing) session.anchor = id;
      return true;
    }
  }
  if (value === 'search') { client.query = ''; session.searchIndex = 0; }
  // A double-click's own presses select the row it is renaming; another row drops the edit.
  session.selection = []; session.anchor = id;
  if (session.renameId !== id) { session.renameId = ''; session.renameTitle = ''; }
  dropHeldRename(client, id);
  return false;
}
/**
 * After each refresh (client.ts, awaited): a success pill's exit is scheduled
 * while this refresh's handle is live, and new projects' favicons and the
 * linkless threads' branch pull requests are asked for inside the refresh (a
 * reply after its answer ended would be dropped).
 */
export async function sidebarRefreshed(client: T3Client, nativeHandle: Native): Promise<void> {
  commitExpiredUndos(client); // The notice's expiry repaint commits what can no longer be undone
  scheduleProviderPill(client, wall(client), delay => { void nativeHandle.later({ op: 'sidebarNotify', delay }).catch(() => undefined); });
  await syncFavicons(client, nativeHandle);
  await syncBranchPullRequests(client, nativeHandle, wall(client)); // A linkless thread's branch pull request
  await watchRowPorts(client, nativeHandle); // The legacy rows' discovered ports
  await openPendingPort(client, nativeHandle); // a background row's port, once its thread is open here
  startHeldRename(client); // a double-click's rename, once its row's environment is in
  visitOpenThread(client, nativeHandle);
}
/** After a thread opens: the server visit lands at once for an unseen completion. */
export function sidebarOpened(client: T3Client, nativeHandle: Native): void { visitOpenThread(client, nativeHandle); }

const menuState = (client: T3Client, thread: Obj) => threadMenuState(client, thread, false, projectScopes(client), wall(client));

/** The sidebar's local ops: view state that never reaches the server. */
export async function sidebarLocal(client: T3Client, _native: Native, op: string, id: string, value: string): Promise<string> {
  if (op.startsWith('legacy-')) return legacyLocal(client, op.slice(7), id, value);
  const prefs = sidebarPrefs(client), session = sidebarSession(client);
  if (op === 'pill-dismiss') { dismissProviderPill(client, value); return ''; }
  if (op === 'shelf') {
    if (id === 'settled') prefs.settledExpanded = !prefs.settledExpanded;
    else if (id === 'snoozed') prefs.snoozedExpanded = !prefs.snoozedExpanded;
    else if (id === 'working') prefs.workingExpanded = !prefs.workingExpanded;
  } else if (op === 'more') session.settledVisible += SETTLED_TAIL_PAGE_COUNT;
  else if (op === 'scope') { prefs.scope = projectScopes(client).some(group => group.key === value) ? value : ''; session.scopeOpen = false; session.scopeQuery = ''; }
  else if (op === 'scope-open') { session.scopeOpen = value === 'close' ? false : !session.scopeOpen; session.scopeQuery = ''; }
  else if (op === 'scope-query') session.scopeQuery = value;
  else if (op === 'scope-key') { if (value === 'Escape') { session.scopeOpen = false; session.scopeQuery = ''; } }
  else if (op === 'width-reset') resetSidebarWidth(client); // Forget the stored width
  else if (op === 'search-hover') {
    const at = client.query.trim() ? searchRows(client, partition(client, wall(client))).findIndex(entry => entry.thread.id === id) : -1;
    if (at >= 0) session.searchIndex = at;
  } else if (op === 'search-key') {
    const count = client.query.trim() ? searchRows(client, partition(client, wall(client))).length : 0;
    if (value === 'Escape') { client.query = ''; session.searchIndex = 0; }
    else if (count && value === 'ArrowDown') session.searchIndex = (session.searchIndex + 1) % count;
    else if (count && value === 'ArrowUp') session.searchIndex = (session.searchIndex - 1 + count) % count;
  } else if (op === 'rename-start') startRename(client, id); // after its first click's open has landed (app.contract)
  else if (op === 'rename-key') { if (value === 'Escape' && session.renameId === id) { session.renameId = ''; session.renameTitle = ''; } }
  else if (op === 'dialog-close') closeDialog(session);
  else if (op.startsWith('calendar-')) calendarLocal(session, op, id, value, localDate(new Date(wall(client)))); // The date popover
  else if (op === 'dialog-field') {
    if (id === 'mode' && (value === 'date' || value === 'duration')) session.dialogMode = value;
    else if (id === 'date') session.dialogDate = value;
    else if (id === 'time') session.dialogTime = value;
    else if (id === 'amount') session.dialogAmount = value;
    else if (id === 'unit' && ['minutes', 'hours', 'days'].includes(value)) session.dialogUnit = value;
    session.dialogError = '';
  }
  else if (op === 'dialog-step') {
    // NumberField (min 0, step any): the stepper moves the amount by one, never below zero.
    const current = Number(session.dialogAmount.trim() || '0');
    if (Number.isFinite(current)) session.dialogAmount = String(Math.max(0, current + (value === '-1' ? -1 : 1)));
    session.dialogError = '';
  }
  else if (op === 'selection-clear') { session.selection = []; }
  else throw new ClientError(`Unknown sidebar action: ${op}`);
  return '';
}

/** The sidebar's server ops. Failures are toasts, never the transcript banner. */
export async function sidebarCommand(client: T3Client, nativeHandle: Native, storage: Files, op: string, id: string, value: string, at = 0): Promise<string> {
  adoptCommandTime(client, at);
  if (op.startsWith('legacy-')) return legacyCommand(client, nativeHandle, storage, op.slice(7), id, value);
  const session = sidebarSession(client);
  const settings = client.local.clientSettings;
  if (op === 'undo') { await undoLatest(client, nativeHandle); return ''; }
  if (op === 'rename') { await rename(client, nativeHandle, id, value); return ''; }
  if (op === 'dialog-confirm') return confirmDialog(client, nativeHandle, storage, value);
  if (op === 'dialog-cancel') { if (session.dialog.kind === 'delete-worktree') await answerWorktree(client, nativeHandle, storage, false); else closeDialog(session); return ''; }
  if (op === 'drop' && id.startsWith('sweep|')) return sweepRelease(client, nativeHandle, storage, id); // upstream 1826fb55cc
  if (op === 'drop') return sidebarDrop(client, nativeHandle, id, value);
  if (op === 'project-settings-group') {
    const group = client.projectGroups().find(entry => entry.key === value), member = group?.members[0];
    session.scopeOpen = false; session.scopeQuery = '';
    if (!member) return '';
    return `sidebar:project-settings:${str(member.id)}`;
  }
  if (op === 'open-draft') {
    try { await (client as unknown as Navigator).openDraft?.(nativeHandle, id); }
    catch (error) { toast(client, 'Could not open draft', error); return ''; }
    return 'sidebar:new-thread';
  }
  if (op === 'discard-project-draft') { discardDraft(client, nativeHandle, `${client.environmentId}:new:${id}`, true); return ''; } // Behind the undo notice
  if (op === 'draft-choice') return draftChoice(client, nativeHandle, id, value); // the draft row's context popover (exact2 #223)
  if (op === 'new-thread-click') {
    // routes/_chat.tsx chat.new: the legacy sidebar keeps the immediate contextual create; ⇧-click (`value` "shift", the
    // press's own MouseEvent, app.contract sidebarNewThread) creates at once.
    if (value !== 'shift' && client.projectGroups().length > 1 && !legacyEnabled(client)) return 'sidebar:palette-new-thread';
    try { await (client as unknown as Navigator).openDraft?.(nativeHandle, client.projectId); }
    catch (error) { toast(client, 'Could not create thread', error); return ''; }
    return 'sidebar:new-thread';
  }
  if (op === 'home') {
    // The index route opens a draft for the most recently active project.
    const latest = projectScopes(client)[0], member = latest ? client.shell.projects.find(project => latest.ids.has(str(project.id))) : undefined;
    try { await (client as unknown as Navigator).openDraft?.(nativeHandle, str(member?.id ?? client.projectId)); }
    catch (error) { toast(client, 'Could not open threads', error); return ''; }
    return 'sidebar:new-thread';
  }
  // A choice from the row's context popover (sidebar-row.contract ThreadMenu, exact2 #223): the row's
  // menu as painted, so a selected row's is the bulk menu, as the right-click's handler decides. The row's
  // thread is on its own environment (handleThreadContextMenu's threadByKey): a background row's choices act there.
  if (op === 'menu-choice') {
    if (session.selection.length > 0 && session.selection.includes(id)) return bulkChoice(client, nativeHandle, storage, value);
    const thread = targetOf(client, id)?.row;
    if (!thread || !value) return '';
    const state = menuState(client, thread);
    return runChoice(client, nativeHandle, storage, thread, value, state.scope?.key ?? '');
  }
  // A row of any environment: a background environment's (`fleet:` id) acts on that environment, as the reference's
  // useThreadActions does with the row's scoped thread ref.
  const thread = targetOf(client, id)?.row;
  if (!thread) { toast(client, 'Thread unavailable', unavailable()); return ''; }
  if (op === 'settle') await park(client, nativeHandle, id, 'settle', '', new Set());
  else if (op === 'snooze') await park(client, nativeHandle, id, 'snooze', value, new Set());
  else if (op === 'unsettle') await attempt(client, 'Failed to un-settle thread', () => promote(client, nativeHandle, id, 'thread.unsettle').then(() => forget(client, id)));
  else if (op === 'unsnooze') await attempt(client, 'Failed to wake thread', () => promote(client, nativeHandle, id, 'thread.unsnooze').then(() => forget(client, id)));
  else if (op === 'pin') await attempt(client, 'Failed to pin thread', () => pin(client, nativeHandle, id));
  else if (op === 'unpin') {
    if (settings?.confirmThreadUnpin && value !== 'confirmed') session.dialog = { kind: 'unpin', threadIds: [id], title: str(thread.title) };
    else await attempt(client, 'Failed to unpin thread', () => unpin(client, nativeHandle, id));
  } else if (op === 'wake-dismiss') { const woke = wokeAt(thread, wall(client)); if (woke) await acknowledgeWoke(client, nativeHandle, id, woke); }
  // The chat header's Snooze › Custom… (useThreadActionMenu's requestCustomSnooze) for its own thread.
  else if (op === 'snooze:custom' && value === 'title') openSnoozeDialog(session, [id], str(thread.title), wall(client), 'title');
  else if (op === 'discard-draft') discardDraft(client, nativeHandle, `${client.environmentId}:${id}`, false); // Behind the undo notice
  else return runChoice(client, nativeHandle, storage, thread, op, '');
  return '';
}

async function runChoice(client: T3Client, nativeHandle: Native, storage: Files, thread: Obj, choice: string, scopeKey: string): Promise<string> {
  const id = str(thread.id), session = sidebarSession(client), prefs = sidebarPrefs(client), settings = client.local.clientSettings;
  if (choice.startsWith('snooze:')) {
    if (choice === 'snooze:custom') { openSnoozeDialog(session, [id], str(thread.title), wall(client)); return ''; }
    const preset = snoozePresets(wall(client), client.local.deviceSettings.timestampFormat).find(entry => `snooze:${entry.id}` === choice);
    if (preset) await park(client, nativeHandle, id, 'snooze', preset.until, new Set());
    return '';
  }
  switch (choice) {
    case 'settle': await park(client, nativeHandle, id, 'settle', '', new Set()); return '';
    case 'unsettle': await attempt(client, 'Failed to un-settle thread', () => promote(client, nativeHandle, id, 'thread.unsettle')); return '';
    case 'unsnooze': await attempt(client, 'Failed to wake thread', () => promote(client, nativeHandle, id, 'thread.unsnooze')); return '';
    case 'pin': await attempt(client, 'Failed to pin thread', () => pin(client, nativeHandle, id)); return '';
    case 'unpin':
      if (settings?.confirmThreadUnpin) { session.dialog = { kind: 'unpin', threadIds: [id], title: str(thread.title) }; return ''; }
      await attempt(client, 'Failed to unpin thread', () => unpin(client, nativeHandle, id)); return '';
    case 'auto-settle:enabled': case 'auto-settle:disabled':
      await attempt(client, 'Failed to update auto-settle', async () => { const target = targetOf(client, id); if (!target) throw unavailable(); await target.send(nativeHandle, { type: 'thread.auto-settle.set', enabled: choice === 'auto-settle:enabled' }); }); return '';
    case 'rename': session.renameId = id; session.renameTitle = str(thread.title); return '';
    case 'regenerate-title': if (thread.titleRegeneration == null) await regenerate(client, nativeHandle, [id], false); return '';
    case 'mark-unread': await markUnread(client, nativeHandle, id); return '';
    case 'filter-by-project': prefs.scope = prefs.scope === scopeKey ? '' : scopeKey; return '';
    case 'copy-path': {
      const path = isBackground(client, id) ? backgroundThreadPath(client, id) : str(thread.worktreePath) || str(client.shell.projects.find(project => project.id === thread.projectId)?.workspaceRoot);
      if (!path) { pushToast(client, { kind: 'error', title: 'Path unavailable', description: 'This thread does not have a workspace path to copy.' }); return ''; }
      await copy(client, nativeHandle, path, 'Path copied'); return '';
    }
    case 'copy-branch': if (thread.branch) await copy(client, nativeHandle, str(thread.branch), 'Branch copied'); return '';
    case 'copy-thread-id': await copy(client, nativeHandle, targetOf(client, id)?.threadId ?? id, 'Thread ID copied'); return '';
    case 'project-settings': return `sidebar:project-settings:${str(thread.projectId)}`;
    case 'new-thread-on-branch': {
      if (!isBackground(client, id)) return openBranchDraft(client, nativeHandle, thread, str(thread.projectId)); // Sidebar.tsx carries the branch the same way
      const worktreePath = str(thread.worktreePath); // a background thread's branch, on its own environment
      try { await openBackgroundDraft(client, nativeHandle, str(thread.projectId), { branch: str(thread.branch), worktreePath, envMode: worktreePath ? 'worktree' : 'local' }); }
      catch (error) { toast(client, 'Could not create thread', error); return ''; }
      return 'sidebar:new-thread';
    }
    case 'archive':
      if (settings?.confirmThreadArchive) { session.dialog = { kind: 'archive', threadIds: [id], title: str(thread.title) }; return ''; }
      await attempt(client, 'Failed to archive thread', () => archive(client, nativeHandle, id)); return '';
    case 'delete':
      if (settings?.confirmThreadDelete !== false) { session.dialog = { kind: 'delete', threadIds: [id], title: str(thread.title) }; return ''; }
      await deleteThreads(client, nativeHandle, storage, [id], false); return '';
    default: throw new ClientError(`Unknown thread action: ${choice}`);
  }
}

/** The selected rows painted now, in the selection's order. */
function selectedRows(client: T3Client, now: number): Obj[] {
  const rendered = new Set(renderedRows(client, partition(client, now)).map(entry => str(entry.thread.id)));
  return sidebarSession(client).selection.filter(id => rendered.has(id)).map(id => targetOf(client, id)?.row).filter((thread): thread is Obj => !!thread);
}
async function bulkChoice(client: T3Client, nativeHandle: Native, storage: Files, choice: string): Promise<string> {
  const session = sidebarSession(client), threads = selectedRows(client, wall(client));
  if (!threads.length) return '';
  const { presets, pinned, regeneratable, ids } = bulkMenuState(client, threads, wall(client)), batch = new Set(ids);
  if (choice.startsWith('snooze:')) {
    if (choice === 'snooze:custom') { openSnoozeDialog(session, ids, '', wall(client)); return ''; }
    const preset = presets.find(entry => `snooze:${entry.id}` === choice);
    session.selection = [];
    if (preset) await snoozeMany(client, nativeHandle, ids, preset.until);
    return '';
  }
  if (choice === 'unpin') { session.selection = []; for (const thread of pinned) await attempt(client, 'Failed to unpin thread', () => unpin(client, nativeHandle, str(thread.id))); return ''; }
  if (choice === 'regenerate-title') { await regenerate(client, nativeHandle, regeneratable.map(thread => str(thread.id)), true); session.selection = []; return ''; }
  if (choice === 'settle') {
    session.selection = [];
    for (const thread of threads) if (thread.settledOverride !== 'settled') await park(client, nativeHandle, str(thread.id), 'settle', '', batch);
    return '';
  }
  if (choice === 'mark-unread') { session.selection = []; for (const id of ids) await markUnread(client, nativeHandle, id); return ''; }
  if (choice === 'delete') {
    if (client.local.clientSettings?.confirmThreadDelete !== false) { session.dialog = { kind: 'delete-many', threadIds: ids, title: '' }; return ''; }
    await deleteThreads(client, nativeHandle, storage, ids, true);
  }
  return '';
}
async function snoozeMany(client: T3Client, nativeHandle: Native, ids: string[], until: string): Promise<void> {
  const batch = new Set(ids), failures: unknown[] = [];
  let snoozed = 0;
  for (const id of ids) {
    const forward = planForward(client, nativeHandle, id, batch);
    try { await snooze(client, nativeHandle, id, until); snoozed++; if (forward) await forward(); } catch (error) { failures.push(error); }
  }
  const letGoFailure = failures.find(letGo);
  if (letGoFailure) throw letGoFailure;
  if (failures.length) pushToast(client, { kind: 'error', title: snoozed > 0 ? `Failed to snooze ${failures.length} thread${failures.length === 1 ? '' : 's'}` : 'Failed to snooze threads', description: failure(failures[0]) });
}
/** The confirmation and custom snooze dialogs' confirm button. */
async function confirmDialog(client: T3Client, nativeHandle: Native, storage: Files, value: string): Promise<string> {
  const session = sidebarSession(client), dialog = session.dialog;
  void value;
  closeDialog(session);
  const [id] = dialog.threadIds;
  if (dialog.kind === 'delete-worktree') { await answerWorktree(client, nativeHandle, storage, true); return ''; }
  if (dialog.kind === 'archive' && id) await attempt(client, 'Failed to archive thread', () => archive(client, nativeHandle, id));
  else if (dialog.kind === 'delete' && id) await deleteThreads(client, nativeHandle, storage, [id], false);
  else if (dialog.kind === 'delete-many') await deleteThreads(client, nativeHandle, storage, dialog.threadIds, true);
  else if (dialog.kind === 'unpin' && id) await attempt(client, 'Failed to unpin thread', () => unpin(client, nativeHandle, id));
  else if (dialog.kind === 'snooze') {
    const resolved = resolveCustomSnooze({ mode: session.dialogMode, date: session.dialogDate, time: session.dialogTime, amount: session.dialogAmount, unit: session.dialogUnit }, wall(client));
    if (!resolved.until) { session.dialog = dialog; session.dialogError = resolved.error; return ''; }
    if (dialog.threadIds.length > 1) { session.selection = []; await snoozeMany(client, nativeHandle, dialog.threadIds, resolved.until); }
    // useThreadActionMenu snoozes with snoozeThread alone: the undo notice, and the open thread stays open.
    else if (id && dialog.from === 'title') { const until = resolved.until; await attempt(client, 'Failed to snooze thread', () => snooze(client, nativeHandle, id, until)); }
    else if (id) await park(client, nativeHandle, id, 'snooze', resolved.until, new Set());
  }
  return '';
}

/** Drag-and-drop placement within Pinned/Active (thread.pin.reorder / thread.active.reorder). */
export async function reorder(client: T3Client, nativeHandle: Native, section: 'pinned' | 'active', orderedIds: string[], movedId: string): Promise<void> {
  const keyName = section === 'pinned' ? 'pinOrderKey' : 'activeOrderKey';
  const keys = new Map(client.shell.threads.filter(thread => section === 'pinned' ? thread.pinnedAt != null : thread.pinnedAt == null)
    .map(thread => [str(thread.id), thread[keyName] == null ? null : str(thread[keyName])] as [string, string | null]));
  for (const assignment of planReorder(orderedIds, keys, movedId)) {
    await dispatch(client, nativeHandle, { type: section === 'pinned' ? 'thread.pin.reorder' : 'thread.active.reorder', threadId: assignment.id, orderKey: assignment.orderKey });
  }
}
