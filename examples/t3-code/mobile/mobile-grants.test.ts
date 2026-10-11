import { expect, test } from 'bun:test';
import { mobileNative } from './client';
import { mobileGrantRequest, mobileSessionGrants, MOBILE_STANDARD_CLIENT_SCOPES } from './mobile-grants';
import { withStandardScope } from './shared/remote-scopes';
import { obj, type Obj } from './shared/domain';
import { T3Client } from './shared/client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { ClientError, type Native } from './shared/protocol';
import { mobileThreadLocalDock } from './thread-local-dock';

const expected = ['orchestration:read', 'orchestration:operate', 'settings:write', 'providers:manage',
  'environment:maintain', 'preview:operate', 'diagnostics:read', 'terminal:read', 'terminal:operate',
  'source-control:write', 'filesystem:read', 'filesystem:write', 'relay:read'];
test('actual mobile native boundary requests pinned granular scopes on every credential exchange', async () => {
  const requests: Obj[] = [];
  const native = mobileNative({ available: true, watch() {}, async later(input) { requests.push(obj(input)); return {}; } });
  for (const op of ['connect', 'pairEnvironment', 'fleetOutdatedPair']) {
    const request = { op, ...withStandardScope({ credential: 'synthetic-bootstrap' }), expectedEnvironmentId: 'one' };
    await native.later(request);
    expect(requests.at(-1)).toEqual({ ...request, scope: expected.join(' ') });
  }
  expect([...MOBILE_STANDARD_CLIENT_SCOPES]).toEqual(expected);
});
test('saved-token reconnects and unrelated requests retain exact identity and no broadened grant', () => {
  for (const request of [{ op: 'connect', credential: '' }, { op: 'retry' }, { op: 'http', path: '/api/auth/session' },
    { op: 'request', credential: 'synthetic', scope: 'orchestration:read' }]) expect(mobileGrantRequest(request)).toBe(request);
});
test('all nine pinned legacy parent fallbacks apply only to old session metadata', () => {
  const parents = {
    'filesystem:read': 'orchestration:read', 'diagnostics:read': 'orchestration:read',
    'settings:write': 'orchestration:operate', 'providers:manage': 'orchestration:operate',
    'environment:maintain': 'orchestration:operate', 'preview:operate': 'orchestration:operate',
    'source-control:write': 'orchestration:operate', 'filesystem:write': 'orchestration:operate', 'terminal:read': 'terminal:operate',
  };
  for (const [permission, parent] of Object.entries(parents)) {
    const legacy = { authenticated: true, scopes: [parent] };
    expect(mobileSessionGrants(legacy, permission)).toBe(true);
    expect(mobileSessionGrants({ ...legacy, auth: { serverUpdateScope: 'environment:maintain' } }, permission)).toBe(false);
    expect(mobileSessionGrants({ ...legacy, permissions: [] }, permission)).toBe(false);
    expect(mobileSessionGrants({ ...legacy, permissions: [parent] }, permission)).toBe(false);
    expect(mobileSessionGrants({ ...legacy, permissions: [permission] }, permission)).toBe(true);
    expect(mobileSessionGrants({ ...legacy, authenticated: false }, permission)).toBe(false);
  }
  expect(mobileSessionGrants({ authenticated: true, scopes: ['orchestration:operate'] }, 'access:write')).toBe(false);
});
test('requesting granular scopes never substitutes for actual constrained server permissions', () => {
  const grant = ['orchestration:read', 'relay:read'];
  const issued = MOBILE_STANDARD_CLIENT_SCOPES.filter(scope => grant.includes(scope));
  expect(mobileSessionGrants({ authenticated: true, permissions: issued }, 'filesystem:read')).toBe(false);
  expect(mobileSessionGrants({ authenticated: true, permissions: 'filesystem:read', scopes: ['filesystem:read'] }, 'filesystem:read')).toBe(false);
});

