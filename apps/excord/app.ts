// Excord's data module (llp/0001 §4.2): the protocol client of `discord/` in module state (`live.ts`).
// The gateway is a stream resource whose snapshot is versions; each list the view shows is a
// resource that takes its version and answers from the store. The open channel's window answers from
// the store too and names the page it lacks, which a mutation fetches.
import type { Answer, Result, Sources } from './app.contract.d.ts';
import * as S from './discord/store';
import * as rest from './discord/rest';
import { cmpId, msFromSnowflake } from './discord/ids';
import { applyEvent, at, call, clock, connect, context, createLive, digest, dispatch, heartbeat, loadSession, nonce, page, recordIdentify, RestError, sendIdentify, signIn, signOut, touched, view, type Live } from './live';
import { protectToken, signInAgain, unlockSession, unprotectToken } from './lock';
import { avatarUrl, createRenderer, timeOf, windowOf, type Directories, type Stored, type Unread } from './render';

export const appId = 'com.exact.excord';
// Where Discord is: the fixture (app.json's `commands.server`) unless the bake says otherwise.
// `EXACT_BACKEND_ORIGIN=https://discord.com` (or `.exact/backend-origin`) replaces this literal at bake,
// and the bake takes the grants below for the origin it bakes; only Charlie makes that build, by hand
// (llp/0001 §2). Such a build has no token until a person signs in, so it makes no request before then.
// A phone reaches the fixture at this Mac's LAN address (`EXACT_BACKEND_ORIGIN=http://10.0.0.73:4790`, the
// server run with `--host 10.0.0.73`): a development build takes the loopback entry below for it, and a
// production build refuses it (`js/bake/src/typescript.mjs`, `keyedGrants`).
export const origin = 'http://127.0.0.1:4790';
const DISCORD: string = 'https://discord.com';
export const grants = {
  'http://127.0.0.1:4790': ['net.fetch http://127.0.0.1:4790', 'net.websocket ws://127.0.0.1:4790',
    'secret.keep fixture-token user-presence optional', 'device.local-auth purpose.lock',
    'fs.read app:/data/fixture-identifies.json', 'fs.write app:/data/fixture-identifies.json'],
  // The gateway resumes on other subdomains (`resume_gateway_url`).
  'https://discord.com': ['net.fetch https://discord.com', 'net.websocket wss://gateway.discord.gg',
    'net.websocket wss://*.discord.gg', 'secret.keep discord-token user-presence optional', 'device.local-auth purpose.lock',
    'fs.read app:/data/discord-identifies.json',
    'fs.write app:/data/discord-identifies.json'],
};

