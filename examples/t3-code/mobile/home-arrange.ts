// @ref llp/1107.004-home-projection.decision.md#arrange-threads
// Pinned ThreadArrangementSheet.tsx and threadOrder.ts. Pure projection over the existing order owner.
import { obj, str, type Obj } from './shared/domain';
import { capabilities, effectiveSnoozed, sortByReturn } from './shared/sidebar-model';
import { homeApplyPending, homeMovePlan, homeOrderKey, mobileHomeOrder, type HomeDropDestination, type HomeOrderSnapshot, type HomeOrderSource } from './home-order';

export interface HomeArrangementRow {
  key: string; environmentId: string; threadId: string; title: string; disabled: boolean;
  canMoveUp: boolean; canMoveDown: boolean; canPin: boolean; canActivate: boolean; canSettle: boolean;
}
export interface HomeArrangementSnapshot {
  version: string; blocked: boolean; workingEnabled: boolean;
  pinned: HomeArrangementRow[]; active: HomeArrangementRow[]; snoozed: HomeArrangementRow[]; settled: HomeArrangementRow[];
}
export function parseHomeArrangeAction(value: string): { version: string; destination: HomeDropDestination } | null {
  let raw: Obj;
  try { raw = obj(JSON.parse(value)); } catch { return null; }
  const destination = obj(raw.destination);
  if (typeof raw.version !== 'string' || !raw.version || !['pinned', 'active', 'settled'].includes(str(destination.section))
    || !['before', 'after'].includes(str(destination.placement)) || !(destination.targetId === null || typeof destination.targetId === 'string' && destination.targetId.length > 0)) return null;
  return { version: raw.version, destination: { section: destination.section as HomeDropDestination['section'],
    targetId: destination.targetId as string | null, placement: destination.placement as HomeDropDestination['placement'] } };
}
export function homeArrangeActionValue(version: string, section: string, targetKey: string, placement: string): string {
  return JSON.stringify({ version, destination: { section, targetId: targetKey || null, placement } });
}
/** Include hidden reservation and parked-state changes, without invalidating for titles or clock-only labels. */
export function homeArrangeVersion(order: HomeOrderSnapshot): string {
  return JSON.stringify([order.workingEnabled, order.sections.pinned.map(homeOrderKey), order.sections.active.map(homeOrderKey),
    order.threads.map(thread => [homeOrderKey(thread), thread.pinOrderKey ?? null, thread.activeOrderKey ?? null,
      thread.pinnedAt ?? null, thread.settledOverride ?? null, thread.snoozedUntil ?? null, thread.archivedAt ?? null,
      thread.unsettledAt ?? thread.createdAt ?? null]).sort((a, b) => str(a[0]).localeCompare(str(b[0])))]);
}
export function homeDropLifecycle(thread: Obj, section: 'pinned' | 'active', now: number) {
  return section === 'pinned' ? { pin: true, unpin: false, unsettle: false, unsnooze: false }
    : { pin: false, unpin: thread.pinnedAt != null, unsettle: thread.settledOverride === 'settled', unsnooze: effectiveSnoozed(thread, now) };
}
export function homeArrangeSnapshot(client: object, sources: HomeOrderSource[], now: number, preparedOrder?: HomeOrderSnapshot): HomeArrangementSnapshot {
  const order = preparedOrder ?? mobileHomeOrder(client, sources, now);
  const seen = new Set([...order.sections.pinned, ...order.sections.active].map(homeOrderKey));
  const parked = order.threads.filter(thread => thread.archivedAt == null && thread.deletedAt == null
    && obj(thread.lineage).relationshipToParent !== 'subagent' && !seen.has(homeOrderKey(thread)));
  const sections = { pinned: homeApplyPending(order.sections.pinned, 'pinned', order.pending),
    active: order.workingEnabled ? sortByReturn(order.sections.active) : homeApplyPending(order.sections.active, 'active', order.pending),
    snoozed: parked.filter(thread => effectiveSnoozed(thread, now)), settled: parked.filter(thread => !effectiveSnoozed(thread, now)) };
  const sourceCaps = new Map(sources.map(source => [source.environmentId, capabilities(source.config)]));
  const project = (section: keyof typeof sections) => sections[section].map(thread => {
    const key = homeOrderKey(thread), caps = sourceCaps.get(str(thread.environmentId))!, available = order.availability.get(key);
    const destinationAllowed = (destination: 'pinned' | 'active') => {
      if (section === destination || destination === 'active' && order.workingEnabled || order.blocked) return false;
      if ((destination === 'pinned' || thread.pinnedAt != null) && !caps.pinning) return false;
      if (destination === 'active' && (section === 'settled' && !caps.settlement || section === 'snoozed' && !caps.snooze)) return false;
      return homeMovePlan({ ordered: order.sections[destination], allThreads: order.threads, section: destination,
        reorderableEnvironmentIds: order.reorderable[destination] }, key, { section: destination, targetId: null, placement: 'before' }) !== null;
    };
    return { key, environmentId: str(thread.environmentId), threadId: str(thread.id), title: str(thread.title),
      disabled: order.blocked || !(caps.pinReorder || caps.activeReorder), canMoveUp: available?.canMoveUp === true,
      canMoveDown: available?.canMoveDown === true, canPin: destinationAllowed('pinned'), canActivate: destinationAllowed('active'),
      canSettle: !order.blocked && section !== 'settled' && caps.settlement };
  });
  return { version: homeArrangeVersion(order), blocked: order.blocked, workingEnabled: order.workingEnabled,
    pinned: project('pinned'), active: project('active'), snoozed: project('snoozed'), settled: project('settled') };
}
