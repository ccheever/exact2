// The shared server-update state of every environment (T3 Code MIT, see LICENSE-T3;
// reference 1e2ecbd975: packages/client-runtime/src/state/server.ts ServerUpdateState,
// serverUpdateStateAtom and updateServer; apps/web/src/components/ServerUpdateAction.tsx
// useServerUpdate, ServerUpdateAction and UPDATE_STAGE_LABELS).
// Port changes: the update itself runs in T3Fleet.swift (T3OutdatedHosts, `mode:
// "connected"`): a socket of its own (naming protocol 2) carries `server.updateServerWithProgress` (else
// `server.updateServer`, a dropped boot-service/respawn socket being the handoff),
// a desktop-managed result is committed with `server.commitDesktopUpdate`, and the
// restart ends when the server's descriptor reports the target version (4 minutes),
// standing in for the lifecycle `ready` event (EXACT2-GAPS X21: no two-way socket in
// the data source). Its job table is the per-environment store: this module reads
// it, derives `ServerUpdateState`, and toasts each result once the focused (or
// background) connection reports the new version, which is when the reference's
// command resolves. The Settings rows, this banner and Update all share it.
import { obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { allJobs, readJobs, startUpdateJob, type OutdatedJob } from './settings-b-outdated';
import { manualUpdateCopy, configInstallation, type ServerInstallation } from './server-installation';
import { CLIENT_VERSION, supportsDesktopAppUpdate, supportsServerUpdateThreadContinuation, resolveServerSelfUpdateCapability } from './version-skew';

export type ServerUpdateStage = 'downloading' | 'installing' | 'resuming';
export type ServerUpdateState =
  | { readonly status: 'idle' }
  | { readonly status: 'running'; readonly stage: ServerUpdateStage; readonly fromVersion: string; readonly targetVersion: string; readonly attempt?: string }
  | { readonly status: 'failed'; readonly stage: ServerUpdateStage; readonly fromVersion: string; readonly targetVersion: string; readonly message: string; readonly attempt?: string };
export const IDLE_SERVER_UPDATE_STATE: ServerUpdateState = { status: 'idle' };

// ── server.ts helpers ─────────────────────────────────────────────────────
export type ServerSelfUpdateResult = { readonly targetVersion: string; readonly method: string; readonly updateId?: string; readonly desktopUpdateToken?: string };
export type ServerSelfUpdateProgressEvent = { readonly type: 'progress'; readonly stage: 'downloading' | 'installing' } | { readonly type: 'complete'; readonly result: ServerSelfUpdateResult };
export type ServerLifecycleReadyEvent = { readonly type: 'ready'; readonly payload: { readonly environment: { readonly serverVersion: string };
  readonly updateOutcome?: { readonly id: string; readonly status: 'committed' | 'rolled-back' | 'failed'; readonly targetVersion?: string; readonly reason?: string } } };

export function serverUpdateStateForProgressEvent(fromVersion: string, targetVersion: string, event: ServerSelfUpdateProgressEvent): Extract<ServerUpdateState, { status: 'running' }> {
  return { status: 'running', stage: event.type === 'complete' ? 'resuming' : event.stage, fromVersion, targetVersion };
}
/** A failure from another server version is stale; running and idle states pass through. */
export function serverUpdateStateForServerVersion(state: ServerUpdateState, serverVersion: string | null): ServerUpdateState {
  return state.status === 'idle' || state.status === 'running' || serverVersion === null || state.fromVersion === serverVersion ? state : IDLE_SERVER_UPDATE_STATE;
}
export function serverUpdateFailureMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Server update failed.';
}

/** One reason of an Effect Cause, as the clone's transport reports it (T3Fleet's `.dropped` is a socket error). */
export type UpdateFailureReason = { readonly kind: 'interrupt' } | { readonly kind: 'fail'; readonly error: unknown };
const SOCKET_ERRORS = ['SocketReadError', 'SocketWriteError', 'SocketCloseError'];
function isRpcSocketError(error: unknown): boolean {
  const value = obj(error);
  return value._tag === 'RpcClientError' && SOCKET_ERRORS.includes(str(obj(value.reason)._tag));
}
/** A transport loss (or an interrupt) is an unacknowledged handoff; an open failure or defect is not. */
export function isLegacyUpdateHandoffLoss(reasons: readonly UpdateFailureReason[]): boolean {
  if (reasons.length > 0 && reasons.every(reason => reason.kind === 'interrupt')) return true;
  return reasons.length > 0 && reasons.every(reason => reason.kind === 'fail' && isRpcSocketError(reason.error));
}