const live: Live = createLive({
  rest: origin,
  gateway: origin === DISCORD ? 'wss://gateway.discord.gg' : origin.replace(/^http/, 'ws'),
  cdn: origin === DISCORD ? 'https://cdn.discordapp.com' : origin,
  fixture: /^http:\/\/(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?$/.test(origin) || privateNetwork(origin),
});
const store = live.store;

/** An http origin at an RFC 1918 or link-local IPv4 address, or a unique-local or link-local IPv6 one: only a
 * development build bakes one, as the fixture's (above). */
function privateNetwork(o: string): boolean {
  const host = /^http:\/\/([^/]+?)(?::\d+)?$/.exec(o)?.[1] ?? '';
  const v4 = /^(\d+)\.(\d+)\.\d+\.\d+$/.exec(host);
  if (v4) {
    const [a, b] = [Number(v4[1]), Number(v4[2])];
    return a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168) || (a === 169 && b === 254);
  }
  return /^\[(?:f[cd][0-9a-f]{2}|fe[89ab][0-9a-f]):/i.test(host);
}

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
  avatar: u => avatarUrl(live.origins.cdn, u),
  nameColor: (channel, roles) => {
    const guild = roles.length ? store.guildMap.get(S.channel(store, channel)?.guild_id ?? '') : undefined;
    if (!guild) return '';
    const colors = roleColors(guild.roles ?? []);
    let best: { position: number; color: string } | undefined;
    for (const id of roles) {
      const c = colors.get(id);
      if (c && (!best || c.position > best.position)) best = c;
    }
    return best?.color ?? '';
  },
};
// Each guild's coloured roles by id, kept while the guild's role array is the same object (the store
// replaces it on any role change).
const colorCache = new WeakMap<object, Map<string, { position: number; color: string }>>();
function roleColors(roles: Record<string, any>[]): Map<string, { position: number; color: string }> {
  let found = colorCache.get(roles);
  if (!found) {
    found = new Map();
    for (const r of roles) {
      const color = Number(r.colors?.primary_color ?? r.color ?? 0);
      if (color > 0) found.set(String(r.id), { position: Number(r.position ?? 0), color: `#${(color & 0xffffff).toString(16).padStart(6, '0')}` });
    }
    colorCache.set(roles, found);
  }
  return found;
}
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
  const first = rows[0] ? timeOf(rows[0] as Stored) : 0;
  return { count: rows.length, more, at: since === NEVER || (first && !more) ? first : msFromSnowflake(since) };
}
// The read position of a channel never read: before its first message.
const NEVER = '0';

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
  const recipients: { id: string }[] = c?.recipients ?? [];
  const avatar = c?.type === 1 && recipients[0] ? avatarUrl(live.origins.cdn, S.user(store, recipients[0].id) ?? recipients[0])
    : c?.type === 3 && c.icon ? `${live.origins.cdn}/channel-icons/${id}/${c.icon}.png?size=128` : '';
  return { id, guild: c?.guild_id ?? '', name, avatar, topic: c?.topic ?? '', version: S.channelVersion(store, id),
    unread: read.unread ? Math.max(1, known.count) : 0, mentions: read.mentionCount,
    firstUnread: first ?? '', firstUnreadAt: read.unread ? known.at : 0,
    lastRead: read.lastReadId ?? (read.unread ? NEVER : ''),
    arrived: live.arrived.get(id) ?? 0,
    typing: typing.length === 0 ? '' : typing.length === 1 ? `${typing[0]} is typing…` : `${typing.join(' and ')} are typing…` };
}
const TEXT = new Set([0, 5]);
const readable = (guild: string) => S.guildChannelList(store, guild).filter(c => TEXT.has(c.type ?? 0) && S.canView(store, c));

const emptyWindow = (channel: string, anchor: string): Window => ({ channel, anchor, rows: [], hasBefore: false,
  hasAfter: false, firstId: '', lastId: '', pin: '', unread: 0, unreadMore: false, unreadAt: 0, need: '', cursor: '' });

// The window: one run of at most 200 cached messages, the live tail or around an anchor, of the rows
// a page has confirmed since the last fresh READY (the rest are left out until a page confirms them).
// It names the one page it lacks (`need`, `cursor`), which the view asks for: the newest 50 for a
// tail with none, 50 around an anchor not held, and 50 beyond an anchor within EDGE rows of the
// window's edge, since the view moves its anchor there only as the reader nears that edge (§2: one
// page on opening a channel, the next as the reader moves toward the edge).
const EDGE = 15;
export function loadWindow(channel: string, anchor: string, since: string): Window {
  if (!channel) return emptyWindow(channel, anchor);
  const w = anchor ? S.windowAround(store, channel, anchor, LIMIT) : S.latestWindow(store, channel, LIMIT);
  const held = (r: S.MessageRow) => r.id === null || S.verified(store, channel, r.id);
  let lo: number, hi: number, pos = -1;
  if (!anchor) {
    hi = w.rows.length; lo = hi;
    while (lo > 0 && held(w.rows[lo - 1])) lo--;
  } else {
    pos = w.rows.findIndex(r => r.id === anchor || r.key === anchor);
    // An anchor that is gone (deleted) holds its place: the first row after it.
    if (pos < 0 && !w.fetchAround) pos = w.rows.findIndex(r => r.id !== null && cmpId(r.id, anchor) > 0);
    if (pos < 0 || !held(w.rows[pos])) { lo = hi = 0; pos = -1; }
    else {
      lo = pos; hi = pos + 1;
      while (lo > 0 && held(w.rows[lo - 1])) lo--;
      while (hi < w.rows.length && held(w.rows[hi])) hi++;
      pos -= lo;
    }
  }
  const rows = w.rows.slice(lo, hi);
  const hasBefore = lo > 0 || w.moreBefore, hasAfter = hi < w.rows.length || w.moreAfter;
  let need = '', cursor = '';
  const firstId = rows.find(r => r.id !== null)?.id ?? '';
  // A tail shorter than a page (a cold channel that heard a message, or one whose older rows a fresh
  // READY left unconfirmed) is filled with one page before it.
  if (!anchor) {
    if (!firstId && (w.fetchLatest || w.rows.length)) need = 'latest';
    else if (firstId && rows.length < 50 && hasBefore) { need = 'before'; cursor = firstId; }
  }
  else if (pos < 0) { need = 'around'; cursor = anchor; }
  else if (pos < EDGE && hasBefore) { need = 'before'; cursor = firstId; }
  else if (rows.length - 1 - pos < EDGE && hasAfter) { need = 'after'; cursor = rows[rows.length - 1].id ?? ''; }
  const firstKey = rows[0] ? (rows[0].id ?? rows[0].key) : '';
  const previous = firstKey ? S.windowBefore(store, channel, firstKey, 1).rows[0] : undefined;
  const shown = windowOf({ rows: rows as Stored[], previous: previous as Stored | undefined, hasBefore, hasAfter },
    channel, anchor, since, unreadAfter(channel, since || null), render);
  // The pin answers this same window with no page: far enough from its top that no page is needed.
  const pinAt = Math.max(shown.rows.length - HALF, Math.min(EDGE, shown.rows.length - 1));
  return { ...shown, pin: anchor || shown.rows[pinAt]?.id || '', need, cursor };
}

