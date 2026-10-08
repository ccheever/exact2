// Provider update logic, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/ProviderUpdateLaunchNotification.logic.ts — isProviderUpdateCandidate,
// isProviderUpdateActive, collectProviderUpdateCandidates, isProviderSettingsUpdateCandidate,
// hasOneClickUpdateProviderCandidate, canOneClickUpdateProviderCandidate,
// providerUpdateNotificationKey, getProviderUpdateInitialToastView,
// shouldShowPrimaryProviderUpdateToast, getProviderUpdateRejectedToastView,
// getProviderUpdateProgressToastView, getProviderUpdateRunToastView,
// collectUpdatedProviderSnapshots, firstFailedProviderUpdateMessage, isTerminalProviderUpdatePhase,
// collectProviderUpdateOutcomeSnapshots and firstRejectedProviderUpdateMessage.
// Changes: a provider is the server's JSON (`Obj`), read with the domain helpers; an atom
// command's result is `CommandResult` (`interrupted` marks what the reference's
// isAtomCommandInterrupted reports: a request the client let go). The sidebar pill is
// sidebar-provider-pill.ts; the WSL grouping (buildLocalEnvironmentUpdateGroups and the row
// status) is out of scope (20261005-wsl-environments).
import { arr, obj, str, type Obj } from './domain';

export type ProviderUpdateToastType = 'warning' | 'loading' | 'error' | 'success';
export type ProviderUpdateToastPhase = 'initial' | 'running' | 'failed' | 'unchanged' | 'succeeded';
export interface ProviderUpdateToastView {
  readonly phase: ProviderUpdateToastPhase; readonly type: ProviderUpdateToastType;
  readonly title: string; readonly description: string; readonly dismissAfterVisibleMs?: number;
}
/** useAtomCommand's result: a value, or a failure (`interrupted` when the request was let go). */
export type CommandResult<T> = { readonly _tag: 'Success'; readonly value: T } | { readonly _tag: 'Failure'; readonly error: unknown; readonly interrupted?: boolean };
export const success = <T>(value: T): CommandResult<T> => ({ _tag: 'Success', value });
export const failure = <T = never>(error: unknown, interrupted = false): CommandResult<T> => ({ _tag: 'Failure', error, ...(interrupted ? { interrupted: true } : {}) });
const isInterrupted = (result: CommandResult<unknown>) => result._tag === 'Failure' && result.interrupted === true;

const PROVIDER_UPDATE_SUCCESS_VISIBLE_MS = 3_000;
/** PROVIDER_DISPLAY_NAMES (packages/contracts/src/model.ts). */
export const PROVIDER_DISPLAY_NAMES: Record<string, string> = { antigravity: 'Antigravity', codex: 'Codex', claudeAgent: 'Claude', cursor: 'Cursor',
  grok: 'Grok', acpRegistry: 'ACP Registry', pi: 'Pi', opencode: 'OpenCode' };
export const providerDisplayName = (driver: string) => PROVIDER_DISPLAY_NAMES[driver] ?? driver;

const formatVersion = (value: string) => value.startsWith('v') ? value : `v${value}`;
const advisoryOf = (provider: Obj) => obj(provider.versionAdvisory);
const compatibilityOf = (provider: Obj) => obj(provider.compatibilityAdvisory);
const updateStatus = (provider: Obj) => str(obj(provider.updateState).status);
const nonNull = (value: unknown) => value !== null && value !== undefined;

/** Terminal update phases: outcomes that are safe to persist as a one-shot row result. */
export function isTerminalProviderUpdatePhase(phase: ProviderUpdateToastPhase): boolean {
  return phase === 'succeeded' || phase === 'failed' || phase === 'unchanged';
}

