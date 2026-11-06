// @ref examples/t3-code/mobile/docs/1130.004-home-projection.decision.md#decision
// Upstream 365aa87982 use-thread-list-v2-shelf-preferences; mobile-owned persistence.
// @ref examples/t3-code/mobile/docs/1130.000-mobile-app-layout.decision.md#shared-typescript
import { bridgeReply, type Native } from './shared/protocol';
import { obj } from './shared/domain';
import { mobileHome, mobileHomeSources } from './home';
import { mobileHomeOrder } from './home-order';
import { homeArrangeSnapshot } from './home-arrange';
import { mobileClient } from './client';
import { mobileNewTaskDraftList, mobileNewTaskDraftPresentation } from './mobile-new-task-drafts';
import type { T3Client } from './shared/client';
import { liveEnvironments } from './shared/live-streams';
import { mobileHomeSearchPending } from './mobile-home-search';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import { mobileHomeActionsObserve, mobileHomeRowProjection } from './home-actions';

type HomeView = {
  nextSwipeRefreshAt: number;
  items: ReturnType<typeof mobileHome>['items'];
  emptyTitle: string;
  emptyDetail: string;
  loading: boolean;
  addEnvironment: boolean;
  fullPageEmpty: boolean;
};

interface HomeViewCache {
  background: EnvironmentFleet;
  backgroundRevision: number;
  saved: unknown;
  revision: number;
  threadEpoch: number;
  threadId: string;
  argsRevision: unknown;
  nowSecond: number;
  query: string;
  settledCount: number;
  loaded: boolean;
  workingEnabled: boolean;
  workingExpanded: boolean;
  snoozedExpanded: boolean;
  settledExpanded: boolean;
  environmentId: string;
  projectKey: string;
  selectedThreadKey: string;
  groupingMode: string;
  requestRoute: string;
  homeVisible: boolean;
  sidebarVisible: boolean;
  environments: unknown;
  surface: string;
  catalogLoaded: boolean;
  searchPending: boolean;
  view: HomeView;
}

const homeViewCache = new WeakMap<T3Client, HomeViewCache>();

export const emptyShelves = { loaded: false, workingEnabled: false, workingExpanded: false,
  snoozedExpanded: false, settledExpanded: false };

export async function mobileShelves(native?: Native | null) {
  if (!native?.available) return emptyShelves;
  const reply = await bridgeReply(native, { op: 'mobileHomePreferences' });
  if (!reply.ok) throw new Error(reply.error!.message);
  const value = obj(reply.value);
  return { loaded: true, workingEnabled: value.workingEnabled === true,
    workingExpanded: value.workingExpanded === true, snoozedExpanded: value.snoozedExpanded === true,
    settledExpanded: value.settledExpanded === true };
}

export async function mobileToggleShelf(section: string, native?: Native | null) {
  if (!native?.available) return { revision: 0, message: 'Open T3 Code on your iPhone or iPad.' };
  if (!['working', 'snoozed', 'settled'].includes(section)) return { revision: 0, message: 'Unknown thread shelf.' };
  const reply = await bridgeReply(native, { op: 'mobileToggleShelf', section });
  return { revision: 0, message: reply.ok ? '' : reply.error!.message };
}

export function mobileHomeArrangeView(active: boolean, now: number, workingEnabled: boolean,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet) {
  if (!active) return { version: '', blocked: true, workingEnabled, pinned: [], active: [], snoozed: [], settled: [] };
  const sources = mobileHomeSources(client, background);
  const order = mobileHomeOrder(client, sources, now, { workingEnabled, observeReturns: true });
  return homeArrangeSnapshot(client, sources, now, order);
}