function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
function sessionFixture(session: Obj, hook?: (request: Obj) => unknown | Promise<unknown>, client: T3Client = new T3Client()) {
  const background = new EnvironmentFleet(), requests: Obj[] = [];
  Object.assign(client, { generation: 3, environmentId: 'one', origin: 'https://one.test', connection: 'connected', shellLoaded: true });
  const native: Native = mobileNative({ available: true, watch() {}, async later(input) {
    const request = obj(input); requests.push(request);
    const override = await hook?.(request); if (override !== undefined) return override;
    const value = request.op === 'environments' ? { saved: [{ environmentId: 'two', origin: 'https://two.test' }] }
      : request.op === 'status' ? { state: 'connected', origin: 'https://two.test', environmentId: 'two', message: '' }
      : request.path === '/api/auth/session' ? session
      : request.method === 'server.getConfig' ? { providers: [], environment: { environmentId: request.fleet ? 'two' : 'one',
        orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } } }
      : request.path === '/api/orchestration/shell' ? { snapshotSequence: 1, projects: [], threads: [] }
      : request.op === 'subscribe' ? { id: `sub-${requests.length}` }
      : request.op === 'events' ? { events: [], latest: 0 } : {};
    return { ok: true, generation: 3, value };
  } }, client, background);
  const syncFleet = () => background.sync(native, { origin: client.origin, environmentId: client.environmentId, connection: 'connected' });
  return { client, background, native, requests, syncFleet };
}

test('focused and fleet bootstrap use actual mobile boundary for current server granular grants', async () => {
  // authScopeResponse deliberately omits granular permissions from its legacy scopes.
  const session = { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate'],
    permissions: ['orchestration:read', 'orchestration:operate', 'providers:manage'], auth: { serverUpdateScope: 'environment:maintain' } };
  const before = JSON.stringify(session), f = sessionFixture(session);
  await f.client.synchronize(f.native); await f.syncFleet();
  expect(f.client.writable).toBe(true);
  expect(f.client.scopes).toEqual(session.permissions);
  expect([...f.background.entries.values()][0]?.scopes).toEqual(session.permissions);
  expect(f.requests.filter(request => request.path === '/api/auth/session')).toHaveLength(2);
  expect(f.requests.filter(request => request.path === '/api/auth/session' && request.fleet)).toHaveLength(1);
  const dock = mobileThreadLocalDock({ feedback: [], usage: { createdAt: '2026-10-11T00:00:00Z', notices: [],
    accounts: [{ id: 'account', driver: 'codex', label: 'Account', instanceId: 'codex',
      limits: { windows: [], resetCredits: { availableCount: 1 } } }] } },
    { usageKey: 'one:thread', viewportHeight: 844, dark: false, canManageProviders: f.client.scopes.includes('providers:manage') });
  expect(dock.accounts[0]).toMatchObject({ resetVisible: true, resetDisabled: false, permissionMessage: '' });
  expect(JSON.stringify(session)).toBe(before);
  expect(f.requests.some(request => request.method === 'provider.consumeResetCredit')).toBe(false);
});

