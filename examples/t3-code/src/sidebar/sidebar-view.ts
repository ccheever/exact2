// Adapted from T3 Code 7c3b3b3605ef (MIT); see LICENSE-T3.
import { instanceIconUrl } from '../providers/acp-icons';
// The sidebar's projection into the snapshot (Sidebar.tsx render): the rows in
// paint order (collapsed shelves contribute only the open thread, as the
// reference renders them, so thread.jump and prev/next follow the same list)
// plus the sidebar's own view state. Pure over the client and `now`.
import { arr, obj, str, type Obj } from '../client/domain';
import { effectiveShortcut } from '../shell/shortcut-labels';
import { PLATFORM } from '../settings/desktop-shell-favicon';
import { machineKind } from '../connections/connections';
import { fleet, fleetThreads, parseFleetThreadId, fleetRunningTerminalIds, remoteThread } from '../connections/settings-b-fleet'; // settings-b: background environments' threads
import type { T3Client } from '../client/client';
import { knownSessions } from '../terminal/terminal-drawer-view';
import { selectRunningSubprocessTerminalIds } from '../terminal/terminal-session';
import { terminalFocused } from '../terminal/terminal-focus';
import { snoozePresets, type SnoozePreset } from './sidebar-presentation';
import { searchMatch, searchRows, threadSearchPending, type SearchPart } from './sidebar-search';
import { SETTLED_TAIL_INITIAL_COUNT, SETTLED_TAIL_PAGE_COUNT, clock, localDate, noteSnapshotTime, sidebarPrefs, sidebarSession, undoLive, undoSummary } from './sidebar-state';
import { NO_CALENDAR, firstWeekday, snoozeCalendar, type SnoozeCalendar } from './snooze-calendar';
import { sidebarProviderPill } from './sidebar-provider-pill';
import { grayIdentity, projectGlyph, type ProjectGlyph } from './sidebar-glyph';
import { rowTips, type SidebarRowTip } from './sidebar-tips';
import { cachedDisplayNames as displayNames, groupLabel, logicalKey } from './project-groups';
import { SIDEBAR_PROBES, textMeasure } from '../shared/laid-out-widths';
import type { Probe } from '../composer/composer-menus';
export { grayIdentity } from './sidebar-glyph';
import { ageLabel, capabilities, canSnooze, isWoke, lastVisited, recedes, sectionOf, settledTimestamp, sidebarStatus, sidebarVisible,
  snoozeWakeLabel, sortActive, sortByReturn, sortPinned, sortSettled, sortSnoozed, sortWorkingThreadsBySend, threadTimeLabel, topStatus, unseenCompletion,
  statusSince, wokeAt, workingDuration, type SidebarSection } from './sidebar-model';
import { sidebarPrBadge } from '../panel/pr-surface';
import { branchPrBadge, branchPullRequest } from './sidebar-branch-pr';
import { notePlaces, rowHoverKey } from './row-hover-key';
import { orderItemsByPreferredIds } from './legacy-sidebar-model'; // the persisted project order
import { resolveRouteReveal } from './route-reveal';
import { WORKTREE_DIALOG_TITLE, worktreeDialogDescription } from './worktree-cleanup';
import { bulkMenuState, menuRows, threadMenuItems, threadMenuState, type MenuRow } from './sidebar-menu'; // the row's context popover (exact2 #223)
import { draftMenuRows } from './draft-rows';
import { replaceContextReferences } from '../composer/composer-editor-menu'; // a draft row's chips read as their labels
import { renameHeld } from './sidebar-rename-wait';
import { rowPullRequestLinks, searchPullRequest } from './search-pull-request';
import { routeRowThreadId } from './sidebar-lineage';
import { isScratch, scratchRootOf } from '../composer/scratch-machine';
import { triggerDisplayModelName } from './model-short-name';
import { inboxReturns } from './thread-inbox';
import { autoDirection } from './title-direction';
import { textDirection } from '../shared/text-direction';
import { splitForMiddleTruncate } from '../details/middle-truncate';

type Identity = (name: string) => { projectMark: string; projectInk: string; projectSurface: string };
type Badge = (provider: Obj | undefined, providers: Obj[]) => { providerBadge: string; providerBadgeColor: string };
export interface SidebarHelpers { projectIdentity: Identity; providerBadge: Badge }

/** ThreadStatusIndicators/Sidebar.tsx runningTerminalIds: shells alone are idle; only child processes count. */
export function runningTerminalIds(client: T3Client, threadId: string): string[] {
  const ref = parseFleetThreadId(threadId) ?? { environmentId: client.environmentId, threadId };
  if (ref.environmentId !== client.environmentId) return fleetRunningTerminalIds(ref.environmentId, ref.threadId);
  return [...selectRunningSubprocessTerminalIds(knownSessions(client, ref))];
}
export const terminalProcessCount = (client: T3Client, threadId: string): number => runningTerminalIds(client, threadId).length;
export const terminalProcessLabel = (count: number): string => count ? `${count} terminal ${count === 1 ? 'process' : 'processes'} running` : '';

