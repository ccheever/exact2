export type Mark = 'bold' | 'italic' | 'underline' | 'strike' | 'spoiler';

export type Inline =
  | { kind: 'text'; text: string; marks: Mark[] }
  | { kind: 'code'; text: string; marks: Mark[] }
  | { kind: 'link'; url: string; children: Inline[]; masked: boolean; marks: Mark[] }
  | { kind: 'user'; id: string; marks: Mark[] }
  | { kind: 'channel'; id: string; marks: Mark[] }
  | { kind: 'role'; id: string; marks: Mark[] }
  | { kind: 'everyone'; which: 'everyone' | 'here'; marks: Mark[] }
  | { kind: 'emoji'; id: string; name: string; animated: boolean; marks: Mark[] }
  | { kind: 'timestamp'; unix: number; style: string; marks: Mark[] }
  | { kind: 'break' };

export type Block =
  | { kind: 'paragraph'; inlines: Inline[] }
  | { kind: 'heading'; level: 1 | 2 | 3; inlines: Inline[] }
  | { kind: 'subtext'; inlines: Inline[] }
  | { kind: 'quote'; blocks: Block[] }
  | { kind: 'codeblock'; lang: string; text: string }
  | { kind: 'list'; ordered: boolean; start: number; items: Block[][] };

const markOrder: Mark[] = ['bold', 'italic', 'underline', 'strike', 'spoiler'];
const unescape = (s: string): string => s.replace(/\\([^0-9A-Za-z\s])/g, '$1');

function text(out: Inline[], value: string, marks: Mark[]): void {
  if (!value) return;
  const last = out[out.length - 1];
  if (last?.kind === 'text' && last.marks.length === marks.length &&
      last.marks.every((mark, i) => mark === marks[i])) {
    last.text += value;
  } else out.push({ kind: 'text', text: value, marks: [...marks] });
}

function append(out: Inline[], nodes: Inline[]): void {
  for (const node of nodes) {
    if (node.kind === 'text') text(out, node.text, node.marks);
    else out.push(node);
  }
}

function marked(marks: Mark[], mark: Mark): Mark[] {
  return markOrder.filter(value => value === mark || marks.includes(value));
}

