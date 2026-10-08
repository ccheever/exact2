// @ref llp/1107.004-home-projection.decision.md#decision
// @ref llp/1107.011-responsive-workspace.decision.md#navigation-and-data-ownership
// Pinned 365aa87982 threadOrder.ts, threadListV2.ts and state/thread-order.ts.
import { obj, str, type Obj } from './shared/domain';
import { capabilities, effectiveSnoozed, orderKeyBetween, planReorder, sortActive, sortPinned, spreadKeys as orderSpreadKeys } from './shared/sidebar-model';

export type HomeOrderSection = 'pinned' | 'active';
export type HomeMoveDirection = 'up' | 'down';
export interface HomeOrderSource {
  environmentId: string; config: Obj; shell: { threads: Obj[] }; origin?: string; connected?: boolean;
}
export interface HomeOrderAssignment { id: string; orderKey: string }
export interface HomeMoveAvailability { canMoveUp: boolean; canMoveDown: boolean }
export interface HomePendingOrder {
  serial: number; section: HomeOrderSection; orderedIds: string[];
  before: Record<string, { key: string | null; anchor: string }>;
  assignments: Record<string, string>; confirmed: string[]; commandsComplete: boolean;
}
export interface HomeUnknownOrder {
  environmentId: string; threadId: string; origin: string; section: HomeOrderSection; orderKey: string;
}
export interface HomeOrderState {
  serial: number; busy: boolean; pending: HomePendingOrder | null; unknown: HomeUnknownOrder | null;
  workingEnabled: boolean; queuedThreadKeys: string[];
}
export interface HomeOrderSnapshot {
  threads: Obj[]; sections: Record<HomeOrderSection, Obj[]>; reorderable: Record<HomeOrderSection, Set<string>>;
  availability: Map<string, HomeMoveAvailability>; pending: HomePendingOrder | null; workingEnabled: boolean; blocked: boolean; queuedThreadKeys: ReadonlySet<string>;
}
const owners = new WeakMap<object, HomeOrderState>();
export function homeOrderState(client: object): HomeOrderState {
  let state = owners.get(client);
  if (!state) { state = { serial: 0, busy: false, pending: null, unknown: null, workingEnabled: false, queuedThreadKeys: [] }; owners.set(client, state); }
  return state;
}
export const homeOrderKey = (row: Obj) => `${str(row.environmentId)}:${str(row.id)}`;
function homeRowOrder(row: Obj, section: HomeOrderSection) {
  return { key: row[section === 'pinned' ? 'pinOrderKey' : 'activeOrderKey'] == null ? null : str(row[section === 'pinned' ? 'pinOrderKey' : 'activeOrderKey']),
    anchor: section === 'pinned' ? str(row.pinnedAt) : str(row.unsettledAt ?? row.createdAt) };
}

/** Full source section; display filters never change move neighbors or reserved keys. */
export function homeOrderedSection(sources: HomeOrderSource[], section: HomeOrderSection, now: number, queued: ReadonlySet<string> = new Set()): Obj[] {
  const rows = sources.flatMap(source => {
    const caps = capabilities(source.config);
    return source.shell.threads.filter(thread => thread.archivedAt == null && obj(thread.lineage).relationshipToParent !== 'subagent'
      && !(caps.settlement && thread.settledOverride === 'settled' && !queued.has(`${source.environmentId}:${str(thread.id)}`))
      && !(caps.snooze && effectiveSnoozed(thread, now)) && (thread.pinnedAt != null) === (section === 'pinned'))
      .map(thread => ({ ...thread, environmentId: source.environmentId }));
  });
  rows.sort((a, b) => str(a.environmentId).localeCompare(str(b.environmentId)));
  return section === 'pinned' ? sortPinned(rows) : sortActive(rows);
}
export function homeOrderAfterMove(orderedIds: string[], movedId: string, direction: HomeMoveDirection): string[] | null {
  const from = orderedIds.indexOf(movedId), to = from + (direction === 'up' ? -1 : 1);
  if (from < 0 || to < 0 || to >= orderedIds.length) return null;
  const next = orderedIds.filter(id => id !== movedId); next.splice(to, 0, movedId); return next;
}
export function homeMovePlan(input: { ordered: Obj[]; allThreads: Obj[]; section: HomeOrderSection; reorderableEnvironmentIds: ReadonlySet<string> }, movedId: string, direction: HomeMoveDirection): HomeOrderAssignment[] | null {
  const writable = new Set(input.allThreads.filter(row => input.reorderableEnvironmentIds.has(str(row.environmentId))).map(homeOrderKey));
  if (!writable.has(movedId)) return null;
  const next = homeOrderAfterMove(input.ordered.map(homeOrderKey), movedId, direction);
  if (!next) return null;
  const keys = new Map(input.allThreads.map(row => [homeOrderKey(row), homeRowOrder(row, input.section).key]));
  const assignments = planReorder(next, keys, movedId);
  return assignments.length && assignments.every(value => writable.has(value.id)) ? assignments : null;
}

