// @ref llp/1109.004-home-projection.decision.md#decision
// Upstream 365aa87982 use-thread-list-v2-shelf-preferences; mobile-owned persistence.
// @ref llp/1109.000-mobile-app-layout.decision.md#shared-typescript
import { bridgeReply, type Native } from './shared/protocol';
import { obj } from './shared/domain';
import { mobileHome, mobileHomeSources } from './home';
import { mobileHomeOrder } from './home-order';
import { homeArrangeSnapshot } from './home-arrange';
import { mobileClient } from './client';
import { mobileNewTaskDraftList, mobileNewTaskDraftPresentation } from './mobile-new-task-drafts';
import type { T3Client } from './shared/client';
import { liveEnvironments } from './shared/live-streams';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import { mobileHomeActionsObserve, mobileHomeMenu, mobileHomeSwipe } from './home-actions';

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

/** Select only the declared Contract shape; the full projection also has internal counts. */
export function mobileHomeView(args: unknown[], client: T3Client = mobileClient, background: EnvironmentFleet = fleet) {
  const [_revision, now, query, settledCount, loaded, workingEnabled, workingExpanded, snoozedExpanded, settledExpanded, environmentId, projectKey, selectedThreadKey, groupingMode, requestRoute, homeVisible, sidebarVisible, environments] = args;
  mobileHomeActionsObserve(String(requestRoute ?? ''), homeVisible === true, sidebarVisible === true, client);
  const order = mobileHomeOrder(client, mobileHomeSources(client, background), Number(now), { workingEnabled: workingEnabled === true, observeReturns: true });
  const drafts = mobileNewTaskDraftList(client).flatMap(record => { const draft = mobileNewTaskDraftPresentation(client, record.key); return draft ? [draft] : []; });
  const draftEnvironments = (Array.isArray(environments) ? environments : []).map(value => { const row = obj(value);
    return { environmentId: String(row.environmentId ?? ''), label: String(row.label ?? ''), machineSymbol: String(row.machineSymbol ?? '') }; });
  const result = mobileHome(Number(now), { drafts, draftEnvironments, orderSnapshot: order, query: String(query ?? ''), settledVisibleCount: Number(settledCount) || 10,
    environmentId: String(environmentId ?? ''), projectKey: String(projectKey ?? ''), selectedThreadKey: String(selectedThreadKey ?? ''),
    groupingMode: String(groupingMode ?? 'repository'), preferencesLoaded: loaded === true, workingEnabled: workingEnabled === true, workingExpanded: workingExpanded === true,
    snoozedExpanded: snoozedExpanded === true, settledExpanded: settledExpanded === true }, client, background);
  const contexts = new Map(liveEnvironments(client, null, background).map(environment => {
    const endpoint = environment.focused ? client : background.entries.get(environment.key);
    return [environment.environmentId, { origin: endpoint?.origin ?? '', generation: endpoint?.generation ?? 0, connected: environment.connected }] as const;
  }));
  for (const item of result.items) if (item.kind === 'thread') {
    item.swipe = mobileHomeSwipe(item.environmentId, item.threadId, Number(now), client, background, order);
    item.menuItems = mobileHomeMenu(item.environmentId, item.threadId, Number(now), client, background, order);
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
  return { nextSwipeRefreshAt, arrangement: homeArrangeSnapshot(client, mobileHomeSources(client, background), Number(now), order), items: result.items, emptyTitle: result.emptyTitle, emptyDetail: result.emptyDetail,
    loading: result.loading, addEnvironment: result.addEnvironment };
}
