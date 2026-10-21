// One connection, driven as app.contract drives it (transport-session.ts, transport.ts): pairing, requests,
// streams, the Ping cadence, the reconnect ladder, probes and the saved-environment ops.
import { describe, test, expect } from 'bun:test';
import { Drive, FakeServer, pairing } from '../support/transport-fixture';

const ORIGIN = 'http://127.0.0.1:4001';
type Obj = Record<string, unknown>;
const value = (reply: Obj) => reply.value as Obj;

async function paired(server = new FakeServer(ORIGIN)) {
  const drive = new Drive(server);
  const reply = await drive.op(pairing());
  expect(reply.ok).toBe(true);
  return { drive, server };
}

describe('pairing (authorization/remote.ts, resolver.ts)', () => {
  test('exchanges the code, checks the session, opens the socket with a ticket and saves the environment', async () => {
    const { drive, server } = await paired();
    const status = value(await drive.op({ op: 'status' }));
    expect([status.state, status.environmentId, status.origin]).toEqual(['connected', 'env-1', ORIGIN]);
    expect(server.requests.map(request => `${request.method} ${request.path}`)).toEqual([
      'GET /.well-known/t3/environment', 'POST /oauth/token', 'GET /api/auth/session', 'POST /api/auth/websocket-ticket']);
    expect(server.requests[1]!.headers['Content-Type']).toBe('application/x-www-form-urlencoded');
    expect(server.requests[2]!.headers.Authorization).toBe('Bearer token-1');
    // The catalog keeps the entry and its token in the one secret.
    const saved = value(await drive.op({ op: 'environments' })).saved as Obj[];
    expect(saved.map(entry => [entry.origin, entry.environmentId, entry.enabled])).toEqual([[ORIGIN, 'env-1', true]]);
    expect(JSON.parse(drive.store.get('t3-catalog')!).tokens).toEqual({ [`${ORIGIN}\nenv-1`]: 'token-1' });
    // The status adds the module's presentation.
    expect(status.presentation).toEqual({ scrollEnds: {} });
  });

  test("a connect nobody awaits is joined by its slot's answer, which settles only once the flow has its URL", async () => {
    const drive = new Drive(new FakeServer(ORIGIN));
    let release: () => void = () => {};
    // The ticket waits until the test lets it go, so the flow is still running when the slot's answer is asked.
    drive.holds['/api/auth/websocket-ticket'] = new Promise<void>(resolve => { release = resolve; });
    void drive.bridge.later({ op: 'connect', origin: ORIGIN, credential: 'code-1' });
    const session = drive.transport.focused;
    expect(session.flowRunning).toBe(session.attempt);
    let joined = false;
    void drive.transport.connect(0, 'focused', session.attempt, 0, drive.module).then(() => { joined = true; });
    for (let round = 0; round < 10; round++) await new Promise(resolve => setTimeout(resolve, 0));
    expect([joined, session.url]).toEqual([false, '']);
    release();
    for (let round = 0; round < 10 && !joined; round++) await new Promise(resolve => setTimeout(resolve, 0));
    expect(joined).toBe(true);
    expect(session.url).toStartWith('ws://127.0.0.1:4001/ws?wsTicket=');
    expect(session.flowRunning).toBe(-1);
  });

  test('a connect whose answer is let go rejects as let go, and the attempt starts again in the slot\'s answer', async () => {
    const server = new FakeServer(ORIGIN), drive = new Drive(server);
    const letGo = Object.assign(new Error('the answer was let go before this reply; the request may already have been sent'), { name: 'FetchError', kind: 'Aborted' });
    let refuse: (error: unknown) => void = () => {};
    drive.holds['/api/auth/session'] = new Promise<void>((_resolve, reject) => { refuse = reject; });
    let outcome: unknown = null;
    void drive.bridge.later({ op: 'connect', origin: ORIGIN, credential: 'code-1' }).then(() => { outcome = 'replied'; }, error => { outcome = error; });
    const session = drive.transport.focused, first = session.attempt;
    for (let round = 0; round < 10; round++) await new Promise(resolve => setTimeout(resolve, 0));
    delete drive.holds['/api/auth/session'];
    refuse(letGo);
    for (let round = 0; round < 10 && outcome === null; round++) await new Promise(resolve => setTimeout(resolve, 0));
    expect(outcome).toBe(letGo);
    expect(session.attempt).toBeGreaterThan(first);
    expect(session.establishing).toBe(true);
    await drive.settle();
    expect(value(await drive.op({ op: 'status' })).state).toBe('connected');
    // The code was exchanged once; the second attempt used the token it saved.
    expect(server.requests.filter(request => request.path === '/oauth/token').length).toBe(1);
  });

  // onboarding.test.ts (0ae1091d01): pairing is an explicit request to connect.
  test('pairing a switched-off environment again turns it back on; a failed pairing leaves it off', async () => {
    const { drive, server } = await paired();
    const enabled = async () => (value(await drive.op({ op: 'environments' })).saved as Obj[]).map(entry => entry.enabled);
    const switchOff = async () => expect((await drive.op({ op: 'setEnvironmentEnabled', origin: ORIGIN, environmentId: 'env-1', enabled: false })).ok).toBe(true);
    await switchOff();
    expect(await enabled()).toEqual([false]);
    server.codes.add('code-2');
    expect((await drive.op({ op: 'pairEnvironment', origin: ORIGIN, credential: 'wrong' })).ok).toBe(false);
    expect(await enabled()).toEqual([false]);
    expect((await drive.op({ op: 'pairEnvironment', origin: ORIGIN, credential: 'code-2' })).ok).toBe(true);
    expect(await enabled()).toEqual([true]);
    // The focused connection's pairing does the same.
    await switchOff();
    server.codes.add('code-3');
    expect((await drive.op(pairing(ORIGIN, 'code-3'))).ok).toBe(true);
    expect(await enabled()).toEqual([true]);
  });

  test('a refused code saves nothing and names the reference error', async () => {
    const drive = new Drive(new FakeServer(ORIGIN));
    const reply = await drive.op(pairing(ORIGIN, 'wrong'));
    expect(reply.ok).toBe(false);
    expect((reply.error as Obj).message).toBe('The environment credential is invalid.');
    const status = value(await drive.op({ op: 'status' }));
    expect(status.state).toBe('error');
    expect(value(await drive.op({ op: 'environments' })).saved).toEqual([]);
  });

  // The welcome wizard's Pair and Settings › Add environment both pair here (connections.ts runConnectionOp).
  test("a pairing target that refuses the connection names the reference's transport error", async () => {
    const { drive } = await paired();
    const reply = await drive.op({ op: 'pairEnvironment', origin: 'http://127.0.0.1:16617', credential: 'abc' });
    expect(reply.ok).toBe(false);
    expect((reply.error as Obj).message).toBe('Failed to fetch remote environment endpoint http://127.0.0.1:16617/.well-known/t3/environment (HttpClientError: Transport error (GET http://127.0.0.1:16617/.well-known/t3/environment)).');
  });

  test('an incompatible server keeps its descriptor and stops', async () => {
    const server = new FakeServer(ORIGIN, 'env-1', { orchestrationProtocolVersion: 1, capabilities: { serverSelfUpdate: 'respawn' } });
    const drive = new Drive(server);
    await drive.op(pairing());
    const status = value(await drive.op({ op: 'status' }));
    expect([status.state, status.failureKind]).toEqual(['error', 'Protocol']);
    expect((status.descriptor as Obj).orchestrationProtocolVersion).toBe(1);
  });
});

