// Thread and provider notifications (MIT reference: components/
// ThreadNotificationCoordinator.tsx, threadNotifications.ts,
// ProviderUpdatePrimaryNotification.tsx, ProviderUpdateLaunchNotification
// .logic.ts, providerUpdateDismissal.ts). The window's focus is the page's
// (`exactPage().hasFocus`, document.hasFocus(): exact2 #219); the macOS side
// (notification center, sound, Dock badge, a click that opens the thread: #224)
// is modules/apple/T3Notifications.swift.
import type { T3Client } from './client';
import { pushToast, dismissToast } from './toast';
import { arr, obj, str, type Obj } from './domain';
import { bridgeReply, type Files, type Native } from './protocol';
import { sidebarStatus, latestRun } from './sidebar-model';
import { DRIVERS } from './providers-meta';
import { shellPrefs } from './shell-prefs';

export type NotifyStatus = { active: boolean; authorization: string; agent: boolean; opened: string; openedThread: string };
type Memory = { attention: string | null; completion: number | null };
type NotifyState = { previous: Map<string, Memory> | null; environmentId: string; mode: string;
  seenUpdateKeys: Set<string>; dismissedUpdateKeys: Set<string> | null; activeUpdate: { key: string; toastId: number } | null };
const states = new WeakMap<T3Client, NotifyState>();
function notifyState(client: T3Client): NotifyState {
  let state = states.get(client);
  if (!state) { state = { previous: null, environmentId: '', mode: '', seenUpdateKeys: new Set(), dismissedUpdateKeys: null, activeUpdate: null }; states.set(client, state); }
  return state;
}

export const hasNotificationSound = (mode: string) => mode === 'sound' || mode === 'notifications-and-sound';
export const hasDesktopNotifications = (mode: string) => mode === 'notifications' || mode === 'notifications-and-sound';

/**
 * The notification center as the native module sees it; `active` is the page's
 * own focus fact (`exactPage().hasFocus`, ThreadNotificationCoordinator's
 * document.hasFocus()), never the module's.
 */
