// The normalized cache the gateway and REST pages feed (llp/0001 §4.1, §4.2). A channel's history is
// a set of contiguous sorted runs (§3.6) that merge when they meet; windows read one run, so the view
// never shows across an unfetched gap. Each mutation returns the channels it changed, whose versions
// (and the sidebar's) move only then. Local echoes sit in the live run until their nonce's
// MESSAGE_CREATE replaces them in place, keeping the row's key (§3.9). A send's REST reply can be
// applied as a MESSAGE_CREATE too; the gateway's copy then updates the same row.
import { cmpId } from './ids.ts';

export type DiscordObject = Record<string, any>;
export interface User extends DiscordObject { id: string }
export interface Guild extends DiscordObject { id: string }
export interface Channel extends DiscordObject { id: string; guild_id?: string; type?: number }
export interface MessageRow extends DiscordObject {
  id: string | null;
  channel_id: string;
  key: string;
  status: 'sent' | 'pending' | 'failed';
  error?: string;
}
export interface Changed { channels: string[]; sidebar: boolean }
export interface HistoryQuery { before?: string; after?: string; around?: string; limit: number }
export interface PendingMessage {
  nonce: string;
  content: string;
  replyTo?: { messageId: string; channelId?: string; guildId?: string };
  nowMs: number;
}
export interface MessageWindow {
  rows: MessageRow[];
  moreBefore: boolean;
  moreAfter: boolean;
  reachesStart: boolean;
  reachesNewest: boolean;
  fetchBefore: string | null;
  fetchAfter: string | null;
  /** Request a cursorless latest page: the live tail has no confirmed rows and its start is unknown. */
  fetchLatest: boolean;
}
export interface ChannelGroup { category: Channel | null; channels: Channel[] }
export interface UnreadState { unread: boolean; mentionCount: number; lastReadId: string | null }
export interface TypingUser { user: User; expiresAtMs: number }
interface ReadState { lastReadId: string | null; mentionCount: number }
interface Entry { row: MessageRow; sortId: string; tie: number; run: Run; unanchored?: boolean }
interface Run {
  entries: Entry[];
  low: string | null;
  high: string | null;
  rowLow: string | null;
  rowHigh: string | null;
  reachesStart: boolean;
  reachesNewest: boolean;
}
interface History {
  messages: Map<string, Entry>;
  pending: Map<string, Entry>;
  tombstones: Set<string>;
  runs: Run[];
  live?: Run;
  typing: Map<string, number>;
  nextTie: number;
}
export interface Store {
  meId: string | null;
  users: Map<string, User>;
  guildMap: Map<string, Guild>;
  channelMap: Map<string, Channel>;
  histories: Map<string, History>;
  readStates: Map<string, ReadState>;
  versions: Map<string, number>;
  sidebarRevision: number;
}
interface Changes { channels: Set<string>; sidebar: boolean }

/** Create an account-local cache; a fresh READY replaces its account data. */
export function createStore(opts: { meId?: string } = {}): Store {
  return {
    meId: opts.meId ?? null, users: new Map(), guildMap: new Map(), channelMap: new Map(),
    histories: new Map(), readStates: new Map(), versions: new Map(), sidebarRevision: 0,
  };
}

function changes(): Changes { return { channels: new Set(), sidebar: false }; }

function finish(s: Store, c: Changes): Changed {
  for (const id of c.channels) s.versions.set(id, (s.versions.get(id) ?? 0) + 1);
  if (c.sidebar) s.sidebarRevision++;
  return { channels: [...c.channels], sidebar: c.sidebar };
}

function historyFor(s: Store, id: string): History {
  let h = s.histories.get(id);
  if (!h) {
    h = { messages: new Map(), pending: new Map(), tombstones: new Set(), runs: [], typing: new Map(), nextTie: 0 };
    s.histories.set(id, h);
  }
  return h;
}

function newRun(): Run {
  return { entries: [], low: null, high: null, rowLow: null, rowHigh: null, reachesStart: false, reachesNewest: false };
}

function liveRun(h: History): Run {
  if (!h.live) {
    h.live = newRun();
    h.live.reachesNewest = true;
    h.runs.push(h.live);
  }
  return h.live;
}

