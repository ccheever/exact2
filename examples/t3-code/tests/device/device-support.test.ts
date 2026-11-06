// T3 Code 1e2ecbd975 (5318d054a5) has no tests for this change; these pin the
// ported rule localPlatformsUnavailable (DeviceSetup.tsx).
import { describe, expect, it } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import { localPlatformsUnavailable } from '../../src/device/device-surface';

const host = (kind: string, available: boolean[]): Obj => ({ id: kind, kind, label: kind, platforms: available.map((value, index) => ({ platform: index ? 'android' : 'ios', available: value, reason: value ? undefined : 'Not here.' })) });
const state = (hostStatus: string, hosts: Obj[]): Obj => ({ hostStatus, hosts, devices: [], hostStatuses: {} });

describe('localPlatformsUnavailable', () => {
  it('is true when some local host has no available platform', () => {
    expect(localPlatformsUnavailable(state('idle', [host('local', [false, false])]))).toBe(true);
    expect(localPlatformsUnavailable(state('idle', [host('local', [false, true])]))).toBe(false);
    expect(localPlatformsUnavailable(state('idle', [host('ssh', [false, false])]))).toBe(false);
    expect(localPlatformsUnavailable(null)).toBe(false);
  });
});
