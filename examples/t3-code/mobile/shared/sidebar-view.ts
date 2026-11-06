// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/sidebar-view.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The sidebar's projection into the snapshot (Sidebar.tsx render): the rows in
// paint order (collapsed shelves contribute only the open thread, as the
// reference renders them, so thread.jump and prev/next follow the same list)
// plus the sidebar's own view state. Pure over the client and `now`.
import { arr, num, obj, str, type Obj } from './domain';
import { machineKind } from './connections';
import { fleetThreads, parseFleetThreadId, fleetTerminalProcessCount } from './settings-b-fleet'; // settings-b: background environments' threads
import type { T3Client } from './client';
import { knownSessions } from './terminal-drawer-view';
import { selectRunningSubprocessTerminalIds } from './terminal-session';
import { terminalFocused } from './terminal-focus';
import { searchMatch, serverMatches, snoozePresets, threadSearchPending, type SearchPart, type SnoozePreset } from './sidebar-presentation';
import { SETTLED_TAIL_INITIAL_COUNT, SETTLED_TAIL_PAGE_COUNT, clock, sidebarPrefs, sidebarSession, undoLive } from './sidebar-state';
import { sidebarProviderPill } from './sidebar-provider-pill';
import { grayIdentity, projectGlyph, type ProjectGlyph } from './r3-sidebar-glyph';
import { rowTips, type SidebarRowTip } from './r3-sidebar-tips';
import { cachedDisplayNames as displayNames } from './r6-polish-groups';
import { SIDEBAR_PROBES, textMeasure } from './r6-polish-measure';
import type { Probe } from './r5-composer-menus';
export { grayIdentity } from './r3-sidebar-glyph';
import { ageLabel, capabilities, canSnooze, isWoke, lastVisited, recedes, sectionOf, settledTimestamp, sidebarStatus, sidebarVisible,
  snoozeWakeLabel, sortActive, sortByReturn, sortPinned, sortSettled, sortSnoozed, sortWorkingThreadsBySend, threadTimeLabel, topStatus, unseenCompletion,
  wokeAt, workingDuration, workingStartedAt, type SidebarSection } from './sidebar-model';
import { sidebarPrBadge } from './r5-panels-pr'; // r5-panels: the sidebar PR badge
import { notePlaces, rowHoverKey } from './r9-input-hover'; // lane r9-input
import { orderItemsByPreferredIds } from './legacy-sidebar-model'; // legacy-sidebar: the persisted project order
import { WORKTREE_DIALOG_TITLE, worktreeDialogDescription } from './worktree-cleanup'; // thread-commands-and-keys: G5

type Identity = (name: string) => { projectMark: string; projectInk: string; projectSurface: string };
type Badge = (provider: Obj | undefined, providers: Obj[]) => { providerBadge: string; providerBadgeColor: string };
export interface SidebarHelpers { projectIdentity: Identity; providerBadge: Badge }

/** ThreadStatusIndicators/Sidebar.tsx: shells alone are idle; only child processes count. */
export function terminalProcessCount(client: T3Client, threadId: string): number {
  const ref = parseFleetThreadId(threadId) ?? { environmentId: client.environmentId, threadId };
  if (ref.environmentId !== client.environmentId) return fleetTerminalProcessCount(ref.environmentId, ref.threadId);
  return selectRunningSubprocessTerminalIds(knownSessions(client, ref)).length;
}
export const terminalProcessLabel = (count: number): string => count ? `${count} terminal ${count === 1 ? 'process' : 'processes'} running` : '';