export function matchesServerUpdateReadyEvent(result: ServerSelfUpdateResult, event: ServerLifecycleReadyEvent): boolean {
  return result.updateId === undefined ? event.payload.environment.serverVersion === result.targetVersion : event.payload.updateOutcome?.id === result.updateId;
}
export function matchesServerUpdateResumeEvent(result: ServerSelfUpdateResult, event: ServerLifecycleReadyEvent): boolean {
  return (result.method === 'desktop-app' && result.desktopUpdateToken !== undefined) || matchesServerUpdateReadyEvent(result, event);
}
/** null when the resumed server committed the requested version, else the terminal error's message. */
export function validateServerUpdateReadyEvent(result: ServerSelfUpdateResult, event: ServerLifecycleReadyEvent): string | null {
  if (result.updateId === undefined) return null;
  const outcome = event.payload.updateOutcome;
  if (outcome?.id === result.updateId && outcome.status === 'committed' && outcome.targetVersion === result.targetVersion
    && event.payload.environment.serverVersion === result.targetVersion) return null;
  return outcome?.reason ?? (outcome ? `The t3@${result.targetVersion} update ${outcome.status}.`
    : 'The service launcher resumed without committing the requested server version.');
}
/**
 * nudgeReconnectDuringUpdateRestart's filter: during a known restart a backoff (or a
 * blocked credential) is retried about every second instead of climbing the ladder.
 */
export function nudgesReconnectDuringUpdateRestart(state: { readonly phase: string; readonly lastFailure?: { readonly reason: string } | null }): boolean {
  return state.phase === 'backoff' || (state.phase === 'blocked' && state.lastFailure?.reason === 'authentication');
}

// ── UPDATE_STAGE_LABELS ────────────────────────────────────────────────────
// The wire "installing" stage is a sub-second launcher handoff, folded into the download.
const UPDATE_STAGE_LABELS: Record<ServerUpdateStage, string> = { downloading: 'Downloading…', installing: 'Downloading…', resuming: 'Restarting…' };
export const serverUpdateStageLabel = (stage: ServerUpdateStage) => UPDATE_STAGE_LABELS[stage] ?? 'Downloading…';

// ── The store: T3Fleet's job table ────────────────────────────────────────
const asStage = (stage: string): ServerUpdateStage => stage === 'installing' || stage === 'resuming' ? stage : 'downloading';
/** A job as ServerUpdateState. A finished job waits as "Restarting…" until its connection reports the version. */
export function serverUpdateStateForJob(job: OutdatedJob | undefined): ServerUpdateState {
  if (!job) return IDLE_SERVER_UPDATE_STATE;
  const base = { stage: asStage(job.stage), fromVersion: job.fromVersion, targetVersion: job.targetVersion, attempt: job.attempt };
  if (job.status === 'running') return { status: 'running', ...base };
  if (job.status === 'done') return { status: 'running', ...base, stage: 'resuming' };
  if (job.status === 'failed') return { status: 'failed', ...base, message: job.message || 'Server update failed.' };
  return IDLE_SERVER_UPDATE_STATE;
}
const keyEnvironment = (key: string) => key.slice(key.indexOf('\n') + 1);
/** The connected-server job of one environment (any of its routes' keys). */
export function connectedJob(environmentId: string, jobs: Readonly<Record<string, OutdatedJob>> = allJobs()): [string, OutdatedJob] | undefined {
  if (!environmentId) return undefined;
  return Object.entries(jobs).find(([key, job]) => job.mode === 'connected' && keyEnvironment(key) === environmentId);
}
/** updateStateAtom(environmentId): the job's state, a stale failure dropped once the server version moved. */
export function serverUpdateStateFor(environmentId: string, serverVersion: string | null, jobs?: Readonly<Record<string, OutdatedJob>>): ServerUpdateState {
  const found = connectedJob(environmentId, jobs);
  const state = serverUpdateStateForJob(found?.[1]);
  // A finished update whose connection already reports the target is over.
  if (found?.[1].status === 'done' && serverVersion === found[1].targetVersion) return IDLE_SERVER_UPDATE_STATE;
  return serverUpdateStateForServerVersion(state, serverVersion);
}

