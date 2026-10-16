// Pairing a saved environment again (EnvironmentRegistry.register → installEntryLocked) restarts its connection, so a
// background environment that failed for want of a credential reconnects with the new one and its row stops saying so.
import { afterEach, expect, test } from 'bun:test';
import { connectionsPage, runConnectionOp, type ConnectionHost } from './connections';
import { fleet, environmentKey } from './settings-b-fleet';
import { resetPrimary } from './local-primary-fixture';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';

afterEach(() => { resetPrimary(); fleet.entries.clear(); });

const REMOTE = 'https://second.example.com', KEY = environmentKey(REMOTE, 'env-b');
const MISSING = 'Paste a pairing link or token from this T3 server.';
const host: ConnectionHost = { connection: 'connected', origin: 'http://127.0.0.1:16210', environmentId: 'env-a', statusMessage: 'Connected.', scopes: [],
  config: { environment: { label: 'This Mac', platform: { machine: 'laptop' } } } };

/** T3Fleet: a background transport keeps its failed state until it is stopped; a new one reads the Keychain when it connects. */
class Transports implements Native {
  available = true; calls: Obj[] = [];
  saved: Obj[] = [{ origin: REMOTE, environmentId: 'env-b', label: 'Lane Third', machine: 'laptop', enabled: true }];
  token = ''; transports = new Map<string, Obj>();
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input), ok = (value: unknown, generation = 1) => ({ ok: true, generation, value });
    this.calls.push(request);
    const key = String(request.fleet ?? '');
    if (key) {
      if (request.op === 'fleetStop') return ok({ stopped: this.transports.delete(key) }, 0);
      if (request.op === 'connect') {
        this.transports.set(key, this.token
          ? { state: 'connected', origin: REMOTE, environmentId: 'env-b', message: 'Connected.' }
          : { state: 'error', origin: REMOTE, environmentId: 'env-b', failureKind: 'Credential', message: MISSING, traceId: '' });
        return ok(this.transports.get(key));
      }
      const transport = this.transports.get(key);
      if (request.op === 'status') return ok(transport ?? { state: 'disconnected', origin: '', environmentId: '', message: '' });
      return { ok: false, generation: 1, error: { kind: 'Closed', message: 'Not in this fixture.', uncertain: false } };
    }
    if (request.op === 'environments') return ok({ saved: this.saved });
    if (request.op === 'pairEnvironment') { this.token = 'bearer'; return ok({ origin: request.origin, environmentId: 'env-b', label: 'Lane Third' }); }
    if (request.op === 'connectionPreferences') return ok({ text: '{}' });
    return ok({});
  }
}
const row = async (native: Native) => (await connectionsPage(host, native, true)).environments.find(entry => entry.environmentId === 'env-b')!;

test('pairing a saved environment whose credential is gone reconnects it and refreshes its row', async () => {
  const native = new Transports(), focus = { origin: host.origin, environmentId: 'env-a', connection: 'connected' };
  await fleet.sync(native, focus); // connects it: no credential in the Keychain
  await fleet.sync(native, focus);
  expect(fleet.entries.get(KEY)).toMatchObject({ phase: 'error', message: MISSING });
  expect((await row(native)).subtitle).toContain(`Connection failed: ${MISSING}`);

  await runConnectionOp(native, 'environment-add', REMOTE, 'PAIRCODE', true, focus as never);
  expect(native.calls.filter(call => call.fleet === KEY && call.op === 'fleetStop')).toHaveLength(1);
  await fleet.sync(native, focus);
  expect(fleet.entries.get(KEY)).toMatchObject({ phase: 'connected', message: '' });
  expect((await row(native)).subtitle).not.toContain('Connection failed');
  expect(native.calls.filter(call => call.fleet === KEY && call.op === 'connect')).toHaveLength(2);
});

test('pairing a new environment stops nothing', async () => {
  const native = new Transports();
  native.saved = [];
  await runConnectionOp(native, 'environment-add', REMOTE, 'PAIRCODE', true, { origin: host.origin, environmentId: 'env-a', connection: 'connected' } as never);
  expect(native.calls.filter(call => call.op === 'fleetStop')).toEqual([]);
});
