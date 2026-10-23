// Excord's data module (llp/0001 §4.2): the protocol client of `discord/` in module state (`live.ts`).
// The gateway is a stream resource whose snapshot is versions; each list the view shows is a
// resource that takes its version and answers from the store, and the open channel's window fetches
// the history it lacks with its own REST pages.
import type { Answer, Result, Sources } from './app.contract.d.ts';
import * as S from './discord/store';
import * as rest from './discord/rest';
import { cmpId, msFromSnowflake } from './discord/ids';
import { applyEvent, at, call, clock, connect, context, createLive, dispatch, heartbeat, nonce, page, RestError, subscribe, touched, type Live } from './live';
import { createRenderer, timeOf, windowOf, type Directories, type Stored, type Unread } from './render';

export const appId = 'com.exact.excord';
// Where Discord is: the fixture (`bun apps/excord/fixture/server.ts --world transcript`) unless the bake
// says otherwise. `EXACT_BACKEND_ORIGIN=https://discord.com` (or `.exact/backend-origin`) replaces this
// literal at bake, and with it the grants below; only Charlie makes that build, by hand (llp/0001 §2).
// Such a build has no token until a person signs in, so it makes no request before then.
export const origin = 'http://127.0.0.1:4790';
const DISCORD: string = 'https://discord.com';
export const grants = origin === DISCORD
  ? ['net.fetch https://discord.com', 'net.websocket wss://gateway.discord.gg', 'net.websocket wss://*.discord.gg',
    'secret.keep discord-token'].join('\n')
  : [`net.fetch ${origin}`, 'net.websocket ws://127.0.0.1:4790', 'secret.keep fixture-token'].join('\n');

