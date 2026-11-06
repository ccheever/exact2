// Thread order (T3 Code 6d450a4284; MIT, see LICENSE-T3): packages/client-runtime/src/state/threadSort.ts
// (sortThreads, getThreadSortTimestamp) and apps/web/src/components/Sidebar.logic.ts (orderItemsByPreferredIds,
// resolveAdjacentThreadId). Pure functions over the raw V2 thread shell.
import { str, type Obj } from '../client/domain';

export type ThreadSortOrder = 'updated_at' | 'created_at';

function toSortableTimestamp(iso: unknown): number | null {
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
function getThreadSortTimestamp(thread: Obj, order: ThreadSortOrder): number {
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

export function resolveAdjacentThreadId<T>(input: { threadIds: readonly T[]; currentThreadId: T | null; direction: 'previous' | 'next' }): T | null {
  const { threadIds, currentThreadId, direction } = input;
  if (!threadIds.length) return null;
  if (currentThreadId === null) return direction === 'previous' ? threadIds[threadIds.length - 1] ?? null : threadIds[0] ?? null;
  const at = threadIds.indexOf(currentThreadId);
  if (at === -1) return null;
  return direction === 'previous' ? (at > 0 ? threadIds[at - 1] ?? null : null) : (at < threadIds.length - 1 ? threadIds[at + 1] ?? null : null);
}
