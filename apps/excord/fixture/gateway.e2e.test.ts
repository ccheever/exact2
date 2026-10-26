// The gateway session against the fixture over a real socket, zlib-stream on.
import { afterAll, afterEach, beforeAll, expect, test } from 'bun:test';
import { control, drive, fixtureState, untilFixture, type Drive } from './drive.ts';
import { startFixture } from './server.ts';

let fixture: ReturnType<typeof startFixture>;
let current: Drive | null = null;
beforeAll(() => { fixture = startFixture({ port: 0, heartbeatInterval: 150, subscribeAll: true }); });
afterAll(async () => { await fixture.close(); });
afterEach(async () => {
  current?.close();
  current = null;
  await control(fixture.origin, 'reset');
});

const start = (opts: Partial<Parameters<typeof drive>[0]> = {}) =>
  (current = drive({ gatewayUrl: fixture.gatewayUrl, ...opts }));
const has = (t: string, n = 1) => (changes: Drive['changes']) =>
  changes.filter(c => c.kind === 'dispatch' && c.t === t).length >= n;

test('login: HELLO, IDENTIFY, READY, READY_SUPPLEMENTAL, then the lazy guild; heartbeats are acked', async () => {
  const d = start();
  await d.until(has('GUILD_CREATE'), 'the lazy guild');
  expect(d.sent[0].op).toBe(2);
  expect(d.sent[0].d.properties.os).toBe('iOS');
  expect(d.dispatches().map(c => c.t)).toEqual(['READY', 'READY_SUPPLEMENTAL', 'GUILD_CREATE']);
  const ready = d.dispatches('READY')[0].d;
  expect(ready.guilds.some((g: any) => g.unavailable)).toBe(true);
  expect(d.state.phase).toBe('ready');
  expect(d.state.sessionId).toBe(ready.session_id);
  // Heartbeats every 150 ms: wait for three to be sent and the last acked.
  await d.until(() => d.sent.filter(f => f.op === 1).length >= 3 && !d.state.awaitingAck && d.state.lastAckAtMs! >= d.state.lastHeartbeatAtMs!, 'three acked heartbeats');
  expect(d.state.phase).toBe('ready');
  expect((await fixtureState(fixture.origin)).identify_count).toBe(1);
});

test('a burst of dispatches arrives in order with seq tracked; edits, deletes and typing follow', async () => {
  const d = start();
  await d.until(has('GUILD_CREATE'), 'login');
  const channel = fixture.world.generalChannelId;
  const { ids } = await control(fixture.origin, 'burst', { channel_id: channel, count: 200 });
  await d.until(has('MESSAGE_CREATE', 200), 'the burst');
  expect(d.dispatches('MESSAGE_CREATE').map(c => c.d.id)).toEqual(ids);
  const seqs = d.dispatches().map(c => c.s);
  expect(seqs).toEqual(seqs.map((_, i) => i + 1));
  await control(fixture.origin, 'edit', { channel_id: channel, message_id: ids[3], content: 'edited' });
  await control(fixture.origin, 'delete', { channel_id: channel, message_id: ids[4] });
  await control(fixture.origin, 'typing', { channel_id: channel, user_id: fixture.world.users[2].id });
  await d.until(has('TYPING_START'), 'typing');
  expect(d.dispatches('MESSAGE_UPDATE')[0].d).toMatchObject({ id: ids[3], content: 'edited' });
  expect(d.dispatches('MESSAGE_DELETE')[0].d).toMatchObject({ id: ids[4] });
  expect(d.state.seq).toBe(d.dispatches().at(-1)!.s);
});

test('RECONNECT resumes on a new socket and replays what was missed, with no second identify', async () => {
  const d = start();
  await d.until(has('GUILD_CREATE'), 'login');
  await control(fixture.origin, 'reconnect');
  await d.until(c => c.some(x => x.kind === 'status' && x.phase === 'backoff'), 'backoff');
  // Sent while the client is between sockets: RESUME must replay it.
  await control(fixture.origin, 'burst', { channel_id: fixture.world.generalChannelId, count: 3 });
  await d.until(has('RESUMED'), 'RESUMED', 15_000);
  await d.until(has('MESSAGE_CREATE', 3), 'the replayed burst');
  expect(d.sent.filter(f => f.op === 6)).toHaveLength(1);
  expect(d.sent.filter(f => f.op === 2)).toHaveLength(1);
  const state = await fixtureState(fixture.origin);
  expect(state.identify_count).toBe(1);
  expect(state.resume_count).toBe(1);
  const seqs = d.dispatches().map(c => c.s);
  expect(new Set(seqs).size).toBe(seqs.length);
});

test('a zombie connection (no ACKs) is closed and resumed', async () => {
  const d = start();
  await d.until(has('GUILD_CREATE'), 'login');
  await control(fixture.origin, 'stop-acks', { on: true });
  await d.until(c => c.some(x => x.kind === 'status' && x.detail === 'heartbeat ACK missing'), 'zombie detection');
  await control(fixture.origin, 'stop-acks', { on: false });
  await d.until(has('RESUMED'), 'RESUMED', 15_000);
  expect((await fixtureState(fixture.origin)).identify_count).toBe(1);
});

