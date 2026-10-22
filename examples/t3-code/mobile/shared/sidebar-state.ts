// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/sidebar-state.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Sidebar state that is not the server's: persisted UI preferences (the
// reference keeps them in localStorage and its persisted UI store) and the
// window's transient sidebar session (rename, selection, paging, dialogs,
// undo), held beside the client the way sidebar-presentation keeps searches.
import { obj, str } from './domain';
import type { T3Client } from './client';

export const SETTLED_TAIL_INITIAL_COUNT = 10;
export const SETTLED_TAIL_PAGE_COUNT = 25;

/**
 * Shelf expansion (t3code:sidebar:*-expanded), project scope (sidebarProjectScopeKey) and local visits;
 * legacy-sidebar: the persisted UI store's projectExpandedById and projectOrder (uiStateStore.ts).
 */
export interface SidebarPrefs {
  settledExpanded: boolean; snoozedExpanded: boolean; workingExpanded: boolean;
  scope: string; visited: Record<string, string>;
  projectExpanded: Record<string, boolean>; projectOrder: string[];
}
export function defaultSidebarPrefs(): SidebarPrefs {
  return { settledExpanded: false, snoozedExpanded: false, workingExpanded: false, scope: '', visited: {}, projectExpanded: {}, projectOrder: [] };
}
export function decodeSidebarPrefs(value: unknown): SidebarPrefs {
  const saved = obj(value), visited: Record<string, string> = {};
  for (const [key, stamp] of Object.entries(obj(saved.visited)).slice(-500)) {
    if (typeof stamp === 'string' && Number.isFinite(Date.parse(stamp))) visited[key] = stamp;
  }
  const projectExpanded: Record<string, boolean> = {};
  for (const [key, value] of Object.entries(obj(saved.projectExpanded)).slice(-1000)) if (key && typeof value === 'boolean') projectExpanded[key] = value;
  const projectOrder = [...new Set((Array.isArray(saved.projectOrder) ? saved.projectOrder : []).filter((key): key is string => typeof key === 'string' && key.length > 0))].slice(0, 1000);
  return { settledExpanded: saved.settledExpanded === true, snoozedExpanded: saved.snoozedExpanded === true,
    workingExpanded: saved.workingExpanded === true, scope: str(saved.scope).slice(0, 2000), visited, projectExpanded, projectOrder };
}
/** The client's preferences carry the sidebar block; older files decode to the defaults. */
export function sidebarPrefs(client: T3Client): SidebarPrefs {
  const local = client.local as unknown as { sidebar?: SidebarPrefs };
  if (!local.sidebar) local.sidebar = defaultSidebarPrefs();
  return local.sidebar;
}