/** defaultInstanceIdForDriver: a driver's default slot is named after it. */
function chooseRepresentativeProvider(current: Obj | undefined, candidate: Obj): Obj {
  if (!current) return candidate;
  const defaultInstanceId = str(candidate.driver);
  if (str(candidate.instanceId) === defaultInstanceId) return candidate;
  if (str(current.instanceId) === defaultInstanceId) return current;
  return str(candidate.checkedAt).localeCompare(str(current.checkedAt)) >= 0 ? candidate : current;
}
function dedupeProvidersByDriver(providers: readonly Obj[]): Obj[] {
  const latest = new Map<string, Obj>();
  for (const provider of providers) latest.set(str(provider.driver), chooseRepresentativeProvider(latest.get(str(provider.driver)), provider));
  return [...latest.values()];
}
function dedupeProvidersByInstanceId(providers: readonly Obj[]): Obj[] {
  const latest = new Map<string, Obj>();
  for (const provider of providers) {
    const current = latest.get(str(provider.instanceId));
    if (!current || str(provider.checkedAt).localeCompare(str(current.checkedAt)) >= 0) latest.set(str(provider.instanceId), provider);
  }
  return [...latest.values()];
}
const getProviderUpdatedDescription = (count: number) => count === 1 ? 'New sessions will use the updated provider.' : 'New sessions will use the updated providers.';

