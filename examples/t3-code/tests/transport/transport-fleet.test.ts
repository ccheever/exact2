// Background environments, the route walk, the better-route check, outdated hosts and activity reports
// (transport.ts, transport-routes.ts, transport-outdated.ts, transport-activity.ts).
import { describe, test, expect } from 'bun:test';
import { Drive, FakeServer, pairing } from '../support/transport-fixture';

type Obj = Record<string, unknown>;
const value = (reply: Obj) => reply.value as Obj;
const A = 'http://127.0.0.1:4001', B = 'http://127.0.0.1:4002';

describe('background environments (registry.ts, T3Fleet)', () => {
  test('a connect with a fleet key opens a background session in its own slot; fleetStop ends it', async () => {
    const a = new FakeServer(A, 'env-a'), b = new FakeServer(B, 'env-b');
    const drive = new Drive(a, b);
    await drive.op(pairing(A));
    b.codes.add('code-b');
    const key = `${B}\nenv-b`;
    // Unknown keys report disconnected until a connect opens them.
    expect(value(await drive.op({ op: 'status', fleet: key })).state).toBe('disconnected');
    const reply = await drive.op({ op: 'connect', fleet: key, origin: B, credential: 'code-b' });
    expect(reply.ok).toBe(true);
    expect(drive.table.s1.key).toBe(key);
    expect(value(await drive.op({ op: 'status', fleet: key })).state).toBe('connected');
    expect(value(await drive.op({ op: 'status' })).environmentId).toBe('env-a');
    expect((value(await drive.op({ op: 'environments' })).saved as Obj[]).map(entry => entry.environmentId)).toEqual(['env-a', 'env-b']);
    // A background session's request goes to its own server.
    b.methods.who = () => 'b';
    expect(value(await drive.op({ op: 'request', fleet: key, method: 'who', payload: {} }))).toBe('b');
    expect(value(await drive.op({ op: 'fleetStop', fleet: key }))).toEqual({ stopped: true });
    await drive.settle();
    expect(drive.table.s1.key).toBe('');
    expect((await drive.op({ op: 'request', fleet: key, method: 'who', payload: {} })).error).toMatchObject({ kind: 'Disconnected' });
  });

  test('at most eight background environments', async () => {
    const drive = new Drive(new FakeServer(A));
    for (let index = 0; index < 8; index++) void drive.bridge.later({ op: 'connect', fleet: `http://10.0.0.${index}:1\nenv-${index}`, origin: `http://10.0.0.${index}:1`, credential: 'x' });
    const ninth = await drive.op({ op: 'connect', fleet: 'http://10.0.0.9:1\nenv-9', origin: 'http://10.0.0.9:1', credential: 'x' });
    expect(ninth.error).toMatchObject({ kind: 'Limit' });
  });

  test("the module's links follow the connected sessions", async () => {
    const drive = new Drive(new FakeServer(A, 'env-a'));
    await drive.op(pairing(A));
    await drive.op({ op: 'status' });
    const links = drive.moduleCalls.filter(call => call.op === 'links');
    expect(links.at(-1)).toEqual({ op: 'links', focused: 'env-a', routes: [{ fleet: '', environmentId: 'env-a', origin: A, token: 'token-1', primary: false }] });
    await drive.op({ op: 'status' });
    expect(drive.moduleCalls.filter(call => call.op === 'links')).toHaveLength(links.length);
  });
});

describe('the embedded server, the primary', () => {
  test('connects with the bearer the module holds, and is never saved', async () => {
    const server = new FakeServer(A, 'env-local');
    server.tokens.add('local-bearer');
    const drive = new Drive(server);
    drive.primaryBearer = 'local-bearer';
    // The connect replies once its socket's URL is in; the socket opens after.
    expect(value(await drive.op({ op: 'connect', origin: A, primary: true }))).toMatchObject({ state: 'connecting', environmentId: 'env-local', primary: true });
    expect(value(await drive.op({ op: 'status' }))).toMatchObject({ state: 'connected', primary: true, focus: 'primary' });
    expect(value(await drive.op({ op: 'environments' })).saved).toEqual([]);
    expect(JSON.parse(drive.store.get('t3-catalog')!).focus).toBe('primary');
  });

  test('keeps its ladder while its server starts', async () => {
    const drive = new Drive(new FakeServer(A, 'env-local'));
    await drive.op({ op: 'connect', origin: A, primary: true });
    const status = value(await drive.op({ op: 'status' }));
    expect(status.state).toBe('reconnecting');
    expect(String(status.message)).toContain('The local server is starting.');
  });
});

describe('routes (driver.ts connectOverRoutes, supervisor.ts checkBetterRoutes)', () => {
  async function twoRoutes() {
    const home = new FakeServer(A, 'env-1'), lan = new FakeServer(B, 'env-1');
    lan.tokens = home.tokens; lan.tickets = home.tickets;
    const drive = new Drive(home, lan);
    await drive.op(pairing(A));
    await drive.op({ op: 'setRoutes', environmentId: 'env-1', routes: [
      { id: A, origin: A, kind: '', learned: false, credential: A },
      { id: B, origin: B, kind: '', learned: true, credential: A }] });
    return { drive, home, lan };
  }

  test('a saved environment walks its routes: an unreachable first route falls to the next', async () => {
    const { drive, home } = await twoRoutes();
    await drive.op({ op: 'disconnect', forget: false });
    home.up = false;
    await drive.op({ op: 'connect', origin: A, credential: '' });
    const status = value(await drive.op({ op: 'status' }));
    expect([status.state, status.origin, status.activeRouteId, status.homeOrigin]).toEqual(['connected', B, B, A]);
  });

  test('over a fallback route, the 60 s check moves back to the better route once it answers', async () => {
    const { drive, home } = await twoRoutes();
    await drive.op({ op: 'disconnect', forget: false });
    home.up = false;
    await drive.op({ op: 'connect', origin: A, credential: '' });
    home.up = true;
    await drive.advance(61_000, 1_000);
    const status = value(await drive.op({ op: 'status' }));
    expect([status.state, status.origin, status.activeRouteId]).toEqual(['connected', A, A]);
  });

  test('removing the route in use reconnects; a removed paired route takes its token with it', async () => {
    const { drive } = await twoRoutes();
    const saved = value(await drive.op({ op: 'setRoutes', environmentId: 'env-1', routes: [{ id: B, origin: B, kind: '', learned: false, credential: B }] }));
    expect(((saved.saved as Obj[])[0]!.routes as Obj[]).map(route => route.id)).toEqual([B]);
    expect(JSON.parse(drive.store.get('t3-catalog')!).tokens).toEqual({});
  });
});

