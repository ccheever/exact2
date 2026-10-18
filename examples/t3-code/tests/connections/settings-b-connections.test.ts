import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import { test, expect } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { contractFn } from '../support/contract-fn-fixture';
import { updateTargets, runConnectionOp, CONNECTION_OPS, CLIENT_VERSION } from '../../src/connections/connections';
import { EnvironmentFleet, environmentKey } from '../../src/connections/settings-b-fleet';
import { initialShell, obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import type { T3Client } from '../../src/client/client';

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
  const client = { scopes: [...AUTH_STANDARD_CLIENT_SCOPES], origin: 'http://127.0.0.1:3773', environmentId: 'env-a', connection: 'connected', generation: 3, config: config('This Mac', '0.0.39', 'respawn') } as unknown as T3Client;
  await runConnectionOp(native, 'environment-update-all', '', '', true, client);
  // server-update.ts: one update job per environment, in the connected mode, toward this client's version.
  expect(calls.filter(call => call.op === 'fleetOutdatedUpdate').map(call => [call.fleet, call.mode, call.targetVersion, call.label]))
    .toEqual([['http://127.0.0.1:3773\nenv-a', 'connected', CLIENT_VERSION, 'This Mac server']]);
  expect(calls.some(call => call.op === 'request')).toBe(false);
  const current = { ...client, config: config('This Mac', CLIENT_VERSION, 'respawn') } as unknown as T3Client;
  await expect(runConnectionOp(native, 'environment-update-all', '', '', true, current)).rejects.toThrow('No saved environment can update itself');
});


// The Icon row is also a submenu trigger: activation opens it, arrows belong to the parent menu.

test('the environment Icon submenu opens its actual row without taking parent arrow navigation', async () => {
  const menu = contractComponent('connections.contract', 'EnvironmentIconMenu');
  const kmOpenKey = await contractFn('menu-keys.contract', 'kmOpenKey', ['k']);
  const kmTriggerTarget = await contractFn('menu-keys.contract', 'kmTriggerTarget', ['items', 'k']);
  const environment = { environmentId: 'remote', iconLock: '', icons: [{ kind: 'desktop', label: 'Desktop' }, { kind: 'server', label: 'Server' }] };
  const calls: unknown[][] = [];
  const state = menu.state();
  const scope = {
    environment, KmItem: (item: unknown) => item, kmOpenKey, kmTriggerTarget,
    preventDefault: () => calls.push(['preventDefault']), stopPropagation: () => calls.push(['stopPropagation']),
    showPopover: (...args: unknown[]) => calls.push(['showPopover', ...args]), focus: (id: string) => calls.push(['focus', id]),
  };
  const derived = menu.derives(state, scope);
  expect(derived.menuItems).toEqual([{ id: 'environment-icon-remote-desktop', label: 'Desktop' }, { id: 'environment-icon-remote-server', label: 'Server' }]);
  const event = { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false };
  for (const key of ['Enter', ' ', 'Space']) {
    calls.length = 0;
    menu.call('triggerKey', state, { ...scope, ...derived }, [key, event]);
    expect(calls).toEqual([
      ['preventDefault'], ['stopPropagation'], ['showPopover', 'environment-icon-remote-popup', { source: 'environment-icon-remote' }],
      ['focus', 'environment-icon-remote-desktop'],
    ]);
  }
  for (const key of ['ArrowDown', 'ArrowUp']) {
    calls.length = 0;
    menu.call('triggerKey', state, { ...scope, ...derived }, [key, event]);
    expect(calls).toEqual([]);
  }
  expect(menu.derives(state, { ...scope, environment: { ...environment, iconLock: 'Managed by host' } }).menuItems).toEqual([]);
});
