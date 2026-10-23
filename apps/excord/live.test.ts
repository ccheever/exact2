import { afterEach, beforeEach, expect, spyOn, test } from 'bun:test';
import type { Answer, Args, Result, Source, Storage, Store, StreamEvent, StreamSocket } from './app.contract.d.ts';
import * as S from './discord/store.ts';
import { readyPayload, startFixture } from './fixture/server.ts';
import { applyEvent, clock, createLive } from './live.ts';
import { createRenderer, windowOf, type Stored } from './render.ts';

const EPOCH = Date.UTC(2026, 9, 22, 12);
const INTERVAL = 41250;
const ids = (rows: { id: string }[]) => rows.map(row => row.id);
const historyPath = (channel: string) => `/api/v9/channels/${channel}/messages`;
let moduleNumber = 0;

function secretsStore() {
  const values = new Map<string, string>();
  return {
    get: (key: string) => values.get(key) ?? null,
    set: (key: string, value: string) => { values.set(key, value); },
    forget: (key: string) => { values.delete(key); },
  };
}

interface Peer {
  server: any;
  socket: StreamSocket;
  incoming: Promise<unknown>;
  snapshots: Result<'gatewayState'>[];
  sent: any[];
}

function serverForTest() {
  let handle: any, websocket: any;
  const serve = spyOn(Bun, 'serve').mockImplementation(((config: any) => {
    handle = config.fetch; websocket = config.websocket;
    return { port: 4790, stop: async () => {} };
  }) as typeof Bun.serve);
  let fixture: ReturnType<typeof startFixture>;
  try { fixture = startFixture({ world: 'transcript', clock: 'fixed', heartbeatInterval: INTERVAL }); }
  finally { serve.mockRestore(); }

  const peers: Peer[] = [];
  const requests: Request[] = [];
  const counts = new Map<string, number>();
  const pending = new Set<Promise<unknown>>();
  const releases: (() => void)[] = [];
  const holds = new Map<string, { entered: (request: Request) => void; wait: Promise<void> }>();
  const track = (work: Promise<unknown>) => {
    pending.add(work);
    void work.then(() => pending.delete(work), () => pending.delete(work));
  };

  async function drain() {
    // Joining both directions also joins frames queued while a mapper answers HELLO or READY.
    for (;;) {
      const outgoing = peers.map(peer => peer.server.data.queue);
      await Promise.all([...pending, ...outgoing]);
      if (!pending.size && peers.every((peer, i) => peer.server.data.queue === outgoing[i])) return;
    }
  }

  async function route(request: Request): Promise<Response> {
    const url = new URL(request.url);
    if (url.origin !== fixture.origin) throw new Error(`Unexpected fixture URL: ${request.url}`);
    requests.push(request);
    counts.set(url.pathname, (counts.get(url.pathname) ?? 0) + 1);
    const held = holds.get(url.pathname);
    if (held) {
      holds.delete(url.pathname);
      held.entered(request);
      const aborted = Promise.withResolvers<never>();
      const abort = () => aborted.reject(request.signal.reason);
      request.signal.addEventListener('abort', abort, { once: true });
      try {
        request.signal.throwIfAborted();
        await Promise.race([held.wait, aborted.promise]);
        request.signal.throwIfAborted();
      } finally { request.signal.removeEventListener('abort', abort); }
    }
    return handle(request, {});
  }

  function connect(url: URL, mapper: NonNullable<RequestInit['exactStream']>) {
    if (url.origin !== fixture.gatewayUrl.slice(0, -1)) throw new Error(`Unexpected gateway URL: ${url}`);
    const peer: Peer = { server: null, socket: null!, incoming: Promise.resolve(), snapshots: [], sent: [] };
    const deliver = (event: Partial<StreamEvent>) => {
      peer.snapshots.push(mapper({ data: '', lastEventId: '', coalesced: 0, ...event } as StreamEvent,
        peer.socket) as Result<'gatewayState'>);
    };
    let readyState = 1;
    const close = (code = 1000, reason = '') => {
      if (readyState !== 1) return;
      readyState = 3;
      websocket.close(peer.server, code);
      track(Promise.resolve().then(() => deliver({ type: 'close', code, reason, wasClean: true })));
    };
    peer.socket = {
      get readyState() { return readyState; }, bufferedAmount: 0,
      send(data) {
        if (readyState !== 1) throw new Error('Send on closed fixture socket');
        peer.sent.push(JSON.parse(String(data)));
        peer.incoming = peer.incoming.then(() => websocket.message(peer.server, data));
        track(peer.incoming);
      },
      close,
    };
    const upgradeUrl = new URL(url); upgradeUrl.protocol = 'http:';
    track(handle(new Request(upgradeUrl, { headers: { Upgrade: 'websocket' } }), {
      upgrade(_request: Request, { data }: any) {
        peer.server = { data, close,
          send(bytes: string | Uint8Array) {
            const buffer = typeof bytes === 'string' ? new TextEncoder().encode(bytes) : bytes;
            deliver({ type: 'message', data: Uint8Array.from(buffer).buffer });
            return buffer.byteLength;
          },
        };
        peers.push(peer);
        deliver({ type: 'open' });
        websocket.open(peer.server);
        return true;
      },
    }));
    return new Promise<never>(() => {});
  }

  const fetchMock = spyOn(globalThis, 'fetch').mockImplementation(((input: string | URL | Request, init?: RequestInit) => {
    const url = new URL(input instanceof Request ? input.url : String(input));
    if (init?.exactStream) return connect(url, init.exactStream);
    return route(new Request(input, init));
  }) as typeof fetch);

  const request = (path: string, body?: object) => route(new Request(fixture.origin + path, {
    method: body ? 'POST' : 'GET', headers: { Authorization: fixture.world.token, 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  }));
  const state = async () => { await drain(); return (await request('/control/state')).json(); };
  const control = async (name: string, body: object) => {
    const response = await request(`/control/${name}`, body);
    expect(response.status).toBe(200);
    await drain();
    return response.json();
  };
  const app = async (secrets = secretsStore()) => {
    const { answer }: { answer: Answer } = await import('./app.ts?case=' + ++moduleNumber);
    const ask = <T extends Source>(source: T, args: Args<T>) =>
      answer(source, args, secrets as Store, null as unknown as Storage, null);
    expect(await ask('loadGuilds', [0])).toEqual([]);
    return { ask, secrets,
      async dial(epoch = EPOCH) {
        const count = peers.length;
        await ask('loadSession', []);
        void ask('gatewayState', [true, epoch]);
        await drain();
        expect(peers).toHaveLength(count + 1);
        const peer = peers.at(-1)!;
        expect(peer.snapshots.at(-1)).toMatchObject({ state: 'ready', me: fixture.world.me.id, fixture: true });
        return peer;
      },
      window: (channel: string, anchor = '', since = '') => ask('loadWindow', [channel, anchor, 0, since]),
    };
  };
  return { fixture, peers, requests, counts, request, state, control, drain, app,
    hold(path: string) {
      const entered = Promise.withResolvers<Request>(), released = Promise.withResolvers<void>();
      holds.set(path, { entered: entered.resolve, wait: released.promise });
      releases.push(released.resolve);
      return { entered: entered.promise, release: released.resolve };
    },
    async close() {
      try {
        for (const release of releases) release();
        await drain();
        await fixture.close();
        await drain();
      } finally { fetchMock.mockRestore(); }
    },
  };
}

let rig: ReturnType<typeof serverForTest>;
let restoreRandom: () => void;
let previousClock: number;
beforeEach(() => {
  // Query imports isolate app.ts, but its dependency live.ts shares this monotonic clock.
  previousClock = clock.now; clock.now = 0;
  const random = spyOn(crypto, 'getRandomValues').mockImplementation(((values: Uint32Array) =>
    values.fill(0x80000000)) as typeof crypto.getRandomValues);
  restoreRandom = () => random.mockRestore();
  rig = serverForTest();
});
afterEach(async () => {
  try { await rig.close(); }
  finally { restoreRandom(); clock.now = previousClock; }
});

test('1. a cold channel asks for one latest page, then pages only at its edge', async () => {
  const app = await rig.app(); await app.dial();
  const channel = rig.fixture.world.generalChannelId, history = rig.fixture.world.messages[channel];
  const path = historyPath(channel);
  const cold = await app.window(channel);
  expect({ need: cold.need, cursor: cold.cursor, rows: cold.rows.length, requests: rig.counts.get(path) ?? 0 })
    .toEqual({ need: 'latest', cursor: '', rows: 0, requests: 0 });
  expect(await app.ask('loadPage', [channel, 'latest', ''])).toEqual({ ok: true, error: '' });
  expect(rig.requests.filter(request => new URL(request.url).pathname === path).map(request =>
    [request.method, new URL(request.url).search])).toEqual([['GET', '?limit=50']]);
  const tail = await app.window(channel);
  expect(tail).toMatchObject({ need: '', hasBefore: true, hasAfter: false });
  // The first of the 50 cached messages supplies the next row's grouping until its predecessor arrives.
  expect(ids(tail.rows)).toEqual(ids(history.slice(-49)));
  const first = history.at(-50)!.id;
  const edge = await app.window(channel, first);
  expect(edge).toMatchObject({ need: 'before', cursor: first });
  expect(rig.counts.get(path)).toBe(1);
  expect(await app.ask('loadPage', [channel, edge.need, edge.cursor])).toEqual({ ok: true, error: '' });
  const grown = await app.window(channel);
  expect(ids(grown.rows)).toEqual(ids(history.slice(-99)));
  expect(grown.need).toBe('');
  expect(rig.counts.get(path)).toBe(2);
  expect(new URL(rig.requests.at(-1)!.url).searchParams.get('before')).toBe(first);
});

test('2. the first gateway turn uses the launch epoch and delays its first heartbeat', async () => {
  const app = await rig.app(), peer = await app.dial();
  expect(peer.sent.filter(frame => frame.op === 2)).toHaveLength(1);
  const acks = async () => (await rig.state()).frames.filter((frame: any) => frame.op === 11).length;
  expect(await acks()).toBe(0);
  await app.ask('heartbeat', [EPOCH + 500]); await rig.drain();
  expect(await acks()).toBe(0);
  await app.ask('heartbeat', [EPOCH + INTERVAL / 2 - 1]); await rig.drain();
  expect(await acks()).toBe(0);
  await app.ask('heartbeat', [EPOCH + INTERVAL / 2]); await rig.drain();
  expect(await acks()).toBe(1);
  expect(peer.sent.filter(frame => frame.op === 1)).toHaveLength(1);
  expect(JSON.parse(app.secrets.get('fixture-identifies') ?? '[]')).toEqual([EPOCH]);
});

test('3. identify timestamps survive a fresh app module with the same secrets', async () => {
  const secrets = secretsStore(), first = await rig.app(secrets), peer = await first.dial();
  const keptAfterFirst = JSON.parse(secrets.get('fixture-identifies') ?? '[]');
  peer.socket.close(1001, 'Relaunch'); await rig.drain();
  const second = await rig.app(secrets); await second.dial(EPOCH + 1000);
  expect((await rig.state()).identify_count).toBe(2);
  expect([keptAfterFirst, JSON.parse(secrets.get('fixture-identifies') ?? '[]')])
    .toEqual([[EPOCH], [EPOCH, EPOCH + 1000]]);
});

test('4. the viewed channel subscribes its guild again after a non-resumable reconnect', async () => {
  const app = await rig.app(), peer = await app.dial();
  const channel = rig.fixture.world.generalChannelId, guild = rig.fixture.world.guilds[0].id;
  expect((await rig.state()).session_details[0].subscriptions).toEqual([]);
  expect(await app.ask('viewChannel', [channel])).toEqual({ ok: true, error: '' });
  const before = await rig.state();
  expect(before.session_details[0].subscriptions).toEqual([{ op: 37, guild_ids: [guild] }]);
  expect(peer.sent.find(frame => frame.op === 37).d.subscriptions[guild].channels).toEqual({ [channel]: [[0, 99]] });
  await rig.control('invalidate', { resumable: false });
  expect(peer.socket.readyState).toBe(3);
  const fresh = await app.dial(EPOCH + 10000), after = await rig.state();
  expect(after).toMatchObject({ identify_count: 2, resume_count: 0, sessions: 1 });
  expect(after.session_details[0].id).not.toBe(before.session_details[0].id);
  expect(after.session_details[0].subscriptions).toEqual([{ op: 37, guild_ids: [guild] }]);
  expect(fresh.sent.filter(frame => frame.op === 37)).toHaveLength(1);
});

test('5. snapshots have fixed keys and only the viewed channel supplies history versions', async () => {
  const app = await rig.app(), peer = await app.dial();
  const keys = ['state', 'detail', 'me', 'fixture', 'sidebar', 'channel', 'version', 'history', 'retryIn', 'attempt'].sort();
  expect(Object.keys(peer.snapshots.at(-1)!).sort()).toEqual(keys);
  const channel = rig.fixture.world.generalChannelId;
  const other = rig.fixture.world.guilds[0].channels.find(value => value.id !== channel)!.id;
  await app.ask('viewChannel', [channel]); await rig.drain();
  await rig.control('heartbeat-request', {});
  const before = peer.snapshots.at(-1)!;
  expect(before.channel).toBe(channel);
  expect(await app.ask('inject', [other, 'message', '', EPOCH + 1000])).toEqual({ ok: true, error: '' });
  await rig.drain();
  const elsewhere = peer.snapshots.at(-1)!;
  expect(elsewhere.sidebar).toBeGreaterThan(before.sidebar);
  expect(elsewhere.version).toBe(before.version);
  expect(elsewhere.history).toBe(before.history);
  expect(await app.ask('inject', [channel, 'message', '', EPOCH + 2000])).toEqual({ ok: true, error: '' });
  await rig.drain();
  const here = peer.snapshots.at(-1)!;
  expect(here.history).toBeGreaterThan(elsewhere.history);
  expect(here.version).toBeGreaterThan(elsewhere.version);
  for (const snapshot of peer.snapshots) expect(Object.keys(snapshot).sort()).toEqual(keys);
});

test('6. a newer page ask is refused without aborting the page already in flight', async () => {
  const app = await rig.app(); await app.dial();
  const channel = rig.fixture.world.generalChannelId, path = historyPath(channel), held = rig.hold(path);
  const first = app.ask('loadPage', [channel, 'latest', '']);
  const request = await held.entered;
  try {
    const second = await app.ask('loadPage', [channel, 'latest', '']);
    expect(second).toEqual({ ok: false, error: 'A page of this channel is still loading' });
    expect(rig.counts.get(path)).toBe(1);
    expect(request.signal.aborted).toBe(false);
  } finally { held.release(); }
  expect(await first).toEqual({ ok: true, error: '' });
  expect(ids((await app.window(channel)).rows)).toEqual(ids(rig.fixture.world.messages[channel].slice(-49)));
  expect(rig.counts.get(path)).toBe(1);
});

test('7. a fresh READY hides stale rows until each page confirms them', async () => {
  const app = await rig.app(); await app.dial();
  const channel = rig.fixture.world.generalChannelId, history = rig.fixture.world.messages[channel];
  expect(await app.ask('loadPage', [channel, 'latest', ''])).toEqual({ ok: true, error: '' });
  for (let pages = 1; pages < 4; pages++) {
    const edge = await app.window(channel, history.at(-50 * pages)!.id);
    expect(edge.need).toBe('before');
    expect(await app.ask('loadPage', [channel, edge.need, edge.cursor])).toEqual({ ok: true, error: '' });
  }
  const deleted = history.at(-75)!.id;
  expect(ids((await app.window(channel)).rows)).toEqual(ids(history.slice(-199)));
  await rig.control('invalidate', { resumable: false });
  // Delete while disconnected, so only REST reconciliation can discover the missing cached message.
  await rig.control('delete', { channel_id: channel, message_id: deleted });
  await app.dial(EPOCH + 10000);
  const stale = await app.window(channel);
  expect({ need: stale.need, rows: stale.rows.length }).toEqual({ need: 'latest', rows: 0 });
  expect(await app.ask('loadPage', [channel, stale.need, stale.cursor])).toEqual({ ok: true, error: '' });
  const tail = await app.window(channel);
  expect(tail).toMatchObject({ need: '', hasBefore: true });
  expect(ids(tail.rows)).toEqual(ids(history.slice(-50)));
  const edge = await app.window(channel, tail.firstId);
  expect(edge).toMatchObject({ need: 'before', cursor: tail.firstId });
  expect(await app.ask('loadPage', [channel, edge.need, edge.cursor])).toEqual({ ok: true, error: '' });
  const confirmed = await app.window(channel);
  expect(ids(confirmed.rows)).toEqual(ids(rig.fixture.world.messages[channel].slice(-100)));
  expect(ids(confirmed.rows)).not.toContain(deleted);
  expect(confirmed.hasBefore).toBe(true);
});

test('8. a never-read channel pages from zero and marks its first message as new', async () => {
  const channel = rig.fixture.world.generalChannelId, history = rig.fixture.world.messages[channel];
  rig.fixture.world.read_state.entries.find(read => read.id === channel)!.last_message_id = null;
  const app = await rig.app(); await app.dial();
  const record = await app.ask('loadChannel', [channel, 0]);
  expect(record.lastRead).toBe('0');
  expect(record.unread).toBeGreaterThanOrEqual(1);
  const cold = await app.window(channel, '0', '0');
  expect(cold.rows).toEqual([]);
  expect(await app.ask('loadPage', [channel, cold.need, cold.cursor])).toEqual({ ok: true, error: '' });
  const first = await app.window(channel, '0', '0');
  // The window asks for the page around 0, which `loadPage` fetches as the page after it.
  expect({ need: cold.need, cursor: cold.cursor, ids: ids(first.rows), isNew: first.rows[0]?.isNew,
    hasBefore: first.hasBefore }).toEqual({ need: 'around', cursor: '0', ids: ids(history.slice(0, 50)),
    isNew: true, hasBefore: false });
});

test('9. deleting an uncached original advances history and redraws its cached reply', async () => {
  const { fixture } = rig, channel = fixture.world.generalChannelId;
  const live = createLive({ rest: fixture.origin, gateway: fixture.gatewayUrl, cdn: fixture.origin, fixture: true });
  applyEvent(live, 'READY', readyPayload(fixture.world, 'reply-test', fixture.gatewayUrl));
  const response = await rig.request(historyPath(channel) + '?limit=50');
  expect(response.status).toBe(200);
  S.applyHistoryPage(live.store, channel, { limit: 50 }, await response.json());
  const originalIds = new Set(ids(fixture.world.messages[channel]));
  const cached = S.latestWindow(live.store, channel, 200);
  const reply = cached.rows.find(row => row.message_reference && originalIds.has(row.message_reference.message_id)
    && !S.message(live.store, channel, row.message_reference.message_id))!;
  expect(reply).toBeDefined();
  const original = reply.message_reference.message_id;
  expect(S.message(live.store, channel, original)).toBeNull();
  const render = createRenderer({ me: () => live.store.meId!, user: id => S.user(live.store, id),
    channelName: id => S.channel(live.store, id)?.name ?? '', roleName: id => id,
    message: (channel, id) => (S.message(live.store, channel, id) as Stored | null) ?? undefined,
    deleted: (channel, id) => S.deleted(live.store, channel, id), emoji: id => `${fixture.origin}/emojis/${id}.png`,
  });
  const window = () => {
    const slice = S.latestWindow(live.store, channel, 200);
    return windowOf({ rows: slice.rows as Stored[], hasBefore: slice.moreBefore, hasAfter: slice.moreAfter },
      channel, '', '', { count: 0, more: false, at: 0 }, render);
  };
  expect(window().rows.find(row => row.id === reply.id)!.reply[0]).toMatchObject({ id: original, state: 'present' });
  const before = live.history.get(channel) ?? 0;
  await rig.control('delete', { channel_id: channel, message_id: original });
  applyEvent(live, 'MESSAGE_DELETE', { channel_id: channel, id: original });
  expect(live.history.get(channel) ?? 0).toBe(before + 1);
  expect(S.message(live.store, channel, original)).toBeNull();
  expect(window().rows.find(row => row.id === reply.id)!.reply[0]).toMatchObject({ id: original, state: 'deleted' });
});
