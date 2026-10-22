// latest(channel, limit), around/before/after(channel, id, limit): oldest-first history pages, each
//   with the message before its first (`previous`) when the store holds it.
// channelList(): channel snapshots; me(), user(id), channelName(id), roleName(id): directories.
// message(channel, id), previous(channel, id): one record or its immediate predecessor (id or nonce);
//   deleted(channel, id): whether the store saw that message deleted, as opposed to never having held it.
// send(channel, content, replyTo, at): pending record; settle(): confirm or fail every pending send;
//   pendingSends(): how many sends await confirmation, in every channel.
// retry(channel, key), remove(channel, key), edit(channel, id, content), react(channel, id, emoji): writes.
// markRead(channel): read through the tail; inject(channel, what, near, at): scripted fixture traffic.
// createWorld(): an independent store; WORLD_NOW: the deterministic fixture epoch in milliseconds.
import { attachment, channelSpecs, emojiNames, generate, idAt, linkEmbed, members, WORLD_NOW } from './fixture';
import type { Result } from './app.contract.d.ts';
export { WORLD_NOW } from './fixture';

export interface User { id: string; username: string; global_name: string | null; bot: boolean; tint: number }
export interface Attachment { id: string; url: string; filename: string; width?: number; height?: number; spoiler: boolean }
export interface Embed {
  type: string; title?: string; description?: string; url?: string; color?: number;
  provider?: { name: string }; author?: { name: string };
  image?: { url: string; width?: number; height?: number };
  thumbnail?: { url: string; width?: number; height?: number };
}
export interface Message {
  id: string; channel_id: string; author: User; content: string; timestamp: number;
  edited_timestamp: number | null; attachments: Attachment[]; embeds: Embed[];
  reactions: { emoji: { id: string | null; name: string }; count: number; me: boolean }[];
  message_reference?: { message_id: string }; nonce?: string; type: number;
  state: 'sent' | 'pending' | 'failed'; revision: number;
}
export interface Page { messages: Message[]; previous?: Message; hasBefore: boolean; hasAfter: boolean }
// `arrived` counts messages created since the store opened; a deletion never lowers it.
interface Channel {
  messages: Message[]; version: number; lastRead: string; typing: string[]; failNext: boolean;
  arrived: number; deleted: Set<string>;
}

function lowerBound(messages: Message[], id: string): number {
  let low = 0, high = messages.length;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (messages[mid].id < id) low = mid + 1;
    else high = mid;
  }
  return low;
}
function indexOf(c: Channel, key: string): number {
  const i = lowerBound(c.messages, key);
  if (c.messages[i]?.id === key) return i;
  return c.messages.findIndex(m => m.nonce === key);
}
function limitOf(limit: number): number { return Number.isFinite(limit) ? Math.max(0, Math.floor(limit)) : 0; }

