// Excord's data module (llp/0001 §4.2): the protocol client of `discord/` in module state (`live.ts`).
// The gateway is a stream resource whose snapshot is versions; each list the view shows is a
// resource that takes its version and answers from the store. The open channel's window answers from
// the store too and names the page it lacks, which a mutation fetches.
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';
import * as S from './discord/store';
import * as rest from './discord/rest';
import { cmpId, msFromSnowflake } from './discord/ids';
import { applyEvent, at, call, clock, connect, context, createLive, digest, dispatch, heartbeat, loadSession, nonce, page, recordIdentify, RestError, sendIdentify, signIn, signOut, touched, view, type Live, type Upload } from './live';
import { emojiChoices, resolveMentions, suggest, switchTargets } from './compose';
import { protectToken, signInAgain, unlockSession, unprotectToken } from './lock';
import { avatarUrl, clip, createRenderer, timeOf, windowOf, type Directories, type Stored, type Unread } from './render';
import { messageLanguage } from './language';

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
    'secret.keep fixture-token user-presence optional', 'device.local-auth purpose.lock', 'device.age-range',
    'fs.read app:/tmp/picked', 'fs.read app:/tmp/upload', 'fs.write app:/tmp/upload', 'fs.read doc:/',
    'fs.read app:/data/fixture-identifies.json', 'fs.write app:/data/fixture-identifies.json'],
  // The gateway resumes on other subdomains (`resume_gateway_url`).
  'https://discord.com': ['net.fetch https://discord.com', 'net.websocket wss://gateway.discord.gg',
    'net.websocket wss://*.discord.gg', 'secret.keep discord-token user-presence optional', 'device.local-auth purpose.lock', 'device.age-range',
    // A message's files go to Discord's upload bucket (`upload` below), with no credentials.
    'net.fetch https://discord-attachments-uploads-prd.storage.googleapis.com',
    'fs.read app:/tmp/picked', 'fs.read app:/tmp/upload', 'fs.write app:/tmp/upload', 'fs.read doc:/',
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
  channelGuild: id => S.channel(store, id)?.guild_id ?? '',
  guildName: id => store.guildMap.get(id)?.name ?? '',
  canOpen: id => { const c = S.channel(store, id); return !!c && (!c.guild_id || (TEXT.has(c.type ?? 0) && S.canView(store, c))); },
  roleName: id => {
    for (const guild of S.guilds(store)) for (const role of guild.roles ?? []) if (role.id === id) return role.name;
    return 'unknown-role';
  },
  message: (channel, id) => (S.message(store, channel, id) as Stored | null) ?? undefined,
  deleted: (channel, id) => S.deleted(store, channel, id),
  emoji: (id, animated) => `${live.origins.cdn}/emojis/${id}.${animated ? 'gif' : 'png'}`,
  // Discord serves a GIF sticker only from its media host (https://docs.discord.com/developers/reference#image-formatting).
  sticker: (id, extension) => `${extension === 'gif' && live.origins.cdn === 'https://cdn.discordapp.com'
    ? 'https://media.discordapp.net' : live.origins.cdn}/stickers/${id}.${extension}`,
  avatar: u => avatarUrl(live.origins.cdn, u),
  roles: channel => store.guildMap.get(S.channel(store, channel)?.guild_id ?? '')?.roles,
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
// A role's colour as text: the colour itself where it reads on the page, else mixed toward black (light
// mode) or white (dark) just far enough for WCAG's 4.5:1, so a pale role on white and a dark one on
// black stay readable, as a name in its own colour must be. The grounds are the darkest a light row
// gets and the lightest a dark one does (a hovered or mentioned row, Android's tinted page), so every
// state of the row clears it.
function legible(rgb: number): string {
  const channel = (c: number) => { const v = c / 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  const luminance = (c: number[]) => 0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2]);
  const contrast = (a: number[], b: number[]) => { const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
  const base = [rgb >> 16 & 255, rgb >> 8 & 255, rgb & 255];
  const toward = (ground: number[], target: number[]) => {
    for (let t = 0; t <= 1; t += 0.05) {
      const c = base.map((v, i) => Math.round(v + (target[i] - v) * t));
      if (contrast(c, ground) >= 4.5) return c;
    }
    return target;
  };
  const hex = (c: number[]) => `#${c.map(v => v.toString(16).padStart(2, '0')).join('')}`;
  return `light-dark(${hex(toward([232, 232, 232], [0, 0, 0]))}, ${hex(toward([52, 52, 54], [255, 255, 255]))})`;
}

// Each guild's coloured roles by id, kept while the guild's role array is the same object (the store
// replaces it on any role change).
const colorCache = new WeakMap<object, Map<string, { position: number; color: string }>>();
function roleColors(roles: Record<string, any>[]): Map<string, { position: number; color: string }> {
  let found = colorCache.get(roles);
  if (!found) {
    found = new Map();
    for (const r of roles) {
      const color = Number(r.colors?.primary_color ?? r.color ?? 0);
      if (color > 0) found.set(String(r.id), { position: Number(r.position ?? 0), color: legible(color) });
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
  return { id, guild: c?.guild_id ?? '', name, avatar, direct: c?.type === 1, topic: c?.topic ?? '', version: S.channelVersion(store, id),
    private: !!c?.guild_id && !S.everyoneSees(store, c), ageRestricted: c?.nsfw === true,
    unread: read.unread ? Math.max(1, known.count) : 0, mentions: read.mentionCount,
    firstUnread: first ?? '', firstUnreadAt: read.unread ? known.at : 0,
    lastRead: read.lastReadId ?? (read.unread ? NEVER : ''),
    arrived: live.arrived.get(id) ?? 0,
    typing: typing.length === 0 ? '' : typing.length === 1 ? `${typing[0]} is typing…` : `${typing.join(' and ')} are typing…` };
}
let pastes = 0;
const IMAGE_TYPES: Record<string, string> = { png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif',
  webp: 'image/webp', heic: 'image/heic', heif: 'image/heif', avif: 'image/avif', tif: 'image/tiff', tiff: 'image/tiff',
  bmp: 'image/bmp' };
// Discord's upload limit for an account without Nitro; a picture over it, or in a format other people's
// clients may not draw (HEIC), is sent as a JPEG that fits.
const UPLOAD_LIMIT = 10 * 1024 * 1024;
// The most a host without `exactBodyFrom` sends as bytes from the data module (`upload`).
const BYTES_IN_A_TURN = 1024 * 1024;
/** A send's files uploaded as Discord's clients upload them: a slot for each from the channel, the
 * file PUT to the slot's URL (no Discord credentials go there), and the names the message then gives.
 * `done` keeps each file's upload as it lands, so a retry uploads only what did not; nothing goes on
 * once the account has changed (a sign-out, another sign-in). */
async function upload(channel: string, key: string, files: Upload[], done: (rest.Uploaded | undefined)[], storage: Storage): Promise<rest.Uploaded[]> {
  const account = live.account;
  const still = () => { if (live.account !== account) throw new RestError(0, 'Signed out'); };
  const waiting = files.map((f, i) => ({ f, i })).filter(({ i }) => !done[i]);
  const ready = await Promise.all(waiting.map(async ({ f, i }) => {
    if (!f.type.startsWith('image/') || (f.size <= UPLOAD_LIMIT && !/hei[cf]/i.test(f.type))) return { f, i };
    try {
      await storage.fs.mkdir('app:/tmp/upload').catch((e: { code?: string }) => { if (e?.code !== 'EEXIST') throw e; });
      const shrunk = await storage.fs.compressImage(f.path, `app:/tmp/upload/${key}-${i}.jpg`,
        { maxDimension: 4096, maxBytes: UPLOAD_LIMIT - 65536, type: 'image/jpeg' });
      return { f: { ...f, path: shrunk.path, size: shrunk.size, type: shrunk.type, name: f.name.replace(/\.[^.]*$/, '') + '.jpg' }, i };
    } catch (error) {
      // A host with no encoder (Linux, Windows, Android), or a browser that cannot decode HEIC, sends a
      // picture that fits as it is.
      if (f.size <= UPLOAD_LIMIT) return { f, i };
      throw new Error(`${f.name} is over Discord's 10 MB limit, and this device cannot make it smaller.`);
    }
  }));
  still();
  if (ready.length) {
    const slots = await call(live, rest.uploadSlots(context(live, true), channel,
      ready.map(({ f, i }) => ({ id: String(i), filename: f.name, file_size: f.size }))));
    const given: { id: string | number; upload_url: string; upload_filename: string }[] = slots?.attachments ?? [];
    if (given.length !== ready.length) throw new Error('Discord did not offer a place for every file.');
    await Promise.all(given.map(async (slot, n) => {
      const { f, i } = ready[n];
      let response: Response;
      try {
        response = await fetch(slot.upload_url, { method: 'PUT', exactBodyFrom: f.path } as RequestInit);
      } catch (error) {
        // A host that cannot send a file by its path (Windows, for now) sends its bytes, which cross the
        // data module's turn: past about a megabyte that turn outlasts its 100 ms budget and is refused
        // with no answer, so a larger file fails here instead, and its row offers Retry.
        if (!(error instanceof Error) || !/Unsupported/.test(`${(error as { kind?: string }).kind ?? ''} ${error.message}`)) throw error;
        if (f.size > BYTES_IN_A_TURN) throw new Error(`${f.name} is too large to send from this device yet.`);
        still();
        response = await fetch(slot.upload_url, { method: 'PUT', body: await storage.fs.readFile(f.path) });
      }
      if (!response.ok) throw new Error(`The upload of ${f.name} failed (HTTP ${response.status}).`);
      done[i] = { id: String(i), filename: f.name, uploaded_filename: slot.upload_filename };
    }));
  }
  still();
  return done as rest.Uploaded[];
}

/** A person's card: their names and picture, and the roles the message they were opened from says they
 * hold in its server, highest first. */
function profileOf(channel: string, userId: string, messageId: string): Result<'userProfile'> {
  const u = S.user(store, userId) ?? { id: userId };
  const guildId = S.channel(store, channel)?.guild_id ?? '';
  // The member as the message carried it, else as the server's member list has them.
  const listed = store.memberLists.get(guildId)?.items.find(i => i && 'member' in i && i.member.user?.id === userId);
  const member = (messageId ? S.message(store, channel, messageId)?.member : null) ?? (listed && 'member' in listed ? listed.member : undefined);
  const held: string[] = member?.roles ?? (userId === store.meId ? store.roles.get(guildId) ?? [] : []);
  const guild = store.guildMap.get(guildId);
  const roles = ((guild?.roles ?? []) as Record<string, any>[]).filter(r => held.includes(String(r.id)))
    .sort((a, b) => Number(b.position ?? 0) - Number(a.position ?? 0))
    .map(r => { const color = Number(r.colors?.primary_color ?? r.color ?? 0);
      return { id: String(r.id), name: String(r.name ?? ''), color: color ? legible(color & 0xffffff) : '',
        swatch: color ? `#${(color & 0xffffff).toString(16).padStart(6, '0')}` : '' }; });
  return { id: userId, name: member?.nick || u.global_name || u.username || userId, handle: u.username ?? '', avatar: avatarUrl(live.origins.cdn, u),
    bot: u.bot === true, color: roles.find(r => r.color)?.color ?? '', roles };
}

const TEXT = new Set([0, 5]);
// Each server's row, with what is unread and mentioned in the channels the account can see: worked out
// once per change to the lists, for the server list and the rail alike.
let guildCache: { version: number; records: Result<'loadGuilds'> } | null = null;
function guildRecords(): Result<'loadGuilds'> {
  const version = S.sidebarVersion(store);
  if (guildCache?.version === version) return guildCache.records;
  const records = S.guilds(store).filter(g => !g.unavailable).map(g => {
    const reads = readable(g.id).map(c => S.unread(store, c.id));
    const name = g.name ?? '';
    return { id: g.id, name, icon: g.icon ? `${live.origins.cdn}/icons/${g.id}/${g.icon}.png?size=128` : '',
      acronym: clip(name.split(/\s+/).filter(Boolean).map((w: string) => Array.from(w)[0]).join(''), 5),
      unread: reads.some(r => r.unread), mentions: reads.reduce((n, r) => n + r.mentionCount, 0) };
  });
  guildCache = { version, records };
  return records;
}
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
export function loadWindow(channel: string, asked: string, since: string): Window {
  if (!channel) return emptyWindow(channel, asked);
  // An anchor Discord's page did not hold is answered with the channel's end, under the anchor asked.
  if (asked && live.missed.has(`${channel}:${asked}`)) return { ...loadWindow(channel, '', since), anchor: asked, pin: asked };
  const anchor = asked;
  const w = anchor ? S.windowAround(store, channel, anchor, LIMIT) : S.latestWindow(store, channel, LIMIT);
  const held = (r: S.MessageRow) => r.id === null || S.verified(store, channel, r.id);
  let lo: number, hi: number, pos = -1;
  if (!anchor) {
    hi = w.rows.length; lo = hi;
    while (lo > 0 && held(w.rows[lo - 1])) lo--;
  } else {
    pos = w.rows.findIndex(r => r.id === anchor || r.key === anchor);
    // An anchor that is gone (deleted) holds its place: the first row after it, or the newest row when it
    // was past the channel's end.
    if (pos < 0 && !w.fetchAround) pos = w.rows.findIndex(r => r.id !== null && cmpId(r.id, anchor) > 0);
    if (pos < 0 && !w.fetchAround && w.reachesNewest) pos = w.rows.length - 1;
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
  messageLanguage: ([text]) => messageLanguage(text),
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
  loadGuilds: () => guildRecords(),
  // The rail: each server the list shows, in the account's order, those in a folder of several under it.
  loadRail: () => {
    const records = guildRecords(), byId = new Map(records.map(g => [g.id, g]));
    const filed = new Map<string, string>();
    for (const f of store.folders) {
      const here = f.guildIds.filter(id => byId.has(id));
      if (f.id && here.length) for (const id of here) filed.set(id, f.id);
    }
    const out: Result<'loadRail'> = [], seen = new Set<string>();
    for (const g of records) {
      const folderId = filed.get(g.id);
      if (!folderId) { out.push({ id: g.id, folder: false, name: '', color: '', guilds: [g], unread: g.unread, mentions: g.mentions }); continue; }
      if (seen.has(folderId)) continue;
      seen.add(folderId);
      const f = store.folders.find(x => x.id === folderId)!;
      const inside = f.guildIds.filter(id => filed.get(id) === folderId).map(id => byId.get(id)!);
      out.push({ id: `folder-${folderId}`, folder: true, name: f.name,
        color: f.color == null ? '' : `#${(f.color & 0xffffff).toString(16).padStart(6, '0')}`, guilds: inside,
        unread: inside.some(x => x.unread), mentions: inside.reduce((n, x) => n + x.mentions, 0) });
    }
    return out;
  },
  // A guild's text channels the account can see, by category; a category with none is left out.
  loadChannels: ([guild]) => S.guildChannels(store, guild).map(group => ({
    id: group.category?.id ?? '', name: group.category?.name ?? '',
    channels: group.channels.filter(c => TEXT.has(c.type ?? 0) && S.canView(store, c)).map(c => channelRecord(c.id)),
  })).filter(group => group.channels.length > 0),
  loadDms: () => S.dms(store).map(c => channelRecord(c.id)),
  loadChannel: ([channel]) => (channel ? channelRecord(channel) : channelRecord('')),
  loadWindow: ([channel, anchor, , since]) => loadWindow(channel, anchor, since),
  loadEditTarget: ([channel]) => {
    const candidate = channel ? S.latestEditable(store, channel) : { row: null, need: '', cursor: '' };
    return { channel, id: candidate.row?.id ?? '', content: candidate.row?.content ?? '',
      need: candidate.need, cursor: candidate.cursor };
  },
  loadPage: ([channel, need, cursor]) => write(async () => {
    const query = QUERIES[need];
    if (!query) throw new Error(`No page called ${need}`);
    await page(live, channel, query(cursor));
    if (need === 'around' && S.windowAround(store, channel, cursor, LIMIT).fetchAround) live.missed.add(`${channel}:${cursor}`);
  }),
  viewChannel: ([channel]) => { view(live, channel); return OK; },
  suggest: ([channel, draft]) => suggest(store, live.origins.cdn, channel, draft),
  emojiChoices: ([channel, query]) => emojiChoices(store, live.origins.cdn, channel, query),
  userProfile: ([channel, userId, messageId]) => profileOf(channel, userId, messageId),
  // The member list of the channel's server, as op 37's subscription delivered it (its first 100 rows).
  loadMembers: ([channel]) => {
    const guildId = S.channel(store, channel)?.guild_id;
    const list = guildId ? store.memberLists.get(guildId) : undefined;
    if (!list) return [];
    const roles = new Map(((store.guildMap.get(guildId!)?.roles ?? []) as Record<string, any>[]).map(r => [String(r.id), String(r.name ?? '')]));
    // Keyed by who or which group, never by place: an INSERT at the top would otherwise re-key every row below it.
    const seen = new Set<string>();
    return list.items.flatMap((item): Result<'loadMembers'> => {
      if (!item) return [];
      if ('group' in item) {
        const label = item.group.id === 'online' ? 'Online' : item.group.id === 'offline' ? 'Offline' : roles.get(item.group.id) ?? 'Members';
        const id = `g:${item.group.id}`;
        if (seen.has(id)) return [];
        seen.add(id);
        return [{ id, user: '', group: true, label, count: item.group.count, avatar: '', color: '', status: '' }];
      }
      const m = item.member, u = S.user(store, m.user?.id) ?? m.user ?? { id: '' };
      if (seen.has(`m:${u.id}`)) return [];
      seen.add(`m:${u.id}`);
      return [{ id: `m:${u.id}`, user: String(u.id), group: false, label: m.nick || u.global_name || u.username || u.id, count: 0,
        avatar: avatarUrl(live.origins.cdn, u), color: dirs.nameColor(channel, m.roles ?? []),
        status: String(m.presence?.status ?? 'offline') }];
    });
  },
  switchTargets: ([query, open]) => open ? switchTargets(store, live.origins.cdn, query, channelRecord) : [],
  echoMessage: ([channel, written, replyTo, now, picked, mention]) => {
    const files: Upload[] = (picked ?? []).map(f => ({ path: f.path, name: f.name, type: f.type, size: f.size,
      width: f.width ?? 0, height: f.height ?? 0 }));
    if (!written.trim() && !files.length) return { ok: false, error: 'A message needs some text.', key: '', channel };
    const content = resolveMentions(store, channel, written);
    const key = nonce(live, now);
    S.addPending(store, channel, { nonce: key, content, nowMs: clock.now,
      // Until Discord answers, each file shows from its local copy.
      attachments: files.map((f, i) => ({ id: `${key}.${i}`, filename: f.name, size: f.size, url: f.path,
        content_type: f.type, ...(f.width && f.height ? { width: f.width, height: f.height } : {}) })),
      ...(replyTo ? { replyTo: { messageId: replyTo, guildId: S.channel(store, channel)?.guild_id } } : {}) });
    live.sends.set(key, { channel, content, replyTo, mention: mention !== false, files });
    touched(live, channel);
    live.arrived.set(channel, (live.arrived.get(channel) ?? 0) + 1);
    return { ok: true, error: '', key, channel };
  },
  // Pictures pasted into the composer (a screenshot arrives as `image.png`), copied beside the uploads as a
  // picked file's copy is: an `image` draws an `app:` path on every host, not a `doc:` handle, and the
  // image calls read only `app:` paths. Anything not a picture is left out.
  adoptPasted: async ([channel, paths, now], _secrets, storage) => {
    const files: Result<'adoptPasted'>['files'] = [], skipped: string[] = [], failed: string[] = [];
    const made = storage.fs.mkdir('app:/tmp/upload').catch((e: { code?: string }) => { if (e?.code !== 'EEXIST') throw e; });
    for (const path of paths) {
      const name = path.split('/').pop() || 'image.png';
      const extension = name.split('.').pop()?.toLowerCase() ?? '';
      const type = Object.hasOwn(IMAGE_TYPES, extension) ? IMAGE_TYPES[extension] : undefined;
      if (!type) { skipped.push(name); continue; }
      // The paste's time and a count name the copy: tabs of one origin share the folder.
      const copy = `app:/tmp/upload/pasted-${Math.round(now)}-${++pastes}-${name}`;
      try {
        await made;
        const stat = await storage.fs.stat(path);
        if (!stat.isFile) { skipped.push(name); continue; }
        // A native host reads a file of up to about 12 MB into the data module; a larger one is refused here.
        await storage.fs.writeFile(copy, await storage.fs.readFile(path));
        files.push({ path: copy, name, type, size: stat.size, width: null, height: null, duration: null });
      } catch {
        failed.push(name);
      }
    }
    const said = [skipped.length ? `${skipped.join(', ')} ${skipped.length === 1 ? 'is' : 'are'} not a picture` : '',
      failed.length ? `${failed.join(', ')} could not be added (it may be too large to paste: try +)` : ''];
    return { channel, files, error: said.filter(Boolean).join('; ') };
  },
  postMessage: async ([channel, key], _secrets, storage) => {
    const send = live.sends.get(key);
    if (!send) return refused('That message is no longer waiting to send.');
    try {
      const reply = send.replyTo ? { messageId: send.replyTo, guildId: S.channel(store, channel)?.guild_id, mention: send.mention } : undefined;
      const attachments = send.files.length ? await upload(channel, key, send.files, send.uploaded ??= [], storage) : undefined;
      const sent = await call(live, rest.sendMessage(context(live, true), channel,
        { content: send.content, nonce: key, replyTo: reply, attachments }));
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
    try {
      await call(live, rest.deleteMessage(context(live), channel, row.id));
    } catch (error) {
      if (error instanceof RestError) throw error;
      throw new Error('Check your connection and try again.');
    }
    applyEvent(live, 'MESSAGE_DELETE', { id: row.id, channel_id: channel });
  }),
  editMessage: ([channel, id, content]) => write(async () => {
    if (!content.trim()) throw new Error('A message needs some text.');
    const edited = await call(live, rest.editMessage(context(live, true), channel, id, resolveMentions(store, channel, content)));
    applyEvent(live, 'MESSAGE_UPDATE', edited);
  }),
  react: ([channel, id, emoji]) => write(async () => {
    const row = S.message(store, channel, id);
    if (!row?.id) throw new Error('Message no longer exists.');
    const existing = (row.reactions ?? []).find((r: any) => (r.emoji.id ?? r.emoji.name) === emoji || r.emoji.name === emoji);
    // A custom emoji is named by its id: the reaction's own, or the server's emoji of that id.
    const custom = existing?.emoji.id ? existing.emoji : /^\d+$/.test(emoji) ? [...store.emojis.values()].flat().find(e => e.id === emoji) : undefined;
    const value = custom ? { id: custom.id, name: custom.name, ...(custom.animated ? { animated: true } : {}) } : { id: null, name: emoji };
    const path = value.id ? `${value.name}:${value.id}` : value.name;
    const mine = existing?.me === true;
    await call(live, (mine ? rest.removeReaction : rest.addReaction)(context(live), channel, row.id, path));
    applyEvent(live, mine ? 'MESSAGE_REACTION_REMOVE' : 'MESSAGE_REACTION_ADD',
      { channel_id: channel, message_id: row.id, user_id: store.meId, emoji: value });
  }),
  // A vote replaces the reader's earlier one; no answers takes it back.
  vote: ([channel, id, answers]) => write(async () => {
    const row = S.message(store, channel, id);
    if (!row?.id || !row.poll) throw new Error('Message no longer exists.');
    const poll = row.poll;
    const before = ((poll.results?.answer_counts ?? []) as any[]).filter(a => a.me_voted).map(a => String(a.id));
    await call(live, rest.votePoll(context(live), channel, row.id, answers));
    // Any vote event during the request (this vote's own, or a later one from another device) replaced the
    // poll, and the gateway's account wins.
    if (S.message(store, channel, id)?.poll !== poll) return;
    for (const answer of before.filter(a => !answers.includes(a))) {
      applyEvent(live, 'MESSAGE_POLL_VOTE_REMOVE', { channel_id: channel, message_id: row.id, user_id: store.meId, answer_id: answer });
    }
    for (const answer of answers.filter(a => !before.includes(a))) {
      applyEvent(live, 'MESSAGE_POLL_VOTE_ADD', { channel_id: channel, message_id: row.id, user_id: store.meId, answer_id: answer });
    }
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
