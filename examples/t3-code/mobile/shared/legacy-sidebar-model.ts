// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/legacy-sidebar-model.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The "Sidebar (legacy)" model (T3 Code 1e2ecbd975; MIT, see LICENSE-T3):
// packages/client-runtime/src/state/threadSort.ts (sortThreads,
// getThreadSortTimestamp, getLatestThreadForProject), apps/web/src/components/
// Sidebar.logic.ts (orderItemsByPreferredIds, sortProjectsForSidebar,
// resolveThreadStatusPill, resolveProjectStatusIndicator,
// buildMultiSelectThreadContextMenuItems, isTrailingDoubleClick,
// resolveAdjacentThreadId), apps/web/src/uiStateStore.ts (resolveProjectExpanded,
// reorderProjects), apps/web/src/timestampFormat.ts (formatRelativeTimeLabel) and
// LegacySidebar.tsx's own helpers. Pure functions over the raw V2 thread shell.
import { str, type Obj } from './domain';
import { latestRun, pendingApproval, pendingInput, shellRuntime, backgroundHolds, unseenCompletion } from './sidebar-model';
import type { MenuItem } from './sidebar-menu';

export type ThreadSortOrder = 'updated_at' | 'created_at';
export type ProjectSortOrder = ThreadSortOrder | 'manual';
export const MIN_SIDEBAR_THREAD_PREVIEW_COUNT = 1;
export const MAX_SIDEBAR_THREAD_PREVIEW_COUNT = 15;
export const DEFAULT_SIDEBAR_THREAD_PREVIEW_COUNT = 6;
export const SIDEBAR_SORT_LABELS: [ProjectSortOrder, string][] = [['updated_at', 'Last user message'], ['created_at', 'Created at'], ['manual', 'Manual']];
export const SIDEBAR_THREAD_SORT_LABELS: [ThreadSortOrder, string][] = [['updated_at', 'Last user message'], ['created_at', 'Created at']];

export function toSortableTimestamp(iso: unknown): number | null {
  if (typeof iso !== 'string' || !iso) return null;
  const ms = Date.parse(iso);
  return Number.isFinite(ms) ? ms : null;
}
function firstSortable(...values: unknown[]): number | null {
  for (const value of values) { const time = toSortableTimestamp(value); if (time !== null) return time; }
  return null;
}
/** getLatestUserMessageTimestamp: the shell's stamp, else the newest user message, else update, else creation. */
function latestUserMessageTimestamp(thread: Obj): number {
  const stamp = toSortableTimestamp(thread.latestUserMessageAt);
  if (stamp !== null) return stamp;
  let latest: number | null = null;
  for (const message of Array.isArray(thread.messages) ? thread.messages as Obj[] : []) {
    if (message.role !== 'user') continue;
    const time = toSortableTimestamp(message.createdAt);
    if (time !== null) latest = latest === null ? time : Math.max(latest, time);
  }
  return latest ?? firstSortable(thread.updatedAt, thread.createdAt) ?? Number.NEGATIVE_INFINITY;
}
export function getThreadSortTimestamp(thread: Obj, order: ThreadSortOrder): number {
  if (order === 'created_at') return firstSortable(thread.createdAt, thread.updatedAt) ?? Number.NEGATIVE_INFINITY;
  return latestUserMessageTimestamp(thread);
}
/** sortThreads: newest first by the order's stamp, ties by id descending. */
export function sortThreads<T extends Obj>(threads: readonly T[], order: ThreadSortOrder): T[] {
  if (threads.length < 2) return [...threads];
  return threads.map(thread => ({ thread, time: getThreadSortTimestamp(thread, order) }))
    .sort((left, right) => right.time - left.time || (str(left.thread.id) < str(right.thread.id) ? 1 : str(left.thread.id) > str(right.thread.id) ? -1 : 0))
    .map(entry => entry.thread);
}
export function getLatestThreadForProject<T extends Obj>(threads: readonly T[], projectId: string, order: ThreadSortOrder): T | null {
  let latest: T | null = null, latestTime = Number.NEGATIVE_INFINITY;
  for (const thread of threads) {
    if (thread.projectId !== projectId || thread.archivedAt != null) continue;
    const time = getThreadSortTimestamp(thread, order);
    if (latest === null || time > latestTime || (time === latestTime && str(thread.id) > str(latest.id))) { latest = thread; latestTime = time; }
  }
  return latest;
}

/** orderItemsByPreferredIds: preferred ids first (each item once), then the rest in source order. */
export function orderItemsByPreferredIds<T, Id>(input: { items: readonly T[]; preferredIds: readonly Id[]; getId: (item: T) => Id; getPreferenceIds?: (item: T) => readonly Id[] }): T[] {
  const { items, preferredIds, getId, getPreferenceIds } = input;
  if (preferredIds.length === 0) return [...items];
  const indexes = new Map<Id, number[]>();
  items.forEach((item, index) => {
    for (const id of new Set(getPreferenceIds?.(item) ?? [getId(item)])) indexes.set(id, [...(indexes.get(id) ?? []), index]);
  });
  const emitted = new Set<number>();
  const ordered = preferredIds.flatMap(id => {
    const index = indexes.get(id)?.find(candidate => !emitted.has(candidate));
    if (index === undefined) return [];
    emitted.add(index);
    return [items[index]!];
  });
  return [...ordered, ...items.filter((_, index) => !emitted.has(index))];
}

