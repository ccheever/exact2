// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) packages/client-runtime/src/state/usage.test.ts:
// "manual usage refresh" (5), "limits refresh cooldown" (2) and "needsCursorKeychainAccess" (2),
// with their original names. Changes: the atom registry harness is a UsageRefreshPort over the same
// resolvers (a query that joins its in-flight read until invalidated, as an atom does); the clock
// is the `now` argument instead of a Date.now spy; vi.fn is a counting stub.
import { describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import { cursorKeychainAccessEnvironments, needsCursorKeychainAccess, refreshUsage, refreshUsageLimits, type UsageAbort, type UsageRefreshPort } from './usage-refresh';

const it = test;
function withResolvers<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
type Rates = { ok: true } | { ok: false; sessionUnavailable: boolean };
const summary: Obj = { contractVersion: 6, readAt: '2026-09-05T12:00:00Z', buckets: [], sources: [] };

function harness(ids = ['a']) {
  const environments = ids.map(environmentId => {
    const rates = withResolvers<Rates>(), scan = withResolvers<Obj>(), scanStarted = withResolvers<void>();
    const entry = { environmentId, rates, scan, scanStarted, connected: true, listeners: new Set<() => void>(),
      // An atom query: reads join the one in flight until it is refreshed.
      inFlight: null as Promise<Obj> | null, stale: true,
      query: (): Promise<Obj> => { scanStarted.resolve(); return scan.promise; } };
    return entry;
  });
  const get = (environmentId: string) => {
    const environment = environments.find(entry => entry.environmentId === environmentId);
    if (!environment) throw new Error(`Unknown environment: ${environmentId}`);
    return environment;
  };
  const setConnected = (environmentId: string, connected: boolean) => { const entry = get(environmentId); entry.connected = connected; for (const listener of entry.listeners) listener(); };
  const port: UsageRefreshPort = {
    refreshRates: environmentId => get(environmentId).rates.promise,
    invalidate: environmentId => { get(environmentId).stale = true; },
    read: (environmentId, signal: UsageAbort) => {
      const entry = get(environmentId);
      if (entry.stale || !entry.inFlight) { entry.stale = false; entry.inFlight = entry.query(); }
      const reading = entry.inFlight;
      return new Promise((resolve, reject) => { signal.onabort = () => resolve(undefined); reading.then(resolve, reject); });
    },
    connected: environmentId => get(environmentId).connected,
    subscribe: (environmentId, listener) => { const entry = get(environmentId); entry.listeners.add(listener); return () => entry.listeners.delete(listener); },
  };
  return { environments, setConnected, port, refresh: () => refreshUsage(port, environments.map(entry => entry.environmentId)) };
}

describe('manual usage refresh', () => {
  it.each(['success', 'failure'])('waits for the rescan after a pricing %s', async result => {
    const { environments: [environment], refresh } = harness();
    const entry = environment!;
    let finished = false;
    const refreshing = refresh().then(() => { finished = true; });
    expect(finished).toBe(false);
    entry.rates.resolve(result === 'success' ? { ok: true } : { ok: false, sessionUnavailable: false });
    await entry.scanStarted.promise;
    expect(finished).toBe(false);
    entry.scan.resolve(summary);
    await refreshing;
    expect(finished).toBe(true);
  });

  it('settles when an environment disconnects during the rescan', async () => {
    const { environments: [environment], refresh, setConnected } = harness();
    const entry = environment!;
    const refreshing = refresh();
    entry.rates.resolve({ ok: true });
    await entry.scanStarted.promise;
    setConnected(entry.environmentId, false);
    await refreshing;
  });

  it('waits for healthy environments without waiting for a recovering environment', async () => {
    const { environments, refresh, setConnected } = harness(['healthy', 'recovering']);
    const [healthy, recovering] = environments;
    setConnected(recovering!.environmentId, false);
    let finished = false;
    const refreshing = refresh().then(() => { finished = true; });
    for (const entry of environments) entry.rates.resolve({ ok: true });
    await healthy!.scanStarted.promise;
    expect(finished).toBe(false);
    healthy!.scan.resolve(summary);
    await refreshing;
    expect(finished).toBe(true);
  });

  it('settles when connected state has no usable RPC session', async () => {
    const { environments: [environment], refresh } = harness();
    const entry = environment!;
    const refreshing = refresh();
    entry.rates.resolve({ ok: false, sessionUnavailable: true });
    await refreshing;
  });

  it('replaces a scan that started before pricing was refreshed', async () => {
    const { environments: [environment], refresh, port } = harness();
    const entry = environment!;
    let reads = 0;
    const rescanned = withResolvers<void>();
    entry.query = () => {
      reads += 1;
      if (reads > 1) { rescanned.resolve(); return Promise.resolve(summary); }
      return new Promise<Obj>(() => {});
    };
    void port.read(entry.environmentId, { aborted: false, onabort: null }); // mounted before the refresh
    expect(reads).toBe(1);
    const refreshing = refresh();
    entry.rates.resolve({ ok: true });
    await rescanned.promise;
    await refreshing;
    expect(reads).toBe(2);
  });
});

describe('limits refresh cooldown', () => {
  it('runs a fresh check after an in-flight check when settings change', async () => {
    const id = 'limits-after-enable', clock = () => 1_000;
    const oldCheck = withResolvers<string>();
    const first = refreshUsageLimits(id, () => oldCheck.promise, clock, true);
    let calls = 0;
    const newCheck = async () => { calls++; return 'new limits'; };
    const afterEnable = refreshUsageLimits(id, newCheck, clock, false, true);
    expect(calls).toBe(0);
    oldCheck.resolve('old limits');
    expect(await first).toBe('old limits');
    expect(await afterEnable).toBe('new limits');
    expect(calls).toBe(1);
  });

  it('joins manual calls and gates automatic refreshes after success or failure', async () => {
    let time = 1_000;
    const clock = () => time;
    for (const fails of [false, true]) {
      const id = `limits-${fails}`;
      const pending = withResolvers<string>();
      let refreshes = 0;
      const refresh = () => { refreshes++; return pending.promise; };
      const first = refreshUsageLimits(id, refresh, clock, true);
      await refreshUsageLimits(id, refresh, clock, true);
      const manual = refreshUsageLimits(id, refresh, clock);
      let settled = 0;
      void manual.then(() => { settled++; }, () => { settled++; });
      expect(settled).toBe(0);
      expect(refreshes).toBe(1);
      if (fails) {
        // bun's `rejects` waits for the promise at once, so the failures are caught first and read after.
        const firstFailure = first.then(() => '', (error: Error) => error.message);
        const manualFailure = manual.then(() => '', (error: Error) => error.message);
        pending.reject(new Error('unavailable'));
        expect(await Promise.all([firstFailure, manualFailure])).toEqual(['unavailable', 'unavailable']);
      } else {
        pending.resolve('quota');
        expect(await first).toBe('quota');
        expect(await manual).toBe('quota');
      }
      await Promise.resolve();
      expect(settled).toBe(1);
      let next = 0;
      const nextCheck = async () => { next++; return undefined; };
      time = 300_999;
      await refreshUsageLimits(id, nextCheck, clock, true);
      expect(next).toBe(0);
      time = 301_000;
      await refreshUsageLimits(id, nextCheck, clock, true);
      expect(next).toBe(1);
      await refreshUsageLimits(id, nextCheck, clock);
      expect(next).toBe(2);
      time = 1_000;
    }
  });
});

describe('needsCursorKeychainAccess', () => {
  const cursorPrompt: Obj = { ...summary, sources: [{ fingerprint: { hostId: 'host', provider: 'cursor', resolvedHomePath: '/Users/me/.cursor/auth.json', volumeId: 'volume' },
    status: 'ok', scannedFiles: 0, skippedFiles: 0, malformedRecords: 0, distinctSessions: 0, message: 'Cursor account usage is off on this environment.', action: 'enableCursorKeychain' }] };
  const cursor = (status: string): Obj => ({ instanceId: 'cursor', driver: 'cursor', enabled: status !== 'disabled', installed: status === 'ready', version: null, status,
    auth: { status: 'unknown' }, checkedAt: '2026-09-05T12:00:00.000Z', models: [], slashCommands: [], skills: [] });

  it('offers access only when Cursor is ready on that environment', () => {
    expect(needsCursorKeychainAccess(cursorPrompt, [cursor('ready')])).toBe(true);
    expect(needsCursorKeychainAccess(cursorPrompt, [cursor('error')])).toBe(false);
    expect(needsCursorKeychainAccess(cursorPrompt, [cursor('disabled')])).toBe(false);
    expect(needsCursorKeychainAccess(cursorPrompt, [])).toBe(false);
    expect(needsCursorKeychainAccess(cursorPrompt, null)).toBe(false);
    expect(needsCursorKeychainAccess(summary, [cursor('ready')])).toBe(false);
  });

  it('stops offering access once any environment reads the Cursor account', () => {
    const off = { summary: cursorPrompt, needsCursorKeychainAccess: true };
    const account: Obj = { ...summary, sources: [{ fingerprint: { hostId: 'cursor.com', provider: 'cursor', resolvedHomePath: 'cursor-account:abc', volumeId: 'abc' },
      status: 'ok', scannedFiles: 1, skippedFiles: 0, malformedRecords: 0, distinctSessions: 1, message: null }] };
    expect(cursorKeychainAccessEnvironments([off, off])).toEqual([off, off]);
    expect(cursorKeychainAccessEnvironments([off, { summary: account, needsCursorKeychainAccess: false }])).toEqual([]);
  });
});
