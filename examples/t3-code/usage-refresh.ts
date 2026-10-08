// The Usage page's two refreshes and the Cursor Keychain offer, T3 Code 1e2ecbd975 (MIT, see
// LICENSE-T3): packages/client-runtime/src/state/usage.ts (needsCursorKeychainAccess,
// cursorKeychainAccessEnvironments, refreshUsageLimits, refreshUsage). Changes:
// - refreshUsageLimits reads the time from a `now` argument (a clock function, read at the call
//   and when the check settles) instead of `Date.now()`: exact2 data sources take time as an
//   argument (EXACT2-GAPS X19), and the agent's clock is the page's clock;
// - refreshUsage takes the environment operations as a port instead of the atom registry: the
//   rates command, the summary query's invalidation and read, and the connection phase with a
//   change listener. The read takes an abort signal of its own shape (`{ aborted, onabort }`):
//   the native runtime has no AbortController. The order and the rules are the reference's.
import { arr, obj, str, type Obj } from './domain';

/** Offer the Cursor Keychain prompt only where a working Cursor provider could use it. */
export function needsCursorKeychainAccess(summary: Obj | null, providers: readonly Obj[] | null): boolean {
  return arr(summary?.sources).some(source => source.action === 'enableCursorKeychain')
    && (providers ?? []).some(provider => provider.driver === 'cursor' && provider.status === 'ready');
}

/**
 * Environments to offer the Cursor Keychain prompt in the usage summary.
 *
 * Any environment reading a Cursor account already reports that account's history from every
 * machine, so the prompt only adds duplicates there. A different account on another machine stays
 * reachable from provider settings.
 */
export function cursorKeychainAccessEnvironments<E extends { readonly summary: Obj | null; readonly needsCursorKeychainAccess: boolean }>(environments: readonly E[]): readonly E[] {
  const hasCursorAccount = environments.some(environment => arr(environment.summary?.sources).some(source => {
    const fingerprint = obj(source.fingerprint);
    return fingerprint.provider === 'cursor' && fingerprint.hostId === 'cursor.com';
  }));
  return hasCursorAccount ? [] : environments.filter(environment => environment.needsCursorKeychainAccess);
}

const limitsRefreshAfter = new Map<string, number>();
const limitsRefreshes = new Map<string, Promise<unknown>>();

/**
 * One `server.refreshProviders` per environment at a time: an automatic refresh inside five
 * minutes of the last check does nothing; a manual one joins the check in flight; `afterPending`
 * runs a fresh check once the current one ends (after Cursor access is enabled).
 */
export async function refreshUsageLimits<A>(environmentId: string, refresh: () => Promise<A>, now: () => number, automatic = false, afterPending = false): Promise<A | undefined> {
  const pending = limitsRefreshes.get(environmentId);
  if (pending !== undefined) {
    if (afterPending) {
      try { await pending; } catch { /* The new check still needs to run if the earlier one failed. */ }
      return refreshUsageLimits(environmentId, refresh, now, false, true);
    }
    // Manual refresh waits for the current check; automatic refresh does not repeat it.
    return automatic ? undefined : ((await pending) as A);
  }
  const refreshAfter = limitsRefreshAfter.get(environmentId) ?? 0;
  if (automatic && now() < refreshAfter) return undefined;
  const current = Promise.resolve().then(refresh).finally(() => {
    limitsRefreshes.delete(environmentId);
    limitsRefreshAfter.set(environmentId, now() + 5 * 60_000);
  });
  limitsRefreshes.set(environmentId, current);
  return await current;
}
/** Tests start each case without the module's checks and windows (the reference test uses a fresh environment id each). */
export function forgetLimitsRefreshes(): void { limitsRefreshAfter.clear(); limitsRefreshes.clear(); }
/** Whether a limits check is in flight for the environment (the Usage page's busy refresh button). */
export const limitsRefreshPending = (environmentId: string) => limitsRefreshes.has(environmentId);

/** An abort signal the port's read honours (the reference passes an AbortSignal to executeAtomQuery). */
export type UsageAbort = { aborted: boolean; onabort: (() => void) | null };
/** What refreshUsage needs from each environment (the reference's atom registry, server atoms and presentations). */
export type UsageRefreshPort = {
  /** `server.refreshUsageRates`; a failure that means there is no usable RPC session says so. */
  refreshRates(environmentId: string): Promise<{ ok: true } | { ok: false; sessionUnavailable: boolean }>;
  /** registry.refresh(query): a read that started before this must not stand for the new one. */
  invalidate(environmentId: string): void;
  /** executeAtomQuery: settles when the summary arrives, fails, or the signal aborts. */
  read(environmentId: string, signal: UsageAbort): Promise<unknown>;
  connected(environmentId: string): boolean;
  subscribe(environmentId: string, listener: () => void): () => void;
};

/** Refresh pricing, then await each selected environment's rescan while it remains connected. */
export async function refreshUsage(port: UsageRefreshPort, environmentIds: readonly string[]): Promise<void> {
  await Promise.all(environmentIds.map(async environmentId => {
    const signal: UsageAbort = { aborted: false, onabort: null };
    const abortWhenDisconnected = () => {
      if (port.connected(environmentId) || signal.aborted) return;
      signal.aborted = true;
      signal.onabort?.();
    };
    const unsubscribe = port.subscribe(environmentId, abortWhenDisconnected);
    abortWhenDisconnected();
    try {
      const rates = await port.refreshRates(environmentId).catch(() => ({ ok: false as const, sessionUnavailable: false }));
      const sessionUnavailable = !rates.ok && rates.sessionUnavailable;
      // Invalidate even on failure so reconnects cannot reuse the old summary.
      port.invalidate(environmentId);
      if (sessionUnavailable || signal.aborted) return;
      await port.read(environmentId, signal).catch(() => undefined);
    } finally {
      unsubscribe();
    }
  }));
}

/** EnvironmentRpcUnavailableError's equivalent in the clone: the transport had no session to send on. */
export const sessionUnavailable = (kind: string) => ['Disconnected', 'Closed', 'stale', 'superseded'].includes(str(kind));
