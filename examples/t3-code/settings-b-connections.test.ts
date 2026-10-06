import { test, expect } from 'bun:test';
import { updateTargets, runConnectionOp, CONNECTION_OPS, CLIENT_VERSION } from './connections';
import { EnvironmentFleet, environmentKey } from './settings-b-fleet';
import { initialShell, obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';

const config = (label: string, serverVersion: string, serverSelfUpdate = '', desktopAppUpdate = false) =>
  ({ environment: { label, serverVersion, capabilities: { ...(serverSelfUpdate ? { serverSelfUpdate } : {}), ...(desktopAppUpdate ? { desktopAppUpdate } : {}) } } });

test('Update all targets connected, older, self-updatable environments only (ServerUpdatesAction eligibility)', async () => {
  const source = new EnvironmentFleet();
  const keyB = environmentKey('http://10.0.0.2:3773', 'env-b'), keyC = environmentKey('http://10.0.0.3:3773', 'env-c'), keyD = environmentKey('http://10.0.0.4:3773', 'env-d');
  const live = (key: string, cfg: Obj, phase = 'connected') => source.entries.set(key, { key, origin: key.split('\n')[0]!, environmentId: key.split('\n')[1]!, phase: phase as 'connected', message: '', traceId: '',
    generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {}, config: cfg, shell: initialShell(), scopes: [], error: '', requested: true });
  live(keyB, config('Box B', '0.0.40', 'respawn'));
  live(keyC, config('Box C', '0.0.40', 'desktop-managed'));          // desktop-managed without the remote trigger: manual only
  live(keyD, config('Box D', '0.0.40', 'boot-service'), 'error');    // not connected
  const saved = [
    { origin: 'http://127.0.0.1:3773', environmentId: 'env-a', enabled: true },
    { origin: 'http://10.0.0.2:3773', environmentId: 'env-b', enabled: true },
    { origin: 'http://10.0.0.3:3773', environmentId: 'env-c', enabled: true },
    { origin: 'http://10.0.0.4:3773', environmentId: 'env-d', enabled: true },
  ];
  const client = { config: config('This Mac', '0.0.39', 'respawn'), generation: 3 } as unknown as T3Client;
  const focus = { origin: 'http://127.0.0.1:3773', environmentId: 'env-a', connection: 'connected' };
  const targets = updateTargets(saved, focus, client, undefined, source);
  expect(targets.map(target => [target.label, target.selfUpdate, target.generation])).toEqual([['This Mac server', 'respawn', 3], ['Box B server', 'respawn', 7]]);
  expect(updateTargets(saved, { ...focus, connection: 'connecting' }, client, undefined, source).map(target => target.label)).toEqual(['Box B server']);
  expect(CONNECTION_OPS).toContain('environment-update-all');
});

test('Update all starts the shared update job for each eligible environment and reports none eligible', async () => {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, later: async (input: unknown) => {
    const request = obj(input); calls.push(request);
    if (request.op === 'environments') return { ok: true, generation: 3, value: { saved: [{ origin: 'http://127.0.0.1:3773', environmentId: 'env-a', enabled: true }] } };
    return { ok: true, generation: 3, value: { started: true, attempt: '1' } };
  } };
  const client = { origin: 'http://127.0.0.1:3773', environmentId: 'env-a', connection: 'connected', generation: 3, config: config('This Mac', '0.0.39', 'respawn') } as unknown as T3Client;
  await runConnectionOp(native, 'environment-update-all', '', '', true, client);
  // server-update.ts: one T3Fleet job per environment, in the connected mode, toward this client's version.
  expect(calls.filter(call => call.op === 'fleetOutdatedUpdate').map(call => [call.fleet, call.mode, call.targetVersion, call.label]))
    .toEqual([['http://127.0.0.1:3773\nenv-a', 'connected', CLIENT_VERSION, 'This Mac server']]);
  expect(calls.some(call => call.op === 'request')).toBe(false);
  const current = { ...client, config: config('This Mac', CLIENT_VERSION, 'respawn') } as unknown as T3Client;
  await expect(runConnectionOp(native, 'environment-update-all', '', '', true, current)).rejects.toThrow('No saved environment can update itself');
});
