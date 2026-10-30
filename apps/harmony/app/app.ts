// Harmony's data module: every source the view declares, answered through
// Snapback4. Each persona has its own device partition (`Snapback.switcher`),
// so switching accounts switches partitions, unread marks included.
import type { Answer, Result, Sources } from './app.contract.d.ts';
import { Snapback, refused, type Write } from './snapback4/snapback.ts';
import { api } from './backend/api.ts';
import { registerSync } from '@exact/background';
import type { Rows } from './backend/api.ts';

export const appId = 'com.example.harmony';            // app.json's "app": { "id" }
// The Snapback origin: `snapback4 dev --port 4400` in the directory above; `.exact/backend-origin` overrides it at bake.
const origin = 'http://127.0.0.1:4400';
// The test call's lines begin with `development`: a production build leaves them out (LLP 1133.033 §13 question 4).
export const grants = `net.fetch ${origin}\nsqlite.open app:/data\nfs.read app:/data\nfs.write app:/data
device.background-sync\ndevice.periodic-background-sync\ndevice.notifications purpose.notifications
development device.microphone purpose.microphone\ndevelopment device.calls purpose.calls\ndevelopment fs.read app:/cache/test-call\ndevelopment fs.write app:/cache/test-call`;

const personas = Snapback.switcher((persona: string) => ({ app: appId, name: `harmony-${persona}`, origin,
  viewer: `dev:${persona}`, headers: () => ({ 'x-snapback-persona': persona }) }));

type Person = Result<'snapshot'>['people'][number];
type Msg = Result<'snapshot'>['thread']['messages'][number];
type Reaction = Msg['reactions'][number];
type Room = Result<'snapshot'>['dms'][number];
type Emoji = Result<'snapshot'>['emoji'][number];
type Db = Parameters<Parameters<typeof personas.use>[3]>[0];

const DAY = 86_400_000;
const GROUP_GAP = 5 * 60_000;
const UNICODE = ['👍', '❤️', '😂', '🎉', '🔥', '👀', '🚀', '🤯', '😭', '🙏', '✅', '👋', '🤔', '💯', '🐶', '☕️', '🎧', '✨', '😄', '👏', '🫡', '🥳', '😎', '🙌'];

const keyOf = (principal: string) => principal.startsWith('dev:') ? principal.slice(4) : principal;
const initialsOf = (name: string) => name.split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]!.toUpperCase()).join('') || '?';
const person = (row: Rows['profiles']): Person => ({ key: keyOf(row.principal), principal: row.principal, name: row.name,
  handle: row.handle, status: row.status, hue: row.hue, initials: initialsOf(row.name) });
const unknown = (principal: string): Person => ({ key: keyOf(principal), principal, name: keyOf(principal), handle: keyOf(principal), status: '', hue: 1, initials: initialsOf(keyOf(principal)) });
const dayKey = (at: number, offsetMinutes: number) => Math.floor((at + offsetMinutes * 60_000) / DAY);
const mentions = (body: string, handle: string) => new RegExp(`(^|[^\\w])@(${handle}|everyone|here)(?![\\w])`, 'i').test(body);

// Local-first: a message write (send, edit, delete, react, pin) shows at once.
// The read its `refreshes` starts answers from the device, the write's
// predicted rows `pending: true`, and runs no round; the view's next read
// (each of those mutations' `then` moves the clock) runs the round that
// delivers it. Nothing runs after an answer ends, so natively the device
// still serves one answer at a time.
const unshown = new Set<string>();
let lastOnline = true;
function shown(persona: string, written: Write): Write {
  if (written.state === 'pending') unshown.add(persona);
  return written;
}

function admit(written: Write): Result<'sendMessage'> {
  if (written.state === 'failed') throw refused(written.why);
  return { ok: true, id: written.newIds[0] ?? '' };
}

async function directory(db: Db, now: number): Promise<Map<string, Person>> {
  const read = await db.read(api.people, {}, now);
  if (read.denied) throw refused(read.denied);
  return new Map((read.data ?? []).map((row) => [row.principal, person(row)]));
}

