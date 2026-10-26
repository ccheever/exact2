import { afterEach, beforeEach, expect, spyOn, test } from 'bun:test';
import type { Answer, Args, Result, Source, Storage, Store, StreamEvent, StreamSocket } from './app.contract.d.ts';
import * as S from './discord/store.ts';
import { snowflakeFromMs } from './discord/ids.ts';
import { readyPayload, startFixture } from './fixture/server.ts';
import { constants as zlibConstants, createDeflate } from 'node:zlib';
import { applyEvent, call, clock, connect, createLive, digest, loadSession, pastedToken, recordIdentify, signIn, signOut } from './live.ts';
import { unlockSession } from './lock.ts';
import { createRenderer, windowOf, type Stored } from './render.ts';

const EPOCH = Date.UTC(2026, 9, 22, 12);
const INTERVAL = 41250;
const ids = (rows: { id: string }[]) => rows.map(row => row.id);
const historyPath = (channel: string) => `/api/v9/channels/${channel}/messages`;
let moduleNumber = 0;

// The store as data code sees it: a locked name's value is `unlock`'s alone, and `get` throws for it.
function secretsStore() {
  const values = new Map<string, string>(), locked = new Set<string>();
  return {
    get: (key: string) => {
      if (locked.has(key)) throw Object.assign(new Error(`InvalidAccessError: secret ${key} is locked`), { name: 'InvalidAccessError' });
      return values.get(key) ?? null;
    },
    set: (key: string, value: string) => { values.set(key, value); },
    forget: (key: string) => { values.delete(key); locked.delete(key); },
    has: (key: string) => values.has(key),
    locked: (key: string) => locked.has(key),
    unlock: async (key: string): Promise<string | null> => values.get(key) ?? null,
    protect: async (key: string) => { if (values.has(key)) locked.add(key); },
    unprotect: async (key: string) => { locked.delete(key); },
  };
}

