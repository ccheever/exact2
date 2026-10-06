// The composer's system notices and the version-differ card (T3 Code MIT, see
// LICENSE-T3; reference 1e2ecbd975: apps/web/src/components/ChatView.tsx
// systemComposerBannerItems, the reconnect grace and the disconnect action;
// ChatView.logic.ts ENVIRONMENT_RECONNECT_WARNING_GRACE_MS and
// hasEnvironmentReconnectWarningGraceElapsed; hooks/useEnvironmentDisconnectDelay.ts;
// chat/ComposerServerUpdateStatus.tsx; chat/ThreadDetailsPanel.tsx's version card;
// chat/PanelLayoutControls.tsx threadPanelHasAttention; confirmDialog's
// requestConfirmDialog for a desktop-managed update).
// Port changes: data sources have no timers (EXACT2-GAPS X19), so the snapshot names
// the current reconnecting and unavailable episodes (`serverUpdate.reconnecting` /
// `.unavailable`, an environment id and a serial) and root tasks in app.contract wait
// 2 s and 20 s on them with `after`, handing the elapsed episode back as the
// snapshot's arguments. "Primary" is the loopback stand-in (isLoopback) until
// local-primary-environment replaces it: canDisconnectEnvironment is the one helper.
// Auto balance (the `automaticEnvironment` gate) belongs to the auto-balance task.
import { obj, str } from './domain';
import type { T3Client } from './client';
import { fleet, environmentKey, isLoopback } from './settings-b-fleet';
import { shellPrefs } from './shell-prefs';
import { buildVersionMismatchDismissalKey, isServerUpdateFailureDismissed, isVersionMismatchDismissed,
  resolveServerConfigVersionMismatch, serverUpdateGuidance, type DismissalStore } from './version-skew';
import { confirmDialogCopy, desktopUpdateConfirmMessage, manualDesktopOnly, serverUpdateActionLabel, serverUpdateStageLabel, serverUpdateStateFor,
  updateTargetFromConfig, type ServerUpdateState, type ServerUpdateTarget } from './server-update';

export const ENVIRONMENT_RECONNECT_WARNING_GRACE_MS = 2_000;
export const ENVIRONMENT_DISCONNECT_DELAY_MS = 20_000;

/** The grace belongs to the episode that elapsed, never to another environment or a later outage. */
export function hasEnvironmentReconnectWarningGraceElapsed(activeKey: string | null, elapsedKey: string | null): boolean {
  return activeKey !== null && activeKey !== '' && activeKey === elapsedKey;
}

// ── The clock (X19 workaround) ────────────────────────────────────────────
type Clock = { reconnecting: string; unavailable: string; serial: number; graceElapsed: string; delayElapsed: string };
const clocks = new WeakMap<object, Clock>();
const clockOf = (client: object): Clock => {
  let clock = clocks.get(client);
  if (!clock) { clock = { reconnecting: '', unavailable: '', serial: 0, graceElapsed: '', delayElapsed: '' }; clocks.set(client, clock); }
  return clock;
};
/** The snapshot's arguments: which episodes the root tasks saw last 2 s and 20 s long. */
export function noteServerUpdateClock(client: object, graceElapsed: string, delayElapsed: string): void {
  const clock = clockOf(client);
  clock.graceElapsed = graceElapsed; clock.delayElapsed = delayElapsed;
}
const isReconnecting = (client: T3Client) => !!client.environmentId && (client.connection === 'connecting' || client.connection === 'reconnecting');
export const isUnavailable = (client: T3Client) => !!client.environmentId && client.connection !== 'connected' && !switchedOff(client);
/** The current episodes, each a new key when it starts (the reference's effect resets on a new id). */
export function serverClock(client: T3Client): { reconnecting: string; unavailable: string } {
  const clock = clockOf(client), env = client.environmentId;
  const episode = (now: boolean, current: string) => !now ? '' : current.startsWith(`${env}#`) ? current : `${env}#${++clock.serial}`;
  clock.reconnecting = episode(isReconnecting(client), clock.reconnecting);
  clock.unavailable = episode(isUnavailable(client), clock.unavailable);
  return { reconnecting: clock.reconnecting, unavailable: clock.unavailable };
}

