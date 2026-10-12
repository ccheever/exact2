// T3 Code 1e2ecbd975 (5318d054a5) has no tests for this change; these pin the
// ported rules: localPlatformsUnavailable (DeviceSetup.tsx, IntegrationsSettings.tsx)
// and the Integrations row's reveal (platformsRevealed).
import { describe, expect, it } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { localPlatformsUnavailable, simulatorSupportRows } from './device-support';

const host = (kind: string, available: boolean[]): Obj => ({ id: kind, kind, label: kind, platforms: available.map((value, index) => ({ platform: index ? 'android' : 'ios', available: value, reason: value ? undefined : 'Not here.' })) });
const state = (hostStatus: string, hosts: Obj[]): Obj => ({ hostStatus, hosts, devices: [], hostStatuses: {} });
const client = () => ({ environmentId: 'env', ready: true, config: { environment: { label: 'Studio' } } }) as unknown as T3Client;

describe('localPlatformsUnavailable', () => {
  it('is true when some local host has no available platform', () => {
    expect(localPlatformsUnavailable(state('idle', [host('local', [false, false])]))).toBe(true);
    expect(localPlatformsUnavailable(state('idle', [host('local', [false, true])]))).toBe(false);
    expect(localPlatformsUnavailable(state('idle', [host('ssh', [false, false])]))).toBe(false);
    expect(localPlatformsUnavailable(null)).toBe(false);
  });
});

describe('Simulator support row (DeviceIntegrationControls platformsRevealed)', () => {
  it('stays hidden while the hub is off', () => {
    expect(simulatorSupportRows(client(), 'env', false, state('disabled', [host('local', [false, false])]), 1)).toEqual([]);
  });

  it('reveals when the hub is on and the local host lacks every platform, before the hub is ready', () => {
    const rows = simulatorSupportRows(client(), 'env', true, state('installing', [host('local', [false, false])]), 1);
    expect(rows.length).toBe(1);
    expect(rows[0].platforms.map(platform => [platform.platform, platform.ready, platform.message])).toEqual([['iOS', false, 'Not here.'], ['Android', false, 'Not here.']]);
    expect(rows[0].disabled).toBe(true);
    expect(rows[0].description).toBe('');
  });

  it('reveals once the hub is ready, stays revealed while on, and resets when the hub turns off', () => {
    const owner = client();
    expect(simulatorSupportRows(owner, 'env', true, state('starting', [host('local', [true, true])]), 1)).toEqual([]);
    expect(simulatorSupportRows(owner, 'env', true, state('ready', [host('local', [true, true])]), 1).length).toBe(1);
    expect(simulatorSupportRows(owner, 'env', true, state('starting', [host('local', [true, true])]), 1).length).toBe(1);
    expect(simulatorSupportRows(owner, 'env', false, state('disabled', [host('local', [true, true])]), 1)).toEqual([]);
    expect(simulatorSupportRows(owner, 'env', true, state('starting', [host('local', [true, true])]), 1)).toEqual([]);
  });

  it('names the inspected environment when several are connected', () => {
    const [row] = simulatorSupportRows(client(), 'env', true, state('ready', [host('local', [true, true])]), 2);
    expect(row.description).toBe('Status for Studio. Select an environment to inspect its simulator support.');
    expect(row.disabled).toBe(false);
  });
});
