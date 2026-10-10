// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/sidebar-model.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The sidebar's thread model (Sidebar.tsx, Sidebar.logic.ts, client-runtime
// models.ts/threadSettled.ts/threadSort.ts; MIT, see LICENSE-T3): status, the
// shelf a thread lives on, the static sort, unread/woke/recede prominence and
// the labels a row draws. Pure functions over the raw V2 thread shell.
// 1e2ecbd975 (1302ccacbd): the Working shelf orders by the last authored send
// (threadInbox.ts sortWorkingThreadsBySend); the active shelf keeps the return order.
import { arr, obj, str, type Obj } from './domain';

export type SidebarSection = 'pinned' | 'active' | 'working' | 'snoozed' | 'settled';
export type SidebarStatus = 'approval' | 'input' | 'working' | 'waiting' | 'failed' | 'limited' | 'ready';
export interface Caps {
  settlement: boolean; snooze: boolean; pinning: boolean; pinReorder: boolean; activeReorder: boolean;
  autoSettleOptOut: boolean; titleRegeneration: boolean; visitedTracking: boolean;
}
export interface Runtime { status: string; activeRunId: string; activityStartedAt: string | null | undefined; updatedAt: string; lastErrorClass: string }
export interface RunSummary { status: string; requestedAt: string | null; startedAt: string | null; completedAt: string | null }

const HOUR = 3_600_000, DAY = 24 * HOUR, QUEUED_GRACE = 2 * 60_000;
const parse = (value: unknown): number => { const time = Date.parse(str(value)); return Number.isFinite(time) ? time : NaN; };
const valid = (value: unknown): boolean => typeof value === 'string' && Number.isFinite(Date.parse(value));

export function capabilities(config: Obj): Caps {
  const caps = obj(obj(config.environment).capabilities);
  return { settlement: caps.threadSettlement === true, snooze: caps.threadSnooze === true, pinning: caps.threadPinning === true,
    pinReorder: caps.threadPinReorder === true, activeReorder: caps.threadActiveReorder === true,
    autoSettleOptOut: caps.threadAutoSettleOptOut === true, titleRegeneration: caps.threadTitleRegeneration === true,
    visitedTracking: caps.threadVisitedTracking === true };
}

const terminal = (status: string) => ['completed', 'interrupted', 'failed', 'cancelled', 'rolled_back'].includes(status);
/**
 * backgroundWorkHoldsCompletion: subagents, monitors and unknown work wake the
 * agent; commands it left running (a dev server) do not hold the thread.
 */
export const backgroundHolds = (thread: Obj): boolean => arr(thread.pendingBackgroundTasks).some(task => str(task.kind) !== 'command');
/** models.ts shellRuntime: idle parks only the roster that holds the run's completion. */
export function shellRuntime(thread: Obj): Runtime | null {
  if (thread.latestRunId == null && thread.activeProviderThreadId == null) return null;
  const park = backgroundHolds(thread) && thread.status !== 'failed';
  return { status: park ? 'idle' : str(thread.activityRunStatus ?? thread.status), activeRunId: str(thread.activeRunId),
    activityStartedAt: thread.activityRunStartedAt === undefined ? undefined : thread.activityRunStartedAt === null ? null : str(thread.activityRunStartedAt),
    updatedAt: str(thread.updatedAt), lastErrorClass: str(thread.lastErrorClass) };
}
/** models.ts presentThreadShell latestRun. */
export function latestRun(thread: Obj): RunSummary | null {
  if (thread.latestRunId == null) return null;
  const status = thread.status === 'idle' ? 'completed' : str(thread.status);
  const completedAt = thread.latestRunCompletedAt === undefined
    ? thread.status === 'idle' || terminal(str(thread.status)) ? str(thread.updatedAt) : null
    : thread.latestRunCompletedAt === null ? null : str(thread.latestRunCompletedAt);
  return { status, requestedAt: thread.latestRunRequestedAt == null ? null : str(thread.latestRunRequestedAt),
    startedAt: thread.latestRunStartedAt == null ? null : str(thread.latestRunStartedAt), completedAt };
}
export function pendingApproval(thread: Obj): boolean {
  const request = thread.pendingRuntimeRequest;
  return request != null && !['user_input', 'auth_refresh'].includes(str(obj(request).kind));
}
export const pendingInput = (thread: Obj): boolean => thread.pendingRuntimeRequest != null && obj(thread.pendingRuntimeRequest).kind === 'user_input';

