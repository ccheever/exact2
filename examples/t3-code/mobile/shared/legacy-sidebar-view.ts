// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/legacy-sidebar-view.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The "Sidebar (legacy)" projection (LegacySidebar.tsx, T3 Code 1e2ecbd975; MIT,
// see LICENSE-T3): projects grouped across this environment and the fleet's
// background environments (sidebarProjectGrouping.ts), sorted by Sort projects
// (Manual keeps the persisted project order), each with its threads sorted by
// Sort threads, the first `sidebarThreadPreviewCount` shown until Show more,
// and the open thread kept under a collapsed project. The visible order is
// what ⌘1-9 and previous/next follow. Pure over the client and `now`.
import { obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import { terminalFocused } from './terminal-focus';
import { fleet, fleetThreads, type EnvironmentFleet } from './settings-b-fleet';
import { groupLabel, logicalKey } from './r6-polish-groups';
import { projectGlyph } from './r3-sidebar-glyph';
import type { ProjectGlyph } from './r3-sidebar-glyph';
import { canArchive } from './sidebar-menu';
import { lastVisited } from './sidebar-model';
import { sidebarPrefs, sidebarSession } from './sidebar-state';
import { terminalProcessCount, shortcutLabel, type SidebarHelpers } from './sidebar-view';
import { currentPullRequestLink } from './shell-pr';
import { lifecycle, lifecycleIcon, shortName, providerOfUrl } from './r5-panels-pr';
import { DEFAULT_SIDEBAR_THREAD_PREVIEW_COUNT, orderItemsByPreferredIds, relativeTimeLabel, resolveProjectExpanded, resolveProjectStatusIndicator,
  resolveAdjacentThreadId, resolveThreadStatusPill, sortProjectsForSidebar, sortThreads, type LegacyMember, type ProjectSortOrder, type StatusPill, type ThreadSortOrder } from './legacy-sidebar-model';

export interface LegacyThreadRow {
  id: string; title: string; label: string; active: boolean; selected: boolean; renaming: boolean; running: boolean;
  terminalCount: number;
  statusLabel: string; statusColor: string; statusDot: string; statusPulse: boolean;
  prIcon: string; prState: string; prTooltip: string; prPending: boolean;
  worktree: boolean; worktreeLabel: string; remoteMachine: string; remoteLabel: string; age: string; jump: string;
}
export interface LegacyProjectRow {
  key: string; name: string; countLabel: string; glyph: ProjectGlyph; expanded: boolean;
  statusLabel: string; statusColor: string; statusDot: string; statusPulse: boolean;
  remoteMachine: string; remoteLabel: string; remoteTooltip: string;
  threads: LegacyThreadRow[]; showEmpty: boolean; showMore: boolean; showLess: boolean;
  hiddenLabel: string; hiddenDot: string; hiddenColor: string; hiddenPulse: boolean; newThreadLabel: string;
}
export interface LegacySidebarView {
  enabled: boolean; projects: LegacyProjectRow[]; manual: boolean; projectSort: string; threadSort: string; previewCount: number;
  newThreadTip: string; searchShortcut: string; confirmArchive: boolean; noProjects: boolean; jumpHints: boolean;
  dialog: string; dialogTitle: string; dialogDescription: string; dialogField: string; dialogDetail: string; dialogGrouping: string; dialogGroupingLabel: string;
  dialogGroupingDescription: string; dialogConfirmLabel: string; dialogTarget: string;
}

// ── Session state (React state in the reference): Show more per project and the open dialog.
export interface LegacyDialog { kind: '' | 'rename-project' | 'grouping' | 'remove-project' | 'archive-many'; memberId: string; title: string; root: string; environment: string; grouping: string; threads: number; ids: string[] }
export interface LegacySession { expandedLists: Set<string>; dialog: LegacyDialog }
const sessions = new WeakMap<T3Client, LegacySession>();
export const emptyDialog = (): LegacyDialog => ({ kind: '', memberId: '', title: '', root: '', environment: '', grouping: 'inherit', threads: 0, ids: [] });
export function legacySession(client: T3Client): LegacySession {
  let session = sessions.get(client);
  if (!session) { session = { expandedLists: new Set(), dialog: emptyDialog() }; sessions.set(client, session); }
  return session;
}

export const legacyEnabled = (client: T3Client): boolean => client.local.clientSettings?.legacySidebarEnabled === true;
const settings = (client: T3Client) => (client.local.clientSettings ?? {}) as unknown as Record<string, unknown>;
export function threadSortOrder(client: T3Client): ThreadSortOrder { return settings(client).sidebarThreadSortOrder === 'created_at' ? 'created_at' : 'updated_at'; }
export function projectSortOrder(client: T3Client): ProjectSortOrder {
  const value = settings(client).sidebarProjectSortOrder;
  return value === 'created_at' || value === 'manual' ? value : 'updated_at';
}
export function previewCount(client: T3Client): number {
  const value = Number(settings(client).sidebarThreadPreviewCount);
  return Number.isInteger(value) && value >= 1 && value <= 15 ? value : DEFAULT_SIDEBAR_THREAD_PREVIEW_COUNT;
}

const projectPath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
export interface LegacyGroup { key: string; name: string; members: (LegacyMember & { project: Obj })[]; createdAt: unknown; updatedAt: unknown; remoteOnly: boolean; remoteLabels: string[] }
/**
 * buildSidebarProjectSnapshots over orderItemsByPreferredIds(projects, projectOrder): this environment's projects,
 * then each synchronized background environment's (ids `fleet:<environment>:<id>`, as fleetThreads names them).
 */
export function legacyGroups(client: T3Client, source: EnvironmentFleet = fleet): LegacyGroup[] {
  const members: (LegacyMember & { project: Obj; key: string })[] = client.shell.projects.map(project => ({
    id: str(project.id), physicalKey: `${client.environmentId}:${projectPath(project.workspaceRoot)}`, title: str(project.title), workspaceRoot: str(project.workspaceRoot),
    environmentLabel: '', remote: false, project, key: logicalKey(client, client.environmentId, project) }));
  for (const entry of source.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation || entry.environmentId === client.environmentId) continue;
    const label = str(obj(entry.config.environment).label);
    for (const project of entry.shell.projects) members.push({ id: `fleet:${entry.environmentId}:${str(project.id)}`,
      physicalKey: `${entry.environmentId}:${projectPath(project.workspaceRoot)}`, title: str(project.title), workspaceRoot: str(project.workspaceRoot),
      environmentLabel: label, remote: true, project, key: logicalKey(client, entry.environmentId, project) });
  }
  const ordered = orderItemsByPreferredIds({ items: members, preferredIds: sidebarPrefs(client).projectOrder, getId: member => member.physicalKey });
  const groups = new Map<string, LegacyGroup>();
  for (const member of ordered) {
    const group = groups.get(member.key) ?? { key: member.key, name: '', members: [], createdAt: member.project.createdAt, updatedAt: member.project.updatedAt ?? member.project.createdAt, remoteOnly: true, remoteLabels: [] };
    group.members.push(member);
    group.remoteOnly = group.remoteOnly && member.remote;
    if (member.remote && member.environmentLabel && !group.remoteLabels.includes(member.environmentLabel)) group.remoteLabels.push(member.environmentLabel);
    groups.set(member.key, group);
  }
  return [...groups.values()].map(group => ({ ...group, name: groupLabel(group.members.map(member => member.project)) || group.members[0]!.title }));
}
/** Every unarchived thread of this environment and the fleet, by its group's key. */
function threadsByGroup(client: T3Client, groups: LegacyGroup[]): Map<string, Obj[]> {
  const groupOf = new Map<string, string>();
  for (const group of groups) for (const member of group.members) groupOf.set(member.id, group.key);
  const out = new Map<string, Obj[]>();
  for (const thread of [...client.shell.threads, ...fleetThreads()]) {
    if (thread.archivedAt != null || thread.deletedAt != null) continue;
    const key = groupOf.get(str(thread.projectId));
    if (key) out.set(key, [...(out.get(key) ?? []), thread]);
  }
  return out;
}
/** Groups in Sort projects order with their sorted threads. */
export function sortedGroups(client: T3Client): { group: LegacyGroup; threads: Obj[] }[] {
  const groups = legacyGroups(client), byGroup = threadsByGroup(client, groups), order = threadSortOrder(client);
  const sortable = groups.map(group => ({ id: group.key, title: group.name, createdAt: group.createdAt, updatedAt: group.updatedAt, group }));
  const threads = groups.flatMap(group => (byGroup.get(group.key) ?? []).map(thread => ({ ...thread, projectId: group.key })));
  return sortProjectsForSidebar(sortable, threads, projectSortOrder(client)).map(entry => ({ group: entry.group, threads: sortThreads(byGroup.get(entry.group.key) ?? [], order) }));
}
export const expansionKeys = (group: LegacyGroup): string[] => [group.key, ...group.members.map(member => member.physicalKey)];
const statusOf = (client: T3Client, thread: Obj) => resolveThreadStatusPill(thread, lastVisited(thread, sidebarPrefs(client).visited[str(thread.id)]));