// ── useServerUpdate / ServerUpdateAction ──────────────────────────────────
export interface ServerUpdateTarget {
  /** The saved environment's key (`origin\nid`) T3Fleet runs the job under. */
  readonly environmentKey: string;
  readonly environmentId: string;
  readonly serverLabel: string;
  readonly selfUpdate: string | null;
  readonly installation?: ServerInstallation | undefined;
  readonly desktopAppUpdate?: boolean;
  readonly threadContinuation?: boolean;
  readonly targetVersion: string;
  readonly fromVersion: string;
  readonly continueThreadsAfterServerUpdate?: boolean;
  /** The config's capabilities, for the native job (a connected server's descriptor may omit them). */
  readonly capabilities?: Obj;
}

/** ServerUpdateAction's label: the manual copy labels without self-update, else the caller's. */
export function serverUpdateActionLabel(target: Pick<ServerUpdateTarget, 'selfUpdate' | 'installation'>, label = 'Update'): string {
  return target.selfUpdate !== null ? label : target.installation?.kind === 'npm-global' ? 'Copy update command' : 'Copy relaunch command';
}
/** A desktop-managed server whose app cannot update remotely gets the sentence, not a button. */
export const manualDesktopOnly = (target: Pick<ServerUpdateTarget, 'selfUpdate' | 'desktopAppUpdate'>) =>
  target.selfUpdate === 'desktop-managed' && target.desktopAppUpdate !== true;
export const desktopUpdateConfirmMessage = (serverLabel: string) =>
  `Update the T3 Code desktop app that runs the ${serverLabel}? It will close and relaunch on that machine.`;
/** ConfirmDialogHost resolveConfirmDialogCopy: the question line titles the dialog. */
export function confirmDialogCopy(message: string): { title: string; description: string } {
  const normalized = message.trim(), index = normalized.indexOf('?');
  return index >= 0 ? { title: normalized.slice(0, index + 1).trim(), description: normalized.slice(index + 1).trim() } : { title: 'Confirm action', description: normalized };
}

/** The target a connected server's config describes (ChatView's versionMismatch* values). */
export function updateTargetFromConfig(config: unknown, environmentKey: string, environmentId: string, serverLabel: string): ServerUpdateTarget {
  const environment = obj(obj(config).environment);
  return {
    environmentKey, environmentId, serverLabel, selfUpdate: resolveServerSelfUpdateCapability(config), installation: configInstallation(config),
    desktopAppUpdate: supportsDesktopAppUpdate(config), threadContinuation: supportsServerUpdateThreadContinuation(config),
    targetVersion: CLIENT_VERSION, fromVersion: str(environment.serverVersion),
    continueThreadsAfterServerUpdate: obj(obj(config).settings).continueThreadsAfterServerUpdate === true, capabilities: obj(environment.capabilities),
  };
}

export type UpdateDeps = {
  start: (key: string, request: Obj) => Promise<Obj>;
  copy: (text: string) => Promise<void>;
  toast: (toast: { kind: 'success' | 'error'; title: string; description: string; stacked?: boolean }) => void;
};
/** The environments whose start is in flight (pendingUpdateEnvironmentIds). The job's running status takes over after. */
const pendingUpdateEnvironmentIds = new Set<string>();
/** Each attempt's toast facts: the job table holds no capability. */
const attempts = new Map<string, { serverLabel: string; selfUpdate: string | null; failureTitle: string }>();

/** Whether this environment's update is in flight (a start, or a running job). */
export function isServerUpdatePending(environmentId: string): boolean {
  return pendingUpdateEnvironmentIds.has(environmentId) || connectedJob(environmentId)?.[1].status === 'running';
}

/**
 * The update a click asks for, after any confirmation: the manual path copies its command;
 * a self-update starts one job per environment (a second click while one runs does nothing).
 * Returns what happened, for the caller and the tests.
 */