export interface SidebarThread {
  id: string; title: string; titleDir: string; renameDir: string; branchHead: string; branchTail: string; projectName: string; projectMark: string; projectInk: string; projectSurface: string;
  projectGray: string; projectGraySurface: string; providerDriver: string; providerBadge: string; providerBadgeColor: string; providerName: string;
  /** An ACP Registry instance's agent icon (ProviderInstanceIcon's acpRegistryIconUrl); '' for other drivers. */
  providerIcon: string;
  terminalCount: number; terminalLabel: string;
  age: string; badge: string; stacked: boolean; badgeIcon: string; badgeState: string; section: string; status: string; selected: boolean; routeThread: boolean; searchFirst: boolean;
  matchLabel: string; matchUser: boolean; matchParts: SearchPart[];
  card: boolean; metaEmpty: boolean; statusIcon: string; statusColor: string; duration: string; titleColor: string; titleWeight: number;
  recede: boolean; pinned: boolean; canPin: boolean; canSettle: boolean; canSnooze: boolean; canUnsettle: boolean; canWake: boolean;
  wakeLabel: string; woke: boolean; branch: string; worktree: boolean; draft: boolean; jump: string; renaming: boolean; multi: boolean;
  regenerating: boolean; label: string; hoverKey: string; rowKey: string; glyph: ProjectGlyph;
  idleRecede: boolean; idleTitleColor: string; idleTitleWeight: number; tips: SidebarRowTip[];
  hoverEnvironment: string; hoverMachine: string; hoverModel: string; hoverHandoff: string; hoverError: string; hoverErrorWarning: boolean;
  workingSince: number; remoteMachine: string; // the ticking Working label; a background environment's machine glyph
  scratchMachine: string; scratchLabel: string; // a "No project" thread's machine, in the branch's place
  searchPr: string; searchPrIcon: string; searchPrState: string; searchPrLabel: string; // a search result's pull request badge (its row's)
  menu: MenuRow[]; // handleThreadContextMenu's items (the bulk menu's on a selected row), for the row's context popover
}
export interface SidebarScope { key: string; label: string; mark: string; ink: string; surface: string; selected: boolean; all: boolean; glyph: ProjectGlyph }
export interface SidebarDraft { id: string; projectName: string; mark: string; ink: string; surface: string; preview: string; glyph: ProjectGlyph; menu: MenuRow[] }
export interface SidebarView {
  drafts: SidebarDraft[];
  searching: boolean; searchPending: boolean; searchIndex: number; searchSelectedId: string;
  settledCount: number; snoozedCount: number; workingCount: number; ticking: boolean; probes: Probe[];
  /** The soonest Working grace's end (window time), 0 for none: the root's timer ends it 2 s after it started. */
  graceUntil: number;
  /** Non-zero while a grace's start or a return's stamp waits for the window's clock, changing with each new one. */
  graceOwed: number;
  settledExpanded: boolean; snoozedExpanded: boolean; workingExpanded: boolean; showMore: number;
  emptyText: string; emptyAddProject: boolean; hasProjects: boolean; projectGroupCount: number; dragEnabled: boolean;
  scopeKey: string; scopeLabel: string; scopeMark: string; scopeInk: string; scopeSurface: string; scopeGlyph: ProjectGlyph; scopes: SidebarScope[];
  scopeOpen: boolean; scopeQuery: string; scopeFirst: string;
  renameId: string; renameTitle: string; renameHeld: boolean; selectionCount: number; undoText: string; undoLabel: string; undoUntil: number; undoHost: string;
  dialog: string; dialogTitle: string; dialogDescription: string; dialogMode: string; dialogDate: string; dialogTime: string; dialogAmount: string; dialogUnit: string; dialogError: string; dialogReturn: string; jumpModifiers: string[];
  calendar: SnoozeCalendar;
  pillKey: string; pillTone: string; pillTitle: string; pillDescription: string; pillDismissible: boolean; sweepEpoch: number;
  /** A timed provider pill's end (window time; its countdown bar runs to it), 0 for one that stays. */
  pillUntil: number;
  /** The open row, while it is listed (each mount of the view scrolls to it), and again while a route change owes it. */
  revealRow: string; revealPending: string;
}

const FOREGROUND = 'light-dark(#18181b, #f3f3f3)';
/** --secondary-label: row metadata, a step below read titles (#8f8f8f on the dark sidebar). */
const SECONDARY = 'light-dark(#71717b, #8f8f8f)';
const MUTED = 'light-dark(#71717b, #a3a3a3)';
const timestamp = (value: unknown) => { const time = Date.parse(str(value)); return Number.isFinite(time) ? time : -Infinity; };

/**
 * Project groups in the sidebar picker's order (sortSidebarV2ProjectGroups):
 * the Project order setting sorts by each group's latest user message (or its
 * threads' creation) with no-thread groups on their own stamps; Manual keeps
 * the source order, as an empty projectOrder does.
 */