/** One project's rendered threads (pinnedCollapsedThread, the preview slice) and what it hides. */
export function projectThreads(client: T3Client, group: LegacyGroup, threads: Obj[]) {
  const expanded = resolveProjectExpanded(sidebarPrefs(client).projectExpanded, expansionKeys(group));
  const pinned = !expanded && client.threadId ? threads.find(thread => thread.id === client.threadId) ?? null : null;
  const listExpanded = legacySession(client).expandedLists.has(group.key), count = previewCount(client);
  const overflowing = threads.length > count;
  const preview = listExpanded || !overflowing ? threads : threads.slice(0, count);
  const visible = new Set([...preview, ...(pinned ? [pinned] : [])].map(thread => str(thread.id)));
  const rendered = pinned ? [pinned] : threads.filter(thread => visible.has(str(thread.id)));
  const hidden = threads.filter(thread => !visible.has(str(thread.id)));
  return { expanded, listExpanded, overflowing, rendered: expanded || pinned ? rendered : [], hidden };
}
/** The sidebar's visible thread order (visibleSidebarThreadKeys): what jump and traversal follow. */
export function legacyVisibleThreadIds(client: T3Client): string[] {
  return sortedGroups(client).flatMap(({ group, threads }) => projectThreads(client, group, threads).rendered.map(thread => str(thread.id)));
}
/** A thread's project list as the shift-click range reads it (orderedProjectThreadKeys). */
export function legacyProjectOrder(client: T3Client, threadId: string): string[] {
  return sortedGroups(client).find(entry => entry.threads.some(thread => thread.id === threadId))?.threads.map(thread => str(thread.id)) ?? [];
}

