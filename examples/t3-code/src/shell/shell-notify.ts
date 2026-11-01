// Thread notifications (MIT reference: components/
// ThreadNotificationCoordinator.tsx and ThreadNotificationCoordinator.logic.ts at 7c3b3b3605ef, threadNotifications.ts).
// Provider updates raise no toast at launch: the sidebar's pill and Settings ›
// Providers offer them. The window's focus is the page's
// (`exactPage().hasFocus`, document.hasFocus(): exact2 #219), and so is the permission
// (app.contract `notificationPermission`, read again when the app comes to the front). The
// system's side is @exact/notifications (LLP 1133.004): the silent post, its click, the Dock
// badge and the permission's request. A post names its destination as a `navigate`
// location the window opens (app.contract notificationOpen); the sounds are the window's
// (`notificationSound`, app.contract notifyCompletionSound).
// The bake binds the package from its exact.json; Bun's tests read the exact.json itself, so they hand in their own.
import * as notificationsPackage from '@exact/notifications';
import { noteSubagentRequests } from '../sidebar/subagent-requests';
import type { T3Client } from '../client/client';
import { pushToast } from './toast';
import { isListedThreadLineage, obj, str, type Shell } from '../client/domain';
import { bridgeReply, type Native } from '../client/protocol';
import { sidebarStatus, latestRun } from '../sidebar/sidebar-model';
import { fleet, fleetThreadId, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { letGo } from '../client/let-go';
import { decodeGuideOpenTarget } from '../pull-requests/guide-target';

export type NotificationStatus = { available: boolean; state: string; badge: boolean };
export type NotificationCalls = {
  showNotification(options: { title: string; body?: string; tag?: string; silent?: boolean; navigate?: string }): Promise<void>;
  closeNotification(options: { tag: string }): Promise<void>;
  setAppBadge(options: { contents?: number }): Promise<void>;
  clearAppBadge(options: object): Promise<void>;
  requestNotificationPermission(options: object): Promise<NotificationStatus>;
};
let notifications = notificationsPackage as unknown as NotificationCalls;
/** A test's own `@exact/notifications` (null puts the package back). */
export function useNotificationCalls(fake: NotificationCalls | null): void { notifications = fake ?? notificationsPackage as unknown as NotificationCalls; }
/** A refusal (denied, a host without notifications, a test without the package) changes nothing the window shows. */
async function quietly<T>(call: () => Promise<T>): Promise<T | undefined> {
  try { return await call(); } catch (error) { if (letGo(error)) throw error; return undefined; }
}

/**
 * `active` is the page's focus; `authorization` the permission state (`granted`, `prompt`, `denied` or `unknown`);
 * `opened` / `openedThread` the latest notification click (`notification-open`), which desktop-activation.ts opens.
 */
export type NotifyStatus = { active: boolean; authorization: string; opened: string; openedThread: string };
/** ThreadNotificationObservation: the attention key, and whether the thread is idle with its latest run completed (when). */
type Memory = { attention: string | null; settled: boolean; completedAt: number | null };
/**
 * threadInbox.ts WORKING_EXIT_GRACE_MS, which sidebar/thread-inbox.ts reads too: the Working shelf keeps a thread that
 * stopped working for 2 s, as an orchestrator run can end moments before its continuation starts; "Thread completed"
 * waits as long.
 */
export const WORKING_EXIT_GRACE_MS = 2_000;
/**
 * createThreadNotificationGate on the shell's clock: a completion waits WORKING_EXIT_GRACE_MS from the first tick after
 * it was seen (`seen` is that answer's reading, `due` 0 until the tick; the clock reads the time only when it ticks).
 */
export type NotifyGate = { pending: Map<string, { at: number; seen: number; due: number }>; handled: Map<string, number>; lastSettled: Map<string, number> };
export const notifyGate = (): NotifyGate => ({ pending: new Map(), handled: new Map(), lastSettled: new Map() });
/** Each environment's EnvironmentNotifications memory and gate, by environment id. */
type NotifyState = { previous: Map<string, Map<string, Memory>>; gates: Map<string, NotifyGate>; mode: string };
const states = new WeakMap<T3Client, NotifyState>();
function notifyState(client: T3Client): NotifyState {
  let state = states.get(client);
  if (!state) { state = { previous: new Map(), gates: new Map(), mode: '' }; states.set(client, state); }
  return state;
}
/** A completion waits out its grace: the shell's clock must keep ticking until it notifies or is cancelled. */
export function notificationsWaiting(client: T3Client): boolean {
  return [...notifyState(client).gates.values()].some(gate => gate.pending.size > 0);
}

export const hasNotificationSound = (mode: string) => mode === 'sound' || mode === 'notifications-and-sound';
export const hasDesktopNotifications = (mode: string) => mode === 'notifications' || mode === 'notifications-and-sound';

/** Each client's latest notification click, which its `navigate` route handed in (app.contract notificationOpen). */
const clicks = new WeakMap<object, { serial: number; target: string }>();
/**
 * `notification-open`: a clicked notification's destination, which is one of the window's own posts' targets: a thread
 * (`fleet:<environment>:<thread>`) or a review guide (guide-target.ts). Anything else opens nothing.
 */
export async function notificationOps(this: T3Client, op: string, id: string): Promise<boolean> {
  if (op !== 'notification-open') return false;
  if (parseFleetThreadId(id) || decodeGuideOpenTarget(id)) clicks.set(this, { serial: (clicks.get(this)?.serial ?? 0) + 1, target: id });
  return true;
}
/** Where a click opens (app.contract `notification` route): `/notification?t=<target>`. */
export const notificationLocation = (target: string) => `/notification?t=${encodeURIComponent(target)}`;

/**
 * The window's facts and the latest click: `active` is the page's own focus (`exactPage().hasFocus`,
 * ThreadNotificationCoordinator's document.hasFocus()), `permission` the root's live read of the package's state
 * ('' keeps the previous one).
 */
export function notifyStatus(client: object, previous: NotifyStatus, focused: boolean, permission: string): NotifyStatus {
  const click = clicks.get(client);
  return { active: focused, authorization: permission || previous.authorization,
    opened: click ? `${click.serial}:${click.target}` : '', openedThread: click?.target ?? '' };
}

/** The notifications posted since the window last had the focus (notificationBadge.ts counts them on the Dock tile). */
const pendings = new WeakMap<object, Set<string>>();
function pending(client: object): Set<string> {
  let tags = pendings.get(client);
  if (!tags) pendings.set(client, tags = new Set());
  return tags;
}
/**
 * `new Notification(title, {body, tag, silent: true})` while permission is granted, as the coordinator posts one
 * only then; its click opens `target`. The badge then counts what is pending (trackNotification).
 */
export async function postNotification(client: object, status: NotifyStatus, title: string, body: string, tag: string, target: string): Promise<void> {
  if (status.authorization !== 'granted') return;
  if ((await quietly(() => notifications.showNotification({ title, body, tag, silent: true, navigate: notificationLocation(target) }).then(() => true))) !== true) return;
  const tags = pending(client);
  tags.add(tag);
  await quietly(() => notifications.setAppBadge({ contents: tags.size }));
}
/** clearPending: each pending notification closes and the badge clears (the window's focus, a mode change). */
async function clearPending(client: object, always: boolean): Promise<void> {
  const tags = pending(client);
  if (!tags.size && !always) return;
  const closing = [...tags];
  tags.clear();
  for (const tag of closing) await quietly(() => notifications.closeNotification({ tag }));
  await quietly(() => notifications.clearAppBadge({}));
}

/** playNotificationSound's files' lengths (assets/notification-*.wav): one playback stands for a burst (`playingUntil`). */
const SOUND_MS = { completion: 602, input: 714 } as const;
type SoundKind = keyof typeof SOUND_MS;
const sounds = new WeakMap<object, Record<SoundKind, number> & { until: Record<SoundKind, number> }>();
function soundsOf(client: object) {
  let value = sounds.get(client);
  if (!value) sounds.set(client, value = { completion: 0, input: 0, until: { completion: 0, input: 0 } });
  return value;
}
/**
 * playNotificationSound: asks the window to play `kind`'s sound (app.contract notifyCompletionSound, notifyInputSound).
 * Each kind keeps its own slot, and one still playing at `now` stands for the next; with no clock (0) each plays.
 */
export function notificationSound(client: object, kind: SoundKind, now: number): void {
  const value = soundsOf(client);
  if (now > 0 && now < value.until[kind]) return;
  value[kind]++;
  value.until[kind] = now + SOUND_MS[kind];
}
/** The shell view's sound requests: each kind's count, which the window plays each time it moves. */
export function notificationSounds(client: object): Record<SoundKind, number> {
  const { completion, input } = soundsOf(client);
  return { completion, input };
}

const reportedFacts = new WeakMap<object, string>();
/**
 * backgroundActivityReporter's window facts (document.visibilityState and
 * document.hasFocus(), re-reported on visibilitychange, focus and blur): the
 * page's `exactPage()` facts, handed to the module's reporter when they change.
 */
export async function reportWindowFacts(owner: object, native: Native, visible: boolean, focused: boolean): Promise<void> {
  const key = `${visible}:${focused}`;
  if (reportedFacts.get(owner) === key) return;
  const response = await bridgeReply(native, { op: 'activityFacts', visible, focused }).catch(() => null);
  if (response?.ok) reportedFacts.set(owner, key);
}

type Transition = { threadId: string; kind: 'completion' | 'input'; status: string; title: string };

/**
 * EnvironmentNotifications: each root thread's observation against the previous shell of the same environment (the
 * focused client's or a background one's). The first live shell only records. Attention notifies at once and replaces
 * the completion before it; a completion waits out the grace in `gate` and notifies once, and an observation that is
 * not settled meanwhile cancels it. Archived threads and stopped runs (latestRunStopRequested) notify no completion.
 * `now` is the shell's clock; a caller without one gets completions at once.
 */
export function threadTransitions(environment: { shell: Shell }, previous: Map<string, Memory> | null, gate: NotifyGate = notifyGate(), now = 0): { next: Map<string, Memory>; transitions: Transition[] } {
  const next = new Map<string, Memory>(), transitions: Transition[] = [], statuses = new Map<string, string>();
  noteSubagentRequests(environment.shell.threads); // subagents are skipped below, so their requests notify through the thread above them
  const cancel = (key: string) => { gate.pending.delete(key); };
  const skip = (key: string, at: number | null) => { if (at !== null && at > (gate.handled.get(key) ?? -Infinity)) gate.handled.set(key, at); };
  const transition = (id: string, kind: Transition['kind'], status: string) => transitions.push({ threadId: id, kind, status,
    title: kind === 'completion' ? 'Thread completed' : status === 'approval' ? 'Approval needed' : status === 'limited' ? 'Usage limit reached' : status === 'failed' ? 'Thread failed' : 'Input needed' });
  for (const thread of environment.shell.threads) {
    if (!isListedThreadLineage(thread.lineage)) continue; // isShownUnderParent: subagents and side chats
    const id = str(thread.id);
    let status: string = sidebarStatus(thread);
    const run = latestRun(thread);
    if (status === 'ready' && run?.status === 'failed') status = 'failed';
    const prior = previous?.get(id);
    const attention = ['input', 'approval', 'failed', 'limited'].includes(status) ? `${str(thread.latestRunId)}:${status}` : null;
    const completedAt = Date.parse(run?.completedAt ?? '');
    const settled = status === 'ready' && run?.status === 'completed';
    const observed: Memory = { attention, settled, completedAt: settled && Number.isFinite(completedAt) ? completedAt : null };
    next.set(id, observed); statuses.set(id, status);
    const archived = thread.archivedAt != null, quiet = thread.latestRunStopRequested === true;
    if (observed.settled && observed.completedAt !== null) gate.lastSettled.set(id, observed.completedAt);
    if (!observed.settled || archived || quiet) cancel(id);
    if (!prior || archived || quiet) skip(id, observed.completedAt);
    if (!prior || archived) continue;
    if (attention && attention !== prior.attention) {
      skip(id, gate.lastSettled.get(id) ?? null);
      cancel(id);
      transition(id, 'input', status);
      continue;
    }
    const at = observed.completedAt;
    // A completion the previous shell already showed settled was taken then (scheduled, notified or skipped).
    if (!observed.settled || at === null || at <= (gate.handled.get(id) ?? -Infinity) || gate.pending.get(id)?.at === at || (prior.settled && prior.completedAt === at)) continue;
    // A caller without a clock (now 0) has no grace to wait out.
    if (!(now > 0)) { gate.handled.set(id, at); transition(id, 'completion', status); continue; }
    gate.pending.set(id, { at, seen: now, due: 0 });
  }
  // retain: threads that left the snapshot are forgotten.
  for (const map of [gate.pending, gate.handled, gate.lastSettled]) for (const key of [...map.keys()]) if (!next.has(key)) map.delete(key);
  for (const [id, wait] of [...gate.pending]) {
    if (!wait.due && now > wait.seen) wait.due = now + WORKING_EXIT_GRACE_MS;
    if (!wait.due || now < wait.due) continue;
    gate.pending.delete(id);
    gate.handled.set(id, wait.at);
    transition(id, 'completion', statuses.get(id) ?? 'ready');
  }
  return { next, transitions };
}

/** Toast type and leading glyph for a transition (ThreadNotificationCoordinator). */
export function transitionToast(transition: Transition) {
  const kind = transition.kind === 'completion' ? 'success' as const : transition.status === 'failed' ? 'error' as const : 'warning' as const;
  const leading = transition.kind === 'completion' ? 'circle-check:success-foreground'
    : transition.status === 'approval' ? 'shield-question:warning-foreground'
      : transition.status === 'failed' ? 'circle-alert:destructive-foreground' : 'message-circle-question:info-foreground';
  return { kind, leading };
}

/** One watched environment: its id, its shell, whether that shell is live, and whether it is the window's focus. */
export type WatchedEnvironment = { environmentId: string; shell: Shell; live: boolean; focused: boolean };

/**
 * ThreadNotificationCoordinator's environment list: the focused environment (T3Client's own
 * connection) and every background one the fleet keeps (settings-b-fleet.ts, the primary among
 * them whenever it is not the focus). A background shell is live once its transport is
 * connected and synchronized for its generation, as `environmentShell`'s "live" status.
 */
export function notifyEnvironments(client: T3Client, source: EnvironmentFleet = fleet): WatchedEnvironment[] {
  const watched: WatchedEnvironment[] = [];
  if (client.environmentId) watched.push({ environmentId: client.environmentId, shell: client.shell, live: client.ready, focused: true });
  for (const entry of source.entries.values()) {
    if (!entry.environmentId || entry.environmentId === client.environmentId) continue;
    watched.push({ environmentId: entry.environmentId, shell: entry.shell, live: entry.phase === 'connected' && entry.synchronized === entry.generation, focused: false });
  }
  return watched;
}

/**
 * ThreadNotificationCoordinator: one EnvironmentNotifications per environment. Each keeps its own
 * memory, cleared while its shell is not live and dropped when the environment leaves. A toast's
 * "Open thread" and a clicked notification name the thread with its environment
 * (`fleet:<environment>:<thread>`, the reference's /$environmentId/$threadId route), which
 * `select-thread` opens in place when that environment is the focus and focuses it otherwise.
 * Every environment is diffed and remembered before the first notification is awaited, so a
 * later call that runs during those awaits diffs a newer shell against memory no older than the
 * shells this call read, and this call never writes an older shell's memory over a newer one.
 */
export async function threadNotifications(client: T3Client, status: NotifyStatus, source: EnvironmentFleet = fleet, now = 0): Promise<void> {
  const state = notifyState(client);
  const prefs = client.local.clientSettings;
  const mode = str(prefs.notificationMode, 'off'), inApp = prefs.inAppNotificationsEnabled === true;
  const modeChanged = state.mode !== mode;
  state.mode = mode;
  // applyMode clears what is pending, and the window's `focus` clears it while desktop notifications are on.
  if (modeChanged || status.active) await clearPending(client, modeChanged);
  if (mode === 'off' && !inApp) { state.previous.clear(); state.gates.clear(); return; }
  const environments = notifyEnvironments(client, source);
  const listed = new Set(environments.map(environment => environment.environmentId));
  for (const environmentId of [...state.previous.keys()]) if (!listed.has(environmentId)) { state.previous.delete(environmentId); state.gates.delete(environmentId); }
  const pending: { environment: WatchedEnvironment; transition: Transition }[] = [];
  for (const environment of environments) {
    if (!environment.live) { state.previous.delete(environment.environmentId); state.gates.delete(environment.environmentId); continue; }
    let gate = state.gates.get(environment.environmentId);
    if (!gate) state.gates.set(environment.environmentId, gate = notifyGate());
    const { next, transitions } = threadTransitions(environment, state.previous.get(environment.environmentId) ?? null, gate, now);
    state.previous.set(environment.environmentId, next);
    for (const transition of transitions) pending.push({ environment, transition });
  }
  for (const { environment, transition } of pending) await notifyTransition(client, status, environment, transition, mode, inApp, now);
}

/** One transition: the sound, then the in-app toast (focused window, not the open thread) or the system notification. */
async function notifyTransition(client: T3Client, status: NotifyStatus, environment: WatchedEnvironment, transition: Transition, mode: string, inApp: boolean, now: number): Promise<void> {
  const thread = environment.shell.threads.find(entry => entry.id === transition.threadId);
  const body = str(thread?.title, 'Untitled thread');
  const target = fleetThreadId(environment.environmentId, transition.threadId);
  if (hasNotificationSound(mode)) notificationSound(client, transition.kind, now);
  if (inApp && status.active && !(environment.focused && transition.threadId === client.threadId)) {
    const { kind, leading } = transitionToast(transition);
    pushToast(client, { kind, title: transition.title, description: body, leading, hideCopy: true,
      action: { label: 'Open thread', op: 'select-thread', id: target } });
    return;
  }
  if (!hasDesktopNotifications(mode) || status.active) return;
  await postNotification(client, status, transition.title, body, `${environment.environmentId}:${transition.threadId}`, target);
}

// --- NotificationSettings permission gate ------------------------------------

const permissionMessages = new WeakMap<T3Client, string>();
export const PERMISSION_DENIED = 'Allow notifications in your browser or system settings, then choose this option again. Sound only is still available.';
/** The Thread notifications row's description while a permission request was refused. */
export function notificationPermissionMessage(client: T3Client): string { return permissionMessages.get(client) ?? ''; }
/**
 * NotificationSettings: before a notification mode is saved, `Notification.requestPermission()`, which asks only
 * while the person has not decided. Refused or undecided keeps the previous value and explains why.
 */
export async function notificationPermission(client: T3Client, mode: string): Promise<boolean> {
  permissionMessages.delete(client);
  if (!hasDesktopNotifications(mode)) return true;
  if ((await quietly(() => notifications.requestNotificationPermission({})))?.state === 'granted') return true;
  permissionMessages.set(client, PERMISSION_DENIED);
  return false;
}