test('INVALID_SESSION false identifies again after its delay', async () => {
  const d = start();
  await d.until(has('GUILD_CREATE'), 'login');
  await control(fixture.origin, 'invalidate', { resumable: false });
  await d.until(has('READY', 2), 'a second READY', 15_000);
  expect(d.sent.filter(f => f.op === 2)).toHaveLength(2);
});

test('a bad token closes 4004 and the session stops for good', async () => {
  const d = start({ token: 'wrong' });
  await d.until(() => d.state.phase === 'stopped', 'stopped');
  expect(d.state.stoppedReason).toContain('4004');
  expect((await fixtureState(fixture.origin)).identify_count).toBe(0);
});

test('the identify budget stops a reconnect loop that cannot resume', async () => {
  const d = start({ identifyBudget: { max: 2, windowMs: 3_600_000 } });
  await d.until(has('GUILD_CREATE'), 'login');
  await control(fixture.origin, 'close', { code: 4009 });
  await d.until(has('READY', 2), 'second identify', 15_000);
  await control(fixture.origin, 'close', { code: 4009 });
  await d.until(() => d.state.phase === 'stopped', 'stopped by the budget', 15_000);
  expect(d.state.stoppedReason).toBe('identify budget exhausted: 2 in the last hour');
  expect((await fixtureState(fixture.origin)).identify_count).toBe(2);
});

test('frames split across WebSocket messages are reassembled', async () => {
  await control(fixture.origin, 'split-frames', { parts: 7 });
  const d = start();
  await d.until(has('GUILD_CREATE'), 'login');
  const frames = (await fixtureState(fixture.origin)).frames;
  expect(frames.some((f: any) => f.parts === 7)).toBe(true);
  expect(d.dispatches('READY')).toHaveLength(1);
});

test('plan §6 sizes: a 3 MiB READY, one wire frame over 1 MiB, and a login burst over 1 MiB', async () => {
  // A fixture of its own at Discord's heartbeat (41.25 s), so no heartbeat falls due while the client inflates
  // megabytes: under load an inflate can hold the loop past the file's 150 ms beat, and the missed ack (a
  // zombie, correctly closed) read as a size failure. A frame refused or a socket closed for its size still
  // leaves the client out of `ready`, and the burst and the oversize frame must still arrive.
  const big = startFixture({ port: 0, subscribeAll: true });
  try {
    await control(big.origin, 'ready-size', { bytes: 3 * 1024 * 1024 });
    await control(big.origin, 'login-burst', { bytes: 1024 * 1024 + 4096 });
    const d = start({ gatewayUrl: big.gatewayUrl });
    let state = await untilFixture(big.origin, s => s.logins_done === 1, 'the login burst sent');
    const login = state.frames.filter((f: any) => f.op === 0);
    const readyFrame = login.find((f: any) => f.t === 'READY');
    expect(readyFrame.inflated_bytes).toBeGreaterThanOrEqual(3 * 1024 * 1024);
    await d.until(() => d.dispatches().length >= login.length, 'the whole login burst', 20_000);
    const loginWire = login.reduce((n: number, f: any) => n + f.wire_bytes, 0);
    console.log(`login burst: ${login.length} frames, ${loginWire} wire bytes; READY ${readyFrame.inflated_bytes} inflated, ${readyFrame.wire_bytes} on the wire`);
    expect(loginWire).toBeGreaterThan(1024 * 1024);
    const before = d.dispatches().length;
    const { wire_bytes } = await control(big.origin, 'oversize-frame', { bytes: 1024 * 1024 + 4096 });
    expect(wire_bytes[0]).toBeGreaterThan(1024 * 1024);
    await d.until(c => c.filter(x => x.kind === 'dispatch').length > before, 'the oversize frame', 20_000);
    expect(d.dispatches().at(-1)!.t).toBe('GUILD_CREATE');
    expect(d.state.phase).toBe('ready');
  } finally {
    current?.close();
    current = null;
    await big.close();
  }
});

test('server heartbeat requests are answered and never mistaken for a dead connection', async () => {
  const d = start();
  await d.until(has('GUILD_CREATE'), 'login');
  for (let i = 0; i < 5; i++) {
    const beats = d.sent.filter(f => f.op === 1).length;
    await control(fixture.origin, 'heartbeat-request');
    await d.until(() => d.sent.filter(f => f.op === 1).length > beats && !d.state.awaitingAck, `request ${i} answered and acked`);
  }
  // Two more periodic intervals with no requests: still the same connection.
  const at = Date.now();
  await d.until(() => Date.now() - at > 320 && !d.state.awaitingAck, 'two quiet intervals');
  expect(d.changes.some(c => c.kind === 'status' && c.detail === 'heartbeat ACK missing')).toBe(false);
  expect(d.dispatches('RESUMED')).toHaveLength(0);
  expect((await fixtureState(fixture.origin)).connections).toBe(1);
});

test('a 1000 close from the server ends the session: the client identifies fresh', async () => {
  const d = start();
  await d.until(has('GUILD_CREATE'), 'login');
  await control(fixture.origin, 'close', { code: 1000 });
  await d.until(has('READY', 2), 'a second READY', 15_000);
  expect(d.sent.filter(f => f.op === 6)).toHaveLength(0);
  expect(d.sent.filter(f => f.op === 2)).toHaveLength(2);
});
