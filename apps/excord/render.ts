import { parse, jumbo } from './discord/markdown';
import type { Block as MarkdownBlock, Inline, Mark } from './discord/markdown';
import type { Result } from './app.contract.d.ts';
import type { Attachment, Message, Page, User } from './world';
import { emojiPath } from './fixture';

export type Row = Result<'loadWindow'>['rows'][number];
type Block = Row['blocks'][number];
type Run = Block['runs'][number];
type Media = Row['media'][number];
export interface Directories {
  me(): User; user(id: string): User | undefined; channelName(id: string): string; roleName(id: string): string;
  message(channel: string, id: string): Message | undefined; deleted(channel: string, id: string): boolean;
}
interface Context { dirs: Directories; block: number; spoiler: number; mentionsMe: boolean }
const displayName = (u: User): string => u.global_name || u.username;

function block(ctx: Context, kind: string, quote: boolean, level = 0, marker = ''): Block {
  return { id: `b${ctx.block++}`, kind, level, marker, quote, lang: '', text: '', runs: [] };
}
function run(id: string, kind: string, text: string, marks: Mark[], spoiler: string): Run {
  return { id, kind, text, bold: marks.includes('bold'), italic: marks.includes('italic'),
    underline: marks.includes('underline'), strike: marks.includes('strike'), spoiler,
    href: '', at: 0, format: '', self: false };
}