function include(run: Run, id: string): void {
  if (run.low === null || cmpId(id, run.low) < 0) run.low = id;
  if (run.high === null || cmpId(id, run.high) > 0) run.high = id;
}

function includeRow(run: Run, id: string): void {
  if (run.rowLow === null || cmpId(id, run.rowLow) < 0) run.rowLow = id;
  if (run.rowHigh === null || cmpId(id, run.rowHigh) > 0) run.rowHigh = id;
}

function compare(a: Entry, b: Entry): number { return cmpId(a.sortId, b.sortId) || a.tie - b.tie; }

function lowerBound(entries: Entry[], id: string, tie = 0): number {
  let low = 0, high = entries.length;
  while (low < high) {
    const mid = (low + high) >>> 1;
    const entry = entries[mid];
    if ((cmpId(entry.sortId, id) || entry.tie - tie) < 0) low = mid + 1;
    else high = mid;
  }
  return low;
}

function insert(run: Run, entry: Entry): void {
  entry.run = run;
  const last = run.entries.at(-1);
  if (!last || compare(last, entry) < 0) run.entries.push(entry);
  else run.entries.splice(lowerBound(run.entries, entry.sortId, entry.tie), 0, entry);
}

function anchorPending(h: History, id: string, before: boolean): void {
  for (const entry of h.pending.values()) if (entry.unanchored) {
    entry.sortId = id;
    if (before) entry.tie -= Number.MAX_SAFE_INTEGER;
    entry.unanchored = false;
  }
}

function meets(a: Run, b: Run): boolean {
  if ((a.reachesNewest && b.reachesNewest) || (a.reachesStart && b.reachesStart)) return true;
  return a.low !== null && a.high !== null && b.low !== null && b.high !== null
    && cmpId(a.low, b.high) <= 0 && cmpId(b.low, a.high) <= 0;
}

function merge(a: Run, b: Run): Run {
  const entries: Entry[] = [];
  const seen = new Set<Entry>();
  let i = 0, j = 0;
  while (i < a.entries.length || j < b.entries.length) {
    const entry = j >= b.entries.length || (i < a.entries.length && compare(a.entries[i], b.entries[j]) <= 0)
      ? a.entries[i++] : b.entries[j++];
    if (!seen.has(entry)) { entries.push(entry); seen.add(entry); }
  }
  a.entries = entries;
  if (b.low !== null) include(a, b.low);
  if (b.high !== null) include(a, b.high);
  if (b.rowLow !== null) includeRow(a, b.rowLow);
  if (b.rowHigh !== null) includeRow(a, b.rowHigh);
  a.reachesStart ||= b.reachesStart;
  a.reachesNewest ||= b.reachesNewest;
  for (const entry of entries) entry.run = a;
  return a;
}

function joinPage(h: History, page: Run, linked = new Set<Run>()): void {
  const ownIndex = h.runs.indexOf(page);
  if (ownIndex >= 0) h.runs.splice(ownIndex, 1);
  for (let i = 0; i < h.runs.length;) {
    const run = h.runs[i];
    if (!linked.has(run) && !meets(page, run)) { i++; continue; }
    h.runs.splice(i, 1);
    page = merge(page, run);
    i = 0;
  }
  h.runs.push(page);
  h.runs.sort((a, b) => cmpId(a.low ?? '0', b.low ?? '0'));
  if (page.reachesNewest) h.live = page;
}

function rememberUser(s: Store, user: DiscordObject | undefined): void {
  if (user?.id) s.users.set(user.id, { ...s.users.get(user.id), ...user } as User);
}

function messageUsers(s: Store, data: DiscordObject): void {
  rememberUser(s, data.author);
  for (const user of data.mentions ?? []) rememberUser(s, user);
}

