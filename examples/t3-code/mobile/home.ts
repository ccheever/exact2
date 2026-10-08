import { blankHomeSwipe, type HomeSwipeData } from './home-swipe';
// @ref llp/1107.004-home-projection.decision.md#decision
// Mobile HomeScreen/threadListV2 at upstream 365aa87982; projection over the shared V2 shell.
// @ref llp/1107.000-mobile-app-layout.decision.md#shared-typescript
// @ref llp/1107.002-design-system-parity.spec.md#typography-and-font-assets
import { mobileClient } from './client';
import { homeApplyPending, mobileHomeOrder, type HomePendingOrder, type HomeOrderSnapshot } from './home-order';
import type { HomeMenuItem } from './home-actions';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj, type Shell } from './shared/domain';
import { fleet, fleetThreadId, type EnvironmentFleet } from './shared/settings-b-fleet';
import { machineKind } from './shared/connections';
import { groupLabel, logicalKey } from './shared/r6-polish-groups';
import { serverMatches, searchMatch } from './shared/sidebar-presentation';
import { ICON_COLORS, projectIdentity } from './shared/settings-b-icons';
import { faviconSrc } from './shared/r3-sidebar-glyph';
import { capabilities, effectiveSnoozed, isWorkingThread, lastVisited, unseenCompletion,
  sidebarStatus, sidebarVisible, snoozeWakeLabel, sortActive, sortByReturn, sortPinned,
  sortSettled, sortSnoozed, sortWorkingThreadsBySend, settledTimestamp, type SidebarSection } from './shared/sidebar-model';

export interface HomeOptions {
  returnedAt?: Readonly<Record<string, number>>;
  pendingOrder?: HomePendingOrder | null;
  orderSnapshot?: HomeOrderSnapshot;
  query?: string; environmentId?: string; projectKey?: string; groupingMode?: string;
  projectSortOrder?: 'created_at' | 'updated_at'; selectedThreadKey?: string;
  workingEnabled?: boolean; workingExpanded?: boolean; snoozedExpanded?: boolean; settledExpanded?: boolean;
  settledVisibleCount?: number; preferencesLoaded?: boolean; catalogLoaded?: boolean;
  /** Mobile outbox ownership stays at the command seam; keys are environmentId:threadId. */
  queuedThreadKeys?: ReadonlySet<string>;
  /** Message-search results must be scoped by environment, never just a server-local thread id. */
  messageMatches?: ReadonlyMap<string, Obj>;
}
export interface HomeSource { environmentId: string; label: string; machine: string; config: Obj; shell: Shell; focused: boolean; origin?: string; connected?: boolean }
export interface HomeProject { key: string; title: string; projectKeys: string[]; environmentId: string; projectId: string }
export interface HomeItem {
  key: string; kind: string; id: string; environmentId: string; threadId: string; section: string; title: string;
  projectTitle: string; projectPresent: boolean; branch: string; environmentLabel: string; machineSymbol: string;
  status: string; statusTone: string; time: string; error: string; card: boolean; pinned: boolean; queued: boolean;
  expanded: boolean; disabled: boolean; count: number; last: boolean; trailingDivider: boolean; selected: boolean;
  favicon: string; iconKind: string; iconText: string; iconColor: string; iconSurface: string; iconSize: number;
  searchExcerpt: string; swipe: HomeSwipeData; menuItems: HomeMenuItem[]; nativeMenu: string;
}
const blankItem = (key: string): HomeItem => ({ key, kind: 'thread', id: '', environmentId: '', threadId: '', section: '', title: '',
  projectTitle: '', projectPresent: false, branch: '', environmentLabel: '', machineSymbol: '', status: '', statusTone: '',
  time: '', error: '', card: false, pinned: false, queued: false, expanded: false, disabled: false, count: 0,
  last: false, trailingDivider: false, selected: false, favicon: '', iconKind: '', iconText: '', iconColor: '', iconSurface: '', iconSize: 0, searchExcerpt: '', swipe: blankHomeSwipe(), menuItems: [], nativeMenu: '' });
