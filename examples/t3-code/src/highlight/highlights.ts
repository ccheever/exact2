// The reference's code colours: @pierre/highlights 0.2.0 (Apache-2.0), which highlights all of the reference's code
// (lib/syntaxHighlighting.ts: 80 languages, each a lexer in its dist/highlights.wasm; the others plain). Hermes runs
// no WebAssembly, so the data crate runs the lexers (data/src/highlights.rs), and a text reaches them through the
// root: a view asks here for a text's tokens, the root's pump (app.contract highlightStep) takes the waiting texts with
// highlightAsk, has the Rust source highlightCode lex them and hands their line records to highlightStore, and the
// view's next answer paints them. Until then a text paints plain, as the reference's code does while its highlighter
// loads, or keeps the tokens of an earlier text it extends (a streaming code block keeps its colours).

import { FILE_TYPES } from './file-languages';
import { LEXER_IDS, TYPE_CLASSES } from './highlights-tables';

/** A run of code text and its colour class (syntax-ink.contract synColor, synSlant, synWeight). */
export interface Token { text: string; cls: string }

/** @pierre/diffs' tokenizeMaxLineLength as the reference passes it: a longer line paints plain. */
export const TOKENIZE_MAX_LINE_LENGTH = 1_000;
/** A longer text paints plain. */
export const HIGHLIGHT_MAX_CHARS = 1_000_000;
/** The text one ask hands out at most, in UTF-16 units (highlights.rs BATCH_BYTES bounds it in UTF-8 bytes too). */
const ASK_CHARS = 256 * 1024;
/** A waiting text no view asked for again within this many asks is dropped (a streaming block's older versions). */
const STALE_ASKS = 60;
/** Lexed texts kept, and the recent ones a longer text of the same language may extend. */
const KEPT = 400, EXTENDABLE = 24;

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

interface Wait { key: string; lexer: number; max: number; text: string; asked: number }
const lexed = new Map<string, Token[]>();
const waiting = new Map<string, Wait>();
const asked = new Map<number, Wait>();
/** The recent lexed texts by language and line limit, newest last, for a text that extends one. */
const recent = new Map<string, string[]>();
let nextId = 1, asks = 0;
/** How many times highlightTokens answered "not yet": a cache of highlight results checks it. */
export let highlightDeferrals = 0;

const keyOf = (lexer: number, max: number, text: string) => `${lexer}\u0000${max}\u0000${text}`;

/** The tokens of a text under the lexer for `nameOrPath`; null when it paints plain (no lexer, or too long). A text
 *  not lexed yet is asked for and paints plain meanwhile, or with the tokens of a recent text it extends. With
 *  `maxLineLength`, a longer line is one plain token. */
export function highlightTokens(text: string, nameOrPath: string, maxLineLength = 0): Token[] | null {
  const lexer = lexerOf(nameOrPath);
  if (!lexer || text.length > HIGHLIGHT_MAX_CHARS) return null;
  const key = keyOf(lexer, maxLineLength, text);
  const known = lexed.get(key);
  if (known) { lexed.delete(key); lexed.set(key, known); return known; }
  const wait = waiting.get(key);
  if (wait) wait.asked = asks;
  else if (![...asked.values()].some(entry => entry.key === key)) waiting.set(key, { key, lexer, max: maxLineLength, text, asked: asks });
  highlightDeferrals++;
  return extended(lexer, maxLineLength, text);
}

/** The tokens of the longest recent text of this language that `text` starts with, and the rest plain. */
function extended(lexer: number, max: number, text: string): Token[] | null {
  let base = '';
  for (const earlier of recent.get(`${lexer}\u0000${max}`) ?? []) if (earlier.length > base.length && text.startsWith(earlier)) base = earlier;
  const tokens = base ? lexed.get(keyOf(lexer, max, base)) : undefined;
  return tokens ? [...tokens, { text: text.slice(base.length), cls: '' }] : null;
}

/** Whether a text waits for its tokens (the root's pump runs while this holds). */
export const highlightPending = (): boolean => waiting.size > 0 || asked.size > 0;
/** For an answer that starts now: whether a text waits for its tokens, or the answer painted one without them. */
export function highlightPendingSince(): () => boolean {
  const deferrals = highlightDeferrals;
  return () => highlightPending() || highlightDeferrals !== deferrals;
}

/** The pump's ask (app.contract highlightStep): the waiting texts, the most recently asked first, up to ASK_CHARS,
 *  each with an id for its answer; `pending` when more wait. A text no view asked for in STALE_ASKS asks is dropped, and
 *  one a round asked for that never stored (its call failed) waits again. */
export function highlightAsk(): { requests: { id: number; language: number; text: string }[]; pending: boolean } {
  asks++;
  for (const wait of asked.values()) waiting.set(wait.key, wait);
  asked.clear();
  const requests: { id: number; language: number; text: string }[] = [];
  let chars = 0;
  for (const wait of [...waiting.values()].reverse().sort((a, b) => b.asked - a.asked)) {
    if (asks - wait.asked > STALE_ASKS) { waiting.delete(wait.key); continue; }
    if (chars > 0 && chars + wait.text.length > ASK_CHARS) continue;
    chars += wait.text.length;
    const id = nextId++;
    waiting.delete(wait.key);
    asked.set(id, wait);
    requests.push({ id, language: wait.lexer, text: wait.text });
  }
  return { requests, pending: waiting.size > 0 };
}

/** The lexers' answer (highlightCode): each text's line records become its tokens. A text the answer left out (past
 *  its byte budget) waits again. `finished` when a text got its tokens. */
export function highlightStore(results: readonly { id: number; records: readonly number[] }[]): { finished: boolean; pending: boolean } {
  for (const { id, records } of results) {
    const wait = asked.get(id);
    if (!wait) continue;
    asked.delete(id);
    lexed.set(wait.key, tokensOf(wait.text, records, wait.max));
    const family = `${wait.lexer}\u0000${wait.max}`, texts = (recent.get(family) ?? []).filter(text => text !== wait.text);
    texts.push(wait.text);
    recent.set(family, texts.slice(-EXTENDABLE));
  }
  for (const wait of asked.values()) waiting.set(wait.key, wait);
  asked.clear();
  while (lexed.size > KEPT) lexed.delete(lexed.keys().next().value!);
  return { finished: results.length > 0, pending: waiting.size > 0 };
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

/** Tests: no lexed, waiting or asked texts. */
export function resetHighlights(): void {
  lexed.clear(); waiting.clear(); asked.clear(); recent.clear(); asks = 0;
}
