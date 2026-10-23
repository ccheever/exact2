// Where each environment's usage answer stands, and which figures on screen are still coming in,
// T3 Code 9c161462c9 (MIT, see LICENSE-T3): packages/client-runtime/src/state/usageProgress.ts
// (usageEnvironmentProgress, updatingProvidersLabel, usageLoadingState). Changes: a summary is the
// server's JSON as the clone keeps it (`Obj`); `isConnected` is the clone's `connected`;
// `usageProgress`, which only the mobile app reads, is left out.
import { arr, obj, str, type Obj } from '../client/domain';

export type UsageProgressEnvironment = {
  readonly label: string;
  readonly connected: boolean;
  readonly isPending: boolean;
  readonly error: string | null;
  readonly summary: Obj | null;
};
export type UsageEnvironmentProgress = { phase: 'inactive' | 'loading' | 'stale' | 'ready' } | { phase: 'partway'; providers: string[] };

/**
 * Where one environment's answer stands for the usage on screen.
 *
 * - `inactive`: disconnected or failed, so waiting on it changes nothing.
 * - `loading`: no summary yet.
 * - `stale`: its summary on screen predates the current request.
 * - `partway`: answered this round while slow sources finish refreshing.
 * - `ready`: answered.
 *
 * While a manual `refreshing` runs, every summary counts as stale.
 */
export function usageEnvironmentProgress(environment: UsageProgressEnvironment, refreshing = false): UsageEnvironmentProgress {
  const { summary } = environment;
  if (!environment.connected || environment.error !== null) return { phase: 'inactive' };
  if (summary === null) return { phase: 'loading' };
  if (refreshing) return { phase: 'stale' };
  // A refreshing source without a pending follow-up has nothing left to wait on.
  if (!environment.isPending) return { phase: 'ready' };
  const providers = [...new Set(arr(summary.sources).flatMap(source => (source.refreshing === true ? [str(obj(source.fingerprint).provider)] : [])))];
  return providers.length > 0 ? { phase: 'partway', providers } : { phase: 'stale' };
}

export function updatingProvidersLabel(providers: readonly string[], providerLabel: (provider: string) => string): string {
  return providers.length === 1 ? `Updating ${providerLabel(providers[0]!)}…` : `Updating ${providers.length} providers…`;
}

/**
 * Which figures in the merged usage on screen are still coming in, so clients can mute just those.
 *
 * - `partial`: totals will change because some environment is still answering.
 * - `providers`: providers whose own figures will change. An environment that has not answered
 *   yet, or whose whole summary is stale, could change any provider, so it sets `everyProvider`.
 */
export function usageLoadingState(environments: readonly UsageProgressEnvironment[], refreshing = false) {
  const providers = new Set<string>();
  let everyProvider = false;
  for (const environment of environments) {
    const progress = usageEnvironmentProgress(environment, refreshing);
    if (progress.phase === 'loading' || progress.phase === 'stale') everyProvider = true;
    if (progress.phase === 'partway') for (const provider of progress.providers) providers.add(provider);
  }
  return { partial: everyProvider || providers.size > 0, everyProvider, providers };
}