export type UndoAction = 'Settled' | 'Snoozed' | 'Unpinned' | 'Archived' | 'Discarded'; // lane r11-upstream: Discarded (95edeb753b)
export interface UndoEntry { action: UndoAction; threadIds: string[]; at: number }
export interface SidebarDialog { kind: '' | 'archive' | 'delete' | 'delete-many' | 'snooze' | 'unpin' | 'delete-worktree'; threadIds: string[]; title: string }
export interface SidebarSession {
  settledVisible: number; settledScope: string;
  searchIndex: number; searchQuery: string; searchPending: boolean;
  renameId: string; renameTitle: string;
  selection: string[]; anchor: string;
  dialog: SidebarDialog;
  undo: UndoEntry | null;
  regenerating: Set<string>;
  busy: Set<string>;
  jumpHints: boolean;
  dismissedPill: string;
  /** A navigation the window performs (Project settings): read by the shell after the command. */
  navigate: { kind: string; projectId: string };
  scopeOpen: boolean; scopeQuery: string;
  dialogMode: string; dialogDate: string; dialogTime: string; dialogAmount: string; dialogUnit: string; dialogError: string;
  /** lane r11-upstream (1826fb55cc): bumped when a row-action sweep is released, which ends the view's sweep. */
  sweepEpoch: number;
}
const sessions = new WeakMap<T3Client, SidebarSession>();
export function sidebarSession(client: T3Client): SidebarSession {
  let session = sessions.get(client);
  if (!session) {
    session = { settledVisible: SETTLED_TAIL_INITIAL_COUNT, settledScope: '', searchIndex: 0, searchQuery: '', searchPending: false,
      renameId: '', renameTitle: '', selection: [], anchor: '', dialog: { kind: '', threadIds: [], title: '' },
      undo: null, regenerating: new Set(), busy: new Set(), jumpHints: false, dismissedPill: '', navigate: { kind: '', projectId: '' }, scopeOpen: false, scopeQuery: '', dialogMode: 'date', dialogDate: '', dialogTime: '', dialogAmount: '2', dialogUnit: 'hours', dialogError: '', sweepEpoch: 0 };
    sessions.set(client, session);
  }
  return session;
}
export function closeDialog(session: SidebarSession): void {
  session.dialog = { kind: '', threadIds: [], title: '' };
  session.dialogError = '';
}
const pad = (value: number, width = 2) => String(value).padStart(width, '0');
/** localSnoozeDate / localSnoozeTime. */
export const localDate = (date: Date) => `${pad(date.getFullYear(), 4)}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
export const localTime = (date: Date) => `${pad(date.getHours())}:${pad(date.getMinutes())}`;
/** CustomSnoozeDialog opens an hour out. */
export function openSnoozeDialog(session: SidebarSession, threadIds: string[], title: string, now: number): void {
  const initial = new Date(now + 3_600_000);
  session.dialog = { kind: 'snooze', threadIds, title };
  session.dialogMode = 'date'; session.dialogDate = localDate(initial); session.dialogTime = localTime(initial);
  session.dialogAmount = '2'; session.dialogUnit = 'hours'; session.dialogError = '';
}
/** resolveCustomSnooze: local calendar input or elapsed time; null for a past or invalid wake. */
export function resolveCustomSnooze(input: { mode: string; date: string; time: string; amount: string; unit: string }, now: number): { until: string | null; error: string } {
  const { mode, date, time, amount, unit } = input;
  let wake: Date;
  if (mode === 'duration') {
    const count = Number(amount), unitMs = ({ minutes: 60_000, hours: 3_600_000, days: 86_400_000 } as Record<string, number>)[unit];
    if (!Number.isFinite(count) || count <= 0 || !unitMs) return { until: null, error: 'Enter a positive duration.' };
    wake = new Date(now + count * unitMs);
  } else {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date) || !/^\d{2}:\d{2}$/.test(time)) return { until: null, error: 'Choose a valid date and time in the future.' };
    wake = new Date(`${date}T${time}:00`);
    if (localDate(wake) !== date || localTime(wake) !== time) return { until: null, error: 'Choose a valid date and time in the future.' };
  }
  return Number.isFinite(wake.getTime()) && wake.getTime() > now ? { until: wake.toISOString(), error: '' }
    : { until: null, error: mode === 'duration' ? 'Enter a positive duration.' : 'Choose a valid date and time in the future.' };
}
/** client.ts load(): the saved file's sidebar block onto fresh preferences. */
export function adoptSidebarPrefs(local: object, saved: { sidebar?: unknown }): void {
  (local as { sidebar?: SidebarPrefs }).sidebar = decodeSidebarPrefs(saved.sidebar);
}
/**
 * The runtime clock where one exists. Data modules have none: the bake refuses a direct `Date.now()` (exact2
 * 8be2b2623) and the data runtime refused it before that, so in the app this returns the fallback. Bun tests
 * stand in for a host clock with `setRuntimeClock`. It is never an instant on its own (the agent's clock counts
 * from 0); `wall` below turns it into one by measuring from the window's wall time; r8-pointer-clock's
 * `wallIso` is the rule for stored stamps.
 */
let runtimeClock: () => number = () => Number.NaN;
export function setRuntimeClock(read: () => number): void { runtimeClock = read; }
export const clock = (fallback = 0): number => { try { const time = runtimeClock(); return Number.isFinite(time) && time > 0 ? time : fallback; } catch { return fallback; } };
/**
 * The wall clock for sidebar commands. The window passes its wall time with
 * each sidebar command (exactTime's epoch plus the window clock); the runtime
 * clock is real time in a shipped app but the agent's virtual clock under
 * test, so the offset between the two keeps every command, timer and notice
 * on the window's time.
 */
const offsets = new WeakMap<T3Client, number>();
export function adoptCommandTime(client: T3Client, at: number): void {
  if (Number.isFinite(at) && at > 1e12) offsets.set(client, at - clock());
}
/**
 * r13-store audit: every persisted value `wall` produces (a snooze's `snoozedUntil`, `visited`) is computed inside
 * a command that adopted the window's instant first (`sidebarCommand` adopts before anything reads), so it is that
 * instant plus the clock's advance. Without an adopted instant `wall` is the bare clock: 0 under the data runtime
 * (snoozePresets then returns nothing), so no 1970 value is written (r13-store.test.ts).
 */
export function wall(client: T3Client): number { return clock() + (offsets.get(client) ?? 0); }
/** showThreadUndoNotice: the notice lives five seconds. */
export function undoLive(client: T3Client): boolean {
  const undo = sidebarSession(client).undo;
  return !!undo && wall(client) - undo.at < 5000 && undo.threadIds.length > 0;
}
