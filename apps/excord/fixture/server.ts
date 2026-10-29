#!/usr/bin/env bun
// A Discord stand-in for tests and agent drives (llp/0001 §4.4): the user gateway (zlib-stream or
// text) and the REST paths Excord uses, over the seeded world, plus `/control/*` to inject traffic.
// `bun apps/excord/fixture/server.ts --world transcript --reset-on-identify`; app.ts's `origin` points here.
// `--history-delay <ms>` holds each older page's answer that long (a phone network's round trip).
// POST /control/clock {advance_ms} advances the selected clock for timestamps, nonce expiry,
// rate limits and reaction batches, flushing due batches before returning. Reset clears the offset.
// With passive capabilities, full guild events require subscribeAll, POST /control/subscribe
// {guild_id} after login, or op 14 {guild_id} / op 37 {subscriptions: {[guild_id]: {}}}.
// Either subscription opcode accepts either shape.
// POST /control/hold-sends {on} holds the app's sends until released, in order.
// POST /control/hold-media {on} holds full-size pictures (a /media/ GET with no query, as the viewer asks;
// the transcript's resized ones carry `width`) until released.
// POST /control/heartbeat-request asks every socket for op 1; POST /control/react-many
// {channel_id, message_id, emoji, user_ids} adds reactions and flushes their batch immediately.
// POST /control/vote {channel_id, message_id, user_id, answer_id, add} adds or takes back one vote on a poll.
// POST /control/rotate-token {token} makes the account's token `token` (a reset keeps it) and sends
// each session a READY carrying it as `auth_token`, as Discord rotates a client's token; /control/state's
// `identify_token` is the token the last IDENTIFY carried.
import { avatarPng, imageSize } from './avatar.ts';
import { constants, createDeflate } from 'node:zlib';
import { AsyncLocalStorage } from 'node:async_hooks';
import type { ServerWebSocket } from 'bun';
import { emojiNames, WORLD_NOW } from './transcript-content.ts';
import { createTranscriptWorld } from './transcript-world.ts';
import { inject, type InjectedDispatch } from './inject.ts';
import { settingsProto } from './settings.ts';
import {
  createMessageGenerator, createWorld, emptyMessage, fileAssets, imageArtwork, pageMessages,
  NONCE_WINDOW_MS, recentNonceMessage, replySnapshot, seededRandom, snowflakeFromMs,
  type Channel, type Emoji, type Guild, type Message, type PageQuery, type World,
} from './world.ts';

export function createZlibFramer() {
  const stream = createDeflate();
  let tail: Promise<unknown> = Promise.resolve();
  let chunks: Buffer[] = [];
  let failure: Error | undefined;
  let closed = false;
  stream.on('data', (chunk: Buffer) => chunks.push(chunk));
  stream.on('error', error => { failure = error; });
  return {
    encode(text: string): Promise<Buffer> {
      if (closed) return Promise.reject(new Error('zlib framer is closed'));
      const result = tail.then(() => new Promise<Buffer>((resolve, reject) => {
        if (failure) { reject(failure); return; }
        chunks = [];
        stream.write(text);
        // Discord keeps one compression dictionary for the whole connection.
        stream.flush(constants.Z_SYNC_FLUSH, error => {
          if (error || failure) reject(error || failure);
          else resolve(Buffer.concat(chunks));
        });
      }));
      tail = result.catch(() => {});
      return result;
    },
    async close() {
      closed = true;
      await tail;
      stream.destroy();
    },
  };
}

export function splitFrames(bytes: Uint8Array, parts: number): Uint8Array[] {
  const count = Math.max(1, Math.min(bytes.length || 1, Math.trunc(parts) || 1));
  return Array.from({ length: count }, (_, i) => bytes.subarray(
    Math.floor(i * bytes.length / count), Math.floor((i + 1) * bytes.length / count)));
}

interface Dispatch { op: 0; t: string; s: number; d: any }
interface ReactionAdd {
  channel_id: string; message_id: string; guild_id?: string; user_id: string; emoji: Emoji;
  burst: false; type: 0;
}
interface ReactionBatch { dueAt: number; adds: ReactionAdd[] }
interface Session {
  id: string; seq: number; dispatches: string[];
  capabilities: number;
  subscribedGuilds: Set<string>;
  subscriptions: { op: number; guild_ids: string[] }[];
  passiveUpdates: number;
  reactions: Map<string, ReactionBatch>;
}
interface Connection {
  id: number;
  compressed: boolean;
  encoder?: ReturnType<typeof createZlibFramer>;
  queue: Promise<unknown>;
  session?: Session;
  closed: boolean;
}
type Socket = ServerWebSocket<Connection>;
export interface RateLimit { limit: number; windowMs: number }
export interface FixtureOptions {
  port?: number;
  hostname?: string;
  seed?: number;
  world?: 'default' | 'transcript';
  clock?: 'wall' | 'fixed'; // Default: fixed for transcript, wall otherwise.
  resetOnIdentify?: boolean;
  heartbeatInterval?: number;
  nonceWindowMs?: number; // Default: five minutes, measured on the fixture clock.
  reactionDebounceMs?: number; // Default: 500 ms from the first add in each message's batch.
  subscribeAll?: boolean; // Default: false; true subscribes every new session to all seeded guilds.
  rateLimits?: { default?: RateLimit; messages?: RateLimit; routes?: Record<string, RateLimit> };
  // An older page's answer (a GET of a channel's messages with `before=`) waits this long, as a phone network's
  // round trip does: nothing else does, not the newest page, the gateway or a send. Default: 0.
  historyDelayMs?: number;
}
interface Bucket extends RateLimit { used: number; reset: number }
interface ForcedLimit { route?: string; global: boolean; count: number; retry_after: number }
interface FrameSize {
  connection_id: number;
  session_id: string | null;
  op: number;
  t: string | null;
  wire_bytes: number;
  inflated_bytes: number;
  compressed: boolean;
  part: number;
  parts: number;
}

const selectedHeaders = ['authorization', 'x-super-properties', 'content-type', 'x-discord-locale',
  'x-discord-timezone', 'x-debug-options', 'user-agent', 'origin'];
const allowedHeaders = 'Authorization, X-Super-Properties, Content-Type, X-Discord-Locale, X-Discord-Timezone, X-Debug-Options';
const rateHeaderNames = ['X-RateLimit-Limit', 'X-RateLimit-Remaining', 'X-RateLimit-Reset',
  'X-RateLimit-Reset-After', 'X-RateLimit-Bucket', 'X-RateLimit-Global', 'X-RateLimit-Scope', 'Retry-After'];
const PASSIVE_GUILD_UPDATE = 1 << 11;
const DEBOUNCE_MESSAGE_REACTIONS = 1 << 13;
const PASSIVE_GUILD_UPDATE_V2 = 1 << 14;

class HttpError extends Error {
  constructor(public status: number, message: string, public code = 0) { super(message); }
}

function integer(value: unknown, fallback: number, max = Number.MAX_SAFE_INTEGER): number {
  const n = value === undefined ? fallback : Number(value);
  if (!Number.isSafeInteger(n) || n < 0 || n > max) throw new HttpError(400, 'Invalid integer');
  return n;
}

function textField(body: any, name: string): string {
  if (typeof body[name] !== 'string') throw new HttpError(400, `${name} must be a string`);
  return body[name];
}

function rateRoute(method: string, path: string): string {
  const normalized = path.replace(/^\/api\/v9/, '')
    .replace(/(\/messages\/)\d+/, '$1:message_id')
    .replace(/(\/reactions\/)[^/]+/, '$1:emoji');
  return `${method} ${normalized}`;
}

function readyGuild(guild: Guild) {
  const { id, channels, roles, members, emojis, ...properties } = guild;
  return { id, properties, channels, roles, members, emojis, joined_at: '2024-01-01T00:00:00.000Z',
    large: false, member_count: members.length, version: 1 };
}

// A large public server's permissions: many roles, channels whose overwrites name them, and the
// account a member holding some of the roles rather than the owner.
export interface GuildShape { roles?: number; channels?: number; overwrites?: number; dms?: number }