export const NO_PROJECT_SCOPE = '\0no-project';
export function projectScopes(client: T3Client): { key: string; name: string; ids: Set<string>; time: number; members: Obj[] }[] {
  const threads = [...client.shell.threads, ...fleetThreads()].filter(sidebarVisible);
  const order = projectSortOrder(client);
  const grouped = new Map<string, { key: string; members: Obj[]; name: string }>();
  const add = (key: string, name: string, members: Obj[]) => {
    const group = grouped.get(key) ?? { key, name, members: [] };
    group.members.push(...members); grouped.set(key, group);
  };
  for (const group of client.projectGroups()) {
    const scratch = group.members.every(member => member.isScratch === true || isScratch(member, str(client.config.scratchWorkspaceRoot)));
    add(scratch ? NO_PROJECT_SCOPE : group.key, scratch ? 'No project' : group.name,
      group.members.map(member => ({ ...member, environmentId: client.environmentId, isScratch: scratch })));
  }
  for (const entry of fleet.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation || entry.environmentId === client.environmentId) continue;
    for (const member of entry.shell.projects) {
      const scratch = member.isScratch === true || isScratch(member, str(entry.config.scratchWorkspaceRoot));
      add(scratch ? NO_PROJECT_SCOPE : logicalKey(client, entry.environmentId, member), scratch ? 'No project' : str(member.title),
        [{ ...member, environmentId: entry.environmentId, id: `fleet:${entry.environmentId}:${str(member.id)}`, isScratch: scratch }]);
    }
  }
  const groups = [...grouped.values()].map(group => {
    const ids = new Set(group.members.map(member => str(member.id)));
    const own = threads.filter(thread => ids.has(str(thread.projectId)));
    const time = own.length ? Math.max(...own.map(thread => threadSortTime(thread, order)))
      : Math.max(...group.members.map(member => order === 'created_at' ? timestamp(member.createdAt) : timestamp(member.updatedAt ?? member.createdAt)));
    return { key: group.key, name: group.key === NO_PROJECT_SCOPE ? 'No project' : groupLabel(group.members) || group.name, ids, time, members: group.members };
  });
  // A saved physical scratch filter may precede the config that identifies its No project group.
  const saved = sidebarPrefs(client).scope, scratch = groups.find(group => group.key === NO_PROJECT_SCOPE);
  if (saved && scratch?.members.some(member => saved === logicalKey(client, str(member.environmentId), member) || saved === `${str(member.environmentId)}:${str(member.workspaceRoot).trim().replace(/\\/g, '/').replace(/\/+$/, '')}`)) sidebarPrefs(client).scope = NO_PROJECT_SCOPE;
  // Manual follows the persisted project order (orderItemsByPreferredIds over physical keys), as the legacy sidebar arranges it.
  if (order === 'manual') return orderItemsByPreferredIds({ items: groups, preferredIds: sidebarPrefs(client).projectOrder, getId: group => group.key,
    getPreferenceIds: group => group.members.map(member => `${str(member.environmentId)}:${str(member.workspaceRoot).trim().replace(/\\/g, '/').replace(/\/+$/, '')}`) });
  return groups.sort((left, right) => (right.time === left.time ? 0 : right.time > left.time ? 1 : -1) || left.name.localeCompare(right.name) || left.key.localeCompare(right.key));
}
/** clientSettings.sidebarProjectSortOrder: updated_at (default), created_at or manual. */
export function projectSortOrder(client: T3Client): 'updated_at' | 'created_at' | 'manual' {
  const value = (client.local.clientSettings as Record<string, unknown> | undefined)?.sidebarProjectSortOrder;
  return value === 'created_at' || value === 'manual' ? value : 'updated_at';
}
/** getThreadSortTimestamp: creation (else update), or the latest user message (else update, else creation). */
function threadSortTime(thread: Obj, order: string): number {
  if (order === 'created_at') { const created = timestamp(thread.createdAt); return Number.isFinite(created) ? created : timestamp(thread.updatedAt); }
  const message = timestamp(thread.latestUserMessageAt);
  if (Number.isFinite(message)) return message;
  const updated = timestamp(thread.updatedAt);
  return Number.isFinite(updated) ? updated : timestamp(thread.createdAt);
}

export interface Partition { pinned: Obj[]; active: Obj[]; working: Obj[]; snoozed: Obj[]; settled: Obj[] }
/** The Working shelf's placement tracker of this window (thread-inbox.ts), keyed as the rows are. */
export const workingInbox = (client: T3Client) => inboxReturns(client, thread => scopedRowKey(client, str(thread.id)));
/** The five shelves for the current scope, each in its reference order. */
export function partition(client: T3Client, now: number): Partition {
  const caps = capabilities(client.config), prefs = sidebarPrefs(client);
  const scope = projectScopes(client).find(group => group.key === prefs.scope);
  const working = client.local.clientSettings?.sidebarWorkingShelfEnabled === true;
  const threads = [...client.shell.threads, ...fleetThreads()];
  const inbox = workingInbox(client).observe(working ? threads : null, now);
  const result: Partition = { pinned: [], active: [], working: [], snoozed: [], settled: [] };
  for (const thread of threads) {
    if (!sidebarVisible(thread) || scope && !scope.ids.has(str(thread.projectId))) continue;
    result[sectionOf(thread, caps, now, working, inbox.isWorking)].push(thread);
  }
  return { pinned: sortPinned(result.pinned), active: working ? sortByReturn(result.active, inbox.returnedAt) : sortActive(result.active),
    working: sortWorkingThreadsBySend(result.working), snoozed: sortSnoozed(result.snoozed), settled: sortSettled(result.settled) };
}

/** Rows as painted: collapsed shelves keep only the open thread; the settled tail pages. */
export function renderedRows(client: T3Client, parts: Partition): { thread: Obj; section: SidebarSection }[] {
  const prefs = sidebarPrefs(client), session = sidebarSession(client);
  const route = routeRowThreadId(client), open = (list: Obj[]) => list.filter(thread => thread.id === route);
  if (session.settledScope !== prefs.scope) { session.settledScope = prefs.scope; session.settledVisible = SETTLED_TAIL_INITIAL_COUNT; }
  let settled = parts.settled.slice(0, session.settledVisible);
  const deep = parts.settled.slice(session.settledVisible).find(thread => thread.id === route);
  if (deep) settled = [...settled, deep];
  const rows: { thread: Obj; section: SidebarSection }[] = [];
  const push = (list: Obj[], section: SidebarSection) => list.forEach(thread => rows.push({ thread, section }));
  push(parts.pinned, 'pinned'); push(parts.active, 'active');
  push(prefs.workingExpanded ? parts.working : open(parts.working), 'working');
  push(prefs.snoozedExpanded ? parts.snoozed : open(parts.snoozed), 'snoozed');
  push(prefs.settledExpanded ? settled : open(settled), 'settled');
  return rows;
}

/**
 * The title's ink (SidebarThreadRow title span). A settled row's whole content is secondary at rest unless it is open
 * (variantAction "unsettle"); a receding card title is foreground/75 and inks up on hover in the row.
 */
function titleInk(card: boolean, section: SidebarSection, recede: boolean, unread: boolean, woke: boolean, status: string, active: boolean): string {
  if (section === 'settled' && !active) return SECONDARY;
  return card
    ? recede ? 'light-dark(#18181bbf, #f3f3f3bf)' : status === 'failed' && !unread && !woke ? 'light-dark(#18181bf2, #f3f3f3f2)' : FOREGROUND
    : recede ? SECONDARY : active || woke || status === 'input' ? FOREGROUND : unread ? MUTED : 'light-dark(#71717bb3, #8f8f8fb3)';
}

