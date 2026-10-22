// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/worktree-cleanup.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Deleting a thread's worktree with it (task thread-commands-and-keys, G5),
// adapted from T3 Code 1e2ecbd975 (MIT); see LICENSE-T3. Sources:
// apps/web/src/worktreeCleanup.ts (getOrphanedWorktreePathForThread and
// formatWorktreePathForDisplay, unchanged), apps/web/src/hooks/useThreadActions.ts
// deleteThread (the orphan check, the "Delete the worktree too?" confirmation,
// the session detach before the delete and the removeWorktree / refreshStatus
// cleanup with its two failure toasts), packages/shared/src/projectSettings.ts
// resolveWorktreeCleanup (via domain.ts effectiveWorktreeRules) and
// packages/client-runtime/src/operations/commands.ts stopThreadSession.
// Changes: the confirmation is the sidebar's dialog (kind "delete-worktree"),
// answered by a later command instead of an awaited promise, so the deletion
// that asked is resumed from `worktreePrompt`; closing terminals with their
// history is one hook (`closeThreadTerminals`), set by terminal-drawer-view.ts.
import { arr, effectiveWorktreeRules, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Files, Native } from './protocol';
import { pushToast } from './toast';
import { shellRuntime } from './sidebar-model';
import { isScratch, scratchRootOf } from './r12-threads-scratch';
import { letGo } from './let-go';

function normalizeWorktreePath(path: unknown): string | null {
  const trimmed = typeof path === 'string' ? path.trim() : '';
  return trimmed ? trimmed : null;
}

/** getOrphanedWorktreePathForThread: the thread's worktree when no other given thread uses it. */
export function getOrphanedWorktreePathForThread(threads: ReadonlyArray<{ id?: unknown; worktreePath?: unknown }>, threadId: string): string | null {
  const targetThread = threads.find(thread => thread.id === threadId);
  if (!targetThread) return null;
  const targetWorktreePath = normalizeWorktreePath(targetThread.worktreePath);
  if (!targetWorktreePath) return null;
  const isShared = threads.some(thread => thread.id !== threadId && normalizeWorktreePath(thread.worktreePath) === targetWorktreePath);
  return isShared ? null : targetWorktreePath;
}

/** formatWorktreePathForDisplay: the path's last segment (either separator), else the path. */
export function formatWorktreePathForDisplay(worktreePath: string): string {
  const trimmed = worktreePath.trim();
  if (!trimmed) return worktreePath;
  const normalized = trimmed.replace(/\\/g, '/').replace(/\/+$/, '');
  const parts = normalized.split('/');
  const lastPart = parts[parts.length - 1]?.trim() ?? '';
  return lastPart.length > 0 ? lastPart : trimmed;
}

/** The ConfirmDialogHost copy of deleteThread's message (the line ending in "?" is the title). */
export const WORKTREE_DIALOG_TITLE = 'Delete the worktree too?';
export const worktreeDialogDescription = (display: string) => `This thread is the only one linked to this worktree:\n${display}`;

export type WorktreePlan = { path: string; display: string; cwd: string };
/**
 * deleteThread's decision for one thread: the orphaned worktree (among the
 * threads that survive this run), whether it can be removed (the project is
 * known and is not the Scratch folder) and whether to ask (the project's
 * "on delete" cleanup rule is off; when it is on the server cleans up).
 * Null when nothing is to be asked or removed.
 */
export function worktreePlan(client: T3Client, threadId: string, deleted: ReadonlySet<string>): WorktreePlan | null {
  const threads = client.shell.threads.filter(thread => thread.id === threadId || (!thread.archivedAt && !deleted.has(str(thread.id))));
  const path = getOrphanedWorktreePathForThread(threads, threadId);
  const thread = client.shell.threads.find(entry => entry.id === threadId);
  const project = thread ? client.shell.projects.find(entry => entry.id === thread.projectId) : undefined;
  if (!path || !project || isScratch(project, scratchRootOf(client.connection === 'connected', obj(client.config)))) return null;
  const settings = obj(obj(client.config).settings);
  const override = obj(obj(settings.projectSettingsOverrides)[str(project.id)]);
  const automatic = Object.keys(settings).length > 0 && effectiveWorktreeRules(settings, override).worktreeOnDelete === true;
  if (automatic) return null;
  return { path, display: formatWorktreePathForDisplay(path), cwd: str(project.workspaceRoot) };
}

/** The deletion paused on the worktree question: the thread asked about, the plan, and the run to resume. */
export type WorktreePrompt = { threadId: string; plan: WorktreePlan; queue: string[]; deleted: Set<string>; bulk: boolean; firstFailure: { ok: false; error: unknown } | null };
const prompts = new WeakMap<T3Client, WorktreePrompt>();
export function worktreePrompt(client: T3Client): WorktreePrompt | undefined { return prompts.get(client); }
export function setWorktreePrompt(client: T3Client, prompt: WorktreePrompt | undefined): void {
  if (prompt) prompts.set(client, prompt); else prompts.delete(client);
}

/** stopThreadSession: detach every provider session of a thread that has a runtime. Failures do not stop the deletion. */
export async function detachThreadSessions(client: T3Client, native: Native, thread: Obj): Promise<void> {
  if (shellRuntime(thread) === null) return;
  const threadId = str(thread.id);
  try {
    const access = client.restAccess(native);
    const projection = threadId === client.threadId ? client.projection : obj(await access.request('orchestration.getThreadProjection', { threadId }));
    const sessions = arr(obj(projection).providerSessions);
    if (!sessions.length) return;
    const [commandId] = await access.ids(1);
    for (const session of sessions) {
      await access.request('orchestration.dispatchCommand', { type: 'provider-session.detach', commandId: `${commandId}:detach:${str(session.id)}`, threadId,
        providerSessionId: str(session.id), reason: 'client-requested' }, true);
    }
  } catch { /* the reference awaits the detach and ignores its result */ }
}

/**
 * closeTerminal({ threadId, deleteHistory: true }) before the delete. The clone
 * has no thread terminal drawer yet (gap TN3): 20261005-terminal-drawer
 * replaces this hook with its `terminal.close` call.
 */
export let closeThreadTerminals: (client: T3Client, native: Native, threadId: string) => Promise<void> = async () => undefined;
export function setCloseThreadTerminals(hook: typeof closeThreadTerminals): void { closeThreadTerminals = hook; }

/**
 * After the thread is gone: vcs.removeWorktree (forced), then vcs.refreshStatus
 * on the project root. A failure is its own stacked toast and never turns the
 * deletion into an error.
 */
export async function removeOrphanedWorktree(client: T3Client, native: Native, storage: Files, plan: WorktreePlan): Promise<void> {
  void storage;
  const access = client.restAccess(native);
  let removed = false;
  try {
    await access.request('vcs.removeWorktree', { cwd: plan.cwd, path: plan.path, force: true }, true);
    removed = true;
    await access.request('vcs.refreshStatus', { cwd: plan.cwd });
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error && error.message ? error.message : 'An error occurred.';
    pushToast(client, { kind: 'error', stacked: true, title: removed ? 'Worktree deleted, but Git status refresh failed' : 'Failed to delete worktree',
      description: removed ? message : `Could not remove ${plan.display}. ${message}` });
  }
}
