// The normalized cache the gateway and REST pages feed (llp/0001 §4.1, §4.2). A channel's history is
// a set of contiguous sorted runs (§3.6) that merge when they meet; windows read one run, so the view
// never shows across an unfetched gap. Each mutation returns the channels it changed, whose versions
// (and the sidebar's) move only then. Local echoes sit in the live run until their nonce's
// MESSAGE_CREATE replaces them in place, keeping the row's key (§3.9). A send's REST reply can be
// applied as a MESSAGE_CREATE too; the gateway's copy then updates the same row.
import { cmpId } from './ids.ts';
import { guildFolders, type Folder } from './settings.ts';

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
  // Files the send carries, shown from their local copies until Discord answers.
  attachments?: DiscordObject[];
  replyTo?: { messageId: string; channelId?: string; guildId?: string };
  nowMs: number;
}
export interface MessageWindow {
  rows: MessageRow[];
  /** Refetch stale rows with since; use an around page at the reader's anchor after a fresh READY. */
  stale: boolean;
  moreBefore: boolean;
  moreAfter: boolean;
  reachesStart: boolean;
  reachesNewest: boolean;
  fetchBefore: string | null;
  fetchAfter: string | null;
  fetchAround: string | null;
  /** Request a cursorless latest page: the live tail has no confirmed rows and its start is unknown. */
  fetchLatest: boolean;
}
export interface ChannelGroup { category: Channel | null; channels: Channel[] }
export interface UnreadState { unread: boolean; mentionCount: number; lastReadId: string | null }
export interface TypingUser { user: User; expiresAtMs: number }
export interface GuildEmoji { id: string; name: string; animated: boolean; available: boolean }
/** One row of a member list: a group's header (a hoisted role's id, "online" or "offline") or a member. */
export type MemberItem = { group: { id: string; count: number } } | { member: DiscordObject };
// `rows` is the revision at which the list's rows last changed: any op but UPDATE (a presence), a new list, or a
// role (a group's name, a name's colour).
export interface MemberList { id: string; items: (MemberItem | undefined)[]; revision: number; rows: number }
interface ReadState { lastReadId: string | null; mentionCount: number }
interface Entry {
  row: MessageRow; sortId: string; tie: number; run: Run; unanchored?: boolean;
  changedAt: number; contentAt: number; reactionsAt: number; pollAt: number; verified: boolean;
}
interface Run {
  entries: Entry[];
  low: string | null;
  high: string | null;
  lowInclusive: boolean;
  highInclusive: boolean;
  // A live arrival can follow this id, but only a fetched row can connect it to older history.
  predecessor?: string;
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
  invalidatedAt: number;
}
export interface Store {
  meId: string | null;
  users: Map<string, User>;
  guildMap: Map<string, Guild>;
  channelMap: Map<string, Channel>;
  histories: Map<string, History>;
  readStates: Map<string, ReadState>;
  // The account's own roles in each guild, which decide what it can see.
  roles: Map<string, string[]>;
  // Each guild's channel ids, rebuilt when a channel joins or leaves the cache (`removals` counts the
  // leaving): a lookup per guild over every channel was quadratic in a large account's READY.
  byGuild: { key: string; ids: Map<string, string[]> } | null;
  removals: number;
  versions: Map<string, number>;
  // The server list's order from the account's settings (`settings.ts`).
  guildOrder: string[];
  // The same list as the account's folders (`settings.ts`), each guild in exactly one.
  folders: Folder[];
  // Each guild's custom emoji, for the composer's suggestions.
  emojis: Map<string, GuildEmoji[]>;
  // Each guild's member list as GUILD_MEMBER_LIST_UPDATE builds it for the channel op 37 subscribed: its
  // list id and its items, groups and members by index (the subscribed range, 0–99).
  memberLists: Map<string, MemberList>;
  // The last revision any member list took: each change takes the next, so no list, whether another
  // guild's or one a new list id started over, repeats a revision a reader saw.
  memberRevision: number;
  sidebarRevision: number;
  // Moves when a known user's name, picture or kind changes, which the DM list shows.
  identityRevision: number;
  sequence: number;
}
interface Changes { channels: Set<string>; sidebar: boolean }

/** Create an account-local cache; a fresh READY replaces its account data. */
export function createStore(opts: { meId?: string } = {}): Store {
  return {
    meId: opts.meId ?? null, users: new Map(), guildMap: new Map(), channelMap: new Map(),
    histories: new Map(), readStates: new Map(), roles: new Map(), byGuild: null, removals: 0, versions: new Map(), guildOrder: [], folders: [], emojis: new Map(), memberLists: new Map(), memberRevision: 0, sidebarRevision: 0, identityRevision: 0, sequence: 0,
  };
}

/** Forget the account (a sign-out): every cached object goes, and every version moves on, so no
 * answer asked after it can match one asked before. */
export function forgetAccount(s: Store): void {
  s.meId = null;
  for (const map of [s.users, s.guildMap, s.channelMap, s.histories, s.readStates, s.roles, s.emojis, s.memberLists]) map.clear();
  s.guildOrder = [];
  s.folders = [];
  s.removals++;
  for (const [id, version] of s.versions) s.versions.set(id, version + 1);
  s.sidebarRevision++;
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
    h = { messages: new Map(), pending: new Map(), tombstones: new Set(), runs: [], typing: new Map(), nextTie: 0, invalidatedAt: 0 };
    s.histories.set(id, h);
  }
  return h;
}

function newRun(): Run {
  return { entries: [], low: null, high: null, lowInclusive: true, highInclusive: true,
    rowLow: null, rowHigh: null, reachesStart: false, reachesNewest: false };
}

function liveRun(h: History): Run {
  if (!h.live) {
    h.live = newRun();
    h.live.reachesNewest = true;
    h.runs.push(h.live);
  }
  return h.live;
}

