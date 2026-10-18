// The index route's landing over every environment (T3 Code 1e2ecbd975, MIT, see LICENSE-T3):
// apps/web/src/routes/_chat.index.tsx IndexDraftLanding waits until every enabled environment's
// shell has bootstrapped (state/shell.ts allEnvironmentShellsBootstrappedAtom), then opens a draft in
// the first project of sortScopedProjectsForSidebar(projects, threads, "updated_at")
// (components/Sidebar.logic.ts), whichever environment holds it. The focused environment is
// T3Client's connection; every other one is a fleet entry (settings-b-fleet.ts). A draft in a
// background environment's project focuses that environment, as a clone toast's Open project does.
import { str, type Obj, type Shell } from '../client/domain';
import type { Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { threadActivity } from './pages-home';
import { fleet, focusFleetProject, type EnvironmentFleet } from '../connections/settings-b-fleet';

export type HomeTarget = { environmentId: string; projectId: string };
type Focused = Pick<T3Client, 'environmentId' | 'shell'>;
type Landing = Focused & Pick<T3Client, 'threadId'> & { homePending: boolean; homeMove: HomeTarget | null };

const timestamp = (value: unknown): number | null => {
  const parsed = typeof value === 'string' ? Date.parse(value) : NaN;
  return Number.isFinite(parsed) ? parsed : null;
};
const synchronized = (source: EnvironmentFleet, focusedId: string) => [...source.entries.values()]
  .filter(entry => entry.environmentId && entry.environmentId !== focusedId && entry.phase === 'connected' && entry.synchronized === entry.generation);

/** sortScopedProjectsForSidebar(…, "updated_at") over the focused shell and each synchronized background one. */
export function scopedProjectsByActivity(client: Focused, source: EnvironmentFleet = fleet, shell: Shell = client.shell): HomeTarget[] {
  const scoped: { environmentId: string; project: Obj; threads: Obj[] }[] = [];
  const add = (environmentId: string, value: Pick<Shell, 'projects' | 'threads'>) => {
    for (const project of value.projects) scoped.push({ environmentId, project, threads: value.threads.filter(thread => !thread.archivedAt && thread.projectId === project.id) });
  };
  add(client.environmentId, shell);
  for (const entry of synchronized(source, client.environmentId)) add(entry.environmentId, entry.shell);
  return scoped
    .map(entry => ({ ...entry, at: entry.threads.length ? Math.max(...entry.threads.map(threadActivity))
      : timestamp(entry.project.updatedAt) ?? timestamp(entry.project.createdAt) ?? Number.NEGATIVE_INFINITY }))
    .sort((left, right) => (right.at === left.at ? 0 : right.at > left.at ? 1 : -1)
      || str(left.project.title).localeCompare(str(right.project.title))
      || left.environmentId.localeCompare(right.environmentId)
      || str(left.project.id).localeCompare(str(right.project.id)))
    .map(entry => ({ environmentId: entry.environmentId, projectId: str(entry.project.id) }));
}

/** The most recently active project of every environment, or null when none holds a project. */
export function homeTarget(client: Focused, source: EnvironmentFleet = fleet, shell: Shell = client.shell): HomeTarget | null {
  return scopedProjectsByActivity(client, source, shell)[0] ?? null;
}

/**
 * allEnvironmentShellsBootstrappedAtom for the background environments: each has its snapshot or
 * is disconnected. One still connecting, or connected without its snapshot yet, keeps the landing waiting.
 */
export function backgroundShellsBootstrapped(client: Pick<T3Client, 'environmentId'>, source: EnvironmentFleet = fleet): boolean {
  return [...source.entries.values()].every(entry => entry.environmentId === client.environmentId
    || (entry.phase === 'connected' ? entry.synchronized === entry.generation : entry.phase !== 'connecting' && !(entry.phase === 'available' && !entry.requested)));
}

/**
 * The focused project the landing picks, or '' while it waits or when the pick is another
 * environment's (kept in `homeMove` until settleHome focuses it). After the launch's landing,
 * a project that went away falls back to the focused environment's most recent one.
 */
export function landHome(client: Landing, fallback: () => string, source: EnvironmentFleet = fleet): string {
  // The shell events drained after the synchronize that picked (its completion marker) come before settleHome:
  // falling back here would land the focused environment's project and drop the move.
  if (client.homeMove) return '';
  if (!client.homePending) return fallback();
  if (client.threadId) { client.homePending = false; return fallback(); }
  if (!backgroundShellsBootstrapped(client, source)) return '';
  client.homePending = false;
  const target = homeTarget(client, source);
  if (!target) return '';
  if (target.environmentId === client.environmentId) return target.projectId;
  client.homeMove = target;
  return '';
}

/** Whether the landing still waits: the reference renders nothing until it has picked. */
export function homeWaiting(client: Pick<T3Client, 'threadId' | 'projectId'> & Partial<{ homePending: boolean; homeMove: HomeTarget | null }>): boolean {
  return !client.threadId && !client.projectId && (client.homePending === true || !!client.homeMove);
}

/**
 * Each refresh: once the fleet has bootstrapped, the launch's landing picks across every
 * environment; a pick in a background environment focuses it on that project's draft.
 */
export async function settleHome(client: T3Client, native: Native, source: EnvironmentFleet = fleet): Promise<boolean> {
  if (client.homePending && client.ready && !client.projectId) {
    const projectId = landHome(client, () => '', source);
    if (projectId) { client.projectId = projectId; client.chooseDefaults(); return true; }
  }
  const move = client.homeMove;
  if (!move) return false;
  client.homeMove = null;
  if (client.threadId || client.projectId) return false;
  const focused = await focusFleetProject(client, native, move.environmentId, move.projectId, source);
  client.adoptStatus(focused.value, focused.generation);
  return true;
}

/**
 * A navigation to the index route mid-session (no thread left after a delete, a `#fragment`
 * link, the start error's Try again): the draft of the most recent project of every environment.
 */
export async function openHome(client: T3Client, native: Native, shell: Shell = client.shell, source: EnvironmentFleet = fleet): Promise<void> {
  const target = homeTarget(client, source, shell);
  if (target && target.environmentId !== client.environmentId) {
    const focused = await focusFleetProject(client, native, target.environmentId, target.projectId, source);
    client.adoptStatus(focused.value, focused.generation);
    return;
  }
  // Partial: the sidebar's tests drive these commands with a client that only holds state.
  await (client as Partial<Pick<T3Client, 'openDraft'>>).openDraft?.(native, target?.projectId ?? '');
}