async function emojiTable(db: Db, now: number): Promise<Map<string, Emoji>> {
  const read = await db.read(api.customEmoji, {}, now);
  if (read.denied) throw refused(read.denied);
  return new Map((read.data ?? []).map((row) => [`:${row.name}:`, { id: row.id, name: row.name, path: row.path, fill: row.fill, mine: false }]));
}

// A row from the thread query, projected onto the view's Msg.
function message(row: Rows['messages'] & { reactions: Rows['reactions'][]; pin: Rows['pins'] | null; replyTo: Rows['messages'] | null; pending?: boolean },
  prev: Rows['messages'] | null, me: Person, who: Map<string, Person>, custom: Map<string, Emoji>, offset: number): Msg {
  const author = who.get(row.authorId) ?? unknown(row.authorId);
  const newDay = prev === null || dayKey(prev.at, offset) !== dayKey(row.at, offset);
  const head = newDay || prev === null || prev.authorId !== row.authorId || row.at - prev.at > GROUP_GAP;
  const groups = new Map<string, Rows['reactions'][]>();
  for (const r of row.reactions) groups.set(r.emoji, [...(groups.get(r.emoji) ?? []), r]);
  const reactions: Reaction[] = [...groups.entries()].map(([emoji, rows]) => {
    const glyph = custom.get(emoji);
    return { emoji, count: rows.length, mine: rows.some((r) => r.principal === me.principal), custom: glyph !== undefined,
      path: glyph?.path ?? '', fill: glyph?.fill ?? '', who: rows.map((r) => (who.get(r.principal) ?? unknown(r.principal)).name).join(', ') };
  });
  const replyAuthor = row.replyTo ? (who.get(row.replyTo.authorId) ?? unknown(row.replyTo.authorId)).name : '';
  return { id: row.id, authorId: row.authorId, author: author.name, initials: author.initials, hue: author.hue, body: row.body, at: row.at,
    newDay, head, edited: row.editedAt !== null, pinned: row.pin !== null, mine: row.authorId === me.principal,
    mentionsMe: row.authorId !== me.principal && mentions(row.body, me.handle),
    replyAuthor, replyBody: row.replyTo?.body ?? '', replyDeleted: row.replyToId !== null && row.replyTo === null,
    pending: row.pending === true, reactions };
}

function typingLine(rows: { principal: string; expiresAt: number }[], me: Person, who: Map<string, Person>, now: number): string {
  const names = rows.filter((r) => r.principal !== me.principal && r.expiresAt > now).map((r) => (who.get(r.principal) ?? unknown(r.principal)).name.split(' ')[0]);
  if (names.length === 0) return '';
  if (names.length === 1) return `${names[0]} is typing…`;
  if (names.length === 2) return `${names[0]} and ${names[1]} are typing…`;
  return 'Several people are typing…';
}

// The one read: everything the view shows, from one answer. Natively the
// device serves one answer at a time, and a continuation that waited on
// another answer's round is refused once that answer ends, so reads never
// overlap each other; only a write can overlap this, briefly.
async function inboxOf(db: Db, persona: string, who: Map<string, Person>): Promise<Pick<Result<'snapshot'>, 'channels' | 'dms'>> {
  const read = await db.read(api.inbox, {}, 0);
  if (read.denied) throw refused(read.denied);
  const me = `dev:${persona}`;
  const rooms: Room[] = (read.data ?? []).flatMap((entry) => {
    if (!entry.room) return [];
    const latest = entry.latest[0];
    const other = entry.room.kind === 'dm' ? who.get(entry.members.map((m) => m.principal).find((p) => p !== me) ?? '') : undefined;
    const latestAuthor = latest ? (who.get(latest.authorId) ?? unknown(latest.authorId)) : null;
    return [{ id: entry.room.id, kind: entry.room.kind, name: other?.name ?? entry.room.name, handle: other?.handle ?? entry.room.name,
      topic: entry.room.topic, category: entry.room.category, unread: entry.unread,
      latestAuthor: latestAuthor ? (latestAuthor.principal === me ? 'You' : latestAuthor.name.split(' ')[0]!) : '',
      latestBody: latest?.body.replace(/\s+/g, ' ') ?? '', latestAt: latest?.at ?? 0,
      hue: other?.hue ?? 1, initials: other?.initials ?? '', status: other?.status ?? '' }];
  });
  const positions = new Map((read.data ?? []).flatMap((e) => e.room ? [[e.room.id, e.room.position] as const] : []));
  const channels = rooms.filter((r) => r.kind === 'text').sort((a, b) => (positions.get(a.id) ?? 0) - (positions.get(b.id) ?? 0) || a.name.localeCompare(b.name));
  const categories: Result<'snapshot'>['channels'] = [];
  for (const r of channels) {
    const c = categories.find((x) => x.name === r.category);
    if (c) c.rooms.push(r); else categories.push({ name: r.category, rooms: [r] });
  }
  return { channels: categories, dms: rooms.filter((r) => r.kind === 'dm').sort((a, b) => b.latestAt - a.latestAt) };
}