const QUERIES: Record<string, (cursor: string) => S.HistoryQuery> = {
  // Around a channel's start (a channel never read) is its first page.
  latest: () => ({ limit: 50 }), around: around => (around === NEVER ? { after: NEVER, limit: 50 } : { around, limit: 50 }),
  before: before => ({ before, limit: 50 }), after: after => ({ after, limit: 50 }),
};

async function write(work: () => Promise<unknown>): Promise<Ack> {
  try { await work(); return OK; } catch (error) { return refused(error); }
}

function newest(channel: string): string | null {
  const tail = S.latestWindow(store, channel, 1).rows[0];
  return tail?.id ?? S.channel(store, channel)?.last_message_id ?? null;
}

const sources: Sources = {
  loadSession: (_args, secrets, storage) => loadSession(live, secrets, storage),
  signIn: ([text], secrets) => { const error = signIn(live, secrets, text); return error ? { ok: false, error } : OK; },
  signOut: (_args, secrets) => { signOut(live, secrets); return OK; },
  unlockSession: (_args, secrets) => unlockSession(live, secrets),
  protectToken: (_args, secrets) => protectToken(live, secrets),
  unprotectToken: (_args, secrets) => unprotectToken(live, secrets),
  signInAgain: (_args, secrets) => signInAgain(live, secrets),
  recordIdentify: async (_args, _secrets, storage) => { await recordIdentify(live, storage); return OK; },
  sendIdentify: () => { sendIdentify(live); return OK; },
  gatewayState: ([ready, epoch], secrets, _storage, native) => connect(live, secrets, native, ready, epoch),
  heartbeat: ([now]) => { heartbeat(live, now); return OK; },
  digest: ([now], secrets) => digest(live, secrets, now),
  loadGuilds: () => S.guilds(store).filter(g => !g.unavailable).map(g => {
    const reads = readable(g.id).map(c => S.unread(store, c.id));
    const name = g.name ?? '';
    return { id: g.id, name, icon: g.icon ? `${live.origins.cdn}/icons/${g.id}/${g.icon}.png?size=128` : '',
      acronym: name.split(/\s+/).filter(Boolean).map((w: string) => Array.from(w)[0]).join('').slice(0, 5),
      unread: reads.some(r => r.unread), mentions: reads.reduce((n, r) => n + r.mentionCount, 0) };
  }),
  // A guild's text channels the account can see, by category; a category with none is left out.
  loadChannels: ([guild]) => S.guildChannels(store, guild).map(group => ({
    id: group.category?.id ?? '', name: group.category?.name ?? '',
    channels: group.channels.filter(c => TEXT.has(c.type ?? 0) && S.canView(store, c)).map(c => channelRecord(c.id)),
  })).filter(group => group.channels.length > 0),
  loadDms: () => S.dms(store).map(c => channelRecord(c.id)),
  loadChannel: ([channel]) => (channel ? channelRecord(channel) : channelRecord('')),
  loadWindow: ([channel, anchor, , since]) => loadWindow(channel, anchor, since),
  loadPage: ([channel, need, cursor]) => write(async () => {
    const query = QUERIES[need];
    if (!query) throw new Error(`No page called ${need}`);
    await page(live, channel, query(cursor));
  }),
  viewChannel: ([channel]) => { view(live, channel); return OK; },
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