/** Sidebar.logic.ts resolveSidebarThreadStatus. */
export function sidebarStatus(thread: Obj): SidebarStatus {
  if (pendingApproval(thread)) return 'approval';
  if (pendingInput(thread)) return 'input';
  const runtime = shellRuntime(thread);
  if (runtime && ['preparing', 'queued', 'starting', 'running', 'waiting'].includes(runtime.status)) return 'working';
  if (runtime?.status === 'idle') return 'waiting';
  if (runtime?.status === 'failed') return runtime.lastErrorClass === 'usage_limit' ? 'limited' : 'failed';
  return 'ready';
}

/** threadSettled.ts hasQueuedTurnStart: a user message no run adopted yet. */
export function hasQueuedTurnStart(thread: Obj, now: number): boolean {
  const runtime = shellRuntime(thread);
  if (runtime && ['preparing', 'queued', 'starting'].includes(runtime.status)) return true;
  const messageAt = parse(thread.latestUserMessageAt);
  if (!Number.isFinite(messageAt) || !Number.isFinite(now) || Math.abs(now - messageAt) > QUEUED_GRACE) return false;
  const run = latestRun(thread);
  if (!run) return true;
  return [run.requestedAt, run.startedAt, run.completedAt].every(value => value == null || parse(value) < messageAt);
}
export function canSnooze(thread: Obj, now: number): boolean {
  return !pendingApproval(thread) && !pendingInput(thread) && !hasQueuedTurnStart(thread, now);
}
/** threadRaisedHandWhileSnoozed: blocked on the user, a fresh failure, or a newer completion. */
export function raisedHand(thread: Obj): boolean {
  if (pendingApproval(thread) || pendingInput(thread)) return true;
  const runtime = shellRuntime(thread), run = latestRun(thread), snoozedAt = thread.snoozedAt == null ? null : str(thread.snoozedAt);
  if ((runtime?.status === 'error' || runtime?.status === 'failed') && (snoozedAt == null || parse(runtime.updatedAt) > parse(snoozedAt))) return true;
  return snoozedAt != null && run?.status === 'completed' && run.completedAt != null && parse(run.completedAt) > parse(snoozedAt);
}
export function effectiveSnoozed(thread: Obj, now: number): boolean {
  if (thread.snoozedUntil == null) return false;
  const wake = parse(thread.snoozedUntil);
  if (!Number.isFinite(wake) || wake <= now) return false;
  return !raisedHand(thread);
}
/** threadWokeAt: when a snoozed thread came back (timer or raised hand). */
export function wokeAt(thread: Obj, now: number): string | null {
  if (thread.snoozedUntil == null) return null;
  const wake = parse(thread.snoozedUntil);
  if (!Number.isFinite(wake)) return null;
  if (raisedHand(thread)) {
    const run = latestRun(thread), snoozedAt = thread.snoozedAt == null ? null : str(thread.snoozedAt);
    if (snoozedAt != null && run?.status === 'completed' && run.completedAt != null && parse(run.completedAt) > parse(snoozedAt)) return run.completedAt;
    return shellRuntime(thread)?.updatedAt ?? snoozedAt;
  }
  return wake <= now ? str(thread.snoozedUntil) : null;
}
/** Server visited tracking wins (even a mark-unread rewind); local only stands in for older servers. */
export function lastVisited(thread: Obj, local: string | undefined): string | undefined {
  if (thread.lastVisitedAt === undefined) return local;
  return thread.lastVisitedAt === null ? undefined : str(thread.lastVisitedAt);
}
/** hasUnseenCompletion: never-visited counts as read. */
export function unseenCompletion(thread: Obj, visited: string | undefined): boolean {
  const completed = parse(latestRun(thread)?.completedAt);
  if (!Number.isFinite(completed) || !visited) return false;
  const seen = parse(visited);
  return !Number.isFinite(seen) || completed > seen;
}
export function isWoke(thread: Obj, woke: string | null, visited: string | undefined): boolean {
  const wokeTime = woke == null ? NaN : parse(woke);
  if (!Number.isFinite(wokeTime)) return false;
  const seen = visited == null ? NaN : parse(visited);
  return (!Number.isFinite(seen) || seen < wokeTime) && thread.settledOverride !== 'settled';
}
export function recedes(status: SidebarStatus, unread: boolean, woke: boolean, active: boolean, selected: boolean): boolean {
  if (active || selected || status === 'input') return false;
  if (status === 'working' || status === 'waiting') return true;
  if (status === 'ready' || status === 'approval') return !unread && !woke;
  return false;
}
/** Working beta: busy work that does not need the user (a plan prompt outranks it). */
export function isWorkingThread(thread: Obj): boolean {
  const status = sidebarStatus(thread);
  if (status !== 'working' && status !== 'waiting') return false;
  const run = latestRun(thread), runtime = shellRuntime(thread);
  // session-logic.ts isLatestRunSettled.
  const settled = run !== null && !['preparing', 'queued', 'starting', 'running', 'waiting'].includes(run.status)
    && runtime?.activeRunId !== str(thread.latestRunId);
  return !(thread.interactionMode === 'plan' && thread.hasActionableProposedPlan === true && settled);
}
/** resolveSidebarThreadSection: snooze wins until its wake, then settlement beats a stale pin. */
export function sectionOf(thread: Obj, caps: Caps, now: number, workingShelf: boolean): SidebarSection {
  if (caps.snooze && effectiveSnoozed(thread, now)) return 'snoozed';
  if (caps.settlement && thread.settledOverride === 'settled') return 'settled';
  if (thread.pinnedAt != null) return 'pinned';
  return workingShelf && isWorkingThread(thread) ? 'working' : 'active';
}
/** filterSidebarV2VisibleThreads: archived and subagent threads stay out of the list. */
export function sidebarVisible(thread: Obj): boolean {
  return thread.archivedAt == null && thread.deletedAt == null && obj(thread.lineage).relationshipToParent !== 'subagent';
}