export function homeMoveAvailability(input: {
  readonly ordered: readonly Obj[];
  readonly allThreads?: readonly Obj[];
  readonly section: HomePendingOrder["section"];
  readonly reorderableEnvironmentIds: ReadonlySet<string>;
  readonly pendingOrder?: HomePendingOrder | null;
}): Map<string, HomeMoveAvailability> {
  const result = new Map<string, HomeMoveAvailability>();
  // A reorder in flight locks the whole list until its receipt lands.
  if (input.pendingOrder != null) return result;
  const rows = input.ordered;
  const orderedIds = rows.map(homeOrderKey);
  const indexById = new Map(orderedIds.map((id, index) => [id, index] as const));
  const keysById = new Map(
    (input.allThreads ?? input.ordered).map(
      (row) => [homeOrderKey(row), homeRowOrder(row, input.section).key] as const,
    ),
  );
  const writableIds = new Set(
    (input.allThreads ?? input.ordered)
      .filter((row) => input.reorderableEnvironmentIds.has(str(row.environmentId)))
      .map(homeOrderKey),
  );
  const visibleIds = new Set(orderedIds);
  const reservedKeys = new Set(
    [...keysById].flatMap(([id, key]) => (!visibleIds.has(id) && key != null ? [key] : [])),
  );
  // Mirror of `planPinnedReorder` for adjacent swaps, hoisted so every row is
  // answered in O(1) amortized instead of one planner probe per row:
  //
  // Fast path (both neighbors keyed): the fresh key between the landing
  // neighbors, walking forward while hidden reserved keys block it. The walk
  // depends only on the neighbor key pair - each adjacency is probed by at
  // most two rows (down of the left member, up of the right member) - so the
  // memo keeps even a fully adversarial reserved-key layout linear per build.
  //
  // Rewrite path (keyless/unusable neighbor, or key space exhausted): fresh
  // spread keys for every row; only positions whose current key differs get
  // written. A swap permutes two rows without changing the multiset, so the
  // assignment set differs from the unswapped baseline at at most those two
  // positions, and mismatch/writability tallies computed once per section
  // answer each row with a constant-size delta.
  const midpoints = new Map<string, string | null>();
  const fastPathKey = (beforeKey: string | null, afterKey: string | null): string | null => {
    const memoKey = `${beforeKey ?? ""}\u0000${afterKey ?? ""}`;
    const cached = midpoints.get(memoKey);
    if (cached !== undefined) return cached;
    let key = orderKeyBetween(beforeKey, afterKey);
    while (key !== null && reservedKeys.has(key)) key = orderKeyBetween(key, afterKey);
    midpoints.set(memoKey, key);
    return key;
  };
  const spreadKeys = orderSpreadKeys(orderedIds.length + reservedKeys.size)
    .filter((key) => !reservedKeys.has(key))
    .slice(0, orderedIds.length);
  const currentKeys = orderedIds.map((id) => keysById.get(id) ?? null);
  const writableRow = orderedIds.map((id) => writableIds.has(id));
  let baselineWrites = 0;
  let baselineUnwritableWrites = 0;
  for (let position = 0; position < orderedIds.length; position += 1) {
    if (currentKeys[position] === spreadKeys[position]) continue;
    baselineWrites += 1;
    if (!writableRow[position]) baselineUnwritableWrites += 1;
  }
  // `movedId` swaps with its neighbor; ids at the two swapped positions change,
  // so only their tally contributions are recomputed.
  const rewriteViable = (index: number): boolean => {
    let writes = baselineWrites;
    let unwritableWrites = baselineUnwritableWrites;
    for (const position of [index, index + 1]) {
      const other = position === index ? index + 1 : index;
      if (currentKeys[position] !== spreadKeys[position]) {
        writes -= 1;
        if (!writableRow[position]) unwritableWrites -= 1;
      }
      // After the swap this position holds the row that was at `other`.
      if (currentKeys[other] !== spreadKeys[position]) {
        writes += 1;
        if (!writableRow[other]) unwritableWrites += 1;
      }
    }
    return writes > 0 && unwritableWrites === 0;
  };
  for (const row of rows) {
    const movedId = homeOrderKey(row);
    const denied = { canMoveUp: false, canMoveDown: false };
    if (!writableIds.has(movedId)) {
      result.set(movedId, denied);
      continue;
    }
    const index = indexById.get(movedId);
    if (index === undefined) {
      result.set(movedId, denied);
      continue;
    }
    const adjacentAvailable = (towardUp: boolean): boolean => {
      const shifted = index + (towardUp ? -1 : 1);
      if (shifted < 0 || shifted >= orderedIds.length) return false;
      // The swap exchanges the row with its neighbor; afterwards the moved row
      // sits at `shifted` between `beforeIndex` and `afterIndex` of the OLD
      // order: moving up it lands between old(index-2) and old(index-1),
      // moving down between old(index+1) and old(index+2).
      const beforeIndex = towardUp ? index - 2 : index + 1;
      const afterIndex = towardUp ? index - 1 : index + 2;
      const beforeId = beforeIndex < 0 ? null : (orderedIds[beforeIndex] ?? null);
      const afterId = afterIndex >= orderedIds.length ? null : (orderedIds[afterIndex] ?? null);
      const beforeKey = beforeId === null ? null : (keysById.get(beforeId) ?? null);
      const afterKey = afterId === null ? null : (keysById.get(afterId) ?? null);
      if ((beforeId === null || beforeKey != null) && (afterId === null || afterKey != null)) {
        const key = fastPathKey(beforeKey, afterKey);
        // A fresh key is a single-write plan for the (writable) moved row.
        if (key !== null) return true;
      }
      // Keyless neighbor or exhausted key space: the section rewrite runs.
      return rewriteViable(Math.min(index, shifted));
    };
    result.set(movedId, {
      canMoveUp: adjacentAvailable(true),
      canMoveDown: adjacentAvailable(false),
    });
  }
  return result;
}

