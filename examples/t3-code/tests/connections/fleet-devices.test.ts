// A background environment's device sessions (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3:
// components/preview/agentOpenedPreviews.ts startAgentOpenedPreviews watchEnvironmentDevices) and its session read's
// timeout ladder (client-runtime state/session.ts fetchEnvironmentSessionState).
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { EnvironmentFleet, environmentKey, fleetDeviceStates, readSessionState } from '../../src/connections/settings-b-fleet';

class FleetNative implements Native {
  scopes = ['orchestration:read', 'orchestration:operate'];
  available = true; calls: Obj[] = []; saved: Obj[] = []; states: Record<string, string> = {}; events: Record<string, Obj[]> = {}; generation = 3;
  sessionTimeouts = 0;
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input), key = String(request.fleet ?? ''); this.calls.push(request);
    const ok = (value: unknown, generation = this.generation) => ({ ok: true, generation, value });
    if (!key) return request.op === 'environments' ? ok({ saved: this.saved }) : { ok: false, generation: 0, error: { kind: 'Arguments', message: 'unexpected', uncertain: false } };
    if (request.op === 'connect') { this.states[key] = 'connected'; return ok({ state: 'connected' }); }
    if (request.op === 'status') return ok({ state: this.states[key] ?? 'disconnected', message: '', failureKind: '', traceId: '' });
    if (request.op === 'http' && request.path === '/api/auth/session') {
      if (this.sessionTimeouts > 0) { this.sessionTimeouts--; return { ok: false, generation: this.generation, error: { kind: 'Network', message: 'timed out', uncertain: false, reason: 'timeout' } }; }
      return ok({ authenticated: true, scopes: this.scopes });
    }
    if (request.op === 'http' && request.path === '/api/orchestration/shell') return ok({ snapshotSequence: 4, projects: [], threads: [] });
    if (request.op === 'request' && request.method === 'server.getConfig') return ok({ environment: { environmentId: 'env-b', label: 'Box' }, providers: [], settings: {} });
    if (request.op === 'request') return ok({});
    if (request.op === 'subscribe') return ok({ id: `${request.key}-sub` });
    if (request.op === 'events') return ok({ events: this.events[key] ?? [], latest: 0 });
    if (request.op === 'ack') return ok({ latest: 0 });
    return ok({});
  }
}
const focus = { origin: 'http://localhost:3773', environmentId: 'env-a', connection: 'connected' };
const key = environmentKey('https://box.example.com', 'env-b');
const hub = (sessions: Obj[]) => ({ hosts: [{ id: 'local', kind: 'local', platforms: [] }], devices: [{ hostId: 'local', id: 'sim-1', name: 'iPhone 17', platform: 'ios' }], sessions });

describe('a background environment\'s devices', () => {
  test('every connected environment subscribes to its device hub, so a device an agent opens there is seen', async () => {
    const native = new FleetNative(), source = new EnvironmentFleet();
    native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
    await source.sync(native, focus); await source.sync(native, focus);
    const subscriptions = () => native.calls.filter(call => call.method === 'subscribeDeviceState');
    expect(subscriptions()).toEqual([expect.objectContaining({ op: 'subscribe', key: 'device-state', payload: {}, fleet: key })]);
    expect(fleetDeviceStates(source)).toEqual([]);
    const session = { threadId: 't1', hostId: 'local', deviceId: 'sim-1', openedAt: '2026-10-27T10:00:00Z' };
    native.events[key] = [{ seq: 1, key: 'device-state', subscriptionId: 'device-state-sub', generation: native.generation, value: hub([session]) }];
    await source.sync(native, focus);
    expect(fleetDeviceStates(source)).toMatchObject([{ environmentId: 'env-b', state: { sessions: [session] } }]);
    // A stream due again subscribes on the next pass; a failure keeps the last state.
    native.events[key] = [{ seq: 2, key: 'device-state', subscriptionId: 'device-state-sub', generation: native.generation, value: { _transportError: { message: 'gone' } } }];
    await source.sync(native, focus);
    expect(fleetDeviceStates(source)).toHaveLength(1);
    native.events[key] = [{ seq: 3, key: 'device-state', subscriptionId: 'device-state-sub', generation: native.generation, value: { _retryDue: true } }];
    await source.sync(native, focus);
    expect(subscriptions()).toHaveLength(2);
  });

  test('a session without orchestration:read never asks for devices', async () => {
    const native = new FleetNative(), source = new EnvironmentFleet();
    native.scopes = [];
    native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
    await source.sync(native, focus); await source.sync(native, focus);
    expect(native.calls.some(call => call.method === 'subscribeDeviceState')).toBe(false);
  });
});

describe('the session read (fetchEnvironmentSessionState)', () => {
  const timingOut = (timeouts: number) => {
    const asked: number[] = [];
    let left = timeouts;
    const call = async (request: Obj) => {
      asked.push(Number(request.timeoutMs));
      return left-- > 0 ? { ok: false, generation: 1, error: { kind: 'Network', message: 'Remote environment endpoint timed out.', reason: 'timeout' } }
        : { ok: true, generation: 1, value: { authenticated: true, scopes: ['orchestration:read'] } };
    };
    return { asked, call };
  };
  test('a timeout asks again at double the time, from 6 s up to 60 s, then fails', async () => {
    const slow = timingOut(2);
    expect(await readSessionState(slow.call, 1)).toMatchObject({ authenticated: true });
    expect(slow.asked).toEqual([6_000, 12_000, 24_000]);
    const stuck = timingOut(10);
    await expect(readSessionState(stuck.call, 1)).rejects.toThrow('timed out');
    expect(stuck.asked).toEqual([6_000, 12_000, 24_000, 48_000, 60_000]);
  });
  test('another failure ends it at once, and a reply from another connection is refused', async () => {
    const asked: Obj[] = [];
    await expect(readSessionState(async request => { asked.push(request); return { ok: false, generation: 1, error: { kind: 'Authentication', message: 'expired' } }; }, 1)).rejects.toThrow('expired');
    expect(asked).toHaveLength(1);
    await expect(readSessionState(async () => ({ ok: true, generation: 2, value: {} }), 1)).rejects.toThrow('The connection changed.');
  });
  test('the fleet bootstrap waits out a slow host instead of failing it', async () => {
    const native = new FleetNative(), source = new EnvironmentFleet();
    native.sessionTimeouts = 1;
    native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
    await source.sync(native, focus); await source.sync(native, focus);
    expect(native.calls.filter(call => call.path === '/api/auth/session').map(call => call.timeoutMs)).toEqual([6_000, 12_000]);
    const entry = source.entries.get(key)!;
    expect(entry).toMatchObject({ phase: 'connected', synchronized: native.generation });
    expect(entry.scopes).toContain('orchestration:read');
  });
});