const byId = (left: Obj, right: Obj) => str(left.id).localeCompare(str(right.id));
const anchor = (thread: Obj) => Math.max(parse(thread.createdAt) || 0, parse(thread.unsettledAt) || 0);
/** sortPinnedThreadsByOrderKey: keyed run first, then keyless newest-created first. */
export function sortPinned(threads: Obj[]): Obj[] {
  const keyed = threads.filter(thread => thread.pinOrderKey != null), keyless = threads.filter(thread => thread.pinOrderKey == null);
  keyed.sort((left, right) => str(left.pinOrderKey) < str(right.pinOrderKey) ? -1 : str(left.pinOrderKey) > str(right.pinOrderKey) ? 1 : byId(left, right));
  keyless.sort((left, right) => (parse(right.createdAt) || 0) - (parse(left.createdAt) || 0) || byId(left, right));
  return [...keyed, ...keyless];
}
/** sortActiveThreadsByOrderKey: new and reopened threads lead; activity never moves a row. */
export function sortActive(threads: Obj[]): Obj[] {
  return [...threads].sort((left, right) => {
    const a = left.activeOrderKey, b = right.activeOrderKey;
    if (a == null && b != null) return -1;
    if (a != null && b == null) return 1;
    const order = a != null && b != null ? (str(a) < str(b) ? -1 : str(a) > str(b) ? 1 : 0) : anchor(right) - anchor(left);
    return order || byId(left, right);
  });
}
/** sortInboxThreadsByReturn (Working beta): newest return to the user first. */
export function sortByReturn(threads: Obj[], observed?: (thread: Obj) => number | undefined): Obj[] {
  const time = (thread: Obj) => { const run = latestRun(thread); return Math.max(anchor(thread), parse(run?.requestedAt) || 0, parse(run?.completedAt) || 0, observed?.(thread) ?? 0); };
  return [...threads].sort((left, right) => time(right) - time(left) || byId(left, right));
}
/**
 * sortWorkingThreadsBySend (threadInbox.ts, 1e2ecbd975 1302ccacbd): the Working
 * shelf lists threads newest first by the last message the user sent. Runs
 * ending and wakes do not move a row. A shell without
 * `latestUserAuthoredMessageAt` (an older server) falls back to the latest
 * run's request time; `null` (an agent-launched thread) leaves creation time.
 * exact2: reads the raw V2 shell through `latestRun`; the reference's
 * sortNewestFirst tie (thread id, then environment id) is kept.
 */