export async function updateEnvironment(target: ServerUpdateTarget, deps: UpdateDeps, failureTitle = 'Server update failed'): Promise<'copied' | 'started' | 'busy' | 'manual' | 'failed'> {
  if (target.selfUpdate === null) {
    const manual = manualUpdateCopy(target.targetVersion, target.installation, target.serverLabel);
    try { await deps.copy(manual.command); }
    catch { deps.toast({ kind: 'error', title: manual.failureTitle, description: manual.failureMessage }); return 'failed'; }
    deps.toast({ kind: 'success', title: manual.title, description: manual.description });
    return 'copied';
  }
  if (manualDesktopOnly(target)) return 'manual';
  if (isServerUpdatePending(target.environmentId)) return 'busy';
  pendingUpdateEnvironmentIds.add(target.environmentId);
  try {
    const reply = await deps.start(target.environmentKey, {
      mode: 'connected', label: target.serverLabel, fromVersion: target.fromVersion || target.targetVersion, targetVersion: target.targetVersion,
      capabilities: target.capabilities ?? {},
      // continueRunningThreads only when the server supports it and its saved setting asks for it.
      extras: target.threadContinuation && target.continueThreadsAfterServerUpdate ? { continueRunningThreads: true } : {},
    });
    if (reply.started !== true) return 'busy';
    attempts.set(str(reply.attempt), { serverLabel: target.serverLabel, selfUpdate: target.selfUpdate, failureTitle });
    return 'started';
  } catch (error) {
    deps.toast({ kind: 'error', title: failureTitle, description: serverUpdateFailureMessage(error), stacked: true });
    return 'failed';
  } finally { pendingUpdateEnvironmentIds.delete(target.environmentId); }
}

/** The deps over the native module and the client's toasts. */
export function nativeUpdateDeps(native: Native, client: T3Client | null): UpdateDeps {
  return {
    start: (key, request) => startUpdateJob(native, key, request),
    copy: async text => { const reply = await bridgeReply(native, { op: 'copyText', text }); if (!reply.ok) throw new Error(reply.error!.message); },
    toast: toast => { if (client) pushToast(client, toast); },
  };
}

/** The success toast's text (useServerUpdate). */
export function serverUpdateSuccessToast(serverLabel: string, selfUpdate: string | null, targetVersion: string) {
  return { title: `${serverLabel} updated`, description: selfUpdate === 'desktop-managed' ? `Desktop app relaunched on ${targetVersion}.` : `Reconnected on t3@${targetVersion}.` };
}

const announced = new Set<string>();
const nudged = new Set<string>();
/**
 * Each connected-server job's result, once: success after its connection reports the
 * target version (a finished job nudges a backing-off transport to retry now), failure
 * at once. `versionOf` reads the version an environment's connection reports.
 */
export async function announceServerUpdates(native: Native, client: T3Client | null, versionOf: (key: string, environmentId: string) => string | null): Promise<void> {
  // announceJobs read the table in the same fleet pass.
  for (const [key, job] of Object.entries(allJobs())) {
    if (job.mode !== 'connected' || job.status === 'running') continue;
    const signature = `${key}\n${job.attempt}\n${job.status}`;
    if (announced.has(signature)) continue;
    const facts = attempts.get(job.attempt) ?? { serverLabel: job.label || 'server', selfUpdate: null, failureTitle: 'Server update failed' };
    if (job.status === 'failed') {
      announced.add(signature);
      if (client) pushToast(client, { kind: 'error', title: facts.failureTitle, description: job.message || 'Server update failed.', stacked: true });
      continue;
    }
    if (job.status !== 'done') continue;
    const version = versionOf(key, keyEnvironment(key));
    if (version !== job.targetVersion) {
      // The server is back; the transport may still be in its backoff. Retry once now.
      if (!nudged.has(signature) && client && keyEnvironment(key) === client.environmentId && client.connection !== 'connected') {
        nudged.add(signature);
        void native.later({ op: 'retry', origin: '', credential: '' }).catch(() => undefined);
      }
      continue;
    }
    announced.add(signature);
    if (client) pushToast(client, { kind: 'success', ...serverUpdateSuccessToast(facts.serverLabel, facts.selfUpdate, job.resultVersion || job.targetVersion) });
    await native.later({ op: 'fleetOutdatedAck', fleet: key }).catch(() => undefined);
    await readJobs(native);
  }
}