/** A saved environment the user switched off is hidden, not offline. */
const switchedOff = (client: T3Client) => fleet.saved.some(entry => str(entry.environmentId) === client.environmentId && entry.enabled === false);
/** Non-primary, non-local: the loopback stand-in decides "primary" (local-primary-environment replaces it). */
export const canDisconnectEnvironment = (origin: string) => !isLoopback(origin);

/** "server" with one environment, else "<label> server" (versionMismatchServerLabel). */
export function serverUpdateLabel(client: T3Client): string {
  const ids = new Set(fleet.saved.map(entry => str(entry.environmentId)).filter(Boolean));
  if (client.environmentId) ids.add(client.environmentId);
  if (ids.size <= 1) return 'server';
  const saved = fleet.saved.find(entry => str(entry.environmentId) === client.environmentId);
  return `${str(saved?.label) || str(obj(client.config.environment).label) || client.environmentId} server`;
}
/** The saved key T3Fleet runs this environment's job under: its home origin, else the focused route. */
export function focusedKey(client: T3Client): string {
  const saved = fleet.saved.find(entry => str(entry.environmentId) === client.environmentId);
  return environmentKey(str(saved?.origin) || client.origin, client.environmentId);
}
export const dismissals = (client: T3Client): DismissalStore => shellPrefs(client);

type VersionNotice = { mismatch: ReturnType<typeof resolveServerConfigVersionMismatch>; key: string | null; shown: boolean };
export function versionNotice(client: T3Client): VersionNotice {
  const mismatch = resolveServerConfigVersionMismatch(client.config);
  const key = mismatch && client.environmentId ? buildVersionMismatchDismissalKey(client.environmentId, mismatch) : null;
  return { mismatch, key, shown: !!mismatch && !!key && !isVersionMismatchDismissed(key, dismissals(client)) };
}
export const updateState = (client: T3Client): ServerUpdateState =>
  serverUpdateStateFor(client.environmentId, str(obj(client.config.environment).serverVersion) || null);

/** What the composer's ComposerNotice needs from a system item (composer-controls-view.ts wraps it). */
export type SystemNotice = { id: string; title: string; variant: string; icon: string; priority: number; description?: string; action?: string; actionLabel?: string; actionTip?: string;
  action2?: string; action2Label?: string; action2Tip?: string; dismiss?: string; dismissLabel?: string; dismissId?: string; tip?: string; iconTone?: string; sep?: boolean; liveRole?: string };

/**
 * systemComposerBannerItems: "<label> is reconnecting|offline" (after the 2 s grace, never while an
 * update runs; Disconnect server after 20 s for a non-primary environment), then the server-version
 * notice: idle "Server update available", running "Updating <server> · Downloading…", failed
 * "Could not update <server> · <message>".
 */