export function isProviderUpdateCandidate(provider: Obj): boolean {
  const compatibility = compatibilityOf(provider), advisory = advisoryOf(provider);
  return provider.enabled === true && compatibility.latestVersionStatus !== 'broken' && compatibility.latestVersionStatus !== 'unsupported'
    && advisory.status === 'behind_latest' && nonNull(advisory.latestVersion);
}
export function isProviderUpdateActive(provider: Obj | undefined): boolean {
  const status = provider ? updateStatus(provider) : '';
  return status === 'queued' || status === 'running';
}
export function collectProviderUpdateCandidates(providers: readonly Obj[]): Obj[] {
  return dedupeProvidersByDriver(providers.filter(isProviderUpdateCandidate));
}
export function isProviderSettingsUpdateCandidate(provider: Obj): boolean {
  const compatibility = compatibilityOf(provider), advisory = advisoryOf(provider);
  return provider.enabled === true && compatibility.latestVersionStatus !== 'broken' && compatibility.latestVersionStatus !== 'unsupported'
    && advisory.status === 'behind_latest' && advisory.canUpdate === true && nonNull(advisory.updateCommand);
}
export function hasOneClickUpdateProviderCandidate(candidate: Obj, providers: readonly Obj[]): boolean {
  const advisory = advisoryOf(candidate);
  if (advisory.canUpdate !== true || !nonNull(advisory.updateCommand)) return false;
  const driverProviders = providers.filter(provider => provider.driver === candidate.driver);
  if (driverProviders.length === 0) return false;
  const updateCommands = new Set<string>();
  for (const provider of driverProviders) {
    if (!isProviderUpdateCandidate(provider)) continue;
    const sibling = advisoryOf(provider);
    if (sibling.canUpdate !== true || !nonNull(sibling.updateCommand)) return false;
    updateCommands.add(str(sibling.updateCommand));
  }
  return updateCommands.size === 1;
}
export function canOneClickUpdateProviderCandidate(candidate: Obj, providers: readonly Obj[]): boolean {
  return !isProviderUpdateActive(candidate) && hasOneClickUpdateProviderCandidate(candidate, providers);
}
export function providerUpdateNotificationKey(providers: readonly Obj[]): string | null {
  const parts = dedupeProvidersByDriver(providers).map(provider => [str(provider.driver), str(advisoryOf(provider).latestVersion)].join(':')).sort();
  return parts.length > 0 ? parts.join('|') : null;
}
function formatProviderList(providers: readonly Obj[]): string {
  const names = providers.map(provider => providerDisplayName(str(provider.driver)));
  return names.length <= 2 ? names.join(' and ') : `${names.slice(0, -1).join(', ')}, and ${names[names.length - 1]}`;
}
function getProviderUpdateInitialToastTitle(providers: readonly Obj[]): string {
  if (providers.length === 1) {
    const provider = providers[0]!;
    return `Update Available: ${providerDisplayName(str(provider.driver))} ${formatVersion(str(advisoryOf(provider).latestVersion))}`;
  }
  return `Updates Available: ${providers.length} providers`;
}
function getFailedProviderUpdateDescription(providers: readonly Obj[]): string {
  if (providers.length === 1) {
    const message = str(obj(providers[0]!.updateState).message);
    if (message) return message;
  }
  return `${formatProviderList(providers)} failed to update. Check provider settings for details.`;
}
export function getProviderUpdateInitialToastView(input: { updateProviders: readonly Obj[]; oneClickProviders: readonly Obj[] }): ProviderUpdateToastView {
  return { phase: 'initial', type: 'warning', title: getProviderUpdateInitialToastTitle(input.updateProviders),
    description: input.oneClickProviders.length > 0 ? 'Install the update now or review provider settings.' : `${formatProviderList(input.updateProviders)} can be updated from provider settings.` };
}
export function shouldShowPrimaryProviderUpdateToast(view: ProviderUpdateToastView): boolean { return view.phase !== 'running'; }
function getProviderUpdateRunningToastView(providerCount: number): ProviderUpdateToastView {
  return { phase: 'running', type: 'loading', title: providerCount === 1 ? 'Updating provider' : 'Updating providers', description: 'Running provider update command.' };
}
export function getProviderUpdateRejectedToastView(providerCount: number, message: string): ProviderUpdateToastView {
  return { phase: 'failed', type: 'error', title: providerCount === 1 ? 'Provider update failed' : 'Provider updates failed', description: message };
}
export function getProviderUpdateProgressToastView(input: { providers: readonly Obj[]; providerCount: number }): ProviderUpdateToastView {
  const providers = dedupeProvidersByDriver(input.providers);
  const failedProviders = providers.filter(provider => updateStatus(provider) === 'failed');
  if (failedProviders.length > 0) {
    return { phase: 'failed', type: 'error', title: failedProviders.length === 1 ? 'Provider update failed' : 'Provider updates failed', description: getFailedProviderUpdateDescription(failedProviders) };
  }
  const unchangedProviders = providers.filter(provider => updateStatus(provider) === 'unchanged');
  if (unchangedProviders.length > 0) {
    return { phase: 'unchanged', type: 'warning', title: unchangedProviders.length === 1 ? 'Provider still needs an update' : 'Providers still need updates',
      description: `${formatProviderList(unchangedProviders)} ${unchangedProviders.length === 1 ? 'still appears' : 'still appear'} outdated. Check provider settings for details.` };
  }
  if (providers.some(provider => isProviderUpdateActive(provider))) return getProviderUpdateRunningToastView(input.providerCount);
  const hasCompleteProviderSnapshots = providers.length >= input.providerCount;
  const allProvidersUpdated = hasCompleteProviderSnapshots && providers.every(provider => updateStatus(provider) === 'succeeded' || !isProviderUpdateCandidate(provider));
  if (allProvidersUpdated) {
    return { phase: 'succeeded', type: 'success', title: input.providerCount === 1 ? 'Provider updated' : 'Provider updates finished',
      description: getProviderUpdatedDescription(input.providerCount), dismissAfterVisibleMs: PROVIDER_UPDATE_SUCCESS_VISIBLE_MS };
  }
  return getProviderUpdateRunningToastView(input.providerCount);
}

/** One provider update sent by the cross-machine "Update all", with its result. */
export interface ProviderUpdateRun { readonly machineLabel: string; readonly driver: string; readonly instanceId: string; readonly result: CommandResult<{ providers: readonly Obj[] }> }
const errorMessage = (error: unknown, fallback: string) => error instanceof Error ? error.message : fallback;

/**
 * Summarize a cross-machine "Update all" as one toast, or null when every request was
 * interrupted. Each update that did not succeed gets its own line.
 */