const emptyThread: Result<'snapshot'>['thread'] = { id: '', kind: '', title: '', topic: '', typing: '', unread: 0, messages: [], pins: [], members: [] };

async function threadOf(db: Db, roomId: string, now: number, offset: number, me: Person, who: Map<string, Person>, custom: Map<string, Emoji>): Promise<Result<'snapshot'>['thread']> {
  const info = await db.read(api.roomInfo, { roomId }, now);
  if (info.denied) throw refused(info.denied);
  const room = info.data?.room;
  if (!room) throw refused({ code: 'NOT_FOUND', message: 'No such room.' });
  const page = await db.read(api.thread, { roomId }, now);
  if (page.denied) throw refused(page.denied);
  const pins = await db.read(api.pinned, { roomId }, now);
  const typing = await db.read(api.typingNow, { roomId }, now);
  const rows = [...(page.data ?? [])].sort((a, b) => a.at - b.at || a.id.localeCompare(b.id));
  const messages = rows.map((row, i) => message(row, i > 0 ? rows[i - 1]! : null, me, who, custom, offset));
  const since = info.data?.lastRead?.at ?? 0;
  const members = info.data?.members.map((m) => who.get(m.principal) ?? unknown(m.principal)) ?? [];
  const other = members.find((p) => p.principal !== me.principal);
  return { id: room.id, kind: room.kind, title: room.kind === 'dm' ? (other?.name ?? 'Direct message') : room.name, topic: room.topic,
    typing: typingLine(typing.data ?? [], me, who, now),
    unread: rows.filter((r) => r.at > since && r.authorId !== me.principal).length,
    messages,
    pins: (pins.data ?? []).flatMap((p) => p.message ? [{ id: p.message.id, author: (who.get(p.message.authorId) ?? unknown(p.message.authorId)).name, body: p.message.body, at: p.message.at }] : []),
    members: members.sort((a, b) => a.name.localeCompare(b.name)) };
}

// Live: Snapback's change feed, GET /changes?since&wait (the long-poll its own
// clients hold), as an Exact long poll (`exactLongPoll`, LLP 1016.000 D8), which
// a stream resource holds without holding an answer: each change the persona can
// see moves `seq`, and the view reads `snapshot` again. The first ask's `since`
// is past any head, which Snapback answers at once with the head; the host then
// asks with `since` = that `seq`, held up to 25 s, and a reply at the same `seq`
// is a heartbeat. Natively the device serves one answer at a time, so the
// long-poll itself cannot be awaited in a source (snapback4/README.md, "Live
// updates").
function live(persona: string): Result<'live'> {
  return fetch(`${origin}/changes?since=${Number.MAX_SAFE_INTEGER}&wait=25`, {
    headers: { 'x-snapback-persona': persona },
    exactStream: (event: { type: string; data?: unknown }) => {
      if (event.type !== 'message' || typeof event.data !== 'string') return { seq: -1, state: event.type };
      try { return { seq: Number((JSON.parse(event.data) as { seq: number }).seq), state: 'open' }; }
      catch { return { seq: -1, state: 'malformed' }; }
    },
    exactLongPoll: { cursor: 'since', from: 'seq' },
  } as RequestInit) as unknown as Result<'live'>;
}