// https://github.com/discord/simple-markdown/blob/master/simple-markdown.js
// em/strong/u share an order: longest capture wins, with em then strong winning ties.
// Discord's spoiler rule wraps any nonempty capture, including whitespace.
// In LINK_INSIDE's lookahead, stopping at the first ']' proves the same
// condition as scanning to the last, without quadratic runs of closing brackets.
const linkInside = '(?:\\[[^\\]]*\\]|[^\\[\\]]|\\](?=[^\\[\\]]*\\]))*';
const linkTarget = '\\s*<?((?:\\([^)]*\\)|[^\\s\\\\]|\\\\.)*?)>?(?:\\s+[\'\"]([\\s\\S]*?)[\'\"])?\\s*';
const targetRule = new RegExp('^\\]\\(' + linkTarget + '\\)');
const safeTarget = /^(?:https?:\/\/|mailto:|discord:\/\/)[^\s<>]+$/i;
interface Rule {
  name: string;
  first: string;
  regex: RegExp;
  mark?: Mark;
  quality?: number;
  link?: boolean;
}
const rules: Rule[] = [
  { name: 'spoiler', first: '|', regex: /^\|\|([\s\S]+?)\|\|/, mark: 'spoiler' },
  { name: 'strong', first: '*', regex: /^\*\*((?:\\[\s\S]|[^\\])+?)\*\*(?!\*)/, mark: 'bold', quality: 0.1 },
  { name: 'em', first: '*_', regex: /^\b_((?:__|\\[\s\S]|[^\\_])+?)_\b|^\*(?=\S)((?:\*\*|\\[\s\S]|\s+(?:\\[\s\S]|[^\s*\\]|\*\*)|[^\s*\\])+?)\*(?!\*)/, mark: 'italic', quality: 0.2 },
  { name: 'u', first: '_', regex: /^__((?:\\[\s\S]|[^\\])+?)__(?!_)/, mark: 'underline', quality: 0 },
  { name: 'del', first: '~', regex: /^~~([\s\S]+?)~~(?!_)/, mark: 'strike' },
  { name: 'inlineCode', first: '`', regex: /^(`+)([\s\S]*?[^`])\1(?!`)/ },
  { name: 'url', first: 'h', regex: /^(https?:\/\/[^\s<]+[^<.,:;"')\]\s])/, link: true },
  { name: 'autolink', first: '<', regex: /^<([^ >]+:\/[^ >]+)>/, link: true },
  { name: 'link', first: '[', regex: new RegExp('^\\[(' + linkInside + ')\\]\\(' + linkTarget + '\\)'), link: true },
  { name: 'user', first: '<', regex: /^<@!?(\d+)>/ },
  { name: 'channel', first: '<', regex: /^<#(\d+)>/ },
  { name: 'role', first: '<', regex: /^<@&(\d+)>/ },
  { name: 'everyone', first: '@', regex: /^@(everyone|here)\b/ },
  { name: 'emoji', first: '<', regex: /^<(a?):([A-Za-z0-9_]+):(\d+)>/ },
  { name: 'timestamp', first: '<', regex: /^<t:(-?\d+)(?::([tTdDfFR]))?>/ },
  { name: 'escape', first: '\\', regex: /^\\([^0-9A-Za-z\s])/ },
];
const byFirst = new Map<string, Rule[]>();
for (const rule of rules) {
  for (const first of rule.first) byFirst.set(first, [...(byFirst.get(first) ?? []), rule]);
}

interface Source {
  value: string;
  textEnd: Uint32Array;
  linkLabel: Uint8Array;
  lastAngle: number;
  codeWidth: Uint32Array;
  codeMatchers: Map<number, RegExp>;
  lastStrike: number;
}

function sourceFor(value: string): Source {
  // Index the fork's text lookahead in one backward pass. Re-running \w+:\S at
  // each letter, or a failed link rule at each '[', rescans the same suffix.
  const textEnd = new Uint32Array(value.length + 1);
  const linkLabel = new Uint8Array(value.length + 1);
  const codeWidth = new Uint32Array(value.length + 1);
  const lastParen = value.lastIndexOf(')');
  let nextOpen = -1;
  let nextClose = -1;
  let next = value.length;
  let scheme = false;
  let spaces = 0;
  let ticks = 0;
  let closingWidth = 0;
  let lastStrike = -1;
  for (let i = value.length - 1; i >= 0; i--) {
    const c = value[i];
    scheme = /\w/.test(c) && (scheme || (value[i + 1] === ':' && i + 2 < value.length && !/\s/.test(value[i + 2])));
    spaces = c === ' ' ? (value[i + 1] === '\n' ? 1 : spaces ? spaces + 1 : 0) : 0;
    if (/[^0-9A-Za-z\s\u00c0-\uffff]/.test(c) || scheme || spaces >= 2 || (c === '\n' && value[i + 1] === '\n')) next = i;
    textEnd[i] = next;
    // A viability index for LINK_INSIDE's three alternatives. It only rejects
    // impossible starts; the anchored link regex still chooses every capture.
    if (c === '[') linkLabel[i] = nextClose < 0 ? 0 : linkLabel[nextClose + 1];
    else if (c === ']') {
      const match = i < lastParen && value[i + 1] === '(' ? targetRule.exec(value.slice(i)) : null;
      const target = match && safeTarget.test(unescape(match[1]));
      linkLabel[i] = target || (nextClose >= 0 && (nextOpen < 0 || nextClose < nextOpen) && linkLabel[i + 1]) ? 1 : 0;
    } else linkLabel[i] = linkLabel[i + 1];
    if (c === '[') nextOpen = i;
    if (c === ']') nextClose = i;
    if (c === '`') ticks++;
    else { closingWidth = Math.max(closingWidth, ticks); ticks = 0; }
    codeWidth[i] = closingWidth;
    if (lastStrike === -1 && c === '~' && value[i + 1] === '~' && value[i + 2] !== '_') lastStrike = i;
  }
  textEnd[value.length] = value.length;
  return { value, textEnd, linkLabel, lastAngle: value.lastIndexOf('>'), codeWidth, codeMatchers: new Map(), lastStrike };
}

interface Capture { rule: Rule; match: RegExpExecArray }
const textRule: Rule = { name: 'text', first: '', regex: /^[\s\S]+/ };