function include(run: Run, id: string, inclusive = true): void {
  if (run.low === null || cmpId(id, run.low) < 0) { run.low = id; run.lowInclusive = inclusive; }
  else if (id === run.low) run.lowInclusive ||= inclusive;
  if (run.high === null || cmpId(id, run.high) > 0) { run.high = id; run.highInclusive = inclusive; }
  else if (id === run.high) run.highInclusive ||= inclusive;
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

function meets(h: History, a: Run, b: Run): boolean {
  if (a.low === null || a.high === null || b.low === null || b.high === null) return false;
  const left = cmpId(a.low, b.high), right = cmpId(b.low, a.high);
  return left <= 0 && right <= 0
    && (left !== 0 || a.lowInclusive || b.highInclusive || h.tombstones.has(a.low))
    && (right !== 0 || b.lowInclusive || a.highInclusive || h.tombstones.has(b.low));
}

function follows(a: Run, b: Run): boolean {
  return !!a.predecessor && b.entries.some(entry => entry.row.id === a.predecessor);
}

function movePending(h: History, run: Run): void {
  for (const entry of h.pending.values()) if (entry.run !== run) {
    eraseEntry(entry);
    entry.sortId = run.entries.at(-1)?.sortId ?? run.rowHigh ?? '0';
    entry.tie = ++h.nextTie;
    entry.unanchored = run.rowHigh === null;
    insert(run, entry);
  }
}

function demoteLive(h: History): void {
  const old = h.live;
  if (old) old.reachesNewest = false;
  h.live = undefined;
  if (h.pending.size) movePending(h, liveRun(h));
  if (old?.low === null && old.entries.length === 0) h.runs.splice(h.runs.indexOf(old), 1);
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
  if (b.low !== null) include(a, b.low, b.lowInclusive);
  if (b.high !== null) include(a, b.high, b.highInclusive);
  a.predecessor ??= b.predecessor;
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
    if (!linked.has(run) && !meets(h, page, run) && !follows(page, run) && !follows(run, page)) { i++; continue; }
    h.runs.splice(i, 1);
    page = merge(page, run);
    i = 0;
  }
  h.runs.push(page);
  h.runs.sort((a, b) => cmpId(a.low ?? '0', b.low ?? '0'));
  if (page.reachesNewest) h.live = page;
}

// What the app draws of a user: a copy that changes none of these keeps the object the store has, so a
// rendered row keyed on it stays drawn (`render.ts`).
const SHOWN = ['username', 'global_name', 'avatar', 'discriminator', 'bot'];
function rememberUser(s: Store, user: DiscordObject | undefined): void {
  if (!user?.id) return;
  const known = s.users.get(user.id);
  if (known && SHOWN.every(key => !(key in user) || known[key] === user[key])) return;
  if (known) {
    s.identityRevision++;
    // A DM is drawn under its recipient's name and picture (`channel`), so its own version moves too.
    for (const [id, channel] of s.channelMap) {
      if (channel.recipient_ids?.includes(user.id)) s.versions.set(id, (s.versions.get(id) ?? 0) + 1);
    }
  }
  s.users.set(user.id, { ...known, ...user } as User);
}

function messageUsers(s: Store, data: DiscordObject): void {
  rememberUser(s, data.author);
  for (const user of data.mentions ?? []) rememberUser(s, user);
  // A forward names the users its forwarded message mentions in that message's own list, and a reply
  // carries the message it answers, which may be a forward the cache does not hold.
  for (const message of [data, data.referenced_message]) {
    for (const snapshot of Array.isArray(message?.message_snapshots) ? message.message_snapshots : []) {
      for (const user of snapshot?.message?.mentions ?? []) rememberUser(s, user);
    }
  }
  for (const user of data.referenced_message?.mentions ?? []) rememberUser(s, user);
  // An app's answer to a command carries whoever used it.
  rememberUser(s, data.interaction_metadata?.user);
  rememberUser(s, data.interaction?.user);
}

function olderEdit(incoming: string | null | undefined, previous: string): boolean {
  if (!incoming) return true;
  const delta = Date.parse(incoming) - Date.parse(previous);
  if (delta !== 0) return delta < 0;
  // Date.parse truncates fractions to milliseconds; Discord timestamps can retain more digits.
  const a = incoming.match(/\.(\d+)/)?.[1] ?? '', b = previous.match(/\.(\d+)/)?.[1] ?? '';
  const length = Math.max(a.length, b.length);
  return a.padEnd(length, '0') < b.padEnd(length, '0');
}

const contentFields = ['content', 'embeds', 'attachments', 'edited_timestamp', 'mentions',
  'mention_roles', 'mention_everyone', 'flags'];

function mergeMessage(entry: Entry, data: DiscordObject, page?: { since?: number }): MessageRow {
  const { error: _error, ...old } = entry.row;
  const next = { ...old, ...data };
  // Partial dispatches can replace embeds without supplying the edit timestamp.
  const timestamp = !page && data.edited_timestamp === undefined ? old.edited_timestamp : data.edited_timestamp;
  const older = old.edited_timestamp && olderEdit(timestamp, old.edited_timestamp);
  const newer = timestamp && olderEdit(old.edited_timestamp, timestamp);
  if (older || (!newer && page?.since !== undefined && entry.contentAt > page.since)) {
    for (const field of contentFields) if (old[field] !== undefined) next[field] = old[field];
  }
  // Reactions and poll votes arrive as their own events, which a page fetched before them must not undo.
  if (page?.since !== undefined && entry.reactionsAt > page.since && old.reactions !== undefined) {
    next.reactions = old.reactions;
  }
  if (page?.since !== undefined && entry.pollAt > page.since && old.poll !== undefined) next.poll = old.poll;
  // A poll without `results` is one whose counts Discord did not send (Discord: Poll Results Object), not one
  // without votes.
  else if (next.poll && !next.poll.results && old.poll?.results) next.poll = { ...next.poll, results: old.poll.results };
  return { ...next, id: data.id, channel_id: old.channel_id, key: old.key, status: 'sent' } as MessageRow;
}

function updateMessage(s: Store, entry: Entry, data: DiscordObject): void {
  const next = mergeMessage(entry, data);
  if (contentFields.some(field => next[field] !== entry.row[field])) entry.contentAt = s.sequence;
  if (next.reactions !== entry.row.reactions) entry.reactionsAt = s.sequence;
  if (next.poll?.results !== entry.row.poll?.results) entry.pollAt = s.sequence;
  entry.row = next;
  entry.changedAt = s.sequence;
}

