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
// many there are; the others are kept up to KEPT and KEPT_BYTES. A view's answer pins, and reports waiting, only the
// texts painted for it: each call that paints names its view.

import { CUSTOM_EXTENSIONS, FILE_TYPES } from './file-languages';
import { LEXER_IDS, TYPE_CLASSES } from './highlights-tables';

/** A run of code text and its colour class (syntax-ink.contract synColor, synSlant, synWeight). */
export interface Token { text: string; cls: string }
/** The views that paint code, each answering through highlightView (app.ts). */
export type HighlightView = 'data' | 'shell' | 'palette' | 'prDetail' | 'settingsCore';

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
/** Lexed texts no view's latest answer paints, kept by count and by the bytes their tokens take, the most recently
 *  used first: 8 MiB of the TypeScript module's 64 MiB heap (js/src/lib.rs DEFAULT_MAX_HEAP). */
const KEPT = 400, KEPT_BYTES = 8 << 20;
/** What a lexed text takes in Hermes's heap: a token about 116 bytes (measured: 44,168 tokens of 4 characters), its
 *  text up to two bytes a character. */
export const TOKEN_BYTES = 116;
export const textBytes = (text: string, tokens: number): number => text.length * 2 + tokens * TOKEN_BYTES;
/** The recent lexed texts of a language a longer text may extend, and the documents kept (highlights.rs DOCUMENTS). */
const EXTENDABLE = 24, DOCUMENTS = 8;
/** A text's turn at a document, in asks: one waiting a turn gets the next document, and a document whose text grew
 *  within a turn is kept for its own until then. */
const TURN = 2 * DOCUMENTS;
/** streamOf's answer for a text that needs a new document while none can be given it. */
const FULL = 'full';
/** highlightCode's `line` for a document with lines left, one to start again, and lexers that trapped. */
const MORE = -1, AGAIN = -2, FAILED = -3;

/** @pierre/diffs getFiletypeFromFileName, as the reference passes a file's path (its path in the project): the whole
 *  path as a name (a Dockerfile or CMakeLists.txt at the root; a nested one is no name), then a compound extension
 *  (component.ts), then a simple one; '' for a path it has no language for. */
export function fileLanguage(path: string): string {
  const has = (key: string) => Object.prototype.hasOwnProperty.call(FILE_TYPES, key);
  const custom = (key: string) => Object.prototype.hasOwnProperty.call(CUSTOM_EXTENSIONS, key);
  if (custom(path)) return CUSTOM_EXTENSIONS[path]!;
  if (has(path)) return FILE_TYPES[path]!;
  const compound = /\.([^/\\]+\.[^/\\]+)$/.exec(path)?.[1];
  if (compound && custom(compound)) return CUSTOM_EXTENSIONS[compound]!;
  if (compound && has(compound)) return FILE_TYPES[compound]!;
  const simple = /\.([^.]+)$/.exec(path)?.[1] ?? '';
  return custom(simple) ? CUSTOM_EXTENSIONS[simple]! : has(simple) ? FILE_TYPES[simple]! : '';
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
/** A view's answer (highlightView): the texts painted for it so far, and whether one of them waited for its tokens. */
interface Answer { painted: Set<Slot>; deferred: boolean }
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
  /** The last ask a request went to it. */
  grown: number;
}

const families = new Map<string, Map<string, Slot>>();
const waiting = new Set<Slot>();
const asked = new Map<number, Slot>();
/** The recent lexed texts of each family, newest last, for a text that extends one. */
const recent = new Map<string, Slot[]>();
let streams: Stream[] = [];
/** The texts each view's latest answer painted, and the answer each view has in flight (its newest). */
const shown = new Map<HighlightView, Set<Slot>>(), painting = new Map<HighlightView, Answer>();
let nextId = 1, nextDoc = 1, asks = 0, uses = 0;
/** How many times highlightTokens answered "not yet": a cache of highlight results checks it. */
export let highlightDeferrals = 0;

/** The tokens of a text under the lexer for `nameOrPath`; null when it paints plain (no lexer, or too long). A text
 *  not lexed yet is asked for and paints plain meanwhile, or with the tokens of a recent text it extends. With
 *  `maxLineLength`, a longer line is one plain token. `view` is the view it is painted for: its answer in flight
 *  keeps the text, and reports it while it waits. */