export function createWorld() {
  const channels = new Map<string, Channel>();
  let sequence = 20000;
  const me = () => members[0];
  const user = (id: string) => members.find(m => m.id === id);
  const channelName = (id: string) => channelSpecs.find(c => c.id === id)?.name ?? id;
  const roleName = (id: string) => ({ '201': 'Maintainers', '202': 'Cyclists' }[id] ?? id);
  function get(channel: string): Channel {
    let c = channels.get(channel);
    if (!c) {
      const g = generate(channel);
      c = { messages: g.messages, lastRead: g.lastRead, version: 1, typing: [], failNext: false, arrived: 0,
        deleted: new Set(g.deleted) };
      channels.set(channel, c);
    }
    return c;
  }
  function page(c: Channel, start: number, end: number): Page {
    start = Math.max(0, Math.min(start, c.messages.length));
    end = Math.max(start, Math.min(end, c.messages.length));
    return { messages: c.messages.slice(start, end), previous: start > 0 ? c.messages[start - 1] : undefined,
      hasBefore: start > 0, hasAfter: end < c.messages.length };
  }
  function latest(channel: string, limit: number): Page {
    const c = get(channel);
    return page(c, c.messages.length - limitOf(limit), c.messages.length);
  }
  function around(channel: string, id: string, limit: number): Page {
    const c = get(channel), found = indexOf(c, id), size = limitOf(limit);
    const i = found >= 0 ? found : Math.min(lowerBound(c.messages, id), c.messages.length - 1);
    return page(c, i - Math.floor(size / 2), i + Math.ceil(size / 2));
  }
  function before(channel: string, id: string, limit: number): Page {
    const c = get(channel), found = indexOf(c, id);
    const i = found >= 0 ? found : lowerBound(c.messages, id);
    return page(c, i - limitOf(limit), i);
  }
  function after(channel: string, id: string, limit: number): Page {
    const c = get(channel), found = indexOf(c, id);
    const i = found >= 0 ? found + 1 : lowerBound(c.messages, id);
    return page(c, i, i + limitOf(limit));
  }
  function message(channel: string, key: string): Message | undefined {
    const c = get(channel);
    return c.messages[indexOf(c, key)];
  }
  function previous(channel: string, key: string): Message | undefined {
    const c = get(channel), i = indexOf(c, key);
    return i > 0 ? c.messages[i - 1] : undefined;
  }
  function deleted(channel: string, id: string): boolean { return get(channel).deleted.has(id); }
  function drop(c: Channel, i: number): void {
    const [m] = i >= 0 ? c.messages.splice(i, 1) : [];
    if (m) c.deleted.add(m.id);
  }
  function requireMessage(channel: string, key: string): Message {
    const m = message(channel, key);
    if (!m) throw new Error('Message no longer exists.');
    return m;
  }
  function own(m: Message): void {
    if (m.author.id !== me().id) throw new Error('Only your own messages can be changed.');
  }
  function touch(c: Channel, m: Message): void { m.revision++; c.version++; }
  function append(c: Channel, channel: string, author: User, content: string, at: number): Message {
    if (!Number.isFinite(at) || at < Date.UTC(2015, 0, 1)) throw new Error('Invalid message time.');
    sequence += 2;
    const m: Message = { id: idAt(at, sequence), channel_id: channel, author, content, timestamp: at,
      edited_timestamp: null, attachments: [], embeds: [], reactions: [], type: 0, state: 'sent', revision: 0 };
    c.messages.splice(lowerBound(c.messages, m.id), 0, m);
    c.arrived++;
    c.typing = c.typing.filter(id => id !== author.id);
    return m;
  }
  function send(channel: string, content: string, replyTo: string, at: number): Message {
    if (!content.trim()) throw new Error('A message needs some text.');
    const c = get(channel), m = append(c, channel, me(), content, at);
    m.nonce = m.id;
    m.state = 'pending';
    if (replyTo) { m.message_reference = { message_id: replyTo }; m.type = 19; }
    c.version++;
    return m;
  }
  function settle(): void {
    for (const c of channels.values()) settleChannel(c);
  }
  function settleChannel(c: Channel): void {
    if (!c.messages.some(m => m.state === 'pending')) return;
    for (const m of c.messages) {
      if (m.state !== 'pending') continue;
      if (c.failNext) { m.state = 'failed'; c.failNext = false; }
      else {
        const oldId = m.id;
        // Reserve the adjacent sequence at send time; confirmation cannot reorder a row.
        m.id = oldId.slice(0, 11) + String(Number(oldId.slice(11)) + 1).padStart(8, '0');
        m.state = 'sent';
        if (c.lastRead === oldId) c.lastRead = m.id;
        for (const reply of c.messages) if (reply.message_reference?.message_id === oldId) {
          reply.message_reference.message_id = m.id;
          reply.revision++;
        }
      }
      m.revision++;
    }
    c.version++;
  }
  function retry(channel: string, key: string): void {
    const m = requireMessage(channel, key); own(m);
    if (m.state !== 'failed') throw new Error('Only a failed send can be retried.');
    m.state = 'pending'; touch(get(channel), m);
  }
  function remove(channel: string, key: string): void {
    const m = requireMessage(channel, key); own(m);
    const c = get(channel);
    drop(c, indexOf(c, key)); c.version++;
  }
  function edit(channel: string, id: string, content: string): void {
    const m = requireMessage(channel, id); own(m);
    if (!content.trim()) throw new Error('A message needs some text.');
    m.content = content; m.edited_timestamp = Math.max(WORLD_NOW, m.timestamp + 1, (m.edited_timestamp ?? 0) + 1);
    touch(get(channel), m);
  }
  function react(channel: string, id: string, emoji: string): void {
    const m = requireMessage(channel, id);
    if (!emoji.trim()) throw new Error('Choose a reaction.');
    const custom = /^(?:<a?:)?([A-Za-z0-9_]+):(\d+)>?$/.exec(emoji);
    const known = emojiNames[Number(emoji) - 500];
    const emojiId = custom?.[2] ?? (known ? emoji : null), name = custom?.[1] ?? known ?? emoji;
    const reaction = m.reactions.find(r => emojiId ? r.emoji.id === emojiId : !r.emoji.id && r.emoji.name === name);
    if (reaction) {
      reaction.count += reaction.me ? -1 : 1; reaction.me = !reaction.me;
      if (!reaction.count) m.reactions.splice(m.reactions.indexOf(reaction), 1);
    } else m.reactions.push({ emoji: { id: emojiId, name }, count: 1, me: true });
    touch(get(channel), m);
  }
  function markRead(channel: string): void {
    const c = get(channel); c.lastRead = c.messages[c.messages.length - 1]?.id ?? ''; c.version++;
  }
  function inject(channel: string, what: string, near: string, at: number): void {
    const c = get(channel);
    if (what === 'fail-next') c.failNext = true;
    else if (what === 'typing') {
      const id = c.typing.includes('101') ? '102' : '101';
      if (!c.typing.includes(id)) c.typing.push(id);
    } else if (what === 'typing-stop') c.typing = [];
    else if (what === 'delete-unread') {
      const i = lowerBound(c.messages, c.lastRead) + 1;
      if (!c.messages[i]) throw new Error('Nothing is unread.');
      drop(c, i);
    }
    else if (what === 'message' || what === 'burst') {
      for (let i = 0; i < (what === 'burst' ? 6 : 1); i++) {
        const author = user(c.typing[0]) ?? members[1 + i % 3];
        append(c, channel, author, ['The new build is ready to try.', 'The anchor held on my phone.',
          'Nice! Checking a longer thread next.', 'I added a photo to the reproduction.',
          'Works after rotating, too.', 'Thanks for checking it!'][i], at + i * 500);
      }
    } else {
      if (!['edit-above', 'delete-above', 'react-above', 'embed-above'].includes(what)) throw new Error(`Unknown fixture event: ${what}`);
      const found = near ? indexOf(c, near) : -1;
      const i = Math.max(0, found >= 0 ? found - 3 : c.messages.length - 11), m = c.messages[i];
      if (!m) throw new Error('No message to change.');
      if (what === 'delete-above') drop(c, i);
      else {
        if (what === 'edit-above') {
          m.content += '\nA longer update from the device:\nThe image finished decoding.\nThe keyboard opened and closed.\nThe visible row kept its offset.';
          m.edited_timestamp = at;
        } else if (what === 'react-above') {
          const r = m.reactions.find(r => !r.emoji.id && r.emoji.name === '✨');
          if (r) r.count++; else m.reactions.push({ emoji: { id: null, name: '✨' }, count: 1, me: false });
        } else {
          const embed = linkEmbed(2);
          const photo = attachment(1);
          embed.image = { url: photo.url, width: photo.width, height: photo.height };
          m.embeds.push(embed);
        }
        m.revision++;
      }
    }
    c.version++;
  }
  function channelList(): Result<'gatewayState'>['channels'] {
    return channelSpecs.map(spec => {
      const c = get(spec.id), start = lowerBound(c.messages, c.lastRead);
      const unread = c.messages.slice(start + (c.messages[start]?.id === c.lastRead ? 1 : 0));
      const names = c.typing.map(id => user(id)?.global_name ?? id);
      return { id: spec.id, name: spec.name, topic: spec.topic, version: c.version, total: c.messages.length,
        unread: unread.length, mentions: unread.filter(m => /<@!?me>|@(?:everyone|here)\b/.test(m.content)).length,
        firstUnread: unread[0]?.id ?? '', firstUnreadAt: unread[0]?.timestamp ?? 0, lastRead: c.lastRead, arrived: c.arrived,
        typing: names.length === 0 ? '' : names.length === 1 ? `${names[0]} is typing…` : `${names.join(' and ')} are typing…` };
    });
  }
  function pendingSends(): number {
    let n = 0;
    for (const c of channels.values()) for (const m of c.messages) n += Number(m.state === 'pending');
    return n;
  }
  return { latest, around, before, after, channelList, me, user, channelName, roleName, message, previous, deleted, pendingSends,
    send, settle, retry, remove, edit, react, markRead, inject };
}

export const { latest, around, before, after, channelList, me, user, channelName, roleName, message, previous, deleted, pendingSends,
  send, settle, retry, remove, edit, react, markRead, inject } = createWorld();