function inlineAt(source: Source, at: number, links: boolean): Capture {
  const value = source.value.slice(at);
  let best: Capture | undefined;
  for (const rule of byFirst.get(value[0]) ?? []) {
    if (rule.link && !links) continue;
    if (rule.name === 'link' && !source.linkLabel[at + 1]) continue;
    if (rule.name === 'autolink' && (source.lastAngle < at || !/^<(?:https?|mailto|discord)/i.test(value))) continue;
    if (rule.name === 'inlineCode' && !source.codeWidth[at]) continue;
    if (rule.name === 'del' && source.lastStrike < at + 3) continue;
    if (best && rule.quality === undefined) break;
    let regex = rule.regex;
    if (rule.name === 'inlineCode') {
      // Opening `+` may backtrack, but widths exceeding every later closing
      // run cannot succeed. Exclude them before running the fork's regex.
      const width = source.codeWidth[at];
      let bounded = source.codeMatchers.get(width);
      if (!bounded) {
        bounded = new RegExp(regex.source.replace('`+', '`{1,' + width + '}'));
        source.codeMatchers.set(width, bounded);
      }
      regex = bounded;
    }
    const match = regex.exec(value);
    if (!match) continue;
    if (rule.name === 'link' && !safeTarget.test(unescape(match[2]))) continue;
    if (rule.name === 'autolink' && !safeTarget.test(match[1])) continue;
    if (rule.name === 'timestamp' && !Number.isSafeInteger(Number(match[1]))) continue;
    if (!best || match[0].length + (rule.quality ?? 0) > best.match[0].length + (best.rule.quality ?? 0)) best = { rule, match };
    if (rule.quality === undefined) break;
  }
  if (best) return best;
  // Equivalent captures to /^[\s\S]+?(?=[^0-9A-Za-z\s\u00c0-\uffff]|\n\n| {2,}\n|\w+:\S|$)/.
  // The indexed boundary avoids the original lookahead's quadratic failure path.
  return { rule: textRule, match: textRule.regex.exec(source.value.slice(at, source.textEnd[at + 1]))! };
}

function literal(out: Inline[], value: string, marks: Mark[]): void {
  const lines = value.split('\n');
  for (let i = 0; i < lines.length; i++) {
    if (i) out.push({ kind: 'break' });
    text(out, lines[i], marks);
  }
}

interface InlineFrame { source: Source; at: number; marks: Mark[]; links: boolean; out: Inline[] }
function inlines(value: string, marks: Mark[] = [], links = true): Inline[] {
  const out: Inline[] = [];
  const stack: InlineFrame[] = [{ source: sourceFor(value), at: 0, marks, links, out }];
  while (stack.length) {
    const frame = stack[stack.length - 1];
    const source = frame.source.value;
    if (frame.at === source.length) { stack.pop(); continue; }
    // A uniform run recursively removes two characters for em, or four for
    // strong/u. Marks are idempotent, so fold those identical nested captures.
    if (frame.at === 0 && /^(?:\*{4,}|_{4,})$/.test(source)) {
      const mark: Mark = source.length % 2 === 0 ? 'italic' : source[0] === '*' ? 'bold' : 'underline';
      const width = source.length % 2 === 0 ? 2 : source.length % 4;
      text(frame.out, source.slice(0, width), marked(frame.marks, mark));
      stack.pop();
      continue;
    }
    const capture = inlineAt(frame.source, frame.at, frame.links);
    frame.at += capture.match[0].length;
    const { rule, match } = capture;
    if (rule.mark) {
      stack.push({ source: sourceFor(match[2] || match[1]), at: 0, marks: marked(frame.marks, rule.mark), links: frame.links, out: frame.out });
    } else if (rule.name === 'link') {
      const children: Inline[] = [];
      frame.out.push({ kind: 'link', url: unescape(match[2]), children, masked: true, marks: [...frame.marks] });
      stack.push({ source: sourceFor(match[1]), at: 0, marks: frame.marks, links: false, out: children });
    } else appendAtom(frame.out, capture, frame.marks);
  }
  return out;
}