describe('requests', () => {
  test('the answer that asks sends the request itself, and its reply comes back through the delivering answer with no tick', async () => {
    const { drive, server } = await paired();
    server.methods['orchestration.getSnapshot'] = (_method, payload) => ({ echoed: payload.x });
    const pumps = { ...drive.pumps }, sent = drive.socketAt(0)!.sent.length;
    let reply: Obj | null = null;
    // The client's own bridge: no tick runs as the op starts or ends.
    void drive.transport.wrap(drive.module)!.later({ op: 'request', method: 'orchestration.getSnapshot', payload: { x: 7 } }).then(answer => { reply = answer as Obj; });
    expect(drive.socketAt(0)!.sent.slice(sent)).toMatchObject([{ _tag: 'Request', tag: 'orchestration.getSnapshot', payload: { x: 7 } }]);
    // Nothing resolves outside the delivering answer, even once the Exit is in.
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(reply).toBeNull();
    await drive.settle();
    expect(reply!.ok).toBe(true);
    expect(value(reply!)).toEqual({ echoed: 7 });
    expect(server.rpcs.at(-1)).toMatchObject({ method: 'orchestration.getSnapshot', payload: { x: 7 } });
    expect(drive.pumps).toEqual(pumps);
  });

  test('a frame no turn may send waits in the outbox, in order, until the mapper sends it after the next frame', async () => {
    const { drive, server } = await paired();
    server.methods.first = () => ({ n: 1 });
    server.methods.second = () => ({ n: 2 });
    const socket = drive.socketAt(0)!, sent = socket.sent.length;
    socket.refusing = true; // a turn that may not send (a mutation's synchronous continuation, say)
    const first = drive.transport.wrap(drive.module)!.later({ op: 'request', method: 'first', payload: {} });
    socket.refusing = false;
    const second = drive.transport.wrap(drive.module)!.later({ op: 'request', method: 'second', payload: {} });
    // The second goes behind the first, which it flushes from its own turn.
    expect(socket.sent.slice(sent).map(frame => frame.tag)).toEqual(['first', 'second']);
    await drive.settle();
    expect([value(await first as Obj), value(await second as Obj)]).toEqual([{ n: 1 }, { n: 2 }]);
  });

  test('a typed failure keeps its kind, reason and detail', async () => {
    const { drive, server } = await paired();
    server.methods['pullRequests.getDetail'] = (_method, _payload, id, srv) => { srv.exit(id, { _tag: 'Failure', cause: [{ _tag: 'Fail', error: { _tag: 'PullRequestOperationError', operation: 'getDetail', reason: 'not-found', detail: 'gone' } }] }); return undefined; };
    const reply = await drive.op({ op: 'request', method: 'pullRequests.getDetail', payload: {} });
    expect(reply.error).toEqual({ kind: 'PullRequestOperationError', message: 'Pull request operation getDetail failed: gone', uncertain: false, reason: 'not-found', detail: 'gone' });
  });

  test('an unanswered request times out uncertain and is interrupted', async () => {
    const { drive, server } = await paired();
    server.methods.slow = () => undefined;
    let reply: Obj | null = null;
    void drive.bridge.later({ op: 'request', method: 'slow', payload: {}, timeout: 2 }).then(answer => { reply = answer as Obj; });
    await drive.advance(1_000);
    expect(reply).toBeNull();
    await drive.advance(1_500);
    expect(reply!.error).toMatchObject({ kind: 'Timeout', uncertain: true });
    expect(drive.socketAt(0)!.sent.some(frame => frame._tag === 'Interrupt')).toBe(true);
  });

  test('a shared read joins an identical one still pending', async () => {
    const { drive, server } = await paired();
    let count = 0;
    server.methods['vcs.refreshStatus'] = () => ({ n: ++count });
    const first = drive.bridge.later({ op: 'request', method: 'vcs.refreshStatus', payload: { cwd: '/r', a: 1 }, share: true });
    const second = drive.bridge.later({ op: 'request', method: 'vcs.refreshStatus', payload: { a: 1, cwd: '/r' }, share: true });
    await drive.settle();
    expect([value(await first as Obj), value(await second as Obj)]).toEqual([{ n: 1 }, { n: 1 }]);
    expect(server.rpcs.filter(rpc => rpc.method === 'vcs.refreshStatus')).toHaveLength(1);
  });

  test('a delivered request replies at once and joins the inbox under its key', async () => {
    const { drive, server } = await paired();
    server.methods['server.feedback'] = () => ({ uploaded: true });
    const reply = await drive.op({ op: 'request', method: 'server.feedback', payload: {}, deliver: 'composer-reply:1' });
    expect(Object.keys(value(reply))).toEqual(['id']);
    await drive.settle();
    const events = value(await drive.op({ op: 'events', after: 0 })).events as Obj[];
    expect(events.map(event => [event.key, event.value])).toEqual([['composer-reply:1', { _reply: { uploaded: true } }]]);
  });

  test('a request while disconnected is refused, not queued', async () => {
    const drive = new Drive(new FakeServer(ORIGIN));
    const reply = await drive.op({ op: 'request', method: 'x', payload: {} });
    expect(reply.error).toMatchObject({ kind: 'Disconnected' });
  });

  test('a stale generation is refused', async () => {
    const { drive } = await paired();
    const reply = await drive.op({ op: 'events', after: 0, generation: -5 });
    expect(reply.error).toMatchObject({ kind: 'stale' });
  });
});