export function highlightTokens(text: string, nameOrPath: string, maxLineLength = 0, view: HighlightView | null = null): Token[] | null {
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
  const answer = view ? painting.get(view) : undefined;
  answer?.painted.add(slot);
  if (slot.tokens) return slot.tokens;
  highlightDeferrals++;
  if (answer) answer.deferred = true;
  const base = extendedSlot(family, text);
  return base?.tokens ? [...base.tokens, { text: text.slice(base.text.length), cls: '' }] : null;
}

/** For a cache of painted results: `view`'s answer paints this text's tokens again (it stays cached). */
export function highlightShown(text: string, nameOrPath: string, maxLineLength = 0, view: HighlightView | null = null): void {
  const slot = families.get(`${lexerOf(nameOrPath)}\u0000${maxLineLength}`)?.get(text);
  if (!slot) return;
  slot.used = ++uses;
  if (view) painting.get(view)?.painted.add(slot);
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

/** One view's answer (app.ts): it reports whether a text painted for the view waited for its tokens, and those texts
 *  stay cached until its next answer, so the cache never drops a text on screen. A newer answer of the view replaces
 *  one still in flight. A text painted for another view, while this one is in flight, is neither pinned nor reported. */
export async function highlightView<T extends object>(view: HighlightView, answer: () => T | Promise<T>): Promise<T & { highlightPending: boolean }> {
  const mine: Answer = { painted: new Set(), deferred: false };
  painting.set(view, mine);
  try {
    const result = await answer();
    if (painting.get(view) === mine) shown.set(view, mine.painted);
    return { ...result, highlightPending: mine.deferred };
  } finally {
    if (painting.get(view) === mine) painting.delete(view);
  }
}

function pinned(): Set<Slot> {
  const slots = new Set<Slot>();
  for (const painted of shown.values()) for (const slot of painted) slots.add(slot);
  for (const answer of painting.values()) for (const slot of answer.painted) slots.add(slot);
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
 *  of a text lexed whole (a streaming block grows); null for a text lexed whole. A text with a CR is a document only
 *  when long, and its versions start it again rather than append (highlightAsk), as the reference's
 *  incrementalHighlighting.ts lexes such a text whole: an append that splits a CRLF, or a version that ends in a CR,
 *  lexes its lines otherwise than the whole text does. FULL when no document can be given it yet. */
function streamOf(slot: Slot): Stream | null | typeof FULL {
  if (!slot.long && slot.text.includes('\r')) return null;
  let best: Stream | null = null;
  for (const stream of streams) {
    if (stream.family === slot.family && stream.text && slot.text.startsWith(stream.text) && (!best || stream.text.length > best.text.length)) best = stream;
  }
  if (best) return best;
  if (!slot.long && !extendedSlot(slot.family, slot.text)) return null;
  if (streams.length >= DOCUMENTS) {
    // highlights.rs drops its least recently used document to start another, the first here. It goes when its text has
    // stopped (no work in flight, no request for a turn), or when this text has waited a turn (its wait counts from its
    // first version, superseded): documents dropped sooner start their texts again every few rounds, and one kept
    // while it grows would starve this text.
    const oldest = streams[0]!;
    if ((working(oldest) || asks - oldest.grown < TURN) && asks - slot.since < TURN) return FULL;
    streams.shift();
  }
  const stream: Stream = { doc: nextDoc++, family: slot.family, text: '', lines: 0, final: [], finalChars: 0, restart: false, busy: false, grown: asks };
  streams.push(stream);
  return stream;
}

/** Whether a text waiting or asked for continues the document. */
function working(stream: Stream): boolean {
  const continues = (slot: Slot) => slot.family === stream.family && slot.text.startsWith(stream.text);
  for (const slot of waiting) if (continues(slot)) return true;
  for (const slot of asked.values()) if (continues(slot)) return true;
  return false;
}

/** The waiting texts no view paints that another waiting text of their family extends, each with the next that
 *  does. In sorted order a text's extensions follow it, the first of them next. */
function superseded(painted: Set<Slot>): [Slot, Slot][] {
  const order = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
  const sorted = [...waiting].sort((a, b) => order(a.family, b.family) || order(a.text, b.text)), found: [Slot, Slot][] = [];
  sorted.forEach((slot, at) => {
    const next = sorted[at + 1];
    if (next && !painted.has(slot) && next.family === slot.family && next.text.startsWith(slot.text)) found.push([slot, next]);
  });
  return found;
}

function drop(slot: Slot): void {
  waiting.delete(slot);
  families.get(slot.family)?.delete(slot.text);
}

/** The pump's ask (app.contract highlightStep): the waiting texts, the longest-waiting first (a long text lexed over
 *  several rounds, or one behind newer texts, keeps its place), whole texts up to ASK_CHARS and one request a document,
 *  each with an id for its answer; `pending` when more wait. A text that has waited STALE_ASKS asks with no view
 *  painting it is dropped, as is one no view paints that a newer waiting text extends (a streaming block's older
 *  version: its document takes the newest, not each in turn a round apart), and one a round asked for that never
 *  stored (its call failed) waits again. */
export function highlightAsk(): { requests: { id: number; language: number; doc: number; from: number; text: string }[]; pending: boolean } {
  asks++;
  for (const slot of asked.values()) {
    waiting.add(slot);
    if (slot.stream) slot.stream.restart = true; // the document may hold what the lost answer sent
  }
  asked.clear();
  for (const stream of streams) stream.busy = false;
  const painted = pinned(), requests: { id: number; language: number; doc: number; from: number; text: string }[] = [];
  // A newer version keeps the older one's place in the queue.
  for (const [slot, next] of superseded(painted)) {
    next.since = Math.min(next.since, slot.since);
    drop(slot);
  }
  let chars = 0;
  for (const slot of [...waiting].sort((a, b) => a.since - b.since)) {
    if (asks - slot.asked > STALE_ASKS && !painted.has(slot)) { drop(slot); continue; }
    const stream = streamOf(slot);
    if (stream === FULL) continue;
    let doc = 0, from = 0, text = slot.text;
    if (stream) {
      if (stream.busy) continue;
      if (stream.restart || !stream.text || (slot.text !== stream.text && slot.text.includes('\r'))) Object.assign(stream, { lines: 0, final: [], finalChars: 0, restart: false });
      else { from = stream.text.length; text = slot.text.slice(from); }
      Object.assign(stream, { text: slot.text, busy: true, grown: asks });
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

/** The lexed texts no view's latest answer paints, past KEPT or KEPT_BYTES, the least recently used first, go. */
function trim(): void {
  const painted = pinned(), lexed: Slot[] = [];
  for (const texts of families.values()) for (const slot of texts.values()) if (slot.tokens && !painted.has(slot)) lexed.push(slot);
  const bytes = (slot: Slot) => textBytes(slot.text, slot.tokens!.length);
  if (lexed.length <= KEPT && lexed.reduce((sum, slot) => sum + bytes(slot), 0) <= KEPT_BYTES) return;
  lexed.sort((a, b) => b.used - a.used);
  let kept = 0;
  lexed.forEach((slot, index) => {
    kept += bytes(slot);
    if (index < KEPT && kept <= KEPT_BYTES) return;
    families.get(slot.family)?.delete(slot.text);
    const texts = recent.get(slot.family);
    if (texts?.includes(slot)) recent.set(slot.family, texts.filter(entry => entry !== slot));
  });
}

/** dist/tokens.js lineRecordsToTokens: line records (a token's UTF-16 end and its type; at a line break the next
 *  line's start and 0xFFFFFFFF) as tokens over the whole text, line breaks plain, a line longer than `max` one plain
 *  token, and neighbours of one class merged. A document's live tokenizer ends a line at a lone CR, which the whole
 *  text's records keep in the line, at the end of the token before it: so does this. */
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
    if (type === 0xffffffff && end === start + 1 && text.charCodeAt(start) === 13 && tokens.length > lineToken) {
      tokens[tokens.length - 1]!.text += '\r';
      start = end;
      continue;
    }
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
