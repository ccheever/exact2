// A store row (a Discord message, `discord/store.ts`) as the transcript's `Message` (shapes.contract):
// Markdown parsed and flattened into blocks of runs, mentions resolved, images in their declared box.
import { parse, jumbo } from './discord/markdown';
import { highlight } from './discord/highlight';
import { outsideCode } from './discord/markup';
import type { Block as MarkdownBlock, Inline, Mark } from './discord/markdown';
import { cmpId, msFromSnowflake } from './discord/ids';
import type { Result } from './app.contract.d.ts';

export type Row = Result<'loadWindow'>['rows'][number];
type Block = Row['blocks'][number];
type Run = Block['runs'][number];
type Media = Row['media'][number];
type Embed = Row['embeds'][number];
type Poll = Row['poll'][number];
// A Discord message as the store keeps it: its `key` (the nonce of a send from here) and `status`.
export type Stored = Record<string, any> & { id: string | null; key: string; channel_id: string; status: string };
export interface User { id: string; username?: string; global_name?: string | null; bot?: boolean; avatar?: string | null; discriminator?: string }

export interface Directories {
  me(): string;
  user(id: string): User | null;
  channelName(id: string): string;
  /** The guild a channel is in ("" for a DM or one the store does not hold). */
  channelGuild(id: string): string;
  /** Whether the reader can open a channel: the store holds it and the account can see it. */
  canOpen(id: string): boolean;
  roleName(id: string): string;
  message(channel: string, id: string): Stored | undefined;
  deleted(channel: string, id: string): boolean;
  emoji(id: string, animated: boolean): string;
  sticker(id: string, extension: string): string;
  /** The user's picture: their avatar, else Discord's default one for them. */
  avatar(user: User): string;
  /** The name colour, `#rrggbb`, of the highest of a channel's guild roles that has one; "" for none. */
  nameColor(channel: string, roles: string[]): string;
  /** What `nameColor` reads for a channel, which the store replaces when a role changes. */
  roles(channel: string): unknown;
}
interface Context { dirs: Directories; block: number; spoiler: number; mentionsMe: boolean }
const displayName = (u: User | undefined | null, fallback = ''): string => u?.global_name || u?.username || fallback;
/** A user's picture on Discord's CDN: their avatar, else the default one Discord gives them (its index is
 * the legacy discriminator modulo 5, or for a unique username the id's timestamp bits modulo 6). */
export function avatarUrl(cdn: string, u: User): string {
  if (u.avatar && /^\d+$/.test(u.id)) return `${cdn}/avatars/${u.id}/${u.avatar}.png?size=128`;
  const legacy = Number(u.discriminator ?? 0);
  const index = legacy > 0 ? legacy % 5 : /^\d+$/.test(u.id) ? (msFromSnowflake(u.id) - 1420070400000) % 6 : 0;
  return `${cdn}/embed/avatars/${index}.png`;
}
export const timeOf = (m: Stored): number => (m.timestamp ? Date.parse(m.timestamp) : m.createdAtMs ?? 0);

function block(ctx: Context, kind: string, quote: boolean, level = 0, marker = ''): Block {
  return { id: `b${ctx.block++}`, kind, level, marker, quote, lang: '', text: '', runs: [] };
}
function run(id: string, kind: string, text: string, marks: Mark[], spoiler: string): Run {
  return { id, kind, text, bold: marks.includes('bold'), italic: marks.includes('italic'),
    underline: marks.includes('underline'), strike: marks.includes('strike'), spoiler,
    href: '', at: 0, format: '', self: false, ref: '' };
}