function storedMessage(s: Store, h: History, channelId: string, data: DiscordObject, c: Changes): Entry | null {
  const id = data.id as string;
  if (!id || h.tombstones.has(id)) return null;
  messageUsers(s, data);
  let entry = h.messages.get(id);
  const nonce = data.nonce == null ? null : String(data.nonce);
  const pending = nonce !== null && data.author?.id === s.meId ? h.pending.get(nonce) : undefined;
  if (pending) {
    if (entry && entry !== pending) eraseEntry(entry);
    entry = pending;
    h.pending.delete(nonce!);
    readOwnMessage(s, channelId, data, c);
  }
  if (entry) {
    const { error: _error, ...previous } = entry.row;
    entry.row = { ...previous, ...data, id, channel_id: channelId, key: previous.key, status: 'sent' };
  } else {
    entry = { row: { ...data, id, channel_id: channelId, key: id, status: 'sent' }, sortId: id, tie: 0, run: newRun() };
  }
  h.messages.set(id, entry);
  return entry;
}

function eraseEntry(entry: Entry): void {
  const run = entry.run;
  const entries = run.entries;
  const index = lowerBound(entries, entry.sortId, entry.tie);
  if (entries[index] === entry) entries.splice(index, 1);
  if (entry.row.id !== null && (entry.row.id === run.rowLow || entry.row.id === run.rowHigh)) {
    run.rowLow = run.rowHigh = null;
    for (const remaining of entries) if (remaining.row.id !== null) includeRow(run, remaining.row.id);
  }
}

function sameFields(a: DiscordObject | undefined, b: DiscordObject): boolean {
  return !!a && Object.keys(b).every(key => a[key] === b[key]);
}

function setChannel(s: Store, data: DiscordObject, c: Changes): void {
  if (!data.id) return;
  const normalized = { ...data };
  if (data.recipients) {
    for (const user of data.recipients) rememberUser(s, user);
    normalized.recipient_ids = data.recipients.map((user: User) => user.id);
    delete normalized.recipients;
  }
  if (sameFields(s.channelMap.get(data.id), normalized)) return;
  s.channelMap.set(data.id, { ...s.channelMap.get(data.id), ...normalized } as Channel);
  c.channels.add(data.id);
  c.sidebar = true;
}

function setGuild(s: Store, data: DiscordObject, c: Changes): void {
  if (!data.id) return;
  const { properties, channels, threads, members, ...rest } = data;
  const guild = { ...properties, ...rest };
  if (!sameFields(s.guildMap.get(data.id), guild)) {
    s.guildMap.set(data.id, { ...s.guildMap.get(data.id), ...guild } as Guild);
    c.sidebar = true;
  }
  for (const channel of [...(channels ?? []), ...(threads ?? [])]) setChannel(s, { ...channel, guild_id: data.id }, c);
  for (const member of members ?? []) rememberUser(s, member.user);
}

function unreadSignature(s: Store, id: string): string {
  const state = unread(s, id);
  return `${state.unread}:${state.mentionCount}`;
}

function setRead(s: Store, id: string, lastReadId: string | null | undefined, mentionCount: number | undefined, c: Changes): void {
  const before = unreadSignature(s, id);
  const previous = s.readStates.get(id);
  const next = { lastReadId: lastReadId === undefined ? previous?.lastReadId ?? null : lastReadId,
    mentionCount: mentionCount ?? previous?.mentionCount ?? 0 };
  if (previous?.lastReadId === next.lastReadId && previous.mentionCount === next.mentionCount) return;
  s.readStates.set(id, next);
  c.channels.add(id);
  c.sidebar ||= before !== unreadSignature(s, id);
}

function readOwnMessage(s: Store, channelId: string, data: DiscordObject, c: Changes): void {
  const lastReadId = s.readStates.get(channelId)?.lastReadId;
  if (data.author?.id === s.meId && (!lastReadId || cmpId(data.id, lastReadId) > 0)) {
    setRead(s, channelId, data.id, undefined, c);
  }
}

function ingestReady(s: Store, data: DiscordObject, c: Changes): void {
  rememberUser(s, data.user);
  if (data.user?.id) s.meId = data.user.id;
  for (const user of data.users ?? []) rememberUser(s, user);
  for (const guild of data.guilds ?? []) setGuild(s, guild, c);
  for (const channel of data.private_channels ?? []) setChannel(s, { type: 1, ...channel }, c);
  for (const members of data.merged_members ?? []) for (const member of members) rememberUser(s, member.user);
  const readState = data.read_state;
  for (const entry of (Array.isArray(readState) ? readState : readState?.entries) ?? []) {
    setRead(s, entry.id ?? entry.channel_id, entry.last_message_id, entry.mention_count, c);
  }
}

