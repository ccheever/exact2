import { afterEach, describe, expect, spyOn, test } from 'bun:test';
import { emojiNames, photos, WORLD_NOW } from './transcript-content.ts';
import { startFixture, type FixtureOptions } from './server.ts';
import { compareIds } from './world.ts';

interface Peer {
  frames: any[];
  sessionId: string;
  closed: boolean;
  send(frame: object): Promise<void>;
  until(condition: () => boolean): Promise<void>;
  close(code?: number): Promise<void>;
}

function serverForTest(sockets: boolean, options: FixtureOptions = {}) {
  let handle: any, gateway: any;
  const serve = sockets ? null : spyOn(Bun, 'serve').mockImplementation(((config: any) => {
    handle = config.fetch; gateway = config.websocket;
    return { port: 4790, stop: async () => {} };
  }) as typeof Bun.serve);
  let fixture: ReturnType<typeof startFixture>;
  try { fixture = startFixture({ port: 0, world: 'transcript', ...options }); }
  finally { serve?.mockRestore(); }
  const peers: Peer[] = [];
  const request = async (path: string, body?: object, method = body ? 'POST' : 'GET'): Promise<Response> => {
    const request = new Request(fixture.origin + path, { method,
      headers: { Authorization: fixture.world.token, Origin: 'http://fixture-client.invalid' },
      ...(body ? { body: JSON.stringify(body) } : {}) });
    return sockets ? fetch(request) : handle(request, {});
  };
  const control = async (name: string, body: object = {}) => {
    const response = await request(`/control/${name}`, body);
    expect(response.status).toBe(200);
    return response.json();
  };
  const state = async () => (await request('/control/state')).json();
  const connect = async (capabilities = 0, resume?: { session_id: string; seq: number }): Promise<Peer> => {
    const waiters = new Set<() => void>();
    const wake = () => { for (const waiter of waiters) waiter(); };
    let send!: (frame: object) => Promise<void>, close!: (code: number) => void;
    const peer: Peer = { frames: [], sessionId: '', closed: false,
      send: frame => send(frame),
      until(condition) {
        return new Promise<void>((resolve, reject) => {
          const check = () => {
            if (condition()) { clearTimeout(timer); waiters.delete(check); resolve(); }
          };
          const timer = setTimeout(() => { waiters.delete(check); reject(new Error('Gateway condition timed out')); }, 3000);
          waiters.add(check); check();
        });
      },
      async close(code = 1000) { if (!peer.closed) close(code); await peer.until(() => peer.closed); },
    };
    peers.push(peer);
    if (sockets) {
      const socket = new WebSocket(fixture.gatewayUrl);
      socket.onmessage = event => { peer.frames.push(JSON.parse(String(event.data))); wake(); };
      socket.onclose = () => { peer.closed = true; wake(); };
      send = async frame => { socket.send(JSON.stringify(frame)); };
      close = code => socket.close(code);
    } else {
      let socket: any;
      await handle(new Request(fixture.origin, { headers: { Upgrade: 'websocket' } }), {
        upgrade(_request: Request, { data }: any) {
          socket = { data,
            send(text: string) { peer.frames.push(JSON.parse(text)); wake(); return text.length; },
            close(code: number) { gateway.close(socket, code); peer.closed = true; wake(); },
          };
          gateway.open(socket); return true;
        },
      });
      send = async frame => { await gateway.message(socket, JSON.stringify(frame)); };
      close = code => socket.close(code);
    }
    await peer.until(() => peer.frames.some(frame => frame.op === 10));
    await peer.send({ op: resume ? 6 : 2, d: { token: fixture.world.token, capabilities, ...resume } });
    await peer.until(() => peer.frames.some(frame => frame.t === (resume ? 'RESUMED' : 'READY_SUPPLEMENTAL')));
    peer.sessionId = resume?.session_id ?? peer.frames.find(frame => frame.t === 'READY').d.session_id;
    return peer;
  };
  const barrier = async (peer: Peer) => {
    const acks = peer.frames.filter(frame => frame.op === 11).length;
    await peer.send({ op: 1, d: null });
    await peer.until(() => peer.frames.filter(frame => frame.op === 11).length > acks);
  };
  const inject = async (what: string, extra: object = {}, channelId = fixture.world.generalChannelId) => {
    const active = peers.filter(peer => !peer.closed);
    const starts = active.map(peer => peer.frames.length);
    const reply = await control('inject', { channel_id: channelId, what, ...extra });
    await Promise.all(active.map(barrier));
    for (const [i, peer] of active.entries()) {
      expect(reply.dispatches.filter((frame: any) => frame.session_id === peer.sessionId)).toEqual(
        peer.frames.slice(starts[i]).filter(frame => frame.op === 0).map(({ s, t, d }) => ({
          session_id: peer.sessionId, s, t, d,
        })));
    }
    expect(reply.dispatches.every((frame: any) => active.some(peer => peer.sessionId === frame.session_id))).toBe(true);
    return reply.dispatches as any[];
  };
  const raw = (request: Request): Promise<Response> => sockets ? fetch(request) : handle(request, {});
  return { fixture, request, raw, control, state, connect, inject, barrier,
    async close() { await Promise.all(peers.map(peer => peer.close())); await fixture.close(); },
  };
}

