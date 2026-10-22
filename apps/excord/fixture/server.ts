#!/usr/bin/env bun
// A Discord stand-in for tests and agent drives (llp/0001 §4.4): the user gateway (zlib-stream or
// text) and the REST paths Excord uses, over the seeded world, plus `/control/*` to inject traffic.
// `bun apps/excord/fixture/server.ts [--port 4790] [--seed 1]`; app.ts's `origin` points here.
import { constants, createDeflate } from 'node:zlib';
import type { ServerWebSocket } from 'bun';
import {
  createMessageGenerator, createWorld, emptyMessage, fileAssets, imageArtwork, pageMessages,
  replySnapshot, seededRandom, snowflakeFromMs,
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
interface Session { id: string; seq: number; dispatches: string[] }
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
  heartbeatInterval?: number;
  rateLimits?: { default?: RateLimit; messages?: RateLimit; routes?: Record<string, RateLimit> };
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
  const { id, channels, roles, members, ...properties } = guild;
  return { id, properties, channels, roles, members, joined_at: '2024-01-01T00:00:00.000Z',
    large: false, member_count: members.length, version: 1 };
}

export function syntheticGuild(world: World, index: number, randomBytes = 0): Guild {
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
  return { ...source, id, name: `Workshop ${index}`, owner_id: world.me.id,
    description: 'A community for making, learning, and sharing projects.', member_count: count,
    roles: [{ ...source.roles[0], id }],
    channels: Array.from({ length: 8 }, (_, c) => ({ ...source.channels[1],
      id: snowflakeFromMs(Date.UTC(2024, 0, 2) + index, c), guild_id: id,
      name: `project-${c}`, position: c, parent_id: null, last_message_id: null })),
    members: Array.from({ length: count }, (_, m) => ({
      user: { ...world.users[m % world.users.length],
        id: snowflakeFromMs(Date.UTC(2024, 0, 3) + index, m), username: word(24), global_name: word(24),
        avatar: word(32), bio: randomBytes ? word(320) : 'Making a little progress every day.' },
      roles: [], nick: word(24), joined_at: '2024-01-01T00:00:00.000Z', deaf: false, mute: false, flags: 0,
    })),
  };
}

