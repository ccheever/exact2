// Provider update logic, T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3):
// apps/web/src/components/ProviderUpdateLaunchNotification.logic.ts — isProviderUpdateCandidate,
// isProviderUpdateActive, collectProviderUpdateCandidates, isProviderSettingsUpdateCandidate,
// hasOneClickUpdateProviderCandidate, canOneClickUpdateProviderCandidate,
// getProviderUpdateRunOutcome and getProviderUpdateRunToastView.
// Changes: a provider is the server's JSON (`Obj`), read with the domain helpers; an atom
// command's result is `CommandResult` (`interrupted` marks what the reference's
// isAtomCommandInterrupted reports: a request the client let go). The sidebar pill is
// sidebar-provider-pill.ts.
import { arr, obj, str, type Obj } from '../client/domain';

/** useAtomCommand's result: a value, or a failure (`interrupted` when the request was let go). */
export type CommandResult<T> = { readonly _tag: 'Success'; readonly value: T } | { readonly _tag: 'Failure'; readonly error: unknown; readonly interrupted?: boolean };
export const success = <T>(value: T): CommandResult<T> => ({ _tag: 'Success', value });
export const failure = <T = never>(error: unknown, interrupted = false): CommandResult<T> => ({ _tag: 'Failure', error, ...(interrupted ? { interrupted: true } : {}) });
const isInterrupted = (result: CommandResult<unknown>) => result._tag === 'Failure' && result.interrupted === true;

/** PROVIDER_DISPLAY_NAMES (packages/contracts/src/model.ts). */
export const PROVIDER_DISPLAY_NAMES: Record<string, string> = { antigravity: 'Antigravity', codex: 'Codex', claudeAgent: 'Claude', cursor: 'Cursor',
  grok: 'Grok', fx: 'fx', muse: 'Muse Code', acpRegistry: 'ACP Registry', pi: 'Pi', opencode: 'OpenCode' };
export const providerDisplayName = (driver: string) => PROVIDER_DISPLAY_NAMES[driver] ?? driver;

const advisoryOf = (provider: Obj) => obj(provider.versionAdvisory);
const compatibilityOf = (provider: Obj) => obj(provider.compatibilityAdvisory);
const nonNull = (value: unknown) => value !== null && value !== undefined;

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
const getProviderUpdatedDescription = (count: number) => count === 1 ? 'New sessions will use the updated provider.' : 'New sessions will use the updated providers.';

export function isProviderUpdateCandidate(provider: Obj): boolean {
  const compatibility = compatibilityOf(provider), advisory = advisoryOf(provider);
  return provider.enabled === true && compatibility.latestVersionStatus !== 'broken' && compatibility.latestVersionStatus !== 'unsupported'
    && advisory.status === 'behind_latest' && nonNull(advisory.latestVersion);
}
export function isProviderUpdateActive(provider: Obj | undefined): boolean {
  const status = provider ? str(obj(provider.updateState).status) : '';
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

/** One provider update sent by the cross-machine "Update all" (and Update everything), with its result. */
export interface ProviderUpdateRun { readonly environmentId: string; readonly machineLabel: string; readonly driver: string; readonly instanceId: string; readonly result: CommandResult<{ providers: readonly Obj[] }> }
const errorMessage = (error: unknown, fallback: string) => error instanceof Error ? error.message : fallback;

/** One update's outcome: interrupted (let go), updated, or a failure line naming its machine and provider. */
export function getProviderUpdateRunOutcome(run: ProviderUpdateRun): { readonly status: 'interrupted' | 'updated' } | { readonly status: 'failed'; readonly line: string } {
  if (isInterrupted(run.result)) return { status: 'interrupted' };
  const label = `${run.machineLabel} · ${providerDisplayName(run.driver)}`;
  if (run.result._tag === 'Failure') return { status: 'failed', line: `${label}: ${errorMessage(run.result.error, 'Provider update failed.')}` };
  const updateState = run.result.value.providers.find(provider => provider.instanceId === run.instanceId)?.updateState;
  return str(obj(updateState).status) === 'succeeded' ? { status: 'updated' }
    : { status: 'failed', line: `${label}: ${str(obj(updateState).message) || 'Provider update did not finish.'}` };
}

/**
 * Summarize a cross-machine "Update all" as one toast, or null when every request was
 * interrupted. Each update that did not succeed gets its own line.
 */
export function getProviderUpdateRunToastView(runs: readonly ProviderUpdateRun[]): { readonly type: 'success' | 'error'; readonly title: string; readonly description: string } | null {
  const settled = runs.map(getProviderUpdateRunOutcome).filter(outcome => outcome.status !== 'interrupted');
  if (settled.length === 0) return null;
  const failureLines = settled.flatMap(outcome => outcome.status === 'failed' ? [outcome.line] : []);
  if (failureLines.length === 0) return { type: 'success', title: settled.length === 1 ? 'Provider updated' : `${settled.length} providers updated`, description: getProviderUpdatedDescription(settled.length) };
  return { type: 'error', title: failureLines.length < settled.length ? `${failureLines.length} of ${settled.length} provider updates failed`
    : settled.length === 1 ? 'Provider update failed' : 'Provider updates failed', description: failureLines.join('\n') };
}

/** The providers of an update RPC's answer ({ providers }). */
export const providersOf = (value: unknown): Obj[] => arr(obj(value).providers);