const blank = { label: '', color: '', dot: '', pulse: false };
const pillFields = (status: StatusPill | null) => status ?? blank;
function threadRow(client: T3Client, thread: Obj, now: number, jump: string): LegacyThreadRow {
  const id = str(thread.id), title = str(thread.title, 'Untitled thread'), session = sidebarSession(client);
  const status = pillFields(statusOf(client, thread));
  const link = currentPullRequestLink(thread.pullRequests), snapshot = link && link.snapshot && typeof link.snapshot === 'object' ? obj(link.snapshot) : null;
  const state = snapshot ? lifecycle(str(snapshot.state, 'open'), snapshot.isDraft === true) : '';
  const stateLabel = ({ open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' } as Record<string, string>)[state] ?? '';
  const worktree = str(thread.worktreePath).trim(), branch = str(thread.branch);
  const remote = str(thread.fleetMachine);
  return {
    id, title, label: [title, status.label].filter(Boolean).join(', '), active: id === client.threadId, selected: session.selection.includes(id),
    terminalCount: terminalProcessCount(client, id),
    renaming: session.renameId === id, running: !canArchive(thread),
    statusLabel: status.label, statusColor: status.color, statusDot: status.dot, statusPulse: status.pulse,
    prIcon: link ? (snapshot ? lifecycleIcon(state) : 'git-pull-request-arrow') : '', prState: state,
    prTooltip: link ? (snapshot ? `${shortName(providerOfUrl(str(link.url)))} #${Number(link.number) || 0} - ${stateLabel}: ${str(snapshot.title)}` : `PR #${Number(link.number) || 0}, status pending`) : '',
    prPending: !!link && !snapshot,
    worktree: worktree !== '', worktreeLabel: worktree ? (branch ? `Worktree: ${worktree} (${branch})` : `Worktree: ${worktree}`) : '',
    remoteMachine: remote ? (remote === 'desktop' ? 'monitor' : remote) : '', remoteLabel: remote ? str(thread.fleetEnvironmentLabel, 'Remote') : '',
    age: relativeTimeLabel(thread.latestUserMessageAt ?? thread.updatedAt ?? thread.createdAt, now), jump,
  };
}

const GROUPING_LABELS: Record<string, string> = { repository: 'Group by repository', repository_path: 'Group by repository path', separate: 'Keep separate' };
const GROUPING_DESCRIPTIONS: Record<string, string> = {
  repository: 'Projects from the same repository share one sidebar row.',
  repository_path: 'Projects group only when both the repository and repo-relative path match.',
  separate: 'Every project path gets its own sidebar row.',
};
/** The dialogs' copy (Rename project, Project grouping, the project removal confirm). */
function dialogView(client: T3Client) {
  const dialog = legacySession(client).dialog, mode = client.local.groupingMode || 'repository';
  const effective = dialog.grouping === 'inherit' ? mode : dialog.grouping;
  const environment = dialog.environment ? [`Environment: ${dialog.environment}`] : [];
  const removal = dialog.threads > 0
    ? [`Path: ${dialog.root}`, ...environment, 'This permanently clears conversation history for those threads and any archived threads.', 'This removes only this project entry.', 'This action cannot be undone.']
    : [`Path: ${dialog.root}`, ...environment, 'This permanently clears any archived conversation history.', 'This removes only this project entry.'];
  return {
    dialog: dialog.kind, dialogTarget: dialog.memberId,
    dialogTitle: dialog.kind === 'rename-project' ? 'Rename project' : dialog.kind === 'grouping' ? 'Project grouping'
      : dialog.kind === 'archive-many' ? `Archive ${dialog.ids.length} thread${dialog.ids.length === 1 ? '' : 's'}?`
      : dialog.kind === 'remove-project' ? (dialog.threads > 0 ? `Remove project "${dialog.title}" and delete its ${dialog.threads} thread${dialog.threads === 1 ? '' : 's'}?` : `Remove project "${dialog.title}"?`) : '',
    dialogDescription: dialog.kind === 'rename-project' ? `Update the title for ${dialog.root}.` : dialog.kind === 'grouping' ? `Choose how ${dialog.root} should be grouped in the sidebar.`
      : dialog.kind === 'remove-project' ? removal.join('\n') : '',
    dialogField: dialog.kind === 'rename-project' ? dialog.title : '', dialogDetail: dialog.kind === 'rename-project' && dialog.environment ? `Environment: ${dialog.environment}` : '',
    dialogGrouping: dialog.grouping,
    dialogGroupingLabel: dialog.grouping === 'inherit' ? `Use global default (${GROUPING_LABELS[mode] ?? mode})` : GROUPING_LABELS[dialog.grouping] ?? dialog.grouping,
    dialogGroupingDescription: GROUPING_DESCRIPTIONS[effective] ?? '', dialogConfirmLabel: dialog.kind === 'rename-project' || dialog.kind === 'grouping' ? 'Save' : 'Confirm',
  };
}

export function legacySidebarSnapshot(client: T3Client, now: number, helpers: Pick<SidebarHelpers, 'projectIdentity'>): LegacySidebarView {
  const enabled = legacyEnabled(client);
  const newThread = shortcutLabel(client, 'chat.newLocal') || shortcutLabel(client, 'chat.new');
  const base = { enabled, manual: projectSortOrder(client) === 'manual', projectSort: projectSortOrder(client), threadSort: threadSortOrder(client), previewCount: previewCount(client),
    newThreadTip: newThread ? `New thread (${newThread})` : 'New thread', searchShortcut: shortcutLabel(client, 'commandPalette.toggle'),
    confirmArchive: client.local.clientSettings?.confirmThreadArchive === true, noProjects: client.shell.projects.length === 0,
    jumpHints: !terminalFocused(client) && client.presentation.sidebarJumpHints === true, ...dialogView(client) };
  if (!enabled) return { ...base, projects: [] };
  const entries = sortedGroups(client);
  const jumps = base.jumpHints ? Array.from({ length: 9 }, (_, index) => shortcutLabel(client, `thread.jump.${index + 1}`)) : [];
  let visibleIndex = 0;
  const projects = entries.map(({ group, threads }): LegacyProjectRow => {
    const view = projectThreads(client, group, threads);
    const status = resolveProjectStatusIndicator(threads.map(thread => statusOf(client, thread)));
    const hidden = pillFields(resolveProjectStatusIndicator(view.hidden.map(thread => statusOf(client, thread))));
    const representative = group.members.find(member => !member.remote) ?? group.members[0]!;
    const glyph = projectGlyph(client, representative.project, helpers.projectIdentity);
    const shown = pillFields(view.expanded ? null : status);
    return {
      key: group.key, name: group.name, countLabel: group.members.length > 1 ? `${group.members.length} projects` : '', glyph, expanded: view.expanded,
      statusLabel: shown.label, statusColor: shown.color, statusDot: shown.dot, statusPulse: shown.pulse,
      remoteMachine: group.remoteOnly ? 'cloud' : '', remoteLabel: group.remoteOnly ? 'Remote project' : '',
      remoteTooltip: group.remoteOnly ? `Remote environment: ${group.remoteLabels.join(', ')}` : '',
      threads: view.rendered.map(thread => threadRow(client, thread, now, jumps[visibleIndex++] ?? '')),
      showEmpty: view.expanded && threads.length === 0, showMore: view.expanded && view.overflowing && !view.listExpanded, showLess: view.expanded && view.overflowing && view.listExpanded,
      hiddenLabel: hidden.label, hiddenDot: hidden.dot, hiddenColor: hidden.color, hiddenPulse: hidden.pulse,
      newThreadLabel: `Create new thread in ${group.name}`,
    };
  });
  return { ...base, projects };
}

/** LegacySidebar's window keydown: traversal without wrapping (resolveAdjacentThreadId) and jumps over the visible order. Null when the switch is off. */
export function legacyTraversal(client: T3Client): { ordered: Obj[]; previous: string; next: string } | null {
  if (!legacyEnabled(client)) return null;
  const ids = legacyVisibleThreadIds(client), current = ids.includes(client.threadId) ? client.threadId : null;
  if (client.threadId && !current) return { ordered: ids.map(id => ({ id, selected: false })), previous: '', next: '' };
  return { ordered: ids.map(id => ({ id, selected: id === client.threadId })),
    previous: resolveAdjacentThreadId({ threadIds: ids, currentThreadId: current, direction: 'previous' }) ?? '',
    next: resolveAdjacentThreadId({ threadIds: ids, currentThreadId: current, direction: 'next' }) ?? '' };
}