const scoped = (environmentId: string, id: unknown) => `${environmentId}:${str(id)}`;
const timestamp = (value: unknown) => { const stamp = Date.parse(str(value)); return Number.isFinite(stamp) ? stamp : -Infinity; };
const machineSymbols: Record<string, string> = { server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer', laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio' };

/** Mobile's elapsed label differs from desktop only below one minute (<1m). */
export function mobileRelativeTime(value: unknown, now: number): string {
  const stamp = timestamp(value), seconds = Math.max(0, Math.floor((now - stamp) / 1000));
  if (!Number.isFinite(stamp) || seconds < 60) return '<1m';
  const minutes = Math.floor(seconds / 60);
  return minutes < 60 ? `${minutes}m` : minutes < 1440 ? `${Math.floor(minutes / 60)}h` : `${Math.floor(minutes / 1440)}d`;
}

/** Only synchronized shells contribute live rows; focused cached shell remains visible on reconnect. */
export function mobileHomeSources(client: T3Client = mobileClient, background: EnvironmentFleet = fleet): HomeSource[] {
  const sources: HomeSource[] = [];
  if (client.environmentId && client.shellLoaded) sources.push({ environmentId: client.environmentId,
    label: str(obj(client.config.environment).label), machine: machineKind(client.config), config: client.config, shell: client.shell, focused: true, origin: client.origin, connected: client.ready });
  for (const entry of background.entries.values()) {
    if (entry.environmentId === client.environmentId || entry.phase !== 'connected' || entry.synchronized !== entry.generation) continue;
    sources.push({ environmentId: entry.environmentId, label: str(obj(entry.config.environment).label), machine: machineKind(entry.config),
      config: entry.config, shell: entry.shell, focused: false, origin: entry.origin, connected: true });
  }
  return sources;
}

/** Cross-environment groups use the same logical keys/labels as desktop; mobile has no per-checkout overrides. */
export function mobileHomeProjects(sources: HomeSource[], options: HomeOptions): HomeProject[] {
  const grouping = { environmentId: '', shell: { projects: [] }, local: { groupingMode: options.groupingMode ?? 'repository', groupingOverrides: {} } };
  const groups = new Map<string, { project: Obj; source: HomeSource }[]>();
  for (const source of sources) {
    if (options.environmentId && source.environmentId !== options.environmentId) continue;
    for (const project of source.shell.projects) {
      const key = logicalKey(grouping, source.environmentId, project), members = groups.get(key) ?? [];
      members.push({ project, source }); groups.set(key, members);
    }
  }
  const activity = (members: { project: Obj; source: HomeSource }[]) => {
    const threadTimes = members.flatMap(({ project, source }) => source.shell.threads
      .filter(thread => thread.archivedAt == null && thread.projectId === project.id)
      .map(thread => options.projectSortOrder === 'created_at' ? timestamp(thread.createdAt ?? thread.updatedAt)
        : timestamp(thread.latestUserMessageAt ?? thread.updatedAt ?? thread.createdAt)));
    return threadTimes.length ? Math.max(...threadTimes) : Math.max(...members.map(({ project }) => timestamp(
      options.projectSortOrder === 'created_at' ? project.createdAt : project.updatedAt ?? project.createdAt)));
  };
  return [...groups].map(([key, members]) => ({ key, title: groupLabel(members.map(member => member.project)),
    projectKeys: members.map(({ source, project }) => scoped(source.environmentId, project.id)),
    environmentId: members[0]!.source.environmentId, projectId: str(members[0]!.project.id), time: activity(members) }))
    .sort((a, b) => (a.time === b.time ? 0 : a.time > b.time ? -1 : 1) || a.title.localeCompare(b.title) || a.key.localeCompare(b.key))
    .map(({ time: _time, ...project }) => project);
}

