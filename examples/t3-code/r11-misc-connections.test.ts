import { afterEach, test, expect } from 'bun:test';
import { connectionsProjection, type ConnectionHost } from './connections';
import { balanceSources } from './r11-misc-connections';
import { environmentKey, type FleetEntry } from './settings-b-fleet';
import { initialShell } from './domain';
import { primaryAt, primaryOff, resetPrimary } from './local-primary-fixture';

afterEach(resetPrimary);
const A = 'http://127.0.0.1:14993', B = 'http://127.0.0.1:14994';
const focusB = (): ConnectionHost => ({ connection: 'connected', origin: B, environmentId: 'env-b', statusMessage: 'Connected.', scopes: ['orchestration:read'],
  config: { environment: { label: 'Mac B', platform: { machine: 'laptop' } } } });
const live = (origin: string, environmentId: string, primary = false): FleetEntry => ({ key: environmentKey(origin, environmentId), origin, environmentId, phase: 'connected', message: '',
  traceId: '', generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: [], error: '', requested: true, primary });

test('the primary (this Mac) leads and always counts; switched-off saved machines do not', () => {
  const rows = [{ key: 'r', origin: 'https://remote.example.com', enabled: true }, { key: 'p', origin: A, enabled: true, primary: true },
    { key: 'b', origin: B, enabled: true }, { key: 'c', origin: 'https://off.example.com', enabled: false }];
  expect(balanceSources(rows).map(({ source, primary }) => [source.key, primary])).toEqual([['p', true], ['r', false], ['b', false]]);
  // The Local environment switched off: no primary, only switched-on machines (a loopback saved one is not "This machine").
  expect(balanceSources(rows.filter(row => !row.primary)).map(({ source, primary }) => [source.key, primary])).toEqual([['r', false], ['b', false]]);
});

test('this machine and B: Load balancing and GitHub sharing show This machine first, then B, as the reference', () => {
  primaryAt(A, 'env-a', 'Mac A');
  const saved = [{ origin: B, environmentId: 'env-b', label: 'Mac B', enabled: true }];
  const page = connectionsProjection(focusB(), saved, new Map([[environmentKey(B, 'env-b'), live(B, 'env-b')], [environmentKey(A, 'env-a'), live(A, 'env-a', true)]]));
  expect(page.machines.map(machine => [machine.label, machine.subtitle, machine.first])).toEqual([['Mac A', 'This machine', true], ['Mac B', 'http://127.0.0.1:14994/', false]]);
  expect(page).toMatchObject({ loadBalancing: false, loadSummary: 'Off', githubSummary: 'Off' });
  // The primary has no row under Environments.
  expect(page.environments.map(row => row.label)).toEqual(['Mac B']);
  // Switched off: B alone has nothing to balance against.
  primaryOff();
  expect(connectionsProjection(focusB(), saved, new Map()).machines).toEqual([]);
  // Two remote machines with one switched off: one machine, hidden.
  const remote = [{ origin: 'https://one.example.com', environmentId: 'e1', label: 'One', enabled: false }, { origin: 'https://two.example.com', environmentId: 'e2', label: 'Two', enabled: true }];
  expect(connectionsProjection({ ...focusB(), origin: 'https://two.example.com', environmentId: 'e2' }, remote, new Map()).machines).toEqual([]);
});