// A wake's outbox round for the persona the tag names. With the window shown
// (`open` names its persona) another persona's waits, registered again, for
// that persona's next open or a wake with no window; a failed round
// registers it again, which waits out the platform's backoff.
async function flushOutbox(tag: string, open: string, storage: Parameters<Answer>[3], native: Parameters<Answer>[4]): Promise<Result<'flushOutbox'>> {
  const persona = tag.replace(/^outbox:/, '');
  if (open !== '' && persona !== open) { await registerSync({ tag }); return { ok: true, sent: false }; }
  return personas.use(persona, storage, native, async (db) => {
    const round = await db.sync();
    lastOnline = round.ok;
    if (!round.ok && (await db.status()).queued?.length) await registerSync({ tag });
    return { ok: true, sent: round.ok };
  });
}

// A catch-up: one round, then the mentions of the persona newer than the mark
// kept on this device, the newest room's as one notification. The mark moves
// only once the answer is made, so a catch-up that never ends posts nothing
// and the next one finds the same messages.
const markFile = (persona: string) => `app:/data/mentions-${persona}.json`;
async function catchUpMentions(persona: string, storage: Parameters<Answer>[3], native: Parameters<Answer>[4]): Promise<Result<'catchUpMentions'>> {
  return personas.use(persona, storage, native, async (db) => {
    const round = await db.sync();
    lastOnline = round.ok;
    const who = await directory(db, 0);
    const me = who.get(`dev:${persona}`) ?? unknown(`dev:${persona}`);
    let mark = 0;
    try { mark = Number(JSON.parse(new TextDecoder().decode(await storage.fs.readFile(markFile(persona)))).at) || 0; } catch { /* none yet */ }
    const found = await db.read(api.search, { q: me.handle }, 0);
    if (found.denied) throw refused(found.denied);
    const hits = (found.data ?? []).filter((m) => m.at > mark && m.authorId !== me.principal && mentions(m.body, me.handle)).sort((a, b) => b.at - a.at);
    const newest = hits[0];
    if (!newest) return { count: 0, roomId: '', title: '', body: '' };
    await storage.fs.atomicWriteFile(markFile(persona), new TextEncoder().encode(JSON.stringify({ at: newest.at })));
    const author = (who.get(newest.authorId) ?? unknown(newest.authorId)).name;
    const room = newest.room?.kind === 'dm' ? author : `#${newest.room?.name ?? ''}`;
    const inRoom = hits.filter((m) => m.roomId === newest.roomId).length;
    return { count: hits.length, roomId: newest.roomId, title: `${author} in ${room}`, body: inRoom > 1 ? `${newest.body} (+${inRoom - 1} more)` : newest.body };
  });
}