/** Select only the declared Contract shape; the full projection also has internal counts. */
export function mobileHomeView(args: unknown[], client: T3Client = mobileClient, background: EnvironmentFleet = fleet) {
  const [_revision, now, query, settledCount, loaded, workingEnabled, workingExpanded, snoozedExpanded, settledExpanded, environmentId, projectKey, selectedThreadKey, groupingMode, requestRoute, homeVisible, sidebarVisible, environments, surface, catalogLoaded] = args;
  mobileHomeActionsObserve(String(requestRoute ?? ''), homeVisible === true, sidebarVisible === true, client);
  const normalized = {
    argsRevision: _revision,
    nowSecond: Math.floor(Number(now) / 1000),
    query: String(query ?? ''),
    settledCount: Number(settledCount) || 10,
    loaded: loaded === true,
    workingEnabled: workingEnabled === true,
    workingExpanded: workingExpanded === true,
    snoozedExpanded: snoozedExpanded === true,
    settledExpanded: settledExpanded === true,
    environmentId: String(environmentId ?? ''),
    projectKey: String(projectKey ?? ''),
    selectedThreadKey: String(selectedThreadKey ?? ''),
    groupingMode: String(groupingMode ?? 'repository'),
    requestRoute: String(requestRoute ?? ''),
    homeVisible: homeVisible === true,
    sidebarVisible: sidebarVisible === true,
    environments,
    surface: String(surface ?? ''),
    catalogLoaded: catalogLoaded !== false,
    searchPending: mobileHomeSearchPending(client),
  };
  const previous = homeViewCache.get(client);
  // Scroll redraws carry a fresh frame clock but no canonical state change. The
  // client and fleet revisions are the same ownership boundary used by Home's
  // action projections, so an unchanged revision can safely reuse every row.
  // Revision zero is fixture/setup state: callers may still mutate those objects
  // in place before the first snapshot, so never retain it.
  if (client.revision > 0 && previous && previous.background === background
    && previous.backgroundRevision === background.revision && previous.saved === background.saved
    && previous.revision === client.revision && previous.threadEpoch === client.threadEpoch && previous.threadId === client.threadId
    && previous.argsRevision === normalized.argsRevision && previous.nowSecond === normalized.nowSecond
    && previous.query === normalized.query && previous.settledCount === normalized.settledCount
    && previous.loaded === normalized.loaded && previous.workingEnabled === normalized.workingEnabled
    && previous.workingExpanded === normalized.workingExpanded && previous.snoozedExpanded === normalized.snoozedExpanded
    && previous.settledExpanded === normalized.settledExpanded && previous.environmentId === normalized.environmentId
    && previous.projectKey === normalized.projectKey && previous.selectedThreadKey === normalized.selectedThreadKey
    && previous.groupingMode === normalized.groupingMode && previous.requestRoute === normalized.requestRoute
    && previous.homeVisible === normalized.homeVisible && previous.sidebarVisible === normalized.sidebarVisible
    && previous.environments === normalized.environments && previous.surface === normalized.surface
    && previous.catalogLoaded === normalized.catalogLoaded && previous.searchPending === normalized.searchPending) return previous.view;
  const order = mobileHomeOrder(client, mobileHomeSources(client, background), Number(now), { workingEnabled: workingEnabled === true, observeReturns: true });
  const drafts = mobileNewTaskDraftList(client).flatMap(record => { const draft = mobileNewTaskDraftPresentation(client, record.key); return draft ? [draft] : []; });
  const draftEnvironments = (Array.isArray(environments) ? environments : []).map(value => { const row = obj(value);
    return { environmentId: String(row.environmentId ?? ''), label: String(row.label ?? ''), machineSymbol: String(row.machineSymbol ?? '') }; });
  const result = mobileHome(Number(now), { drafts, draftEnvironments, orderSnapshot: order, query: normalized.query, settledVisibleCount: normalized.settledCount,
    environmentId: normalized.environmentId, projectKey: normalized.projectKey, selectedThreadKey: normalized.selectedThreadKey,
    groupingMode: normalized.groupingMode, catalogLoaded: normalized.catalogLoaded, preferencesLoaded: normalized.loaded, workingEnabled: normalized.workingEnabled, workingExpanded: normalized.workingExpanded,
    snoozedExpanded: normalized.snoozedExpanded, settledExpanded: normalized.settledExpanded }, client, background);
  const live = liveEnvironments(client, null, background);
  const contexts = new Map(live.map(environment => {
    const endpoint = environment.focused ? client : background.entries.get(environment.key);
    return [environment.environmentId, { origin: endpoint?.origin ?? '', generation: endpoint?.generation ?? 0, connected: environment.connected }] as const;
  }));
  const rowProjection = mobileHomeRowProjection(live, Number(now), order, client, background);
  for (const item of result.items) if (item.kind === 'thread') {
    item.swipe = rowProjection.swipe(item.environmentId, item.threadId);
    item.menuItems = rowProjection.menu(item.environmentId, item.threadId);
    const recovery = rowProjection.recoveryGeneration(item.environmentId, item.threadId);
    item.dismissalIdentity = encodeURIComponent(JSON.stringify([requestRoute, item.environmentId, item.threadId,
      contexts.get(item.environmentId)?.origin ?? '', contexts.get(item.environmentId)?.generation ?? 0, item.swipe.resetKey, ...(recovery ? [recovery] : [])]));
    item.nativeMenu = JSON.stringify({ identity: item.key, requestRoute: String(requestRoute ?? ''), enabled: sidebarVisible === true, items: item.menuItems, swipeItems: item.swipe.snoozeItems, swipeSnoozable: item.swipe.snoozable, swipeResetKey: item.swipe.resetKey,
      environmentId: item.environmentId, threadId: item.threadId, ...contexts.get(item.environmentId),
      homeVisible: homeVisible === true, sidebarVisible: sidebarVisible === true });
  }
  for (const item of result.items) if (item.kind === 'draft') item.nativeMenu = JSON.stringify({ identity: item.key,
    requestRoute: String(requestRoute ?? ''), enabled: sidebarVisible === true, items: item.menuItems });
  const nextSwipeRefreshAt = result.items.reduce((earliest, item) => {
    const at = item.swipe.gateExpiry > 0 ? item.swipe.gateExpiry + 50 : 0;
    return at > Number(now) && (earliest === 0 || at < earliest) ? at : earliest;
  }, 0);
  // ThreadNavigationSidebar has a plain list label, separate from HomeScreen's card.
  const sidebar = surface === 'sidebar', needle = String(query ?? '').trim();
  const project = result.projects.find(project => project.key === projectKey || project.projectKeys.includes(String(projectKey ?? '')));
  const searching = needle === client.query.trim() && normalized.searchPending
    && live.some(environment => environment.connected && (!environmentId || environment.environmentId === environmentId));
  const sidebarEmpty = result.items.length ? '' : catalogLoaded === false ? 'Loading threads…'
    : needle ? searching ? 'Searching thread messages…' : 'No matching threads'
    : project ? `No threads in ${project.title}` : 'No threads yet';
  const view = { nextSwipeRefreshAt, items: result.items, emptyTitle: result.emptyTitle, emptyDetail: result.emptyDetail,
    ...(sidebar ? { emptyTitle: sidebarEmpty, emptyDetail: '', loading: false, addEnvironment: false, fullPageEmpty: false }
      : { loading: result.loading, addEnvironment: result.addEnvironment, fullPageEmpty: result.fullPageEmpty }) };
  if (client.revision > 0) homeViewCache.set(client, { background, backgroundRevision: background.revision, saved: background.saved,
    revision: client.revision, threadEpoch: client.threadEpoch, threadId: client.threadId, view, ...normalized });
  return view;
}