function storedMessage(s: Store, h: History, channelId: string, data: DiscordObject, c: Changes,
  page?: { since?: number }): Entry | null {
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
    if (page) entry.row = mergeMessage(entry, data, page);
    else updateMessage(s, entry, data);
  } else {
    entry = { row: { ...data, id, channel_id: channelId, key: id, status: 'sent' }, sortId: id, tie: 0,
      run: newRun(), changedAt: page ? 0 : s.sequence, contentAt: page ? 0 : s.sequence,
      reactionsAt: !page && data.reactions !== undefined ? s.sequence : 0, pollAt: !page && data.poll?.results !== undefined ? s.sequence : 0,
      verified: true };
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

function laterId(a: string | null | undefined, b: string | null | undefined): string | null {
  return a && (!b || cmpId(a, b) > 0) ? a : b ?? null;
}

function newestCached(h: History | undefined, last: string | null | undefined): string | null {
  let newest = last ?? null;
  for (const run of h?.runs ?? []) newest = laterId(newest, run.rowHigh);
  return newest;
}

function setChannel(s: Store, data: DiscordObject, c: Changes): void {
  if (!data.id) return;
  const normalized = { ...data };
  const old = s.channelMap.get(data.id), h = s.histories.get(data.id);
  const last = newestCached(h, laterId(old?.last_message_id, data.last_message_id));
  if (last !== null || data.last_message_id !== undefined) normalized.last_message_id = last;
  if (data.recipients) {
    for (const user of data.recipients) rememberUser(s, user);
    normalized.recipient_ids = data.recipients.map((user: User) => user.id);
    delete normalized.recipients;
  }
  if (sameFields(old, normalized)) return;
  if (old && normalized.guild_id !== undefined && old.guild_id !== normalized.guild_id) s.removals++;
  s.channelMap.set(data.id, { ...old, ...normalized } as Channel);
  if (h?.live && last && (!h.live.rowHigh || cmpId(last, h.live.rowHigh) > 0)) demoteLive(h);
  c.channels.add(data.id);
  c.sidebar = true;
}

function setGuild(s: Store, data: DiscordObject, c: Changes): void {
  if (!data.id) return;
  // What the app never shows is not kept: a large account's READY is mostly these.
  const { properties, channels, threads, members, emojis, stickers, guild_scheduled_events, stage_instances,
    voice_states, embedded_activities, activity_instances, presences, soundboard_sounds, ...rest } = data;
  const { emojis: listed, ...metadata } = properties ?? {};
  const guild = { ...metadata, ...rest };
  if (!sameFields(s.guildMap.get(data.id), guild)) {
    s.guildMap.set(data.id, { ...s.guildMap.get(data.id), ...guild } as Guild);
    c.sidebar = true;
  }
  if (Array.isArray(emojis ?? listed)) setEmojis(s, data.id, emojis ?? listed);
  for (const channel of [...(channels ?? []), ...(threads ?? [])]) setChannel(s, { ...channel, guild_id: data.id }, c);
  for (const member of members ?? []) rememberMember(s, data.id, member, c);
}

function setEmojis(s: Store, guildId: string, emojis: DiscordObject[]): void {
  s.emojis.set(guildId, emojis.filter(e => e.id && e.name).map(e => ({ id: String(e.id), name: String(e.name),
    animated: e.animated === true, available: e.available !== false })));
}

// The list's operations, as Discord sends them: SYNC replaces a range (what it leaves out of the range is
// gone), INVALIDATE forgets one, INSERT and
// DELETE shift what follows, UPDATE replaces one item. A new list id (another channel's permissions)
// starts the list over. Members met here join the user directory.
function updateMemberList(s: Store, d: DiscordObject): void {
  const id = String(d.id ?? '');
  let list = s.memberLists.get(d.guild_id);
  const fresh = !list || list.id !== id;
  if (!list || fresh) { list = { id, items: [], revision: 0, rows: 0 }; s.memberLists.set(d.guild_id, list); }
  const items = list.items.slice();
  const take = (item: any): MemberItem | undefined => {
    if (item?.member) { rememberUser(s, item.member.user); return { member: item.member }; }
    return item?.group ? { group: { id: String(item.group.id), count: Number(item.group.count ?? 0) } } : undefined;
  };
  let moved = fresh;
  for (const op of Array.isArray(d.ops) ? d.ops : []) {
    if (op.op !== 'UPDATE') moved = true;
    const [from, to] = Array.isArray(op.range) ? op.range.map(Number) : [0, -1];
    if (op.op === 'SYNC') {
      const given = Array.isArray(op.items) ? op.items : [];
      for (let i = from; i <= to && i < 200 && (i - from < given.length || i < items.length); i++) items[i] = take(given[i - from]);
    }
    else if (op.op === 'INVALIDATE') for (let i = from; i <= to && i < items.length; i++) items[i] = undefined;
    else if (op.op === 'INSERT' && Number.isInteger(op.index)) items.splice(op.index, 0, take(op.item));
    else if (op.op === 'DELETE' && Number.isInteger(op.index)) items.splice(op.index, 1);
    else if (op.op === 'UPDATE' && Number.isInteger(op.index)) items[op.index] = take(op.item);
  }
  // Its own revision, not the lists': a presence moving in a large server would otherwise redraw them.
  const revision = ++s.memberRevision;
  s.memberLists.set(d.guild_id, { id, items: items.slice(0, 200), revision, rows: moved ? revision : list.rows });
}

function rememberMember(s: Store, guildId: string, member: DiscordObject, c: Changes): void {
  rememberUser(s, member.user);
  const id = member.user_id ?? member.user?.id;
  if (!guildId || !id || id !== s.meId || !Array.isArray(member.roles)) return;
  const old = s.roles.get(guildId);
  if (old && old.length === member.roles.length && old.every((role, i) => role === member.roles[i])) return;
  s.roles.set(guildId, [...member.roles]);
  c.sidebar = true;
}

function setRole(s: Store, guildId: string, role: DiscordObject | null, roleId: string, c: Changes): void {
  const guild = s.guildMap.get(guildId);
  if (!guild) return;
  const roles = ((guild.roles ?? []) as DiscordObject[]).filter(r => r.id !== roleId);
  s.guildMap.set(guildId, { ...guild, roles: role ? [...roles, role] : roles } as Guild);
  c.sidebar = true;
  // The member list names its groups and colours its names by these roles, so its rows change too.
  const list = s.memberLists.get(guildId);
  if (list) { const revision = ++s.memberRevision; s.memberLists.set(guildId, { ...list, revision, rows: revision }); }
}

function unreadSignature(s: Store, id: string): string {
  const state = unread(s, id);
  return `${state.unread}:${state.mentionCount}`;
}

function setRead(s: Store, id: string, lastReadId: string | number | null | undefined, mentionCount: number | undefined, c: Changes): void {
  const before = unreadSignature(s, id);
  const previous = s.readStates.get(id);
  // An id, which a read state may give as the number 0 (nothing read): kept as a string, as every id is.
  const read = lastReadId === undefined || lastReadId === null ? lastReadId : String(lastReadId);
  const next = { lastReadId: read === undefined ? previous?.lastReadId ?? null : read,
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
  if (typeof data.user_settings_proto === 'string') setOrder(s, data.user_settings_proto, c);
  for (const guild of data.guilds ?? []) setGuild(s, guild, c);
  for (const channel of data.private_channels ?? []) setChannel(s, { type: 1, ...channel }, c);
  // `merged_members[i]` holds the account's own member in `guilds[i]`.
  (data.merged_members ?? []).forEach((members: DiscordObject[], i: number) => {
    for (const member of members ?? []) rememberMember(s, data.guilds?.[i]?.id, member, c);
  });
  const readState = data.read_state;
  for (const entry of (Array.isArray(readState) ? readState : readState?.entries) ?? []) {
    setRead(s, entry.id ?? entry.channel_id, entry.last_message_id, entry.mention_count, c);
  }
}

// Settings that cannot be read leave the order as it was.
function setOrder(s: Store, proto: string, c: Changes): void {
  let folders: Folder[] | null;
  try { folders = guildFolders(proto); } catch { return; }
  if (!folders || JSON.stringify(folders) === JSON.stringify(s.folders)) return;
  s.folders = folders;
  s.guildOrder = folders.flatMap(f => f.guildIds);
  c.sidebar = true;
}

function removeChannel(s: Store, id: string, c: Changes): void {
  if (!s.channelMap.has(id) && !s.histories.has(id) && !s.readStates.has(id)) return;
  s.channelMap.delete(id);
  s.removals++;
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
    let run = liveRun(h);
    const newest = run.high ?? channel?.last_message_id;
    if (newest && cmpId(data.id, newest) < 0 && (run.low === null || cmpId(data.id, run.low) < 0)) {
      run = newRun();
      h.runs.push(run);
    }
    insert(run, entry);
  }
  if (pending && entry.run !== h.live) { eraseEntry(entry); insert(liveRun(h), entry); }
  if (entry.run === h.live && entry.run.low === null && channel?.last_message_id
    && cmpId(data.id, channel.last_message_id) > 0) entry.run.predecessor = channel.last_message_id;
  include(entry.run, data.id);
  includeRow(entry.run, data.id);
  if (pending) joinPage(h, entry.run);
  readOwnMessage(s, id, data, c);
  const last = newestCached(h, laterId(channel?.last_message_id, data.id));
  if (channel && last !== channel.last_message_id) {
    s.channelMap.set(id, { ...channel, last_message_id: last });
    if (channel.type === 1 || channel.type === 3) c.sidebar = true;
  } else if (!channel) {
    s.channelMap.set(id, { id, guild_id: data.guild_id, type: data.guild_id ? 0 : 1, last_message_id: last });
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
  entry.changedAt = entry.reactionsAt = s.sequence;
  const old = (entry.row.reactions ?? []) as DiscordObject[];
  if (entry.row.reactions === undefined) entry.row = { ...entry.row, reactions: old };
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
    // A client that applies its own reaction on the REST reply sees the gateway's copy as a repeat.
    if ((!add && !previous) || (mine && add && previous?.me) || (mine && !add && !previous?.me)) return;
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

// A vote on a poll: Discord sends one event an answer a voter adds or takes back.
function pollVote(s: Store, t: string, data: DiscordObject, c: Changes): void {
  const entry = s.histories.get(data.channel_id)?.messages.get(data.message_id);
  const poll = entry?.row.poll;
  const mine = data.user_id === s.meId, add = t === 'MESSAGE_POLL_VOTE_ADD';
  // Counts Discord did not send stay unknown until a page or an update brings them (`partial`); only the
  // reader's own choice is kept meanwhile.
  const partial = !poll?.results || poll.results.partial === true;
  if (!entry || !poll || (partial && !mine)) return;
  const counts = (poll.results?.answer_counts ?? []) as DiscordObject[];
  const index = counts.findIndex(a => String(a.id) === String(data.answer_id));
  const previous = counts[index];
  // A vote applied on the REST reply comes back from the gateway as a repeat.
  if ((!add && !previous) || (mine && add && previous?.me_voted) || (mine && !add && !previous?.me_voted)) return;
  const updated = { ...previous, id: previous?.id ?? Number(data.answer_id), count: Math.max(0, (previous?.count ?? 0) + (add ? 1 : -1)),
    me_voted: mine ? add : previous?.me_voted ?? false };
  const next = [...counts];
  if (index < 0) next.push(updated);
  else next[index] = updated;
  // Partial results are no news about the counts, so a page asked before them still brings its own.
  entry.changedAt = s.sequence;
  if (!partial) entry.pollAt = s.sequence;
  entry.row = { ...entry.row, poll: { ...poll, results: { ...(poll.results ?? { is_finalized: false, partial: true }), answer_counts: next } } };
  c.channels.add(data.channel_id);
}

function deleteMessages(s: Store, channelId: string, ids: string[], c: Changes): void {
  const h = historyFor(s, channelId);
  const channel = s.channelMap.get(channelId);
  const last = channel?.last_message_id;
  const before = unreadSignature(s, channelId);
  const run = last ? h.messages.get(last)?.run ?? h.runs.find(run => run.low !== null && run.high !== null
    && cmpId(run.low, last) <= 0 && cmpId(last, run.high) <= 0) : undefined;
  for (const id of ids) {
    if (!id) continue;
    h.tombstones.add(id);
    const entry = h.messages.get(id);
    if (entry) { eraseEntry(entry); h.messages.delete(id); c.channels.add(channelId); }
  }
  let predecessor: string | null = null;
  if (last && ids.includes(last)) {
    for (const entry of run?.entries ?? []) if (entry.row.id && cmpId(entry.row.id, last) < 0) {
      predecessor = laterId(predecessor, entry.row.id);
    }
  }
  // A deleted head can remain Discord's cursor when its predecessor is not known.
  const next = newestCached(h, predecessor ?? last);
  if (channel && next !== (last ?? null)) {
    s.channelMap.set(channelId, { ...channel, last_message_id: next });
    c.channels.add(channelId);
    c.sidebar ||= channel.type === 1 || channel.type === 3 || before !== unreadSignature(s, channelId);
  }
}

function passiveUpdate(s: Store, channels: DiscordObject[], members: DiscordObject[], guildId: string, c: Changes): void {
  for (const member of members) rememberUser(s, member.user);
  for (const data of channels) {
    if (!data.id) continue;
    const old = s.channelMap.get(data.id);
    const h = s.histories.get(data.id);
    const before = unreadSignature(s, data.id);
    const next: Channel = { ...old, id: data.id, guild_id: guildId };
    const last = newestCached(h, laterId(old?.last_message_id, data.last_message_id));
    if (last !== null || data.last_message_id !== undefined) next.last_message_id = last;
    if (data.last_pin_timestamp !== undefined) next.last_pin_timestamp = data.last_pin_timestamp;
    if (sameFields(old, next)) continue;
    s.channelMap.set(data.id, next);
    if (h?.live && next.last_message_id && (!h.live.rowHigh || cmpId(next.last_message_id, h.live.rowHigh) > 0)) demoteLive(h);
    c.channels.add(data.id);
    c.sidebar ||= !old || before !== unreadSignature(s, data.id);
  }
}

/** Apply one decoded gateway dispatch and advance only the affected resource versions. */
export function applyDispatch(s: Store, t: string, d: DiscordObject, nowMs: number): Changed {
  s.sequence++;
  const c = changes();
  switch (t) {
    case 'READY':
      for (const id of new Set([...s.channelMap.keys(), ...s.readStates.keys()])) c.channels.add(id);
      s.users.clear(); s.guildMap.clear(); s.channelMap.clear(); s.readStates.clear(); s.roles.clear(); s.emojis.clear(); s.memberLists.clear();
      s.guildOrder = [];
      s.folders = [];
      s.removals++;
      ingestReady(s, d, c);
      for (const [id, h] of s.histories) {
        h.invalidatedAt = s.sequence;
        for (const entry of h.messages.values()) entry.verified = false;
        if (h.messages.size) c.channels.add(id);
        if (h.typing.size) { h.typing.clear(); c.channels.add(id); }
        const latest = s.channelMap.get(id)?.last_message_id;
        if (h.live && (!latest || !h.live.rowHigh || cmpId(h.live.rowHigh, latest) < 0)) {
          demoteLive(h);
          c.channels.add(id);
        }
      }
      c.sidebar = true;
      break;
    case 'READY_SUPPLEMENTAL': ingestReady(s, d, c); break;
    // Type 1 is the preloaded settings, which hold the folders.
    case 'USER_SETTINGS_PROTO_UPDATE':
      if (d.settings?.type === 1 && typeof d.settings.proto === 'string') setOrder(s, d.settings.proto, c);
      break;
    case 'PASSIVE_UPDATE_V1': passiveUpdate(s, d.channels ?? [], d.members ?? [], d.guild_id, c); break;
    case 'PASSIVE_UPDATE_V2': passiveUpdate(s, d.updated_channels ?? [], d.updated_members ?? [], d.guild_id, c); break;
    case 'GUILD_CREATE': setGuild(s, { unavailable: false, ...d }, c); break;
    case 'GUILD_UPDATE': setGuild(s, d, c); break;
    case 'GUILD_DELETE':
      if (d.unavailable) setGuild(s, d, c);
      else {
        c.sidebar = s.guildMap.delete(d.id);
        s.emojis.delete(d.id);
        for (const channel of s.channelMap.values()) if (channel.guild_id === d.id) removeChannel(s, channel.id, c);
      }
      break;
    case 'GUILD_MEMBER_UPDATE': rememberMember(s, d.guild_id, d, c); break;
    case 'GUILD_MEMBER_LIST_UPDATE': if (d.guild_id) updateMemberList(s, d); break;
    case 'GUILD_EMOJIS_UPDATE': if (d.guild_id && Array.isArray(d.emojis)) setEmojis(s, d.guild_id, d.emojis); break;
    case 'GUILD_ROLE_CREATE':
    case 'GUILD_ROLE_UPDATE': if (d.role?.id) setRole(s, d.guild_id, d.role, d.role.id, c); break;
    case 'GUILD_ROLE_DELETE': setRole(s, d.guild_id, null, d.role_id, c); break;
    case 'CHANNEL_CREATE':
    case 'CHANNEL_UPDATE': setChannel(s, d, c); break;
    case 'CHANNEL_DELETE': removeChannel(s, d.id, c); break;
    case 'MESSAGE_CREATE': createMessage(s, d, c); break;
    case 'MESSAGE_UPDATE': {
      const entry = s.histories.get(d.channel_id)?.messages.get(d.id);
      if (entry) {
        messageUsers(s, d);
        updateMessage(s, entry, d);
        c.channels.add(d.channel_id);
      }
      break;
    }
    case 'MESSAGE_DELETE':
    case 'MESSAGE_DELETE_BULK': deleteMessages(s, d.channel_id, t === 'MESSAGE_DELETE_BULK' ? d.ids ?? [] : [d.id], c); break;
    case 'MESSAGE_REACTION_ADD':
    case 'MESSAGE_REACTION_REMOVE':
    case 'MESSAGE_REACTION_REMOVE_ALL':
    case 'MESSAGE_REACTION_REMOVE_EMOJI': reaction(s, t, d, c); break;
    case 'MESSAGE_POLL_VOTE_ADD':
    case 'MESSAGE_POLL_VOTE_REMOVE': pollVote(s, t, d, c); break;
    case 'MESSAGE_REACTION_ADD_MANY':
      for (const r of d.reactions ?? []) for (const user_id of r.users ?? []) {
        reaction(s, 'MESSAGE_REACTION_ADD', { ...d, emoji: r.emoji, user_id }, c);
      }
      break;
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

/** Capture before starting a history request; pass it back as applyHistoryPage's since option. */
export function storeSeq(s: Store): number { return s.sequence; }

/** Add a complete range, keeping dispatch changes newer than the request and reconciling absent rows. */
export function applyHistoryPage(s: Store, channelId: string, query: HistoryQuery, messages: DiscordObject[],
  options: { since?: number } = {}): Changed {
  if (!Number.isInteger(query.limit) || query.limit < 1 || query.limit > 50) throw new RangeError('History limit must be 1–50');
  if ([query.before, query.after, query.around].filter(value => value !== undefined).length > 1) {
    throw new Error('History takes only one cursor');
  }
  const h = historyFor(s, channelId);
  const c = changes();
  const page = newRun();
  const latestPage = !query.before && !query.after && !query.around;
  if (latestPage && messages.length === 0 && options.since === undefined) {
    if (h.live) {
      h.live.reachesStart = false;
      for (const entry of h.live.entries) if (entry.row.id !== null) entry.verified = false;
    }
    c.channels.add(channelId);
    return finish(s, c);
  }
  const linked = new Set<Run>();
  const received = new Set<string>();
  let confirmation: string | undefined;
  let newest: string | undefined;
  for (const message of messages) {
    if (!message.id) continue;
    received.add(message.id);
    include(page, message.id);
    if (!newest || cmpId(message.id, newest) > 0) newest = message.id;
    if (message.author?.id === s.meId && h.pending.has(String(message.nonce))
      && (!confirmation || cmpId(message.id, confirmation) < 0)) confirmation = message.id;
  }
  // Exclusive endpoints prove the adjacent interval, not the cursor row itself.
  if (query.before) include(page, query.before, false);
  if (query.after) include(page, query.after, false);
  const short = messages.length < query.limit;
  // A page after id 0 begins at the channel's first message.
  page.reachesStart = query.after === '0' || (!query.after && !query.around && short);
  page.reachesNewest = (!query.before && !query.after && !query.around) || (!!query.after && short);
  const latest = s.channelMap.get(channelId)?.last_message_id;
  if (latest && page.high !== null && cmpId(page.high, latest) >= 0 && !query.before) page.reachesNewest = true;
  if (page.low === null && query.around) return { channels: [], sidebar: false };
  if (confirmation && !latest && !h.live?.rowHigh) page.reachesNewest = true;
  const oldLive = h.live;
  const liveAfterEmptyPage = latestPage && page.high === null && options.since !== undefined
    && options.since >= h.invalidatedAt && oldLive?.entries.some(entry => entry.row.id !== null && entry.changedAt > options.since!);
  const connected = oldLive && (meets(h, page, oldLive) || (oldLive.predecessor && received.has(oldLive.predecessor)) || liveAfterEmptyPage);
  if (liveAfterEmptyPage && oldLive) linked.add(oldLive);
  if (page.reachesNewest && oldLive && !connected && oldLive.rowHigh !== null) {
    if (page.high !== null && cmpId(page.high, oldLive.rowHigh) < 0) page.reachesNewest = false;
    else demoteLive(h);
  }
  const missing: string[] = [];
  for (const [id, entry] of h.messages) {
    if (received.has(id) || (options.since !== undefined && entry.changedAt > options.since)) continue;
    // Above the page's newest row it is authoritative only with `since`: without it, a row the
    // gateway delivered while the request was in flight is indistinguishable from a deleted one.
    const open = options.since !== undefined;
    const above = page.low === null ? open
      : cmpId(id, page.low) > 0 || (id === page.low && page.lowInclusive) || page.reachesStart;
    const below = page.high === null ? open
      : cmpId(id, page.high) < 0 || (id === page.high && page.highInclusive)
        || (open && (latestPage || (!!query.after && short)));
    if (id !== query.before && id !== query.after && above && below) missing.push(id);
  }
  if (page.reachesNewest && (!oldLive || !connected)) {
    movePending(h, page);
    if (h.live?.low === null && h.live.entries.length === 0) h.runs.splice(h.runs.indexOf(h.live), 1);
    h.live = page;
  }
  if (confirmation) anchorPending(h, confirmation, true);
  else if (newest && !query.before && !query.around && (!query.after || messages.length < query.limit)) {
    anchorPending(h, newest, false);
  }
  const seen = new Set(page.entries);
  for (const message of messages) {
    if (!message.id) continue;
    const entry = storedMessage(s, h, channelId, message, c, options);
    if (entry) {
      entry.verified = options.since === undefined || options.since >= h.invalidatedAt;
      includeRow(page, message.id);
      if (!seen.has(entry)) {
        page.entries.push(entry);
        seen.add(entry);
        if (entry.run.entries.length) linked.add(entry.run);
      }
    }
  }
  page.entries.sort(compare);
  // A shared row joins runs even when its confirmed id differs from its local display position.
  joinPage(h, page, linked);
  for (const entry of page.entries) entry.run = page;
  deleteMessages(s, channelId, missing, c);
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
      ...(message.attachments?.length ? { attachments: message.attachments } : {}),
      author: s.users.get(s.meId ?? '') ?? { id: s.meId }, createdAtMs: message.nowMs, status: 'pending',
      ...(reply ? { message_reference: { message_id: reply.messageId, channel_id: reply.channelId ?? channelId,
        ...(reply.guildId ? { guild_id: reply.guildId } : {}) } } : {}) },
    // Freeze display order on confirmation: another user's message may have arrived after this slot.
    sortId: last?.sortId ?? lastId ?? '0', tie: ++h.nextTie, run,
    unanchored: last?.unanchored ?? (!last && !lastId),
    changedAt: 0, contentAt: 0, reactionsAt: 0, pollAt: 0, verified: true,
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

/** Mark a failed local send pending again, for a retry with the same nonce. */
export function retryPending(s: Store, channelId: string, nonce: string): Changed {
  const entry = s.histories.get(channelId)?.pending.get(nonce);
  if (!entry || entry.row.status !== 'failed') return { channels: [], sidebar: false };
  const { error: _error, ...row } = entry.row;
  entry.row = { ...row, status: 'pending' } as MessageRow;
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

/** Forget typing that has ended on the caller's clock. The typing line reads `typingUsers` at its own time, so
 * no version moves; the count is of entries dropped. */
export function expireTyping(s: Store, nowMs: number): number {
  let dropped = 0;
  for (const h of s.histories.values()) for (const [userId, expiry] of h.typing) {
    if (expiry <= nowMs) { h.typing.delete(userId); dropped++; }
  }
  return dropped;
}

/** Read the version used by a channel's message, typing and read-state resources. */
export function channelVersion(s: Store, channelId: string): number { return s.versions.get(channelId) ?? 0; }

/** Read the version used by guild, channel, DM and unread sidebar resources. */
export function sidebarVersion(s: Store): number { return s.sidebarRevision; }

/** A window's most rows: 200, raised by two pages for a fling's lead (F1), so a page that lands while the reader
 * is within the lead's 100 rows of the top is never shed from the window it lands in. */
export const WINDOW_CAP = 300;
function windowLimit(limit: number): number {
  if (!Number.isFinite(limit) || limit < 1) throw new RangeError('Window limit must be positive');
  return Math.min(WINDOW_CAP, Math.floor(limit));
}

function sliceWindow(run: Run | undefined, start: number, end: number): MessageWindow {
  if (!run) return { rows: [], moreBefore: true, moreAfter: true, reachesStart: false, reachesNewest: false,
    fetchBefore: null, fetchAfter: null, fetchAround: null, fetchLatest: false, stale: false };
  start = Math.max(0, start);
  end = Math.min(run.entries.length, end);
  const reachesStart = start === 0 && run.reachesStart;
  const reachesNewest = end === run.entries.length && run.reachesNewest;
  const entries = run.entries.slice(start, end);
  // Exclusive cursors must name cached ids: low/high can be uncached page endpoints.
  return { rows: entries.map(entry => entry.row), stale: entries.some(entry => !entry.verified), fetchAround: null,
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
    if (run.low !== null && run.high !== null && (cmpId(id, run.low) >= 0 || run.reachesStart)
      && (cmpId(id, run.high) <= 0 || run.reachesNewest)) {
      return { run, index: lowerBound(run.entries, id), found: false };
    }
  }
  return null;
}

/** Return up to WINDOW_CAP rows at the live tail; an unloaded tail stays empty and fetchable. */
export function latestWindow(s: Store, channelId: string, limit = 50): MessageWindow {
  const count = windowLimit(limit);
  const run = s.histories.get(channelId)?.live;
  const window = sliceWindow(run, Math.max(0, (run?.entries.length ?? 0) - count), run?.entries.length ?? 0);
  if (!run) window.fetchLatest = true;
  return window;
}

/** Find the newest editable own message without crossing an unconfirmed history gap.
 * A caller may fetch the missing page and ask again; pending sends are never edit targets. */
export function latestEditable(s: Store, channelId: string): { row: MessageRow | null; need: string; cursor: string } {
  const run = s.histories.get(channelId)?.live;
  const missing = (cursor: string) => ({ row: null, need: cursor ? 'before' : 'latest', cursor });
  if (!run?.reachesNewest) return missing('');
  let cursor = '';
  for (let i = run.entries.length - 1; i >= 0; i--) {
    const { row, verified } = run.entries[i];
    if (row.status !== 'sent' || row.id === null) continue;
    if (!verified) return missing(cursor);
    if (row.author?.id === s.meId && !row.webhook_id && !row.message_snapshots?.length && (row.type === undefined || row.type === 0 || row.type === 19)) {
      return { row, need: '', cursor: '' };
    }
    cursor = row.id;
  }
  return run.reachesStart ? { row: null, need: '', cursor: '' } : missing(cursor);
}

/** Return one bounded run centered on an id, without crossing an unfetched history gap. */
export function windowAround(s: Store, channelId: string, messageId: string, limit = 50): MessageWindow {
  const count = windowLimit(limit);
  const location = locate(s, channelId, messageId);
  if (!location) return { ...sliceWindow(undefined, 0, 0), fetchAround: messageId };
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
/** The guilds in the account's order. One the settings do not place, joined since they were saved,
 * comes first, newest first, where Discord's client puts it. */
export function guilds(s: Store): Guild[] {
  const place = new Map(s.guildOrder.map((id, i) => [id, i]));
  const joined = (g: Guild) => Date.parse(g.joined_at ?? '') || 0;
  return [...s.guildMap.values()].sort((a, b) => {
    const pa = place.get(a.id), pb = place.get(b.id);
    if (pa === undefined || pb === undefined) return pa === pb ? joined(b) - joined(a) || cmpId(b.id, a.id) : pa === undefined ? -1 : 1;
    return pa - pb;
  });
}

function channelOrder(a: Channel, b: Channel): number { return (a.position ?? 0) - (b.position ?? 0) || cmpId(a.id, b.id); }

/** Group a guild's channels by category, with uncategorized channels first and position/id ordering. */
function channelIdsOf(s: Store, guildId: string): string[] {
  const key = `${s.channelMap.size}:${s.removals}`;
  if (s.byGuild?.key !== key) {
    const ids = new Map<string, string[]>();
    for (const channel of s.channelMap.values()) {
      if (!channel.guild_id) continue;
      const list = ids.get(channel.guild_id);
      if (list) list.push(channel.id); else ids.set(channel.guild_id, [channel.id]);
    }
    s.byGuild = { key, ids };
  }
  return s.byGuild.ids.get(guildId) ?? [];
}

/** A guild's channels in no order, for a count over them. */
export function guildChannelList(s: Store, guildId: string): Channel[] {
  return channelIdsOf(s, guildId).map(id => s.channelMap.get(id)!).filter(channel => channel?.guild_id === guildId);
}

export function guildChannels(s: Store, guildId: string): ChannelGroup[] {
  const channels = guildChannelList(s, guildId);
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

const ADMINISTRATOR = 1n << 3n;
const VIEW_CHANNEL = 1n << 10n;
const bits = (value: unknown): bigint => {
  try { return BigInt(typeof value === 'string' || typeof value === 'number' ? value : 0); } catch { return 0n; }
};

// What a guild grants the account before a channel's overwrites: the roles it holds and their
// permissions, or `null` when it sees every channel (the owner, an administrator, or a cache that
// cannot decide yet). A guild, its roles and the account's roles are replaced, never changed in place,
// so the same three objects give the same grant, and a channel object its same answer: a large
// account's sidebar asks for thousands of channels on every change.
interface Grant { held: Set<string>; base: bigint }
const grants = new WeakMap<Guild, { mine: string[] | undefined; me: string; grant: Grant | null }>();
const answers = new WeakMap<Channel, { grant: Grant; visible: boolean }>();

function grantOf(s: Store, guild: Guild, me: string): Grant | null {
  const mine = s.roles.get(guild.id);
  const kept = grants.get(guild);
  if (kept && kept.mine === mine && kept.me === me) return kept.grant;
  let grant: Grant | null = null;
  const roles = (guild.roles ?? []) as DiscordObject[];
  if (guild.owner_id !== me && mine && roles.length) {
    // A role the guild no longer has (GUILD_ROLE_DELETE) grants nothing, nor do its overwrites.
    const existing = new Set(roles.map(role => role.id));
    const held = new Set([guild.id, ...mine.filter(id => existing.has(id))]);
    let base = 0n;
    for (const role of roles) if (held.has(role.id)) base |= bits(role.permissions);
    if (!(base & ADMINISTRATOR)) grant = { held, base };
  }
  grants.set(guild, { mine, me, grant });
  return grant;
}

/** Whether the account may see a guild channel: its roles' permissions with the channel's overwrites
 * applied, @everyone's first, then its roles' together, then its own, as Discord computes them
 * (https://discord.com/developers/docs/topics/permissions#permission-overwrites). What the cache cannot
 * decide (no roles yet) is shown. */
export function canView(s: Store, channel: Channel): boolean {
  const guild = channel.guild_id ? s.guildMap.get(channel.guild_id) : undefined;
  if (!guild || !s.meId) return true;
  const grant = grantOf(s, guild, s.meId);
  if (!grant) return true;
  const kept = answers.get(channel);
  if (kept?.grant === grant) return kept.visible;
  let permissions = grant.base;
  const overwrites = (channel.permission_overwrites ?? []) as DiscordObject[];
  const apply = (o: DiscordObject | undefined) => { if (o) permissions = (permissions & ~bits(o.deny)) | bits(o.allow); };
  apply(overwrites.find(o => o.id === guild.id));
  let deny = 0n, allow = 0n;
  for (const o of overwrites) if (o.id !== guild.id && o.type === 0 && grant.held.has(o.id)) { deny |= bits(o.deny); allow |= bits(o.allow); }
  permissions = (permissions & ~deny) | allow;
  apply(overwrites.find(o => o.type === 1 && o.id === s.meId));
  const visible = (permissions & VIEW_CHANNEL) !== 0n;
  answers.set(channel, { grant, visible });
  return visible;
}

/** Whether @everyone may see a guild channel: Discord marks one it may not as private (a lock on its
 * mark), whoever is reading. */
export function everyoneSees(s: Store, channel: Channel): boolean {
  const guild = channel.guild_id ? s.guildMap.get(channel.guild_id) : undefined;
  if (!guild) return true;
  const everyone = ((guild.roles ?? []) as DiscordObject[]).find(role => role.id === guild.id);
  let permissions = bits(everyone?.permissions);
  if (permissions & ADMINISTRATOR) return true;
  const o = ((channel.permission_overwrites ?? []) as DiscordObject[]).find(x => x.id === guild.id);
  if (o) permissions = (permissions & ~bits(o.deny)) | bits(o.allow);
  return (permissions & VIEW_CHANNEL) !== 0n;
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

/** Whether a channel is a DM or a group DM. */
export function isDirect(s: Store, channelId: string): boolean {
  const type = s.channelMap.get(channelId)?.type;
  return type === 1 || type === 3;
}

/** Read one normalized channel, including resolved DM recipients when present. */
export function channel(s: Store, channelId: string): Channel | null {
  const value = s.channelMap.get(channelId);
  return value ? withRecipients(s, value) : null;
}

/** Read one cached message by id, or a local send by its nonce. */
export function message(s: Store, channelId: string, id: string): MessageRow | null {
  const h = s.histories.get(channelId);
  return (h?.messages.get(id) ?? h?.pending.get(id))?.row ?? null;
}

/** Read one cached message by its row key: a send confirmed here keeps its nonce as key. */
export function messageByKey(s: Store, channelId: string, key: string): MessageRow | null {
  const found = message(s, channelId, key);
  if (found) return found;
  for (const entry of s.histories.get(channelId)?.messages.values() ?? []) if (entry.row.key === key) return entry.row;
  return null;
}

/** Whether a cached message is confirmed by a page fetched since the last fresh READY; a local send is. */
export function verified(s: Store, channelId: string, id: string): boolean {
  return s.histories.get(channelId)?.messages.get(id)?.verified ?? true;
}

/** Whether a cached message replies to one of these ids. */
export function repliesTo(s: Store, channelId: string, ids: string[]): boolean {
  const wanted = new Set(ids);
  for (const entry of s.histories.get(channelId)?.messages.values() ?? []) {
    if (wanted.has(entry.row.message_reference?.message_id)) return true;
  }
  return false;
}

/** Whether the store saw this message deleted, as opposed to never having held it. */
export function deleted(s: Store, channelId: string, id: string): boolean {
  return s.histories.get(channelId)?.tombstones.has(id) ?? false;
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