const sources: Sources = {
  live: ([persona, _epoch]) => live(persona),
  flushOutbox: ([tag, open], _store, storage, native) => flushOutbox(tag, open, storage, native),
  catchUpMentions: ([persona], _store, storage, native) => catchUpMentions(persona, storage, native),
  // Whether this build holds the test call's grant: a production build leaves the line out.
  testCallBuild: async () => ({ available: grants.split('\n').some((line) => line.startsWith('device.calls ')) }),
  // The test call's take, deleted once it has played back or the call ended early.
  discardTestCall: async ([path], _store, storage) => {
    try { await storage.fs.rm(path); } catch { /* already gone */ }
    return { path };
  },
  snapshot: ([persona, roomId, q, now, offset, _seq], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'snapshot'>> => {
    const round = unshown.delete(persona) ? { ok: lastOnline } : await db.sync();
    lastOnline = round.ok;
    // Writes still on this device go out at the next chance (LLP 1133.017 §11): one tag a persona.
    if (!round.ok && (await db.status()).queued?.length) await registerSync({ tag: `outbox:${persona}` });
    const who = await directory(db, now);
    const me = who.get(`dev:${persona}`) ?? unknown(`dev:${persona}`);
    const custom = await emojiTable(db, now);
    const { channels, dms } = await inboxOf(db, persona, who);
    const emoji = [...custom.values()].map((e) => ({ ...e, mine: false })).sort((a, b) => a.name.localeCompare(b.name));
    const owned = await db.read(api.customEmoji, {}, now);
    for (const e of emoji) e.mine = (owned.data ?? []).some((row) => row.id === e.id && row.createdBy === me.principal);
    let thread = emptyThread, threadMissing = false;
    if (roomId !== '') {
      try { thread = await threadOf(db, roomId, now, offset, me, who, custom); }
      catch (e) { if ((e as { code?: string }).code === 'NOT_MEMBER' || (e as { code?: string }).code === 'NOT_FOUND') threadMissing = true; else throw e; }
    }
    let hits: Result<'snapshot'>['hits'] = [];
    if (q !== '') {
      const found = await db.read(api.search, { q }, now);
      if (found.denied) throw refused(found.denied);
      hits = (found.data ?? []).map((row) => ({ id: row.id, roomId: row.roomId, roomKind: row.room?.kind ?? 'text', roomName: row.room?.name ?? '',
        author: (who.get(row.authorId) ?? unknown(row.authorId)).name, body: row.body, at: row.at }));
    }
    return { loaded: true, online: round.ok, people: [...who.values()].sort((a, b) => a.name.localeCompare(b.name)), channels, dms, emoji, thread, threadMissing, hits };
  }),

  // Writes are admitted, never awaited to the server: the device predicts
  // each one at once and the world's next read (every write's `refreshes`)
  // runs the round that delivers it. Natively a mutation that waited on a
  // round another answer started would be refused when that answer ended,
  // so no write waits on one; a device refusal (a `require`, a unique
  // clash it can see) is thrown here, a server refusal lands in `refusals()`.
  sendMessage: ([persona, roomId, body, replyToId], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'sendMessage'>> =>
    admit(shown(persona, await db.write(api.sendMessage, { roomId, body, replyToId: replyToId ?? null }, 0)))),
  editMessage: ([persona, messageId, body], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'editMessage'>> =>
    admit(shown(persona, await db.write(api.editMessage, { messageId, body }, 0)))),
  deleteMessage: ([persona, messageId], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'deleteMessage'>> =>
    admit(shown(persona, await db.write(api.deleteMessage, { messageId }, 0)))),
  react: ([persona, messageId, emoji], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'react'>> => {
    if (!UNICODE.includes(emoji) && !/^:[a-z0-9_]{2,32}:$/.test(emoji) && [...emoji].length > 4) throw refused({ code: 'E_INPUT', message: 'Not an emoji.' });
    return admit(shown(persona, await db.write(api.react, { messageId, emoji }, 0)));
  }),
  pin: ([persona, messageId], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'pin'>> =>
    admit(shown(persona, await db.write(api.pin, { messageId }, 0)))),
  setTyping: ([persona, roomId], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'setTyping'>> =>
    admit(await db.write(api.setTyping, { roomId }, 0))),
  markRead: ([persona, roomId], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'markRead'>> =>
    admit(await db.write(api.markRead, { roomId }, 0))),
  // The new room's id is the first the prediction minted.
  createChannel: ([persona, name, topic, category], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'createChannel'>> =>
    admit(await db.write(api.createChannel, { name, topic, category }, 0))),
  // An existing conversation is found on the device; only a new one is written.
  openDm: ([persona, other], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'openDm'>> => {
    const read = await db.read(api.inbox, {}, 0);
    if (read.denied) throw refused(read.denied);
    const existing = (read.data ?? []).find((e) => e.room?.kind === 'dm' && e.members.some((m) => m.principal === other));
    if (existing) return { ok: true, id: existing.roomId };
    return admit(await db.write(api.openDm, { other }, 0));
  }),
  setProfile: ([persona, name, handle, status, hue], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'setProfile'>> =>
    admit(await db.write(api.setProfile, { name, handle, status, hue }, 0))),
  createEmoji: ([persona, name, path, fill], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'createEmoji'>> =>
    admit(await db.write(api.createEmoji, { name, path, fill }, 0))),
  deleteEmoji: ([persona, emojiId], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'deleteEmoji'>> =>
    admit(await db.write(api.deleteEmoji, { emojiId }, 0))),
};

export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
