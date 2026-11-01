// Background environments, the route walk, the better-route check, outdated hosts and activity reports
// (transport.ts, transport-routes.ts, transport-outdated.ts, transport-activity.ts).
import { describe, test, expect } from 'bun:test';
import { Drive, FakeServer, pairing } from '../support/transport-fixture';

type Obj = Record<string, unknown>;
const value = (reply: Obj) => reply.value as Obj;
// LAN addresses: a token paired over plain HTTP on the LAN may be borrowed by another plain-HTTP route (sendsTokenInClear).
const A = 'http://192.168.1.10:4001', B = 'http://192.168.1.11:4002';

describe('background environments (registry.ts)', () => {
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

  test('the module is told no routes or bearers: it holds no connection of its own', async () => {
    const drive = new Drive(new FakeServer(A, 'env-a'), new FakeServer(B, 'env-b'));
    await drive.op(pairing(A));
    await drive.op({ op: 'status' });
    expect(drive.moduleCalls.map(call => call.op).filter(op => op !== 'status')).toEqual([]);
    expect(JSON.stringify(drive.moduleCalls)).not.toContain('token-1');
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
    const { drive, home, lan } = await twoRoutes();
    await drive.op({ op: 'disconnect', forget: false });
    home.up = false;
    await drive.op({ op: 'connect', origin: A, credential: '' });
    const status = value(await drive.op({ op: 'status' }));
    expect([status.state, status.origin, status.activeRouteId, status.homeOrigin]).toEqual(['connected', B, B, A]);
    // The learned route asks for the descriptor with no credential first, then borrows the paired route's token, and owns none.
    const lanRequests = lan.requests.map(request => `${request.path} ${request.headers.Authorization ?? '-'}`);
    expect(lanRequests[0]).toBe('/.well-known/t3/environment -');
    expect(lanRequests).toContain('/api/auth/websocket-ticket/rpc Bearer token-1');
    expect(Object.keys(JSON.parse(drive.store.get('t3-catalog')!).tokens)).toEqual([`${A}\nenv-1`]);
  });

  test('a learned route above the one in use is checked at once and takes over, not at the 60 s check', async () => {
    const home = new FakeServer(A, 'env-1'), lan = new FakeServer(B, 'env-1');
    lan.tokens = home.tokens; lan.tickets = home.tickets;
    const drive = new Drive(home, lan);
    await drive.op(pairing(A));
    await drive.op({ op: 'setRoutes', environmentId: 'env-1', routes: [
      { id: B, origin: B, kind: '', learned: true, credential: A },
      { id: A, origin: A, kind: '', learned: false, credential: A }] });
    await drive.advance(5_000, 250);
    const status = value(await drive.op({ op: 'status' }));
    expect([status.state, status.origin, status.activeRouteId]).toEqual(['connected', B, B]);
    expect(lan.requests.some(request => request.path === '/api/auth/session' && request.headers.Authorization === 'Bearer token-1')).toBe(true);
  });

  // routes.ts sendsTokenInClear / usableRoutes, driver.ts preflight (c9b8f21bcf).
  test('a token paired over loopback never goes to a learned plain-HTTP address: the route is left out of the walk and the checks', async () => {
    const LOOPBACK = 'http://127.0.0.1:4001';
    const home = new FakeServer(LOOPBACK, 'env-1'), lan = new FakeServer(B, 'env-1');
    lan.tokens = home.tokens; lan.tickets = home.tickets;
    const drive = new Drive(home, lan);
    await drive.op(pairing(LOOPBACK));
    await drive.op({ op: 'setRoutes', environmentId: 'env-1', routes: [
      { id: `learned:env-1:${B}@${LOOPBACK}`, origin: B, kind: '', learned: true, credential: LOOPBACK },
      { id: LOOPBACK, origin: LOOPBACK, kind: '', learned: false, credential: LOOPBACK }] });
    await drive.advance(61_000, 1_000);
    expect(value(await drive.op({ op: 'status' })).activeRouteId).toBe(LOOPBACK);
    await drive.op({ op: 'disconnect', forget: false });
    home.up = false;
    await drive.op({ op: 'connect', origin: LOOPBACK, credential: '' });
    expect(value(await drive.op({ op: 'status' })).state).not.toBe('connected');
    expect(lan.requests.filter(request => request.headers.Authorization)).toEqual([]);
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

  // A session read asked for during the 60 s check moves `check`, which lets the answer running the check go.
  test('a route check let go by a session read runs in the answer that replaces it, started before or after the rejection', async () => {
    const letGo = Object.assign(new Error('the answer was let go before this reply; the request may already have been sent'), { name: 'FetchError', kind: 'Aborted' });
    for (const order of ['rejection first', 'new answer first'] as const) {
      const { drive, home } = await twoRoutes();
      await drive.op({ op: 'disconnect', forget: false });
      home.up = false;
      await drive.op({ op: 'connect', origin: A, credential: '' });
      home.up = true;
      let refuse: (error: unknown) => void = () => {};
      drive.holds['/.well-known/t3/environment'] = new Promise<void>((_resolve, reject) => { refuse = reject; });
      await drive.advance(61_000, 1_000);
      expect(value(await drive.op({ op: 'status' })).activeRouteId).toBe(B);
      await drive.op({ op: 'readSession' });
      const session = drive.transport.focused;
      const replace = () => drive.transport.connect(0, 'focused', session.attempt, session.check, drive.module);
      delete drive.holds['/.well-known/t3/environment'];
      if (order === 'rejection first') { refuse(letGo); await drive.settle(); await replace(); }
      else { const replaced = replace(); refuse(letGo); await replaced; }
      await drive.advance(3_000, 250);
      const status = value(await drive.op({ op: 'status' }));
      expect([order, status.state, status.activeRouteId]).toEqual([order, 'connected', A]);
    }
  });

  test('removing the route in use reconnects; a removed paired route takes its token with it', async () => {
    const { drive } = await twoRoutes();
    const saved = value(await drive.op({ op: 'setRoutes', environmentId: 'env-1', routes: [{ id: B, origin: B, kind: '', learned: false, credential: B }] }));
    expect(((saved.saved as Obj[])[0]!.routes as Obj[]).map(route => route.id)).toEqual([B]);
    expect(JSON.parse(drive.store.get('t3-catalog')!).tokens).toEqual({});
  });
});

describe('outdated hosts (outdatedHostUpdate.ts)', () => {
  // registry.ts setCompatibility (#15052): incompatibility pauses the connection and keeps the switch.
  test('pairs a self-updating older host switched on, pauses it as unsupported, then updates it over a bare socket and reconnects', async () => {
    const old = new FakeServer(A, 'env-old', { orchestrationProtocolVersion: 1, serverVersion: '0.0.40', capabilities: { serverSelfUpdate: 'respawn', serverSelfUpdateProgress: true } });
    const drive = new Drive(old);
    const paired = await drive.op({ op: 'fleetOutdatedPair', fleet: `${A}\n`, credential: 'code-1', scope: 's' });
    expect(value(paired)).toMatchObject({ environmentId: 'env-old', serverUpdateRequired: true });
    expect((value(await drive.op({ op: 'environments' })).saved as Obj[])[0]).toMatchObject({ environmentId: 'env-old', enabled: true });
    // The background session the fleet opens stops as unsupported, and stays stopped.
    await drive.op({ op: 'connect', fleet: `${A}\nenv-old`, origin: A, credential: '' });
    await drive.advance(10_000);
    expect(value(await drive.op({ op: 'status', fleet: `${A}\nenv-old` }))).toMatchObject({ state: 'error', failureKind: 'Protocol' });
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
    expect(socket.sent[0]).toMatchObject({ _tag: 'Request', tag: 'server.updateServerWithProgress', payload: { targetVersion: '0.0.46' }, headers: [] });
    expect(Object.keys(socket.sent[0]!.payload as Obj)).toEqual(['targetVersion']);
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toMatchObject({ [key]: { status: 'running', stage: 'resuming' } });
    old.descriptor.orchestrationProtocolVersion = 2; old.descriptor.serverVersion = '0.0.46';
    await drive.advance(1_500);
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toMatchObject({ [key]: { status: 'done', resultVersion: '0.0.46', fromVersion: '0.0.40' } });
    expect((value(await drive.op({ op: 'environments' })).saved as Obj[])[0]).toMatchObject({ enabled: true });
    // The block is cleared: the stopped session connects again on its own.
    await drive.advance(2_000);
    expect(value(await drive.op({ op: 'status', fleet: key })).state).toBe('connected');
    await drive.op({ op: 'fleetOutdatedAck', fleet: key });
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toEqual({});
  });

  // outdatedHostUpdate.ts (#18563): a host updated by hand needs no update, and a bare socket would be refused.
  test('Update on a host that is already compatible switches it back on without opening a socket', async () => {
    const old = new FakeServer(A, 'env-old', { orchestrationProtocolVersion: 1, serverVersion: '0.0.40', capabilities: { serverSelfUpdate: 'respawn' } });
    const drive = new Drive(old);
    await drive.op({ op: 'fleetOutdatedPair', fleet: `${A}\n`, credential: 'code-1' });
    const key = `${A}\nenv-old`;
    await drive.op({ op: 'connect', fleet: key, origin: A, credential: '' });
    await drive.advance(2_000);
    expect(value(await drive.op({ op: 'status', fleet: key })).failureKind).toBe('Protocol');
    old.descriptor.orchestrationProtocolVersion = 2; old.descriptor.serverVersion = '0.0.47';
    const sockets = old.sockets.length;
    expect(value(await drive.op({ op: 'fleetOutdatedUpdate', fleet: key, targetVersion: '0.0.46' }))).toMatchObject({ started: true });
    await drive.settle();
    expect(value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs).toMatchObject({ [key]: { status: 'done', resultVersion: '0.0.47' } });
    expect(old.rpcs.some(rpc => rpc.method.startsWith('server.updateServer'))).toBe(false);
    await drive.advance(2_000);
    expect(value(await drive.op({ op: 'status', fleet: key })).state).toBe('connected');
    expect(old.sockets.length).toBe(sockets + 1); // the session's own socket, not an update socket
  });

  // transport-outdated.ts: the job's socket ticket is an RPC one (remote.ts issueWebSocketTicketWithFallback).
  test("the update job's ticket mints at /rpc, and at the bare path after a 404 from a server that predates ticket routes", async () => {
    for (const ticketRoutes of [true, false]) {
      const old = new FakeServer(A, 'env-old', { orchestrationProtocolVersion: 1, serverVersion: '0.0.40', capabilities: { serverSelfUpdate: 'respawn' } });
      old.ticketRoutes = ticketRoutes;
      const drive = new Drive(old);
      await drive.op({ op: 'fleetOutdatedPair', fleet: `${A}\n`, credential: 'code-1' });
      old.methods['server.updateServer'] = () => ({ targetVersion: '0.0.46', method: 'respawn' });
      const before = old.requests.length, sockets = old.sockets.length;
      await drive.op({ op: 'fleetOutdatedUpdate', fleet: `${A}\nenv-old`, targetVersion: '0.0.46' });
      await drive.settle();
      expect(old.requests.slice(before).filter(request => request.path.startsWith('/api/auth/websocket-ticket')).map(request => request.path))
        .toEqual(ticketRoutes ? ['/api/auth/websocket-ticket/rpc'] : ['/api/auth/websocket-ticket/rpc', '/api/auth/websocket-ticket']);
      // The server accepts a socket only on a ticket it minted; this one carried the update.
      expect(old.sockets.length).toBe(sockets + 1);
      expect(old.rpcs.map(rpc => rpc.method)).toContain('server.updateServer');
    }
  });

  // outdatedHostUpdate.ts and state/server.ts (#23532): once progress arrived, a socket lost at the handoff is the restart.
  test('a boot-service or respawn update whose socket drops after progress waits for the restart; without progress it fails', async () => {
    for (const [method, progress, expected] of [['boot-service', true, 'resuming'], ['respawn', true, 'resuming'], ['respawn', false, 'failed'], ['desktop-app', true, 'failed']] as const) {
      const old = new FakeServer(A, 'env-old', { orchestrationProtocolVersion: 1, serverVersion: '0.0.40', capabilities: { serverSelfUpdate: method, serverSelfUpdateProgress: true } });
      const drive = new Drive(old);
      await drive.op({ op: 'fleetOutdatedPair', fleet: `${A}\n`, credential: 'code-1' });
      old.methods['server.updateServerWithProgress'] = (_method, _payload, id, server) => {
        if (progress) server.chunk(id, [{ type: 'progress', stage: 'installing' }]);
        return undefined;
      };
      const key = `${A}\nenv-old`;
      await drive.op({ op: 'fleetOutdatedUpdate', fleet: key, targetVersion: '0.0.46' });
      await drive.settle();
      old.up = false;
      await drive.settle();
      const job = (value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs as Obj)[key] as Obj;
      expect([method, progress, job.status === 'running' ? job.stage : job.status]).toEqual([method, progress, expected]);
      if (expected !== 'resuming') { expect(job.message).toBe('The server connection closed (1006).'); continue; }
      old.up = true; old.descriptor.orchestrationProtocolVersion = 2; old.descriptor.serverVersion = '0.0.46';
      await drive.advance(1_500);
      expect((value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs as Obj)[key]).toMatchObject({ status: 'done', resultVersion: '0.0.46' });
    }
  });

  test('a connected server that does not report the target in time: the reference sentence asks to reconnect', async () => {
    const server = new FakeServer(A, 'env-1', { serverVersion: '0.1.5', capabilities: { serverSelfUpdate: 'boot-service', serverSelfUpdateProgress: true } });
    const drive = new Drive(server);
    await drive.op(pairing(A));
    server.methods['server.updateServerWithProgress'] = (_method, _payload, id, fake) => {
      fake.chunk(id, [{ type: 'progress', stage: 'installing' }]);
      return undefined;
    };
    const key = `${A}\nenv-1`;
    await drive.op({ op: 'fleetOutdatedUpdate', fleet: key, targetVersion: '0.1.6', mode: 'connected', capabilities: server.descriptor.capabilities });
    await drive.settle();
    server.up = false;
    await drive.settle();
    server.up = true;
    await drive.advance(32 * 60_000 + 2_000, 60_000);
    expect((value(await drive.op({ op: 'fleetOutdatedJobs', fleet: '\n' })).jobs as Obj)[key])
      .toMatchObject({ status: 'failed', message: 'Could not confirm that the server resumed on t3@0.1.6. Reconnect to check the update outcome.' });
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
  test('recent until 45 s pass without an interaction; the next one reports at once', async () => {
    const server = new FakeServer(A);
    const drive = new Drive(server);
    await drive.op(pairing(A));
    await drive.op({ op: 'activityFacts', visible: true, focused: true });
    await drive.advance(500);
    const reports = () => server.rpcs.filter(rpc => rpc.method === 'server.reportClientActivity').map(rpc => rpc.payload);
    expect(reports().at(-1)!.recentlyInteracted).toBe(true); // the reporter's start counts, as the reference's clock does
    await drive.advance(50_000, 1_000);
    expect(reports().at(-1)!.recentlyInteracted).toBe(false);
    const count = reports().length;
    drive.transport.activity.interaction(drive.now); // app.contract `interacted`, through app.ts
    await drive.advance(1_500, 250);
    expect(reports()).toHaveLength(count + 1);
    expect(reports().at(-1)!.recentlyInteracted).toBe(true);
    // A move within the window changes nothing until the cadence.
    drive.transport.activity.interaction(drive.now);
    await drive.advance(1_500, 250);
    expect(reports()).toHaveLength(count + 1);
  });
});