function appendAtom(out: Inline[], { rule, match }: Capture, marks: Mark[]): void {
  switch (rule.name) {
    case 'inlineCode':
      out.push({ kind: 'code', text: match[2].replace(/^ (?= *`)|(` *) $/g, '$1'), marks: [...marks] });
      break;
    case 'url': case 'autolink':
      out.push({ kind: 'link', url: match[1], children: [{ kind: 'text', text: match[1], marks: [...marks] }], masked: false, marks: [...marks] });
      break;
    case 'user': case 'channel': case 'role':
      out.push({ kind: rule.name, id: match[1], marks: [...marks] });
      break;
    case 'everyone':
      out.push({ kind: 'everyone', which: match[1] as 'everyone' | 'here', marks: [...marks] });
      break;
    case 'emoji':
      out.push({ kind: 'emoji', id: match[3], name: match[2], animated: match[1] === 'a', marks: [...marks] });
      break;
    case 'timestamp':
      out.push({ kind: 'timestamp', unix: Number(match[1]), style: match[2] ?? 'f', marks: [...marks] });
      break;
    case 'escape': text(out, match[1], marks); break;
    default: literal(out, match[0], marks);
  }
}

interface Line {
  at: number;
  end: number;
  spaces: number;
  blankEnd: number;
  trimEnd: number;
  markerAt?: number;
  marker?: ListMarker;
}
interface Frame {
  line: number;
  end: number;
  lineStart: boolean;
  blocks: Block[];
  paragraph: Inline[];
  newline: boolean;
  cache?: { source: Source; at: number; multiline: boolean };
}
interface ListMarker { indent: number; ordered: boolean; start: number; body: number }

function linesFor(value: string): Line[] {
  const lines: Line[] = [];
  let at = 0;
  while (at <= value.length) {
    const newline = value.indexOf('\n', at);
    const end = newline === -1 ? value.length : newline;
    const line = value.slice(at, end);
    let blankEnd = end;
    while (blankEnd > at && (value[blankEnd - 1] === ' ' || value[blankEnd - 1] === '\t')) blankEnd--;
    lines.push({ at, end, spaces: at + /^ */.exec(line)![0].length,
      blankEnd, trimEnd: at + line.trimEnd().length });
    if (newline === -1) break;
    at = end + 1;
  }
  return lines;
}

function listMarker(value: string, line: Line): ListMarker | undefined {
  if (line.markerAt !== line.spaces) {
    line.markerAt = line.spaces;
    const match = /^(?:([-*])|(\d{1,9})\.)[ \t]+/.exec(value.slice(line.spaces, line.end));
    line.marker = match ? { indent: 0, ordered: !match[1], start: match[2] ? Number(match[2]) : 1,
      body: line.spaces + match[0].length } : undefined;
  }
  return line.marker && { ...line.marker, indent: line.spaces - line.at };
}

function frameFor(line: number, end: number, blocks: Block[]): Frame {
  return { line, end, lineStart: true, blocks, paragraph: [], newline: false };
}

function flush(frame: Frame): void {
  if (frame.paragraph.length) frame.blocks.push({ kind: 'paragraph', inlines: frame.paragraph });
  frame.paragraph = [];
  frame.newline = false;
}

function quoteWidth(value: string, line: Line): number {
  if (value[line.at] !== '>') return 0;
  if (line.at + 1 === line.end) return 1;
  return value[line.at + 1] === ' ' || value[line.at + 1] === '\t' ? 2 : 0;
}

function stripQuote(value: string, line: Line, width: number): void {
  line.at += width;
  line.spaces = line.at;
  while (line.spaces < line.end && value[line.spaces] === ' ') line.spaces++;
}

function remainder(value: string, lines: Line[], frame: Frame): string {
  const parts: string[] = [];
  for (let i = frame.line; i < frame.end; i++) parts.push(value.slice(lines[i].at, lines[i].end));
  return parts.join('\n');
}

function advance(lines: Line[], frame: Frame, width: number): void {
  if (frame.cache) frame.cache.at += width;
  while (width) {
    const line = lines[frame.line];
    const available = line.end - line.at;
    if (width <= available) { line.at += width; frame.lineStart = false; break; }
    width -= available + 1;
    frame.line++;
    frame.lineStart = true;
  }
}

