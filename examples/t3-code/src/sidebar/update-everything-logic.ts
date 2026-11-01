// Update everything's pure logic, T3 Code 2359643cff (MIT, see LICENSE-T3):
// apps/web/src/components/sidebar/updateEverything.logic.ts and packages/shared/src/cliRelease.ts
// cliReleaseChannelOf.
// Port changes: a machine's server config is the server's JSON (`Obj`), and a server target is
// server-update.ts's (`updateTargetFromConfig`, which also carries the saved environment's key the
// transport runs the job under). The fleet's Stable and Nightly switch (collectChannelSwitchTargets) is not ported;
// awaitsTrackSwitch counts the machines it would move, for About's Update track.
import { obj, str, type Obj } from '../client/domain';
import { resolveServerSelfUpdateCapability, resolveVersionMismatch, supportsDesktopAppUpdate } from '../connections/version-skew';
import { updateTargetFromConfig, type ServerUpdateState, type ServerUpdateTarget } from '../connections/server-update';

export interface UpdateEverythingMachine {
  readonly environmentId: string;
  /** The saved environment's key (`origin\nid`) a server update runs under. */
  readonly key: string;
  readonly label: string;
  readonly serverConfig: Obj | null;
  readonly connected: boolean;
  readonly hostedByThisApp: boolean;
  readonly canMaintain: boolean;
  readonly updateState: ServerUpdateState;
}

/** cliReleaseChannelOf (packages/shared/src/cliRelease.ts). */
export function cliReleaseChannelOf(version: string): 'stable' | 'nightly' | 'preview' {
  const channel = /^[^-+]+-(nightly|preview)\.\d{8}\.\d+$/.exec(version)?.[1];
  return channel === 'nightly' || channel === 'preview' ? channel : 'stable';
}

const environmentOf = (config: Obj) => obj(config.environment);

export interface RestartingMachine { readonly environmentId: string; readonly continuesThreads: boolean }

export function collectRestartingMachines(machines: readonly UpdateEverythingMachine[], servers: readonly Pick<ServerUpdateTarget, 'environmentId'>[],
  installsLocal: boolean): RestartingMachine[] {
  const updated = new Set(servers.map(server => server.environmentId));
  return machines.flatMap(machine => {
    const config = machine.serverConfig;
    if (config === null || !machine.connected || !(updated.has(machine.environmentId) || (installsLocal && machine.hostedByThisApp))) return [];
    return [{ environmentId: machine.environmentId,
      continuesThreads: obj(environmentOf(config).capabilities).threadRestartContinuation === true && obj(config.settings).continueThreadsAfterServerUpdate === true }];
  });
}

export interface SkippedMachine { readonly environmentId: string; readonly label: string; readonly reason: string }

/**
 * Skip current and app-hosted servers; the latter update with the app and would relaunch mid-run.
 * Desktop-managed servers use their own feed. Each server's updater checks that it can install the
 * release before it restarts, so no registry precheck runs here.
 */
export function collectServerUpdateTargets(machines: readonly UpdateEverythingMachine[], targetVersion: string):
  { targets: ServerUpdateTarget[]; skipped: SkippedMachine[] } {
  const targets: ServerUpdateTarget[] = [];
  const skipped: SkippedMachine[] = [];
  for (const machine of machines) {
    const config = machine.serverConfig;
    if (machine.hostedByThisApp) continue;
    if (config !== null && resolveVersionMismatch(str(environmentOf(config).serverVersion) || null, targetVersion) === null) continue;
    const skip = (reason: string) => skipped.push({ environmentId: machine.environmentId, label: machine.label, reason });
    const unavailable = unavailableReason(machine);
    if (config === null || unavailable !== null) { skip(unavailable ?? 'not connected'); continue; }
    const selfUpdate = resolveServerSelfUpdateCapability(config);
    const desktopAppUpdate = supportsDesktopAppUpdate(config);
    if (selfUpdate === null) { skip('needs a manual update'); continue; }
    if (selfUpdate === 'desktop-managed' && !desktopAppUpdate) { skip('needs a newer desktop app'); continue; }
    const serverVersion = str(environmentOf(config).serverVersion);
    if (selfUpdate !== 'desktop-managed' && cliReleaseChannelOf(serverVersion) !== cliReleaseChannelOf(targetVersion)) {
      skip(`on the ${cliReleaseChannelOf(serverVersion)} channel`);
      continue;
    }
    targets.push({ ...updateTargetFromConfig(config, machine.key, machine.environmentId, machine.label), targetVersion });
  }
  return { targets, skipped };
}

