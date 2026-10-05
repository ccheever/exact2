import { test, expect } from 'bun:test';
import { connectionsProjection, type ConnectionHost } from './connections';
import { balanceSources } from './r11-misc-connections';
import { environmentKey, type FleetEntry } from './settings-b-fleet';
import { initialShell } from './domain';

const A = 'http://127.0.0.1:14993', B = 'http://127.0.0.1:14994';
const focusB = (): ConnectionHost => ({ connection: 'connected', origin: B, environmentId: 'env-b', statusMessage: 'Connected.', scopes: ['orchestration:read'],
  config: { environment: { label: 'Mac B', platform: { machine: 'laptop' } } } });
const live = (origin: string, environmentId: string): FleetEntry => ({ key: environmentKey(origin, environmentId), origin, environmentId, phase: 'connected', message: '',
  traceId: '', generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: [], error: '', requested: true });

test('the primary (this Mac) leads and counts while switched off; other switched-off machines do not', () => {
  const rows = [{ key: 'a', origin: A, enabled: false }, { key: 'r', origin: 'https://remote.example.com', enabled: true },
    { key: 'b', origin: B, enabled: true }, { key: 'c', origin: 'https://off.example.com', enabled: false }];
  expect(balanceSources(rows).map(({ source, primary }) => [source.key, primary])).toEqual([['a', true], ['r', false], ['b', false]]);
  // No server on this Mac: no primary, only switched-on machines (round 9's rule).
  expect(balanceSources(rows.filter(row => row.origin.startsWith('https'))).map(({ source }) => source.key)).toEqual(['r']);
  // Two servers on this Mac: the first saved one is the primary.
  expect(balanceSources(rows.slice(1)).map(({ source, primary }) => [source.key, primary])).toEqual([['b', true], ['r', false]]);
});

test('A switched off and B added: Load balancing and GitHub sharing show A (This machine) and B, as the reference', () => {
  const saved = [{ origin: A, environmentId: 'env-a', label: 'Mac A', enabled: false }, { origin: B, environmentId: 'env-b', label: 'Mac B', enabled: true }];
  const page = connectionsProjection(focusB(), saved, new Map([[environmentKey(B, 'env-b'), live(B, 'env-b')]]));
  expect(page.machines.map(machine => [machine.label, machine.subtitle, machine.first])).toEqual([['Mac A', 'This machine', true], ['Mac B', 'http://127.0.0.1:14994/', false]]);
  expect(page).toMatchObject({ loadBalancing: false, loadSummary: 'Off', githubSummary: 'Off' });
  // The single saved environment alone has nothing to balance against.
  expect(connectionsProjection(focusB(), saved.slice(1), new Map()).machines).toEqual([]);
  // Two remote machines with one switched off: one machine, hidden.
  const remote = [{ origin: 'https://one.example.com', environmentId: 'e1', label: 'One', enabled: false }, { origin: 'https://two.example.com', environmentId: 'e2', label: 'Two', enabled: true }];
  expect(connectionsProjection({ ...focusB(), origin: 'https://two.example.com', environmentId: 'e2' }, remote, new Map()).machines).toEqual([]);
});