export interface SortableProject { id: string; title: string; createdAt?: unknown; updatedAt?: unknown }
/** getProjectSortTimestamp: the newest thread stamp, else the project's own. */
export function getProjectSortTimestamp(project: SortableProject, threads: readonly Obj[], order: ThreadSortOrder): number {
  if (threads.length) return threads.reduce((latest, thread) => Math.max(latest, getThreadSortTimestamp(thread, order)), Number.NEGATIVE_INFINITY);
  if (order === 'created_at') return toSortableTimestamp(project.createdAt) ?? Number.NEGATIVE_INFINITY;
  return toSortableTimestamp(project.updatedAt ?? project.createdAt) ?? Number.NEGATIVE_INFINITY;
}
/** sortProjectsForSidebar: Manual keeps the given order; otherwise newest activity, then title, then id. */
export function sortProjectsForSidebar<P extends SortableProject>(projects: readonly P[], threads: readonly Obj[], order: ProjectSortOrder): P[] {
  if (order === 'manual') return [...projects];
  const byProject = new Map<string, Obj[]>();
  for (const thread of threads) byProject.set(str(thread.projectId), [...(byProject.get(str(thread.projectId)) ?? []), thread]);
  return projects.map(project => ({ project, time: getProjectSortTimestamp(project, byProject.get(project.id) ?? [], order) }))
    .sort((left, right) => (right.time === left.time ? 0 : right.time > left.time ? 1 : -1)
      || left.project.title.localeCompare(right.project.title) || left.project.id.localeCompare(right.project.id))
    .map(entry => entry.project);
}

export type StatusLabel = 'Working' | 'Connecting' | 'Completed' | 'Pending Approval' | 'Awaiting Input' | 'Waiting' | 'Plan Ready';
/** ThreadStatusPill with the Tailwind classes as the clone's light-dark colours (text, dot). */
export interface StatusPill { label: StatusLabel; color: string; dot: string; pulse: boolean }
const PRIORITY: Record<StatusLabel, number> = { 'Pending Approval': 5, 'Awaiting Input': 4, Working: 3, Connecting: 3, Waiting: 2.5, 'Plan Ready': 2, Completed: 1 };
const pill = (label: StatusLabel, color: string, dot: string, pulse = false): StatusPill => ({ label, color, dot, pulse });
const SKY = pill('Working', 'light-dark(#0084d1, #74d4ffcc)', 'light-dark(#00a6f4, #74d4ffcc)', true);
/** session-logic.ts isLatestRunSettled: the latest run ended and is not the runtime's active run. */
function latestRunSettled(thread: Obj): boolean {
  const run = latestRun(thread), runtime = shellRuntime(thread);
  return run !== null && !['preparing', 'queued', 'starting', 'running', 'waiting'].includes(run.status) && runtime?.activeRunId !== str(thread.latestRunId);
}
/** resolveThreadStatusPill: approval, input, work, connecting, background wait, plan ready, then an unseen completion. */
export function resolveThreadStatusPill(thread: Obj, lastVisitedAt: string | undefined): StatusPill | null {
  if (pendingApproval(thread)) return pill('Pending Approval', 'light-dark(#e17100, #ffd230e6)', 'light-dark(#fe9a00, #ffd230e6)');
  if (pendingInput(thread)) return pill('Awaiting Input', 'light-dark(#4f39f6, #a3b3ffe6)', 'light-dark(#615fff, #a3b3ffe6)');
  const status = shellRuntime(thread)?.status ?? '';
  if (status === 'running' || status === 'waiting') return SKY;
  if (status === 'preparing' || status === 'starting' || status === 'queued') return { ...SKY, label: 'Connecting' };
  if (backgroundHolds(thread)) return pill('Waiting', 'light-dark(#71717b, #818181)', 'light-dark(#71717b, #818181)');
  if (thread.interactionMode === 'plan' && latestRunSettled(thread) && thread.hasActionableProposedPlan === true)
    return pill('Plan Ready', 'light-dark(#7f22fe, #c4b4ffe6)', 'light-dark(#8e51ff, #c4b4ffe6)');
  if (unseenCompletion(thread, lastVisitedAt)) return pill('Completed', 'light-dark(#009966, #5ee9b5e6)', 'light-dark(#00bc7d, #5ee9b5e6)');
  return null;
}
/** resolveProjectStatusIndicator: the highest-priority pill; the first of equals wins. */
export function resolveProjectStatusIndicator(statuses: readonly (StatusPill | null)[]): StatusPill | null {
  let best: StatusPill | null = null;
  for (const status of statuses) if (status && (!best || PRIORITY[status.label] > PRIORITY[best.label])) best = status;
  return best;
}