export function syntheticGuild(world: World, index: number, randomBytes = 0, shape: GuildShape = {}): Guild {
  const random = seededRandom(world.seed + index * 65537);
  const source = world.guilds[0];
  const id = snowflakeFromMs(Date.UTC(2024, 0, 1), index);
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
  const word = (size: number) => {
    const chars = Buffer.allocUnsafe(size);
    for (let i = 0; i < size; i++) chars[i] = alphabet.charCodeAt(Math.floor(random() * alphabet.length));
    return chars.toString('ascii');
  };
  const count = Math.max(32, Math.ceil(randomBytes / 320));
  const roleCount = Math.max(1, shape.roles ?? 1), overwrites = shape.overwrites ?? 0;
  const roles = [{ ...source.roles[0], id }, ...Array.from({ length: roleCount - 1 }, (_, r) => ({ ...source.roles[0],
    id: snowflakeFromMs(Date.UTC(2024, 0, 4) + index, r), name: `role-${r}`, position: r + 1, permissions: '0' }))];
  const mine = roles.slice(1).filter((_, r) => r % 10 === 0).map(role => role.id);
  return { ...source, id, name: `Workshop ${index}`, owner_id: shape.roles ? world.users[1].id : world.me.id,
    description: 'A community for making, learning, and sharing projects.', member_count: count,
    roles,
    channels: Array.from({ length: shape.channels ?? 8 }, (_, c) => ({ ...source.channels[1],
      id: snowflakeFromMs(Date.UTC(2024, 0, 2) + index, c), guild_id: id,
      name: `project-${c}`, position: c, parent_id: null, last_message_id: null,
      permission_overwrites: Array.from({ length: Math.min(overwrites, roleCount) }, (_, o) => ({
        id: roles[(c * 7 + o * 13) % roleCount].id, type: 0, allow: o % 2 ? '1024' : '0', deny: o % 2 ? '0' : '2048' })) })),
    members: [...(shape.roles ? [{ user: world.me, roles: mine, joined_at: '2024-01-01T00:00:00.000Z', deaf: false, mute: false, flags: 0 }] : []),
      ...Array.from({ length: count }, (_, m) => ({
      user: { ...world.users[m % world.users.length],
        id: snowflakeFromMs(Date.UTC(2024, 0, 3) + index, m), username: word(24), global_name: word(24),
        avatar: word(32), bio: randomBytes ? word(320) : 'Making a little progress every day.' },
      roles: [], nick: word(24), joined_at: '2024-01-01T00:00:00.000Z', deaf: false, mute: false, flags: 0,
    }))],
  };
}

export function readyPayload(world: World, sessionId: string, gatewayUrl: string) {
  return {
    v: 9, user: world.me,
    guilds: world.guilds.map(guild => world.lazyGuildId && guild.id === world.lazyGuildId
      ? { id: guild.id, unavailable: true } : readyGuild(guild)),
    private_channels: world.private_channels, users: world.users.filter(user => user.id !== world.me.id),
    read_state: world.read_state,
    user_guild_settings: { entries: world.guilds.map(guild => ({ guild_id: guild.id,
      muted: false, suppress_everyone: false, suppress_roles: false, message_notifications: 1,
      channel_overrides: [] })), partial: false, version: 1 },
    session_id: sessionId, resume_gateway_url: gatewayUrl,
    sessions: [{ session_id: sessionId, status: 'online', activities: [],
      client_info: { client: 'desktop', os: 'unknown', version: 0 } }],
    auth_session_id_hash: 'fixture-auth-session', country_code: 'US',
    analytics_token: 'fixture-analytics', relationships: [], presences: [],
    connected_accounts: [], experiments: [], guild_experiments: [], friend_suggestion_count: 0,
    required_action: null, tutorial: null, user_settings: { locale: 'en-US', theme: 'dark', status: 'online' },
    user_settings_proto: settingsProto(world.guilds.map(guild => ({ ids: [guild.id] }))),
  };
}

export function sizedReady(world: World, sessionId: string, gatewayUrl: string, seq: number, bytes: number, shape: GuildShape = {}): Dispatch {
  const frame: Dispatch = { op: 0, t: 'READY', s: seq, d: readyPayload(world, sessionId, gatewayUrl) };
  // A long-lived account's direct messages: one per old conversation, a few unread. READY shares the
  // world's lists, so they are copied first.
  frame.d.private_channels = [...frame.d.private_channels];
  frame.d.read_state = { ...frame.d.read_state, entries: [...frame.d.read_state.entries] };
  for (let i = 0; i < (shape.dms ?? 0); i++) {
    const id = snowflakeFromMs(Date.UTC(2023, 0, 1) + i, 0), last = snowflakeFromMs(Date.UTC(2023, 6, 1) + i * 1000, 0);
    frame.d.private_channels.push({ id, type: 1, recipient_ids: [world.users[1 + i % (world.users.length - 1)].id],
      last_message_id: last, flags: 0 });
    frame.d.read_state.entries.push(i % 25 === 0 ? { id, last_message_id: id, mention_count: 1 } : { id, last_message_id: last, mention_count: 0 });
  }
  let size = Buffer.byteLength(JSON.stringify(frame));
  let index = 1;
  while (size < bytes) {
    const guild = readyGuild(syntheticGuild(world, index++, 0, shape));
    const added = Buffer.byteLength(JSON.stringify(guild)) + 1;
    if (size + added > bytes) break;
    frame.d.guilds.push(guild);
    size += added;
  }
  // The synthetic guilds sit in a folder after the world's, newest at its top; the settings grow
  // with them, so a guild that no longer fits comes off.
  for (;;) {
    const synthetic = frame.d.guilds.slice(world.guilds.length).map((guild: Guild) => guild.id).reverse();
    if (!synthetic.length) break;
    frame.d.user_settings_proto = settingsProto([...world.guilds.map(guild => ({ ids: [guild.id] })), { ids: synthetic, id: 4242, name: 'Workshops', color: 0x5865f2 }]);
    size = Buffer.byteLength(JSON.stringify(frame));
    if (size <= bytes) break;
    frame.d.guilds.pop();
  }
  if (size < bytes) {
    const description = frame.d.guilds[0].properties;
    const sentence = ' Members share project updates and workshop notes.';
    description.description += sentence.repeat(Math.ceil((bytes - size) / sentence.length)).slice(0, bytes - size);
  }
  return frame;
}

