// T3 Code 5d3d78ab93 (MIT; LICENSE-T3), markdownPipeline.ts: image sources kept by the sanitizer. This indexes the
// HTML images markdown-parse draws (its html.rs and entities.rs; LLP 1033 D9): image blocks, and the media run of an
// image drawn in a line (markdown_media.rs inline_html_image): inside a line of text, beside other images on one line
// box (markdown-parse html.rs `Line`), or in an `<a href>`. It never inserts HTML into a host view.
import { LEGACY, NAMED } from './html-entities';

/** `inline`: a media run, whose href is its light source's; else an image block. `link`: the `<a href>` around it. */
export type HtmlImage = { alt: string; src: string; dark: string; light: string; width: number; height: number; link: string; offset: number; inline: boolean };
type Tag = { name: string; closing: boolean; attributes: Map<string, string>; end: number };
type Picture = { first: number; authored: [boolean, boolean]; dark: string | null; light: string | null };
type Open = { name: string; picture: Picture | null; link: string | null };
/** One line box of a markup's images (`images[start..end]`): `continues`, no `<p>`, `<div>` or `<br>` before it in
 *  the markup; `open`, none after it either. */
type Line = { start: number; end: number; continues: boolean; open: boolean };
type Markup = { images: HtmlImage[]; lines: Line[]; end: number; open: string[] };
const SPACE = /[\t\n\f\r ]/;
const SCHEMES = ['http', 'https', 'file', 't3-context'];
// hast-util-sanitize's `href` protocols and ChatMarkdown's own (markdownPipeline.ts).
const LINK_SCHEMES = ['http', 'https', 'irc', 'ircs', 'mailto', 'xmpp', 'file', 't3-citation', 't3-context', 't3-thread'];
// The elements an image may sit in that draw nothing of their own, those whose start tag closes an open `<p>`, and the
// blocks of their own after the sanitizer (it drops `<center>` and `<figure>` and keeps what they hold).
const WRAPPERS = ['div', 'p', 'center', 'figure', 'strong', 'em'];
const CLOSES_P = ['div', 'p', 'center', 'figure'];
const BLOCKS = ['div', 'p'];
const LIMIT = 8192;
// The windows-1252 characters a numeric reference to 0x80–0x9F stands for.
const C1 = [0x20ac, 0x81, 0x201a, 0x192, 0x201e, 0x2026, 0x2020, 0x2021, 0x2c6, 0x2030, 0x160, 0x2039, 0x152, 0x8d, 0x17d, 0x8f,
  0x90, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x2dc, 0x2122, 0x161, 0x203a, 0x153, 0x9d, 0x17e, 0x178];