describe('streams (rpc/client.ts)', () => {
  test('chunks join the inbox in order, each acknowledged as it arrives', async () => {
    const { drive, server } = await paired();
    server.methods['orchestration.subscribeShell'] = () => undefined;
    const subscribed = value(await drive.op({ op: 'subscribe', key: 'shell', method: 'orchestration.subscribeShell', payload: { afterSequence: 0 } }));
    await drive.settle();
    const id = String(subscribed.id);
    server.chunk(id, [{ kind: 'a' }, { kind: 'b' }]);
    server.chunk(id, [{ kind: 'c' }]);
    await drive.settle();
    const batch = value(await drive.op({ op: 'events', after: 0 }));
    expect((batch.events as Obj[]).map(event => (event.value as Obj).kind)).toEqual(['a', 'b', 'c']);
    expect(drive.socketAt(0)!.sent.filter(frame => frame._tag === 'Ack').map(frame => frame.requestId)).toEqual([id, id]);
    const ack = value(await drive.op({ op: 'ack', through: Number(batch.latest) }));
    expect(ack.latest).toBe(batch.latest);
  });

  test('a failed stream reports its error, then resubscribes on this session after 250 ms', async () => {
    const { drive, server } = await paired();
    server.methods['orchestration.subscribeThread'] = () => undefined;
    const id = String(value(await drive.op({ op: 'subscribe', key: 'thread', method: 'orchestration.subscribeThread', payload: {} })).id);
    await drive.settle();
    server.exit(id, { _tag: 'Failure', cause: [{ _tag: 'Fail', error: { _tag: 'ThreadMissing', message: 'gone for now' } }] });
    await drive.settle();
    let events = value(await drive.op({ op: 'events', after: 0 })).events as Obj[];
    expect(events.map(event => event.value)).toEqual([{ _transportError: { kind: 'ThreadMissing', message: 'gone for now', uncertain: false, reason: '', detail: '' }, retryAfterMs: 250 }]);
    await drive.advance(500);
    events = value(await drive.op({ op: 'events', after: 1 })).events as Obj[];
    expect(events.map(event => event.value)).toEqual([{ _retryDue: true }]);
  });

  test('unsubscribe interrupts the stream; at most sixteen are open', async () => {
    const { drive, server } = await paired();
    server.methods.sub = () => undefined;
    for (let index = 0; index < 16; index++) await drive.op({ op: 'subscribe', key: `k${index}`, method: 'sub', payload: {} });
    expect((await drive.op({ op: 'subscribe', key: 'k16', method: 'sub', payload: {} })).error).toMatchObject({ kind: 'Busy' });
    await drive.op({ op: 'unsubscribe', key: 'k0' });
    await drive.settle();
    expect(drive.socketAt(0)!.sent.filter(frame => frame._tag === 'Interrupt')).toHaveLength(1);
    expect((await drive.op({ op: 'subscribe', key: 'k16', method: 'sub', payload: {} })).ok).toBe(true);
  });
});