describe('outdated hosts (outdatedHostUpdate.ts)', () => {
  test('pairs a self-updating older host switched off, then updates it over a bare socket and switches it back on', async () => {
    const old = new FakeServer(A, 'env-old', { orchestrationProtocolVersion: 1, serverVersion: '0.0.40', capabilities: { serverSelfUpdate: 'respawn', serverSelfUpdateProgress: true } });
    const drive = new Drive(old);
    const paired = await drive.op({ op: 'fleetOutdatedPair', fleet: `${A}\n`, credential: 'code-1', scope: 's' });
    expect(value(paired)).toMatchObject({ environmentId: 'env-old', serverUpdateRequired: true });
    expect((value(await drive.op({ op: 'environments' })).saved as Obj[])[0]).toMatchObject({ environmentId: 'env-old', enabled: false });
    old.methods['server.updateServerWithProgress'] = (_method, _payload, id, server) => {
      server.chunk(id, [{ type: 'progress', stage: 'installing' }, { type: 'complete', result: { targetVersion: '0.0.46', method: 'respawn' } }]);
      server.exit(id, { _tag: 'Success', value: {} });
      return undefined;
    };
    const key = `${A}\nenv-old`;
    expect(value(await drive.op({ op: 'fleetOutdatedUpdate', fleet: key, targetVersion: '0.0.46' }))).toMatchObject({ started: true });
    await drive.settle();
    // The bare socket: no protocol gate, and an unheadered request.
    const socket = old.sockets.at(-1)!;
    expect(new URL(socket.url).searchParams.has('orchestrationProtocol')).toBe(false);
    expect(socket.sent[0]).toMatchObject({ _tag: 'Request', tag: 'server.updateServerWithProgress', headers: [] });
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toMatchObject({ [key]: { status: 'running', stage: 'resuming' } });
    old.descriptor.orchestrationProtocolVersion = 2; old.descriptor.serverVersion = '0.0.46';
    await drive.advance(1_500);
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toMatchObject({ [key]: { status: 'done', resultVersion: '0.0.46', fromVersion: '0.0.40' } });
    expect((value(await drive.op({ op: 'environments' })).saved as Obj[])[0]).toMatchObject({ enabled: true });
    await drive.op({ op: 'fleetOutdatedAck', fleet: key });
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toEqual({});
  });

  test('a host that does not come back in time fails the job', async () => {
    const old = new FakeServer(A, 'env-old', { orchestrationProtocolVersion: 1, capabilities: { serverSelfUpdate: 'respawn' } });
    const drive = new Drive(old);
    await drive.op({ op: 'fleetOutdatedPair', fleet: `${A}\n`, credential: 'code-1' });
    old.methods['server.updateServer'] = () => ({ targetVersion: '0.0.46', method: 'respawn' });
    const key = `${A}\nenv-old`;
    await drive.op({ op: 'fleetOutdatedUpdate', fleet: key, targetVersion: '0.0.46' });
    await drive.advance(241_000, 1_000);
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toMatchObject({ [key]: { status: 'failed', message: 'Server env-old did not come back on a compatible T3 Code version.' } });
  });
});

describe('activity reports (backgroundActivityReporter.ts)', () => {
  test('reports once the page facts are in, after the debounce, then every 25 s, with retained scopes', async () => {
    const server = new FakeServer(A);
    const drive = new Drive(server);
    await drive.op(pairing(A));
    await drive.advance(1_000);
    expect(server.rpcs.filter(rpc => rpc.method === 'server.reportClientActivity')).toHaveLength(0); // no facts yet
    await drive.op({ op: 'activityFacts', visible: true, focused: true });
    await drive.advance(500);
    const reports = () => server.rpcs.filter(rpc => rpc.method === 'server.reportClientActivity').map(rpc => rpc.payload);
    expect(reports()).toHaveLength(1);
    expect(reports()[0]).toMatchObject({ environmentId: 'env-1', clientKind: 'desktop-renderer', visible: true, focused: true, appState: 'active', ttlMs: 45_000, scopes: [{ type: 'provider-status' }] });
    server.methods.subscribeVcsStatus = () => undefined;
    await drive.op({ op: 'subscribe', key: 'vcs', method: 'subscribeVcsStatus', payload: { cwd: '/repo' } });
    await drive.advance(500);
    expect(reports().at(-1)!.scopes).toEqual([{ type: 'provider-status' }, { type: 'vcs-status', cwd: '/repo' }]);
    const count = reports().length;
    await drive.advance(25_500, 500);
    expect(reports().length).toBe(count + 1);
    // The client id is kept in the catalog.
    expect(JSON.parse(drive.store.get('t3-catalog')!).activityClientId).toBe(reports()[0]!.clientId);
  });
});