export async function nativeNotifyStatus(native: Native, previous: NotifyStatus, focused: boolean): Promise<NotifyStatus> {
  try {
    const response = await bridgeReply(native, { op: 'notifyStatus' });
    if (!response.ok) return { ...previous, active: focused };
    const value = obj(response.value);
    return { active: focused, authorization: str(value.authorization, 'unknown'), agent: value.agent === true,
      opened: str(value.opened), openedThread: str(value.openedThread) };
  } catch { return { ...previous, active: focused }; }
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
 * EnvironmentNotifications: diff each root thread's attention key and latest
 * completion against the previous shell. The first live shell only records.
 */
export function threadTransitions(client: T3Client, previous: Map<string, Memory> | null): { next: Map<string, Memory>; transitions: Transition[] } {
  const next = new Map<string, Memory>(), transitions: Transition[] = [];
  for (const thread of client.shell.threads) {
    if (obj(thread.lineage).relationshipToParent === 'subagent') continue;
    const id = str(thread.id);
    let status: string = sidebarStatus(thread);
    const run = latestRun(thread);
    if (status === 'ready' && run?.status === 'failed') status = 'failed';
    const prior = previous?.get(id);
    const attention = ['input', 'approval', 'failed', 'limited'].includes(status) ? `${str(thread.latestRunId)}:${status}` : null;
    const completedAt = Date.parse(run?.completedAt ?? '');
    // Waiting only on commands (a dev server) is done; subagents and monitors wake the agent.
    const settled = status === 'ready' || (status === 'waiting' && !arr(thread.pendingBackgroundTasks).some(task => str(task.kind) !== 'command'));
    const completion = settled && run?.status === 'completed' && Number.isFinite(completedAt) ? completedAt : prior?.completion ?? null;
    next.set(id, { attention, completion });
    if (!prior || thread.archivedAt != null) continue;
    const kind = attention && attention !== prior.attention ? 'input'
      : completion !== null && (prior.completion === null || completion > prior.completion) ? 'completion' : null;
    if (!kind) continue;
    const title = kind === 'completion' ? 'Thread completed' : status === 'approval' ? 'Approval needed'
      : status === 'limited' ? 'Usage limit reached' : status === 'failed' ? 'Thread failed' : 'Input needed';
    transitions.push({ threadId: id, kind, status, title });
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

export async function threadNotifications(client: T3Client, native: Native, status: NotifyStatus): Promise<void> {
  const state = notifyState(client);
  const prefs = client.local.clientSettings;
  const mode = str(prefs.notificationMode, 'off'), inApp = prefs.inAppNotificationsEnabled === true;
  if (state.mode !== mode) await bridgeReply(native, { op: 'notifyClear' }).catch(() => undefined);
  state.mode = mode;
  if (!client.ready || client.environmentId !== state.environmentId) {
    state.environmentId = client.ready ? client.environmentId : '';
    state.previous = null;
    if (!client.ready) return;
  }
  if (mode === 'off' && !inApp) { state.previous = null; return; }
  const { next, transitions } = threadTransitions(client, state.previous);
  state.previous = next;
  for (const transition of transitions) {
    const thread = client.shell.threads.find(entry => entry.id === transition.threadId);
    const body = str(thread?.title, 'Untitled thread');
    if (hasNotificationSound(mode)) await bridgeReply(native, { op: 'notifySound', kind: transition.kind }).catch(() => undefined);
    if (inApp && status.active && transition.threadId !== client.threadId) {
      const { kind, leading } = transitionToast(transition);
      pushToast(client, { kind, title: transition.title, description: body, leading, hideCopy: true,
        action: { label: 'Open thread', op: 'select-thread', id: transition.threadId } });
      continue;
    }
    if (!hasDesktopNotifications(mode) || status.active) continue;
    await bridgeReply(native, { op: 'notifyPost', title: transition.title, body,
      tag: `${client.environmentId}:${transition.threadId}`, threadId: transition.threadId }).catch(() => undefined);
  }
}

// --- NotificationSettings permission gate ------------------------------------

const permissionMessages = new WeakMap<T3Client, string>();
export const PERMISSION_DENIED = 'Allow notifications in your browser or system settings, then choose this option again. Sound only is still available.';
/** The Thread notifications row's description while a permission request was refused. */
export function notificationPermissionMessage(client: T3Client): string { return permissionMessages.get(client) ?? ''; }
/**
 * Before a notification mode is saved, ask macOS (never from an agent-launched
 * app). Refused or undecided keeps the previous value and explains why.
 */
export async function notificationPermission(client: T3Client, native: Native, mode: string): Promise<boolean> {
  permissionMessages.delete(client);
  if (!hasDesktopNotifications(mode)) return true;
  try {
    const response = await bridgeReply(native, { op: 'notifyAuthorize' });
    const authorization = str(obj(response.value).authorization);
    if (response.ok && (authorization === 'authorized' || authorization === 'provisional')) return true;
  } catch { /* unavailable: the message below says what to do */ }
  permissionMessages.set(client, PERMISSION_DENIED);
  return false;
}

// --- Provider update launch notification ---------------------------------

type Candidate = { driver: string; instanceId: string; latestVersion: string; oneClick: boolean };

/** isProviderUpdateCandidate, deduplicated by driver (the newest check wins). */
export function updateCandidates(providers: Obj[]): Candidate[] {
  const byDriver = new Map<string, Obj>();
  for (const provider of providers) {
    const version = obj(provider.versionAdvisory), compat = obj(provider.compatibilityAdvisory);
    if (provider.enabled === false || compat.latestVersionStatus === 'broken' || compat.latestVersionStatus === 'unsupported'
      || version.status !== 'behind_latest' || !str(version.latestVersion)) continue;
    const driver = str(provider.driver), current = byDriver.get(driver);
    if (!current || str(provider.checkedAt).localeCompare(str(current.checkedAt)) >= 0) byDriver.set(driver, provider);
  }
  return [...byDriver.values()].map(provider => {
    const version = obj(provider.versionAdvisory);
    return { driver: str(provider.driver), instanceId: str(provider.instanceId), latestVersion: str(version.latestVersion),
      oneClick: version.canUpdate === true && typeof version.updateCommand === 'string' && version.updateCommand !== '' };
  });
}
export function updateKey(candidates: Candidate[]): string {
  return candidates.map(candidate => `${candidate.driver}:${candidate.latestVersion}`).sort().join('|');
}
const driverName = (driver: string) => DRIVERS.find(entry => entry.id === driver)?.label ?? driver;
const formatVersion = (value: string) => value.startsWith('v') ? value : `v${value}`;
function providerList(candidates: Candidate[]): string {
  const names = candidates.map(candidate => driverName(candidate.driver));
  return names.length <= 2 ? names.join(' and ') : `${names.slice(0, -1).join(', ')}, and ${names[names.length - 1]}`;
}
export function updateToastView(candidates: Candidate[]) {
  const oneClick = candidates.filter(candidate => candidate.oneClick);
  return {
    title: candidates.length === 1 ? `Update Available: ${driverName(candidates[0]!.driver)} ${formatVersion(candidates[0]!.latestVersion)}` : `Updates Available: ${candidates.length} providers`,
    description: oneClick.length > 0 ? 'Install the update now or review provider settings.' : `${providerList(candidates)} can be updated from provider settings.`,
    oneClick,
  };
}

// Dismissed update sets live in the client's one preference file (shell-prefs.ts):
// the native adapter writes no other path. shellView saves after a dismissal.

/** ProviderUpdatePrimaryNotification: one prompt per update set, remembered when dismissed. */
export async function providerUpdates(client: T3Client, _storage?: Files): Promise<void> {
  const state = notifyState(client);
  if (!client.ready || !client.preferencesLoaded) return;
  if (!state.dismissedUpdateKeys) state.dismissedUpdateKeys = new Set(shellPrefs(client).providerUpdateDismissals);
  const candidates = updateCandidates(arr(client.config.providers));
  const key = candidates.length ? updateKey(candidates) : '';
  if (state.activeUpdate && state.activeUpdate.key !== key) {
    dismissToast(client, state.activeUpdate.toastId); state.activeUpdate = null;
  }
  if (!key || state.dismissedUpdateKeys.has(key) || state.seenUpdateKeys.has(key) || state.activeUpdate) return;
  state.seenUpdateKeys.add(key);
  const view = updateToastView(candidates);
  const settings = { label: 'Settings', op: 'ui:settings', id: 'providers' };
  const dismissed = state.dismissedUpdateKeys;
  const toastId = pushToast(client, { kind: 'warning', title: view.title, description: view.description, timeoutMs: 0, stacked: true,
    actionVariant: 'outline', hideCopy: true, key: 'provider-update',
    ...(candidates.length === 1 ? { leading: `provider:${candidates[0]!.driver}` } : {}),
    ...(view.oneClick.length ? { action: { label: 'Update', op: 'shell:provider-update', id: view.oneClick.map(candidate => candidate.instanceId).join(',') }, secondary: settings } : { action: settings }),
    onClose: () => { dismissed.add(key); shellPrefs(client).providerUpdateDismissals = [...dismissed].slice(-100); } });
  state.activeUpdate = { key, toastId };
}