test('explicit narrow, empty and malformed grants never inherit legacy authority in either bootstrap', async () => {
  for (const permissions of [[], null, 'providers:manage', ['filesystem:read'], ['providers:manage', false, 3]]) {
    const session = { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate', 'providers:manage'], permissions };
    const expected = Array.isArray(permissions) ? permissions.filter(value => typeof value === 'string') : [];
    const f = sessionFixture(session);
    await f.client.synchronize(f.native); await f.syncFleet();
    expect(f.client.scopes).toEqual(expected);
    expect(f.client.writable).toBe(false);
    expect([...f.background.entries.values()][0]?.scopes).toEqual(expected);
  }
});

test('absent permissions use only the pinned legacy-parent rules and reauthentication retires old grants', async () => {
  const session: Obj = { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate', 'terminal:operate'] };
  const f = sessionFixture(session);
  await f.client.synchronize(f.native); await f.syncFleet();
  for (const scope of [...expected, 'access:write']) {
    expect(f.client.scopes.includes(scope)).toBe(mobileSessionGrants(session, scope));
    expect([...f.background.entries.values()][0]?.scopes.includes(scope)).toBe(mobileSessionGrants(session, scope));
  }
  session.auth = { serverUpdateScope: 'environment:maintain' };
  await f.client.synchronize(f.native);
  expect(f.client.scopes).toEqual(session.scopes);
  session.permissions = [];
  await f.client.synchronize(f.native);
  expect(f.client.scopes).toEqual([]); expect(f.client.writable).toBe(false);
});

test('provider-only grant stays separate from the copied general write policy', async () => {
  const f = sessionFixture({ authenticated: true, scopes: ['orchestration:read'], permissions: ['orchestration:read', 'providers:manage'] });
  await f.client.synchronize(f.native);
  expect(f.client.scopes.includes('providers:manage')).toBe(true); expect(f.client.writable).toBe(false);
  // rpc's write flag marks uncertainty; it does not impose the command gate.
  expect(await f.client.rpc(f.native, 'test.permissionEcho', {}, true)).toEqual({});
  expect(f.requests.at(-1)?.method).toBe('test.permissionEcho');
});

test('the boundary preserves unrelated, failed and unauthenticated replies and all envelope fields', async () => {
  const session = { authenticated: true, scopes: [], permissions: ['providers:manage'] }, error = { kind: 'Denied', message: 'Denied', uncertain: false };
  for (const response of [{ ok: false, generation: 9, value: session, error },
    { ok: true, generation: 9, value: { ...session, authenticated: false } }]) {
    const native = mobileNative({ available: true, watch() {}, async later() { return response; } });
    expect(await native.later({ op: 'http', path: '/api/auth/session' })).toBe(response);
  }
  const response = { ok: true, generation: 9, value: session, error, trace: { id: 'same' } };
  const requests: unknown[] = [], native = mobileNative({ available: true, watch() {}, async later(request) { requests.push(request); return response; } });
  for (const request of [{ op: 'http', path: '/api/auth/session?extra=1' }, { op: 'request', method: '/api/auth/session' },
    { op: 'http', path: '/api/orchestration/shell' }]) expect(await native.later(request)).toBe(response);
  const request = { op: 'http', path: '/api/auth/session', generation: 9, fleet: 'two' }, result = obj(await native.later(request));
  expect(result).toEqual({ ...response, value: { ...session, scopes: ['providers:manage'] } });
  expect(result.error).toBe(error); expect(result.trace).toBe(response.trace); expect(requests.at(-1)).toBe(request);
  expect(session.scopes).toEqual([]);
});

test('transfer-encoded auth decodes once through the existing transfer cleanup and preserves its envelope', async () => {
  const session = { authenticated: true, scopes: ['orchestration:read'], permissions: ['orchestration:read', 'providers:manage'] };
  const text = JSON.stringify(session), split = Math.floor(text.length / 2), chunks = [text.slice(0, split), text.slice(split)];
  const requests: Obj[] = [], trace = {}, response = { ok: true, generation: 7, trace, value: { _nativeTransfer: { id: 'auth', parts: 2 } } };
  const native = mobileNative({ available: true, watch() {}, async later(input) {
    const request = obj(input); requests.push({ ...request });
    if (request.op === 'http') return response;
    if (request.op === 'readChunk') return { ok: true, generation: 7, value: { text: chunks[Number(request.index)] } };
    if (request.op === 'releaseChunk') return { ok: true, generation: 7, value: {} };
    throw new Error('Unexpected request');
  } });
  const request = { op: 'http', path: '/api/auth/session', generation: 7, fleet: 'two' };
  const pending = native.later(request); request.fleet = 'replacement';
  const result = obj(await pending);
  expect(result).toEqual({ ...response, value: { ...session, scopes: session.permissions } }); expect(result.trace).toBe(trace);
  expect(requests.map(request => request.op)).toEqual(['http', 'readChunk', 'readChunk', 'releaseChunk']);
  expect(requests.slice(1).every(request => request.generation === 7)).toBe(true);
  expect(requests.every(request => request.fleet === 'two')).toBe(true);
});

test('wrong and late generations cannot publish normalized grants into focused or fleet owners', async () => {
  const session = { authenticated: true, scopes: [], permissions: ['orchestration:operate', 'providers:manage'] };
  const wrong = sessionFixture(session, request => request.path === '/api/auth/session' ? { ok: true, generation: 2, value: session } : undefined);
  await expect(wrong.client.synchronize(wrong.native)).rejects.toThrow('The connection changed');
  await wrong.syncFleet(); expect(wrong.client.scopes).toEqual([]);
  expect([...wrong.background.entries.values()][0]?.scopes).toEqual([]);
  for (const move of [false, true]) {
    const started = deferred<void>(), reply = deferred<unknown>();
    const f = sessionFixture(session, request => { if (request.path === '/api/auth/session') { started.resolve(); return reply.promise; } });
    const loading = f.client.synchronize(f.native); await started.promise;
    f.client.adoptStatus({ state: 'connected', origin: move ? 'https://new.test' : f.client.origin,
      environmentId: move ? 'new' : f.client.environmentId, message: '' }, 4);
    reply.resolve({ ok: true, generation: 3, value: session });
    await expect(loading).rejects.toHaveProperty('kind', 'stale');
    expect(f.client.scopes).toEqual([]); expect(f.client.ready).toBe(false);
    expect(f.requests.some(request => request.method === 'server.getConfig')).toBe(false);
  }
  const started = deferred<void>(), reply = deferred<unknown>();
  const f = sessionFixture(session, request => { if (request.path === '/api/auth/session') { started.resolve(); return reply.promise; } });
  const loading = f.syncFleet(); await started.promise;
  const entry = [...f.background.entries.values()][0]!; entry.generation = 4;
  reply.resolve({ ok: true, generation: 3, value: session }); await loading;
  expect(entry.scopes).toEqual([]); expect(entry.synchronized).toBe(-1); expect(entry.error).toContain('The connection changed');
});

test('unauthenticated bootstrap and a broken auth transfer never adopt partial authority', async () => {
  const unauthenticated = sessionFixture({ authenticated: false });
  await expect(unauthenticated.client.synchronize(unauthenticated.native)).rejects.toThrow('Pair with the T3 server again.');
  await unauthenticated.syncFleet();
  expect(unauthenticated.client.scopes).toEqual([]);
  expect([...unauthenticated.background.entries.values()][0]?.scopes).toEqual([]);
  const f = sessionFixture({}, request => request.path === '/api/auth/session'
    ? { ok: true, generation: 3, value: { _nativeTransfer: { id: 'broken', parts: 2 } } }
    : request.op === 'readChunk' ? { ok: true, generation: 2, value: { text: '{"authenticated":true}' } } : undefined);
  await expect(f.client.synchronize(f.native)).rejects.toThrow('The connection changed while reading a T3 response.');
  expect(f.client.scopes).toEqual([]);
  expect(f.requests.filter(request => request.op === 'releaseChunk')).toHaveLength(1);
  expect(f.requests.some(request => request.method === 'server.getConfig')).toBe(false);
});


const granted: Obj = { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate'],
  permissions: ['orchestration:read', 'orchestration:operate', 'providers:manage'] };
const narrow: Obj = { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate'], permissions: ['orchestration:read'] };
function response(value: Obj, generation = 3) { return { ok: true, generation, value }; }
function authNative(client: T3Client, read: () => unknown | Promise<unknown>, background = new EnvironmentFleet()): Native {
  return mobileNative({ available: true, watch() {}, async later() { return read(); } }, client, background);
}
async function authorized() {
  const client = new MobileDraftClient(), f = sessionFixture(granted, undefined, client);
  await client.synchronize(f.native); return { client, f };
}

test('direct auth success replaces old grants and failure recovery publishes only the new grant', async () => {
  for (const failFirst of [false, true]) {
    const { client } = await authorized();
    if (failFirst) await expect(client.http(authNative(client, () => ({ ok: false, generation: 3, error: { kind: 'auth', message: 'Expired' } })), '/api/auth/session')).rejects.toThrow('Expired');
    const value = await client.http(authNative(client, () => response(narrow)), '/api/auth/session');
    expect(value.permissions).toEqual(['orchestration:read']);
    expect(client.scopes).toEqual(['orchestration:read']);
    expect(client.sessionHasScope('providers:manage')).toBe(false);
    client.scopes = ['providers:manage'];
    expect(client.scopes).toEqual(['orchestration:read']);
    expect(() => client.scopes.push('providers:manage')).toThrow();
  }
});

test('already-returned broad synchronize cannot overwrite newer success or failure', async () => {
  for (const failure of [false, true]) {
    const { client, f } = await authorized(), http = client.http.bind(client);
    let first = true;
    client.http = async (...args) => {
      const value = await http(...args);
      if (first) {
        first = false;
        const native = authNative(client, () => failure ? { ok: false, generation: 3, error: { kind: 'auth', message: 'Expired' } } : response(narrow));
        if (failure) await expect(http(native, '/api/auth/session')).rejects.toThrow('Expired');
        else await http(native, '/api/auth/session');
      }
      return value;
    };
    await client.synchronize(f.native);
    expect(client.scopes).toEqual(failure ? [] : ['orchestration:read']);
    expect(client.sessionHasScope('providers:manage')).toBe(false);
  }
});

test('pending reads retain the accepted grant, superseded reads cannot replace it, and cancellation does not revoke', async () => {
  const { client } = await authorized(), started = deferred<void>(), pending = deferred<unknown>();
  const read = client.http(authNative(client, () => { started.resolve(); return pending.promise; }), '/api/auth/session');
  await started.promise; expect(client.sessionHasScope('providers:manage')).toBe(true);
  await client.http(authNative(client, () => response(narrow)), '/api/auth/session');
  pending.resolve(response(granted)); await expect(read).rejects.toHaveProperty('kind', 'superseded');
  expect(client.scopes).toEqual(['orchestration:read']);
  for (const error of [{ name: 'FetchError', kind: 'Aborted' }, new ClientError('stale', 'stale')]) {
    await expect(client.http(authNative(client, () => { throw error; }), '/api/auth/session')).rejects.toHaveProperty('kind', error.kind === 'Aborted' ? 'superseded' : 'stale');
    expect(client.scopes).toEqual(['orchestration:read']);
  }
});

test('wrong-generation failures preserve current grants and unrelated config failure does not revoke auth success', async () => {
  const { client } = await authorized();
  for (const generation of [2, 4]) {
    await expect(client.http(authNative(client, () => ({ ok: false, generation, error: { kind: 'auth', message: 'Expired' } })), '/api/auth/session')).rejects.toHaveProperty('kind', 'stale');
    expect(client.sessionHasScope('providers:manage')).toBe(true);
  }
  const f = sessionFixture(granted, request => request.method === 'server.getConfig'
    ? { ok: false, generation: 3, error: { kind: 'transport', message: 'Config failed' } } : undefined, client);
  await expect(client.synchronize(f.native)).rejects.toThrow('Config failed');
  expect(client.sessionHasScope('providers:manage')).toBe(true);
});

test('focused status and local replacements retire old grants without accepting stale completion', async () => {
  for (const change of ['generation', 'origin', 'environment', 'disconnect', 'local']) {
    const { client } = await authorized(), entered = deferred<void>(), pending = deferred<unknown>();
    const read = client.http(authNative(client, () => { entered.resolve(); return pending.promise; }), '/api/auth/session');
    await entered.promise;
    if (change === 'local') client.local = { ...client.local };
    else client.adoptStatus({ state: change === 'disconnect' ? 'disconnected' : 'connected',
      origin: change === 'origin' ? 'https://other.test' : client.origin,
      environmentId: change === 'environment' ? 'other' : client.environmentId, message: '' }, change === 'generation' ? 4 : 3);
    expect(client.sessionHasScope('providers:manage')).toBe(false);
    pending.resolve(response(granted)); await expect(read).rejects.toHaveProperty('kind', 'stale');
    expect(client.scopes).toEqual([]);
  }
});

test('nested adapters decode and publish once, including reused request objects and failure replies', async () => {
  const { client } = await authorized(); let value: unknown = response(narrow), calls = 0;
  const inner = authNative(client, () => { calls++; return value; });
  const outer = mobileNative(inner, client), request = { op: 'http', path: '/api/auth/session', generation: 3 };
  const before = client.revision;
  await outer.later(request); expect(calls).toBe(1); expect(client.revision - before).toBe(1);
  expect(client.scopes).toEqual(['orchestration:read']);
  value = { ok: false, generation: 3, error: { kind: 'auth', message: 'Expired' } };
  await outer.later(request); expect(calls).toBe(2); expect(client.scopes).toEqual([]);
  value = response(granted); await outer.later(request); expect(client.sessionHasScope('providers:manage')).toBe(true);
  const text = JSON.stringify(narrow), operations: string[] = [];
  const chunks = mobileNative({ available: true, watch() {}, async later(input) {
    const r = obj(input); operations.push(String(r.op));
    return response(r.op === 'http' ? { _nativeTransfer: { id: 'nested', parts: 1 } } : r.op === 'readChunk' ? { text } : {});
  } }, client);
  await mobileNative(chunks, client).later(request);
  expect(operations).toEqual(['http', 'readChunk', 'releaseChunk']); expect(client.scopes).toEqual(['orchestration:read']);
});

test('fleet direct reads narrow their own grant and phase, removal, replacement and disable end authority', async () => {
  for (const change of ['phase', 'remove', 'replace', 'disable', 'focus']) {
    const client = new MobileDraftClient(), f = sessionFixture(granted, undefined, client);
    await client.synchronize(f.native); await f.syncFleet();
    const entry = [...f.background.entries.values()][0]!;
    const native = authNative(client, () => response(narrow), f.background);
    await native.later({ op: 'http', path: '/api/auth/session', fleet: entry.key, generation: 3 });
    expect(entry.scopes).toEqual(['orchestration:read']); expect(client.sessionHasScope('providers:manage')).toBe(true);
    const grantNative = authNative(client, () => response(granted), f.background);
    await grantNative.later({ op: 'http', path: '/api/auth/session', fleet: entry.key, generation: 3 });
    const entered = deferred<void>(), pending = deferred<unknown>();
    const old = authNative(client, () => { entered.resolve(); return pending.promise; }, f.background).later({ op: 'http', path: '/api/auth/session', fleet: entry.key, generation: 3 });
    await entered.promise;
    if (change === 'phase') { entry.phase = 'reconnecting'; entry.phase = 'connected'; }
    else if (change === 'remove') f.background.entries.delete(entry.key);
    else if (change === 'replace') f.background.entries.set(entry.key, { ...entry, scopes: [] });
    else if (change === 'disable') f.background.saved[0]!.enabled = false;
    else client.environmentId = entry.environmentId;
    expect(entry.scopes).toEqual([]);
    pending.resolve(response(granted)); await expect(old).rejects.toHaveProperty('kind', 'stale');
    expect(entry.scopes).toEqual([]);
  }
});

test('two fleet entries and focus keep independent ordered grants', async () => {
  const client = new MobileDraftClient(), f = sessionFixture(granted, undefined, client);
  await client.synchronize(f.native); await f.syncFleet();
  const first = [...f.background.entries.values()][0]!;
  const second = { ...first, key: 'https://three.test\nthree', origin: 'https://three.test', environmentId: 'three', scopes: [] };
  f.background.saved.push({ origin: second.origin, environmentId: second.environmentId });
  f.background.entries.set(second.key, second);
  const native = authNative(client, () => response(granted), f.background);
  await native.later({ op: 'http', path: '/api/auth/session', fleet: second.key, generation: 3 });
  await authNative(client, () => response(narrow), f.background).later({ op: 'http', path: '/api/auth/session', fleet: first.key, generation: 3 });
  expect(first.scopes).toEqual(['orchestration:read']);
  expect(second.scopes).toEqual(granted.permissions); expect(client.scopes).toEqual(granted.permissions);
});

test('reused failed envelopes and thrown errors revoke the current grant independently for each client', async () => {
  for (const thrown of [false, true]) {
    const { client: a } = await authorized(), { client: b } = await authorized();
    const failure = new ClientError('Expired', 'auth');
    const envelope = { ok: false, generation: 3, error: { kind: 'auth', message: 'Expired' } };
    const fail = () => { if (thrown) throw failure; return envelope; };
    for (let count = 0; count < 4; count++) {
      await expect(a.http(authNative(a, fail), '/api/auth/session')).rejects.toHaveProperty('kind', 'auth');
    }
    expect(b.sessionHasScope('providers:manage')).toBe(true);
    await expect(b.http(authNative(b, fail), '/api/auth/session')).rejects.toHaveProperty('kind', 'auth');
    expect(b.scopes).toEqual([]); expect(b.sessionHasScope('providers:manage')).toBe(false);
  }
});

test('a receipt from an independent newer read cannot impersonate nested work for the same client', async () => {
  for (const thrown of [false, true]) {
    const { client } = await authorized(), entered = deferred<void>(), pending = deferred<void>();
    const failure = new ClientError('Expired', 'auth');
    const envelope = { ok: false, generation: 3, error: { kind: 'auth', message: 'Expired' } };
    const fail = () => { if (thrown) throw failure; return envelope; };
    const old = client.http(authNative(client, async () => { entered.resolve(); await pending.promise; return fail(); }), '/api/auth/session');
    await entered.promise;
    await expect(client.http(authNative(client, fail), '/api/auth/session')).rejects.toHaveProperty('kind', 'auth');
    const revision = client.revision;
    pending.resolve(); await expect(old).rejects.toHaveProperty('kind', 'superseded');
    expect(client.scopes).toEqual([]); expect(client.revision).toBe(revision);
  }
});

test('nested thrown failures publish once without reusing receipts across subsequent invocations', async () => {
  const { client } = await authorized(), failure = new ClientError('Expired', 'auth');
  let fail = true, calls = 0;
  const inner = authNative(client, () => { calls++; if (fail) throw failure; return response(granted); });
  const native = mobileNative(mobileNative(inner, client), client);
  for (let round = 0; round < 2; round++) {
    const revision = client.revision;
    await expect(client.http(native, '/api/auth/session')).rejects.toHaveProperty('kind', 'auth');
    expect(client.revision - revision).toBe(1); expect(client.scopes).toEqual([]);
    fail = false; await client.http(native, '/api/auth/session');
    expect(client.sessionHasScope('providers:manage')).toBe(true); fail = true;
  }
  expect(calls).toBe(4);
});

test('same-endpoint reconnect retires old reads as stale while a newer grant remains accepted', async () => {
  const { client } = await authorized(), entered = deferred<void>(), pending = deferred<unknown>();
  const old = client.http(authNative(client, () => { entered.resolve(); return pending.promise; }), '/api/auth/session');
  await entered.promise;
  const status = { origin: client.origin, environmentId: client.environmentId, message: '' };
  client.adoptStatus({ ...status, state: 'disconnected' }, 3);
  client.adoptStatus({ ...status, state: 'connected' }, 3);
  await client.http(authNative(client, () => response(narrow)), '/api/auth/session');
  const revision = client.revision;
  pending.resolve(response(granted)); await expect(old).rejects.toHaveProperty('kind', 'stale');
  expect(client.scopes).toEqual(['orchestration:read']); expect(client.revision).toBe(revision);
});

async function concurrentNestedFailure(read: () => unknown) {
  const { client: a } = await authorized(), { client: b } = await authorized();
  const revisions = [a.revision, b.revision];
  const nested = (client: MobileDraftClient) => mobileNative(authNative(client, read), client);
  const results = await Promise.all([
    a.http(nested(a), '/api/auth/session').catch(error => error),
    b.http(nested(b), '/api/auth/session').catch(error => error),
  ]);
  expect(results.map(error => error.kind)).toEqual(['auth', 'auth']);
  expect([a.scopes, b.scopes]).toEqual([[], []]);
  expect([a.revision - revisions[0]!, b.revision - revisions[1]!]).toEqual([1, 1]);
}

test('concurrent nested clients preserve their own auth failure for one reused envelope', async () => {
  const envelope = { ok: false, generation: 3, error: { kind: 'auth', message: 'Expired' } };
  await concurrentNestedFailure(() => envelope);
});

test('concurrent nested clients preserve their own auth failure for one reused thrown error', async () => {
  const error = new ClientError('Expired', 'auth');
  await concurrentNestedFailure(() => { throw error; });
});