export interface HomeCatalogState {
  loading: boolean; hasConnections: boolean; hasReadyEnvironment: boolean; hasLoadedShell: boolean;
  connecting: boolean; connectionState: string; error: string;
}
/** Literal HomeScreen deriveEmptyState copy, with the transport catalog's facts supplied by the caller. */
export function mobileHomeEmpty(catalog: HomeCatalogState, projectCount: number) {
  if (catalog.loading) return { title: 'Loading environments', detail: 'Checking saved environments on this device.', loading: true };
  if (!catalog.hasConnections) return { title: 'No environments connected', detail: 'Add an environment to load projects and start coding sessions.', loading: false };
  if (['available', 'offline', 'error', 'unsupported'].includes(catalog.connectionState) && !catalog.hasLoadedShell) return {
    title: catalog.connectionState === 'unsupported' ? 'Client not supported' : 'Environment unavailable',
    detail: catalog.error || 'The saved environment is offline. Check the URL or start the environment, then retry.', loading: false };
  if (catalog.connecting && !catalog.hasLoadedShell && !catalog.error) return { title: 'Connecting to environment',
    detail: 'Loading projects and threads from the saved environment.', loading: true };
  if (!projectCount && catalog.hasLoadedShell) return { title: 'No projects found', detail: 'The connected environment did not report any projects.', loading: false };
  return { title: 'No threads yet', detail: 'Create a task to start a new coding runtime in one of your connected projects.', loading: false };
}

