// The reference's code colours: @pierre/highlights 0.2.0 (Apache-2.0), which highlights all of the reference's code
// (lib/syntaxHighlighting.ts: 80 languages, each a lexer in its dist/highlights.wasm; the others plain). Hermes runs
// no WebAssembly, so the data crate runs the lexers (data/src/highlights.rs), and a text reaches them through the
// root: a view asks here for a text's tokens, the root's pump (app.contract highlightStep) takes the waiting texts with
// highlightAsk, has the Rust source highlightCode lex them and hands their line records to highlightStore, and the
// view's next answer paints them. Until then a text paints plain, as the reference's code does while its highlighter
// loads, or keeps the tokens of an earlier text it extends (a streaming code block keeps its colours).
//
// A call to the lexers has a budget, so a long text is a document they keep and lex over several rounds, and a
// streaming block is a document each new version appends to (the reference's incrementalHighlighting.ts): only the
// line it continues and its new lines are lexed again. The texts each view's latest answer paints stay cached however
// many there are; the others are kept up to KEPT.

import { FILE_TYPES } from './file-languages';
import { LEXER_IDS, TYPE_CLASSES } from './highlights-tables';

/** A run of code text and its colour class (syntax-ink.contract synColor, synSlant, synWeight). */
export interface Token { text: string; cls: string }

/** @pierre/diffs' tokenizeMaxLineLength as the reference passes it: a longer line paints plain. */
export const TOKENIZE_MAX_LINE_LENGTH = 1_000;
/** A longer text paints plain. */
export const HIGHLIGHT_MAX_CHARS = 1_000_000;
/** highlights.rs WHOLE_BYTES: a text of more UTF-8 bytes is a document the lexers keep and lex over several rounds. */
export const WHOLE_BYTES = 32 * 1024;
/** The whole texts one ask hands out at most, in UTF-16 units: about what one highlightCode call lexes. */
const ASK_CHARS = 32 * 1024;
/** A waiting text no view's latest answer paints, and none asked for within this many asks, is dropped. */
const STALE_ASKS = 60;
/** Lexed texts no view's latest answer paints, kept by count and by characters, the most recently used first. */
const KEPT = 400, KEPT_CHARS = 4_000_000;
/** The recent lexed texts of a language a longer text may extend, and the documents kept (highlights.rs DOCUMENTS). */
const EXTENDABLE = 24, DOCUMENTS = 8;
/** highlightCode's `line` for a document with lines left, one to start again, and lexers that trapped. */
const MORE = -1, AGAIN = -2, FAILED = -3;

/** @pierre/diffs getFiletypeFromFileName, as the reference passes a file's path (its path in the project): the whole
 *  path as a name (a Dockerfile or CMakeLists.txt at the root; a nested one is no name), then a compound extension
 *  (component.ts), then a simple one; '' for a path it has no language for. */
export function fileLanguage(path: string): string {
  const has = (key: string) => Object.prototype.hasOwnProperty.call(FILE_TYPES, key);
  if (has(path)) return FILE_TYPES[path]!;
  const compound = /\.([^/\\]+\.[^/\\]+)$/.exec(path)?.[1];
  if (compound && has(compound)) return FILE_TYPES[compound]!;
  const simple = /\.([^.]+)$/.exec(path)?.[1] ?? '';
  return has(simple) ? FILE_TYPES[simple]! : '';
}

/** The lexer for a fence word or a file path, 0 for plain text. A word (no "/" or ".") is a fence word, which the
 *  reference passes on as written, gitignore as ini (MarkdownCodeBlock.logic.ts extractFenceLanguage), and the lexers
 *  read lower-cased (dist/highlighter.js langIdOf); anything else is a path (fileLanguage). */
export function lexerOf(nameOrPath: string): number {
  const raw = nameOrPath.trim();
  if (!raw) return 0;
  const name = !raw.includes('/') && !raw.includes('.') ? (raw === 'gitignore' ? 'ini' : raw) : fileLanguage(raw);
  return LEXER_IDS[name.toLowerCase()] ?? 0;
}