/** Sidebar.tsx prBadge: the thread's links (resolveThreadPullRequestBadge), else, with no current link, the pull request
 * useLinkedThreadPullRequest reads for it (its branch's; sidebar-branch-pr.ts). */
function rowPrBadge(client: T3Client, thread: Obj): ReturnType<typeof sidebarPrBadge> {
  const linked = sidebarPrBadge({ ...thread, pullRequests: rowPullRequestLinks(client, thread) });
  if (linked.badge) return linked;
  const pr = branchPullRequest(client, thread);
  return pr ? branchPrBadge(pr) : linked;
}
/**
 * A row's place in the list: its thread with its environment, the same whether that environment is the focused one (`<id>`)
 * or a background one (`fleet:<environment>:<id>`). The reference keys its rows by the scoped thread key, so opening a
 * background row keeps its element; here the open moves the window to that environment, which renames the row's id,
 * and a list keyed by that id would replace the element between a double-click's two clicks.
 */
export function scopedRowKey(client: T3Client, id: string): string {
  const ref = parseFleetThreadId(id);
  return ref ? `${ref.environmentId}:${ref.threadId}` : `${client.environmentId}:${id}`;
}
/** projectGroupsSpanEnvironments: the sidebar's projects come from more than one machine. */
export function projectsSpanEnvironments(client: T3Client): boolean {
  let machines = client.shell.projects.length ? 1 : 0;
  for (const entry of fleet.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation || entry.environmentId === client.environmentId) continue;
    if (entry.shell.projects.length && ++machines > 1) return true;
  }
  return false;
}
/**
 * Sidebar.tsx scratchMachineLabelFor: when the projects span machines, a thread in a machine's "No project" folder
 * names that machine (its label and glyph) where a branch would be.
 */
export function scratchMachine(client: T3Client, thread: Obj, spans: boolean): { machine: string; label: string } | null {
  if (!spans) return null;
  const ref = parseFleetThreadId(str(thread.id));
  if (!ref) {
    const project = client.shell.projects.find(candidate => candidate.id === thread.projectId);
    return isScratch(project, scratchRootOf(true, client.config)) ? { machine: machineKind(client.config), label: str(obj(client.config.environment).label) } : null;
  }
  const entry = [...fleet.entries.values()].find(candidate => candidate.environmentId === ref.environmentId);
  const project = entry?.shell.projects.find(candidate => `fleet:${ref.environmentId}:${str(candidate.id)}` === thread.projectId);
  return entry && isScratch(project, scratchRootOf(true, entry.config)) ? { machine: str(thread.fleetMachine, 'server'), label: str(thread.fleetEnvironmentLabel) } : null;
}
/**
 * ProviderInstanceIcon's ACP icon (applyProviderInstanceSettings): an ACP Registry instance's agent id and icon
 * come from its settings entry's config; a local agent has none.
 */