// Fixture slugs are not snowflakes; the parser accepts only numeric Discord ids.
function symbolic(inline: Inline): Inline[] {
  if (inline.kind !== 'text') return [inline];
  const result: Inline[] = [], pattern = /<(@!?|#|@&)([A-Za-z][\w-]*)>/g;
  let start = 0, match: RegExpExecArray | null;
  while ((match = pattern.exec(inline.text))) {
    if (match.index > start) result.push({ ...inline, text: inline.text.slice(start, match.index) });
    result.push({ kind: match[1] === '#' ? 'channel' : match[1] === '@&' ? 'role' : 'user', id: match[2], marks: inline.marks });
    start = match.index + match[0].length;
  }
  if (start < inline.text.length) result.push({ ...inline, text: inline.text.slice(start) });
  return result;
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
      for (const child of node.children) for (const part of symbolic(child)) visit(part, node.url, marks);
    } else if (node.kind === 'text' || node.kind === 'code') {
      emit(href ? 'link' : node.kind, node.text, marks, { href });
    } else if (node.kind === 'user') {
      const u = ctx.dirs.user(node.id), self = node.id === ctx.dirs.me().id;
      if (self) ctx.mentionsMe = true;
      emit('user', `@${u ? displayName(u) : node.id}`, marks, { self });
    } else if (node.kind === 'channel') emit('channel', `#${ctx.dirs.channelName(node.id)}`, marks);
    else if (node.kind === 'role') emit('role', `@${ctx.dirs.roleName(node.id)}`, marks);
    else if (node.kind === 'everyone') {
      ctx.mentionsMe = true; emit('everyone', `@${node.which}`, marks);
    } else if (node.kind === 'emoji') emit('emoji', `:${node.name}:`, marks, { href: emojiPath(node.name) });
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
  for (const node of nodes) for (const part of symbolic(node)) visit(part);
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
      if (node.kind === 'codeblock') { b.text = node.text; b.lang = node.lang; }
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
function media(a: Attachment): Media {
  const width = a.width ?? 0, height = a.height ?? 0;
  const scale = width > 0 && height > 0 ? Math.min(1, 320 / width, 320 / height) : 0;
  return { id: a.id, src: a.url, width, height, fitWidth: width * scale, fitHeight: height * scale,
    alt: a.filename, spoiler: a.spoiler };
}
function context(dirs: Directories): Context { return { dirs, block: 0, spoiler: 0, mentionsMe: false }; }

export function renderMessage(m: Message, previous: Message | undefined, dirs: Directories): Row {
  const ctx = context(dirs), parsed = parse(m.content), isJumbo = jumbo(parsed);
  let blocks = flatten(parsed, ctx);
  const text = plain(blocks);
  if (isJumbo && blocks.some(b => b.runs.some(r => r.kind === 'emoji'))) {
    const custom = block(ctx, 'emoji', false), unicode = block(ctx, 'paragraph', false);
    for (const b of blocks) for (const r of b.runs) (r.kind === 'emoji' ? custom : unicode).runs.push(r);
    blocks = [custom];
    if (unicode.runs.some(r => r.text.trim())) blocks.push(unicode);
  }
  const edited = m.edited_timestamp !== null;
  if (edited) {
    if (blocks[blocks.length - 1]?.kind !== 'paragraph') blocks.push(block(ctx, 'paragraph', false));
    const last = blocks[blocks.length - 1];
    last.runs.push(run(`${last.id}.r${last.runs.length}`, 'edited', ' (edited)', [], ''));
  }
  const mentionsMe = ctx.mentionsMe;
  const embeds: Row['embeds'] = m.embeds.map((e, i) => {
    const image = (kind: 'image' | 'thumbnail'): Media[] => e[kind] ? [media({ id: `e${i}.${kind}`,
      ...e[kind], filename: e.title ?? '', spoiler: false })] : [];
    return { id: `e${i}`, color: e.color === undefined ? '' : `#${(e.color & 0xffffff).toString(16).padStart(6, '0')}`,
      provider: e.provider?.name ?? '', author: e.author?.name ?? '', title: e.title ?? '', url: e.url ?? '',
      description: flatten(parse(e.description ?? ''), ctx), image: image('image'), thumbnail: image('thumbnail') };
  });
  const reply: Row['reply'] = [];
  if (m.message_reference) {
    const id = m.message_reference.message_id, ref = dirs.message(m.channel_id, id);
    reply.push({ id, author: ref ? displayName(ref.author) : '', tint: ref?.author.tint ?? 0,
      preview: ref ? plain(flatten(parse(ref.content), context(dirs))).replace(/\s+/g, ' ').trim().slice(0, 100) : '',
      state: ref ? 'present' : dirs.deleted(m.channel_id, id) ? 'deleted' : 'unknown' });
  }
  const author = displayName(m.author), system = m.type === 7 ? `${author} joined the server.` : '';
  const delta = previous ? m.timestamp - previous.timestamp : 0;
  return { key: m.nonce || m.id, id: m.id, authorId: m.author.id, author, tint: m.author.tint,
    initials: author.trim().split(/\s+/).slice(0, 2).map(s => Array.from(s)[0]).join('').toUpperCase(),
    bot: m.author.bot, at: m.timestamp, prevAt: previous?.timestamp ?? 0,
    grouped: !!previous && previous.author.id === m.author.id && delta >= 0 && delta < 7 * 60000 &&
      previous.type === 0 && m.type === 0 && !m.message_reference && previous.state !== 'failed',
    edited, mine: m.author.id === dirs.me().id, state: m.state, mentionsMe, jumbo: isJumbo,
    system, plain: system || text, source: m.content, isNew: false, blocks, media: m.attachments.map(media), embeds,
    reactions: m.reactions.map(r => ({ id: r.emoji.id || r.emoji.name, emoji: r.emoji.id ? '' : r.emoji.name,
      src: r.emoji.id ? emojiPath(r.emoji.name) : '', name: r.emoji.name, count: r.count, me: r.me })), reply };
}

export function createRenderer(dirs: Directories): (m: Message, previous?: Message) => Row {
  const cache = new Map<string, { signature: string; row: Row }>();
  return (m, previous) => {
    const ref = m.message_reference ? dirs.message(m.channel_id, m.message_reference.message_id) : undefined;
    const key = `${m.channel_id}:${m.id}`;
    const signature = `${m.revision}/${previous?.id}/${previous?.revision}/${ref?.id}/${ref?.revision}`;
    const found = cache.get(key);
    if (found?.signature === signature) return found.row;
    const row = renderMessage(m, previous, dirs);
    cache.set(key, { signature, row });
    if (cache.size > 1200) cache.delete(cache.keys().next().value!);
    return row;
  };
}

export type Window = Result<'loadWindow'>;
const HALF = 100;

// The transcript's window from a store's page. A row is drawn only with the
// message before it in hand, so its group and day divider are decided once: a
// first message whose predecessor the store does not hold is left out until a
// page holds both. `since` is the read position the channel opened at ("" for
// none): the first message after it carries the New divider.
export function windowOf(page: Page, channel: string, anchor: string, since: string,
  render: (m: Message, previous?: Message) => Row): Window {
  const whole = !!page.previous || !page.hasBefore;
  const held = whole ? page.messages : page.messages.slice(1);
  const before = whole ? page.previous : page.messages[0];
  const rows = held.map((m, i) => {
    const previous = i ? held[i - 1] : before;
    const row = render(m, previous);
    const isNew = since !== '' && m.id > since && (!previous || previous.id <= since);
    return isNew ? { ...row, isNew } : row;
  });
  return { channel, anchor, rows, hasBefore: page.hasBefore, hasAfter: page.hasAfter,
    firstId: rows[0]?.id ?? '', lastId: rows[rows.length - 1]?.id ?? '',
    pin: anchor || rows[Math.max(0, rows.length - HALF)]?.id || '' };
}