function removeChannel(s: Store, id: string, c: Changes): void {
  if (!s.channelMap.has(id) && !s.histories.has(id) && !s.readStates.has(id)) return;
  s.channelMap.delete(id);
  s.histories.delete(id);
  s.readStates.delete(id);
  c.channels.add(id);
  c.sidebar = true;
}

function createMessage(s: Store, data: DiscordObject, c: Changes): void {
  const id = data.channel_id as string;
  if (!id || !data.id) return;
  const h = historyFor(s, id);
  const existed = h.messages.has(data.id);
  const nonce = data.nonce == null ? null : String(data.nonce);
  const pending = nonce !== null && data.author?.id === s.meId ? h.pending.get(nonce) : undefined;
  const before = unreadSignature(s, id);
  if (h.tombstones.has(data.id)) return;
  if (!existed || pending) anchorPending(h, data.id, true);
  const entry = storedMessage(s, h, id, data, c);
  if (!entry) return;
  const channel = s.channelMap.get(id);
  if (!existed && !pending) {
    const run = liveRun(h);
    if (run.low === null && channel?.last_message_id) include(run, channel.last_message_id);
    insert(run, entry);
  }
  include(entry.run, data.id);
  includeRow(entry.run, data.id);
  if (pending) joinPage(h, entry.run);
  readOwnMessage(s, id, data, c);
  if (channel && (!channel.last_message_id || cmpId(data.id, channel.last_message_id) > 0)) {
    s.channelMap.set(id, { ...channel, last_message_id: data.id });
    if (channel.type === 1 || channel.type === 3) c.sidebar = true;
  } else if (!channel) {
    s.channelMap.set(id, { id, guild_id: data.guild_id, type: data.guild_id ? 0 : 1, last_message_id: data.id });
    c.sidebar = true;
  }
  if (!existed && data.author?.id !== s.meId && (data.mention_everyone
    || (s.meId && (data.mentions ?? []).some((user: User) => user.id === s.meId)))) {
    const read = s.readStates.get(id);
    if (!read?.lastReadId || cmpId(data.id, read.lastReadId) > 0) {
      s.readStates.set(id, { lastReadId: read?.lastReadId ?? null, mentionCount: (read?.mentionCount ?? 0) + 1 });
    }
  }
  h.typing.delete(data.author?.id);
  c.channels.add(id);
  c.sidebar ||= before !== unreadSignature(s, id);
}

function reaction(s: Store, t: string, data: DiscordObject, c: Changes): void {
  const entry = s.histories.get(data.channel_id)?.messages.get(data.message_id);
  if (!entry) return;
  const old = (entry.row.reactions ?? []) as DiscordObject[];
  let next = old;
  const matches = (r: DiscordObject) => data.emoji?.id ? r.emoji.id === data.emoji.id
    : !r.emoji.id && r.emoji.name === data.emoji?.name;
  if (t === 'MESSAGE_REACTION_REMOVE_ALL') next = [];
  else if (t === 'MESSAGE_REACTION_REMOVE_EMOJI') next = old.filter(r => !matches(r));
  else {
    const index = old.findIndex(matches);
    const previous = old[index];
    const mine = data.user_id === s.meId;
    const add = t === 'MESSAGE_REACTION_ADD';
    if ((!add && !previous) || (mine && add && previous?.me)) return;
    const count = Math.max(0, (previous?.count ?? 0) + (add ? 1 : -1));
    const updated = { ...previous, emoji: { ...data.emoji }, count, me: mine ? add : previous?.me ?? false };
    next = [...old];
    if (index < 0) next.push(updated);
    else if (count === 0) next.splice(index, 1);
    else next[index] = updated;
  }
  if (next.length === old.length && next.every((value, i) => value === old[i])) return;
  entry.row = { ...entry.row, reactions: next };
  c.channels.add(data.channel_id);
}