export interface SidebarThread {
  id: string; title: string; projectName: string; projectMark: string; projectInk: string; projectSurface: string;
  projectGray: string; projectGraySurface: string; providerDriver: string; providerBadge: string; providerBadgeColor: string; providerName: string;
  terminalCount: number; terminalLabel: string;
  age: string; badge: string; stacked: boolean; badgeIcon: string; badgeState: string; section: string; status: string; selected: boolean; searchFirst: boolean;
  matchLabel: string; matchUser: boolean; matchParts: SearchPart[];
  card: boolean; statusIcon: string; statusColor: string; duration: string; titleColor: string; titleWeight: number;
  recede: boolean; pinned: boolean; canPin: boolean; canSettle: boolean; canSnooze: boolean; canUnsettle: boolean; canWake: boolean;
  wakeLabel: string; woke: boolean; branch: string; worktree: boolean; draft: boolean; jump: string; renaming: boolean; multi: boolean;
  regenerating: boolean; label: string; hoverKey: string; glyph: ProjectGlyph;
  idleRecede: boolean; idleTitleColor: string; idleTitleWeight: number; tips: SidebarRowTip[];
  hoverEnvironment: string; hoverMachine: string; hoverModel: string; hoverHandoff: string; hoverError: string; hoverErrorWarning: boolean;
  workingSince: number; remoteMachine: string; // r6-polish: the ticking Working label; a background environment's machine glyph
}
export interface SidebarScope { key: string; label: string; mark: string; ink: string; surface: string; selected: boolean; all: boolean; glyph: ProjectGlyph }
export interface SidebarDraft { id: string; projectName: string; mark: string; ink: string; surface: string; preview: string; glyph: ProjectGlyph }
export interface SidebarView {
  drafts: SidebarDraft[];
  searching: boolean; searchPending: boolean; searchIndex: number; searchSelectedId: string;
  settledCount: number; snoozedCount: number; workingCount: number; ticking: boolean; probes: Probe[];
  settledExpanded: boolean; snoozedExpanded: boolean; workingExpanded: boolean; showMore: number;
  emptyText: string; emptyAddProject: boolean; hasProjects: boolean; projectGroupCount: number; dragEnabled: boolean;
  scopeKey: string; scopeLabel: string; scopeMark: string; scopeInk: string; scopeSurface: string; scopeGlyph: ProjectGlyph; scopes: SidebarScope[];
  scopeOpen: boolean; scopeQuery: string; scopeFirst: string; navigateKind: string; navigateProject: string;
  renameId: string; renameTitle: string; selectionCount: number; undoText: string; undoLabel: string; undoUntil: number;
  dialog: string; dialogTitle: string; dialogDescription: string; dialogMode: string; dialogDate: string; dialogTime: string; dialogAmount: string; dialogUnit: string; dialogError: string; jumpHints: boolean;
  pillKey: string; pillTone: string; pillTitle: string; pillDescription: string; pillDismissible: boolean; sweepEpoch: number;
}

const FOREGROUND = 'light-dark(#27272a, #f3f3f3)';
const SECONDARY = 'light-dark(#71717b, #818181)';
const timestamp = (value: unknown) => { const time = Date.parse(str(value)); return Number.isFinite(time) ? time : -Infinity; };

/**
 * Project groups in the sidebar picker's order (sortSidebarV2ProjectGroups):
 * the Project order setting sorts by each group's latest user message (or its
 * threads' creation) with no-thread groups on their own stamps; Manual keeps
 * the source order, as an empty projectOrder does.
 */