/** channelSwitchLabel: a track's name. */
export const channelSwitchLabel = (channel: 'latest' | 'nightly') => (channel === 'nightly' ? 'Nightly' : 'Stable');
/** desktopUpdateChannelOf (packages/shared/src/cliRelease.ts): nightly for an `x-nightly.YYYYMMDD.N` version, else latest. */
const trackOf = (version: string): 'latest' | 'nightly' => (/^[^-+]+-nightly\.\d{8}\.\d+$/.test(version) ? 'nightly' : 'latest');
/**
 * awaitsTrackSwitch: a remote host on the other track that a switch can still move (trackSwitchMove with another
 * track: not a preview build, able to update itself, and a desktop-managed one only with the channel switch and not
 * on a Mac).
 */
export function awaitsTrackSwitch(machine: Pick<UpdateEverythingMachine, 'serverConfig' | 'hostedByThisApp'>, channel: 'latest' | 'nightly'): boolean {
  const config = machine.serverConfig;
  if (machine.hostedByThisApp || config === null) return false;
  const environment = environmentOf(config), version = str(environment.serverVersion);
  if (cliReleaseChannelOf(version) === 'preview') return false;
  if (trackOf(version) === channel) return false;
  const selfUpdate = resolveServerSelfUpdateCapability(config);
  if (selfUpdate === null) return false;
  return selfUpdate !== 'desktop-managed' || (obj(environment.capabilities).desktopAppChannelSwitch === true && str(obj(environment.platform).os) !== 'darwin');
}

function unavailableReason(machine: UpdateEverythingMachine): string | null {
  if (machine.serverConfig === null || !machine.connected) return 'not connected';
  if (!machine.canMaintain) return 'no permission';
  if (machine.updateState.status === 'running') return 'already updating';
  return null;
}

export interface UpdateEverythingSteps<Providers, Servers> {
  readonly downloadLocal?: () => Promise<boolean>;
  readonly updateProviders: () => Promise<Providers>;
  readonly updateServers: () => Promise<Servers>;
}

/**
 * Providers finish before any server restarts, so no install is cut off. The caller installs this app afterwards when `localReady`, because
 * the relaunch ends the run; a failed download skips only that install.
 */
export async function runUpdateEverything<Providers, Servers>(steps: UpdateEverythingSteps<Providers, Servers>): Promise<{ localReady: boolean; providers: Providers; servers: Servers }> {
  const [localReady, remote] = await Promise.all([
    steps.downloadLocal?.() ?? Promise.resolve(true),
    steps.updateProviders().then(async providers => ({ providers, servers: await steps.updateServers() })),
  ]);
  return { localReady, ...remote };
}

export type UpdateRunMachineStatus = 'providers' | 'waiting' | 'downloading' | 'restarting' | 'done' | 'failed';

export interface UpdateRunMachine {
  readonly environmentId: string;
  readonly label: string;
  readonly updatesProviders: boolean;
  readonly server: ServerUpdateTarget | null;
  readonly handedOffVersion: string | null;
  readonly startedAt: number;
  readonly result: 'done' | 'failed' | null;
  readonly failure: string | null;
  readonly providerFailure: string | null;
}

export interface UpdateEverythingRun {
  readonly localVersion: string | null;
  readonly phase: 'providers' | 'servers' | 'countdown' | 'installing';
  readonly restartAt: number | null;
  readonly machines: readonly UpdateRunMachine[];
}

export function resolveUpdateRunMachineStatus(machine: UpdateRunMachine, phase: UpdateEverythingRun['phase'], serverState: ServerUpdateState): UpdateRunMachineStatus {
  if (machine.result !== null) return machine.result;
  if (machine.handedOffVersion !== null) return 'restarting';
  if (phase === 'providers' && machine.updatesProviders) return 'providers';
  if (machine.server !== null && serverState.status === 'running') return serverState.stage === 'resuming' ? 'restarting' : 'downloading';
  return 'waiting';
}

export function isServerUpdateHandedOff(state: ServerUpdateState): boolean {
  return state.status === 'running' && state.stage === 'resuming';
}

