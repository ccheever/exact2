// @ref llp/1109.004-home-projection.decision.md#decision
// Pinned365aa87982 ArchivedThreadsHeader + ScreenHeader. No archive cache/request ownership.
import { arr, obj, str } from './shared/domain';

export interface ArchiveChromeEvent { routeKey: string; kind: string; value: string }
const blankEvent = (): ArchiveChromeEvent => ({ routeKey: '', kind: '', value: '' });
/** Arguments stay serializable; only the active root route may adopt a native event. */
export function archiveChromeView(args: unknown[]) {
  const [routeKey, width, safeBottom, liquidGlass, query, environmentId, sortOrder, rows] = args;
  const compact = Number(width) < 700;
  const environments = arr(rows).map(row => ({ id: str(row.id), label: str(row.label) })).filter(row => row.id);
  return { routeKey: str(routeKey), configuration: JSON.stringify({ routeKey: str(routeKey), compact,
    query: str(query), environmentId: str(environmentId), sortOrder: sortOrder === 'oldest' ? 'oldest' : 'newest', environments }),
    toolbarHeight: compact && liquidGlass === true ? 56 + Math.max(0, Number(safeBottom) || 0) : 0 };
}
export function archiveChromeEvent(input: string, routeKey: string, environments: unknown): ArchiveChromeEvent {
  let event;
  try { event = obj(JSON.parse(input)); } catch { return blankEvent(); }
  if (!routeKey || event.routeKey !== routeKey || typeof event.value !== 'string') return blankEvent();
  if (event.kind === 'search' || event.kind === 'sort' && ['newest', 'oldest'].includes(event.value)
    || event.kind === 'environment' && (event.value === '' || arr(environments).some(row => row.id === event.value))) {
    return { routeKey, kind: str(event.kind), value: event.value };
  }
  return blankEvent();
}