/** Apply one decoded gateway dispatch and advance only the affected resource versions. */
export function applyDispatch(s: Store, t: string, d: DiscordObject, nowMs: number): Changed {
  const c = changes();
  switch (t) {
    case 'READY':
      for (const id of new Set([...s.channelMap.keys(), ...s.readStates.keys()])) c.channels.add(id);
      s.users.clear(); s.guildMap.clear(); s.channelMap.clear(); s.readStates.clear();
      ingestReady(s, d, c);
      for (const [id, h] of s.histories) {
        if (h.typing.size) { h.typing.clear(); c.channels.add(id); }
        const latest = s.channelMap.get(id)?.last_message_id;
        if (h.live && (!latest || !h.live.rowHigh || cmpId(h.live.rowHigh, latest) < 0)) {
          h.live.reachesNewest = false;
          h.live = undefined;
          c.channels.add(id);
        }
      }
      c.sidebar = true;
      break;
    case 'READY_SUPPLEMENTAL': ingestReady(s, d, c); break;
    case 'GUILD_CREATE': setGuild(s, { unavailable: false, ...d }, c); break;
    case 'GUILD_UPDATE': setGuild(s, d, c); break;
    case 'GUILD_DELETE':
      if (d.unavailable) setGuild(s, d, c);
      else {
        c.sidebar = s.guildMap.delete(d.id);
        for (const channel of s.channelMap.values()) if (channel.guild_id === d.id) removeChannel(s, channel.id, c);
      }
      break;
    case 'CHANNEL_CREATE':
    case 'CHANNEL_UPDATE': setChannel(s, d, c); break;
    case 'CHANNEL_DELETE': removeChannel(s, d.id, c); break;
    case 'MESSAGE_CREATE': createMessage(s, d, c); break;
    case 'MESSAGE_UPDATE': {
      const entry = s.histories.get(d.channel_id)?.messages.get(d.id);
      if (entry) {
        messageUsers(s, d);
        entry.row = { ...entry.row, ...d, key: entry.row.key, status: 'sent' };
        c.channels.add(d.channel_id);
      }
      break;
    }
    case 'MESSAGE_DELETE':
    case 'MESSAGE_DELETE_BULK': {
      const h = historyFor(s, d.channel_id);
      for (const id of t === 'MESSAGE_DELETE_BULK' ? d.ids ?? [] : [d.id]) {
        if (!id) continue;
        h.tombstones.add(id);
        const entry = h.messages.get(id);
        if (entry) { eraseEntry(entry); h.messages.delete(id); c.channels.add(d.channel_id); }
      }
      break;
    }
    case 'MESSAGE_REACTION_ADD':
    case 'MESSAGE_REACTION_REMOVE':
    case 'MESSAGE_REACTION_REMOVE_ALL':
    case 'MESSAGE_REACTION_REMOVE_EMOJI': reaction(s, t, d, c); break;
    case 'TYPING_START': {
      if (d.user_id === s.meId) break;
      rememberUser(s, d.member?.user);
      const h = historyFor(s, d.channel_id);
      const expires = nowMs + 10000;
      if (h.typing.get(d.user_id) !== expires) { h.typing.set(d.user_id, expires); c.channels.add(d.channel_id); }
      break;
    }
    case 'MESSAGE_ACK': setRead(s, d.channel_id, d.message_id, d.mention_count ?? 0, c); break;
    case 'CHANNEL_UNREAD_UPDATE':
      for (const read of d.channel_unreads ?? [d]) {
        const id = read.channel_id ?? read.id;
        if (id) setRead(s, id, read.last_message_id, read.mention_count, c);
      }
      break;
  }
  return finish(s, c);
}