/** Pure mobile shelf projection. No shared shell objects or persistence are mutated. */
export function projectMobileHome(sources: HomeSource[], now: number, options: HomeOptions = {}) {
  const projects = mobileHomeProjects(sources, options), selectedProject = projects.find(project => project.key === options.projectKey || project.projectKeys.includes(options.projectKey ?? ''));
  const titles = new Map(projects.flatMap(project => project.projectKeys.map(key => [key, project.title] as const)));
  const parts: Record<SidebarSection, Obj[]> = { pinned: [], active: [], working: [], snoozed: [], settled: [] };
  const ownership = new Map<Obj, HomeSource>(), query = options.query?.trim().toLowerCase() ?? '';
  let nextSnoozeWakeAt: number | null = null;
  for (const source of sources) for (const raw of source.shell.threads) {
    if (!sidebarVisible(raw) || options.environmentId && source.environmentId !== options.environmentId
      || selectedProject && !selectedProject.projectKeys.includes(scoped(source.environmentId, raw.projectId))) continue;
    const thread: Obj = { ...raw, environmentId: source.environmentId };
    const key = scoped(source.environmentId, thread.id);
    const prs = arr(thread.pullRequests);
    if (query && !str(thread.title).toLowerCase().includes(query)
      && !prs.some(pr => `#${String(pr.number ?? "")}`.includes(query) || str(pr.title).toLowerCase().includes(query))
      && !options.messageMatches?.has(key)) continue;
    ownership.set(thread, source);
    const caps = capabilities(source.config);
    const snoozed = caps.snooze && effectiveSnoozed(thread, now);
    if (snoozed) { const wake = timestamp(thread.snoozedUntil); nextSnoozeWakeAt = Math.min(nextSnoozeWakeAt ?? Infinity, wake); }
    const section: SidebarSection = snoozed ? 'snoozed'
      : caps.settlement && thread.settledOverride === 'settled' && !options.queuedThreadKeys?.has(key) ? 'settled'
      : thread.pinnedAt != null ? 'pinned' : options.workingEnabled && isWorkingThread(thread) ? 'working' : 'active';
    parts[section].push(thread);
  }
  // Shared sort helpers tie on raw thread id; stable input supplies mobile's environment-id tie.
  Object.values(parts).forEach(rows => rows.sort((left, right) => str(left.environmentId).localeCompare(str(right.environmentId))));
  parts.pinned = sortPinned(parts.pinned); parts.active = options.workingEnabled ? sortByReturn(parts.active, thread => options.returnedAt?.[scoped(str(thread.environmentId), thread.id)]) : sortActive(parts.active);
  parts.pinned = homeApplyPending(parts.pinned, 'pinned', options.pendingOrder ?? null);
  if (!options.workingEnabled) parts.active = homeApplyPending(parts.active, 'active', options.pendingOrder ?? null);
  parts.working = sortWorkingThreadsBySend(parts.working); parts.snoozed = sortSnoozed(parts.snoozed); parts.settled = sortSettled(parts.settled);
  const items: HomeItem[] = [], isSelected = (thread: Obj) => scoped(str(thread.environmentId), thread.id) === options.selectedThreadKey;
  const append = (thread: Obj, section: SidebarSection) => {
    const source = ownership.get(thread)!, key = scoped(source.environmentId, thread.id), project = source.shell.projects.find(project => project.id === thread.projectId);
    const status = sidebarStatus(thread), card = !['settled', 'snoozed'].includes(section);
    const unread = status === 'ready' && unseenCompletion(thread, lastVisited(thread, undefined));
    const statusLabel = ({ approval: 'Approval', input: 'Input', working: obj(thread.goal).status === 'active' ? 'Goal' : 'Working', failed: 'Failed', limited: 'Limited' } as Record<string, string>)[status] ?? (unread ? 'Done' : '');
    const icon = obj(project?.projectIcon), match = options.messageMatches?.get(key);
    const iconKind = icon.kind === 'lucide' ? 'monogram' : str(icon.kind);
    const iconText = icon.kind === 'lucide' ? projectIdentity(str(project?.title).normalize('NFKC')).monogram : str(icon.emoji ?? icon.text);
    const iconColor = ICON_COLORS.find(color => color.value === icon.color)?.swatch ?? '';
    const glyphCount = Array.from(iconText.replace(/\p{M}/gu, '')).length;
    items.push({ ...blankItem(key), id: source.focused ? str(thread.id) : fleetThreadId(source.environmentId, str(thread.id)),
      environmentId: source.environmentId, threadId: str(thread.id), section, title: str(thread.title),
      projectTitle: titles.get(scoped(source.environmentId, thread.projectId)) ?? str(project?.title), projectPresent: !!project,
      branch: str(thread.branch), environmentLabel: sources.length > 1 ? source.label : '', machineSymbol: machineSymbols[source.machine] ?? machineSymbols.server!,
      status: statusLabel, statusTone: unread ? 'done' : status,
      time: section === 'snoozed' ? snoozeWakeLabel(thread.snoozedUntil, now) : card && (status !== 'ready' || unread) ? ''
        : mobileRelativeTime((section === 'settled' ? settledTimestamp(thread) : null) ?? thread.latestUserMessageAt ?? thread.updatedAt ?? thread.createdAt, now),
      error: status === 'failed' || status === 'limited' ? str(thread.lastError) : '', card, pinned: section === 'pinned',
      queued: options.queuedThreadKeys?.has(key) === true, selected: isSelected(thread),
      iconKind, iconText, iconColor, iconSurface: iconColor ? `${iconColor}26` : '', iconSize: 15 * (glyphCount === 1 ? 0.6 : 0.515625),
      searchExcerpt: searchMatch(thread, options.query ?? '', match).matchParts.map(part => part.text).join('') });
  };
  parts.pinned.forEach(thread => append(thread, 'pinned')); parts.active.forEach(thread => append(thread, 'active'));
  let hiddenSettledCount = 0;
  for (const section of ['working', 'snoozed', 'settled'] as const) {
    const rows = parts[section]; if (!rows.length) continue;
    const expanded = options[`${section}Expanded`] === true;
    const label = section[0]!.toUpperCase() + section.slice(1);
    items.push({ ...blankItem(`shelf:${section}`), kind: 'shelf', section, title: expanded ? label : `${label} (${rows.length})`,
      count: rows.length, expanded, disabled: options.preferencesLoaded === false });
    let page = rows;
    if (section === 'settled') {
      page = rows.slice(0, Math.max(0, options.settledVisibleCount ?? 10));
      const selected = rows.slice(page.length).find(isSelected); if (selected) page.push(selected);
      hiddenSettledCount = rows.length - page.length;
    }
    (expanded ? page : page.filter(isSelected)).forEach(thread => append(thread, section));
    if (section === 'settled' && expanded && hiddenSettledCount) items.push({ ...blankItem('settled:more'), kind: 'more', section,
      title: `Show more (${hiddenSettledCount} settled hidden)`, count: hiddenSettledCount });
  }
  items.forEach((item, index) => { item.last = index === items.length - 1; item.trailingDivider = item.kind === 'thread' && items[index + 1]?.kind === 'thread'; });
  const selectedEnvironment = sources.find(source => source.environmentId === options.environmentId);
  const empty = query ? { title: 'No results', detail: `No threads matching "${options.query?.trim()}".` }
    : selectedProject ? { title: `No threads in ${selectedProject.title}`, detail: 'Choose another project or create a new task.' }
    : options.environmentId ? { title: `No threads in ${selectedEnvironment?.label || 'this environment'}`, detail: 'Choose another environment or create a new task.' }
    : { title: 'No threads yet', detail: 'Create a task to start a new coding session.' };
  return { items, projects, counts: { pinned: parts.pinned.length, active: parts.active.length, working: parts.working.length,
    snoozed: parts.snoozed.length, settled: parts.settled.length }, hiddenSettledCount, nextSnoozeWakeAt,
    emptyTitle: items.length ? '' : empty.title, emptyDetail: items.length ? '' : empty.detail };
}