export function getProviderUpdateRunToastView(runs: readonly ProviderUpdateRun[]): Pick<ProviderUpdateToastView, 'type' | 'title' | 'description'> | null {
  const settled = runs.filter(run => !isInterrupted(run.result));
  if (settled.length === 0) return null;
  const failureLines = settled.flatMap(run => {
    const label = `${run.machineLabel} · ${providerDisplayName(run.driver)}`;
    if (run.result._tag === 'Failure') return [`${label}: ${errorMessage(run.result.error, 'Provider update failed.')}`];
    const updateState = run.result.value.providers.find(provider => provider.instanceId === run.instanceId)?.updateState;
    return str(obj(updateState).status) === 'succeeded' ? [] : [`${label}: ${str(obj(updateState).message) || 'Provider update did not finish.'}`];
  });
  if (failureLines.length === 0) return { type: 'success', title: settled.length === 1 ? 'Provider updated' : `${settled.length} providers updated`, description: getProviderUpdatedDescription(settled.length) };
  return { type: 'error', title: failureLines.length < settled.length ? `${failureLines.length} of ${settled.length} provider updates failed`
    : settled.length === 1 ? 'Provider update failed' : 'Provider updates failed', description: failureLines.join('\n') };
}

export function collectUpdatedProviderSnapshots(input: { results: readonly CommandResult<{ providers: readonly Obj[] }>[]; providerInstanceIds: ReadonlySet<string> }): Obj[] {
  const matched: Obj[] = [];
  for (const result of input.results) {
    if (result._tag === 'Failure') continue;
    for (const provider of result.value.providers) if (input.providerInstanceIds.has(str(provider.instanceId))) matched.push(provider);
  }
  return dedupeProvidersByInstanceId(matched);
}
export function firstFailedProviderUpdateMessage(results: readonly CommandResult<unknown>[]): string | null {
  const failed = results.find(result => result._tag === 'Failure');
  if (!failed || failed._tag !== 'Failure') return null;
  return errorMessage(failed.error, 'Provider update failed.');
}

// --- Multi-environment outcomes (the parts of the WSL helpers the logic tests cover) ---------

/** The settled result of dispatching a provider update to one backend; `provider` is its snapshot of the targeted instance. */
export interface LocalProviderUpdateOutcome { readonly environmentId: string; readonly isPrimary: boolean; readonly driver: string; readonly instanceId: string; readonly provider: Obj | null }
export type SettledResult<T> = { status: 'fulfilled'; value: T } | { status: 'rejected'; reason: unknown };
const PROVIDER_UPDATE_STATUS_SEVERITY: Record<string, number> = { succeeded: 1, queued: 2, running: 2, unchanged: 3, failed: 4 };
const severity = (provider: Obj) => PROVIDER_UPDATE_STATUS_SEVERITY[updateStatus(provider)] ?? 0;
export function firstRejectedProviderUpdateMessage(results: readonly SettledResult<unknown>[]): string | null {
  const rejected = results.find(result => result.status === 'rejected');
  if (!rejected || rejected.status !== 'rejected') return null;
  return errorMessage(rejected.reason, 'Provider update failed.');
}
/** One representative snapshot per driver, keeping the worst status across every backend. */
export function collectProviderUpdateOutcomeSnapshots(results: readonly SettledResult<LocalProviderUpdateOutcome>[]): Obj[] {
  const worstByDriver = new Map<string, Obj>();
  for (const result of results) {
    if (result.status !== 'fulfilled' || result.value.provider === null) continue;
    const provider = result.value.provider, current = worstByDriver.get(str(provider.driver));
    if (!current || severity(provider) > severity(current)) worstByDriver.set(str(provider.driver), provider);
  }
  return [...worstByDriver.values()];
}

/** The providers of an update RPC's answer ({ providers }). */
export const providersOf = (value: unknown): Obj[] => arr(obj(value).providers);