/** Add a REST page as a contiguous run, joining only overlapping spans or proven endpoints. */
export function applyHistoryPage(s: Store, channelId: string, query: HistoryQuery, messages: DiscordObject[]): Changed {
  if (!Number.isInteger(query.limit) || query.limit < 1 || query.limit > 50) throw new RangeError('History limit must be 1–50');
  if ([query.before, query.after, query.around].filter(value => value !== undefined).length > 1) {
    throw new Error('History takes only one cursor');
  }
  const h = historyFor(s, channelId);
  const c = changes();
  const page = newRun();
  const seen = new Set<Entry>();
  const linked = new Set<Run>();
  let confirmation: string | undefined;
  let newest: string | undefined;
  for (const message of messages) {
    if (!message.id) continue;
    if (!newest || cmpId(message.id, newest) > 0) newest = message.id;
    if (message.author?.id === s.meId && h.pending.has(String(message.nonce))
      && (!confirmation || cmpId(message.id, confirmation) < 0)) confirmation = message.id;
  }
  if (confirmation) anchorPending(h, confirmation, true);
  else if (newest && !query.before && !query.around && (!query.after || messages.length < query.limit)) {
    anchorPending(h, newest, false);
  }
  for (const message of messages) {
    if (!message.id) continue;
    include(page, message.id);
    const entry = storedMessage(s, h, channelId, message, c);
    if (entry && !seen.has(entry)) {
      includeRow(page, message.id);
      page.entries.push(entry);
      seen.add(entry);
      if (entry.run.entries.length) linked.add(entry.run);
    }
  }
  page.entries.sort(compare);
  // before/after exclude the cursor, but prove continuity right up to it, even if it was deleted.
  if (query.before) include(page, query.before);
  if (query.after) include(page, query.after);
  const short = messages.length < query.limit;
  page.reachesStart = !query.after && !query.around && short;
  page.reachesNewest = (!query.before && !query.after && !query.around) || (!!query.after && short);
  const latest = s.channelMap.get(channelId)?.last_message_id;
  if (latest && page.high !== null && cmpId(page.high, latest) >= 0 && !query.before) page.reachesNewest = true;
  if (page.low === null && query.around) return { channels: [], sidebar: false };
  // A shared row joins runs even when its confirmed id differs from its local display position.
  joinPage(h, page, linked);
  for (const entry of page.entries) entry.run = page;
  c.channels.add(channelId);
  return finish(s, c);
}

/** Append a local send after the live tail, keyed by its caller-supplied nonce. */
export function addPending(s: Store, channelId: string, message: PendingMessage): Changed {
  const h = historyFor(s, channelId);
  if (h.pending.has(message.nonce)) return { channels: [], sidebar: false };
  const run = liveRun(h);
  const last = run.entries.at(-1);
  const lastId = s.channelMap.get(channelId)?.last_message_id;
  const reply = message.replyTo;
  const entry: Entry = {
    row: { id: null, channel_id: channelId, key: message.nonce, nonce: message.nonce, content: message.content,
      author: s.users.get(s.meId ?? '') ?? { id: s.meId }, createdAtMs: message.nowMs, status: 'pending',
      ...(reply ? { message_reference: { message_id: reply.messageId, channel_id: reply.channelId ?? channelId,
        ...(reply.guildId ? { guild_id: reply.guildId } : {}) } } : {}) },
    // Freeze display order on confirmation: another user's message may have arrived after this slot.
    sortId: last?.sortId ?? lastId ?? '0', tie: ++h.nextTie, run,
    unanchored: last?.unanchored ?? (!last && !lastId),
  };
  insert(run, entry);
  h.pending.set(message.nonce, entry);
  return finish(s, { channels: new Set([channelId]), sidebar: false });
}

/** Keep a failed local send visible so the view can offer Retry and Delete. */
export function failPending(s: Store, channelId: string, nonce: string, error: string): Changed {
  const entry = s.histories.get(channelId)?.pending.get(nonce);
  if (!entry || (entry.row.status === 'failed' && entry.row.error === error)) return { channels: [], sidebar: false };
  entry.row = { ...entry.row, status: 'failed', error };
  return finish(s, { channels: new Set([channelId]), sidebar: false });
}

/** Remove an unconfirmed local send without creating a server-message tombstone. */
export function removePending(s: Store, channelId: string, nonce: string): Changed {
  const h = s.histories.get(channelId);
  const entry = h?.pending.get(nonce);
  if (!h || !entry) return { channels: [], sidebar: false };
  eraseEntry(entry);
  h.pending.delete(nonce);
  return finish(s, { channels: new Set([channelId]), sidebar: false });
}