function providerIconUrl(instance: Obj | undefined, settings: Obj): string {
  if (str(instance?.driver) !== 'acpRegistry') return '';
  const config = obj(obj(obj(settings.providerInstances)[str(instance?.instanceId)]).config);
  return config.source === 'local' ? '' : instanceIconUrl({ config }, 'acpRegistry');
}
/** The server config of the thread's own environment (Sidebar.tsx providerEntryByEnvironmentAndInstanceId). */
function threadConfig(client: T3Client, thread: Obj): Obj {
  const ref = parseFleetThreadId(str(thread.id));
  if (!ref || ref.environmentId === client.environmentId) return client.config;
  return obj([...fleet.entries.values()].find(entry => entry.environmentId === ref.environmentId)?.config);
}
function row(client: T3Client, thread: Obj, section: SidebarSection, now: number, helpers: SidebarHelpers, extra: { searchFirst: boolean; jump: string; searching: boolean; menu: MenuRow[]; spans: boolean; match?: Obj }): SidebarThread {
  const caps = capabilities(client.config), prefs = sidebarPrefs(client), session = sidebarSession(client);
  const environment = threadConfig(client, thread), providers = arr(environment.providers);
  const project = client.shell.projects.find(candidate => candidate.id === thread.projectId);
  // The label is the group's name (project-groups.ts); the favicon keeps the checkout's own title (ProjectFavicon).
  const projectTitle = str(thread.fleetProjectTitle) || str(project?.title), projectName = displayNames(client).get(str(thread.projectId)) || projectTitle;
  const instance = providers.find(provider => provider.instanceId === obj(thread.modelSelection).instanceId);
  const prs = arr(thread.pullRequests), id = str(thread.id);
  const terminalCount = terminalProcessCount(client, id);
  // The open thread's row, or its ancestor's while a subagent is open; a draft and a PR click follow the open thread itself.
  const active = id === routeRowThreadId(client), routeThread = id === client.threadId, multi = session.selection.includes(id);
  const visited = lastVisited(thread, prefs.visited[id]);
  const status = sidebarStatus(thread), unread = unseenCompletion(thread, visited), woke = isWoke(thread, wokeAt(thread, now), visited);
  const recede = recedes(status, unread, woke, active, multi), idleRecede = recedes(status, unread, woke, false, multi);
  const pill = topStatus(status, woke, unread, obj(thread.goal).status === 'active');
  const card = section === 'active' || section === 'pinned' || section === 'working';
  const started = statusSince(thread, status);
  const scratch = str(thread.branch) ? null : scratchMachine(client, thread, extra.spans);
  const prBadge = rowPrBadge(client, thread), remote = scratch ? null : remoteThread(client, thread);
  const searchPr = extra.searching ? searchPullRequest(client, thread, client.query) : null;
  const draftKey = `${client.environmentId}:${id}`;
  // A background environment's row draws its own server's project (its favicon is that server's).
  const ref = parseFleetThreadId(id), background = ref && ref.environmentId !== client.environmentId
    ? [...fleet.entries.values()].find(entry => entry.environmentId === ref.environmentId)?.shell.projects.find(candidate => `fleet:${ref.environmentId}:${str(candidate.id)}` === thread.projectId) : undefined;
  const glyph = background ? projectGlyph(client, background, helpers.projectIdentity, ref!.environmentId)
    : project ? projectGlyph(client, project, helpers.projectIdentity) : projectGlyph(client, { title: projectTitle }, helpers.projectIdentity);
  return {
    id, title: str(thread.title, 'Untitled thread'), projectName,
    // A title reads in its majority direction, start-aligned; the rename field follows what is typed, as `dir="auto"`.
    titleDir: textDirection(str(thread.title, 'Untitled thread')) ?? 'ltr',
    renameDir: autoDirection(session.renameId === id ? session.renameTitle : str(thread.title, 'Untitled thread')) ?? 'ltr', ...helpers.projectIdentity(projectTitle), ...grayIdentity(helpers.projectIdentity(projectTitle).projectInk),
    providerDriver: str(instance?.driver), ...helpers.providerBadge(instance, providers), providerName: str(instance?.displayName), providerIcon: providerIconUrl(instance, obj(environment.settings)),
    terminalCount, terminalLabel: terminalProcessLabel(terminalCount),
    age: section === 'settled' ? ageLabel(settledTimestamp(thread), now) : threadTimeLabel(thread, now),
    ...prBadge, // resolveThreadPullRequestBadge (pr-surface.ts), else the branch's pull request
    section, status: pill?.label ?? '', selected: active, routeThread, searchFirst: extra.searchFirst,
    ...searchMatch(extra.match, session.searchFoundFor), // a row listed for its message
    card, statusIcon: pill?.icon ?? '', statusColor: pill?.color ?? SECONDARY,
    // metaLineEmpty: only the provider icon would remain, so a card drops its third line and the icon joins the title.
    metaEmpty: !str(thread.branch) && !scratch && !prBadge.badge && !remote && terminalCount === 0,
    duration: started ? workingDuration(now - Date.parse(started)) : '',
    workingSince: started ? Date.parse(started) : 0, remoteMachine: remote?.machine ?? '',
    scratchMachine: scratch?.machine ?? '', scratchLabel: scratch?.label ?? '',
    searchPr: searchPr?.text ?? '', searchPrIcon: searchPr?.icon ?? '', searchPrState: searchPr?.state ?? '', searchPrLabel: searchPr?.label ?? '',
    titleColor: titleInk(card, section, recede, unread, woke, status, active), titleWeight: recede ? 400 : 500, recede,
    pinned: thread.pinnedAt != null, canPin: caps.pinning,
    canSettle: caps.settlement && card, canSnooze: caps.snooze && card && canSnooze(thread, now),
    canUnsettle: caps.settlement && section === 'settled', canWake: caps.snooze && section === 'snoozed',
    wakeLabel: section === 'snoozed' && thread.snoozedUntil != null ? snoozeWakeLabel(thread.snoozedUntil, now) : '',
    // ThreadWorktreeIndicator: the row draws FolderGit2 only for a worktree thread; a checkout thread's branch stands alone.
    woke, branch: str(thread.branch), ...branchSplit(str(thread.branch)), worktree: str(thread.worktreePath).trim() !== '', draft: !routeThread && !!(client.local.drafts[draftKey] ?? '').trim(),
    jump: extra.jump, renaming: session.renameId === id, multi, regenerating: thread.titleRegeneration != null || session.regenerating.has(id),
    label: [str(thread.title, 'Untitled thread'), pill?.label ?? '', projectName].filter(Boolean).join(', '),
    // A row's hover belongs to its place: a row that moves to another shelf under a still pointer is not hovered.
    hoverKey: extra.searching ? id : rowHoverKey(client, id, section), glyph, // a moved row's stale hover never matches again
    rowKey: scopedRowKey(client, id),
    // On a utility page no row is open (the route is not a thread): the open row paints as any other.
    idleRecede, idleTitleColor: titleInk(card, section, idleRecede, unread, woke, status, false), idleTitleWeight: idleRecede ? 400 : 500,
    tips: extra.searching ? [] : rowTips({ card, draft: !routeThread && !!(client.local.drafts[draftKey] ?? '').trim(), pinned: thread.pinnedAt != null, canPin: caps.pinning, woke,
      canSnooze: caps.snooze && card && canSnooze(thread, now), canSettle: caps.settlement && card, canWake: caps.snooze && section === 'snoozed', canUnsettle: caps.settlement && section === 'settled' }, textMeasure(client.presentation)),
    ...hoverDetails(client, thread, providers, instance, helpers),
    menu: extra.menu,
  };
}

/** shortcutLabelForCommand: the last binding for a command, as macOS glyphs. */
export function shortcutLabel(client: T3Client, command: string): string {
  const binding = [...arr(client.config.keybindings)].reverse().find(entry => entry.command === command);
  const shortcut = obj(binding?.shortcut), key = str(shortcut.key);
  return binding && key ? `${shortcut.ctrlKey ? '⌃' : ''}${shortcut.altKey ? '⌥' : ''}${shortcut.shiftKey ? '⇧' : ''}${shortcut.modKey || shortcut.metaKey ? '⌘' : ''}${key.length === 1 ? key.toUpperCase() : key}` : '';
}
/** threadUndoLabel: the undo shortcut's label, "⌘Z to undo", or "Undo" without one. */
export function undoLabel(client: T3Client): string {
  const shortcut = shortcutLabel(client, 'thread.undo');
  return shortcut ? `${shortcut} to undo` : 'Undo';
}
/**
 * The modifiers of each thread.jump.N shortcut in effect (effectiveShortcutsForCommand: the newest binding whose
 * `when` holds and whose chord no newer binding claims), named as the window names the ones held
 * (press-modifiers.contract heldModifiers), mod as ⌘ on a Mac and Control elsewhere: the window shows the rows'
 * jump labels while one set is held alone (shouldShowThreadJumpHintsForModifiers, app.contract threadJumpsHeld).
 */