/** One text under one lexer and line limit, from the first time a view asks for it: waiting, asked, then lexed. */
interface Slot {
  family: string; lexer: number; max: number; text: string;
  tokens: Token[] | null;
  /** The lexers trapped on it: it paints plain, as does a longer text that extends it. */
  failed: boolean;
  /** It goes to the lexers as a document: past WHOLE_BYTES, or highlights.rs answered that it is. */
  long: boolean;
  /** The ask at which a view last asked for it, the ask it began waiting at, and its last use (the cache's order). */
  asked: number; since: number; used: number;
  stream: Stream | null;
}
/** A document the lexers keep: a long text, or a streaming block's versions. */
interface Stream {
  doc: number; family: string;
  /** The text the document holds. */
  text: string;
  /** The lines whose tokens are final, all but the last of the text last answered, with their tokens and length. */
  lines: number; final: Token[]; finalChars: number;
  /** An answer was lost or out of step: the next request starts the document again. */
  restart: boolean;
  /** A request of this ask goes to it. */
  busy: boolean;
}

const families = new Map<string, Map<string, Slot>>();
const waiting = new Set<Slot>();
const asked = new Map<number, Slot>();
/** The recent lexed texts of each family, newest last, for a text that extends one. */
const recent = new Map<string, Slot[]>();
let streams: Stream[] = [];
/** The texts each view's latest answer painted, and those the answers in flight have painted so far. */
const shown = new Map<string, Set<Slot>>(), painting = new Map<string, Set<Slot>>();
let nextId = 1, nextDoc = 1, asks = 0, uses = 0;
/** How many times highlightTokens answered "not yet": a cache of highlight results checks it. */
export let highlightDeferrals = 0;

/** The tokens of a text under the lexer for `nameOrPath`; null when it paints plain (no lexer, or too long). A text
 *  not lexed yet is asked for and paints plain meanwhile, or with the tokens of a recent text it extends. With
 *  `maxLineLength`, a longer line is one plain token. */
export function highlightTokens(text: string, nameOrPath: string, maxLineLength = 0): Token[] | null {
  const lexer = lexerOf(nameOrPath);
  if (!lexer || text.length > HIGHLIGHT_MAX_CHARS) return null;
  const family = `${lexer}\u0000${maxLineLength}`;
  let texts = families.get(family);
  if (!texts) families.set(family, texts = new Map());
  let slot = texts.get(text);
  if (!slot) {
    slot = { family, lexer, max: maxLineLength, text, tokens: null, failed: false, long: long(text), asked: asks, since: asks, used: 0, stream: null };
    texts.set(text, slot);
    if (extendedSlot(family, text)?.failed) { slot.tokens = [{ text, cls: '' }]; slot.failed = true; }
    else waiting.add(slot);
  }
  slot.used = ++uses; slot.asked = asks;
  for (const painted of painting.values()) painted.add(slot);
  if (slot.tokens) return slot.tokens;
  highlightDeferrals++;
  const base = extendedSlot(family, text);
  return base?.tokens ? [...base.tokens, { text: text.slice(base.text.length), cls: '' }] : null;
}

/** For a cache of painted results: the view's answer paints this text's tokens again (it stays cached). */
export function highlightShown(text: string, nameOrPath: string, maxLineLength = 0): void {
  const slot = families.get(`${lexerOf(nameOrPath)}\u0000${maxLineLength}`)?.get(text);
  if (!slot) return;
  slot.used = ++uses;
  for (const painted of painting.values()) painted.add(slot);
}

/** The longest recent lexed text of the family that `text` extends. */
function extendedSlot(family: string, text: string): Slot | undefined {
  let base: Slot | undefined;
  for (const slot of recent.get(family) ?? []) {
    if (slot.text.length < text.length && (!base || slot.text.length > base.text.length) && text.startsWith(slot.text)) base = slot;
  }
  return base;
}

/** Whether a text waits for its tokens (the root's pump runs while this holds). */
export const highlightPending = (): boolean => waiting.size > 0 || asked.size > 0;

/** One view's answer (app.ts): it reports whether it painted a text that waits for its tokens, and the texts it paints
 *  stay cached until its next answer, so the cache never drops a text on screen. A newer answer of the view replaces
 *  one still in flight. */
export async function highlightView<T extends object>(view: string, answer: () => T | Promise<T>): Promise<T & { highlightPending: boolean }> {
  const painted = new Set<Slot>(), deferrals = highlightDeferrals;
  painting.set(view, painted);
  try {
    const value = await answer();
    if (painting.get(view) === painted) shown.set(view, painted);
    return { ...value, highlightPending: highlightDeferrals !== deferrals };
  } finally {
    if (painting.get(view) === painted) painting.delete(view);
  }
}