export function homeCreatePending(section: HomeOrderSection, ordered: Obj[], movedId: string, direction: HomeMoveDirection, assignments: HomeOrderAssignment[], serial: number): HomePendingOrder {
  const orderedIds = homeOrderAfterMove(ordered.map(homeOrderKey), movedId, direction);
  if (!orderedIds) throw new Error('Cannot begin an invalid thread move');
  return { serial, section, orderedIds, before: Object.fromEntries(ordered.map(row => [homeOrderKey(row), homeRowOrder(row, section)])),
    assignments: Object.fromEntries(assignments.map(row => [row.id, row.orderKey])), confirmed: [], commandsComplete: false };
}
export function homeReconcilePending(pending: HomePendingOrder, ordered: Obj[]): HomePendingOrder | null {
  if (ordered.length !== Object.keys(pending.before).length) return null;
  const confirmed = new Set(pending.confirmed);
  for (const row of ordered) {
    const id = homeOrderKey(row), before = pending.before[id], current = homeRowOrder(row, pending.section);
    if (!before || before.anchor !== current.anchor) return null;
    const assigned = pending.assignments[id];
    if (assigned !== undefined && current.key === assigned) confirmed.add(id);
    else if (current.key !== before.key || confirmed.has(id)) return null;
  }
  if (pending.commandsComplete && confirmed.size === Object.keys(pending.assignments).length) return null;
  return confirmed.size === pending.confirmed.length ? pending : { ...pending, confirmed: [...confirmed] };
}
export function homeApplyPending<T extends Obj>(rows: T[], section: HomeOrderSection, pending: HomePendingOrder | null): T[] {
  if (!pending || pending.section !== section) return rows;
  const rank = new Map(pending.orderedIds.map((id, index) => [id, index]));
  return [...rows].sort((a, b) => (rank.get(homeOrderKey(a)) ?? Infinity) - (rank.get(homeOrderKey(b)) ?? Infinity));
}
/** Reuses root's existing shell/config/queue/clock observations; no subscription or timer owner. */
export function mobileHomeOrder(client: object, sources: HomeOrderSource[], now: number,
  options: { workingEnabled?: boolean; queuedThreadKeys?: ReadonlySet<string> } = {}): HomeOrderSnapshot {
  const state = homeOrderState(client);
  if (options.workingEnabled !== undefined) state.workingEnabled = options.workingEnabled;
  if (options.queuedThreadKeys !== undefined) state.queuedThreadKeys = [...options.queuedThreadKeys];
  const queued = new Set(state.queuedThreadKeys), sections = { pinned: homeOrderedSection(sources, 'pinned', now, queued), active: homeOrderedSection(sources, 'active', now, queued) };
  if (state.pending) state.pending = homeReconcilePending(state.pending, sections[state.pending.section]);
  if (state.unknown) {
    const unknown = state.unknown, source = sources.find(value => value.environmentId === unknown.environmentId && value.origin === unknown.origin && value.connected);
    const thread = source?.shell.threads.find(row => row.id === unknown.threadId);
    if (source && (!thread || homeRowOrder(thread, unknown.section).key === unknown.orderKey)) state.unknown = null;
  }
  const threads = sources.flatMap(source => source.shell.threads.map(row => ({ ...row, environmentId: source.environmentId })));
  const reorderable = { pinned: new Set<string>(), active: new Set<string>() };
  for (const source of sources) { const caps = capabilities(source.config); if (caps.pinReorder) reorderable.pinned.add(source.environmentId); if (caps.activeReorder) reorderable.active.add(source.environmentId); }
  const availability = new Map<string, HomeMoveAvailability>(), blocked = state.busy || !!state.pending || !!state.unknown;
  if (!blocked) for (const section of ['pinned', 'active'] as const) {
    if (section === 'active' && state.workingEnabled) continue;
    for (const entry of homeMoveAvailability({ ordered: sections[section], allThreads: threads, section, reorderableEnvironmentIds: reorderable[section] })) availability.set(...entry);
  }
  return { threads, sections, reorderable, availability, pending: state.pending, workingEnabled: state.workingEnabled, blocked, queuedThreadKeys: queued };
}