export function systemComposerNotices(client: T3Client, options: { automaticEnvironment?: boolean } = {}): SystemNotice[] {
  const items: SystemNotice[] = [];
  if (!client.environmentId) return items;
  const state = updateState(client), running = state.status === 'running';
  const clock = clockOf(client), current = serverClock(client);
  const unavailable = isUnavailable(client), reconnecting = unavailable && isReconnecting(client);
  const graceElapsed = hasEnvironmentReconnectWarningGraceElapsed(current.reconnecting || null, clock.graceElapsed || null);
  const delayElapsed = hasEnvironmentReconnectWarningGraceElapsed(current.unavailable || null, clock.delayElapsed || null);
  const disconnect = unavailable && delayElapsed && canDisconnectEnvironment(client.origin)
    ? { op: 'su:disconnect', label: 'Disconnect server', tip: "Hide this server's threads. Switch it on again in Connections." } : null;
  const label = str(obj(client.config.environment).label).trim() || 'T3 server';
  if (unavailable && !(reconnecting && (running || !graceElapsed))) {
    items.push({ id: `environment-unavailable:${client.environmentId}`, variant: client.connection === 'error' ? 'error' : 'warning', icon: 'wifi-off', priority: 2,
      title: `${label} is ${reconnecting ? 'reconnecting' : 'offline'}`,
      ...(!reconnecting ? { action: 'su:reconnect', actionLabel: 'Reconnect' } : {}),
      ...(disconnect ? { action2: disconnect.op, action2Label: disconnect.label, action2Tip: disconnect.tip } : {}) });
  }
  const version = versionNotice(client), failureDismissed = isServerUpdateFailureDismissed(state);
  if (options.automaticEnvironment || !(state.status === 'idle' ? version.shown : !failureDismissed)) return items;
  const serverLabel = serverUpdateLabel(client), failed = state.status === 'failed', target = updateTargetFromConfig(client.config, '', client.environmentId, serverLabel);
  const notice: SystemNotice = { id: `server-version:${client.environmentId}`, variant: failed ? 'error' : 'default', priority: running ? 1 : 2,
    icon: running ? 'spinner' : failed ? 'circle-alert' : 'download', iconTone: failed ? 'error' : '', title: 'Server update available' };
  if (state.status !== 'idle') {
    const title = `${failed ? 'Could not update' : 'Updating'} ${serverLabel}`, detail = state.status === 'failed' ? state.message : serverUpdateStageLabel(state.stage);
    Object.assign(notice, { title, description: detail, sep: true, tip: `${title}: ${detail}`, liveRole: failed ? 'alert' : 'status' });
  } else if (version.mismatch) {
    notice.tip = `${serverLabel} ${version.mismatch.serverVersion} → ${version.mismatch.clientVersion}`;
    if (target.selfUpdate !== null && !(target.selfUpdate === 'desktop-managed' && target.desktopAppUpdate)) notice.description = serverUpdateGuidance(target.selfUpdate);
  }
  if (running) { if (disconnect) Object.assign(notice, { action: disconnect.op, actionLabel: disconnect.label, actionTip: disconnect.tip }); }
  else if (version.mismatch && !manualDesktopOnly(target)) Object.assign(notice, { action: 'su:update', actionLabel: serverUpdateActionLabel(target, failed ? 'Retry' : 'Update') });
  if (!running && (failed || version.key)) Object.assign(notice, { dismiss: 'su:dismiss', dismissLabel: 'Dismiss update notice', dismissId: failed ? state.attempt ?? '' : '' });
  items.push(notice);
  return items;
}

/** The details card's version-differ warning and the toggle's attention dot (ShellDetails / Snapshot fields). */
export function versionCard(client: T3Client): { versionClient: string; versionServer: string; versionLabel: string } {
  const version = versionNotice(client);
  return version.shown && version.mismatch
    ? { versionClient: version.mismatch.clientVersion, versionServer: version.mismatch.serverVersion, versionLabel: serverUpdateLabel(client) }
    : { versionClient: '', versionServer: '', versionLabel: '' };
}

// ── The desktop-managed confirmation ───────────────────────────────────────
export const confirms = new WeakMap<object, ServerUpdateTarget>();
/** The snapshot's `serverUpdate` row: the episodes for the root tasks, the dot, the confirm dialog. */
export function serverUpdateView(client: T3Client) {
  const clock = serverClock(client), confirm = confirms.get(client), copy = confirm ? confirmDialogCopy(desktopUpdateConfirmMessage(confirm.serverLabel)) : null;
  return { reconnecting: clock.reconnecting, unavailable: clock.unavailable, attention: isUnavailable(client) || versionNotice(client).shown,
    confirmTitle: copy?.title ?? '', confirmBody: copy?.description ?? '' };
}
