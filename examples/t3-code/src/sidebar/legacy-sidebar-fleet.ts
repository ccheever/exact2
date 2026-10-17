// The legacy sidebar on a background environment (T3 Code 1e2ecbd975; MIT, see LICENSE-T3): every project
// and thread action of LegacySidebar.tsx targets the member's or thread's own environment
// (projectEnvironment.update/delete with `environmentId: member.environmentId`, threadEnvironment.* and
// useThreadActions over a scoped thread ref, handleNewThread(scopeProjectRef(member.environmentId, …))).
// A background environment is one the fleet keeps connected (settings-b-fleet.ts); its rows carry
// `fleet:<environment>:<id>` ids. Requests go through that environment's own transport, as
// desktop-activation.ts's do; a new thread there focuses the environment, as opening its thread does.
import { arr, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { canArchive } from './sidebar-menu';
import { capabilities, latestRun, shellRuntime, wokeAt } from './sidebar-model';
import { sidebarPrefs, sidebarSession, wall } from './sidebar-state';
import { formatWorktreePathForDisplay, worktreePlan, type WorktreePlan } from './worktree-cleanup';
import { EnvironmentFleet, fleet, parseFleetThreadId, type FleetEntry } from '../connections/settings-b-fleet';
import { patchDraftContext, type DraftContext } from '../composer/composer-controls-branch';

/** A background environment's requests, through its own transport and generation. */
export interface EnvAccess {
  environmentId: string; label: string; entry: FleetEntry; config: Obj; shell: { projects: Obj[]; threads: Obj[] };
  request(method: string, payload: Obj): Promise<Obj>;
  dispatch(payload: Obj): Promise<void>;
}
/** The fleet's connected, synchronized entry for an environment that is not the focused one. */
export function backgroundEntry(client: T3Client, environmentId: string, source: EnvironmentFleet = fleet): FleetEntry | null {
  if (!environmentId || environmentId === client.environmentId) return null;
  return [...source.entries.values()].find(entry => entry.environmentId === environmentId && entry.phase === 'connected' && entry.synchronized === entry.generation) ?? null;
}
export function envAccess(client: T3Client, native: Native, environmentId: string, source: EnvironmentFleet = fleet): EnvAccess | null {
  const entry = backgroundEntry(client, environmentId, source);
  if (!entry) return null;
  const call = async (request: Obj): Promise<Obj> => {
    const generation = entry.generation;
    const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { ...request, generation });
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
    if (reply.generation !== generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
    return obj(reply.value);
  };
  const request = (method: string, payload: Obj) => call({ op: 'request', method, payload });
  return {
    environmentId, label: str(obj(entry.config.environment).label), entry, config: entry.config, shell: entry.shell, request,
    dispatch: async payload => { const [commandId] = await client.restAccess(native).ids(1); await request('orchestration.dispatchCommand', { ...payload, commandId }); },
  };
}
const connected = (access: EnvAccess | null): EnvAccess => {
  if (!access) throw new ClientError('That environment is no longer connected.');
  return access;
};
/** A background row id's environment and raw id; null for the focused environment's rows. */
export function backgroundRef(client: T3Client, id: string): { environmentId: string; id: string } | null {
  const ref = parseFleetThreadId(id);
  return ref && ref.environmentId !== client.environmentId ? { environmentId: ref.environmentId, id: ref.threadId } : null;
}
export const isBackground = (client: T3Client, id: string): boolean => backgroundRef(client, id) !== null;
/** A background thread's shell row (its own ids), or undefined. */
export function backgroundThread(client: T3Client, id: string, source: EnvironmentFleet = fleet): Obj | undefined {
  const ref = backgroundRef(client, id), entry = ref ? backgroundEntry(client, ref.environmentId, source) : null;
  return entry?.shell.threads.find(thread => str(thread.id) === ref!.id);
}

// ── Projects (projectEnvironment.update / .delete on the member's environment) ──────────────────
export async function writeBackgroundProject(client: T3Client, native: Native, memberId: string, payload: Obj): Promise<void> {
  const ref = backgroundRef(client, memberId);
  if (!ref) throw new ClientError('That project is not on a background environment.');
  const access = connected(envAccess(client, native, ref.environmentId));
  const [commandId] = await client.restAccess(native).ids(1);
  await access.request('projects.mutate', { ...payload, projectId: ref.id, commandId });
}
/** Threads a background member holds (the "Project is not empty" count and the removal's wording). */
export function backgroundProjectThreads(client: T3Client, memberId: string, source: EnvironmentFleet = fleet): Obj[] {
  const ref = backgroundRef(client, memberId), entry = ref ? backgroundEntry(client, ref.environmentId, source) : null;
  return entry ? entry.shell.threads.filter(thread => str(thread.projectId) === ref!.id) : [];
}
/** releaseProjectDraftUploads / clearProjectDraftThreadId for a removed background member. */
export function forgetBackgroundDrafts(client: T3Client, memberId: string, threads: Obj[]): void {
  const ref = backgroundRef(client, memberId);
  if (!ref) return;
  for (const thread of threads) delete client.local.drafts[`${ref.environmentId}:${str(thread.id)}`];
  delete client.local.drafts[`${ref.environmentId}:new:${ref.id}`];
}
/**
 * The window shows one environment: a background environment becomes the focus (as opening its thread does,
 * select-thread), with `selection` as its selection when given, else the one it last had.
 */
export async function focusBackground(client: T3Client, native: Native, environmentId: string, selection?: { projectId: string; threadId: string }, source: EnvironmentFleet = fleet): Promise<void> {
  const entry = backgroundEntry(client, environmentId, source);
  if (!entry) throw new ClientError('That environment is no longer connected.');
  if (selection) client.local.selections[environmentId] = selection;
  source.forget(entry.key);
  await native.later({ op: 'fleetStop', fleet: entry.key }).catch(() => undefined);
  const reply = await bridgeReply(native, { op: 'connect', origin: entry.origin, ...(entry.primary ? { primary: true } : { credential: '' }) });
  if (!reply.ok) throw new Error(reply.error!.message);
  (client as unknown as { adoptStatus?(value: Obj, generation: number): void }).adoptStatus?.(obj(reply.value), reply.generation);
}
/**
 * handleNewThread(scopeProjectRef(member.environmentId, member.id), options): the project's draft in its own
 * environment, which becomes the focus, with the options' workspace context.
 */
export async function openBackgroundDraft(client: T3Client, native: Native, memberId: string, context?: DraftContext, source: EnvironmentFleet = fleet): Promise<void> {
  const ref = backgroundRef(client, memberId);
  if (!ref || !backgroundEntry(client, ref.environmentId, source)) throw new ClientError('That environment is no longer connected.');
  if (context) patchDraftContext(client, context, `${ref.environmentId}:new:${ref.id}`);
  await focusBackground(client, native, ref.environmentId, { projectId: ref.id, threadId: '' }, source);
}

// ── Threads (useThreadActions on the thread's environment) ────────────────────────────────────
type Remember = (client: T3Client, native: Native, action: 'Archived', threadId: string, failureTitle: string, undo: (later: Native) => Promise<void>) => void;
/** archiveThread: refused while it runs; the undo notice brings it back on its own environment. */
export async function archiveBackground(client: T3Client, native: Native, id: string, remember: Remember): Promise<void> {
  const ref = backgroundRef(client, id), thread = backgroundThread(client, id);
  if (!ref || !thread) return;
  if (!canArchive(thread)) throw new ClientError('Stop the running turn before archiving this thread.');
  await connected(envAccess(client, native, ref.environmentId)).dispatch({ type: 'thread.archive', threadId: ref.id });
  const woke = wokeAt(thread, wall(client));
  if (woke) sidebarPrefs(client).visited[id] = woke;
  remember(client, native, 'Archived', id, 'Failed to undo archive', async later => {
    await connected(envAccess(client, later, ref.environmentId)).dispatch({ type: 'thread.unarchive', threadId: ref.id });
  });
}
export async function renameBackground(client: T3Client, native: Native, id: string, title: string): Promise<void> {
  const ref = backgroundRef(client, id);
  if (ref) await connected(envAccess(client, native, ref.environmentId)).dispatch({ type: 'thread.metadata.update', threadId: ref.id, title });
}
/** markThreadUnread: the server's watermark where that environment tracks visits, else this device's. */
export async function markUnreadBackground(client: T3Client, native: Native, id: string): Promise<void> {
  const ref = backgroundRef(client, id), access = ref ? envAccess(client, native, ref.environmentId) : null;
  if (!ref) return;
  if (access && capabilities(access.config).visitedTracking) { await access.dispatch({ type: 'thread.mark-unread', threadId: ref.id }); return; }
  const completed = latestRun(backgroundThread(client, id) ?? {})?.completedAt;
  if (completed) sidebarPrefs(client).visited[id] = new Date(Date.parse(completed) - 1).toISOString();
}
/** The worktree question for a background thread: its environment's threads and settings decide it. */
export function backgroundWorktreePlan(client: T3Client, id: string, deleted: ReadonlySet<string>): (WorktreePlan & { environmentId: string }) | null {
  const ref = backgroundRef(client, id), entry = ref ? backgroundEntry(client, ref.environmentId) : null;
  if (!ref || !entry) return null;
  const local = new Set([...deleted].map(key => backgroundRef(client, key)?.id ?? key));
  const plan = worktreePlan(client, ref.id, local, { shell: entry.shell, config: entry.config, connected: true });
  return plan ? { ...plan, environmentId: ref.environmentId } : null;
}
/**
 * deleteThread on a background thread: the worktree question first (asked once, then resumed with its answer),
 * the provider sessions detached, its terminals closed, the delete, then the orphaned worktree's removal.
 */
export async function removeBackground(client: T3Client, native: Native, id: string, deleted: ReadonlySet<string>, answer?: boolean): Promise<'deleted' | 'prompt' | 'missing'> {
  const ref = backgroundRef(client, id), thread = backgroundThread(client, id), access = ref ? envAccess(client, native, ref.environmentId) : null;
  if (!ref || !thread || !access) return 'missing';
  const plan = backgroundWorktreePlan(client, id, deleted);
  if (plan && answer === undefined) return 'prompt';
  if (shellRuntime(thread) !== null) {
    try {
      const sessions = arr(obj(await access.request('orchestration.getThreadProjection', { threadId: ref.id })).providerSessions);
      for (const session of sessions) await access.dispatch({ type: 'provider-session.detach', threadId: ref.id, providerSessionId: str(session.id), reason: 'client-requested' });
    } catch (error) { if (letGo(error)) throw error; /* the reference awaits the detach and ignores its result */ }
  }
  try { await access.request('terminal.close', { threadId: ref.id, deleteHistory: true }); } catch (error) { if (letGo(error)) throw error; }
  await access.dispatch({ type: 'thread.delete', threadId: ref.id });
  delete client.local.drafts[`${ref.environmentId}:${ref.id}`];
  const session = sidebarSession(client);
  session.selection = session.selection.filter(selected => selected !== id);
  if (plan && answer === true) {
    let removed = false;
    try {
      await access.request('vcs.removeWorktree', { cwd: plan.cwd, path: plan.path, force: true });
      removed = true;
      await access.request('vcs.refreshStatus', { cwd: plan.cwd });
    } catch (error) {
      if (letGo(error)) throw error;
      const message = error instanceof Error && error.message ? error.message : 'An error occurred.';
      pushToast(client, { kind: 'error', stacked: true, title: removed ? 'Worktree deleted, but Git status refresh failed' : 'Failed to delete worktree',
        description: removed ? message : `Could not remove ${formatWorktreePathForDisplay(plan.path)}. ${message}` });
    }
  }
  return 'deleted';
}
/** A background thread's Copy Path: its worktree, else its project's root on that environment. */
export function backgroundThreadPath(client: T3Client, id: string): string {
  const ref = backgroundRef(client, id), entry = ref ? backgroundEntry(client, ref.environmentId) : null, thread = backgroundThread(client, id);
  if (!thread) return '';
  return str(thread.worktreePath) || str(entry?.shell.projects.find(project => str(project.id) === str(thread.projectId))?.workspaceRoot);
}
