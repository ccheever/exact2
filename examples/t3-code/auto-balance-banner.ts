// The multi-machine update banner of an Auto balance draft (T3 Code MIT, see LICENSE-T3;
// reference 1e2ecbd975: apps/web/src/components/chat/useAutoBalanceUpdateBanner.tsx, and
// ServerUpdateAction.tsx ServerUpdatesAction, ServerUpdateProgress and the manual paths of
// ServerUpdateAction). It reuses server-update.ts: the per-environment job table is the update
// store, `updateEnvironment` the single-flight start, `announceServerUpdates` the result toasts.
// Port changes: the popover's rows are data (`machines`) the composer notice draws; a
// desktop-app batch asks through the confirm dialog server-update-banner added (su:confirm runs
// the held batch); the dismissed notices are the persisted dismissal store (the reference's
// component state only keeps a remount key, `dismissals` here).
import { obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import { fleet, type EnvironmentFleet } from './settings-b-fleet';
import { allJobs, type OutdatedJob } from './settings-b-outdated';
import { buildVersionMismatchDismissalKey, dismissServerUpdateFailure, dismissVersionMismatch, isServerUpdateFailureDismissed,
  isVersionMismatchDismissed, resolveServerConfigVersionMismatch, type DismissalStore } from './version-skew';
import { DESKTOP_MANAGED_NOTE, manualUpdateCopy } from './server-installation';
import { confirmDialogCopy, isServerUpdatePending, serverUpdateActionLabel, serverUpdateStageLabel, serverUpdateStateFor, updateEnvironment, updateTargetFromConfig,
  type ServerUpdateState, type ServerUpdateTarget, type UpdateDeps } from './server-update';
import { environmentOptions } from './r4-git-env';
import { autoBalanceState } from './auto-balance';
import { dismissals, focusedKey } from './server-update-notices';
import type { SystemNotice } from './server-update-notices';

/** One machine the Auto draft could run on, with its server config and connection. */
export type BannerEnvironment = { environmentId: string; key: string; label: string; config: Obj; connected: boolean };
export type BannerMachine = ServerUpdateTarget & { connected: boolean; remoteUpdate: boolean; state: ServerUpdateState; dismissKey: string | null };

/** useAutoBalanceUpdateBanner's machine list: every machine with an update notice that is not dismissed. */
export function autoBalanceUpdateMachines(environments: readonly BannerEnvironment[], store: DismissalStore,
  jobs: Readonly<Record<string, OutdatedJob>> = allJobs()): BannerMachine[] {
  return environments.flatMap(environment => {
    const mismatch = resolveServerConfigVersionMismatch(environment.config);
    const serverVersion = str(obj(obj(environment.config).environment).serverVersion) || null;
    const state = serverUpdateStateFor(environment.environmentId, serverVersion, jobs);
    const dismissKey = mismatch ? buildVersionMismatchDismissalKey(environment.environmentId, mismatch) : null;
    if (state.status === 'idle' ? !mismatch || isVersionMismatchDismissed(dismissKey, store) : isServerUpdateFailureDismissed(state)) return [];
    const target = updateTargetFromConfig(environment.config, environment.key, environment.environmentId, environment.label);
    const remoteUpdate = target.selfUpdate !== null && (target.selfUpdate !== 'desktop-managed' || target.desktopAppUpdate === true);
    return [{ ...target, targetVersion: state.status === 'idle' ? mismatch!.clientVersion : state.targetVersion,
      connected: environment.connected, remoteUpdate, state, dismissKey }];
  });
}

/** A popover row (the notice draws it): name, then a progress, failure, manual or ready line. */
export type MachineRow = { id: string; label: string; status: string; line: string;
  /** The manual path: "Copy update command" / "Copy relaunch command", or the desktop sentence instead of a button. */
  copyLabel: string; note: string };
export function machineRow(machine: BannerMachine): MachineRow {
  const row = { id: machine.environmentId, label: machine.serverLabel, status: machine.state.status, line: '', copyLabel: '', note: '' };
  if (machine.state.status === 'failed') return { ...row, line: machine.state.message };
  if (machine.state.status === 'running') return { ...row, line: serverUpdateStageLabel(machine.state.stage) };
  if (!machine.remoteUpdate) {
    const desktopOnly = machine.selfUpdate === 'desktop-managed' && machine.desktopAppUpdate !== true;
    return { ...row, line: 'Manual update required', copyLabel: desktopOnly ? '' : serverUpdateActionLabel(machine), note: desktopOnly ? DESKTOP_MANAGED_NOTE : '' };
  }
  return { ...row, line: machine.connected ? `Ready to update to ${machine.targetVersion}` : 'Reconnect this machine to update' };
}

export type AutoBalanceBanner = SystemNotice & { machines: MachineRow[]; menuLabel: string };
/**
 * The banner item: "Update available for N machines" / "Updating N machines" / "Could not
 * update N machines" (N = running, else failed, else all), "N needs/need a manual update", and
 * Update all / Update K machines / Retry (none while one runs). `dismissals` counts dismissals,
 * the reference's remount key.
 */
export function autoBalanceUpdateBanner(machines: readonly BannerMachine[], dismissed = 0): AutoBalanceBanner | null {
  if (machines.length === 0) return null;
  const running = machines.filter(machine => machine.state.status === 'running').length;
  const failed = machines.filter(machine => machine.state.status === 'failed').length;
  const manual = machines.filter(machine => !machine.remoteUpdate).length;
  const targets = machines.filter(machine => machine.connected && machine.remoteUpdate && machine.state.status !== 'running');
  const count = running || failed || machines.length;
  const prefix = running ? 'Updating' : failed ? 'Could not update' : 'Update available for';
  const title = `${prefix} ${count} ${count === 1 ? 'machine' : 'machines'}`;
  const label = failed > 0 ? 'Retry' : targets.length === machines.length ? 'Update all'
    : `Update ${targets.length} ${targets.length === 1 ? 'machine' : 'machines'}`;
  return {
    id: `auto-balance-server-updates-${dismissed}`, variant: failed ? 'error' : 'default', priority: running ? 1 : 2,
    // ComposerServerUpdateIcon: a spinner while one runs, the red alert after a failure, else the download arrow.
    icon: running ? 'spinner' : failed ? 'circle-alert' : 'download', iconTone: failed ? 'error' : '', title,
    ...(manual > 0 ? { description: `${manual} ${manual === 1 ? 'needs' : 'need'} a manual update` } : {}),
    ...(running === 0 && targets.length > 0 ? { action: 'ab:update-all', actionLabel: label } : {}),
    dismissLabel: 'Dismiss update notice', ...(running ? {} : { dismiss: 'ab:dismiss' }),
    machines: machines.map(machineRow), menuLabel: `${title}. View machines`,
  };
}

// ── ServerUpdatesAction ───────────────────────────────────────────────────
/** Machines that can update from here; the manual ones stay in the machine list. */
export const eligibleTargets = (targets: readonly ServerUpdateTarget[]) =>
  targets.filter(target => target.selfUpdate !== null && (target.selfUpdate !== 'desktop-managed' || target.desktopAppUpdate === true));
export const desktopAppsConfirmMessage = (labels: readonly string[]) =>
  `Update the T3 Code desktop apps on ${labels.join(', ')}? They will close and relaunch on those machines.`;
const batches = new WeakSet<object>();
/** isPending: the action is disabled while its batch starts. */
export const updatesPending = (owner: object) => batches.has(owner);
/**
 * handleUpdate: one confirmation for every desktop app (undefined means proceed), then each
 * machine through the single-flight update, its failure named by its label. A second click while
 * the batch starts does nothing.
 */
export async function updateServers(owner: object, targets: readonly ServerUpdateTarget[], deps: UpdateDeps,
  confirm: (message: string) => Promise<boolean | undefined>): Promise<'busy' | 'cancelled' | 'started'> {
  if (batches.has(owner)) return 'busy';
  batches.add(owner);
  try {
    const available = eligibleTargets(targets).filter(target => !isServerUpdatePending(target.environmentId));
    const desktop = available.filter(target => target.selfUpdate === 'desktop-managed');
    if (desktop.length > 0 && !((await confirm(desktopAppsConfirmMessage(desktop.map(target => target.serverLabel)))) ?? true)) return 'cancelled';
    await Promise.all(available.map(target => updateEnvironment(target, deps, `${target.serverLabel} update failed`)));
    return 'started';
  } finally { batches.delete(owner); }
}

// ── The focused client's banner ───────────────────────────────────────────
/** The Auto draft's machines: the logical project's, offline ones included ("Reconnect this machine to update"). */
export function autoBalanceEnvironments(client: T3Client, source: EnvironmentFleet = fleet): BannerEnvironment[] {
  const entries = [...source.entries.values()];
  return environmentOptions(client, source, true).map(option => {
    if (option.selected) return { environmentId: option.id, key: focusedKey(client), label: option.label, config: client.config, connected: client.connection === 'connected' };
    const entry = entries.find(candidate => candidate.environmentId === option.id);
    return { environmentId: option.id, key: str(entry?.key), label: option.label, config: entry?.config ?? {}, connected: entry?.phase === 'connected' };
  });
}
const dismissCount = new WeakMap<object, number>();
const heldBatches = new WeakMap<object, ServerUpdateTarget[]>();
/** The banner while the draft is on Auto (it replaces the single-machine notice). */
export function autoBalanceNotices(client: T3Client): AutoBalanceBanner[] {
  if (!autoBalanceState(client).automatic) return [];
  const banner = autoBalanceUpdateBanner(autoBalanceUpdateMachines(autoBalanceEnvironments(client), dismissals(client)), dismissCount.get(client) ?? 0);
  return banner ? [banner] : [];
}
/** The desktop-app batch waiting on the confirm dialog (server-update-notices.ts serverUpdateView reads it). */
export function heldBatchCopy(client: T3Client): { title: string; description: string } | null {
  const held = heldBatches.get(client);
  return held ? confirmDialogCopy(desktopAppsConfirmMessage(held.filter(target => target.selfUpdate === 'desktop-managed').map(target => target.serverLabel))) : null;
}
/** su:confirm / su:cancel for a held batch: true when there was one. */
export async function settleHeldBatch(client: T3Client, confirmed: boolean, deps: UpdateDeps): Promise<boolean> {
  const held = heldBatches.get(client);
  if (!held) return false;
  heldBatches.delete(client);
  if (confirmed) await updateServers(client, held, deps, async () => true);
  return true;
}

/** The banner's ops: `ab:update-all`, `ab:dismiss`, `ab:copy` (a manual machine's command). */
export async function autoBalanceBannerOp(client: T3Client, op: string, id: string, deps: UpdateDeps): Promise<boolean> {
  if (op !== 'ab:update-all' && op !== 'ab:dismiss' && op !== 'ab:copy') return false;
  const machines = autoBalanceState(client).automatic ? autoBalanceUpdateMachines(autoBalanceEnvironments(client), dismissals(client)) : [];
  if (op === 'ab:update-all') {
    const targets = machines.filter(machine => machine.connected && machine.remoteUpdate && machine.state.status !== 'running');
    if (machines.some(machine => machine.state.status === 'running')) return true;
    await updateServers(client, targets, deps, async () => { heldBatches.set(client, eligibleTargets(targets).filter(target => !isServerUpdatePending(target.environmentId))); return false; });
  } else if (op === 'ab:dismiss') {
    if (machines.some(machine => machine.state.status === 'running')) return true;
    for (const { state, dismissKey } of machines) {
      dismissServerUpdateFailure(state);
      dismissVersionMismatch(dismissKey, dismissals(client));
    }
    dismissCount.set(client, (dismissCount.get(client) ?? 0) + 1);
  } else {
    // ServerUpdateAction's manual path in a popover row: copy that machine's command.
    const machine = machines.find(candidate => candidate.environmentId === id);
    if (!machine || machine.selfUpdate !== null) return true;
    const manual = manualUpdateCopy(machine.targetVersion, machine.installation, machine.serverLabel);
    try { await deps.copy(manual.command); }
    catch { deps.toast({ kind: 'error', title: manual.failureTitle, description: manual.failureMessage }); return true; }
    deps.toast({ kind: 'success', title: manual.title, description: manual.description });
  }
  return true;
}
