// @ref llp/1107.004-home-projection.decision.md#decision
// Upstream 365aa87982 use-thread-list-v2-shelf-preferences; mobile-owned persistence.
// @ref llp/1107.000-mobile-app-layout.decision.md#shared-typescript
import { bridgeReply, type Native } from './shared/protocol';
import { obj } from './shared/domain';
import { mobileHome } from './home';
import { mobileHomeActionsObserve, mobileHomeMenu } from './home-actions';

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
export function mobileHomeView(args: unknown[]) {
  const [_revision, now, query, settledCount, loaded, workingEnabled, workingExpanded, snoozedExpanded, settledExpanded, environmentId, projectKey, selectedThreadKey, groupingMode, requestRoute, homeVisible, sidebarVisible] = args;
  mobileHomeActionsObserve(String(requestRoute ?? ''), homeVisible === true, sidebarVisible === true);
  const result = mobileHome(Number(now), { query: String(query ?? ''), settledVisibleCount: Number(settledCount) || 10,
    environmentId: String(environmentId ?? ''), projectKey: String(projectKey ?? ''), selectedThreadKey: String(selectedThreadKey ?? ''),
    groupingMode: String(groupingMode ?? 'repository'), preferencesLoaded: loaded === true, workingEnabled: workingEnabled === true, workingExpanded: workingExpanded === true,
    snoozedExpanded: snoozedExpanded === true, settledExpanded: settledExpanded === true });
  for (const item of result.items) if (item.kind === 'thread') item.menuItems = mobileHomeMenu(item.environmentId, item.threadId, Number(now));
  return { items: result.items, emptyTitle: result.emptyTitle, emptyDetail: result.emptyDetail,
    loading: result.loading, addEnvironment: result.addEnvironment };
}