export function startFixture(options: FixtureOptions = {}) {
  const seed = options.seed ?? 1;
  const hostname = options.hostname ?? '127.0.0.1';
  const fixedClock = (options.clock ?? (options.world === 'transcript' ? 'fixed' : 'wall')) === 'fixed';
  const buildWorld = () => options.world === 'transcript' ? createTranscriptWorld(origin) : createWorld(seed, origin);
  const heartbeatInterval = options.heartbeatInterval ?? 41250;
  const nonceWindowMs = integer(options.nonceWindowMs, NONCE_WINDOW_MS);
  const reactionDebounceMs = integer(options.reactionDebounceMs, 500, 2147483647);
  let world: World;
  let origin = '', gatewayUrl = '';
  // Files a send uploads, as Discord's upload bucket holds them: by the name its slot gave, bytes once PUT.
  const uploads = new Map<string, { filename: string; bytes: Uint8Array | null }>();
  let uploadCount = 0;
  let connectionCounter = 0, sessionCounter = 0, identifyCount = 0, resumeCount = 0, loginsDone = 0;
  let rotatedToken: string | null = null, identifyToken: string | null = null;
  let stopAcks = false, readyBytes = 0, loginBytes = 0, frameParts = 1, syntheticCounter = 10000;
  let readyShape: GuildShape = {};
  // The next login holds the guilds back for this long (READY names them unavailable, GUILD_CREATE comes later):
  // what the launch's restore waits on while the reader acts first. Kept through a reset (it is for the next
  // IDENTIFY, which resets the world; an explicit reset drops it).
  let holdGuildsMs = 0;
  let closed = false;
  let resetVersion = 0;
  let clockOffsetMs = 0, messageTimestamp = 0, messageSequence = 0, passiveUpdateCount = 0;
  let reactionTimer: ReturnType<typeof setTimeout> | undefined;
  const nowMs = () => (fixedClock ? WORLD_NOW : Date.now()) + clockOffsetMs;
  const sockets = new Set<Socket>();
  const sessions = new Map<string, Session>();
  const buckets = new Map<string, Bucket>();
  const forced: ForcedLimit[] = [];
  const requests: { method: string; path: string; headers: Record<string, string> }[] = [];
  const frames: FrameSize[] = [];
  const reactionUsers = new Map<string, Set<string>>();
  const burstTimers = new Map<ReturnType<typeof setTimeout>, () => void>();
  const typingUsers = new Map<string, Set<string>>();
  const failNext = new Set<string>();
  // While held, a send's reply (and its MESSAGE_CREATE) waits for `hold-sends {on: false}`.
  let holdSends = false;
  const heldSends: (() => void)[] = [];
  const releaseSends = () => { holdSends = false; for (const resume of heldSends.splice(0)) resume(); };
  let holdMedia = false;
  const heldMedia: (() => void)[] = [];
  const releaseMedia = () => { holdMedia = false; for (const resume of heldMedia.splice(0)) resume(); };
  const injectionDispatches = new AsyncLocalStorage<InjectedDispatch[]>();

  function nextMessage(requested = nowMs()) {
    let ms = Math.max(requested, messageTimestamp);
    messageSequence = ms === messageTimestamp ? messageSequence + 1 : 0;
    if (messageSequence >= 2 ** 22) { ms++; messageSequence = 0; }
    messageTimestamp = ms;
    world.nextTimestamp = ms + 1;
    return { ms, id: snowflakeFromMs(ms, messageSequence) };
  }

  function send(socket: Socket, frame: object | string): Promise<number> {
    const serialized = typeof frame === 'string' ? frame : JSON.stringify(frame);
    const meta = JSON.parse(serialized);
    const parts = frameParts;
    const captured = injectionDispatches.getStore();
    const sessionId = socket.data.session?.id;
    const result = socket.data.queue.then(async () => {
      if (socket.data.closed) return 0;
      const compressed = socket.data.compressed;
      const bytes = compressed ? await socket.data.encoder!.encode(serialized) : Buffer.from(serialized);
      if (socket.data.closed) return 0;
      // Splitting is at WebSocket message boundaries, before the zlib suffix.
      // Uncompressed JSON stays in one message so JSON.parse can read it.
      const pieces = compressed ? splitFrames(bytes, parts) : [bytes];
      for (let i = 0; i < pieces.length; i++) {
        socket.send(compressed ? pieces[i] : serialized);
        frames.push({ connection_id: socket.data.id, session_id: socket.data.session?.id ?? null,
          op: meta.op, t: meta.t ?? null, wire_bytes: pieces[i].byteLength,
          inflated_bytes: bytes === pieces[i] || i === 0 ? Buffer.byteLength(serialized) : 0,
          compressed, part: i + 1, parts: pieces.length });
      }
      if (frames.length > 512) frames.splice(0, frames.length - 512);
      if (captured && meta.op === 0 && sessionId) {
        captured.push({ session_id: sessionId, s: meta.s, t: meta.t, d: meta.d });
      }
      return bytes.byteLength;
    });
    socket.data.queue = result.catch(() => { socket.close(1011, 'Fixture compression failed'); });
    return result;
  }

  function dispatch(session: Session, event: string, data: unknown): string {
    const frame = JSON.stringify({ op: 0, t: event, s: ++session.seq, d: data });
    session.dispatches.push(frame);
    return frame;
  }

  function deliver(session: Session, event: string, data: unknown): Promise<number>[] {
    // Offline sessions retain the same dispatches for RESUME's replay.
    const frame = dispatch(session, event, data);
    return [...sockets].filter(socket => socket.data.session === session).map(socket => send(socket, frame));
  }

  function passiveVersion(session: Session, guildId?: string): number {
    if (!guildId || session.subscribedGuilds.has(guildId)) return 0;
    return session.capabilities & PASSIVE_GUILD_UPDATE_V2 ? 2
      : session.capabilities & PASSIVE_GUILD_UPDATE ? 1 : 0;
  }

  function flushReactionBatch(session: Session, key: string): Promise<number>[] {
    const batch = session.reactions.get(key);
    if (!batch) return [];
    session.reactions.delete(key);
    if (batch.adds.length === 1) return deliver(session, 'MESSAGE_REACTION_ADD', batch.adds[0]);
    const { channel_id, message_id, guild_id } = batch.adds[0];
    const reactions = new Map<string, { emoji: Emoji; users: string[] }>();
    for (const { emoji, user_id } of batch.adds) {
      const key = emoji.id ?? emoji.name;
      let reaction = reactions.get(key);
      if (!reaction) { reaction = { emoji, users: [] }; reactions.set(key, reaction); }
      if (!reaction.users.includes(user_id)) reaction.users.push(user_id);
    }
    return deliver(session, 'MESSAGE_REACTION_ADD_MANY', { channel_id, message_id,
      ...(guild_id ? { guild_id } : {}), reactions: [...reactions.values()] });
  }

  function scheduleReactions() {
    clearTimeout(reactionTimer);
    reactionTimer = undefined;
    if (fixedClock) return;
    let next = Infinity;
    for (const session of sessions.values()) for (const batch of session.reactions.values()) {
      next = Math.min(next, batch.dueAt);
    }
    if (next !== Infinity && !closed) reactionTimer = injectionDispatches.exit(() => setTimeout(
      () => flushReactions().catch(() => {}), Math.max(0, next - nowMs())));
  }

  function flushReactions(): Promise<number[]> {
    const pending: Promise<number>[] = [], now = nowMs();
    for (const session of sessions.values()) for (const [key, batch] of session.reactions) {
      if (now >= batch.dueAt) pending.push(...flushReactionBatch(session, key));
    }
    scheduleReactions();
    return Promise.all(pending);
  }

  function broadcastReactionAdds(adds: ReactionAdd[], immediate = false): Promise<number[]> {
    const pending: Promise<number>[] = [];
    for (const session of sessions.values()) {
      const keys = new Set<string>(), now = nowMs();
      for (const add of adds) {
        if (passiveVersion(session, add.guild_id)) continue;
        if (!(session.capabilities & DEBOUNCE_MESSAGE_REACTIONS)) {
          pending.push(...deliver(session, 'MESSAGE_REACTION_ADD', add));
          continue;
        }
        const key = `${add.channel_id}/${add.message_id}`;
        let batch = session.reactions.get(key);
        if (!keys.has(key) && batch && now >= batch.dueAt) {
          pending.push(...flushReactionBatch(session, key));
          batch = undefined;
        }
        if (!batch) {
          batch = { dueAt: now + reactionDebounceMs, adds: [] };
          session.reactions.set(key, batch);
        }
        batch.adds.push(add);
        keys.add(key);
      }
      if (immediate) for (const key of keys) pending.push(...flushReactionBatch(session, key));
    }
    scheduleReactions();
    return Promise.all(pending);
  }

  function broadcast(event: string, data: any, immediate = false): Promise<number[]> {
    if (event === 'MESSAGE_REACTION_ADD') {
      const pending: Promise<number>[] = [];
      if (immediate) for (const session of sessions.values()) {
        if (!passiveVersion(session, data.guild_id)) pending.push(...flushReactionBatch(session, `${data.channel_id}/${data.message_id}`));
      }
      const added = broadcastReactionAdds([data], immediate);
      return pending.length ? Promise.all([Promise.all(pending), added]).then(groups => groups.flat()) : added;
    }
    const pending: Promise<number>[] = [];
    const guildId = data.guild_id ?? world.channels.find(channel => channel.id === data.channel_id)?.guild_id;
    for (const session of sessions.values()) {
      const version = passiveVersion(session, guildId);
      if (version && event === 'MESSAGE_CREATE') {
        const channels = [{ id: data.channel_id, last_message_id: data.id, last_pin_timestamp: null }];
        const update = version === 2
          ? { guild_id: guildId, updated_channels: channels, updated_members: [],
            removed_voice_states: [], updated_voice_states: [] }
          : { guild_id: guildId, channels, members: [], voice_states: [] };
        pending.push(...deliver(session, `PASSIVE_UPDATE_V${version}`, update));
        session.passiveUpdates++;
        passiveUpdateCount++;
        continue;
      }
      if (version && (event === 'TYPING_START' || /^MESSAGE_(UPDATE|DELETE|REACTION_)/.test(event))) continue;
      // A removal must follow queued adds, or the delayed batch would restore the reaction.
      if (event === 'MESSAGE_REACTION_REMOVE' || event === 'MESSAGE_DELETE' || event === 'MESSAGE_DELETE_BULK') {
        for (const id of data.ids ?? [data.message_id ?? data.id]) {
          pending.push(...flushReactionBatch(session, `${data.channel_id}/${id}`));
        }
      }
      pending.push(...deliver(session, event, data));
    }
    scheduleReactions();
    return Promise.all(pending);
  }

  async function login(socket: Socket, capabilities: number) {
    const version = resetVersion;
    const session: Session = { id: `fixture-${seed}-${++sessionCounter}`, seq: 0, dispatches: [], capabilities,
      subscribedGuilds: new Set(options.subscribeAll ? world.guilds.map(guild => guild.id) : []),
      subscriptions: [], passiveUpdates: 0, reactions: new Map() };
    sessions.set(session.id, session);
    socket.data.session = session;
    const size = readyBytes, burst = loginBytes;
    readyBytes = 0;
    const shape = readyShape;
    readyShape = {};
    loginBytes = 0;
    const held = holdGuildsMs;
    holdGuildsMs = 0;
    const ready = sizedReady(world, session.id, gatewayUrl, session.seq + 1, size, shape);
    if (held) ready.d.guilds = world.guilds.map(guild => ({ id: guild.id, unavailable: true }));
    let sent = await send(socket, dispatch(session, 'READY', ready.d));
    if (version !== resetVersion || socket.data.closed) return;
    sent += await send(socket, dispatch(session, 'READY_SUPPLEMENTAL', {
      guilds: world.guilds.map(guild => ({ id: guild.id, voice_states: [], embedded_activities: [] })),
      merged_members: world.guilds.map(guild => guild.members),
      merged_presences: { guilds: world.guilds.map(() => []), friends: [] }, lazy_private_channels: [],
    }));
    if (world.lazyGuildId) {
      sent += await send(socket, dispatch(session, 'GUILD_CREATE', world.guilds.find(guild => guild.id === world.lazyGuildId)));
    }
    if (held) {
      setTimeout(() => {
        if (version !== resetVersion || socket.data.closed) return;
        for (const guild of world.guilds) void send(socket, dispatch(session, 'GUILD_CREATE', guild)).catch(() => {});
      }, held);
    }
    while (!socket.data.closed && sent <= burst && burst > 0) {
      const guild = syntheticGuild(world, syntheticCounter++, Math.min(200000, burst - sent + 4096));
      sent += await send(socket, dispatch(session, 'GUILD_CREATE', guild));
    }
    if (version === resetVersion && !socket.data.closed) loginsDone++;
  }

  // The first 100 rows of a guild's member list: each hoisted role's members, then the rest online, then
  // those offline (every third member), as GUILD_MEMBER_LIST_UPDATE's SYNC carries them.
  function memberList(guildId: string) {
    const guild = world.guilds.find(g => g.id === guildId);
    const members = guild?.members ?? [];
    const hoisted = (guild?.roles ?? []).filter(r => r.hoist).sort((a, b) => b.position - a.position);
    const status = (i: number) => i % 3 === 2 ? 'offline' : i % 4 === 1 ? 'idle' : 'online';
    const groups: { id: string; members: typeof members }[] = [];
    const placed = new Set<string>();
    for (const role of hoisted) {
      const holders = members.filter((m, i) => status(i) !== 'offline' && !placed.has(m.user.id) && m.roles.includes(role.id));
      holders.forEach(m => placed.add(m.user.id));
      if (holders.length) groups.push({ id: role.id, members: holders });
    }
    groups.push({ id: 'online', members: members.filter((m, i) => status(i) !== 'offline' && !placed.has(m.user.id)) });
    groups.push({ id: 'offline', members: members.filter((_, i) => status(i) === 'offline') });
    const items = groups.filter(g => g.members.length).flatMap(g => [{ group: { id: g.id, count: g.members.length } },
      ...g.members.map(m => ({ member: { ...m, presence: { user: { id: m.user.id }, status: status(members.indexOf(m)), activities: [] } } }))]).slice(0, 100);
    return { guild_id: guildId, id: 'everyone', member_count: members.length,
      online_count: members.filter((_, i) => status(i) !== 'offline').length,
      groups: groups.map(g => ({ id: g.id, count: g.members.length })), ops: [{ op: 'SYNC', range: [0, 99], items }] };
  }

  async function receive(socket: Socket, raw: string | Buffer) {
    if (socket.data.closed) return;
    let frame: any;
    try { frame = JSON.parse(String(raw)); } catch { socket.close(4002, 'Invalid JSON'); return; }
    if (frame?.op === 1) {
      if (!stopAcks) await send(socket, { op: 11, d: null });
      return;
    }
    if (frame?.op === 14 || frame?.op === 37) {
      const session = socket.data.session;
      if (!session) return;
      const guildIds = typeof frame.d?.guild_id === 'string' ? [frame.d.guild_id]
        : frame.d?.subscriptions && typeof frame.d.subscriptions === 'object'
          ? Object.keys(frame.d.subscriptions) : [];
      for (const id of guildIds) session.subscribedGuilds.add(id);
      session.subscriptions.push({ op: frame.op, guild_ids: guildIds });
      // A channel's member list, as Discord answers a subscription that names a range of it.
      for (const id of guildIds) {
        if (frame.op === 37 && frame.d.subscriptions[id]?.channels) await Promise.all(deliver(session, 'GUILD_MEMBER_LIST_UPDATE', memberList(id)));
      }
      return;
    }
    if (frame?.op !== 2 && frame?.op !== 6) return;
    if (frame.op === 2) identifyToken = frame.d?.token ?? null;
    if (frame.d?.token !== world.token) { socket.close(4004, 'Authentication failed'); return; }
    if (socket.data.session) { socket.close(4005, 'Already authenticated'); return; }
    if (frame.op === 2) {
      if (options.resetOnIdentify) reset(socket);
      identifyCount++;
      await login(socket, Number.isSafeInteger(frame.d.capabilities) ? frame.d.capabilities : 0);
      return;
    }
    resumeCount++;
    const session = sessions.get(frame.d.session_id);
    if (!session) { await send(socket, { op: 9, d: false }); return; }
    const seq = frame.d.seq;
    if (!Number.isInteger(seq) || seq < 0 || seq > session.seq) {
      socket.close(4007, 'Invalid sequence'); return;
    }
    socket.data.session = session;
    // Queue the complete replay before live broadcasts can join the socket.
    const replay = session.dispatches.slice(seq).map(frame => send(socket, frame));
    replay.push(send(socket, dispatch(session, 'RESUMED', { _trace: [] })));
    await Promise.all(replay);
  }

  function channel(id: string): Channel {
    const found = world.channels.find(value => value.id === id && value.type !== 4);
    if (!found) throw new HttpError(404, 'Unknown Channel', 10003);
    return found;
  }

  function message(channelId: string, id: string): Message {
    channel(channelId);
    const found = world.messages[channelId].find(value => value.id === id);
    if (!found) throw new HttpError(404, 'Unknown Message', 10008);
    return found;
  }

  function createMessage(channelId: string, content: string, authorId = world.me.id, body: any = {}, at?: number): Message {
    const destination = channel(channelId);
    const author = world.users.find(user => user.id === authorId);
    if (!author) throw new HttpError(400, 'Unknown author');
    const reference = body.message_reference;
    const earlier = reference ? message(channelId, textField(reference, 'message_id')) : undefined;
    if (reference?.channel_id && reference.channel_id !== channelId) throw new HttpError(400, 'Reply must be in this channel');
    const { ms, id } = nextMessage(at);
    const created = emptyMessage(destination, author, ms, content);
    created.id = id;
    if (body.nonce !== undefined) created.nonce = body.nonce;
    if (earlier) {
      created.type = 19;
      created.message_reference = { message_id: earlier.id, channel_id: channelId,
        ...(destination.guild_id ? { guild_id: destination.guild_id } : {}) };
      created.referenced_message = replySnapshot(earlier);
    }
    if (Array.isArray(body.attachments)) created.attachments = body.attachments.map((a: any, i: number) => {
      const held = uploads.get(textField(a, 'uploaded_filename'));
      if (!held?.bytes) throw new HttpError(400, 'Attachment was not uploaded', 50035);
      const url = `${origin}/uploads/${encodeURIComponent(textField(a, 'uploaded_filename'))}`;
      const kind = held.filename.split('.').pop()?.toLowerCase() ?? '';
      const content_type = { png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif', webp: 'image/webp' }[kind]
        ?? 'application/octet-stream';
      return { id: snowflakeFromMs(ms, 200000 + i), filename: held.filename, size: held.bytes.length, url, proxy_url: url,
        content_type, ...imageSize(held.bytes) };
    });
    created.mentions = world.users.filter(user => content.includes(`<@${user.id}>`) || content.includes(`<@!${user.id}>`));
    created.mention_roles = world.guilds.flatMap(guild => guild.roles)
      .filter(role => content.includes(`<@&${role.id}>`)).map(role => role.id);
    created.mention_everyone = /@(?:everyone|here)\b/.test(content);
    const history = world.messages[channelId];
    const later = history.findIndex(message => BigInt(message.id) > BigInt(created.id));
    history.splice(later < 0 ? history.length : later, 0, created);
    destination.last_message_id = history.at(-1)!.id;
    typingUsers.get(channelId)?.delete(authorId);
    return created;
  }

  async function edit(channelId: string, id: string, content: string) {
    const current = message(channelId, id);
    current.content = content;
    current.edited_timestamp = new Date(nowMs()).toISOString();
    await broadcast('MESSAGE_UPDATE', { id, channel_id: channelId, guild_id: current.guild_id,
      content, edited_timestamp: current.edited_timestamp });
    return current;
  }

  async function remove(channelId: string, ids: string[], bulk: boolean) {
    const destination = channel(channelId);
    const deleted = new Set(ids);
    world.messages[channelId] = world.messages[channelId].filter(value => !deleted.has(value.id));
    destination.last_message_id = world.messages[channelId].at(-1)?.id ?? null;
    await broadcast(bulk ? 'MESSAGE_DELETE_BULK' : 'MESSAGE_DELETE', {
      channel_id: channelId, ...(destination.guild_id ? { guild_id: destination.guild_id } : {}),
      ...(bulk ? { ids } : { id: ids[0] }),
    });
  }

  function parseEmoji(value: unknown): Emoji {
    if (typeof value === 'object' && value !== null) {
      const emoji = value as Emoji;
      if (typeof emoji.name !== 'string' || (emoji.id !== null && typeof emoji.id !== 'string')) {
        throw new HttpError(400, 'Invalid emoji');
      }
      return { id: emoji.id, name: emoji.name };
    }
    if (typeof value !== 'string' || !value) throw new HttpError(400, 'Invalid emoji');
    const custom = /^(.*):(\d+)$/.exec(value);
    return { id: custom?.[2] ?? null, name: custom?.[1] ?? value };
  }

  function changeReaction(channelId: string, id: string, emoji: Emoji, userId: string, add: boolean, repeat = false): ReactionAdd | undefined {
    const current = message(channelId, id);
    if (!world.users.some(user => user.id === userId)) throw new HttpError(400, 'Unknown user');
    let reaction = current.reactions.find(value => value.emoji.id === emoji.id && value.emoji.name === emoji.name);
    const key = `${channelId}/${id}/${emoji.id ?? emoji.name}`;
    let users = reactionUsers.get(key);
    if (!users) {
      users = new Set(reaction?.me ? [world.me.id] : []);
      if (reaction) for (const user of world.users) {
        if (users.size >= reaction.count) break;
        if (user.id !== world.me.id) users.add(user.id);
      }
      reactionUsers.set(key, users);
    }
    if (users.has(userId) === add && !repeat) return;
    if (add) users.add(userId); else users.delete(userId);
    if (!reaction) {
      reaction = { emoji, count: 0, me: false, count_details: { burst: 0, normal: 0 },
        burst_count: 0, me_burst: false, burst_colors: [] };
      current.reactions.push(reaction);
    }
    // Seeded counts can include more people than the fixture's named users.
    reaction.count += add ? 1 : -1;
    reaction.count_details.normal = reaction.count;
    reaction.me = users.has(world.me.id);
    if (!reaction.count) current.reactions.splice(current.reactions.indexOf(reaction), 1);
    return {
      channel_id: channelId, message_id: id, user_id: userId, emoji,
      ...(current.guild_id ? { guild_id: current.guild_id } : {}), burst: false, type: 0,
    };
  }

  async function react(channelId: string, id: string, emoji: Emoji, userId: string, add: boolean) {
    const changed = changeReaction(channelId, id, emoji, userId, add);
    if (changed) await broadcast(add ? 'MESSAGE_REACTION_ADD' : 'MESSAGE_REACTION_REMOVE', changed);
  }

  // One voter's vote on one answer, which Discord sends as its own event. Only the reader's choice is kept.
  async function pollVote(channelId: string, messageId: string, userId: string, answerId: number, add: boolean) {
    const poll = (message(channelId, messageId) as { poll?: any }).poll;
    if (!poll?.answers.some((a: { answer_id: number }) => a.answer_id === answerId)) throw new HttpError(400, 'Not an answer');
    poll.results ??= { is_finalized: false, answer_counts: [] };
    let count = poll.results.answer_counts.find((a: { id: number }) => a.id === answerId);
    if (!count) poll.results.answer_counts.push(count = { id: answerId, count: 0, me_voted: false });
    count.count = Math.max(0, count.count + (add ? 1 : -1));
    if (userId === world.me.id) count.me_voted = add;
    const guildId = channel(channelId).guild_id;
    await broadcast(add ? 'MESSAGE_POLL_VOTE_ADD' : 'MESSAGE_POLL_VOTE_REMOVE', { user_id: userId, channel_id: channelId,
      message_id: messageId, answer_id: answerId, ...(guildId ? { guild_id: guildId } : {}) });
  }

  async function typing(channelId: string, userId: string) {
    const destination = channel(channelId);
    const user = world.users.find(value => value.id === userId);
    if (!user) throw new HttpError(400, 'Unknown user');
    if (!typingUsers.has(channelId)) typingUsers.set(channelId, new Set());
    typingUsers.get(channelId)!.add(userId);
    await broadcast('TYPING_START', { channel_id: channelId, user_id: userId,
      timestamp: Math.floor(nowMs() / 1000), ...(destination.guild_id
        ? { guild_id: destination.guild_id, member: world.guilds.find(guild => guild.id === destination.guild_id)
          ?.members.find(member => member.user.id === userId) } : {}) });
  }

  function limitRequest(method: string, path: string, headers: Headers): ForcedLimit | undefined {
    const route = rateRoute(method, path);
    const rule = options.rateLimits?.routes?.[route] ?? (/^POST \/channels\/[^/]+\/messages$/.test(route)
      ? options.rateLimits?.messages ?? { limit: 5, windowMs: 5000 }
      : options.rateLimits?.default ?? { limit: 1000, windowMs: 5000 });
    const now = nowMs();
    let bucket = buckets.get(route);
    if (!bucket || now >= bucket.reset) {
      bucket = { ...rule, used: 0, reset: now + rule.windowMs };
      buckets.set(route, bucket);
    }
    const inject = forced.find(value => value.count > 0 && (value.global || !value.route
      || value.route === route || value.route === path || value.route === path.replace(/^\/api\/v9/, '')));
    if (inject) inject.count--;
    const retry = inject ?? (bucket.used >= bucket.limit
      ? { global: false, count: 1, retry_after: Math.max(0, (bucket.reset - now) / 1000) } : undefined);
    if (!retry) bucket.used++;
    headers.set('X-RateLimit-Limit', String(bucket.limit));
    headers.set('X-RateLimit-Remaining', String(retry ? 0 : Math.max(0, bucket.limit - bucket.used)));
    headers.set('X-RateLimit-Reset', String(retry ? now / 1000 + retry.retry_after : bucket.reset / 1000));
    headers.set('X-RateLimit-Reset-After', String(retry?.retry_after ?? Math.max(0, (bucket.reset - now) / 1000)));
    // The bucket hash omits the major parameter; channels remain independent.
    headers.set('X-RateLimit-Bucket', `fixture-${route.replace(/\/channels\/\d+/, '/channels/:channel_id')}`);
    if (retry) {
      headers.set('Retry-After', String(retry.retry_after));
      headers.set('X-RateLimit-Scope', retry.global ? 'global' : 'user');
      if (retry.global) headers.set('X-RateLimit-Global', 'true');
    }
    return retry;
  }

  function state() {
    return { connections: sockets.size, sessions: sessions.size, identify_count: identifyCount, resume_count: resumeCount,
      identify_token: identifyToken,
      logins_done: loginsDone,
      connection_details: [...sockets].map(socket => ({ id: socket.data.id,
        session_id: socket.data.session?.id ?? null, compressed: socket.data.compressed })),
      session_details: [...sessions.values()].map(session => ({ id: session.id, seq: session.seq,
        dispatches: session.dispatches.length, capabilities: session.capabilities,
        subscribed_guilds: [...session.subscribedGuilds], subscriptions: session.subscriptions,
        passive_updates: session.passiveUpdates, pending_reaction_batches: session.reactions.size })),
      requests: [...requests], frames: [...frames], passive_updates: passiveUpdateCount,
      now_ms: nowMs(), clock_offset_ms: clockOffsetMs,
      nonce_window_ms: nonceWindowMs, reaction_debounce_ms: reactionDebounceMs,
      stop_acks: stopAcks, ready_bytes: readyBytes, login_bytes: loginBytes, split_parts: frameParts };
  }

  function reset(keep?: Socket) {
    resetVersion++;
    for (const [timer, resolve] of burstTimers) { clearTimeout(timer); resolve(); }
    burstTimers.clear();
    // A client the reset superseded is told to stop (4004 is fatal to it), so it cannot
    // reconnect and reset the world under the session that replaced it.
    for (const socket of sockets) if (socket !== keep) {
      socket.data.closed = true; socket.close(keep ? 4004 : 1000, 'Fixture reset');
    }
    sockets.clear(); sessions.clear(); buckets.clear(); reactionUsers.clear();
    typingUsers.clear(); failNext.clear(); releaseSends(); releaseMedia();
    clearTimeout(reactionTimer); reactionTimer = undefined;
    clockOffsetMs = messageTimestamp = messageSequence = passiveUpdateCount = 0;
    forced.length = requests.length = frames.length = 0;
    connectionCounter = sessionCounter = identifyCount = resumeCount = loginsDone = 0;
    readyBytes = loginBytes = 0; readyShape = {}; frameParts = 1; stopAcks = false; syntheticCounter = 10000;
    // An IDENTIFY's reset (`keep`) leaves the guild hold for that login; an explicit reset drops it.
    if (!keep) holdGuildsMs = 0;
    world = buildWorld();
    if (rotatedToken) world.token = rotatedToken;
    if (keep) {
      keep.data.session = undefined;
      keep.data.id = ++connectionCounter;
      sockets.add(keep);
    }
  }

  async function control(name: string, body: any) {
    switch (name) {
      case 'inject': {
        const channelId = textField(body, 'channel_id');
        channel(channelId);
        const at = integer(body.at_ms, nowMs(), 8640000000000000 - 2500);
        if (at < Date.UTC(2015, 0, 1)) throw new HttpError(400, 'Invalid message time');
        const request = { channel_id: channelId, what: textField(body, 'what'),
          near: body.near === undefined ? '' : textField(body, 'near'), at_ms: at };
        const version = resetVersion;
        const dispatches: InjectedDispatch[] = [];
        await injectionDispatches.run(dispatches, () => inject(world, request, {
          origin, typingUsers, failNext, remove, typing, broadcast,
          holdGuilds: (ms: number) => { holdGuildsMs = ms; },
          isCurrent: () => version === resetVersion && !closed,
          create: (channelId, content, authorId, at) => createMessage(channelId, content, authorId, {}, at),
          react: async (channelId, id) => {
            // Each menu click counts another add; the stand-in does not deduplicate reactors.
            const added = changeReaction(channelId, id, { id: null, name: '✨' }, world.users[1].id, true, true);
            if (added) await broadcast('MESSAGE_REACTION_ADD', added, true);
          },
          badRequest: message => { throw new HttpError(400, message); },
        }));
        return { ok: true, dispatches };
      }
      case 'hold-sends':
        if (typeof body.on !== 'boolean') throw new HttpError(400, 'on must be a boolean');
        if (body.on) holdSends = true; else releaseSends();
        break;
      case 'hold-media':
        if (typeof body.on !== 'boolean') throw new HttpError(400, 'on must be a boolean');
        if (body.on) holdMedia = true; else releaseMedia();
        break;
      case 'clock':
        clockOffsetMs += integer(body.advance_ms, 0, 8640000000000000 - nowMs());
        await flushReactions();
        return { ok: true, now_ms: nowMs(), clock_offset_ms: clockOffsetMs };
      case 'subscribe': {
        const guildId = textField(body, 'guild_id');
        for (const session of sessions.values()) session.subscribedGuilds.add(guildId);
        break;
      }
      case 'burst': {
        const channelId = textField(body, 'channel_id');
        const destination = channel(channelId);
        if (body.author_id !== undefined && !world.users.some(user => user.id === body.author_id)) {
          throw new HttpError(400, 'Unknown author');
        }
        const count = integer(body.count, 1, 100000), interval = integer(body.interval_ms, 0, 60000);
        const generate = createMessageGenerator({ channel: destination, users: world.users, me: world.me,
          roleId: world.guilds.find(guild => guild.id === destination.guild_id)?.roles[1].id,
          assetOrigin: origin, random: seededRandom(integer(body.seed, seed, 4294967295)) });
        const version = resetVersion;
        const ids: string[] = [];
        for (let i = 0; i < count && !closed && version === resetVersion; i++) {
          if (i && interval) {
            let elapsed = false;
            await new Promise<void>(resolve => {
              const timer = setTimeout(() => { burstTimers.delete(timer); elapsed = true; resolve(); }, interval);
              burstTimers.set(timer, resolve);
            });
            if (!elapsed) break;
          }
          const history = world.messages[channelId];
          const { ms, id } = nextMessage();
          const created = generate(history, ms, body.author_id);
          created.id = id;
          history.push(created);
          destination.last_message_id = created.id;
          typingUsers.get(channelId)?.delete(created.author.id);
          ids.push(created.id);
          await broadcast('MESSAGE_CREATE', created);
        }
        return { count: ids.length, ids };
      }
      case 'edit': return edit(textField(body, 'channel_id'), textField(body, 'message_id'), textField(body, 'content'));
      case 'delete':
        message(textField(body, 'channel_id'), textField(body, 'message_id'));
        await remove(body.channel_id, [body.message_id], false); break;
      case 'delete-bulk':
        if (!Array.isArray(body.ids) || body.ids.some((id: unknown) => typeof id !== 'string')) throw new HttpError(400, 'ids must be strings');
        await remove(textField(body, 'channel_id'), body.ids, true); break;
      case 'react':
        if (typeof body.add !== 'boolean') throw new HttpError(400, 'add must be a boolean');
        await react(textField(body, 'channel_id'), textField(body, 'message_id'), parseEmoji(body.emoji),
          textField(body, 'user_id'), body.add); break;
      case 'vote':
        if (typeof body.add !== 'boolean' || !Number.isInteger(body.answer_id)) throw new HttpError(400, 'add must be a boolean, answer_id an integer');
        await pollVote(textField(body, 'channel_id'), textField(body, 'message_id'), textField(body, 'user_id'), body.answer_id, body.add); break;
      case 'react-many': {
        const channelId = textField(body, 'channel_id'), id = textField(body, 'message_id');
        const emoji = parseEmoji(body.emoji);
        message(channelId, id);
        if (!Array.isArray(body.user_ids) || body.user_ids.some((id: unknown) => typeof id !== 'string'
          || !world.users.some(user => user.id === id))) throw new HttpError(400, 'user_ids must name known users');
        const adds: ReactionAdd[] = [];
        for (const userId of body.user_ids) {
          const changed = changeReaction(channelId, id, emoji, userId, true);
          if (changed) adds.push(changed);
        }
        await broadcastReactionAdds(adds, true);
        break;
      }
      case 'typing': await typing(textField(body, 'channel_id'), textField(body, 'user_id')); break;
      case 'heartbeat-request': await Promise.all([...sockets].map(socket => send(socket, { op: 1, d: null }))); break;
      case 'reconnect': await Promise.all([...sockets].map(socket => send(socket, { op: 7, d: null }))); break;
      case 'invalidate': {
        const resumable = body.resumable === true;
        await Promise.all([...sockets].map(socket => send(socket, { op: 9, d: resumable })));
        if (!resumable) {
          sessions.clear();
          scheduleReactions();
          for (const socket of sockets) socket.data.session = undefined;
        }
        break;
      }
      case 'close': {
        const code = integer(body.code, 4000, 4999);
        if (!(code >= 3000 || [1000, 1001, 1002, 1003, 1007, 1008, 1009, 1011, 1012, 1013].includes(code))) {
          throw new HttpError(400, 'Invalid WebSocket close code');
        }
        for (const socket of sockets) socket.close(code, 'Fixture control');
        break;
      }
      case 'stop-acks': stopAcks = body.on === true; break;
      case 'rotate-token': {
        const token = textField(body, 'token');
        world.token = rotatedToken = token;
        await Promise.all([...sessions.values()].flatMap(session =>
          deliver(session, 'READY', { ...readyPayload(world, session.id, gatewayUrl), auth_token: token })));
        break;
      }
      case 'ready-size':
        readyBytes = integer(body.bytes, 3 * 1024 * 1024, 64 * 1024 * 1024);
        readyShape = { roles: integer(body.roles ?? 1, 1, 1000), channels: integer(body.channels ?? 8, 1, 500),
          overwrites: integer(body.overwrites ?? 0, 0, 100), dms: integer(body.dms ?? 0, 0, 10000) };
        break;
      case 'login-burst': loginBytes = integer(body.bytes, 1024 * 1024 + 4096, 64 * 1024 * 1024); break;
      case 'oversize-frame': {
        const bytes = integer(body.bytes, 1024 * 1024 + 4096, 64 * 1024 * 1024);
        const guild = syntheticGuild(world, syntheticCounter++, bytes * 2);
        const sizes = await broadcast('GUILD_CREATE', guild);
        return { wire_bytes: sizes };
      }
      case 'split-frames': frameParts = Math.max(1, integer(body.parts, 1, 1024)); break;
      case 'rate-limit': {
        const retry = Number(body.retry_after ?? 1);
        if (!Number.isFinite(retry) || retry < 0) throw new HttpError(400, 'Invalid retry_after');
        forced.push({ route: body.route, global: body.global === true,
          count: integer(body.count, 1, 100000), retry_after: retry });
        break;
      }
      case 'reset':
        reset();
        break;
      default: throw new HttpError(404, 'Unknown control endpoint');
    }
    return { ok: true };
  }

  async function rest(request: Request, url: URL): Promise<{ status: number; body?: unknown }> {
    const path = url.pathname.slice('/api/v9'.length), method = request.method;
    if (method === 'GET' && path === '/gateway') return { status: 200, body: { url: gatewayUrl } };
    if (method === 'GET' && path === '/users/@me') return { status: 200, body: world.me };
    const slots = /^\/channels\/([^/]+)\/attachments$/.exec(path);
    if (slots && method === 'POST') {
      channel(slots[1]);
      const files = (await readBody(request)).files;
      if (!Array.isArray(files) || !files.length || files.length > 10) throw new HttpError(400, 'files must list 1 to 10 files');
      return { status: 200, body: { attachments: files.map((f: any) => {
        const filename = textField(f, 'filename');
        if (!/^[^/\\]{1,200}$/.test(filename)) throw new HttpError(400, 'Invalid filename');
        const name = `${String(++uploadCount).padStart(8, '0')}/${filename}`;
        uploads.set(name, { filename, bytes: null });
        return { id: f.id, upload_url: `${origin}/upload/${encodeURIComponent(name)}`, upload_filename: name };
      }) } };
    }
    // A vote replaces the reader's earlier one; no answers takes it back. Discord sends an event an answer.
    const vote = /^\/channels\/([^/]+)\/polls\/([^/]+)\/answers\/@me$/.exec(path);
    if (vote && method === 'PUT') {
      const [, channelId, messageId] = vote;
      const poll = (message(channelId, messageId) as { poll?: any }).poll;
      if (!poll) throw new HttpError(400, 'Not a poll');
      const chosen = (await readBody(request)).answer_ids;
      const answers = poll.answers.map((a: { answer_id: number }) => String(a.answer_id));
      if (!Array.isArray(chosen) || chosen.some(id => typeof id !== 'string' || !answers.includes(id))
        || (!poll.allow_multiselect && chosen.length > 1)) throw new HttpError(400, 'Invalid answers');
      for (const id of answers) {
        const mine = !!poll.results?.answer_counts.find((a: { id: number }) => String(a.id) === id)?.me_voted;
        if (mine !== chosen.includes(id)) await pollVote(channelId, messageId, world.me.id, Number(id), !mine);
      }
      return { status: 204 };
    }
    const match = /^\/channels\/([^/]+)\/(messages|typing)(?:\/([^/]+)(?:\/(ack|reactions)(?:\/([^/]+)\/@me)?)?)?$/.exec(path);
    if (!match) throw new HttpError(404, 'Unknown endpoint');
    const [, channelId, kind, messageId, action, encodedEmoji] = match;
    channel(channelId);
    if (kind === 'typing' && !messageId && method === 'POST') {
      await typing(channelId, world.me.id); return { status: 204 };
    }
    if (kind !== 'messages') throw new HttpError(404, 'Unknown endpoint');
    if (!messageId && method === 'GET') {
      const query: PageQuery = {};
      for (const name of ['before', 'after', 'around'] as const) {
        const value = url.searchParams.get(name);
        if (value !== null) {
          if (!/^\d+$/.test(value)) throw new HttpError(400, 'Invalid message cursor');
          query[name] = value;
        }
      }
      if (url.searchParams.has('limit')) {
        const limit = Number(url.searchParams.get('limit'));
        if (!Number.isFinite(limit)) throw new HttpError(400, 'Invalid limit');
        query.limit = limit;
      }
      if ([query.before, query.after, query.around].filter(Boolean).length > 1) throw new HttpError(400, 'Conflicting cursors');
      if (query.before && options.historyDelayMs) await new Promise(resume => setTimeout(resume, options.historyDelayMs));
      return { status: 200, body: pageMessages(world.messages[channelId], query) };
    }
    if (!messageId && method === 'POST') {
      if (holdSends) await new Promise<void>(resume => heldSends.push(resume));
      if (failNext.delete(channelId)) throw new HttpError(500, 'Fixture failure');
      const body = await readBody(request), content = textField(body, 'content');
      if (!content.trim() && !(Array.isArray(body.attachments) && body.attachments.length)) throw new HttpError(400, 'Cannot send an empty message', 50006);
      if (body.nonce !== undefined && !['string', 'number'].includes(typeof body.nonce)) throw new HttpError(400, 'Invalid nonce');
      if (body.enforce_nonce && body.nonce !== undefined) {
        const now = nowMs();
        for (const history of Object.values(world.messages)) {
          const previous = recentNonceMessage(history, world.me.id, body.nonce, now, nonceWindowMs);
          if (previous) return { status: 200, body: previous };
        }
      }
      const created = createMessage(channelId, content, world.me.id, body);
      await broadcast('MESSAGE_CREATE', created);
      return { status: 200, body: created };
    }
    if (!messageId) throw new HttpError(404, 'Unknown endpoint');
    const current = message(channelId, messageId);
    if (!action && (method === 'PATCH' || method === 'DELETE')) {
      if (current.author.id !== world.me.id) throw new HttpError(403, 'Cannot edit or delete another user\'s message', 50005);
      if (method === 'PATCH') return { status: 200, body: await edit(channelId, messageId, textField(await readBody(request), 'content')) };
      await remove(channelId, [messageId], false); return { status: 204 };
    }
    if (action === 'reactions' && encodedEmoji && (method === 'PUT' || method === 'DELETE')) {
      let decoded: string;
      try { decoded = decodeURIComponent(encodedEmoji); } catch { throw new HttpError(400, 'Invalid emoji encoding'); }
      await react(channelId, messageId, parseEmoji(decoded), world.me.id, method === 'PUT');
      return { status: 204 };
    }
    if (action === 'ack' && !encodedEmoji && method === 'POST') {
      const entry = world.read_state.entries.find(value => value.id === channelId)!;
      entry.last_message_id = messageId; entry.mention_count = 0; world.read_state.version++;
      await broadcast('MESSAGE_ACK', { channel_id: channelId, message_id: messageId,
        mention_count: 0, version: world.read_state.version, manual: false });
      return { status: 200, body: { token: null } };
    }
    throw new HttpError(404, 'Unknown endpoint');
  }

  async function readBody(request: Request) {
    let body: any;
    try { body = await request.json(); } catch { throw new HttpError(400, 'Invalid JSON body'); }
    if (!body || typeof body !== 'object' || Array.isArray(body)) throw new HttpError(400, 'Expected a JSON object');
    return body;
  }

  const server = Bun.serve<Connection>({
    port: options.port ?? 4790, hostname,
    async fetch(request, server) {
      const url = new URL(request.url);
      requests.push({ method: request.method, path: url.pathname + url.search,
        headers: Object.fromEntries(selectedHeaders.flatMap(name => {
          const value = request.headers.get(name); return value === null ? [] : [[name, value]];
        })) });
      if (request.headers.get('upgrade')?.toLowerCase() === 'websocket') {
        const data: Connection = { id: ++connectionCounter, compressed: url.searchParams.get('compress') === 'zlib-stream',
          queue: Promise.resolve(), closed: false };
        if (data.compressed) data.encoder = createZlibFramer();
        if (server.upgrade(request, { data })) return;
        await data.encoder?.close();
        return new Response('WebSocket upgrade failed', { status: 400 });
      }
      const headers = new Headers({ 'Access-Control-Allow-Origin': request.headers.get('Origin') ?? '*',
        'Access-Control-Allow-Methods': 'GET, POST, PATCH, PUT, DELETE, OPTIONS',
        'Access-Control-Allow-Headers': allowedHeaders, 'Access-Control-Expose-Headers': rateHeaderNames.join(', '),
        'Vary': 'Origin', 'Cache-Control': 'no-store' });
      const json = (body: unknown, status = 200) => Response.json(body, { status, headers });
      if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers });
      const isRest = url.pathname === '/api/v9' || url.pathname.startsWith('/api/v9/');
      const failingSend = request.method === 'POST' && /^\/api\/v9\/channels\/([^/]+)\/messages$/.exec(url.pathname);
      const retry = isRest && !(failingSend && failNext.has(failingSend[1]))
        ? limitRequest(request.method, url.pathname, headers) : undefined;
      if (request.method === 'GET' && (url.pathname.startsWith('/media/') || url.pathname.startsWith('/emojis/') || url.pathname.startsWith('/stickers/'))) {
        const media = /^\/media\/([a-z]+)\.png$/.exec(url.pathname);
        const emoji = /^\/emojis\/(50[0-8])\.png$/.exec(url.pathname);
        // The world's one sticker (601) is drawn with an emoji's picture.
        const sticker = /^\/stickers\/601\.png$/.test(url.pathname);
        const name = media?.[1] ?? (emoji ? emojiNames[Number(emoji[1]) - 500] : sticker ? emojiNames[1] : undefined);
        if (!name || !/^[a-z]+$/.test(name)) return json({ message: 'Unknown asset', code: 0 }, 404);
        if (media && holdMedia && !url.search) await new Promise<void>(resume => heldMedia.push(resume));
        const file = Bun.file(`${import.meta.dir}/../assets/${media ? 'media' : 'emoji'}/${name}.png`);
        if (!await file.exists()) return json({ message: 'Unknown asset', code: 0 }, 404);
        headers.set('Content-Type', 'image/png');
        return new Response(file, { headers });
      }
      const upload = /^\/(upload|uploads)\/([^/]+)$/.exec(url.pathname);
      if (upload) {
        let name: string;
        try { name = decodeURIComponent(upload[2]); } catch { return json({ message: 'Unknown upload', code: 0 }, 404); }
        const held = uploads.get(name);
        if (!held) return json({ message: 'Unknown upload', code: 0 }, 404);
        if (upload[1] === 'upload' && request.method === 'PUT') {
          // The bucket takes the bytes alone: a Discord credential sent here would be a leak.
          if (request.headers.has('Authorization')) return json({ message: 'Credentials sent to the upload bucket', code: 0 }, 400);
          held.bytes = new Uint8Array(await request.arrayBuffer());
          return new Response(null, { status: 200, headers });
        }
        if (upload[1] === 'uploads' && request.method === 'GET' && held.bytes) return new Response(held.bytes, { headers });
        return json({ message: 'Unknown upload', code: 0 }, 404);
      }
      const avatar = request.method === 'GET' ? avatarPng(url.pathname) : null;
      if (avatar) {
        headers.set('Content-Type', 'image/png');
        return new Response(avatar, { headers });
      }
      const dimensions = /^\/assets\/([1-9]\d{0,4})x([1-9]\d{0,4})\.svg$/.exec(url.pathname);
      if (dimensions) {
        headers.set('Content-Type', 'image/svg+xml');
        return new Response(imageArtwork(Number(dimensions[1]), Number(dimensions[2])), { headers });
      }
      const filename = url.pathname.slice('/assets/'.length);
      if (url.pathname.startsWith('/assets/') && Object.hasOwn(fileAssets, filename)) {
        const file = fileAssets[filename];
        headers.set('Content-Type', file.content_type);
        return new Response(file.body, { headers });
      }
      try {
        if (url.pathname === '/control/state' && request.method === 'GET') return json(state());
        if (url.pathname.startsWith('/control/') && request.method === 'POST') {
          const text = await request.text();
          let body: any;
          try { body = text ? JSON.parse(text) : {}; } catch { throw new HttpError(400, 'Invalid JSON body'); }
          if (!body || typeof body !== 'object' || Array.isArray(body)) throw new HttpError(400, 'Expected a JSON object');
          return json(await control(url.pathname.slice('/control/'.length), body));
        }
        if (!isRest) throw new HttpError(404, 'Unknown endpoint');
        if (request.headers.get('Authorization') !== world.token) throw new HttpError(401, '401: Unauthorized');
        if (retry) return json({ message: 'You are being rate limited.', retry_after: retry.retry_after, global: retry.global }, 429);
        const result = await rest(request, url);
        return result.status === 204 ? new Response(null, { status: 204, headers }) : json(result.body, result.status);
      } catch (error) {
        if (error instanceof HttpError) return json({ message: error.message, code: error.code }, error.status);
        console.error('Fixture request failed:', error instanceof Error ? error.message : 'unknown error');
        return json({ message: 'Fixture request failed' }, 500);
      }
    },
    websocket: {
      idleTimeout: 0, maxPayloadLength: 128 * 1024 * 1024, backpressureLimit: 128 * 1024 * 1024,
      open(socket) {
        sockets.add(socket);
        void send(socket, { op: 10, d: { heartbeat_interval: heartbeatInterval } }).catch(() => {});
      },
      message(socket, message) {
        return receive(socket, message).catch(() => { socket.close(1011, 'Fixture gateway failed'); });
      },
      close(socket, code) {
        socket.data.closed = true;
        sockets.delete(socket);
        if ([1000, 1001, 4007, 4009].includes(code) && socket.data.session) {
          if (sessions.get(socket.data.session.id) === socket.data.session) sessions.delete(socket.data.session.id);
          scheduleReactions();
        }
        void socket.data.encoder?.close();
      },
    },
  });
  const host = hostname.includes(':') ? `[${hostname}]` : hostname;
  origin = `http://${host}:${server.port}`;
  gatewayUrl = `ws://${host}:${server.port}/`;
  world = buildWorld();
  return {
    origin, gatewayUrl, control: `${origin}/control`, get world() { return world; },
    async close() {
      closed = true;
      clearTimeout(reactionTimer); reactionTimer = undefined;
      for (const [timer, resolve] of burstTimers) { clearTimeout(timer); resolve(); }
      burstTimers.clear();
      const encoders = [...sockets].map(socket => {
        socket.data.closed = true; socket.close(1001, 'Fixture stopped'); return socket.data.encoder?.close();
      });
      await server.stop(true);
      await Promise.all(encoders);
      sockets.clear(); sessions.clear();
    },
  };
}