export function readyPayload(world: World, sessionId: string, gatewayUrl: string) {
  return {
    v: 9, user: world.me,
    guilds: world.guilds.map(guild => guild.id === world.lazyGuildId
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
  };
}

export function sizedReady(world: World, sessionId: string, gatewayUrl: string, seq: number, bytes: number): Dispatch {
  const frame: Dispatch = { op: 0, t: 'READY', s: seq, d: readyPayload(world, sessionId, gatewayUrl) };
  let size = Buffer.byteLength(JSON.stringify(frame));
  let index = 1;
  while (size < bytes) {
    const guild = readyGuild(syntheticGuild(world, index++));
    const added = Buffer.byteLength(JSON.stringify(guild)) + 1;
    if (size + added > bytes) break;
    frame.d.guilds.push(guild);
    size += added;
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
  const heartbeatInterval = options.heartbeatInterval ?? 41250;
  let world: World;
  let origin = '', gatewayUrl = '';
  let connectionCounter = 0, sessionCounter = 0, identifyCount = 0, resumeCount = 0, loginsDone = 0;
  let stopAcks = false, readyBytes = 0, loginBytes = 0, frameParts = 1, syntheticCounter = 10000;
  let closed = false;
  let resetVersion = 0;
  const sockets = new Set<Socket>();
  const sessions = new Map<string, Session>();
  const buckets = new Map<string, Bucket>();
  const forced: ForcedLimit[] = [];
  const requests: { method: string; path: string; headers: Record<string, string> }[] = [];
  const frames: FrameSize[] = [];
  const reactionUsers = new Map<string, Set<string>>();
  const burstTimers = new Map<ReturnType<typeof setTimeout>, () => void>();

  function send(socket: Socket, frame: object | string): Promise<number> {
    const serialized = typeof frame === 'string' ? frame : JSON.stringify(frame);
    const meta = JSON.parse(serialized);
    const parts = frameParts;
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

  function broadcast(event: string, data: unknown): Promise<number[]> {
    const pending: Promise<number>[] = [];
    // Offline sessions retain the same dispatches for RESUME's replay.
    for (const session of sessions.values()) {
      const frame = dispatch(session, event, data);
      for (const socket of sockets) if (socket.data.session === session) pending.push(send(socket, frame));
    }
    return Promise.all(pending);
  }

  async function login(socket: Socket) {
    const session: Session = { id: `fixture-${seed}-${++sessionCounter}`, seq: 0, dispatches: [] };
    sessions.set(session.id, session);
    socket.data.session = session;
    const size = readyBytes, burst = loginBytes;
    readyBytes = 0;
    loginBytes = 0;
    const ready = sizedReady(world, session.id, gatewayUrl, session.seq + 1, size);
    let sent = await send(socket, dispatch(session, 'READY', ready.d));
    sent += await send(socket, dispatch(session, 'READY_SUPPLEMENTAL', {
      guilds: world.guilds.map(guild => ({ id: guild.id, voice_states: [], embedded_activities: [] })),
      merged_members: world.guilds.map(guild => guild.members),
      merged_presences: { guilds: world.guilds.map(() => []), friends: [] }, lazy_private_channels: [],
    }));
    sent += await send(socket, dispatch(session, 'GUILD_CREATE', world.guilds.find(guild => guild.id === world.lazyGuildId)));
    while (!socket.data.closed && sent <= burst && burst > 0) {
      const guild = syntheticGuild(world, syntheticCounter++, Math.min(200000, burst - sent + 4096));
      sent += await send(socket, dispatch(session, 'GUILD_CREATE', guild));
    }
    loginsDone++;
  }

  async function receive(socket: Socket, raw: string | Buffer) {
    if (socket.data.closed) return;
    let frame: any;
    try { frame = JSON.parse(String(raw)); } catch { socket.close(4002, 'Invalid JSON'); return; }
    if (frame?.op === 1) {
      if (!stopAcks) await send(socket, { op: 11, d: null });
      return;
    }
    if (frame?.op !== 2 && frame?.op !== 6) return;
    if (frame.d?.token !== world.token) { socket.close(4004, 'Authentication failed'); return; }
    if (socket.data.session) { socket.close(4005, 'Already authenticated'); return; }
    if (frame.op === 2) { identifyCount++; await login(socket); return; }
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

  function createMessage(channelId: string, content: string, authorId = world.me.id, body: any = {}): Message {
    const destination = channel(channelId);
    const author = world.users.find(user => user.id === authorId);
    if (!author) throw new HttpError(400, 'Unknown author');
    const reference = body.message_reference;
    const earlier = reference ? message(channelId, textField(reference, 'message_id')) : undefined;
    if (reference?.channel_id && reference.channel_id !== channelId) throw new HttpError(400, 'Reply must be in this channel');
    const created = emptyMessage(destination, author, world.nextTimestamp++, content);
    if (body.nonce !== undefined) created.nonce = body.nonce;
    if (earlier) {
      created.type = 19;
      created.message_reference = { message_id: earlier.id, channel_id: channelId,
        ...(destination.guild_id ? { guild_id: destination.guild_id } : {}) };
      created.referenced_message = replySnapshot(earlier);
    }
    created.mentions = world.users.filter(user => content.includes(`<@${user.id}>`) || content.includes(`<@!${user.id}>`));
    created.mention_roles = world.guilds.flatMap(guild => guild.roles)
      .filter(role => content.includes(`<@&${role.id}>`)).map(role => role.id);
    created.mention_everyone = /@(?:everyone|here)\b/.test(content);
    world.messages[channelId].push(created);
    destination.last_message_id = created.id;
    return created;
  }

  async function edit(channelId: string, id: string, content: string) {
    const current = message(channelId, id);
    current.content = content;
    current.edited_timestamp = new Date(world.nextTimestamp++).toISOString();
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

  async function react(channelId: string, id: string, emoji: Emoji, userId: string, add: boolean) {
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
    if (users.has(userId) === add) return;
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
    await broadcast(add ? 'MESSAGE_REACTION_ADD' : 'MESSAGE_REACTION_REMOVE', {
      channel_id: channelId, message_id: id, user_id: userId, emoji,
      ...(current.guild_id ? { guild_id: current.guild_id } : {}), burst: false, type: 0,
    });
  }

  async function typing(channelId: string, userId: string) {
    const destination = channel(channelId);
    const user = world.users.find(value => value.id === userId);
    if (!user) throw new HttpError(400, 'Unknown user');
    await broadcast('TYPING_START', { channel_id: channelId, user_id: userId,
      timestamp: Math.floor(world.nextTimestamp / 1000), ...(destination.guild_id
        ? { guild_id: destination.guild_id, member: world.guilds.find(guild => guild.id === destination.guild_id)
          ?.members.find(member => member.user.id === userId) } : {}) });
  }

  function limitRequest(method: string, path: string, headers: Headers): ForcedLimit | undefined {
    const route = rateRoute(method, path);
    const rule = options.rateLimits?.routes?.[route] ?? (/^POST \/channels\/[^/]+\/messages$/.test(route)
      ? options.rateLimits?.messages ?? { limit: 5, windowMs: 5000 }
      : options.rateLimits?.default ?? { limit: 1000, windowMs: 5000 });
    const now = Date.now();
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
      logins_done: loginsDone,
      connection_details: [...sockets].map(socket => ({ id: socket.data.id,
        session_id: socket.data.session?.id ?? null, compressed: socket.data.compressed })),
      session_details: [...sessions.values()].map(session => ({ id: session.id, seq: session.seq,
        dispatches: session.dispatches.length })), requests: [...requests], frames: [...frames],
      stop_acks: stopAcks, ready_bytes: readyBytes, login_bytes: loginBytes, split_parts: frameParts };
  }

  async function control(name: string, body: any) {
    switch (name) {
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
            world.nextTimestamp += interval;
          }
          const history = world.messages[channelId];
          const created = generate(history, world.nextTimestamp++, body.author_id);
          history.push(created);
          destination.last_message_id = created.id;
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
      case 'typing': await typing(textField(body, 'channel_id'), textField(body, 'user_id')); break;
      case 'reconnect': await Promise.all([...sockets].map(socket => send(socket, { op: 7, d: null }))); break;
      case 'invalidate': {
        const resumable = body.resumable === true;
        await Promise.all([...sockets].map(socket => send(socket, { op: 9, d: resumable })));
        if (!resumable) {
          sessions.clear();
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
      case 'ready-size': readyBytes = integer(body.bytes, 3 * 1024 * 1024, 64 * 1024 * 1024); break;
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
        resetVersion++;
        for (const [timer, resolve] of burstTimers) { clearTimeout(timer); resolve(); }
        burstTimers.clear();
        for (const socket of sockets) { socket.data.closed = true; socket.close(1000, 'Fixture reset'); }
        sockets.clear(); sessions.clear(); buckets.clear(); reactionUsers.clear();
        forced.length = requests.length = frames.length = 0;
        connectionCounter = sessionCounter = identifyCount = resumeCount = loginsDone = 0;
        readyBytes = loginBytes = 0; frameParts = 1; stopAcks = false; syntheticCounter = 10000;
        world = createWorld(seed, origin);
        break;
      default: throw new HttpError(404, 'Unknown control endpoint');
    }
    return { ok: true };
  }

  async function rest(request: Request, url: URL): Promise<{ status: number; body?: unknown }> {
    const path = url.pathname.slice('/api/v9'.length), method = request.method;
    if (method === 'GET' && path === '/gateway') return { status: 200, body: { url: gatewayUrl } };
    if (method === 'GET' && path === '/users/@me') return { status: 200, body: world.me };
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
      return { status: 200, body: pageMessages(world.messages[channelId], query) };
    }
    if (!messageId && method === 'POST') {
      const body = await readBody(request), content = textField(body, 'content');
      if (body.nonce !== undefined && !['string', 'number'].includes(typeof body.nonce)) throw new HttpError(400, 'Invalid nonce');
      if (body.enforce_nonce && body.nonce !== undefined) {
        const previous = world.messages[channelId].findLast(value => value.author.id === world.me.id
          && value.nonce !== undefined && String(value.nonce) === String(body.nonce));
        if (previous) return { status: 200, body: previous };
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
      const retry = isRest ? limitRequest(request.method, url.pathname, headers) : undefined;
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
        void receive(socket, message).catch(() => { socket.close(1011, 'Fixture gateway failed'); });
      },
      close(socket, code) {
        socket.data.closed = true;
        sockets.delete(socket);
        if ([1000, 1001, 4007, 4009].includes(code) && socket.data.session) {
          if (sessions.get(socket.data.session.id) === socket.data.session) sessions.delete(socket.data.session.id);
        }
        void socket.data.encoder?.close();
      },
    },
  });
  const host = hostname.includes(':') ? `[${hostname}]` : hostname;
  origin = `http://${host}:${server.port}`;
  gatewayUrl = `ws://${host}:${server.port}/`;
  world = createWorld(seed, origin);
  return {
    origin, gatewayUrl, control: `${origin}/control`, get world() { return world; },
    async close() {
      closed = true;
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
    if (flag !== '--port' && flag !== '--seed') throw new Error(`Unknown option: ${flag}`);
    if (args[i + 1] === undefined) throw new Error(`Missing value for ${flag}`);
    const value = integer(args[++i], 0, flag === '--port' ? 65535 : 4294967295);
    if (flag === '--port') options.port = value; else options.seed = value;
  }
  const fixture = startFixture(options);
  console.log(fixture.origin);
  for (const signal of ['SIGINT', 'SIGTERM'] as const) process.once(signal, async () => {
    await fixture.close(); process.exit(0);
  });
}