export function threadJumpModifiers(client: T3Client, platform = PLATFORM): string[] {
  const mac = /mac|iphone|ipad|ipod/i.test(platform), sets = new Set<string>();
  for (let index = 1; index <= 9; index++) {
    const shortcut = effectiveShortcut(client.config, `thread.jump.${index}`);
    if (!shortcut) continue;
    const meta = shortcut.metaKey === true || (shortcut.modKey === true && mac), control = shortcut.ctrlKey === true || (shortcut.modKey === true && !mac);
    sets.add([meta ? 'meta' : '', control ? 'control' : '', shortcut.altKey ? 'alt' : '', shortcut.shiftKey ? 'shift' : ''].filter(Boolean).join('+'));
  }
  return [...sets];
}
/** thread.jump.N labels for the first nine painted rows, from the server's bindings. */
function jumpLabels(client: T3Client, count: number): string[] {
  return Array.from({ length: Math.min(9, count) }, (_, index) => shortcutLabel(client, `thread.jump.${index + 1}`));
}

/**
 * Sidebar.tsx's route reveal, before the shelves are partitioned: a route the project scope hides widens it to all
 * projects (persisted, the selection cleared), so the new route's first frame lists its row.
 */
function revealRoute(client: T3Client, searching: boolean): void {
  const prefs = sidebarPrefs(client), session = sidebarSession(client), route = routeRowThreadId(client);
  const scope = prefs.scope ? projectScopes(client).find(group => group.key === prefs.scope) : undefined;
  const { widensScope } = resolveRouteReveal({ reveal: session.reveal, routeKey: route ? scopedRowKey(client, route) : '', searching, scoped: !!scope,
    hidden: key => {
      const thread = [...client.shell.threads, ...fleetThreads()].find(candidate => scopedRowKey(client, str(candidate.id)) === key);
      return thread && sidebarVisible(thread) ? !scope?.ids.has(str(thread.projectId)) : undefined;
    } });
  if (widensScope) { prefs.scope = ''; session.selection = []; session.anchor = ''; }
}
/** The open row to scroll to: once for the route (revealPending), once for each mount of the view (revealRow). */
function revealRows(client: T3Client, rows: { thread: Obj }[], searching: boolean): { revealRow: string; revealPending: string } {
  const route = routeRowThreadId(client);
  const revealRow = !searching && route && rows.some(entry => entry.thread.id === route) ? route : '';
  return { revealRow, revealPending: revealRow && sidebarSession(client).reveal.pendingKey === scopedRowKey(client, revealRow) ? revealRow : '' };
}