/** Expire typing on the caller's clock and invalidate just the affected channels. */
export function expireTyping(s: Store, nowMs: number): Changed {
  const c = changes();
  for (const [id, h] of s.histories) for (const [userId, expiry] of h.typing) {
    if (expiry <= nowMs) { h.typing.delete(userId); c.channels.add(id); }
  }
  return finish(s, c);
}

/** Read the version used by a channel's message, typing and read-state resources. */
export function channelVersion(s: Store, channelId: string): number { return s.versions.get(channelId) ?? 0; }

/** Read the version used by guild, channel, DM and unread sidebar resources. */
export function sidebarVersion(s: Store): number { return s.sidebarRevision; }

function windowLimit(limit: number): number {
  if (!Number.isFinite(limit) || limit < 1) throw new RangeError('Window limit must be positive');
  return Math.min(200, Math.floor(limit));
}

function sliceWindow(run: Run | undefined, start: number, end: number): MessageWindow {
  if (!run) return { rows: [], moreBefore: true, moreAfter: true, reachesStart: false, reachesNewest: false,
    fetchBefore: null, fetchAfter: null, fetchLatest: false };
  start = Math.max(0, start);
  end = Math.min(run.entries.length, end);
  const reachesStart = start === 0 && run.reachesStart;
  const reachesNewest = end === run.entries.length && run.reachesNewest;
  // Exclusive cursors must name cached ids: low/high can be uncached page endpoints.
  return { rows: run.entries.slice(start, end).map(entry => entry.row),
    moreBefore: !reachesStart, moreAfter: !reachesNewest, reachesStart, reachesNewest,
    fetchBefore: start === 0 && !run.reachesStart ? run.rowLow : null,
    fetchAfter: end === run.entries.length && !run.reachesNewest ? run.rowHigh : null,
    fetchLatest: run.reachesNewest && run.rowHigh === null && !run.reachesStart };
}

function locate(s: Store, channelId: string, id: string): { run: Run; index: number; found: boolean } | null {
  const h = s.histories.get(channelId);
  if (!h) return null;
  const entry = h.messages.get(id) ?? h.pending.get(id);
  if (entry) return { run: entry.run, index: lowerBound(entry.run.entries, entry.sortId, entry.tie), found: true };
  for (const run of h.runs) {
    if ((run.low === null || cmpId(id, run.low) >= 0 || run.reachesStart)
      && (run.high === null || cmpId(id, run.high) <= 0 || run.reachesNewest)) {
      return { run, index: lowerBound(run.entries, id), found: false };
    }
  }
  return null;
}

/** Return up to 200 rows at the live tail; an unloaded tail stays empty and fetchable. */
export function latestWindow(s: Store, channelId: string, limit = 50): MessageWindow {
  const count = windowLimit(limit);
  const run = s.histories.get(channelId)?.live;
  const window = sliceWindow(run, Math.max(0, (run?.entries.length ?? 0) - count), run?.entries.length ?? 0);
  if (!run) window.fetchLatest = true;
  return window;
}

/** Return one bounded run centered on an id, without crossing an unfetched history gap. */
export function windowAround(s: Store, channelId: string, messageId: string, limit = 50): MessageWindow {
  const count = windowLimit(limit);
  const location = locate(s, channelId, messageId);
  if (!location) return sliceWindow(undefined, 0, 0);
  const start = Math.max(0, Math.min(location.index - Math.floor(count / 2), location.run.entries.length - count));
  return sliceWindow(location.run, start, start + count);
}

/** Return rows strictly before a cursor within its cached contiguous run. */
export function windowBefore(s: Store, channelId: string, cursorId: string, limit = 50): MessageWindow {
  const count = windowLimit(limit);
  const location = locate(s, channelId, cursorId);
  if (!location) return sliceWindow(undefined, 0, 0);
  return sliceWindow(location.run, location.index - count, location.index);
}