export function buildMultiSelectThreadContextMenuItems(input: { count: number; hasRunningThread: boolean }): MenuItem[] {
  return [
    { id: 'mark-unread', label: `Mark unread (${input.count})` },
    { id: 'archive', label: `Archive (${input.count})`, disabled: input.hasRunningThread },
    { id: 'delete', label: `Delete (${input.count})`, destructive: true },
  ];
}
/** The legacy thread menu (LegacySidebar handleThreadContextMenu): no pin, settle or snooze rows. */
export function legacyThreadMenuItems(branch: string): MenuItem[] {
  return [
    ...(branch ? [{ id: 'new-thread-on-branch', label: `New thread on ${branch}` }] : []),
    { id: 'rename', label: 'Rename thread' },
    { id: 'mark-unread', label: 'Mark unread' },
    { id: 'copy-path', label: 'Copy Path' },
    { id: 'copy-thread-id', label: 'Copy Thread ID' },
    { id: 'project-settings', label: 'Project settings' },
    { id: 'delete', label: 'Delete', destructive: true },
  ];
}
export interface LegacyMember { id: string; physicalKey: string; title: string; workspaceRoot: string; environmentLabel: string; remote: boolean }
/** formatProjectMemberActionLabel: a single project reads its title, a group's members their place. */
export function memberActionLabel(member: LegacyMember, groupedCount: number): string {
  if (groupedCount <= 1) return member.title;
  return member.environmentLabel ? `${member.environmentLabel} — ${member.workspaceRoot}` : member.workspaceRoot;
}
/**
 * The project header's menu (handleProjectButtonContextMenu): Rename, Group into..., Copy Path,
 * Project settings, Remove; with several members each targeted row is a submenu of members.
 * A background environment's member is listed, but only its path can be copied from here.
 */
export function legacyProjectMenuItems(members: readonly LegacyMember[]): MenuItem[] {
  const targeted = (action: string, label: string, destructive = false): MenuItem => {
    const leaf = (member: LegacyMember): MenuItem => ({ id: `${action}:${member.physicalKey}`, label: memberActionLabel(member, members.length),
      ...(destructive ? { destructive: true } : {}), ...(member.remote && action !== 'copy-path' ? { disabled: true } : {}) });
    if (members.length === 1) return { ...leaf(members[0]!), label };
    return { id: `${action}:submenu`, label, children: members.map(leaf) };
  };
  return [targeted('rename', 'Rename'), targeted('grouping', 'Group into...'), targeted('copy-path', 'Copy Path'),
    { id: 'project-settings', label: 'Project settings' }, targeted('delete', 'Remove', true)];
}

/** isTrailingDoubleClick: the second click of a double-click never navigates. */
export const isTrailingDoubleClick = (detail: number): boolean => detail > 1;
export function clampSidebarThreadPreviewCount(value: number): number {
  return Math.min(MAX_SIDEBAR_THREAD_PREVIEW_COUNT, Math.max(MIN_SIDEBAR_THREAD_PREVIEW_COUNT, Math.round(value)));
}
export function resolveAdjacentThreadId<T>(input: { threadIds: readonly T[]; currentThreadId: T | null; direction: 'previous' | 'next' }): T | null {
  const { threadIds, currentThreadId, direction } = input;
  if (!threadIds.length) return null;
  if (currentThreadId === null) return direction === 'previous' ? threadIds[threadIds.length - 1] ?? null : threadIds[0] ?? null;
  const at = threadIds.indexOf(currentThreadId);
  if (at === -1) return null;
  return direction === 'previous' ? (at > 0 ? threadIds[at - 1] ?? null : null) : (at < threadIds.length - 1 ? threadIds[at + 1] ?? null : null);
}
/** resolveProjectExpanded: the first stored key wins; a project is expanded by default. */
export function resolveProjectExpanded(expandedById: Readonly<Record<string, boolean>>, keys: readonly string[]): boolean {
  for (const key of keys) { const value = expandedById[key]; if (value !== undefined) return value; }
  return true;
}
/** uiStateStore reorderProjects: the dragged ids move to the target's original index. */
export function reorderProjects(order: readonly string[], dragged: readonly string[], target: readonly string[]): string[] {
  if (!dragged.length) return [...order];
  const draggedSet = new Set(dragged), targetSet = new Set(target);
  if (dragged.every(id => targetSet.has(id))) return [...order];
  const targetIndex = order.findIndex(id => targetSet.has(id));
  if (targetIndex < 0) return [...order];
  const next = [...order], removed: string[] = [];
  let before = 0;
  for (let index = next.length - 1; index >= 0; index--) {
    if (draggedSet.has(next[index]!)) { removed.unshift(next.splice(index, 1)[0]!); if (index < targetIndex) before++; }
  }
  if (!removed.length) return [...order];
  next.splice(targetIndex - Math.max(0, before - 1), 0, ...removed);
  return next;
}
/** formatRelativeTimeLabel: "just now", then "5m ago", "3h ago", "2d ago". */
export function relativeTimeLabel(value: unknown, now: number): string {
  const time = toSortableTimestamp(value);
  if (time === null || !Number.isFinite(now)) return '';
  const seconds = Math.floor((now - time) / 1000);
  if (now - time < 0 || seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h ago` : `${Math.floor(hours / 24)}d ago`;
}