export function projectScopes(client: T3Client): { key: string; name: string; ids: Set<string>; time: number; members: Obj[] }[] {
  const threads = client.shell.threads.filter(sidebarVisible);
  const order = projectSortOrder(client);
  const groups = client.projectGroups().map(group => {
    const ids = new Set(group.members.map(member => str(member.id)));
    const own = threads.filter(thread => ids.has(str(thread.projectId)));
    const time = own.length ? Math.max(...own.map(thread => threadSortTime(thread, order)))
      : Math.max(...group.members.map(member => order === 'created_at' ? timestamp(member.createdAt) : timestamp(member.updatedAt ?? member.createdAt)));
    return { key: group.key, name: str(group.name), ids, time, members: group.members };
  });
  // legacy-sidebar: Manual follows the persisted project order (orderItemsByPreferredIds over physical keys), as the legacy sidebar arranges it.
  if (order === 'manual') return orderItemsByPreferredIds({ items: groups, preferredIds: sidebarPrefs(client).projectOrder, getId: group => group.key,
    getPreferenceIds: group => group.members.map(member => `${client.environmentId}:${str(member.workspaceRoot).trim().replace(/\\/g, '/').replace(/\/+$/, '')}`) });
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
/** The five shelves for the current scope, each in its reference order. */
export function partition(client: T3Client, now: number): Partition {
  const caps = capabilities(client.config), prefs = sidebarPrefs(client);
  const scope = projectScopes(client).find(group => group.key === prefs.scope);
  const working = client.local.clientSettings?.sidebarWorkingShelfEnabled === true;
  const result: Partition = { pinned: [], active: [], working: [], snoozed: [], settled: [] };
  for (const thread of [...client.shell.threads, ...fleetThreads()]) {
    if (!sidebarVisible(thread) || scope && !scope.ids.has(str(thread.projectId))) continue;
    result[sectionOf(thread, caps, now, working)].push(thread);
  }
  return { pinned: sortPinned(result.pinned), active: working ? sortByReturn(result.active) : sortActive(result.active),
    working: sortWorkingThreadsBySend(result.working), snoozed: sortSnoozed(result.snoozed), settled: sortSettled(result.settled) };
}

/** Rows as painted: collapsed shelves keep only the open thread; the settled tail pages. */
export function renderedRows(client: T3Client, parts: Partition): { thread: Obj; section: SidebarSection }[] {
  const prefs = sidebarPrefs(client), session = sidebarSession(client);
  const open = (list: Obj[]) => list.filter(thread => thread.id === client.threadId);
  if (session.settledScope !== prefs.scope) { session.settledScope = prefs.scope; session.settledVisible = SETTLED_TAIL_INITIAL_COUNT; }
  let settled = parts.settled.slice(0, session.settledVisible);
  const deep = parts.settled.slice(session.settledVisible).find(thread => thread.id === client.threadId);
  if (deep) settled = [...settled, deep];
  const rows: { thread: Obj; section: SidebarSection }[] = [];
  const push = (list: Obj[], section: SidebarSection) => list.forEach(thread => rows.push({ thread, section }));
  push(parts.pinned, 'pinned'); push(parts.active, 'active');
  push(prefs.workingExpanded ? parts.working : open(parts.working), 'working');
  push(prefs.snoozedExpanded ? parts.snoozed : open(parts.snoozed), 'snoozed');
  push(prefs.settledExpanded ? settled : open(settled), 'settled');
  return rows;
}

/** The search's matches over every shelf in sidebar order: title matches first, then server content matches. */
export function searchRows(client: T3Client, parts: Partition): { thread: Obj; section: SidebarSection }[] {
  const query = client.query.trim().toLowerCase();
  if (!query) return [];
  const server = serverMatches(client), titles: { thread: Obj; section: SidebarSection }[] = [], content: typeof titles = [];
  for (const section of ['pinned', 'active', 'working', 'snoozed', 'settled'] as SidebarSection[]) {
    for (const thread of parts[section]) {
      const terms = [str(thread.title), ...arr(thread.pullRequests).flatMap(pr => [`#${num(pr.number)}`, str(pr.title), str(pr.url)])];
      if (terms.some(term => term.toLowerCase().includes(query))) titles.push({ thread, section });
      else if (server.has(str(thread.id))) content.push({ thread, section });
    }
  }
  return [...titles, ...content];
}

/** The title's ink (Sidebar.tsx title span); a settled row's whole content is secondary/70 at rest. */
function titleInk(card: boolean, section: SidebarSection, recede: boolean, unread: boolean, woke: boolean, status: string, active: boolean): string {
  if (section === 'settled') return 'light-dark(#71717bb3, #818181b3)';
  return card
    ? recede ? SECONDARY : unread || woke || status === 'input' ? FOREGROUND : status === 'failed' ? 'light-dark(#27272af2, #f3f3f3f2)' : 'light-dark(#27272ae6, #f3f3f3e6)'
    : recede ? 'light-dark(#71717bb3, #818181b3)' : active || woke || status === 'input' ? FOREGROUND : unread ? SECONDARY : 'light-dark(#71717bb3, #818181b3)';
}

function row(client: T3Client, thread: Obj, section: SidebarSection, now: number, helpers: SidebarHelpers, extra: { searchFirst: boolean; jump: string; searching: boolean }): SidebarThread {
  const caps = capabilities(client.config), prefs = sidebarPrefs(client), session = sidebarSession(client);
  const providers = arr(client.config.providers);
  const project = client.shell.projects.find(candidate => candidate.id === thread.projectId);
  // r6-polish: the label is the group's name (r6-polish-groups.ts); the favicon keeps the checkout's own title (ProjectFavicon).
  const projectTitle = str(thread.fleetProjectTitle) || str(project?.title), projectName = displayNames(client).get(str(thread.projectId)) || projectTitle;
  const instance = providers.find(provider => provider.instanceId === obj(thread.modelSelection).instanceId);
  const prs = arr(thread.pullRequests), id = str(thread.id);
  const terminalCount = terminalProcessCount(client, id);
  const active = id === client.threadId, multi = session.selection.includes(id);
  const visited = lastVisited(thread, prefs.visited[id]);
  const status = sidebarStatus(thread), unread = unseenCompletion(thread, visited), woke = isWoke(thread, wokeAt(thread, now), visited);
  const recede = recedes(status, unread, woke, active, multi), idleRecede = recedes(status, unread, woke, false, multi);
  const pill = topStatus(status, woke, unread);
  const card = section === 'active' || section === 'pinned' || section === 'working';
  const started = workingStartedAt(thread);
  const draftKey = `${client.environmentId}:${id}`;
  const glyph = project ? projectGlyph(client, project, helpers.projectIdentity) : projectGlyph(client, { title: projectTitle }, helpers.projectIdentity);
  return {
    id, title: str(thread.title, 'Untitled thread'), projectName, ...helpers.projectIdentity(projectTitle), ...grayIdentity(helpers.projectIdentity(projectTitle).projectInk),
    providerDriver: str(instance?.driver), ...helpers.providerBadge(instance, providers), providerName: str(instance?.displayName),
    terminalCount, terminalLabel: terminalProcessLabel(terminalCount),
    age: section === 'settled' ? ageLabel(settledTimestamp(thread), now) : threadTimeLabel(thread, now),
    ...sidebarPrBadge(thread), // r5-panels: resolveThreadPullRequestBadge (r5-panels-pr.ts)
    section, status: pill?.label ?? '', selected: active, searchFirst: extra.searchFirst,
    ...searchMatch(thread, client.query, serverMatches(client).get(id)),
    card, statusIcon: pill?.icon ?? '', statusColor: pill?.color ?? SECONDARY,
    duration: status === 'working' && started ? workingDuration(now - Date.parse(started)) : '',
    workingSince: status === 'working' && started && Number.isFinite(Date.parse(started)) ? Date.parse(started) : 0, remoteMachine: str(thread.fleetMachine),
    titleColor: titleInk(card, section, recede, unread, woke, status, active), titleWeight: recede ? 400 : 500, recede,
    pinned: thread.pinnedAt != null, canPin: caps.pinning,
    canSettle: caps.settlement && card, canSnooze: caps.snooze && card && canSnooze(thread, now),
    canUnsettle: caps.settlement && section === 'settled', canWake: caps.snooze && section === 'snoozed',
    wakeLabel: section === 'snoozed' && thread.snoozedUntil != null ? snoozeWakeLabel(thread.snoozedUntil, now) : '',
    // ThreadWorktreeIndicator: the row draws FolderGit2 only for a worktree thread; a checkout thread's branch stands alone.
    woke, branch: str(thread.branch), worktree: str(thread.worktreePath).trim() !== '', draft: !active && !!(client.local.drafts[draftKey] ?? '').trim(),
    jump: extra.jump, renaming: session.renameId === id, multi, regenerating: thread.titleRegeneration != null || session.regenerating.has(id),
    label: [str(thread.title, 'Untitled thread'), pill?.label ?? '', projectName].filter(Boolean).join(', '),
    // A row's hover belongs to its place: a row that moves to another shelf under a still pointer is not hovered.
    hoverKey: extra.searching ? id : rowHoverKey(client, id, section), glyph, // lane r9-input: a moved row's stale hover never matches again
    // On a utility page no row is open (the route is not a thread): the open row paints as any other.
    idleRecede, idleTitleColor: titleInk(card, section, idleRecede, unread, woke, status, false), idleTitleWeight: idleRecede ? 400 : 500,
    tips: extra.searching ? [] : rowTips({ card, draft: !active && !!(client.local.drafts[draftKey] ?? '').trim(), pinned: thread.pinnedAt != null, canPin: caps.pinning, woke,
      canSnooze: caps.snooze && card && canSnooze(thread, now), canSettle: caps.settlement && card, canWake: caps.snooze && section === 'snoozed', canUnsettle: caps.settlement && section === 'settled' }, textMeasure(client.presentation)),
    ...hoverDetails(client, thread, providers, instance, helpers),
  };
}

/** shortcutLabelForCommand: the last binding for a command, as macOS glyphs. */
export function shortcutLabel(client: T3Client, command: string): string {
  const binding = [...arr(client.config.keybindings)].reverse().find(entry => entry.command === command);
  const shortcut = obj(binding?.shortcut), key = str(shortcut.key);
  return binding && key ? `${shortcut.ctrlKey ? '⌃' : ''}${shortcut.altKey ? '⌥' : ''}${shortcut.shiftKey ? '⇧' : ''}${shortcut.modKey || shortcut.metaKey ? '⌘' : ''}${key.length === 1 ? key.toUpperCase() : key}` : '';
}
/** thread.jump.N labels for the first nine painted rows, from the server's bindings. */
function jumpLabels(client: T3Client, count: number): string[] {
  return Array.from({ length: Math.min(9, count) }, (_, index) => shortcutLabel(client, `thread.jump.${index + 1}`));
}

export function sidebarSnapshot(client: T3Client, now: number, helpers: SidebarHelpers): { threads: SidebarThread[]; searchFirstId: string; snoozePresets: SnoozePreset[]; sidebar: SidebarView } {
  const prefs = sidebarPrefs(client), session = sidebarSession(client), parts = partition(client, now);
  const scopes = projectScopes(client), scope = scopes.find(group => group.key === prefs.scope);
  const searching = client.query.trim() !== '';
  notePlaces(client, parts); // lane r9-input: a row's moves count while its shelf is collapsed too
  const rows = searching ? searchRows(client, parts) : renderedRows(client, parts);
  if (session.searchQuery !== client.query) { session.searchQuery = client.query; session.searchIndex = 0; }
  const searchIndex = rows.length ? Math.min(session.searchIndex, rows.length - 1) : 0;
  const hints = !searching && !terminalFocused(client) && client.presentation.sidebarJumpHints === true;
  const jumps = hints ? jumpLabels(client, rows.length) : [];
  const threads = rows.map(({ thread, section }, index) => row(client, thread, section, now, helpers,
    { searchFirst: searching && index === searchIndex, jump: jumps[index] ?? '', searching }));
  const total = parts.pinned.length + parts.active.length + parts.working.length + parts.snoozed.length + parts.settled.length;
  const shownSettled = Math.min(parts.settled.length, session.settledVisible + (parts.settled.slice(session.settledVisible).some(thread => thread.id === client.threadId) ? 1 : 0));
  const hidden = parts.settled.length - shownSettled;
  const identity = helpers.projectIdentity(scope?.name ?? '');
  const undo = session.undo, live = undoLive(client);
  const pill = sidebarProviderPill(client, clock(now));
  return {
    threads, searchFirstId: searching ? str(rows[searchIndex]?.thread.id) : '',
    snoozePresets: snoozePresets(now, client.local.deviceSettings.timestampFormat),
    sidebar: {
      drafts: searching ? [] : draftRows(client, helpers.projectIdentity),
      searching, searchPending: searching && rows.length === 0 && threadSearchPending(client), searchIndex,
      searchSelectedId: searching ? str(rows[searchIndex]?.thread.id) : '',
      settledCount: parts.settled.length, snoozedCount: parts.snoozed.length, workingCount: parts.working.length, ticking: threads.some(thread => thread.workingSince > 0), probes: SIDEBAR_PROBES,
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
      navigateKind: session.navigate.kind, navigateProject: session.navigate.projectId,
      renameId: session.renameId, renameTitle: session.renameTitle, selectionCount: session.selection.length,
      undoText: undo && live ? `${undo.action} ${undo.threadIds.length} ${undo.action === 'Discarded' ? 'draft' : 'thread'}${undo.threadIds.length === 1 ? '' : 's'},` : '', // r11-upstream: drafts (95edeb753b)
      undoUntil: undo && live ? undo.at + 5000 : 0, // lane r8-keys: the window hides the notice at this wall time (showThreadUndoNotice's 5 s)
      undoLabel: shortcutLabel(client, 'thread.undo') ? `${shortcutLabel(client, 'thread.undo')} to undo` : 'Undo',
      dialog: session.dialog.kind, dialogTitle: dialogTitle(session.dialog), dialogDescription: dialogDescription(session.dialog),
      dialogMode: session.dialogMode, dialogDate: session.dialogDate, dialogTime: session.dialogTime, dialogAmount: session.dialogAmount, dialogUnit: session.dialogUnit, dialogError: session.dialogError,
      jumpHints: hints,
      pillKey: pill?.key ?? '', pillTone: pill?.tone ?? '', pillTitle: pill?.title ?? '', pillDescription: pill?.description ?? '', pillDismissible: pill?.dismissible ?? false,
      sweepEpoch: session.sweepEpoch, // lane r11-upstream (1826fb55cc)
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
function dialogDescription(dialog: { kind: string; threadIds: string[]; title: string }): string {
  if (dialog.kind === 'delete-worktree') return worktreeDialogDescription(dialog.title);
  if (dialog.kind === 'delete') return 'This permanently clears conversation history for this thread.';
  if (dialog.kind === 'delete-many') return 'This permanently clears conversation history for these threads.';
  if (dialog.kind === 'unpin') return 'This will move the thread out of your pinned section.';
  return '';
}

/** A project group's glyph: its representative member under the group's name (the scoped logical project). */
function groupGlyph(client: T3Client, group: { name: string; members: Obj[] } | undefined, identity: Identity): ProjectGlyph {
  return projectGlyph(client, group ? { ...(group.members[0] ?? {}), title: group.name } : { title: '' }, identity);
}

/** filterSidebarProjectScopeItems: a query hides "All projects" and keeps matching groups. */
function scopeItems(client: T3Client, query: string, selected: string, scopes: { key: string; name: string; members: Obj[] }[], identity: Identity): SidebarScope[] {
  const needle = query.trim().toLowerCase();
  const items: SidebarScope[] = [{ key: '', label: 'All projects', mark: '', ink: '', surface: '', selected: selected === '', all: true, glyph: groupGlyph(client, undefined, identity) },
    ...scopes.map(group => { const mark = identity(group.name); return { key: group.key, label: group.name, mark: mark.projectMark, ink: mark.projectInk, surface: mark.projectSurface, selected: group.key === selected, all: false, glyph: groupGlyph(client, group, identity) }; })];
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
    rows.push({ id, projectName: name, mark: mark.projectMark, ink: mark.projectInk, surface: mark.projectSurface, glyph: projectGlyph(client, project, identity),
      preview: text ? text.split('\n', 1)[0]! : `${attachments} attachment${attachments === 1 ? '' : 's'}` });
  }
  return rows;
}

/** SidebarThreadTooltip: environment, model · instance, hand-offs and the last error. */
function hoverDetails(client: T3Client, thread: Obj, providers: Obj[], instance: Obj | undefined, helpers: SidebarHelpers) {
  const kind = str(thread.fleetMachine) || machineKind(client.config), selection = obj(thread.modelSelection);
  const model = arr(instance?.models).find(entry => entry.slug === selection.model);
  const modelLabel = str(model?.name, str(selection.model));
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