// App storage in memory; `refuse` makes every write fail, as a full or unavailable store's does.
function memoryStorage(refuse = false) {
  const files = new Map<string, Uint8Array>();
  const fs = {
    async readFile(path: string) {
      const bytes = files.get(path);
      if (!bytes) throw Object.assign(new Error(`ENOENT: ${path}`), { code: 'ENOENT' });
      return bytes.slice().buffer;
    },
    async atomicWriteFile(path: string, data: ArrayBuffer | Uint8Array) {
      if (refuse) throw new Error('QuotaExceededError: the store is full');
      files.set(path, new Uint8Array(data instanceof Uint8Array ? data : new Uint8Array(data)));
    },
  };
  return { fs, files, record: () => JSON.parse(new TextDecoder().decode(files.get('app:/data/fixture-identifies.json') ?? new TextEncoder().encode('[]'))) };
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
  const holds = new Map<string, { entered: (request: Request) => void; wait: Promise<void>; reply: boolean }>();
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
      // A held reply: the server has acted on the request, and only its answer waits.
      const answered = held.reply ? await handle(request, {}) : undefined;
      held.entered(request);
      const aborted = Promise.withResolvers<never>();
      const abort = () => aborted.reject(request.signal.reason);
      request.signal.addEventListener('abort', abort, { once: true });
      try {
        request.signal.throwIfAborted();
        await Promise.race([held.wait, aborted.promise]);
        request.signal.throwIfAborted();
      } finally { request.signal.removeEventListener('abort', abort); }
      if (answered) return answered;
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
  const app = async (secrets = secretsStore(), storage = memoryStorage()) => {
    const { answer }: { answer: Answer } = await import('./app.ts?case=' + ++moduleNumber);
    const ask = <T extends Source>(source: T, args: Args<T>) =>
      answer(source, args, secrets as Store, storage as unknown as Storage, null);
    expect(await ask('loadGuilds', [0])).toEqual([]);
    return { ask, secrets, storage,
      async dial(epoch = EPOCH, ready = 'ready') {
        const count = peers.length;
        await ask('loadSession', []);
        void ask('gatewayState', [true, epoch]);
        await drain();
        // The view's `record` task: the IDENTIFY the mapper holds goes out once its record is written.
        if (peers.at(-1)?.snapshots.at(-1)?.state === 'recording') {
          await ask('recordIdentify', []);
          await ask('sendIdentify', []);
          await drain();
        }
        // The view's `digest` task: a frame too large for the stream's own turn is applied here (a test that
        // dials expecting another state reads the queue itself).
        for (let turns = 0; ready === 'ready' && (peers.at(-1)?.snapshots.at(-1)?.queued ?? 0) > 0 && turns < 100; turns++) {
          peers.at(-1)!.snapshots.push(await ask('digest', [epoch]));
          await drain();
        }
        expect(peers).toHaveLength(count + 1);
        const peer = peers.at(-1)!;
        expect(peer.snapshots.at(-1)).toMatchObject(ready === 'ready' ? { state: 'ready', me: fixture.world.me.id, fixture: true } : { state: ready });
        return peer;
      },
      window: (channel: string, anchor = '', since = '') => ask('loadWindow', [channel, anchor, 0, since]),
    };
  };
  return { fixture, peers, requests, counts, request, state, control, drain, app,
    hold(path: string, reply = false) {
      const entered = Promise.withResolvers<Request>(), released = Promise.withResolvers<void>();
      holds.set(path, { entered: entered.resolve, wait: released.promise, reply });
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
  expect(app.storage.record()).toEqual([EPOCH]);
});

test('3. identify timestamps survive a fresh app module with the same storage', async () => {
  const secrets = secretsStore(), storage = memoryStorage(), first = await rig.app(secrets, storage), peer = await first.dial();
  const keptAfterFirst = storage.record();
  peer.socket.close(1001, 'Relaunch'); await rig.drain();
  const second = await rig.app(secrets, storage); await second.dial(EPOCH + 1000);
  expect((await rig.state()).identify_count).toBe(2);
  expect([keptAfterFirst, storage.record()]).toEqual([[EPOCH], [EPOCH, EPOCH + 1000]]);
});

test('3b. an identify whose record cannot be kept is not sent, and the gateway stops saying why', async () => {
  const app = await rig.app(secretsStore(), memoryStorage(true)), peer = await app.dial(EPOCH, 'stopped');
  expect(peer.sent.filter(frame => frame.op === 2)).toEqual([]);
  expect(peer.snapshots.at(-1)!.detail).toBe('the identify record could not be kept (QuotaExceededError: the store is full)');
  expect((await rig.state()).identify_count).toBe(0);
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
  const keys = ['state', 'detail', 'me', 'meName', 'fixture', 'queued', 'seq', 'sidebar', 'channel', 'version', 'history', 'members', 'retryIn', 'attempt'].sort();
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

test('6b. a vote whose reply comes after the gateway has moved on leaves the gateway\'s account', async () => {
  const app = await rig.app(); await app.dial();
  const channel = rig.fixture.world.generalChannelId, asked = rig.fixture.world.messages[channel].at(-1)! as { id: string; poll?: unknown };
  asked.poll = { question: { text: 'Lunch?' }, answers: [{ answer_id: 1, poll_media: { text: 'Tacos' } }, { answer_id: 2, poll_media: { text: 'Ramen' } }],
    expiry: null, allow_multiselect: false, results: { is_finalized: false, answer_counts: [] } };
  await app.ask('viewChannel', [channel]); await rig.drain();
  expect(await app.ask('loadPage', [channel, 'latest', ''])).toEqual({ ok: true, error: '' });
  const held = rig.hold(`/api/v9/channels/${channel}/polls/${asked.id}/answers/@me`, true);
  const voted = app.ask('vote', [channel, asked.id, ['2']]);
  await held.entered;
  // This vote's own event arrives, then another device takes it back, before the reply.
  await rig.control('vote', { channel_id: channel, message_id: asked.id, user_id: rig.fixture.world.me.id, answer_id: 2, add: false });
  await rig.drain();
  await app.ask('digest', [EPOCH + 100]);
  held.release();
  expect(await voted).toEqual({ ok: true, error: '' });
  await rig.drain();
  await app.ask('digest', [EPOCH + 200]);
  const row = (await app.window(channel)).rows.find(r => r.id === asked.id)!;
  expect(row.poll[0]).toMatchObject({ voted: false, answers: [{ count: 0, me: false }, { count: 0, me: false }] });
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
    channelName: id => S.channel(live.store, id)?.name ?? '', channelGuild: () => '', canOpen: () => false, roleName: id => id,
    message: (channel, id) => (S.message(live.store, channel, id) as Stored | null) ?? undefined,
    deleted: (channel, id) => S.deleted(live.store, channel, id), emoji: id => `${fixture.origin}/emojis/${id}.png`,
    avatar: () => '', nameColor: () => '', roles: () => undefined,
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

test('9b. a person\'s new name redraws the open channel, and an unchanged member update does not', () => {
  const { fixture } = rig, channel = fixture.world.generalChannelId, guild = fixture.world.guilds[0].id;
  const live = createLive({ rest: fixture.origin, gateway: fixture.gatewayUrl, cdn: fixture.origin, fixture: true });
  applyEvent(live, 'READY', readyPayload(fixture.world, 'member-test', fixture.gatewayUrl));
  live.viewing = { channel, guild };
  const user = fixture.world.users.find(u => u.id !== fixture.world.me.id)!;
  const update = (global_name: string | null) => applyEvent(live, 'GUILD_MEMBER_UPDATE', { guild_id: guild, roles: [], user: { ...user, global_name } });
  update(user.global_name);
  const before = live.history.get(channel) ?? 0;
  update(user.global_name);
  expect(live.history.get(channel) ?? 0).toBe(before);
  update('Someone New');
  expect(live.history.get(channel) ?? 0).toBe(before + 1);
});

test('10. a token Discord refuses (4004) is forgotten and the gateway says signed out', async () => {
  const secrets = secretsStore();
  secrets.set('fixture-token', 'not-the-fixture-token');
  const app = await rig.app(secrets), peer = await app.dial(EPOCH, 'signed-out');
  expect(peer.snapshots.at(-1)!.detail).toContain('4004');
  expect(secrets.get('fixture-token')).toBeNull();
});

test('11. signing out closes the socket and empties the lists; the next session connects again', async () => {
  const app = await rig.app(), peer = await app.dial();
  expect((await app.ask('loadGuilds', [1])).length).toBeGreaterThan(0);
  expect(await app.ask('signOut', [])).toEqual({ ok: true, error: '' });
  await rig.drain();
  expect(peer.socket.readyState).toBe(3);
  expect(app.secrets.get('fixture-token')).toBeNull();
  expect(await app.ask('loadGuilds', [2])).toEqual([]);
  expect(await app.ask('loadDms', [2])).toEqual([]);
  await app.dial(EPOCH + 1000);
  expect((await app.ask('loadGuilds', [3])).length).toBeGreaterThan(0);
});

test('12. a pasted token is taken from its header value or quotes; anything else is refused before it is kept', () => {
  const live = createLive({ rest: 'https://discord.com', gateway: 'wss://gateway.discord.gg', cdn: 'https://cdn.discordapp.com', fixture: false });
  const secrets = secretsStore();
  const token = 'MTIzNDU2Nzg5MDEyMzQ1Njc4.GabcDE.abcdefghijklmnopqrstuvwxyz0123456789_-AB';
  expect(pastedToken(`  "${token}"\n`)).toBe(token);
  expect(pastedToken(`Bearer ${token}`)).toBeNull();
  expect(pastedToken('short')).toBeNull();
  expect(signIn(live, secrets, 'not a token')).toContain("doesn't look like a Discord token");
  expect(secrets.get('discord-token')).toBeNull();
  expect(signIn(live, secrets, `"${token}"`)).toBe('');
  expect(secrets.get('discord-token')).toBe(token);
  expect(live.token).toBe(token);
});

test('12b. an identify record that lands after its connection was retired does not let the next connection identify', async () => {
  const live = createLive({ rest: 'http://127.0.0.1:4790', gateway: 'ws://127.0.0.1:4790', cdn: 'http://127.0.0.1:4790', fixture: true });
  let land!: () => void;
  const storage = { fs: { atomicWriteFile: () => new Promise<void>(resolve => { land = resolve; }) } } as unknown as Storage;
  live.gw = { identifies: [EPOCH] } as any;
  live.held = 'first';
  const writing = recordIdentify(live, storage);
  // The connection is retired and the next one's HELLO holds its own IDENTIFY while the write is pending.
  live.attempt++; live.held = 'second'; live.recorded = '';
  land(); await writing;
  expect(live.recorded).toBe('');
});

test('13. a READY too large for one turn waits for digest, which applies it a part a turn, and what came after, in order', async () => {
  (globalThis as any).scheduler ??= { yield: async () => {} };
  await rig.control('ready-size', { bytes: 16 * 1024 * 1024 });
  const app = await rig.app(), peer = await app.dial(EPOCH, 'identifying');
  const waiting = peer.snapshots.at(-1)!;
  expect(waiting.queued).toBeGreaterThan(0);
  expect(await app.ask('loadGuilds', [1])).toEqual([]);
  const digested = await app.ask('digest', [EPOCH + 100]);
  expect(digested).toMatchObject({ state: 'ready', queued: 0, me: rig.fixture.world.me.id });
  expect(digested.seq).toBeGreaterThan(waiting.seq);
  const guilds = await app.ask('loadGuilds', [digested.sidebar]);
  // Its guilds were applied in parts, each with the account's roles in it.
  expect(guilds.length).toBeGreaterThan(500);
  expect((await app.ask('loadChannels', [guilds[0].id, digested.sidebar])).length).toBeGreaterThan(0);
  // Once the queue is empty the stream's frames are applied as they come again.
  await app.ask('heartbeat', [EPOCH + INTERVAL]); await rig.drain();
  expect(peer.snapshots.at(-1)).toMatchObject({ state: 'ready', queued: 0 });
});

test('14. the rail puts the folder\'s servers under it, in the account\'s order, with what they hold unread', async () => {
  await rig.control('ready-size', { bytes: 300000 });
  const app = await rig.app(), peer = await app.dial();
  const sidebar = peer.snapshots.at(-1)!.sidebar;
  const rail = await app.ask('loadRail', [sidebar]);
  const guilds = await app.ask('loadGuilds', [sidebar]);
  const folder = rail.find((r: any) => r.folder)!;
  expect(folder).toMatchObject({ id: 'folder-4242', name: 'Workshops', color: '#5865f2' });
  expect(folder.guilds.length).toBeGreaterThan(1);
  // Every server shows once: alone, or in its folder.
  expect(rail.flatMap((r: any) => r.guilds.map((g: any) => g.id)).sort()).toEqual(guilds.map((g: any) => g.id).sort());
  expect(folder.mentions).toBe(folder.guilds.reduce((n: number, g: any) => n + g.mentions, 0));
  expect(peer.snapshots.at(-1)).toMatchObject({ state: 'ready' });
});

test('15. a jump to a message no page holds asks once, then shows the channel\'s end', async () => {
  const app = await rig.app(); await app.dial();
  const channel = rig.fixture.world.generalChannelId, path = historyPath(channel);
  // Past the channel's newest message, as a link to one deleted or never sent can be.
  const past = snowflakeFromMs(Date.UTC(2030, 0, 1));
  const asked = await app.window(channel, past);
  expect({ need: asked.need, cursor: asked.cursor }).toEqual({ need: 'around', cursor: past });
  expect(await app.ask('loadPage', [channel, 'around', past])).toEqual({ ok: true, error: '' });
  const settled = await app.window(channel, past);
  expect(settled.need).not.toBe('around');
  expect(settled.anchor).toBe(past);
  const before = rig.counts.get(path) ?? 0;
  expect(before).toBe(1);
  if (settled.need) await app.ask('loadPage', [channel, settled.need, settled.cursor]);
  expect((await app.window(channel, past)).rows.length).toBeGreaterThan(0);
});

// A scheduler whose yields wait until the test lets them go, so a frame can arrive between two turns of
// a digest.
function gatedScheduler() {
  const waiting: (() => void)[] = [];
  const previous = (globalThis as any).scheduler;
  (globalThis as any).scheduler = { yield: () => new Promise<void>(resolve => waiting.push(resolve)) };
  return {
    waiting,
    async releaseAll() { while (waiting.length) { waiting.shift()!(); await Promise.resolve(); await new Promise(r => setTimeout(r, 0)); } },
    restore() { (globalThis as any).scheduler = previous; },
  };
}

// A connection to no server: the test drives its mapper with zlib-stream packets it makes.
async function mapperRig() {
  const live = createLive({ rest: 'http://127.0.0.1:4790', gateway: 'ws://127.0.0.1:4790', cdn: 'http://127.0.0.1:4790', fixture: true });
  live.token = 'fixture-token'; live.ready = true;
  let mapper!: (event: any, socket?: any) => any;
  const fetchSpy = spyOn(globalThis, 'fetch').mockImplementation(((_url: string, init: any) => {
    mapper = init.exactStream; return new Promise(() => {});
  }) as any);
  const sent: string[] = [];
  const socket = { readyState: 1, bufferedAmount: 0, send: (frame: string) => { sent.push(frame); }, close: () => {} };
  void connect(live, secretsStore() as any, null, true, EPOCH);
  const deflate = createDeflate();
  const packet = async (payload: unknown) => {
    const chunks: Buffer[] = [];
    const take = (chunk: Buffer) => chunks.push(chunk);
    deflate.on('data', take);
    await new Promise<void>(resolve => { deflate.write(JSON.stringify(payload)); deflate.flush(zlibConstants.Z_SYNC_FLUSH, () => resolve()); });
    deflate.off('data', take);
    return new Uint8Array(Buffer.concat(chunks)).buffer;
  };
  mapper({ type: 'open' }, socket);
  mapper({ type: 'message', data: await packet({ op: 10, d: { heartbeat_interval: INTERVAL } }) });
  // Random text does not compress, so this READY's packet is many slices.
  let seed = 7;
  const noise = Buffer.from(Uint8Array.from({ length: 600_000 }, () => (seed = (Math.imul(seed, 1103515245) + 12345) >>> 0) >>> 24)).toString('base64');
  const ready = { op: 0, t: 'READY', s: 1, d: { session_id: 's', user: { id: '1', username: 'me' }, guilds: [], noise } };
  return { live, sent, packet, ready, mapper: (event: any) => mapper(event), restore: () => fetchSpy.mockRestore() };
}

test('14. a frame that arrives while digest runs waits behind it', async () => {
  const rig = await mapperRig(), { live, mapper, packet } = rig;
  const gate = gatedScheduler();
  try {
    expect(mapper({ type: 'message', data: await packet(rig.ready) }).queued).toBe(1);
    const digesting = digest(live, secretsStore() as any, EPOCH + 10);
    await Promise.resolve();
    expect(gate.waiting.length).toBe(1);
    // The heartbeat request is a packet of the same zlib stream: inflated now, it would land inside READY's.
    expect(mapper({ type: 'message', data: await packet({ op: 1, d: null }) }).queued).toBe(2);
    await gate.releaseAll();
    expect(await digesting).toMatchObject({ queued: 0 });
    expect(live.gw!.phase).toBe('ready');
    expect(rig.sent.filter(frame => frame.startsWith('{"op":1,'))).toHaveLength(1);
  } finally { gate.restore(); rig.restore(); }
});

test('16. a digest whose continuation never runs leaves its place, and the next digest goes on from it', async () => {
  const rig = await mapperRig(), { live, mapper, packet } = rig;
  const gate = gatedScheduler();
  try {
    mapper({ type: 'message', data: await packet(rig.ready) });
    // The host refuses a turn over budget, and its yield never settles: this one is never let go.
    void digest(live, secretsStore() as any, EPOCH + 10);
    await Promise.resolve();
    expect(gate.waiting.length).toBe(1);
    const stopped = gate.waiting.shift()!;
    gate.restore();
    (globalThis as any).scheduler = { yield: async () => {} };
    expect(await digest(live, secretsStore() as any, EPOCH + 20)).toMatchObject({ queued: 0 });
    expect(live.gw!.phase).toBe('ready');
    // Were it let go after all, it would find its queue taken and stop.
    const before = live.gw!.seq;
    stopped();
    await new Promise(r => setTimeout(r, 0));
    expect(live.gw!.seq).toBe(before);
  } finally { gate.restore(); rig.restore(); }
});

test('17. a 4004 forgets the account and the channel on screen, as signing out does', async () => {
  (globalThis as any).scheduler ??= { yield: async () => {} };
  const rig = await mapperRig(), { live, mapper, packet } = rig;
  try {
    mapper({ type: 'message', data: await packet(rig.ready) });
    await digest(live, secretsStore() as any, EPOCH + 10);
    live.viewing = { channel: '100', guild: '10' };
    const account = live.account;
    const after = mapper({ type: 'close', code: 4004, reason: 'Authentication failed.' });
    expect(after).toMatchObject({ state: 'signed-out', me: '', channel: '' });
    expect(after.detail).toContain('4004');
    expect(live.account).toBe(account + 1);
    expect(live.token).toBe('');
    await expect(call(live, { method: 'GET', url: 'http://127.0.0.1:4790/api/v9/users/@me', headers: {}, route: 'me' } as any))
      .rejects.toThrow('Signed out');
  } finally { rig.restore(); }
});

test('15. signing out forgets the queue, and a REST reply that lands after it is not applied', async () => {
  (globalThis as any).scheduler ??= { yield: async () => {} };
  const app = await rig.app(); await app.dial();
  const channel = rig.fixture.world.generalChannelId, held = rig.hold(historyPath(channel));
  const paging = app.ask('loadPage', [channel, 'latest', '']);
  await held.entered;
  expect(await app.ask('signOut', [])).toEqual({ ok: true, error: '' });
  held.release();
  expect(await paging).toEqual({ ok: false, error: 'Signed out' });
  expect((await app.window(channel)).rows).toEqual([]);
  const after = await app.ask('digest', [EPOCH + 1000]);
  expect(after).toMatchObject({ queued: 0 });
});

test('18. a packet that compresses well inflates about a megabyte a turn, from its first slice', async () => {
  const rig = await mapperRig(), { live, mapper, packet } = rig;
  const inflated = () => (live.gw as any).inflater.out as number;
  const turns: number[] = [];
  let last = 0;
  const previous = (globalThis as any).scheduler;
  (globalThis as any).scheduler = { yield: async () => { turns.push(inflated() - last); last = inflated(); } };
  try {
    // Random text first, which barely compresses, then guilds that compress more than a hundredfold.
    const guild = (i: number) => ({ id: `${i}`, name: 'the same guild again '.repeat(100), channels: [{ id: `${i}0`, name: 'general', type: 0 }] });
    const ready = { ...rig.ready, d: { ...rig.ready.d, guilds: Array.from({ length: 4000 }, (_, i) => guild(i)) } };
    mapper({ type: 'message', data: await packet(ready) });
    last = inflated();
    await digest(live, secretsStore() as any, EPOCH + 10);
    expect(live.gw!.phase).toBe('ready');
    expect(turns.length).toBeGreaterThan(3);
    expect(Math.max(...turns)).toBeLessThan(1.5 * 1024 * 1024);
  } finally { (globalThis as any).scheduler = previous; rig.restore(); }
});

test('19. a REST reply that lands after a sign-out leaves the next account\'s limits alone', async () => {
  const live = createLive({ rest: 'https://discord.com', gateway: 'wss://gateway.discord.gg', cdn: 'https://cdn.discordapp.com', fixture: false });
  const secrets = secretsStore();
  live.token = 'a'.repeat(60);
  const lands: ((response: Response) => void)[] = [];
  const fetchSpy = spyOn(globalThis, 'fetch').mockImplementation((() => new Promise<Response>(resolve => { lands.push(resolve); })) as any);
  try {
    const request = { method: 'GET', url: 'https://discord.com/api/v9/users/@me', headers: {}, route: 'GET /users/@me' } as any;
    const first = call(live, request);
    signOut(live, secrets);
    live.token = 'b'.repeat(60);
    const second = call(live, request);
    // The old account's request is still out on that route; the new account's is sent all the same.
    expect(lands).toHaveLength(2);
    lands[0](new Response(JSON.stringify({ message: 'You are being rate limited.', retry_after: 60, global: true }),
      { status: 429, headers: { 'X-RateLimit-Global': 'true', 'Retry-After': '60' } }));
    await expect(first).rejects.toThrow('Signed out');
    lands[1](new Response('{}', { status: 200 }));
    expect(await second).toEqual({});
  } finally { fetchSpy.mockRestore(); }
});

// Lock Excord (LLP 1133.010 §9): no answer carries the token, and a launch that begins locked answers nothing of
// the account, so the gateway opens nothing, until the person passes the check.
test('a locked launch answers no token and is not ready until unlocked; a dropped token signs the person out', async () => {
  const origins = { rest: 'http://127.0.0.1:4790', gateway: 'ws://127.0.0.1:4790', cdn: 'http://127.0.0.1:4790', fixture: true };
  const secrets = secretsStore(), storage = memoryStorage() as unknown as Storage, live = createLive(origins);
  secrets.set('fixture-token', 'secret-token-1');
  await secrets.protect('fixture-token');
  const locked = await loadSession(live, secrets as unknown as Store, storage);
  expect(locked).toEqual({ ready: false, signedIn: true, locked: true, lockOn: true });
  expect(await unlockSession(live, secrets as unknown as Store)).toEqual({ ok: true, error: '' });
  const unlocked = await loadSession(live, secrets as unknown as Store, storage);
  expect(unlocked).toEqual({ ready: true, signedIn: true, locked: false, lockOn: true });
  expect(JSON.stringify([locked, unlocked])).not.toContain('secret-token');

  const gone = { ...secretsStore(), locked: () => true, unlock: async () => null };
  const dropped = createLive(origins);
  expect(await unlockSession(dropped, gone as unknown as Store)).toEqual({ ok: false, error: 'Sign in again' });
  const after = { ...secretsStore(), locked: () => false };
  expect(await loadSession(dropped, after as unknown as Store, storage)).toEqual({ ready: true, signedIn: false, locked: false, lockOn: false });
});