/** Call after mobileSnapshot refresh. Root owns watches, timer, navigation, shelf persistence, and search RPC. */
export function mobileHome(now: number, options: HomeOptions = {}, client: T3Client = mobileClient, background: EnvironmentFleet = fleet) {
  const sources = mobileHomeSources(client, background);
  const matches = options.messageMatches ?? new Map((options.query?.trim() === client.query.trim() ? [...serverMatches(client)] : []).map(([id, match]) => [scoped(client.environmentId, id), match]));
  const order = options.orderSnapshot ?? mobileHomeOrder(client, sources, now, { ...options, observeReturns: true });
  const projection = projectMobileHome(sources, now, { ...options, returnedAt: order.returnedAt, messageMatches: matches, pendingOrder: order.pending, queuedThreadKeys: order.queuedThreadKeys });
  projection.items.forEach(item => {
    if (item.kind !== 'thread' || item.environmentId !== client.environmentId) return;
    const raw = client.shell.threads.find(thread => thread.id === item.threadId);
    const project = client.shell.projects.find(project => project.id === raw?.projectId);
    if (project) item.favicon = faviconSrc(client, project);
  });
  const entries = [...background.entries.values()], state = client.connection === 'disconnected' ? 'available' : client.connection;
  const catalog: HomeCatalogState = { loading: options.catalogLoaded === false,
    hasConnections: background.saved.length > 0 || !!client.environmentId, hasReadyEnvironment: client.ready || sources.some(source => !source.focused),
    hasLoadedShell: sources.length > 0, connecting: ['connecting', 'reconnecting'].includes(state) || entries.some(entry => ['connecting', 'reconnecting'].includes(entry.phase)),
    connectionState: state, error: client.error || entries.find(entry => entry.error)?.error || '' };
  const anyThreads = sources.some(source => source.shell.threads.some(thread => thread.archivedAt == null && thread.deletedAt == null));
  const empty = anyThreads ? null : mobileHomeEmpty(catalog, sources.reduce((sum, source) => sum + source.shell.projects.length, 0));
  return { ...projection, emptyTitle: empty?.title ?? projection.emptyTitle, emptyDetail: empty?.detail ?? projection.emptyDetail,
    loading: empty?.loading ?? false, addEnvironment: !anyThreads && !catalog.hasReadyEnvironment, hasAnyThreads: anyThreads };
}