export function closeFinishedUpdateRun(run: UpdateEverythingRun): UpdateEverythingRun | null {
  return run.phase === 'servers' && run.localVersion === null && run.machines.every(machine => machine.result === 'done') ? null : run;
}

export const UPDATE_SLOW_AFTER_MS = 3 * 60 * 1000;

export function formatUpdateElapsed(ms: number): string {
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 1) return '<1m';
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

export interface UpdateRunMachineView { readonly machine: UpdateRunMachine; readonly status: UpdateRunMachineStatus }

export type UpdateRunLocalState = { readonly status: 'downloading'; readonly percent: number | null } | { readonly status: 'waiting' } | null;

export interface UpdateCapsuleView {
  readonly tone: 'default' | 'slow' | 'failed';
  readonly label: string;
  readonly slow?: { readonly status: string; readonly elapsed: string };
  readonly action: 'later' | 'retry' | 'dismiss' | null;
}

const isActive = (status: UpdateRunMachineStatus) => status !== 'done' && status !== 'failed';

/** The machine the capsule names once it passes UPDATE_SLOW_AFTER_MS: the longest-running one that is not waiting. */
export function slowestUpdateRunMachine(machines: readonly UpdateRunMachineView[]): UpdateRunMachineView | undefined {
  const active = machines.filter(({ status }) => isActive(status) && status !== 'waiting');
  return [...active].sort((a, b) => a.machine.startedAt - b.machine.startedAt)[0];
}

export function describeUpdateCapsule(run: UpdateEverythingRun, machines: readonly UpdateRunMachineView[], local: UpdateRunLocalState, now: number): UpdateCapsuleView {
  if (run.phase === 'countdown' && run.restartAt !== null) {
    const seconds = Math.max(0, Math.ceil((run.restartAt - now) / 1000));
    return { tone: 'default', label: `Restart in ${seconds}s`, action: 'later' };
  }
  if (run.phase === 'installing') return { tone: 'default', label: 'Restarting…', action: null };
  const slowest = slowestUpdateRunMachine(machines);
  if (slowest && now - slowest.machine.startedAt >= UPDATE_SLOW_AFTER_MS) {
    return { tone: 'slow', label: slowest.machine.label,
      slow: { status: RUN_STATUS_LABELS[slowest.status], elapsed: formatUpdateElapsed(now - slowest.machine.startedAt) }, action: null };
  }
  const done = machines.filter(({ status }) => status === 'done').length;
  if (machines.some(({ status }) => isActive(status))) return { tone: 'default', label: `${done} of ${machines.length} updated`, action: null };
  if (local?.status === 'downloading') {
    return { tone: 'default', label: `Downloading${local.percent === null ? '' : ` ${Math.floor(local.percent)}%`}`, action: null };
  }
  const failed = machines.filter(({ status }) => status === 'failed');
  if (failed.length > 0) {
    return { tone: 'failed', label: failed.length === 1 ? `${failed[0]!.machine.label} failed` : `${failed.length} failed`,
      action: failed.some(({ machine }) => machine.server !== null) ? 'retry' : 'dismiss' };
  }
  return { tone: 'default', label: `${done} of ${machines.length} updated`, action: null };
}

export const RUN_STATUS_LABELS: Record<UpdateRunMachineStatus, string> = {
  providers: 'updating providers', waiting: 'waiting', downloading: 'downloading', restarting: 'restarting', done: 'done', failed: 'failed',
};

export interface UpdateEverythingResult {
  readonly localVersion: string | null;
  readonly updatedServers: number;
  readonly updatedProviders: number;
  readonly failures: readonly string[];
  readonly restarting: readonly string[];
}

/** useServerUpdate's outcome (server-update.ts ServerUpdateOutcome). */
export type ServerUpdateOutcome = { readonly status: 'updated' | 'skipped' } | { readonly status: 'failed'; readonly message: string };
/** getProviderUpdateRunOutcome's (provider-updates.ts). */
export type ProviderUpdateOutcome = { readonly status: 'interrupted' | 'updated' } | { readonly status: 'failed'; readonly line: string };