function lookup(name: string): string | null {
  const key = `&${name};`, at = NAMED.indexOf(key);
  if (at < 0) return null;
  const start = at + key.length, first = String.fromCodePoint(NAMED.codePointAt(start)!).length;
  return NAMED.slice(start, NAMED.indexOf('&', start + first));
}
/** A named reference after its `&`, and how much of `text` it used. */
function named(text: string): [string, number] | null {
  const run = /^[A-Za-z\d]*/.exec(text)![0].length;
  if (!run) return null;
  const full = text[run] === ';' ? lookup(text.slice(0, run)) : null;
  if (full !== null) return [full, run + 1];
  // The longest legacy name the run starts with, which an `=` or an alphanumeric after it leaves as text.
  const name = LEGACY.split(' ').filter(legacy => legacy && text.startsWith(legacy)).reduce((a, b) => (b.length > a.length ? b : a), '');
  if (!name || name.length < run || text[run] === '=') return null;
  const value = lookup(name);
  return value === null ? null : [value, name.length];
}
/** A numeric reference after its `&#`, and how much of `text` it used. */
function numeric(text: string): [string, number] | null {
  const hex = text[0] === 'x' || text[0] === 'X', lead = hex ? 1 : 0;
  const digits = (hex ? /^[\da-fA-F]*/ : /^\d*/).exec(text.slice(lead))![0];
  if (!digits) return null;
  let code = 0;
  for (const digit of digits) code = Math.min(code * (hex ? 16 : 10) + parseInt(digit, 16), 0x110000);
  code = code >= 0x80 && code <= 0x9f ? C1[code - 0x80]! : code === 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff) ? 0xfffd : code;
  return [String.fromCodePoint(code), lead + digits.length + (text[lead + digits.length] === ';' ? 1 : 0)];
}
/** An attribute value's character references, decoded as an HTML tokenizer decodes them (markdown-parse entities.rs). */
export function decode(value: string): string {
  if (!value.includes('&')) return value;
  let out = '', at = 0;
  for (let amp = value.indexOf('&'); amp >= 0; amp = value.indexOf('&', at)) {
    out += value.slice(at, amp);
    const rest = value.slice(amp + 1), number = rest[0] === '#';
    const reference = number ? numeric(rest.slice(1)) : named(rest);
    if (reference) { out += reference[0]; at = amp + 1 + (number ? 1 : 0) + reference[1]; } else { out += '&'; at = amp + 1; }
  }
  return out + value.slice(at);
}
const drivePath = (url: string) => /^[A-Za-z]:[/\\]/.test(url);
/** hast-util-sanitize's safeProtocol over ChatMarkdown's image protocols, or `schemes`. */
function protocolAllowed(url: string, schemes = SCHEMES): boolean {
  const colon = url.indexOf(':');
  return colon < 0 || /[/?#]/.test(url.slice(0, colon)) || schemes.includes(url.slice(0, colon));
}
/** JavaScript's `Number(value)` for the plain decimal numbers HTML authors write, kept when positive, rounded. */
function pixels(value: string | undefined): number {
  const parsed = value === undefined || !/^\s*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?\s*$/.test(value) ? NaN : Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? Math.min(Math.round(parsed), 0xffffffff) : 0;
}
// A drive path in `src` is kept in `dataLocalSrc`; one in a `<source>` becomes a `file:` URL before the sanitizer.
const allowed = (url: string) => drivePath(url) || protocolAllowed(url);
const sourceAllowed = (url: string) => (drivePath(url) ? SCHEMES.includes('file') : protocolAllowed(url));
function tag(text: string, at: number): Tag | null {
  const start = /^<(\/?)([a-z][a-z\d]*)(?=[\s/>])/i.exec(text.slice(at));
  if (!start) return null;
  const attributes = new Map<string, string>();
  let cursor = at + start[0].length;
  while (cursor < text.length) {
    while (SPACE.test(text[cursor] ?? '') || text[cursor] === '/') cursor++;
    if (text[cursor] === '>') return { name: start[2]!.toLowerCase(), closing: !!start[1], attributes, end: cursor + 1 };
    const name = /^[^\s=/>]+/.exec(text.slice(cursor))?.[0];
    if (!name) return null;
    cursor += name.length;
    while (SPACE.test(text[cursor] ?? '')) cursor++;
    let value = '';
    if (text[cursor] === '=') {
      cursor++;
      while (SPACE.test(text[cursor] ?? '')) cursor++;
      const quote = text[cursor];
      if (quote === '"' || quote === "'") {
        const end = text.indexOf(quote, cursor + 1);
        if (end < 0) return null;
        value = text.slice(cursor + 1, end); cursor = end + 1;
      } else {
        value = /^[^\s>]+/.exec(text.slice(cursor))?.[0] ?? '';
        cursor += value.length;
      }
    }
    if (!attributes.has(name.toLowerCase())) attributes.set(name.toLowerCase(), decode(value));
  }
  return null;
}
/** An `<img>`: `data-local-src` stands in for `src` (a drive path in `src` wins), `data-dark-src` and `data-light-src` are
 *  its theme sources; and whether it has each of those two. */
function image(node: Tag, offset: number, inline: boolean): [HtmlImage, [boolean, boolean]] {
  const attribute = (name: string) => node.attributes.get(name), own = attribute('src'), local = attribute('data-local-src');
  const src = own !== undefined && drivePath(own) ? own : local ?? own ?? '';
  const themed = (name: string) => { const url = attribute(name); return url === undefined ? undefined : protocolAllowed(url) ? url : ''; };
  const dark = themed('data-dark-src'), light = themed('data-light-src');
  return [{ alt: attribute('alt') ?? '', src: allowed(src) ? src : '', dark: dark ?? '', light: light ?? '', width: pixels(attribute('width')),
    height: pixels(attribute('height')), link: '', offset, inline }, [dark !== undefined, light !== undefined]];
}
const startsTag = (text: string, names: string[]) => names.some(name => new RegExp(`^<${name}(?=$|[\\s/>])`, 'i').test(text));
const startsImage = (text: string) => startsTag(text, ['img', 'picture', 'a']);
/**
 * The element at `at` (markdown-parse html.rs `element`): one `<img>`, one `<picture>` (its first `<img>` takes the first
 * colour-scheme `<source>` of each scheme it has no source for), or one wrapper or `<a>` holding only those, wrappers,
 * `<br>`s and comments, with the line boxes its images are drawn in. `ends`: the end of `text` closes what is left open,
 * as an HTML parser's end of input does.
 */
function element(text: string, at: number, ends: boolean, offset: number, inline: boolean): Markup | null {
  const first = tag(text, at);
  if (!first || first.closing || !(first.name === 'img' || first.name === 'picture' || first.name === 'a' || WRAPPERS.includes(first.name))) return null;
  const images: HtmlImage[] = [], open: Open[] = [], ids: number[] = [];
  // Each image's line: a `<p>` or `<div>` starting or ending, or a `<br>`, starts the next one.
  let line = 0;
  const keep = (url: string | null) => (url !== null && sourceAllowed(url) ? url : '');
  const close = (index: number) => {
    for (const { picture } of open.splice(index)) {
      const target = picture && picture.first >= 0 ? images[picture.first]! : null;
      if (!picture || !target) continue;
      if (!picture.authored[0]) target.dark = keep(picture.dark);
      if (!picture.authored[1]) target.light = keep(picture.light);
    }
  };
  const lastOpen = (name: string) => open.map(item => item.name).lastIndexOf(name);
  let next: Tag | null = first, cursor = at, left: string[] = [];
  for (;;) {
    let node = next;
    next = null;
    if (!node) {
      while (/\s/.test(text[cursor] ?? '')) cursor++;
      if (text.startsWith('<!--', cursor)) {
        const end = text.indexOf('-->', cursor + 4);
        if (end < 0) return null;
        cursor = end + 3;
        continue;
      }
      if (cursor >= text.length && ends) { left = open.map(item => item.name); close(0); break; }
      node = tag(text, cursor);
      if (!node) return null;
    }
    cursor = node.end;
    const picture = open[open.length - 1]?.picture ?? null;
    if (node.name === 'img' && !node.closing) {
      const [drawn, authored] = image(node, offset, inline);
      if (picture && picture.first < 0) { picture.first = images.length; picture.authored = authored; }
      drawn.link = [...open].reverse().find(item => item.link !== null)?.link ?? '';
      images.push(drawn);
      ids.push(line);
    } else if (node.name === 'source' && !node.closing && picture) {
      const scheme = /^\(\s*prefers-color-scheme\s*:\s*(dark|light)\s*\)$/i.exec(node.attributes.get('media') ?? '')?.[1]?.toLowerCase() as 'dark' | 'light' | undefined;
      const url = (node.attributes.get('srcset') ?? '').split(',')[0]!.trim().split(/\s/, 1)[0] ?? '';
      if (scheme && picture[scheme] === null) picture[scheme] = url || null;
    } else if (node.name === 'picture' && !node.closing && !picture) {
      open.push({ name: 'picture', picture: { first: -1, authored: [false, false], dark: null, light: null }, link: null });
    } else if (!node.closing && !picture && WRAPPERS.includes(node.name)) {
      if (CLOSES_P.includes(node.name) && lastOpen('p') >= 0) { close(lastOpen('p')); line++; }
      if (BLOCKS.includes(node.name)) line++;
      open.push({ name: node.name, picture: null, link: null });
    } else if (node.name === 'a' && !node.closing && !picture) {
      // An `<a>` inside an `<a>` is one HTML would close first; a refused `href` is dropped, its images unlinked.
      if (open.some(item => item.link !== null)) return null;
      const href = node.attributes.get('href') ?? '';
      open.push({ name: 'a', picture: null, link: protocolAllowed(href, LINK_SCHEMES) ? href : '' });
    } else if (node.closing) {
      // A stray `</p>` is an empty paragraph; HTML ignores any other end tag that closes nothing.
      if (lastOpen(node.name) >= 0) {
        if (open.slice(lastOpen(node.name)).some(item => BLOCKS.includes(item.name))) line++;
        close(lastOpen(node.name));
      } else if (node.name === 'p') return null;
    } else if (node.name === 'br' && !picture) line++;
    else return null;
    if (!open.length) break;
  }
  if (['strong', 'em', 'a'].includes(first.name) && !images.length) return null;
  const lines: Line[] = [];
  ids.forEach((id, index) => {
    if (index > 0 && ids[index - 1] === id) lines[lines.length - 1]!.end = index + 1;
    else lines.push({ start: index, end: index + 1, continues: id === 0, open: false });
  });
  if (lines.length) lines[lines.length - 1]!.open = ids[ids.length - 1] === line;
  return { images, lines, end: cursor, open: left };
}

/** markdown-parse html_images: the markup that starts line `index`, up to the next blank line, its last element ending
 *  its line; its images, their line boxes (elements one after another, no block between, share one), the line after it
 *  and what its end left open. */
function blockImages(lines: string[], index: number, offset: number): { images: HtmlImage[]; lines: Line[]; next: number; open: string[] } | null {
  let source = '', ends = true;
  for (let next = index; next < lines.length && lines[next]!.trim(); next++) {
    if (source.length + lines[next]!.length > LIMIT) { ends = false; break; }
    source += (source ? '\n' : '') + lines[next];
  }
  const images: HtmlImage[] = [], boxes: Line[] = [];
  let found: { count: number; lines: Line[]; next: number; open: string[] } | null = null;
  for (let cursor = 0; ;) {
    while (/\s/.test(source[cursor] ?? '')) cursor++;
    const markup = element(source, cursor, ends, offset + cursor, false);
    if (!markup) break;
    const base = images.length;
    markup.lines.forEach((box, at) => {
      const last = boxes[boxes.length - 1];
      if (at === 0 && box.continues && last?.open) { last.end = box.end + base; last.open = box.open; }
      else boxes.push({ ...box, start: box.start + base, end: box.end + base });
    });
    images.push(...markup.images);
    cursor = markup.end;
    const newline = source.indexOf('\n', cursor), lineEnd = newline < 0 ? source.length : newline;
    if (!source.slice(cursor, lineEnd).trim()) {
      found = { count: images.length, lines: boxes.map(box => ({ ...box })), next: index + source.slice(0, lineEnd).split('\n').length, open: markup.open };
    }
  }
  if (!found) return null;
  // Inline HTML (no CommonMark HTML block starts the markup) is a paragraph of its own: no image outside shares its line.
  const first = lines[index]!.trimStart(), tagAt = tag(first, 0);
  if (!(startsTag(first, CLOSES_P) || (tagAt && !first.slice(tagAt.end).trim()))) {
    if (found.lines[0]) found.lines[0].continues = false;
    if (found.lines.length) found.lines[found.lines.length - 1]!.open = false;
  }
  return { images: images.slice(0, found.count), lines: found.lines, next: found.next, open: found.open };
}
/** markdown-parse closes_open: a line of only the end tags of what image markup left open draws nothing. */
function closesOpen(line: string, open: string[]): boolean {
  const names: string[] = [];
  for (let at = line.search(/\S/); at >= 0 && at < line.length;) {
    const node = tag(line, at);
    if (!node || !node.closing || node.attributes.size || !(WRAPPERS.includes(node.name) || node.name === 'picture')) return false;
    names.push(node.name);
    at = node.end;
    while (/\s/.test(line[at] ?? '')) at++;
  }
  const closes = names.length > 0 && names.length <= open.length && names.every((name, i) => name === open[open.length - 1 - i]);
  if (closes) open.length -= names.length;
  return closes;
}
/** The end of the code span whose backticks start at `at`: past the next run of as many, or past its own run. */
function codeSpanEnd(line: string, at: number): number {
  const runAt = (from: number) => { let end = from; while (line[end] === '`') end++; return end - from; };
  const run = runAt(at);
  for (let next = line.indexOf('`', at + run); next >= 0;) {
    const length = runAt(next);
    if (length === run) return next + run;
    next = line.indexOf('`', next + length);
  }
  return at + run;
}
/** The images inside one line of text, outside code spans and escapes; `ends`: the line ends its paragraph. */
function inlineImages(line: string, offset: number, ends: boolean): HtmlImage[] {
  const out: HtmlImage[] = [];
  for (let at = 0; at < line.length;) {
    if (line[at] === '\\') { at += 2; continue; }
    if (line[at] === '`') { at = codeSpanEnd(line, at); continue; }
    const cut = line.slice(0, at + LIMIT);
    const markup = line[at] === '<' && startsImage(line.slice(at)) ? element(cut, at, ends && cut.length === line.length, offset + at, true) : null;
    if (markup) { out.push(...markup.images); at = markup.end; } else at++;
  }
  return out;
}

/**
 * The HTML images of a message in order, as markdown-parse draws them: image blocks (a line of image markup, outside
 * fences) and images inside a line of text (not in a fence, a code span or a line of literal HTML).
 */
export function markdownHtmlImages(text: string): HtmlImage[] {
  const images: HtmlImage[] = [], open: string[] = [];
  const lines = text.split('\n');
  // The images of the line box the next image markup may go on with (markdown-parse `Pending`), and whether it may.
  let going: HtmlImage[] = [], goingOn = false;
  // A line of two images or more, or one holding a linked image, is a paragraph of media runs (lib.rs line_blocks).
  const settle = (line: HtmlImage[]) => {
    if (line.length > 1 || line.some(image => image.link)) for (const image of line) image.inline = true;
  };
  let offset = 0, fence = '', width = 0, prose = false;
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index]!, trimmed = line.trimStart(), at = offset;
    offset += line.length + 1;
    const mark = /^(`{3,}|~{3,})/.exec(trimmed)?.[1];
    // Any block but image markup or a blank line ends the line box going on.
    if (trimmed.trim() && !(startsImage(trimmed) || startsTag(trimmed, WRAPPERS))) goingOn = false;
    if (fence) {
      if (mark?.[0] === fence && mark.length >= width && !trimmed.slice(mark.length).trim()) fence = '';
      prose = false;
      continue;
    }
    if (mark) { fence = mark[0]!; width = mark.length; prose = false; continue; }
    const block = startsImage(trimmed) || startsTag(trimmed, WRAPPERS) ? blockImages(lines, index, at) : null;
    if (block) {
      block.lines.forEach((box, n) => {
        const drawn = block.images.slice(box.start, box.end);
        if (n === 0 && box.continues && goingOn) going.push(...drawn);
        else going = drawn;
        settle(going);
        goingOn = n === block.lines.length - 1 && box.open;
      });
      images.push(...block.images);
      open.push(...block.open);
      for (; index < block.next - 1; index++) offset += lines[index + 1]!.length + 1;
      prose = false;
      continue;
    }
    if (!trimmed.trim()) { prose = false; continue; }
    goingOn = false;
    if (closesOpen(trimmed, open)) { prose = false; continue; }
    // A line of HTML that starts no paragraph stays literal (one that goes on with a paragraph is its text).
    const leads = startsImage(trimmed) && element(trimmed.slice(0, LIMIT), 0, false, 0, true) !== null;
    if (!prose && trimmed.startsWith('<') && trimmed.length > 2 && !leads) continue;
    const ends = !lines[index + 1]?.trim();
    if (/<(?:img|picture)/i.test(line)) images.push(...inlineImages(line, at, ends));
    prose = !/^(?:#{1,6}(?:\s|$)|>|[-*+]\s|\d{1,9}[.)]\s|\|)/.test(trimmed);
  }
  return images;
}