export function sortWorkingThreadsBySend(threads: Obj[]): Obj[] {
  const time = (thread: Obj) => {
    const sent = thread.latestUserAuthoredMessageAt === undefined ? latestRun(thread)?.requestedAt : thread.latestUserAuthoredMessageAt;
    return Math.max(parse(thread.createdAt) || 0, parse(sent) || 0);
  };
  return [...threads].sort((left, right) => time(right) - time(left) || byId(left, right)
    || str(left.environmentId).localeCompare(str(right.environmentId)));
}
/** resolveSettledThreadTimestamp: settledAt, else the latest message/run stamp, else updatedAt. */
export function settledTimestamp(thread: Obj): string | null {
  if (valid(thread.settledAt)) return str(thread.settledAt);
  const run = latestRun(thread);
  let latest: string | null = null, latestTime = -Infinity;
  for (const candidate of [thread.latestUserMessageAt, run?.requestedAt, run?.startedAt, run?.completedAt]) {
    if (valid(candidate) && parse(candidate) > latestTime) { latest = str(candidate); latestTime = parse(candidate); }
  }
  return latest ?? (valid(thread.updatedAt) ? str(thread.updatedAt) : null);
}
export function sortSettled(threads: Obj[]): Obj[] {
  const time = (thread: Obj) => { const stamp = settledTimestamp(thread); return stamp ? parse(stamp) : 0; };
  return [...threads].sort((left, right) => time(right) - time(left) || byId(left, right));
}
/** Soonest wake first. */
export function sortSnoozed(threads: Obj[]): Obj[] {
  return [...threads].sort((left, right) => (parse(left.snoozedUntil) || 0) - (parse(right.snoozedUntil) || 0));
}