// A link to a Discord channel or message (`https://discord.com/channels/<guild or @me>/<channel>[/<message>]`)
// opens in the app when the reader can open the channel: its run's `ref` is `m:<guild>:<channel>:<message>` (""
// for a DM's guild or no message). One the account cannot open opens in the browser, as any link does.
const DISCORD_LINK = /^https:\/\/(?:(?:www|ptb|canary)\.)?discord(?:app)?\.com\/channels\/(@me|\d+)\/(\d+)(?:\/(\d+))?\/?(?:[?#].*)?$/i;
function messageLink(href: string, dirs: Directories): string {
  const m = DISCORD_LINK.exec(href);
  if (!m || !dirs.canOpen(m[2])) return '';
  return `m:${dirs.channelGuild(m[2])}:${m[2]}:${m[3] ?? ''}`;
}

function inlines(nodes: Inline[], b: Block, ctx: Context): void {
  let activeSpoiler = '';
  function emit(kind: string, text: string, marks: Mark[], extra: Partial<Run> = {}): void {
    if (marks.includes('spoiler')) { if (!activeSpoiler) activeSpoiler = `s${ctx.spoiler++}`; }
    else activeSpoiler = '';
    b.runs.push({ ...run(`${b.id}.r${b.runs.length}`, kind, text, marks, activeSpoiler), ...extra });
  }
  function visit(node: Inline, href = '', inherited: Mark[] = []): void {
    if (node.kind === 'break') {
      const last = b.runs[b.runs.length - 1];
      if (last?.kind === 'text') last.text += '\n';
      else emit('text', '\n', inherited);
      return;
    }
    const marks = [...new Set([...inherited, ...node.marks])];
    if (node.kind === 'link') {
      for (const child of node.children) visit(child, node.url, marks);
    } else if (node.kind === 'text' || node.kind === 'code') {
      emit(href ? 'link' : node.kind, node.text, marks, { href, ref: href ? messageLink(href, ctx.dirs) : '' });
    } else if (node.kind === 'user') {
      const u = ctx.dirs.user(node.id), self = node.id === ctx.dirs.me();
      if (self) ctx.mentionsMe = true;
      emit('user', `@${displayName(u, node.id)}`, marks, { self, ref: `u:${node.id}` });
    } else if (node.kind === 'channel') {
      emit('channel', `#${ctx.dirs.channelName(node.id)}`, marks,
        { ref: ctx.dirs.canOpen(node.id) ? `c:${ctx.dirs.channelGuild(node.id)}:${node.id}` : '' });
    }
    else if (node.kind === 'role') emit('role', `@${ctx.dirs.roleName(node.id)}`, marks);
    else if (node.kind === 'everyone') {
      ctx.mentionsMe = true; emit('everyone', `@${node.which}`, marks);
    } else if (node.kind === 'emoji') emit('emoji', `:${node.name}:`, marks, { href: ctx.dirs.emoji(node.id, node.animated) });
    else {
      const at = node.unix * 1000, date = new Date(at);
      // A run's text is its plain form (Copy Text, a reply's preview); the view draws the instant.
      const valid = Number.isFinite(date.getTime());
      const day = valid ? date.toLocaleDateString('en-US', { dateStyle: 'long', timeZone: 'UTC' }) : String(node.unix);
      const time = valid ? `${date.toLocaleTimeString('en-US', { timeStyle: 'short', timeZone: 'UTC' })} UTC` : '';
      const formats: Record<string, string> = { t: 'time-short', T: 'time-medium', d: 'date-short', D: 'date-long', R: 'relative' };
      if (node.style === 'f' || node.style === 'F') {
        emit('time', day, marks, { at, format: node.style === 'F' ? 'date-full' : 'date-long' });
        emit('text', ' ', marks);
        emit('time', time, marks, { at, format: 'time-short' });
      } else emit('time', node.style === 't' || node.style === 'T' ? time : day, marks, { at, format: formats[node.style] ?? 'date-long' });
    }
  }
  for (const node of nodes) visit(node);
}

function flatten(nodes: MarkdownBlock[], ctx: Context, quote = false, depth = 0): Block[] {
  const out: Block[] = [];
  for (const node of nodes) {
    if (node.kind === 'quote') out.push(...flatten(node.blocks, ctx, true, depth));
    else if (node.kind === 'list') {
      node.items.forEach((children, i) => {
        let first = true;
        for (const child of children) {
          if (child.kind === 'paragraph') {
            const b = block(ctx, 'item', quote, depth, first ? node.ordered ? `${node.start + i}.` : '•' : '');
            inlines(child.inlines, b, ctx); out.push(b); first = false;
          } else out.push(...flatten([child], ctx, quote, child.kind === 'list' ? depth + 1 : depth));
        }
      });
    } else {
      const b = block(ctx, node.kind === 'codeblock' ? 'code' : node.kind, quote, node.kind === 'heading' ? node.level : 0);
      if (node.kind === 'codeblock') {
        b.text = node.text; b.lang = node.lang;
        // A known language's block is its tokens, each a run of kind `hl-<token>`.
        b.runs = (highlight(node.text, node.lang) ?? []).map((t, i) => run(`${b.id}.r${i}`, `hl-${t.kind}`, t.text, [], ''));
      }
      else inlines(node.inlines, b, ctx);
      out.push(b);
    }
  }
  return out;
}

function plain(blocks: Block[]): string {
  return blocks.map(b => (b.marker ? `${b.marker} ` : '') + (b.kind === 'code' ? b.text : b.runs
    .filter(r => r.kind !== 'edited').map(r => r.text).join(''))).join('\n');
}
interface Picture { url?: string; proxy_url?: string; width?: number; height?: number }
// Discord's media proxy resizes to the `width` and `height` asked: an image is fetched at the pixels
// of its box on a 3x display, not at the size it was uploaded.
const PIXELS = 3;
function media(id: string, p: Picture, alt: string, spoiler: boolean): Media {
  const width = p.width ?? 0, height = p.height ?? 0;
  const scale = width > 0 && height > 0 ? Math.min(1, 320 / width, 320 / height) : 0;
  const pixels = Math.min(1, scale * PIXELS);
  const src = p.proxy_url && /^https?:/.test(p.proxy_url) && scale > 0 && pixels < 1
    ? `${p.proxy_url}${p.proxy_url.includes('?') ? '&' : '?'}width=${Math.round(width * pixels)}&height=${Math.round(height * pixels)}`
    : p.url ?? p.proxy_url ?? '';
  return { id, src, full: p.url ?? p.proxy_url ?? '', width, height, fitWidth: width * scale, fitHeight: height * scale, alt, spoiler };
}
/** A file's size as Discord shows it: "820 bytes", "14.2 KB", "3.1 MB". */
export function byteSize(n: number): string {
  if (n < 1024) return `${n} bytes`;
  const [value, unit] = n < 1024 ** 2 ? [n / 1024, 'KB'] : n < 1024 ** 3 ? [n / 1024 ** 2, 'MB'] : [n / 1024 ** 3, 'GB'];
  return `${value.toFixed(value < 10 ? 1 : 0)} ${unit}`;
}
const isImage = (a: Record<string, any>): boolean =>
  String(a.content_type ?? '').startsWith('image/') || /\.(png|jpe?g|gif|webp)$/i.test(String(a.filename ?? ''));
function context(dirs: Directories): Context { return { dirs, block: 0, spoiler: 0, mentionsMe: false }; }

// A reply is type 19; a local send has no type until Discord answers it.
const isReply = (m: Stored): boolean => !!m.message_reference?.message_id && (m.type === 19 || m.type === undefined);

function replyOf(m: Stored, dirs: Directories): Row['reply'] {
  if (!isReply(m)) return [];
  const id = String(m.message_reference.message_id);
  const held = dirs.message(m.channel_id, id);
  // Discord sends the original with the reply (`referenced_message`), null when it was deleted.
  const ref = held ?? (m.referenced_message || undefined);
  const gone = m.referenced_message === null || dirs.deleted(m.channel_id, id);
  if (!ref || (gone && !held)) return [{ id, author: '', avatar: '', color: '', preview: '', state: gone ? 'deleted' : 'unknown' }];
  const author: User = dirs.user(ref.author?.id) ?? ref.author ?? { id: '' };
  return [{ id, author: displayName(author),
    avatar: dirs.avatar(author), color: dirs.nameColor(m.channel_id, ref.member?.roles ?? []),
    // A forward's own words first, then the message it forwards.
    preview: ([ref.message_snapshots?.[0]?.message ? ref.content : '', (ref.message_snapshots?.[0]?.message ?? ref).content]
      .map(words => plain(flatten(parse(String(words ?? '')), context(dirs)))).filter(Boolean).join(' ')
      || stickersOf(ref.message_snapshots?.[0]?.message ?? ref).map(stickerName).join(', ')
      || String((ref.message_snapshots?.[0]?.message ?? ref).poll?.question?.text ?? ''))
      .replace(/\s+/g, ' ').trim().slice(0, 100),
    state: 'present' }];
}

/** A message's text as the composer shows it for an edit: a mention of a person the directories know is
 * `@handle`, outside code, which `compose.ts` sends back as the same mention. A channel's or custom
 * emoji's stays as written, since a name may not lead back to the one it named (two channels called
 * #help; a server's emoji in a DM). */
function editable(content: string, dirs: Directories): string {
  return outsideCode(content, part => part.replace(/<@!?(\d+)>/g, (whole, id) => {
    const u = dirs.user(id);
    return u?.username ? `@${u.username}` : whole;
  }));
}

// A sticker's picture as Discord serves it, by its format: PNG and APNG as `.png`, GIF as `.gif`; a
// Lottie one (3) has none. Discord draws a sticker at 160 by 160.
const STICKER_EXTENSION: Record<number, string> = { 1: 'png', 2: 'png', 4: 'gif' };
const stickersOf = (m: Stored): Record<string, any>[] => Array.isArray(m.sticker_items) ? m.sticker_items : [];
const stickerName = (s: Record<string, any>): string => String(s.name ?? '') || 'Sticker';
// A system message's line and its icon. A closed poll's (46) names it from its `poll_result` embed.
const SYSTEM: Record<number, [string, (who: string, m: Stored) => string]> = {
  6: ['pin', who => `${who} pinned a message.`],
  7: ['forward', who => `${who} joined the server.`],
  46: ['chart', (who, m) => [`${who}'s poll`, pollResult(m, 'poll_question_text'), 'has closed.'].filter(Boolean).join(' ')],
};
const pollResult = (m: Stored, name: string): string => String((m.embeds ?? []).find((e: Record<string, any>) => e.type === 'poll_result')
  ?.fields?.find((f: Record<string, any>) => f.name === name)?.value ?? '');

// A poll (Discord's `poll`). Its counts are in `results`, which Discord leaves out until it has some; the
// most-voted answers of a finalized poll win.
function pollOf(m: Stored, dirs: Directories): Poll[] {
  const p = m.poll;
  if (!p || typeof p !== 'object') return [];
  const counts = new Map<string, Record<string, any>>((p.results?.answer_counts ?? []).map((a: Record<string, any>) => [String(a.id), a]));
  const answers = (Array.isArray(p.answers) ? p.answers : []).map((a: Record<string, any>) => {
    const id = String(a.answer_id), e = a.poll_media?.emoji, n = counts.get(id);
    return { id, text: String(a.poll_media?.text ?? ''), emoji: e && !e.id ? String(e.name ?? '') : '',
      src: e?.id ? dirs.emoji(String(e.id), e.animated === true) : '', count: Number(n?.count ?? 0), me: n?.me_voted === true };
  });
  const votes = answers.reduce((sum: number, a: { count: number }) => sum + a.count, 0);
  const top = Math.max(0, ...answers.map((a: { count: number }) => a.count)), closed = p.results?.is_finalized === true;
  return [{ question: String(p.question?.text ?? ''), multi: p.allow_multiselect === true, known: !!p.results && p.results.partial !== true,
    expiry: p.expiry ? Date.parse(p.expiry) || 0 : 0, closed, voted: answers.some((a: { me: boolean }) => a.me), votes,
    answers: answers.map((a: Record<string, any>) => ({ ...a, percent: votes ? Math.round(100 * a.count / votes) : 0,
      winner: closed && top > 0 && a.count === top })) }];
}

export function renderMessage(m: Stored, previous: Stored | undefined, dirs: Directories): Row {
  // A forward (Discord's `message_snapshots`) carries no words of its own: what shows is the forwarded
  // message, under a Forwarded label.
  const snapshot: Stored | undefined = m.message_snapshots?.[0]?.message;
  const shown: Stored = snapshot ? { ...snapshot, key: m.key, id: m.id, channel_id: m.channel_id, status: m.status } : m;
  const stickers = stickersOf(shown), lottie = stickers.filter(s => !STICKER_EXTENSION[s.format_type]);
  // A Lottie sticker's name follows the text in the message's own size, so its emoji are not jumbo.
  const ctx = context(dirs), content = String(shown.content ?? ''), parsed = parse(content), isJumbo = jumbo(parsed) && !lottie.length;
  // A jumbo message's custom emoji are images in its paragraph, in order with its Unicode ones.
  const blocks = flatten(parsed, ctx);
  // A forward's own words, if it has any, show above its Forwarded label; only they and the forward's own
  // mentions can mention the reader (Discord does not for the forwarded message's).
  // They share the message's context so its block and spoiler ids stay distinct (a spoiler in each is
  // revealed apart); only the reader's mention is counted afresh.
  let ownMentionsMe = ctx.mentionsMe, note: Block[] = [];
  if (snapshot) { ctx.mentionsMe = false; note = flatten(parse(String(m.content ?? '')), ctx); ownMentionsMe = ctx.mentionsMe; }
  // The message as words: its text, then its stickers' names (a sticker alone reads as its name).
  const poll = pollOf(shown, dirs);
  const text = [plain(note), plain(blocks), stickers.map(stickerName).join(', '), poll[0]?.question ?? ''].filter(Boolean).join('\n');
  const edited = shown.edited_timestamp != null;
  if (edited) {
    if (blocks[blocks.length - 1]?.kind !== 'paragraph') blocks.push(block(ctx, 'paragraph', false));
    const last = blocks[blocks.length - 1];
    last.runs.push(run(`${last.id}.r${last.runs.length}`, 'edited', ' (edited)', [], ''));
  }
  // A Lottie sticker is an animation nothing here plays: its name stands in for it, as words, never markup.
  for (const s of lottie) {
    const b = block(ctx, 'paragraph', false);
    b.runs.push(run(`${b.id}.r0`, 'text', stickerName(s), ['italic'], ''));
    blocks.push(b);
  }
  const me = dirs.me(), authorId = String(m.author?.id ?? '');
  const mentionsMe = ownMentionsMe || (authorId !== me && (m.mention_everyone === true
    || (m.mentions ?? []).some((u: User) => u.id === me)));
  // A closed poll's result card is drawn as its system line.
  const embeds: Embed[] = (shown.embeds ?? []).filter((e: Record<string, any>) => e.type !== 'poll_result').map((e: Record<string, any>, i: number) => {
    const image = (kind: 'image' | 'thumbnail'): Media[] => e[kind] ? [media(`e${i}.${kind}`, e[kind], e.title ?? '', false)] : [];
    // An image or video link's embed is its picture alone, as Discord draws it.
    const bare = e.type === 'image' || e.type === 'gifv' || e.type === 'video';
    return { id: `e${i}`, color: e.color == null ? '' : `#${(e.color & 0xffffff).toString(16).padStart(6, '0')}`,
      provider: e.provider?.name ?? '', author: e.author?.name ?? '', title: e.title ?? '', url: e.url ?? '',
      description: flatten(parse(e.description ?? ''), ctx),
      image: bare ? (e.image ? image('image') : image('thumbnail')) : image('image'), thumbnail: bare ? [] : image('thumbnail') };
  });
  const user: User = dirs.user(authorId) ?? m.author ?? { id: authorId };
  const author = displayName(user, authorId);
  const [systemIcon, line] = SYSTEM[m.type] ?? ['', () => ''];
  const system = line(author, m);
  const at = timeOf(m), prevAt = previous ? timeOf(previous) : 0, delta = previous ? at - prevAt : 0;
  return { key: m.key, id: m.id ?? m.key, authorId, author,
    avatar: dirs.avatar(user), color: dirs.nameColor(m.channel_id, m.member?.roles ?? []),
    bot: m.author?.bot === true, at, prevAt,
    grouped: !!previous && String(previous.author?.id) === authorId && delta >= 0 && delta < 7 * 60000 &&
      (previous.type ?? 0) === 0 && (m.type ?? 0) === 0 && !isReply(m) && previous.status !== 'failed',
    edited, mine: authorId === me, state: m.status, mentionsMe, jumbo: isJumbo,
    // A forward is never edited: it has no words of its own to edit.
    system, systemIcon, plain: system || text, source: snapshot ? '' : editable(content, dirs), isNew: false, blocks,
    forwarded: !!snapshot, note,
    media: (shown.attachments ?? []).filter(isImage).map((a: Record<string, any>) =>
      media(String(a.id), a, String(a.filename ?? ''), String(a.filename ?? '').startsWith('SPOILER_'))),
    stickers: stickers.filter(s => STICKER_EXTENSION[s.format_type]).map(s =>
      ({ id: String(s.id), src: dirs.sticker(String(s.id), STICKER_EXTENSION[s.format_type]), name: String(s.name ?? '') })),
    files: (shown.attachments ?? []).filter((a: Record<string, any>) => !isImage(a)).map((a: Record<string, any>) =>
      ({ id: String(a.id), name: String(a.filename ?? 'file'), size: byteSize(Number(a.size ?? 0)), url: String(a.url ?? ''),
        spoiler: String(a.filename ?? '').startsWith('SPOILER_') })),
    embeds,
    reactions: (m.reactions ?? []).map((r: Record<string, any>) => ({ id: r.emoji.id || r.emoji.name,
      emoji: r.emoji.id ? '' : r.emoji.name, src: r.emoji.id ? dirs.emoji(r.emoji.id, r.emoji.animated === true) : '',
      name: r.emoji.name, count: r.count, me: r.me === true })),
    poll, reply: replyOf(m, dirs) };
}

/** A renderer that redraws a row only when its message, its predecessor, its reply's original (or its
 * deletion), its author or its guild's roles changed: the store replaces each of these objects whenever it
 * changes it. */
export function createRenderer(dirs: Directories): (m: Stored, previous?: Stored) => Row {
  const cache = new Map<string, { m: Stored; previous?: Stored; ref?: Stored; gone: boolean; roles: unknown; author: User | null; row: Row }>();
  return (m, previous) => {
    const refId = m.message_reference?.message_id;
    const ref = refId ? dirs.message(m.channel_id, String(refId)) : undefined;
    const gone = refId ? dirs.deleted(m.channel_id, String(refId)) : false;
    const roles = dirs.roles(m.channel_id), author = dirs.user(String(m.author?.id ?? ''));
    const key = `${m.channel_id}:${m.key}`, found = cache.get(key);
    if (found && found.m === m && found.previous === previous && found.ref === ref && found.gone === gone
      && found.roles === roles && found.author === author) return found.row;
    const row = renderMessage(m, previous, dirs);
    cache.delete(key);
    cache.set(key, { m, previous, ref, gone, roles, author, row });
    if (cache.size > 1200) cache.delete(cache.keys().next().value!);
    return row;
  };
}

export type Window = Result<'loadWindow'>;
const HALF = 100;
export interface Slice { rows: Stored[]; previous?: Stored; hasBefore: boolean; hasAfter: boolean }
export interface Unread { count: number; more: boolean; at: number }

// The transcript's window from a run of the store. A row is drawn only with the message before it
// in hand, so its group and day divider are decided once: a first message whose predecessor the
// store does not hold is left out until a page holds both. `since` is the read position the channel
// opened at ("" for none): the first message after it carries the New divider.
export function windowOf(slice: Slice, channel: string, anchor: string, since: string, unread: Unread,
  render: (m: Stored, previous?: Stored) => Row): Window {
  const whole = !!slice.previous || !slice.hasBefore;
  const held = whole ? slice.rows : slice.rows.slice(1);
  const before = whole ? slice.previous : slice.rows[0];
  const rows = held.map((m, i) => {
    const previous = i ? held[i - 1] : before;
    const row = render(m, previous);
    const isNew = since !== '' && m.id !== null && cmpId(m.id, since) > 0
      && (!previous || previous.id === null || cmpId(previous.id, since) <= 0);
    return isNew ? { ...row, isNew } : row;
  });
  return { channel, anchor, rows, hasBefore: slice.hasBefore, hasAfter: slice.hasAfter,
    firstId: rows[0]?.id ?? '', lastId: rows[rows.length - 1]?.id ?? '',
    pin: anchor || rows[Math.max(0, rows.length - HALF)]?.id || '',
    unread: unread.count, unreadMore: unread.more, unreadAt: unread.at, need: '', cursor: '' };
}
