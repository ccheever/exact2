// A background environment's device sessions (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3:
// components/preview/agentOpenedPreviews.ts startAgentOpenedPreviews watchEnvironmentDevices) and its grants, read on
// their own as the per-environment session atom reads them (client-runtime state/session.ts sessionStateAtom; the
// transport's readSession asks again from 6 s up to 60 s on a timeout).
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { EnvironmentFleet, environmentKey, fleetDeviceStates } from '../../src/connections/settings-b-fleet';

class FleetNative implements Native {
  scopes = ['orchestration:read', 'orchestration:operate'];
  available = true; calls: Obj[] = []; saved: Obj[] = []; states: Record<string, string> = {}; events: Record<string, Obj[]> = {}; generation = 3;
  /** The session read's answer, filed in the inbox once the test lets it arrive (`answerSession`). */
  sessionReads = 0; sessionAnswer: Obj | null = null;
  watch() {}
  answerSession(reply: Obj = { _reply: { authenticated: true, scopes: this.scopes } }) { this.sessionAnswer = reply; }
  async later(input: unknown): Promise<unknown> {
    const request = obj(input), key = String(request.fleet ?? ''); this.calls.push(request);
    const ok = (value: unknown, generation = this.generation) => ({ ok: true, generation, value });
    if (!key) return request.op === 'environments' ? ok({ saved: this.saved }) : { ok: false, generation: 0, error: { kind: 'Arguments', message: 'unexpected', uncertain: false } };
    if (request.op === 'connect') { this.states[key] = 'connected'; return ok({ state: 'connected' }); }
    if (request.op === 'status') return ok({ state: this.states[key] ?? 'disconnected', message: '', failureKind: '', traceId: '' });
    if (request.op === 'readSession') return ok({ id: String(++this.sessionReads) });
    if (request.op === 'events' && this.sessionAnswer && this.sessionReads > 0) {
      const answer = { seq: 100 + this.sessionReads, key: 'session-state', subscriptionId: String(this.sessionReads), generation: this.generation, value: this.sessionAnswer };
      this.sessionAnswer = null;
      return ok({ events: [answer, ...(this.events[key] ?? [])], latest: 0 });
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
    native.answerSession();
    await source.sync(native, focus); await source.sync(native, focus); await source.sync(native, focus);
    const subscriptions = () => native.calls.filter(call => call.method === 'subscribeDeviceState');
    expect(subscriptions()).toEqual([expect.objectContaining({ op: 'subscribe', key: 'device-state', payload: {}, fleet: key })]);
    expect(fleetDeviceStates(source)).toEqual([]);
    const session = { threadId: 't1', hostId: 'local', deviceId: 'sim-1', openedAt: '2026-10-27T10:00:00Z' };
    native.events[key] = [{ seq: 2001, key: 'device-state', subscriptionId: 'device-state-sub', generation: native.generation, value: hub([session]) }];
    await source.sync(native, focus);
    expect(fleetDeviceStates(source)).toMatchObject([{ environmentId: 'env-b', state: { sessions: [session] } }]);
    // A stream due again subscribes on the next pass; a failure keeps the last state.
    native.events[key] = [{ seq: 2002, key: 'device-state', subscriptionId: 'device-state-sub', generation: native.generation, value: { _transportError: { message: 'gone' } } }];
    await source.sync(native, focus);
    expect(fleetDeviceStates(source)).toHaveLength(1);
    native.events[key] = [{ seq: 2003, key: 'device-state', subscriptionId: 'device-state-sub', generation: native.generation, value: { _retryDue: true } }];
    await source.sync(native, focus);
    expect(subscriptions()).toHaveLength(2);
  });

  test('a session without orchestration:read never asks for devices', async () => {
    const native = new FleetNative(), source = new EnvironmentFleet();
    native.scopes = [];
    native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
    native.answerSession();
    await source.sync(native, focus); await source.sync(native, focus); await source.sync(native, focus);
    expect(native.calls.some(call => call.method === 'subscribeDeviceState')).toBe(false);
  });
});

describe('a background environment\'s grants (sessionStateAtom)', () => {
  test('the config and the shell do not wait on the session read; the grants arrive on their own', async () => {
    const native = new FleetNative(), source = new EnvironmentFleet();
    native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
    await source.sync(native, focus); await source.sync(native, focus);
    const entry = source.entries.get(key)!;
    // Synchronized with no grants yet: the read was asked, never awaited, and nothing reads /api/auth/session itself.
    expect(entry).toMatchObject({ phase: 'connected', synchronized: native.generation, scopes: [] });
    expect(native.calls.filter(call => call.op === 'readSession')).toEqual([expect.objectContaining({ fleet: key })]);
    expect(native.calls.some(call => call.path === '/api/auth/session')).toBe(false);
    expect(native.calls.some(call => call.method === 'subscribeDeviceState')).toBe(false);
    native.answerSession();
    await source.sync(native, focus); await source.sync(native, focus);
    expect(entry.scopes).toContain('orchestration:read');
    expect(native.calls.some(call => call.method === 'subscribeDeviceState')).toBe(true);
  });
  test('a failed read grants nothing, and a new connection reads its grants again', async () => {
    const native = new FleetNative(), source = new EnvironmentFleet();
    native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
    native.answerSession({ _replyError: { kind: 'Network', message: 'timed out', reason: 'timeout' } });
    await source.sync(native, focus); await source.sync(native, focus); await source.sync(native, focus);
    const entry = source.entries.get(key)!;
    expect(entry.scopes).toEqual([]);
    native.generation = 4;
    await source.sync(native, focus);
    expect(native.calls.filter(call => call.op === 'readSession')).toHaveLength(2);
    native.answerSession();
    await source.sync(native, focus);
    expect(entry.scopes).toContain('orchestration:read');
  });
  test('a new connection keeps the last grants until its own read lands (stale-while-revalidate)', async () => {
    const native = new FleetNative(), source = new EnvironmentFleet();
    native.saved = [{ origin: 'https://box.example.com', environmentId: 'env-b', enabled: true }];
    native.answerSession();
    await source.sync(native, focus); await source.sync(native, focus); await source.sync(native, focus);
    const entry = source.entries.get(key)!;
    expect(entry.scopes).toContain('orchestration:operate');
    native.generation = 4;
    await source.sync(native, focus);
    // Synchronized on the new connection, its read asked and not answered: the last grants still stand.
    expect(entry).toMatchObject({ synchronized: 4 });
    expect(native.calls.filter(call => call.op === 'readSession')).toHaveLength(2);
    expect(entry.scopes).toContain('orchestration:operate');
    native.scopes = ['orchestration:read'];
    native.answerSession();
    await source.sync(native, focus);
    expect(entry.scopes).toContain('orchestration:read');
    expect(entry.scopes).not.toContain('orchestration:operate');
  });
});