/** compactSidebarTimeLabel(formatRelativeTimeLabel): now, 5m, 3h, 2d. */
export function ageLabel(value: unknown, now: number): string {
  const time = parse(value);
  if (!Number.isFinite(time)) return '';
  const elapsed = now - time;
  if (elapsed < 60_000) return 'now';
  const minutes = Math.floor(elapsed / 60_000);
  return minutes < 60 ? `${minutes}m` : minutes < 1440 ? `${Math.floor(minutes / 60)}h` : `${Math.floor(minutes / 1440)}d`;
}
/** threadTimeLabel: latest user message, else updatedAt. */
export const threadTimeLabel = (thread: Obj, now: number) => ageLabel(thread.latestUserMessageAt ?? thread.updatedAt, now);
/** snoozeWakeLabel: minutes round up so a hidden snooze never reads 0m. */
export function snoozeWakeLabel(until: unknown, now: number): string {
  const wake = parse(until);
  if (!Number.isFinite(wake) || !Number.isFinite(now)) return 'now';
  const remaining = wake - now;
  if (remaining <= 0) return 'now';
  if (remaining < HOUR) return `${Math.max(1, Math.ceil(remaining / 60_000))}m`;
  if (remaining < DAY) return `${Math.ceil(remaining / HOUR)}h`;
  return `${Math.ceil(remaining / DAY)}d`;
}
export function workingDuration(elapsed: number): string {
  const seconds = Number.isFinite(elapsed) ? Math.max(0, Math.floor(elapsed / 1000)) : 0;
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  return minutes < 60 ? `${minutes}m` : `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}
/** resolveThreadWorkingStartedAt: the activity-owning run's start. */
export function workingStartedAt(thread: Obj): string | null {
  const runtime = shellRuntime(thread), run = latestRun(thread);
  if (runtime && runtime.activityStartedAt !== undefined) return valid(runtime.activityStartedAt) ? str(runtime.activityStartedAt) : null;
  if (run && run.completedAt === null && str(thread.latestRunId) === runtime?.activeRunId) {
    return valid(run.startedAt) ? run.startedAt : valid(run.requestedAt) ? run.requestedAt : null;
  }
  return null;
}

export interface TopStatus { label: string; icon: string; color: string }
/** Sidebar.tsx topStatus: the pill that replaces the age at the card's top right. */
export function topStatus(status: SidebarStatus, woke: boolean, unread: boolean): TopStatus | null {
  if (status === 'working') return { label: 'Working', icon: 'circle-dashed', color: '#2b7fff' };
  if (status === 'waiting') return { label: 'Waiting', icon: '', color: 'light-dark(#71717b, #818181)' };
  if (status === 'approval') return { label: 'Approval', icon: 'shield-question', color: 'light-dark(#bb4d00, #ffb900)' };
  if (status === 'input') return { label: 'Input', icon: 'message-circle-question', color: 'light-dark(#4f39f6, #a3b3ff)' };
  if (status === 'limited') return { label: 'Limited', icon: 'circle-alert', color: '#fe9a00' };
  if (status === 'failed') return { label: 'Failed', icon: 'circle-alert', color: 'light-dark(#fb2c36, #fb414a)' };
  if (woke) return { label: 'Woke', icon: 'alarm-clock', color: '#fe9a00' };
  if (unread) return { label: 'Done', icon: 'circle-check', color: '#00bc7d' };
  return null;
}

/** Fractional pin/active order keys (threadSort.ts pinOrderKeyBetween/planPinnedReorder). */
const DIGITS = 'abcdefghijklmnopqrstuvwxyz';
const validKey = (key: string) => key.length > 0 && [...key].every(char => DIGITS.includes(char)) && key.charAt(key.length - 1) !== DIGITS[0];
function midpoint(a: string, b: string): string {
  if (b !== '') {
    let n = 0;
    while ((a.charAt(n) || DIGITS[0]) === b.charAt(n)) n += 1;
    if (n > 0) return b.slice(0, n) + midpoint(a.slice(n), b.slice(n));
  }
  const digitA = a === '' ? 0 : DIGITS.indexOf(a.charAt(0));
  const digitB = b === '' ? DIGITS.length : DIGITS.indexOf(b.charAt(0));
  if (digitB - digitA > 1) return DIGITS.charAt(Math.round((digitA + digitB) / 2));
  if (b.length > 1) return b.charAt(0);
  return DIGITS.charAt(digitA) + midpoint(a.slice(1), '');
}
export function orderKeyBetween(before: string | null, after: string | null): string | null {
  const a = before ?? '', b = after ?? '';
  if (a !== '' && !validKey(a) || b !== '' && !validKey(b) || b !== '' && a >= b) return null;
  return midpoint(a, b);
}
export function spreadKeys(count: number): string[] {
  let width = 2, space = DIGITS.length ** width;
  while (space <= (count + 1) * 2) { width += 1; space *= DIGITS.length; }
  const step = space / (count + 1), keys: string[] = [];
  for (let index = 0; index < count; index++) {
    let value = Math.round(step * (index + 1));
    if (value % DIGITS.length === 0) value += 1;
    let key = '';
    for (let digit = 0; digit < width; digit++) { key = DIGITS.charAt(value % DIGITS.length) + key; value = Math.floor(value / DIGITS.length); }
    keys.push(key);
  }
  return keys;
}
/** planPinnedReorder: one write when both neighbours are keyed, else materialize the section. */
export function planReorder(orderedIds: string[], keysById: Map<string, string | null>, movedId: string): { id: string; orderKey: string }[] {
  const visible = new Set(orderedIds);
  const reserved = new Set([...keysById].flatMap(([id, key]) => !visible.has(id) && key != null ? [key] : []));
  const at = orderedIds.indexOf(movedId);
  if (at < 0) return [];
  const beforeId = at > 0 ? orderedIds[at - 1]! : null, afterId = at < orderedIds.length - 1 ? orderedIds[at + 1]! : null;
  const beforeKey = beforeId ? keysById.get(beforeId) ?? null : null, afterKey = afterId ? keysById.get(afterId) ?? null : null;
  if ((beforeId === null || beforeKey != null) && (afterId === null || afterKey != null)) {
    let key = orderKeyBetween(beforeKey, afterKey);
    while (key !== null && reserved.has(key)) key = orderKeyBetween(key, afterKey);
    if (key !== null) return [{ id: movedId, orderKey: key }];
  }
  const keys = spreadKeys(orderedIds.length + reserved.size).filter(key => !reserved.has(key)).slice(0, orderedIds.length);
  return orderedIds.flatMap((id, index) => keysById.get(id) === keys[index] ? [] : [{ id, orderKey: keys[index]! }]);
}