// Lazy captures can finish on this line; a backtracked backtick opener can
// still prefer its full width on a later line. Unfinished candidates and greedy
// link labels also need the remainder. Ordinary list bodies index only this line.
function needsRemainder(value: string): boolean {
  for (const rule of byFirst.get(value[0]) ?? []) {
    if (rule.name === 'link') return true;
    if (!rule.mark && rule.name !== 'inlineCode') continue;
    const match = rule.regex.exec(value);
    if (match && (rule.name !== 'inlineCode' || match[1].length === /^`+/.exec(value)![0].length)) continue;
    if (rule.name === 'strong' && !value.startsWith('**')) continue;
    if (rule.name === 'u' && !value.startsWith('__')) continue;
    if (rule.name === 'del' && !value.startsWith('~~')) continue;
    if (rule.name === 'spoiler' && !value.startsWith('||')) continue;
    if (rule.name === 'em') {
      if (value[0] === '*' && (/^\*\s/.test(value) || (value[1] === '*' && value[2] !== '*'))) continue;
      if (value[0] === '_' && value[1] === '_' && value[2] !== '_') continue;
    }
    return true;
  }
  return false;
}

function inlineSource(value: string, lines: Line[], frame: Frame): { source: Source; at: number } {
  const line = lines[frame.line];
  const tail = value.slice(line.at, line.end);
  if (!frame.cache?.multiline && frame.line + 1 < frame.end && needsRemainder(tail)) {
    frame.cache = { source: sourceFor(remainder(value, lines, frame)), at: 0, multiline: true };
  } else if (!frame.cache) frame.cache = { source: sourceFor(tail), at: 0, multiline: false };
  return frame.cache;
}

const fenceRule = /^```(?:([\w+.#-]+)[ \t]*\n)?([\s\S]*?)```/;

/** Parse message content without performing I/O or resolving Discord identifiers. */
export function parse(content: string): Block[] {
  const blocks: Block[] = [];
  if (!content.trim()) return blocks;
  const value = content.replace(/\r\n?/g, '\n');
  const lines = linesFor(value);
  // Containers own disjoint line ranges in this shared table. Stripping a quote
  // consumes its prefix; stripping list indentation changes an offset in O(1).
  // Neither operation copies or indexes the remaining body at each depth.
  const stack: Frame[] = [frameFor(0, lines.length, blocks)];
  while (stack.length) {
    const frame = stack[stack.length - 1];
    if (frame.line === frame.end) { flush(frame); stack.pop(); continue; }
    const line = lines[frame.line];
    if (!frame.lineStart && line.at === line.end) {
      frame.newline = frame.paragraph.length > 0;
      frame.line++;
      frame.lineStart = true;
      if (frame.cache?.multiline) frame.cache.at++;
      else frame.cache = undefined;
      continue;
    }
    if (frame.lineStart) {
      if (line.at >= line.blankEnd) {
        flush(frame);
        frame.line++;
        frame.cache = undefined;
        continue;
      }
      const multi = value.startsWith('>>>', line.at) && /[ \t]/.test(value[line.at + 3] ?? '');
      const quote = quoteWidth(value, line);
      if (multi || quote) {
        flush(frame);
        frame.cache = undefined;
        const child: Block[] = [];
        frame.blocks.push({ kind: 'quote', blocks: child });
        const from = frame.line;
        if (multi) {
          let at = line.at + 3;
          while (at < line.end && (value[at] === ' ' || value[at] === '\t')) at++;
          stripQuote(value, line, at - line.at);
          frame.line = frame.end;
        } else {
          while (frame.line < frame.end) {
            const current = lines[frame.line];
            const width = quoteWidth(value, current);
            if (!width) break;
            stripQuote(value, current, width);
            frame.line++;
          }
        }
        stack.push(frameFor(from, frame.line, child));
        continue;
      }
      const marker = listMarker(value, line);
      if (marker && marker.indent <= 3) {
        flush(frame);
        frame.cache = undefined;
        const list: Extract<Block, { kind: 'list' }> = { kind: 'list', ordered: marker.ordered, start: marker.start, items: [] };
        frame.blocks.push(list);
        const children: Frame[] = [];
        while (frame.line < frame.end) {
          const first = lines[frame.line];
          const item = listMarker(value, first);
          if (!item || item.indent !== marker.indent || item.ordered !== marker.ordered) break;
          const from = frame.line++;
          first.at = first.spaces = item.body;
          while (frame.line < frame.end) {
            const continuation = lines[frame.line];
            const next = listMarker(value, continuation);
            if (next && next.indent <= marker.indent) break;
            const indent = continuation.spaces - continuation.at;
            const nonblank = continuation.at < continuation.trimEnd;
            if (nonblank && indent <= marker.indent) break;
            const following = frame.line + 1 < frame.end ? lines[frame.line + 1] : undefined;
            if (!nonblank && (!following || following.spaces === following.at ||
                following.spaces === following.end || /\s/.test(value[following.spaces]))) break;
            // Discord's nested bullets use two spaces, including under ordered items.
            continuation.at += Math.min(indent, marker.indent + 2);
            frame.line++;
          }
          const child: Block[] = [];
          list.items.push(child);
          children.push(frameFor(from, frame.line, child));
        }
        for (let i = children.length - 1; i >= 0; i--) stack.push(children[i]);
        continue;
      }
      const tail = value.slice(line.at, line.end);
      const heading = /^(#{1,3})[ \t]+([^\n]*)/.exec(tail);
      const subtext = /^-#[ \t]+([^\n]*)/.exec(tail);
      if (heading || subtext) {
        flush(frame);
        frame.cache = undefined;
        if (heading) frame.blocks.push({ kind: 'heading', level: heading[1].length as 1 | 2 | 3, inlines: inlines(heading[2]) });
        else frame.blocks.push({ kind: 'subtext', inlines: inlines(subtext![1]) });
        advance(lines, frame, (heading ?? subtext)![0].length);
        continue;
      }
    }
    const tail = value.slice(line.at, line.end);
    const fence = tail.startsWith('```') ? fenceRule.exec(tail) ?? fenceRule.exec(remainder(value, lines, frame)) : null;
    if (fence) {
      flush(frame);
      frame.cache = undefined;
      frame.blocks.push({ kind: 'codeblock', lang: fence[1] ?? '', text: fence[2].replace(/^\n+|\n+$/g, '') });
      advance(lines, frame, fence[0].length);
      continue;
    }
    const indexed = inlineSource(value, lines, frame);
    const capture = inlineAt(indexed.source, indexed.at, true);
    // Text stops at a line boundary so block rules can run there. A matched
    // inline capture owns its newlines, even when they look like block syntax.
    if (capture.rule.name === 'text') {
      const newline = capture.match[0].indexOf('\n');
      if (newline !== -1) capture.match[0] = capture.match[0].slice(0, newline);
    }
    if (frame.newline) { frame.paragraph.push({ kind: 'break' }); frame.newline = false; }
    if (capture.rule.mark) append(frame.paragraph, inlines(capture.match[2] || capture.match[1], [capture.rule.mark]));
    else if (capture.rule.name === 'link') {
      frame.paragraph.push({ kind: 'link', url: unescape(capture.match[2]), children: inlines(capture.match[1], [], false), masked: true, marks: [] });
    } else appendAtom(frame.paragraph, capture, []);
    advance(lines, frame, capture.match[0].length);
  }
  return blocks;
}

// ES2022 Unicode properties cover joined emoji without Intl.Segmenter or a host API.
const emojiSequence = /(?:[0-9#*]\uFE0F?\u20E3|[\u{1F1E6}-\u{1F1FF}]{2}|\p{Extended_Pictographic}\uFE0F?[\u{1F3FB}-\u{1F3FF}]?(?:[\u{E0020}-\u{E007E}]+\u{E007F})?(?:\u200D\p{Extended_Pictographic}\uFE0F?[\u{1F3FB}-\u{1F3FF}]?)*)/uy;

/** Discord enlarges up to 27 unformatted emoji in otherwise blank message content. */
export function jumbo(blocks: Block[]): boolean {
  let count = 0;
  for (const block of blocks) {
    if (block.kind !== 'paragraph') return false;
    for (const inline of block.inlines) {
      if (inline.kind === 'break') continue;
      if (inline.kind !== 'text' && inline.kind !== 'emoji') return false;
      if (inline.marks.length) return false;
      if (inline.kind === 'emoji') { if (++count > 27) return false; continue; }
      let at = 0;
      while (at < inline.text.length) {
        if (/\s/u.test(inline.text[at])) { at++; continue; }
        emojiSequence.lastIndex = at;
        const match = emojiSequence.exec(inline.text);
        if (!match || ++count > 27) return false;
        at = emojiSequence.lastIndex;
      }
    }
  }
  return count > 0;
}