if (import.meta.main) {
  const options: FixtureOptions = {};
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    const flag = args[i];
    if (flag === '--reset-on-identify') { options.resetOnIdentify = true; continue; }
    if (!['--port', '--seed', '--world', '--clock', '--host', '--history-delay'].includes(flag)) throw new Error(`Unknown option: ${flag}`);
    if (args[i + 1] === undefined) throw new Error(`Missing value for ${flag}`);
    // The address it listens on and names itself by (READY's gateway, asset URLs): a phone's, this Mac's LAN address.
    if (flag === '--host') { options.hostname = args[++i]; continue; }
    if (flag === '--world') {
      const world = args[++i];
      if (world !== 'default' && world !== 'transcript') throw new Error('world must be default or transcript');
      options.world = world; continue;
    }
    if (flag === '--history-delay') { options.historyDelayMs = integer(args[++i], 0, 60000); continue; }
    if (flag === '--clock') {
      const clock = args[++i];
      if (clock !== 'fixed' && clock !== 'wall') throw new Error('clock must be fixed or wall');
      options.clock = clock; continue;
    }
    const value = integer(args[++i], 0, flag === '--port' ? 65535 : 4294967295);
    if (flag === '--port') options.port = value; else options.seed = value;
  }
  if (options.port === undefined && process.env.PORT !== undefined) options.port = integer(process.env.PORT, 4790, 65535);
  const fixture = startFixture(options);
  console.log(fixture.origin);
  for (const signal of ['SIGINT', 'SIGTERM'] as const) process.once(signal, async () => {
    await fixture.close(); process.exit(0);
  });
}