export function collectUpdateEverythingResult(input: {
  readonly localVersion: string | null;
  readonly localFailures: readonly string[];
  readonly servers: readonly { readonly label: string; readonly outcome: ServerUpdateOutcome }[];
  readonly providers: readonly ProviderUpdateOutcome[];
}): UpdateEverythingResult {
  return {
    localVersion: input.localVersion,
    updatedServers: input.servers.filter(({ outcome }) => outcome.status === 'updated').length,
    updatedProviders: input.providers.filter(outcome => outcome.status === 'updated').length,
    restarting: [],
    failures: [
      ...input.localFailures.map(failure => `This app: ${failure}`),
      ...input.servers.flatMap(({ label, outcome }) => outcome.status === 'failed' ? [`${label}: ${outcome.message}`] : []),
      ...input.providers.flatMap(outcome => outcome.status === 'failed' ? [outcome.line] : []),
    ],
  };
}

export function getUpdateEverythingToastView(result: UpdateEverythingResult, appVersion: string): { type: 'success' | 'error'; title: string; description?: string } | null {
  const parts = [result.localVersion !== null && result.localVersion === appVersion ? 'this app' : null, countLabel(result.updatedServers, 'server'),
    countLabel(result.updatedProviders, 'provider')].filter((part): part is string => part !== null);
  const restarting = result.restarting.length === 0 ? []
    : [`${joinList(result.restarting)} ${result.restarting.length === 1 ? 'is' : 'are'} still restarting.`];
  if (result.failures.length === 0) {
    if (parts.length === 0) return null;
    return { type: 'success', title: `Updated ${joinList(parts)}`, ...(restarting.length === 0 ? {} : { description: restarting.join('\n') }) };
  }
  return { type: 'error', title: parts.length === 0 ? 'Update failed' : `Updated ${joinList(parts)}`, description: [...result.failures, ...restarting].join('\n') };
}

export interface UpdateEverythingPlan {
  readonly localVersion: string | null;
  /** Machines whose servers this app hosts; this app's update restarts them. */
  readonly localMachines: readonly { readonly environmentId: string; readonly label: string }[];
  readonly servers: readonly Pick<ServerUpdateTarget, 'environmentId' | 'serverLabel' | 'selfUpdate' | 'targetVersion'>[];
  readonly providerMachines: readonly { readonly environmentId: string; readonly label: string; readonly providers: readonly string[] }[];
  readonly skipped: readonly SkippedMachine[];
}

export interface UpdateEverythingRow {
  readonly environmentId: string | null;
  readonly label: string;
  readonly app: 'this-app' | 'server' | 'desktop-app' | null;
  readonly skipped: string | null;
  readonly providers: readonly string[];
}

function countLabel(count: number, noun: string): string | null {
  return count === 0 ? null : `${count} ${noun}${count === 1 ? '' : 's'}`;
}

function joinList(parts: readonly string[]): string {
  if (parts.length <= 2) return parts.join(' and ');
  return `${parts.slice(0, -1).join(', ')}, and ${parts.at(-1)}`;
}

/** One row per machine: this app's machines first, then servers, provider-only, and skipped. */
export function describeUpdateEverything(plan: UpdateEverythingPlan): { summary: string; version: string | null; providerCount: number; rows: UpdateEverythingRow[] } {
  const providerCount = plan.providerMachines.reduce((count, machine) => count + machine.providers.length, 0);
  const summary = `Update ${joinList([plan.localVersion ? 'this app' : null, countLabel(plan.servers.length, 'server'), countLabel(providerCount, 'provider')]
    .filter((part): part is string => part !== null))}`;
  const rows = new Map<string | null, UpdateEverythingRow>();
  const update = (environmentId: string | null, label: string, change: Partial<UpdateEverythingRow>) => {
    const row = rows.get(environmentId) ?? { environmentId, label, app: null, skipped: null, providers: [] };
    rows.set(environmentId, { ...row, ...change });
  };
  if (plan.localVersion) {
    if (plan.localMachines.length === 0) update(null, 'This app', { app: 'this-app' });
    for (const machine of plan.localMachines) update(machine.environmentId, machine.label, { app: 'this-app' });
  }
  for (const server of plan.servers) update(server.environmentId, server.serverLabel, { app: server.selfUpdate === 'desktop-managed' ? 'desktop-app' : 'server' });
  for (const machine of plan.providerMachines) update(machine.environmentId, machine.label, { providers: machine.providers });
  for (const machine of plan.skipped) update(machine.environmentId, machine.label, { skipped: machine.reason });
  return { summary, version: plan.localVersion ?? plan.servers.find(server => server.selfUpdate !== 'desktop-managed')?.targetVersion ?? null,
    providerCount, rows: Array.from(rows.values()) };
}