export function sidebarSnapshot(client: T3Client, now: number, helpers: SidebarHelpers): { threads: SidebarThread[]; searchFirstId: string; snoozePresets: SnoozePreset[]; sidebar: SidebarView } {
  // The legacy sidebar keeps its own reveal while it is the one shown (legacy-sidebar-view.ts).
  const searching = client.query.trim() !== '', reveals = client.local.clientSettings?.legacySidebarEnabled !== true;
  noteSnapshotTime(client, now);
  if (reveals) revealRoute(client, searching);
  const prefs = sidebarPrefs(client), session = sidebarSession(client), parts = partition(client, now);
  const scopes = projectScopes(client), scope = scopes.find(group => group.key === prefs.scope);
  notePlaces(client, parts); // a row's moves count while its shelf is collapsed too
  const rows: { thread: Obj; section: SidebarSection; match?: Obj }[] = searching ? searchRows(client, parts, now) : renderedRows(client, parts);
  if (session.searchQuery !== client.query) { session.searchQuery = client.query; session.searchKey = ''; }
  const searchIndex = Math.max(0, session.searchKey ? rows.findIndex(entry => entry.thread.id === session.searchKey) : 0);
  // The embedded terminal owns keystrokes while it has focus, and a search numbers no rows.
  const jumping = !searching && !terminalFocused(client);
  const jumps = jumping ? jumpLabels(client, rows.length) : [];
  // A right-click opens the row's own menu; on a row of the selection, the bulk menu over the selected rows painted now.
  const selected = searching ? [] : session.selection.map(id => rows.find(entry => entry.thread.id === id)?.thread).filter((thread): thread is Obj => !!thread);
  const bulk = selected.length ? menuRows(bulkMenuState(client, selected, now).items) : [];
  // A background environment's row has its menu too, by that environment's capabilities (handleThreadContextMenu).
  const menuOf = (thread: Obj): MenuRow[] => searching ? []
    : session.selection.includes(str(thread.id)) ? bulk : menuRows(threadMenuItems(threadMenuState(client, thread, false, scopes, now)));
  const spans = projectsSpanEnvironments(client);
  const threads = rows.map(({ thread, section, match }, index) => row(client, thread, section, now, helpers,
    { searchFirst: searching && index === searchIndex, jump: jumps[index] ?? '', searching, menu: menuOf(thread), spans, match }));
  const total = parts.pinned.length + parts.active.length + parts.working.length + parts.snoozed.length + parts.settled.length;
  const shownSettled = Math.min(parts.settled.length, session.settledVisible + (parts.settled.slice(session.settledVisible).some(thread => thread.id === routeRowThreadId(client)) ? 1 : 0));
  const hidden = parts.settled.length - shownSettled;
  const identity = helpers.projectIdentity(scope ? groupTitle(scope) : '');
  const undo = session.undo, live = undoLive(client);
  const pill = sidebarProviderPill(client, clock(now));
  const inbox = workingInbox(client), graceUntil = inbox.nextDeadline(), graceOwed = inbox.owed();
  return {
    threads, searchFirstId: searching ? str(rows[searchIndex]?.thread.id) : '',
    snoozePresets: snoozePresets(now, client.local.deviceSettings.timestampFormat),
    sidebar: {
      drafts: searching ? [] : draftRows(client, helpers.projectIdentity),
      searching, searchPending: searching && rows.length === 0 && threadSearchPending(client), searchIndex,
      searchSelectedId: searching ? str(rows[searchIndex]?.thread.id) : '',
      settledCount: parts.settled.length, snoozedCount: parts.snoozed.length, workingCount: parts.working.length, probes: SIDEBAR_PROBES,
      // A Working row's label ticks each second, and so does the clock a timed provider pill waits on. The Working
      // shelf's grace runs on the root's timers (app.contract workingGraceOwed, workingGraceEnds).
      ticking: threads.some(thread => thread.workingSince > 0) || (pill?.until ?? 0) > 0, graceUntil, graceOwed,
      settledExpanded: prefs.settledExpanded, snoozedExpanded: prefs.snoozedExpanded, workingExpanded: prefs.workingExpanded,
      showMore: prefs.settledExpanded && hidden > 0 ? Math.min(hidden, SETTLED_TAIL_PAGE_COUNT) : 0,
      emptyText: searching || total > 0 || draftRows(client, helpers.projectIdentity).length > 0 ? '' : client.shell.projects.length === 0 ? 'No projects yet' : scope ? `No threads in ${scope.name} yet` : 'No threads yet',
      emptyAddProject: !searching && total === 0 && client.shell.projects.length === 0,
      hasProjects: client.shell.projects.length > 0, projectGroupCount: scopes.length,
      dragEnabled: !searching && capabilities(client.config).pinning && capabilities(client.config).pinReorder && session.renameId === '',
      scopeKey: scope?.key ?? '', scopeLabel: scope?.name ?? '', scopeMark: scope ? identity.projectMark : '', scopeInk: scope ? identity.projectInk : '', scopeSurface: scope ? identity.projectSurface : '',
      scopeGlyph: groupGlyph(client, scope, helpers.projectIdentity),
      scopes: scopeItems(client, session.scopeQuery, scope?.key ?? '', scopes, helpers.projectIdentity),
      scopeOpen: session.scopeOpen, scopeQuery: session.scopeQuery,
      scopeFirst: scopeItems(client, session.scopeQuery, scope?.key ?? '', scopes, helpers.projectIdentity)[0]?.key ?? '',
      renameId: session.renameId, renameTitle: session.renameTitle, renameHeld: renameHeld(client), selectionCount: session.selection.length,
      undoText: undo && live ? `${undoSummary(undo)},` : '', // drafts (upstream 95edeb753b)
      undoUntil: undo && live ? undo.at + 5000 : 0, // the window hides the notice at this wall time (showThreadUndoNotice's 5 s)
      undoLabel: undoLabel(client),
      // The first bottom shelf header hosts the notice; while searching or with no rows it heads the footer.
      undoHost: searching || total === 0 ? '' : parts.working.length > 0 ? 'working' : parts.snoozed.length > 0 ? 'snoozed' : 'settled',
      dialog: session.dialog.kind, dialogTitle: dialogTitle(session.dialog), dialogDescription: dialogDescription(session.dialog),
      dialogMode: session.dialogMode, dialogDate: session.dialogDate, dialogTime: session.dialogTime, dialogAmount: session.dialogAmount, dialogUnit: session.dialogUnit, dialogError: session.dialogError,
      // The date button's text and its calendar popover.
      calendar: session.dialog.kind !== 'snooze' ? NO_CALENDAR
        : snoozeCalendar(session.dialogDate, session.calendarMonth || session.dialogDate.slice(0, 7), localDate(new Date(now)), firstWeekday(), session.calendarFocus, session.calendarFocusSeq),
      dialogReturn: dialogReturn(session.dialog), // Base UI's finalFocus
      jumpModifiers: jumping ? threadJumpModifiers(client) : [],
      pillKey: pill?.key ?? '', pillTone: pill?.tone ?? '', pillTitle: pill?.title ?? '', pillDescription: pill?.description ?? '', pillDismissible: pill?.dismissible ?? false, pillUntil: pill?.until ?? 0,
      sweepEpoch: session.sweepEpoch, // upstream 1826fb55cc
      ...(reveals ? revealRows(client, rows, searching) : { revealRow: '', revealPending: '' }),
    },
  };
}

function dialogTitle(dialog: { kind: string; threadIds: string[]; title: string }): string {
  if (dialog.kind === 'archive') return `Archive thread "${dialog.title}"?`;
  if (dialog.kind === 'delete') return `Delete thread "${dialog.title}"?`;
  if (dialog.kind === 'delete-many') return `Delete ${dialog.threadIds.length} thread${dialog.threadIds.length === 1 ? '' : 's'}?`;
  if (dialog.kind === 'snooze') return 'Snooze until';
  if (dialog.kind === 'unpin') return `Unpin thread "${dialog.title}"?`;
  if (dialog.kind === 'delete-worktree') return WORKTREE_DIALOG_TITLE;
  return '';
}
/** Where a dialog the sidebar opened gives the focus back as it closes (Base UI's finalFocus): its one
 * thread's row (`sidebar-row.contract`), whose own snooze and menu triggers collapse once the pointer leaves;
 * the chat header's title (`chat.contract`) for the Custom snooze its menu opened. */