/** Return rows strictly after a cursor within its cached contiguous run. */
export function windowAfter(s: Store, channelId: string, cursorId: string, limit = 50): MessageWindow {
  const count = windowLimit(limit);
  const location = locate(s, channelId, cursorId);
  if (!location) return sliceWindow(undefined, 0, 0);
  const start = location.index + (location.found ? 1 : 0);
  return sliceWindow(location.run, start, start + count);
}

/** Return normalized guilds in READY order, retaining unavailable guild placeholders. */
export function guilds(s: Store): Guild[] { return [...s.guildMap.values()]; }

function channelOrder(a: Channel, b: Channel): number { return (a.position ?? 0) - (b.position ?? 0) || cmpId(a.id, b.id); }

/** Group a guild's channels by category, with uncategorized channels first and position/id ordering. */
export function guildChannels(s: Store, guildId: string): ChannelGroup[] {
  const channels = [...s.channelMap.values()].filter(channel => channel.guild_id === guildId);
  const categories = channels.filter(channel => channel.type === 4).sort(channelOrder);
  const groups = categories.map(category => ({ category, channels: [] as Channel[] }));
  const byId = new Map(groups.map(group => [group.category.id, group]));
  const ungrouped: ChannelGroup = { category: null, channels: [] };
  for (const channel of channels) if (channel.type !== 4) {
    (byId.get(channel.parent_id) ?? ungrouped).channels.push(channel);
  }
  const result = ungrouped.channels.length ? [ungrouped, ...groups] : groups;
  for (const group of result) group.channels.sort(channelOrder);
  return result;
}

function withRecipients(s: Store, value: Channel): Channel {
  return value.recipient_ids ? { ...value, recipients: value.recipient_ids.map((id: string) => s.users.get(id) ?? { id }) } : value;
}

/** Return DMs and group DMs by descending last message id, resolving deduplicated recipients. */
export function dms(s: Store): Channel[] {
  return [...s.channelMap.values()].filter(channel => channel.type === 1 || channel.type === 3)
    .sort((a, b) => cmpId(b.last_message_id ?? '0', a.last_message_id ?? '0') || cmpId(a.id, b.id))
    .map(value => withRecipients(s, value));
}

/** Read one normalized channel, including resolved DM recipients when present. */
export function channel(s: Store, channelId: string): Channel | null {
  const value = s.channelMap.get(channelId);
  return value ? withRecipients(s, value) : null;
}

/** Read a normalized user, or null until a dispatch supplies it. */
export function user(s: Store, userId: string): User | null { return s.users.get(userId) ?? null; }

/** Compare a channel's last message with its read cursor and report its server mention count. */
export function unread(s: Store, channelId: string): UnreadState {
  const read = s.readStates.get(channelId);
  const last = s.channelMap.get(channelId)?.last_message_id;
  return { unread: !!last && (!read?.lastReadId || cmpId(last, read.lastReadId) > 0),
    mentionCount: read?.mentionCount ?? 0, lastReadId: read?.lastReadId ?? null };
}

/** Find the first cached id after the read cursor in its run; an absent cursor requires the channel's start. */
export function firstUnread(s: Store, channelId: string): string | null {
  const read = unread(s, channelId);
  if (!read.unread) return null;
  const run = read.lastReadId === null ? s.histories.get(channelId)?.runs.find(run => run.reachesStart)
    : locate(s, channelId, read.lastReadId)?.run;
  let first: string | null = null;
  for (const entry of run?.entries ?? []) {
    const id = entry.row.id;
    if (id !== null && (read.lastReadId === null || cmpId(id, read.lastReadId) > 0)
      && (first === null || cmpId(id, first) < 0)) first = id;
  }
  return first;
}

/** Read active typists at the supplied time; expiry invalidation is the caller's expireTyping tick. */
export function typingUsers(s: Store, channelId: string, nowMs: number): TypingUser[] {
  const result: TypingUser[] = [];
  for (const [id, expiresAtMs] of s.histories.get(channelId)?.typing ?? []) {
    if (expiresAtMs > nowMs) result.push({ user: s.users.get(id) ?? { id }, expiresAtMs });
  }
  return result;
}