for (const sockets of [false, true]) describe(sockets ? 'with sockets' : 'without ports', () => {
  let rig: ReturnType<typeof serverForTest> | undefined;
  const start = (options: FixtureOptions = {}) => (rig = serverForTest(sockets, options));
  afterEach(async () => { await rig?.close(); rig = undefined; });

  test('fixed clock, monotonic creation, transcript reset and complete READY', async () => {
    const r = start(), { fixture } = r;
    const peer = await r.connect();
    await r.barrier(peer);
    expect(peer.frames.filter(frame => frame.op === 0).map(frame => frame.t)).toEqual(['READY', 'READY_SUPPLEMENTAL']);
    expect(peer.frames.find(frame => frame.t === 'READY').d.guilds[0].properties.name).toBe('Exact Club');
    expect(await r.state()).toMatchObject({ now_ms: WORLD_NOW, clock_offset_ms: 0 });
    const path = `/api/v9/channels/${fixture.world.generalChannelId}/messages`;
    const first = await (await r.request(path, { content: 'first' })).json();
    expect(first.timestamp).toBe(new Date(WORLD_NOW).toISOString());
    expect(first.dispatches).toBeUndefined();
    const burst = await r.inject('burst', { at_ms: WORLD_NOW + 10000 });
    expect(burst.map(frame => Date.parse(frame.d.timestamp))).toEqual(
      Array.from({ length: 6 }, (_, i) => WORLD_NOW + 10000 + i * 500));
    const backward = await r.inject('message', { at_ms: WORLD_NOW - 1000 });
    const next = await (await r.request(path, { content: 'next' })).json();
    expect(next.timestamp).toBe(burst.at(-1).d.timestamp);
    expect(backward[0].d.timestamp).toBe(next.timestamp);
    expect(compareIds(burst.at(-1).d.id, backward[0].d.id)).toBeLessThan(0);
    expect(compareIds(backward[0].d.id, next.id)).toBeLessThan(0);
    await r.control('clock', { advance_ms: 60000 });
    expect(Date.parse((await (await r.request(path, { content: 'later' })).json()).timestamp)).toBe(WORLD_NOW + 60000);
    await r.control('reset');
    await peer.until(() => peer.closed);
    expect(fixture.world.messages[fixture.world.generalChannelId]).toHaveLength(10000);
    expect(await r.state()).toMatchObject({ now_ms: WORLD_NOW, sessions: 0, connections: 0, clock_offset_ms: 0 });
    expect(await (await r.request(path, { content: 'first' })).json()).toEqual(first);
  });

  test('--history-delay holds only an older page, which still lands whole', async () => {
    const r = start({ historyDelayMs: 2000 }), { fixture } = r;
    const path = `/api/v9/channels/${fixture.world.generalChannelId}/messages`;
    const order: string[] = [];
    const timed = async (name: string, query: string) => {
      const t0 = performance.now(), response = await r.request(`${path}?${query}`), rows = await response.json();
      order.push(name);
      return { ms: performance.now() - t0, rows };
    };
    const newest = await timed('newest', 'limit=50');
    // The older page is held; the newest and an around page asked after it are not, so they answer first.
    const older = timed('older', `limit=50&before=${newest.rows.at(-1).id}`);
    await timed('newest again', 'limit=50');
    await timed('around', `limit=50&around=${newest.rows[10].id}`);
    const held = await older;
    expect(order).toEqual(['newest', 'newest again', 'around', 'older']);
    expect(held.ms).toBeGreaterThanOrEqual(1990);
    expect(held.rows).toHaveLength(50);
    expect(compareIds(held.rows[0].id, newest.rows.at(-1).id)).toBeLessThan(0);
  });

  test('wall clock can override transcript default and fixed clock also works for the default world', async () => {
    const r = start({ clock: 'wall' });
    const before = Date.now(), state = await r.state(), after = Date.now();
    expect(state.now_ms).toBeGreaterThanOrEqual(before); expect(state.now_ms).toBeLessThanOrEqual(after);
    await r.close(); rig = undefined;
    const fixed = start({ world: 'default', clock: 'fixed' });
    expect((await fixed.state()).now_ms).toBe(WORLD_NOW);
    expect(fixed.fixture.world.guilds).toHaveLength(3);
  });

  test('typing, bursts and passive updates report the exact per-session wire frames', async () => {
    const r = start(), { fixture } = r;
    const plain = await r.connect(), passive = await r.connect((1 << 11) | (1 << 14)), subscribed = await r.connect(1 << 14);
    await subscribed.send({ op: 37, d: { subscriptions: { [fixture.world.guilds[0].id]: {} } } });
    await r.barrier(subscribed);
    const users = fixture.world.users;
    for (const index of [1, 2]) {
      const frames = await r.inject('typing');
      expect(frames).toHaveLength(2);
      expect(frames.every(frame => frame.t === 'TYPING_START' && frame.d.user_id === users[index].id)).toBe(true);
    }
    const frames = await r.inject('burst');
    expect(frames.filter(frame => frame.session_id === passive.sessionId).map(frame => frame.t)).toEqual(Array(6).fill('PASSIVE_UPDATE_V2'));
    for (const peer of [plain, subscribed]) {
      const creates = frames.filter(frame => frame.session_id === peer.sessionId);
      expect(creates.map(frame => frame.d.author.id)).toEqual([1, 2, 3, 1, 2, 3].map(i => users[i].id));
      expect(creates.map(frame => frame.d.content)).toEqual(['The new build is ready to try.', 'The anchor held on my phone.',
        'Nice! Checking a longer thread next.', 'I added a photo to the reproduction.',
        'Works after rotating, too.', 'Thanks for checking it!']);
    }
    expect((await r.inject('typing'))[0].d.user_id).toBe(users[1].id);
    expect((await r.inject('message'))[0].d.author.id).toBe(users[1].id);
    expect((await r.inject('typing'))[0].d.user_id).toBe(users[1].id);
    const dm = await r.inject('message', {}, fixture.world.private_channels[0].id);
    expect(dm).toHaveLength(3);
    expect(dm.every(frame => frame.t === 'MESSAGE_CREATE' && !Object.hasOwn(frame.d, 'guild_id'))).toBe(true);
  });

  test('above edits, embeds, repeated reactions and deletions match the stand-in', async () => {
    const r = start(), { fixture } = r;
    await r.connect(1 << 13);
    const passive = await r.connect(1 << 14);
    const channelId = fixture.world.generalChannelId, history = fixture.world.messages[channelId];
    const near = history[8000].id, target = history[7997], content = target.content;
    const edit = await r.inject('edit-above', { near, at_ms: WORLD_NOW + 1234 });
    expect(edit).toHaveLength(1); expect(edit[0].t).toBe('MESSAGE_UPDATE');
    expect(edit[0].d).toEqual(target);
    expect(target.content).toBe(content + '\nA longer update from the device:\nThe image finished decoding.'
      + '\nThe keyboard opened and closed.\nThe visible row kept its offset.');
    expect(target.edited_timestamp).toBe(new Date(WORLD_NOW + 1234).toISOString());
    const originalEdit = structuredClone(edit[0]);
    const embed = await r.inject('embed-above', { near });
    expect(embed[0].d).toEqual(target);
    expect(target.embeds.at(-1)).toMatchObject({ type: 'rich', color: 0x2f9e78,
      provider: { name: 'Exact Journal' }, author: { name: 'Buildkite' },
      image: { url: `${fixture.origin}/media/ridge.png`, proxy_url: `${fixture.origin}/media/ridge.png`, width: 800, height: 600 } });
    expect(edit[0]).toEqual(originalEdit);
    for (let i = 1; i <= 2; i++) {
      const reaction = await r.inject('react-above', { near });
      expect(reaction).toHaveLength(1);
      expect(reaction[0]).toMatchObject({ t: 'MESSAGE_REACTION_ADD', d: { message_id: target.id,
        emoji: { id: null, name: '✨' }, user_id: fixture.world.users[1].id } });
      expect(target.reactions.find(reaction => reaction.emoji.name === '✨')).toMatchObject({ count: i, me: false,
        count_details: { normal: i, burst: 0 } });
    }
    await r.control('react', { channel_id: channelId, message_id: target.id, emoji: '🌵',
      user_id: fixture.world.users[2].id, add: true });
    const flushed = await r.inject('react-above', { near });
    expect(flushed.map(frame => [frame.t, frame.d.emoji.name])).toEqual([
      ['MESSAGE_REACTION_ADD', '🌵'], ['MESSAGE_REACTION_ADD', '✨'],
    ]);
    expect((await r.state()).session_details.every((session: any) => session.pending_reaction_batches === 0)).toBe(true);
    expect((await r.inject('delete-above', { near }))[0].d.id).toBe(target.id);
    expect(fixture.world.messages[channelId].some(message => message.id === target.id)).toBe(false);
    const fallback = fixture.world.messages[channelId].at(-11)!;
    expect((await r.inject('edit-above', { near: '999' }))[0].d.id).toBe(fallback.id);
    expect((await r.inject('edit-above', { near: history[0].id }))[0].d.id).toBe(history[0].id);
    expect(passive.frames.filter(frame => frame.t?.startsWith('MESSAGE_'))).toHaveLength(0);
  });

  test('delete-unread, fail-next and invalid inputs leave the correct history', async () => {
    const r = start(), { fixture } = r;
    await r.connect();
    const channelId = fixture.world.generalChannelId, history = fixture.world.messages[channelId];
    const firstUnread = history.at(-37)!;
    expect((await r.inject('delete-unread'))[0].d.id).toBe(firstUnread.id);
    expect(fixture.world.messages[channelId]).toHaveLength(9999);
    const emptyUnread = fixture.world.guilds[0].channels[2].id;
    expect((await r.request('/control/inject', { channel_id: emptyUnread, what: 'delete-unread' })).status).toBe(400);
    const before = fixture.world.messages[channelId].length;
    expect(await r.inject('fail-next')).toEqual([]);
    await r.control('rate-limit', { global: true, count: 1 });
    const path = `/api/v9/channels/${channelId}/messages`;
    const failure = await r.request(path, { content: 'try once' });
    expect(failure.status).toBe(500); expect(await failure.json()).toEqual({ message: 'Fixture failure', code: 0 });
    expect(fixture.world.messages[channelId]).toHaveLength(before);
    expect((await r.request(path, { content: 'rate limited' })).status).toBe(429);
    const success = await r.request(path, { content: 'try again' });
    expect(success.status).toBe(200); expect((await success.json()).dispatches).toBeUndefined();
    expect(fixture.world.messages[channelId]).toHaveLength(before + 1);
    for (const extra of [{ what: 'unknown' }, { what: 'message', at_ms: -1 }, { what: 'edit-above', near: 5 },
      { what: 'burst', at_ms: 'not a time' }]) {
      expect((await r.request('/control/inject', { channel_id: channelId, ...extra })).status).toBe(400);
    }
  });

  test('IDENTIFY resets all controls and other sockets, while RESUME preserves mutations', async () => {
    const r = start({ resetOnIdentify: true }), { fixture } = r;
    const old = await r.connect(1 << 13);
    const channelId = fixture.world.generalChannelId;
    await r.inject('message');
    await r.inject('typing');
    await r.inject('fail-next');
    await r.control('clock', { advance_ms: 60000 });
    await r.control('react', { channel_id: channelId, message_id: fixture.world.messages[channelId].at(-1)!.id,
      emoji: '🌵', user_id: fixture.world.me.id, add: true });
    await r.control('stop-acks', { on: true });
    await r.control('ready-size', { bytes: 100000 });
    await r.control('login-burst', { bytes: 5000 });
    await r.control('split-frames', { parts: 7 });
    await r.control('rate-limit', { global: true, count: 5 });
    const fresh = await r.connect();
    await old.until(() => old.closed);
    await r.barrier(fresh);
    expect(fixture.world.messages[channelId]).toHaveLength(10000);
    expect(await r.state()).toMatchObject({ connections: 1, sessions: 1, identify_count: 1, resume_count: 0,
      logins_done: 1, clock_offset_ms: 0, now_ms: WORLD_NOW, stop_acks: false, ready_bytes: 0, login_bytes: 0,
      split_parts: 1, passive_updates: 0 });
    expect(fresh.frames.filter(frame => frame.op === 0).map(frame => frame.t)).toEqual(['READY', 'READY_SUPPLEMENTAL']);
    expect((await r.inject('typing'))[0].d.user_id).toBe(fixture.world.users[1].id);
    const path = `/api/v9/channels/${channelId}/messages`;
    const sent = await r.request(path, { content: 'preserved on resume' });
    expect(sent.status).toBe(200);
    await r.barrier(fresh);
    const lastSeq = fresh.frames.filter(frame => frame.op === 0).at(-1).s;
    await fresh.close(4000);
    await r.inject('message');
    await r.inject('fail-next');
    await r.control('clock', { advance_ms: 1000 });
    const resumed = await r.connect(0, { session_id: fresh.sessionId, seq: lastSeq });
    expect(resumed.frames.filter(frame => frame.op === 0).map(frame => frame.t)).toEqual(['MESSAGE_CREATE', 'RESUMED']);
    expect(fixture.world.messages[channelId]).toHaveLength(10002);
    expect(await r.state()).toMatchObject({ identify_count: 1, resume_count: 1, clock_offset_ms: 1000 });
    expect((await r.request(path, { content: 'failure survived resume' })).status).toBe(500);
  });

  test('overlapping injections keep their own delivered dispatches', async () => {
    const r = start();
    const peer = await r.connect();
    const channel_id = r.fixture.world.generalChannelId;
    const replies = await Promise.all(['burst', 'typing', 'message'].map(what => r.control('inject', { channel_id, what })));
    await r.barrier(peer);
    expect(replies.map(reply => reply.dispatches.length)).toEqual([6, 1, 1]);
    const dispatches = replies.flatMap(reply => reply.dispatches);
    expect(new Set(dispatches.map(frame => frame.s)).size).toBe(8);
    for (const { session_id, s, t, d } of dispatches) {
      expect(session_id).toBe(peer.sessionId);
      expect(peer.frames.find(frame => frame.s === s)).toEqual({ op: 0, s, t, d });
    }
  });

  test('local PNGs, CORS and rejected asset paths', async () => {
    const r = start();
    for (const [i, path] of [...photos.map(photo => `/media/${photo.name}.png`),
      ...emojiNames.map((_, i) => `/emojis/${500 + i}.png`)].entries()) {
      const response = await r.request(path);
      expect(response.status).toBe(200);
      expect(response.headers.get('Content-Type')).toBe('image/png');
      expect(response.headers.get('Access-Control-Allow-Origin')).toBe('http://fixture-client.invalid');
      const bytes = new Uint8Array(await response.arrayBuffer());
      expect([...bytes.slice(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
      const local = i < photos.length ? `media/${photos[i].name}` : `emoji/${emojiNames[i - photos.length]}`;
      expect(bytes).toEqual(new Uint8Array(await Bun.file(`${import.meta.dir}/../assets/${local}.png`).arrayBuffer()));
    }
    for (const path of ['/media/Harbor.png', '/media/harbor1.png', '/media/unknown.png', '/media/%2e%2e%2fharbor.png',
      '/media/harbor.svg', '/emojis/499.png', '/emojis/509.png', '/emojis/0501.png', '/emojis/ferris.png']) {
      expect((await r.request(path)).status).toBe(404);
    }
    expect((await r.request('/media/harbor.png', {}, 'POST')).status).toBe(404);
  });

  test('a file goes up as Discord takes it: a slot, a PUT without credentials, then the message names it', async () => {
    const r = start(), channel = r.fixture.world.generalChannelId;
    const png = new Uint8Array(await Bun.file(`${import.meta.dir}/../assets/media/cafe.png`).arrayBuffer());
    const slots = await (await r.request(`/api/v9/channels/${channel}/attachments`,
      { files: [{ id: '0', filename: 'cafe.png', file_size: png.length, is_clip: false }] })).json();
    const [slot] = slots.attachments;
    expect(slot).toMatchObject({ id: '0', upload_filename: '00000001/cafe.png' });
    const put = (headers: Record<string, string>) => r.raw(new Request(slot.upload_url, { method: 'PUT', headers, body: png }));
    expect((await put({ Authorization: r.fixture.world.token })).status).toBe(400);
    expect((await r.request(`/api/v9/channels/${channel}/messages`, { content: '', nonce: '9',
      attachments: [{ id: '0', filename: 'cafe.png', uploaded_filename: slot.upload_filename }] })).status).toBe(400);
    expect((await put({})).status).toBe(200);
    const sent = await (await r.request(`/api/v9/channels/${channel}/messages`, { content: '', nonce: '10',
      attachments: [{ id: '0', filename: 'cafe.png', uploaded_filename: slot.upload_filename }] })).json();
    expect(sent.attachments[0]).toMatchObject({ filename: 'cafe.png', size: png.length, content_type: 'image/png', width: 640, height: 640 });
    const served = await r.request(new URL(sent.attachments[0].url).pathname);
    expect(new Uint8Array(await served.arrayBuffer())).toEqual(png);
    expect((await r.request(`/api/v9/channels/${channel}/messages`, { content: ' ', nonce: '11' })).status).toBe(400);
  });
});