describe('liveness and the reconnect ladder (supervisor.ts)', () => {
  // makeProtocolSocket (pingInterval 5 s, PING_TIMEOUT 15 s); rpc/session.ts names the silence; supervisor.ts (#18469).
  test('a Ping every five seconds; a host silent for 15 s is reconnected, and only what waited on it hears why', async () => {
    const { drive, server } = await paired();
    // The connected tick runs every second, so the Ping goes at its next tick.
    await drive.advance(6_000);
    expect(drive.socketAt(0)!.sent.filter(frame => frame._tag === 'Ping').length).toBeGreaterThan(0);
    // The host freezes: nothing it is sent is answered.
    server.respond = () => [];
    let failed: Obj | null = null;
    const { generation } = value(await drive.op({ op: 'status' })) as { generation?: number };
    void drive.bridge.later({ op: 'request', method: 'git.status', payload: {}, ...(generation === undefined ? {} : { generation }) }).then(reply => { failed = reply as Obj; });
    await drive.advance(14_000);
    expect(value(await drive.op({ op: 'status' })).state).toBe('connected');
    // The last Pong counts from the tick after it; the Ping due 20 s after it finds the silence (the ladder waits 1 s or more).
    await drive.advance(6_500);
    const status = value(await drive.op({ op: 'status' }));
    expect(status.state).toBe('reconnecting');
    // A session came up, so losing it is not a failed connection: the reconnect names no failure.
    expect(String(status.message)).toMatch(/^Reconnecting in \ds…$/);
    expect(status.failureKind).toBe('');
    expect((failed as Obj | null)?.error).toMatchObject({
      kind: 'Network', uncertain: true, unavailable: true,
      message: 'Server env-1 stopped responding for 15s. It may be overloaded, asleep, or off the network.',
    });
  });

  test('any frame counts as the host answering, and a late tick sends a Ping before it gives up', async () => {
    const { drive, server } = await paired();
    server.methods.sub = () => undefined;
    await drive.op({ op: 'subscribe', key: 'k', method: 'sub', payload: {} });
    const id = String(server.rpcs.at(-1)!.id);
    // Pongs stop, but stream values keep coming.
    const respond = server.respond.bind(server);
    server.respond = socket => respond(socket).filter(frame => frame._tag !== 'Pong');
    for (let second = 0; second < 30; second += 1) { server.chunk(id, [{ second }]); await drive.advance(1_000, 1_000); }
    expect(value(await drive.op({ op: 'status' })).state).toBe('connected');
    // A tick a minute late (the Mac slept): the first thing it does is ask, not give up.
    server.respond = respond;
    const pings = drive.socketAt(0)!.sent.filter(frame => frame._tag === 'Ping').length;
    await drive.advance(60_000, 60_000);
    expect(drive.socketAt(0)!.sent.filter(frame => frame._tag === 'Ping').length).toBe(pings + 1);
    await drive.advance(2_000);
    expect(value(await drive.op({ op: 'status' })).state).toBe('connected');
  });

  test('a frame counts from the tick that sees it, so held ticks do not shorten the 15 s of silence', async () => {
    const { drive, server } = await paired();
    server.methods.sub = () => undefined;
    await drive.op({ op: 'subscribe', key: 'k', method: 'sub', payload: {} });
    const id = String(server.rpcs.at(-1)!.id);
    await drive.advance(2_000);
    // The window's timers are held for 13 s while the socket keeps delivering frames.
    for (let second = 0; second < 13; second += 1) {
      drive.now += 1_000;
      server.chunk(id, [{ second }]);
      await drive.turn();
    }
    // The host goes quiet just as the ticks resume: 7 s of silence is not 15.
    server.respond = () => [];
    await drive.advance(7_000, 1_000);
    expect(value(await drive.op({ op: 'status' })).state).toBe('connected');
  });

  test('a socket that never opens still reports its failure on the ladder', async () => {
    const server = new FakeServer(ORIGIN);
    const drive = new Drive(server);
    expect((await drive.op(pairing())).ok).toBe(true);
    // The host answers HTTP but refuses every new socket, then drops the live one.
    server.accept = () => null;
    server.restart();
    await drive.settle();
    await drive.advance(2_500);
    const status = value(await drive.op({ op: 'status' }));
    expect(status.state).toBe('reconnecting');
    expect(String(status.message)).toMatch(/^Server env-1 could not establish a WebSocket connection\. Reconnecting in \d+s…$/);
  });

  test('a server restart reconnects after the ladder delay and resubscribes nothing by itself', async () => {
    const { drive, server } = await paired();
    server.up = false;
    await drive.settle();
    let status = value(await drive.op({ op: 'status' }));
    expect(status.state).toBe('reconnecting');
    expect(String(status.message)).toContain('Reconnecting in 2s…');
    // The ladder's wait runs the tick at its slower cadence, not every frame.
    expect(drive.transport.focused.establishing).toBe(false);
    server.up = true;
    await drive.advance(2_500);
    status = value(await drive.op({ op: 'status' }));
    expect(status.state).toBe('connected');
    // The saved token, not a new pairing.
    expect(server.requests.filter(request => request.path === '/oauth/token')).toHaveLength(1);
  });

  test('Retry probes a live socket instead of replacing it', async () => {
    const { drive, server } = await paired();
    const reply = await drive.op({ op: 'retry', origin: '', credential: '' });
    expect(value(reply).state).toBe('connected');
    expect(server.rpcs.at(-1)!.method).toBe('server.probe');
    expect(server.sockets).toHaveLength(1);
  });

  test('a probe that goes unanswered reconnects at once, skipping the ladder', async () => {
    const { drive, server } = await paired();
    server.methods['server.probe'] = () => undefined;
    void drive.bridge.later({ op: 'retry', origin: '', credential: '' });
    await drive.advance(3_500);
    expect(value(await drive.op({ op: 'status' })).state).toBe('connected');
    expect(server.sockets.length).toBe(2);
  });

  test('an expired session stops and forgets its token', async () => {
    const { drive, server } = await paired();
    server.tokens.clear();
    server.up = false; await drive.settle(); server.up = true;
    await drive.advance(2_500);
    const status = value(await drive.op({ op: 'status' }));
    expect([status.state, status.failureKind]).toEqual(['error', 'Authentication']);
    expect(JSON.parse(drive.store.get('t3-catalog')!).tokens).toEqual({});
  });
});