export function dialogReturn(dialog: { kind: string; threadIds: string[]; from?: string }): string {
  if (dialog.kind !== '' && dialog.from === 'title') return 'thread-title';
  return dialog.kind !== '' && dialog.threadIds.length === 1 ? `thread-${dialog.threadIds[0]}` : '';
}
function dialogDescription(dialog: { kind: string; threadIds: string[]; title: string }): string {
  if (dialog.kind === 'delete-worktree') return worktreeDialogDescription(dialog.title);
  if (dialog.kind === 'delete') return 'This permanently clears conversation history for this thread.';
  if (dialog.kind === 'delete-many') return 'This permanently clears conversation history for these threads.';
  if (dialog.kind === 'unpin') return 'This will move the thread out of your pinned section.';
  return '';
}

/** A project group's glyph: its representative member's own (ProjectFavicon over the snapshot, whose title is the member's). */
function groupGlyph(client: T3Client, group: { key?: string; name: string; members: Obj[] } | undefined, identity: Identity): ProjectGlyph {
  return projectGlyph(client, group ? { ...(group.members[0] ?? {}), title: groupTitle(group) } : { title: '' }, identity);
}
/** The representative member's title, which names the group's monogram; the group's name for No project or a member without one. */
const groupTitle = (group: { key?: string; name: string; members: Obj[] }): string => group.key === NO_PROJECT_SCOPE ? group.name : str(group.members[0]?.title) || group.name;

/** filterSidebarProjectScopeItems: a query hides "All projects" and keeps matching groups. */
function scopeItems(client: T3Client, query: string, selected: string, scopes: { key: string; name: string; members: Obj[] }[], identity: Identity): SidebarScope[] {
  const needle = query.trim().toLowerCase();
  const items: SidebarScope[] = [{ key: '', label: 'All projects', mark: '', ink: '', surface: '', selected: selected === '', all: true, glyph: groupGlyph(client, undefined, identity) },
    ...scopes.map(group => { const mark = identity(groupTitle(group)); return { key: group.key, label: group.name, mark: mark.projectMark, ink: mark.projectInk, surface: mark.projectSurface, selected: group.key === selected, all: false, glyph: groupGlyph(client, group, identity) }; })];
  return needle ? items.filter(item => !item.all && item.label.toLowerCase().includes(needle)) : items;
}

/** SidebarDraftBlock: new-thread drafts with text or attachments, other than the one on screen, newest project first. */
export function draftRows(client: T3Client, identity: Identity): SidebarDraft[] {
  const prefix = `${client.environmentId}:new:`, scope = projectScopes(client).find(group => group.key === sidebarPrefs(client).scope);
  const rows: SidebarDraft[] = [];
  for (const project of client.shell.projects) {
    const id = str(project.id), key = `${prefix}${id}`;
    if (!client.threadId && client.projectId === id) continue;
    if (scope && !scope.ids.has(id)) continue;
    const text = (client.local.drafts[key] ?? '').trim(), attachments = (client.local.snapshotDrafts?.[key] ?? []).length;
    if (!text && !attachments) continue;
    const name = str(project.title), mark = identity(name);
    // SidebarDraftRow's promptPreview: a context chip reads as its label (replaceComposerContextReferences), not its link.
    const prompt = replaceContextReferences(text, reference => reference.label).trim().split('\n', 1)[0] ?? '';
    rows.push({ id, projectName: name, mark: mark.projectMark, ink: mark.projectInk, surface: mark.projectSurface, glyph: projectGlyph(client, project, identity),
      preview: prompt || `${attachments} attachment${attachments === 1 ? '' : 's'}`, menu: draftMenuRows(client, id) });
  }
  return rows;
}

/** The branch's head and the tail middle truncation keeps (both '' when it needs no split). */
function branchSplit(branch: string): { branchHead: string; branchTail: string } {
  const split = splitForMiddleTruncate(branch);
  return { branchHead: split?.head ?? '', branchTail: split?.tail ?? '' };
}

/** SidebarThreadTooltip: environment, model · instance, hand-offs and the last error. The model is the name it is said by
 * (getTriggerDisplayModelName under the instance's driver), else the selection's slug. */
function hoverDetails(client: T3Client, thread: Obj, providers: Obj[], instance: Obj | undefined, helpers: SidebarHelpers) {
  const kind = str(thread.fleetMachine) || machineKind(client.config), selection = obj(thread.modelSelection);
  const model = arr(instance?.models).find(entry => entry.slug === selection.model);
  const modelLabel = model ? triggerDisplayModelName(model, str(instance?.driver)) : str(selection.model);
  const badge = helpers.providerBadge(instance, providers).providerBadge !== '';
  const current = str(selection.instanceId);
  const history = Array.isArray(thread.providerInstanceHistory) ? thread.providerInstanceHistory : [];
  const previous = history.map(value => str(value)).filter(id => id && id !== current)
    .map(id => str(providers.find(provider => provider.instanceId === id)?.displayName, id));
  const runtime = thread.lastError ? str(thread.lastErrorClass) : '';
  return {
    hoverEnvironment: str(thread.fleetEnvironmentLabel) || str(obj(client.config.environment).label),
    hoverMachine: kind === 'desktop' ? 'monitor' : ['laptop', 'cloud', 'mac-mini', 'mac-studio'].includes(kind) ? kind : 'server',
    hoverModel: instance ? (badge ? `${modelLabel} · ${str(instance.displayName)}` : modelLabel) : '',
    hoverHandoff: previous.length ? `Handed off from ${previous.join(', ')}` : '',
    hoverError: thread.lastError ? (runtime === 'usage_limit' ? 'Usage limit reached' : 'Error occurred') : '',
    hoverErrorWarning: runtime === 'usage_limit',
  };
}