function pinned(): Set<Slot> {
  const slots = new Set<Slot>();
  for (const views of [shown, painting]) for (const painted of views.values()) for (const slot of painted) slots.add(slot);
  return slots;
}

/** Whether a text has more UTF-8 bytes than highlights.rs lexes whole (WHOLE_BYTES). */
function long(text: string): boolean {
  if (text.length * 3 <= WHOLE_BYTES) return false;
  let bytes = text.length;
  for (let at = 0; at < text.length && bytes <= WHOLE_BYTES; at++) {
    const code = text.charCodeAt(at);
    // Two bytes for U+0080-U+07FF, three for the rest of the BMP, four for a surrogate pair (two units).
    if (code >= 0x80) bytes += code < 0x800 || (code >= 0xd800 && code < 0xe000) ? 1 : 2;
  }
  return bytes > WHOLE_BYTES;
}

/** The document a text goes to: the one of its family it extends, else a new one for a long text or the next version
 *  of a text lexed whole (a streaming block grows); null for a text lexed whole. */
function streamOf(slot: Slot): Stream | null {
  let best: Stream | null = null;
  for (const stream of streams) {
    if (stream.family === slot.family && stream.text && slot.text.startsWith(stream.text) && (!best || stream.text.length > best.text.length)) best = stream;
  }
  if (best) return best;
  if (!slot.long && !extendedSlot(slot.family, slot.text)) return null;
  const stream: Stream = { doc: nextDoc++, family: slot.family, text: '', lines: 0, final: [], finalChars: 0, restart: false, busy: false };
  streams.push(stream);
  if (streams.length > DOCUMENTS) streams = streams.slice(-DOCUMENTS);
  return stream;
}

function drop(slot: Slot): void {
  waiting.delete(slot);
  families.get(slot.family)?.delete(slot.text);
}

/** The pump's ask (app.contract highlightStep): the waiting texts, the longest-waiting first (a long text lexed over
 *  several rounds, or one behind newer texts, keeps its place), whole texts up to ASK_CHARS and one request a document,
 *  each with an id for its answer; `pending` when more wait. A text that has waited STALE_ASKS asks with no view
 *  painting it is dropped, and one a round asked for that never stored (its call failed) waits again. */
export function highlightAsk(): { requests: { id: number; language: number; doc: number; from: number; text: string }[]; pending: boolean } {
  asks++;
  for (const slot of asked.values()) {
    waiting.add(slot);
    if (slot.stream) slot.stream.restart = true; // the document may hold what the lost answer sent
  }
  asked.clear();
  for (const stream of streams) stream.busy = false;
  const painted = pinned(), requests: { id: number; language: number; doc: number; from: number; text: string }[] = [];
  let chars = 0;
  for (const slot of [...waiting].sort((a, b) => a.since - b.since)) {
    if (asks - slot.asked > STALE_ASKS && !painted.has(slot)) { drop(slot); continue; }
    const stream = streamOf(slot);
    let doc = 0, from = 0, text = slot.text;
    if (stream) {
      if (stream.busy) continue;
      if (stream.restart || !stream.text) Object.assign(stream, { lines: 0, final: [], finalChars: 0, restart: false });
      else { from = stream.text.length; text = slot.text.slice(from); }
      Object.assign(stream, { text: slot.text, busy: true });
      streams = [...streams.filter(entry => entry !== stream), stream];
      doc = stream.doc;
    } else {
      if (chars > 0 && chars + text.length > ASK_CHARS) continue;
      chars += text.length;
    }
    slot.stream = stream;
    const id = nextId++;
    waiting.delete(slot);
    asked.set(id, slot);
    requests.push({ id, language: slot.lexer, doc, from, text });
  }
  return { requests, pending: waiting.size > 0 };
}

/** The lexers' answer (highlightCode): each text's records from a line on become its tokens. A document with lines
 *  left, or one to start again, waits for the next round, as does a whole text the answer left out (past its budget).
 *  `finished` when a text got its tokens. */