describe('disconnect and forget', () => {
  test('Disconnect keeps the token; Forget removes the environment and its token', async () => {
    const { drive } = await paired();
    expect(value(await drive.op({ op: 'disconnect', forget: false })).state).toBe('disconnected');
    expect(JSON.parse(drive.store.get('t3-catalog')!).tokens).not.toEqual({});
    await drive.op({ op: 'connect', origin: ORIGIN, credential: '' });
    expect(value(await drive.op({ op: 'status' })).state).toBe('connected');
    await drive.op({ op: 'disconnect', forget: true });
    expect(JSON.parse(drive.store.get('t3-catalog')!)).toMatchObject({ saved: [], tokens: {}, origin: '' });
  });

  test('a relaunch restores the remembered origin of a saved environment', async () => {
    const { drive } = await paired();
    const catalog = drive.store.get('t3-catalog')!;
    const next = new Drive(new FakeServer(ORIGIN));
    next.store.set('t3-catalog', catalog);
    const status = value(await next.op({ op: 'status' }));
    expect([status.state, status.origin]).toEqual(['disconnected', ORIGIN]);
  });
});

describe('HTTP through fetch', () => {
  test('preview streams mint their own authenticated ticket without widening generic HTTP writes', async () => {
    const { drive, server } = await paired();
    const access = value(await drive.op({ op: 'previewStreamAccess' }));
    expect(access).toMatchObject({ httpBase: `${ORIGIN}/api/preview-stream`, wsBase: 'ws://127.0.0.1:4001/api/preview-stream' });
    expect(typeof access.ticket).toBe('string');
    expect(server.requests.at(-1)).toMatchObject({ method: 'POST', path: '/api/auth/websocket-ticket', headers: { Authorization: 'Bearer token-1' } });
    expect((await drive.op({ op: 'http', path: '/api/auth/websocket-ticket', method: 'POST' })).error).toMatchObject({ kind: 'Arguments' });
    await drive.op({ op: 'disconnect' });
    expect((await drive.op({ op: 'previewStreamAccess' })).error).toMatchObject({ kind: 'Disconnected' });
  });

  test('a read sends the bearer; the pull request diff is a POST of its allowed fields', async () => {
    const { drive, server } = await paired();
    server.routes['/api/orchestration/shell'] = () => ({ status: 200, body: { projects: [] } });
    server.routes['/api/pull-requests/diff'] = request => ({ status: 200, body: { echoed: JSON.parse(request.body) } });
    expect(value(await drive.op({ op: 'http', path: '/api/orchestration/shell' }))).toEqual({ projects: [] });
    expect(server.requests.at(-1)!.headers.Authorization).toBe('Bearer token-1');
    const diff = await drive.op({ op: 'prDiff', payload: { projectId: 'p', repository: 'o/r', number: 3 } });
    expect(value(diff)).toEqual({ echoed: { number: 3, projectId: 'p', repository: 'o/r' } });
    expect((await drive.op({ op: 'http', path: '/x', method: 'POST' })).error).toMatchObject({ kind: 'Arguments' });
  });

  test('an unavailable pull request host reads the reference sentence', async () => {
    const { drive, server } = await paired();
    server.routes['/api/pull-requests/diff'] = () => ({ status: 503, body: { _tag: 'PullRequestUnavailableError', reason: 'cli-unauthenticated', provider: 'github' } });
    const diff = await drive.op({ op: 'prDiff', payload: { projectId: 'p', repository: 'o/r', number: 3 } });
    expect((diff.error as Obj).message).toBe('GitHub CLI is not authenticated. Run `gh auth login` and retry.');
  });

  test('an attachment uploads its bytes under its type, without the bearer', async () => {
    const { drive, server } = await paired();
    server.routes['/api/attachments/upload/a1'] = request => ({ status: 200, body: { stored: request.body, type: request.headers['Content-Type'], auth: request.headers.Authorization ?? '' } });
    const reply = await drive.op({ op: 'uploadAttachment', path: '/api/attachments/upload/a1', base64: btoa('hello'), contentType: 'text/plain' });
    expect(value(reply)).toEqual({ stored: '<5 bytes>', type: 'text/plain', auth: '' });
    expect((await drive.op({ op: 'uploadAttachment', path: '/api/attachments/upload/a1', base64: btoa('not a png') })).error).toMatchObject({ message: 'The captured image is not a PNG.' });
  });
});
