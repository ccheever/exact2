// T3 Code 5d3d78ab93 (MIT; LICENSE-T3), markdownPipeline.ts: image sources kept by
// the sanitizer. This indexes the standalone HTML image blocks markdown-parse draws;
// it never inserts HTML into a host view.
export type HtmlImage = { alt: string; src: string; dark: string; light: string; offset: number };
type Tag = { name: string; closing: boolean; attributes: Map<string, string>; end: number };
const SPACE = /[\t\n\f\r ]/;
const SCHEMES = new Set(['http', 'https', 'file', 't3-context']);

function decode(value: string): string {
  const named: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" };
  return value.replace(/&(#(?:[xX][\da-fA-F]+|\d+)|amp|lt|gt|quot|apos);/g, (whole, name: string) => {
    if (named[name]) return named[name]!;
    const number = /^#x/i.test(name) ? parseInt(name.slice(2), 16) : Number(name.slice(1));
    return number > 0 && number <= 0x10ffff && !(number >= 0xd800 && number <= 0xdfff) ? String.fromCodePoint(number) : whole;
  });
}
function allowed(url: string): string {
  const colon = url.indexOf(':');
  return colon < 0 || /[/#?]/.test(url.slice(0, colon)) || /^[a-z]:[/\\]/i.test(url) || SCHEMES.has(url.slice(0, colon)) ? url : '';
}
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
function element(text: string, at: number): { images: HtmlImage[]; end: number } | null {
  const first = tag(text, at);
  if (!first || first.closing || !['img', 'picture'].includes(first.name)) return null;
  const image = (node: Tag): HtmlImage => ({ alt: node.attributes.get('alt') ?? '', src: allowed(node.attributes.get('src') ?? ''), dark: '', light: '', offset: at });
  if (first.name === 'img') return { images: [image(first)], end: first.end };
  const images: HtmlImage[] = [];
  const sources = new Map<string, string>();
  let cursor = first.end;
  while (cursor < text.length) {
    while (/\s/.test(text[cursor] ?? '')) cursor++;
    if (text.startsWith('<!--', cursor)) {
      const end = text.indexOf('-->', cursor + 4);
      if (end < 0) return null;
      cursor = end + 3; continue;
    }
    const node = tag(text, cursor);
    if (!node) return null;
    cursor = node.end;
    if (node.name === 'picture' && node.closing) {
      if (images[0]) { images[0].dark = allowed(sources.get('dark') ?? ''); images[0].light = allowed(sources.get('light') ?? ''); }
      return { images, end: cursor };
    }
    if (node.name === 'img' && !node.closing) images.push(image(node));
    else if (node.name === 'source' && !node.closing) {
      const scheme = /^\(\s*prefers-color-scheme\s*:\s*(dark|light)\s*\)$/i.exec(node.attributes.get('media') ?? '')?.[1]?.toLowerCase();
      const url = (node.attributes.get('srcset') ?? '').trim().split(/\s/, 1)[0]?.replace(/,+$/, '') ?? '';
      if (scheme && url && !sources.has(scheme)) sources.set(scheme, url);
    } else if (!node.closing || !['img', 'source'].includes(node.name)) return null;
  }
  return null;
}

/** Match markdown-parse's bounded, whole-line image blocks, excluding fences and inline code. */
export function markdownHtmlImages(text: string): HtmlImage[] {
  const images: HtmlImage[] = [];
  const lines = text.split('\n');
  let offset = 0, fence = '', width = 0;
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index]!, trimmed = line.trimStart();
    const mark = /^(`{3,}|~{3,})/.exec(trimmed)?.[1];
    if (fence) {
      if (mark?.[0] === fence && mark.length >= width && !trimmed.slice(mark.length).trim()) fence = '';
    } else if (mark) { fence = mark[0]!; width = mark.length; }
    else if (line.length <= 8192 && /^<(?:img|picture)(?=[\s/>])/i.test(trimmed)) {
      let source = line;
      for (let next = index + 1; next < lines.length && lines[next]!.trim() && source.length + lines[next]!.length <= 8192; next++) source += `\n${lines[next]}`;
      const found: HtmlImage[] = [];
      let cursor = 0, accepted = 0, end = 0;
      while (cursor < source.length) {
        while (/\s/.test(source[cursor] ?? '')) cursor++;
        const item = element(source, cursor);
        if (!item) break;
        found.push(...item.images.map(image => ({ ...image, offset: offset + image.offset })));
        cursor = item.end;
        const newline = source.indexOf('\n', cursor), lineEnd = newline < 0 ? source.length : newline;
        if (!source.slice(cursor, lineEnd).trim()) { accepted = found.length; end = lineEnd; }
      }
      if (end) {
        images.push(...found.slice(0, accepted));
        const count = source.slice(0, end).split('\n').length;
        for (let consumed = 0; consumed < count - 1; consumed++) offset += lines[index++]!.length + 1;
      }
    }
    offset += lines[index]!.length + 1;
  }
  return images;
}