const live: Live = createLive({
  rest: origin,
  gateway: origin === DISCORD ? 'wss://gateway.discord.gg' : origin.replace(/^http/, 'ws'),
  cdn: origin === DISCORD ? 'https://cdn.discordapp.com' : origin,
  fixture: /^http:\/\/(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?$/.test(origin),
});
const store = live.store;

type Channel = Result<'loadChannel'>;
type Window = Result<'loadWindow'>;
type Ack = Result<'markRead'>;
const OK: Ack = { ok: true, error: '' };
const refused = (error: unknown): Ack => ({ ok: false, error: error instanceof Error ? error.message : String(error) });

const dirs: Directories = {
  me: () => store.meId ?? '',
  user: id => S.user(store, id),
  channelName: id => S.channel(store, id)?.name ?? 'unknown-channel',
  roleName: id => {
    for (const guild of S.guilds(store)) for (const role of guild.roles ?? []) if (role.id === id) return role.name;
    return 'unknown-role';
  },
  message: (channel, id) => (S.message(store, channel, id) as Stored | null) ?? undefined,
  deleted: (channel, id) => S.deleted(store, channel, id),
  emoji: (id, animated) => `${live.origins.cdn}/emojis/${id}.${animated ? 'gif' : 'png'}`,
};
const render = createRenderer(dirs);
const LIMIT = 200, HALF = 100;

// What is unread after `since`: counted in the cached run that holds it, a lower bound (`more`) when
// that run does not reach the newest message or `since` is not cached; its time is then the read
// message's, since the first unread one is not known.
function unreadAfter(channel: string, since: string | null): Unread {
  if (!since) return { count: 0, more: false, at: 0 };
  const after = S.windowAfter(store, channel, since, LIMIT);
  const rows = after.rows.length ? after.rows : S.latestWindow(store, channel, LIMIT).rows.filter(r => r.id && cmpId(r.id, since) > 0);
  const located = after.rows.length > 0 || !!S.message(store, channel, since);
  const more = !located || !after.reachesNewest;
  return { count: rows.length, more, at: rows[0] && !more ? timeOf(rows[0] as Stored) : msFromSnowflake(since) };
}

function displayName(id: string): string {
  const u = S.user(store, id);
  return u?.global_name || u?.username || id;
}

function channelRecord(id: string): Channel {
  const c = S.channel(store, id);
  const read = S.unread(store, id);
  const known = read.unread ? unreadAfter(id, read.lastReadId) : { count: 0, more: false, at: 0 };
  const first = S.firstUnread(store, id);
  const typing = S.typingUsers(store, id, clock.now).map(t => t.user.global_name || t.user.username || displayName(t.user.id));
  const name = c?.name || (c?.recipients ?? []).map((u: { id: string }) => displayName(u.id)).join(', ');
  return { id, guild: c?.guild_id ?? '', name, topic: c?.topic ?? '', version: S.channelVersion(store, id),
    unread: read.unread ? Math.max(1, known.count) : 0, mentions: read.mentionCount,
    firstUnread: first ?? '', firstUnreadAt: read.unread ? known.at : 0, lastRead: read.lastReadId ?? '',
    arrived: live.arrived.get(id) ?? 0,
    typing: typing.length === 0 ? '' : typing.length === 1 ? `${typing[0]} is typing…` : `${typing.join(' and ')} are typing…` };
}
const TEXT = new Set([0, 5]);

const emptyWindow = (channel: string, anchor: string): Window => ({ channel, anchor, rows: [], hasBefore: false,
  hasAfter: false, firstId: '', lastId: '', pin: '', unread: 0, unreadMore: false, unreadAt: 0 });
const position = (w: S.MessageWindow, id: string): number => w.rows.findIndex(r => r.id === id || r.key === id);

// The window: one run of at most 200 messages, the live tail or around an anchor, fetching the pages
// (50 each, one at a time) the store cannot fill: a cold tail's whole window, which the view pages
// from 50 rows before its edge, and up to two on each side of an anchor. After a fresh READY its
// rows are refetched.
async function loadWindow(channel: string, anchor: string, since: string): Promise<Window> {
  if (!channel) return emptyWindow(channel, anchor);
  const read = () => (anchor ? S.windowAround(store, channel, anchor, LIMIT) : S.latestWindow(store, channel, LIMIT));
  let w = read();
  if (w.stale) { await page(live, channel, anchor ? { around: anchor, limit: 50 } : { limit: 50 }); w = read(); }
  if (anchor && w.fetchAround) { await page(live, channel, { around: anchor, limit: 50 }); w = read(); }
  if (!anchor && w.fetchLatest) { await page(live, channel, { limit: 50 }); w = read(); }
  for (let i = 0; i < (anchor ? 2 : 3) && w.fetchBefore && (anchor ? position(w, anchor) < HALF : w.rows.length < LIMIT); i++) {
    await page(live, channel, { before: w.fetchBefore, limit: 50 });
    w = read();
  }
  for (let i = 0; anchor && i < 2 && w.fetchAfter && w.rows.length - position(w, anchor) - 1 < HALF - 1; i++) {
    await page(live, channel, { after: w.fetchAfter, limit: 50 });
    w = read();
  }
  const firstKey = w.rows[0] ? (w.rows[0].id ?? w.rows[0].key) : '';
  const previous = firstKey ? S.windowBefore(store, channel, firstKey, 1).rows[0] : undefined;
  return windowOf({ rows: w.rows as Stored[], previous: previous as Stored | undefined, hasBefore: w.moreBefore,
    hasAfter: w.moreAfter }, channel, anchor, since, unreadAfter(channel, since || null), render);
}

async function write(work: () => Promise<unknown>): Promise<Ack> {
  try { await work(); return OK; } catch (error) { return refused(error); }
}

function newest(channel: string): string | null {
  const tail = S.latestWindow(store, channel, 1).rows[0];
  return tail?.id ?? S.channel(store, channel)?.last_message_id ?? null;
}

const sources: Sources = {
  gatewayState: (_args, secrets, _storage, native) => connect(live, secrets, native),
  heartbeat: ([now]) => { heartbeat(live, now); return OK; },
  loadGuilds: () => S.guilds(store).filter(g => !g.unavailable).map(g => {
    const channels = S.guildChannels(store, g.id).flatMap(group => group.channels).filter(c => TEXT.has(c.type ?? 0));
    const reads = channels.map(c => S.unread(store, c.id));
    return { id: g.id, name: g.name ?? '', unread: reads.some(r => r.unread), mentions: reads.reduce((n, r) => n + r.mentionCount, 0) };
  }),
  loadChannels: ([guild]) => S.guildChannels(store, guild).flatMap(group => group.channels)
    .filter(c => TEXT.has(c.type ?? 0)).map(c => channelRecord(c.id)),
  loadDms: () => S.dms(store).map(c => channelRecord(c.id)),
  loadChannel: ([channel]) => (channel ? channelRecord(channel) : channelRecord('')),
  loadWindow: ([channel, anchor, , since]) => loadWindow(channel, anchor, since),
  subscribe: ([channel]) => { subscribe(live, S.channel(store, channel)?.guild_id ?? ''); return OK; },
  echoMessage: ([channel, content, replyTo, now]) => {
    if (!content.trim()) return { ok: false, error: 'A message needs some text.', key: '', channel };
    const key = nonce(live, now);
    S.addPending(store, channel, { nonce: key, content, nowMs: clock.now,
      ...(replyTo ? { replyTo: { messageId: replyTo, guildId: S.channel(store, channel)?.guild_id } } : {}) });
    live.sends.set(key, { channel, content, replyTo });
    touched(live, channel);
    live.arrived.set(channel, (live.arrived.get(channel) ?? 0) + 1);
    return { ok: true, error: '', key, channel };
  },
  postMessage: async ([channel, key]) => {
    const send = live.sends.get(key);
    if (!send) return refused('That message is no longer waiting to send.');
    try {
      const reply = send.replyTo ? { messageId: send.replyTo, guildId: S.channel(store, channel)?.guild_id } : undefined;
      const sent = await call(live, rest.sendMessage(context(live, true), channel, { content: send.content, nonce: key, replyTo: reply }));
      live.sends.delete(key);
      applyEvent(live, 'MESSAGE_CREATE', sent);
      return OK;
    } catch (error) {
      S.failPending(store, channel, key, error instanceof Error ? error.message : String(error));
      touched(live, channel);
      return refused(error);
    }
  },
  retrySend: ([channel, key]) => {
    if (!live.sends.has(key)) return { ok: false, error: 'That message is no longer waiting to send.', key, channel };
    S.retryPending(store, channel, key);
    touched(live, channel);
    return { ok: true, error: '', key, channel };
  },
  deleteMessage: ([channel, key]) => write(async () => {
    const row = S.messageByKey(store, channel, key);
    if (!row) throw new Error('Message no longer exists.');
    if (row.status !== 'sent' || row.id === null) {
      S.removePending(store, channel, key); live.sends.delete(key); touched(live, channel); return;
    }
    await call(live, rest.deleteMessage(context(live), channel, row.id));
    applyEvent(live, 'MESSAGE_DELETE', { id: row.id, channel_id: channel });
  }),
  editMessage: ([channel, id, content]) => write(async () => {
    if (!content.trim()) throw new Error('A message needs some text.');
    const edited = await call(live, rest.editMessage(context(live, true), channel, id, content));
    applyEvent(live, 'MESSAGE_UPDATE', edited);
  }),
  react: ([channel, id, emoji]) => write(async () => {
    const row = S.message(store, channel, id);
    if (!row?.id) throw new Error('Message no longer exists.');
    const existing = (row.reactions ?? []).find((r: any) => (r.emoji.id ?? r.emoji.name) === emoji || r.emoji.name === emoji);
    const value = existing?.emoji.id ? { id: existing.emoji.id, name: existing.emoji.name } : { id: null, name: emoji };
    const path = value.id ? `${value.name}:${value.id}` : value.name;
    const mine = existing?.me === true;
    await call(live, (mine ? rest.removeReaction : rest.addReaction)(context(live), channel, row.id, path));
    applyEvent(live, mine ? 'MESSAGE_REACTION_REMOVE' : 'MESSAGE_REACTION_ADD',
      { channel_id: channel, message_id: row.id, user_id: store.meId, emoji: value });
  }),
  // An ack names the newest message the reader has seen (§2: sent as the official client sends it).
  markRead: ([channel]) => write(async () => {
    const last = newest(channel);
    if (!last || !S.unread(store, channel).unread) return;
    const reply = await call(live, rest.ack(context(live, true), channel, last,
      { token: live.ackToken, lastViewed: Math.floor((clock.now - 1420070400000) / 86400000) }));
    if (typeof reply?.token === 'string' || reply?.token === null) live.ackToken = reply.token;
    applyEvent(live, 'MESSAGE_ACK', { channel_id: channel, message_id: last, mention_count: 0 });
  }),
  // Typing goes out at most once in nine seconds a channel while the person writes, as Discord's
  // clients send it.
  sendTyping: ([channel, now]) => write(async () => {
    at(now);
    if (clock.now - (live.typedAt.get(channel) ?? -Infinity) < 9000) return;
    live.typedAt.set(channel, clock.now);
    await call(live, rest.typing(context(live), channel));
  }),
  expireTyping: ([now]) => { S.expireTyping(store, at(now)); return OK; },
  inject: ([channel, what, near, now]) => write(async () => {
    if (!live.origins.fixture) throw new Error('Only the fixture takes injected traffic.');
    const response = await fetch(`${live.origins.rest}/control/inject`, { method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ channel_id: channel, what, near, at_ms: Math.round(at(now)) }) });
    const reply = await response.json();
    if (!response.ok) throw new RestError(response.status, reply?.message ?? `HTTP ${response.status}`);
    // The fixture's frames for this session, applied now rather than when the socket delivers them.
    for (const frame of reply.dispatches ?? []) {
      if (frame.session_id === live.gw?.sessionId) dispatch(live, frame.t, frame.s, frame.d);
    }
  }),
};
export const answer: Answer = (source, args, secrets, storage, native) => sources[source](args, secrets, storage, native);