export function highlightStore(results: readonly { id: number; line: number; records: readonly number[] }[]): { finished: boolean; pending: boolean } {
  let finished = false;
  for (const { id, line, records } of results) {
    const slot = asked.get(id);
    if (!slot) continue;
    asked.delete(id);
    const stream = slot.stream;
    if (line === MORE) { waiting.add(slot); continue; }
    if (line === AGAIN || (stream && line >= 0 && line !== stream.lines)) {
      if (stream) stream.restart = true; else slot.long = true;
      waiting.add(slot);
      continue;
    }
    if (line === FAILED) {
      Object.assign(slot, { tokens: [{ text: slot.text, cls: '' }], failed: true });
      streams = streams.filter(entry => entry !== stream);
    } else if (stream) {
      // The records count from the start of the first line not yet final; every line but the last is final now, since
      // an append changes only the line it continues.
      const region = tokensOf(slot.text.slice(stream.finalChars), records, slot.max);
      let breaks = 0, lastStart = 0;
      for (let at = 0; at + 1 < records.length; at += 2) if (records[at + 1] === 0xffffffff) { breaks++; lastStart = records[at]!; }
      let cut = 0;
      for (let chars = 0; cut < region.length && chars + region[cut]!.text.length <= lastStart; cut++) chars += region[cut]!.text.length;
      slot.tokens = stream.final.concat(region);
      Object.assign(stream, { final: stream.final.concat(region.slice(0, cut)), finalChars: stream.finalChars + lastStart, lines: stream.lines + breaks });
    } else slot.tokens = tokensOf(slot.text, records, slot.max);
    const texts = (recent.get(slot.family) ?? []).filter(entry => entry !== slot);
    texts.push(slot);
    recent.set(slot.family, texts.slice(-EXTENDABLE));
    finished = true;
  }
  for (const slot of asked.values()) {
    waiting.add(slot);
    if (slot.stream) slot.stream.restart = true;
  }
  asked.clear();
  trim();
  return { finished, pending: waiting.size > 0 };
}

/** The lexed texts no view's latest answer paints, past KEPT or KEPT_CHARS, the least recently used first, go. */
function trim(): void {
  const painted = pinned(), lexed: Slot[] = [];
  for (const texts of families.values()) for (const slot of texts.values()) if (slot.tokens && !painted.has(slot)) lexed.push(slot);
  if (lexed.length <= KEPT && lexed.reduce((sum, slot) => sum + slot.text.length, 0) <= KEPT_CHARS) return;
  lexed.sort((a, b) => b.used - a.used);
  let chars = 0;
  lexed.forEach((slot, index) => {
    chars += slot.text.length;
    if (index < KEPT && chars <= KEPT_CHARS) return;
    families.get(slot.family)?.delete(slot.text);
    const texts = recent.get(slot.family);
    if (texts?.includes(slot)) recent.set(slot.family, texts.filter(entry => entry !== slot));
  });
}

/** dist/tokens.js lineRecordsToTokens: line records (a token's UTF-16 end and its type; at a line break the next
 *  line's start and 0xFFFFFFFF) as tokens over the whole text, line breaks plain, a line longer than `max` one plain
 *  token, and neighbours of one class merged. */
export function tokensOf(text: string, records: readonly number[], max = 0): Token[] {
  const tokens: Token[] = [];
  let start = 0, lineStart = 0, lineToken = 0;
  const push = (end: number, cls: string) => {
    if (end <= start) return;
    const last = tokens[tokens.length - 1];
    if (last && last.cls === cls && tokens.length > lineToken) last.text += text.slice(start, end);
    else tokens.push({ text: text.slice(start, end), cls });
    start = end;
  };
  const endLine = (end: number) => {
    if (max > 0 && end - lineStart >= max) tokens.splice(lineToken, tokens.length - lineToken, { text: text.slice(lineStart, end), cls: '' });
  };
  for (let at = 0; at + 1 < records.length; at += 2) {
    const end = records[at]!, type = records[at + 1]!;
    if (type === 0xffffffff) {
      endLine(start);
      push(end, '');
      lineStart = start; lineToken = tokens.length;
      continue;
    }
    push(end, TYPE_CLASSES[type] ?? '');
  }
  push(text.length, '');
  endLine(text.length);
  return tokens;
}

/** Tests: no lexed, waiting or asked texts, documents or views. */
export function resetHighlights(): void {
  families.clear(); waiting.clear(); asked.clear(); recent.clear(); shown.clear(); painting.clear();
  streams = []; asks = 0;
}
