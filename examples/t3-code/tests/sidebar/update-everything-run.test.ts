import { describe, expect, test } from 'bun:test';
import { runEverything, retryEverythingServers, updateEverythingState, type RunDeps } from '../../src/sidebar/update-everything';
import { success } from '../../src/providers/provider-updates';
import type { ServerUpdateOutcome, ServerUpdateTarget } from '../../src/connections/server-update';
import type { ProviderUpdateMachine } from '../../src/providers/providers-upkeep';

const target = (id: string): ServerUpdateTarget => ({ environmentId: id, environmentKey: `https://${id}\n${id}`, serverLabel: id,
  selfUpdate: 'boot-service', targetVersion: '0.0.50', fromVersion: '0.0.40' });
const provider = (id: string): ProviderUpdateMachine => ({ key: id, environmentId: id, label: id, candidates: [{ instanceId: 'codex', driver: 'codex' }],
  target: { key: id, label: id, providers: [], request: async () => ({}) } });
const deferred = <T>() => { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; };
// Explicit promise barriers model the installer and remote server; no elapsed-time waits.
const flush = async () => { await Promise.resolve(); await Promise.resolve(); };

describe('Update everything run', () => {
  test('updates providers once before concurrent servers, and a slow server leaves the other machine done', async () => {
    const state = updateEverythingState({}), providers = deferred<Awaited<ReturnType<RunDeps['providers']>>>();
    const fast = deferred<ServerUpdateOutcome>(), slow = deferred<ServerUpdateOutcome>();
    const calls: string[] = [], results: Parameters<RunDeps['toast']>[0][] = [];
    const deps: RunDeps = { providers: async () => { calls.push('providers'); return providers.promise; },
      server: async server => { calls.push(server.environmentId); return server.environmentId === 'fast' ? fast.promise : slow.promise; },
      now: () => 1000, changed: () => {}, toast: result => { results.push(result); } };
    const plan = { providers: [provider('fast')], servers: [target('fast'), target('slow')] };
    const run = runEverything(state, plan, deps);
    await runEverything(state, plan, deps);
    expect(calls).toEqual(['providers']);
    expect(state.run?.phase).toBe('providers');
    providers.resolve([{ environmentId: 'fast', machineLabel: 'fast', driver: 'codex', instanceId: 'codex',
      result: success({ providers: [{ instanceId: 'codex', updateState: { status: 'succeeded' } }] }) }]);
    await flush(); await flush();
    expect(calls).toEqual(['providers', 'fast', 'slow']);
    expect(state.run?.phase).toBe('servers');
    fast.resolve({ status: 'updated' }); await flush(); await flush();
    expect(state.run?.machines.map(machine => [machine.environmentId, machine.result])).toEqual([['fast', 'done'], ['slow', null]]);
    expect(results).toEqual([]);
    slow.resolve({ status: 'failed', message: 'Installer failed' }); await run;
    expect(results[0]).toMatchObject({ updatedProviders: 1, updatedServers: 1, failures: ['slow: Installer failed'] });
    expect(state.run?.machines[1]?.result).toBe('failed');
    retryEverythingServers(state, { ...deps, server: async server => { calls.push(`retry:${server.environmentId}`); return { status: 'updated' }; } });
    await flush();
    expect(calls).toEqual(['providers', 'fast', 'slow', 'retry:slow']);
    expect(state.run).toBeNull();
  });

  test('a server retry retains a provider failure and never re-runs the provider', async () => {
    const state = updateEverythingState({});
    const deps: RunDeps = { providers: async () => [{ environmentId: 'a', machineLabel: 'a', driver: 'codex', instanceId: 'codex',
      result: success({ providers: [{ instanceId: 'codex', updateState: { status: 'failed', message: 'No disk space' } }] }) }],
      server: async () => ({ status: 'failed', message: 'Restart failed' }), now: () => 1, changed: () => {}, toast: () => {} };
    await runEverything(state, { providers: [provider('a')], servers: [target('a')] }, deps);
    expect(state.run?.machines[0]).toMatchObject({ failure: 'Restart failed', providerFailure: 'a · Codex: No disk space' });
    retryEverythingServers(state, { ...deps, server: async () => ({ status: 'updated' }) }); await flush();
    expect(state.run?.machines[0]).toMatchObject({ server: null, result: 'failed', failure: 'a · Codex: No disk space' });
  });
});
