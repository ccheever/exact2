// The shared highlighter's Shiki path. For every language the reference's Shiki 4.2
// bundle has (all 235, with their aliases, but the eight left out for their
// licences, which paint plain, LICENSE-T3; r12-render-grammar.ts and its chunks) the tokens are what the reference paints: r12-render-textmate.ts runs the grammar, and each
// run's pierre-light / pierre-dark colour pair names the synColor class that paints it
// (markdown.contract), with "+i" when the theme sets italic (synSlant). Chat code blocks, the diff
// panel, the Files panel, the attachment code preview and content-search lines all reach this
// through timeline-highlight.ts's highlight().
//
// Cost: Hermes runs the slowest grammars here at about 70,000 characters a second,
// so once the app turns slicing on (startHighlightTurn), one answer tokenizes at most
// SLICE_BUDGET_CHARS (at most about 50 ms; a data source has no clock, so the budget is counted in
// characters). A text that does not finish keeps the heuristic tokenizer's colours (shikiTokens
// answers null) and goes on in highlightSlice turns, which the root drives while a resource reports
// highlightPending(); its Shiki tokens replace the heuristic ones when it is done.
// A text that extends a recently tokenized one (a streaming code block, the same file again)
// resumes from the last unchanged line's state.

import { colorOf, grammarId, grammarWork, SCAN_UNITS_PER_MS, styleOf, tokenizeLineWithin, turnWork, unicodeText, type LineProgress, type LineState } from './r12-render-textmate';
import { FILE_TYPES } from './r12-render-filetypes';
import type { Cls, Token } from './timeline-highlight';

/** A longer text gets no Shiki tokens (the reference has no limit): highlight() paints it as
 *  plain text, since it is over HEURISTIC_MAX_CHARS too. */
export const SHIKI_MAX_CHARS = 1_000_000;
/** The characters one data-module turn tokenizes: at most about 50 ms in the pinned Hermes for the slowest
 *  grammars here (typescript, html; measured in that Hermes). */
export const SLICE_BUDGET_CHARS = 2_500;
/** The regex work one turn may do (r12-render-textmate.ts turnWork, checked before each search): a line
 *  that costs more (a template-dense cpp line: its function-definition regexes traverse the rest of the line once per
 *  template) goes on next turn from the scan it was in, so a turn stays inside the runner's 100 ms answer budget while the
 *  line runs to Shiki's 500 ms limit (LINE_SCAN_LIMIT). */
export const SLICE_SCAN_UNITS = 33 * SCAN_UNITS_PER_MS;
/** The share of a turn a line that starts the turn may spend on one-time work beside its own cost (advance). */
export const FIRST_LINE_WORK = 0.5;
/** A text nobody asked for again within this many turns is dropped (about five seconds of slices). */
const STALE_TURNS = 300;
/** Grammars Hermes runs well under TypeScript's 70,000 characters a second,
 *  with the weight each of their characters takes in the slice budget. Measured over the grammar samples
 *  and real files: cpp 37,000 a second; every other grammar
 *  52,000 (javascript) or more. */
export const SLOW_LANGUAGES: Record<string, number> = { cpp: 2 };
/** The same for a line with a character outside ASCII, whose regexes run in their Unicode \b forms:
 *  most grammars take about twice as long there (the default, 2); these more (the same measurement,
 *  each sample with Korean text on every line). */
export const UNICODE_SLOW_LANGUAGES: Record<string, number> = {
  asm: 7, d: 6, 'fortran-fixed-form': 6, 'fortran-free-form': 5, surrealql: 4, codeql: 3, cpp: 3, llvm: 3, swift: 3, 'system-verilog': 3,
};

/** Each pierre-light / pierre-dark foreground pair the themes define, as its synColor class. */
export const CLASS_OF_PAIR: Record<string, Cls> = {
  '#d32a61/#ff678d': 'kw', '#a631be/#d568ea': 'decl', '#693acf/#9d6afb': 'fn', '#d47628/#ffa359': 'var',
  '#d5901c/#ffab16': 'const', '#636363/#a3a3a3': 'param', '#d5a910/#ffd452': 'flag', '#199f43/#5ecc71': 'str',
  '#16a994/#61d5c0': 'esc', '#1ca1c7/#68cdf2': 'num', '#08c0ef/#08c0ef': 'op', '#636363/#636363': 'punct',
  '#737373/#737373': 'com', '#d5512f/#ff855e': 'tag', '#18a46c/#60d199': 'attr', '#17a5af/#64d1db': 'regex',
  '#1a85d4/#69b1ff': 'deco', '#0a0a0a/#fafafa': '',
};
const classCache = new Map<string, Cls>();
function classOf(scopes: string[]): Cls {
  const key = scopes.join(' ');
  let cls = classCache.get(key);
  if (cls === undefined) {
    const base = CLASS_OF_PAIR[`${colorOf(scopes, 'light')}/${colorOf(scopes, 'dark')}`] ?? '';
    cls = styleOf(scopes, 'light').italic ? `${base}+i` as Cls : base;
    if (classCache.size > 4000) classCache.clear();
    classCache.set(key, cls);
  }
  return cls;
}

/** The grammar @pierre/diffs' getFiletypeFromFileName picks for a file path as the reference passes it (the
 *  path in its project): the whole path as a name (a Dockerfile or CMakeLists.txt at the root; a nested one is no
 *  name), then a compound extension (component.ts), then a simple one; '' (plain text) for a path it lacks. */
export function fileLanguage(path: string): string {
  const has = (key: string) => Object.prototype.hasOwnProperty.call(FILE_TYPES, key);
  if (has(path)) return grammarId(FILE_TYPES[path]!);
  const compound = /\.([^/\\]+\.[^/\\]+)$/.exec(path)?.[1];
  if (compound && has(compound)) return grammarId(FILE_TYPES[compound]!);
  const simple = /\.([^.]+)$/.exec(path)?.[1] ?? '';
  return has(simple) ? grammarId(FILE_TYPES[simple]!) : '';
}
/** The grammar for a fence word or a file path, or '' (plain text). A word (no "/" or ".") is a fence word,
 *  which the reference's Shiki loads as written: one of its ids or aliases (```Python is no language there), and
 *  gitignore as ini (ChatMarkdown.tsx extractFenceLanguage). Anything else is a path (fileLanguage); a caller
 *  with a path that may be a bare name (a root Dockerfile) asks fileLanguage itself. */
export function shikiLanguage(nameOrPath: string): string {
  const raw = nameOrPath.trim();
  if (!raw) return '';
  if (!raw.includes('/') && !raw.includes('.')) return grammarId(raw === 'gitignore' ? 'ini' : raw);
  return fileLanguage(raw);
}

/** A tokenized text: its line limit (0 for none; TOKENIZE_MAX_LINE_LENGTH), per line the state after it and
 *  its class runs (classes resolved as lines finish). */
interface Document { lang: string; max: number; lines: string[]; states: LineState[]; runs: Token[][] }
const documents: Document[] = [];
const results = new Map<string, Token[]>();
/** A text still being tokenized: its lines, the states and runs so far, the turn it was last asked in, and its next line's
 *  progress when a turn's budget stopped that line part way. */
interface Job extends Document { key: string; asked: number; progress: LineProgress | null }
const jobs = new Map<string, Job>();

// ── Turns and slices ────────────────────────────────────────────────────────────────────────
let slicing = false, turnLeft = Infinity, turnWhole = Infinity, turnScan = Infinity, turnScanWhole = Infinity, turn = 0;
/** The app calls this at the start of each answer: from then on a turn tokenizes at most `budget` characters and does at
 *  most `scan` units of regex work. */
export function startHighlightTurn(budget = SLICE_BUDGET_CHARS, scan = SLICE_SCAN_UNITS): void {
  slicing = true;
  turnLeft = turnWhole = budget;
  turnScan = turnScanWhole = scan;
  turn++;
}
/** Tests: back to unsliced, everything tokenized at once, and no queued work. */
export function resetHighlightSlicing(): void {
  slicing = false; turnLeft = turnWhole = turnScan = turnScanWhole = Infinity; turn = 0; jobs.clear(); results.clear(); documents.length = 0;
}
/** A text waiting for its Shiki tokens paints the heuristic colours up to this length and plain text
 *  above it: the TypeScript heuristic takes 16 ms for 20,000 characters but 1.2 s for 200,000 in Hermes. */
export const HEURISTIC_MAX_CHARS = 20_000;
/** Whether shikiTokens deferred this text to background turns. */
export function shikiDeferred(text: string, nameOrPath: string, maxLineLength = 0): boolean {
  const lang = shikiLanguage(nameOrPath);
  return !!lang && jobs.has(textKey(lang, maxLineLength, text));
}
const textKey = (lang: string, max: number, text: string) => `${lang}\u0000${max}\u0000${text}`;
/** Whether a text is waiting for its Shiki tokens (the root keeps slicing while this holds). */
export const highlightPending = (): boolean => jobs.size > 0;
/** How many times shikiTokens answered "not yet": a cache of highlight results checks it. */
export let highlightDeferrals = 0;
/** For an answer that starts now: whether a text waits for its tokens, or the answer painted one without them. The
 *  second holds after that text's job has finished, while the answer's value still has the heuristic colours: the
 *  shell asks for the Files panel's tokens before its last awaits, and a slice may finish the job during them. */
export function highlightPendingSince(): () => boolean {
  const deferrals = highlightDeferrals;
  return () => jobs.size > 0 || highlightDeferrals !== deferrals;
}

function newJob(text: string, lang: string, max: number, key: string): Job {
  const lines = text.split(/\r\n|\r|\n/);
  // Resume after the longest run of unchanged leading lines among the recent documents and jobs.
  let base: Document | null = null, shared = 0;
  const consider = (doc: Document, done: number) => {
    if (doc.lang !== lang || doc.max !== max) return;
    let same = 0;
    while (same < lines.length && same < done && doc.lines[same] === lines[same]) same++;
    if (same > shared) { shared = same; base = doc; }
  };
  for (const doc of documents) consider(doc, doc.lines.length);
  for (const job of jobs.values()) consider(job, job.states.length);
  const from = base as Document | null;
  return { key, lang, max, lines, asked: turn, states: from ? from.states.slice(0, shared) : [], runs: from ? from.runs.slice(0, shared) : [], progress: null };
}
/** A turn's budget: what is left of it, all of it, and the regex work left and in all (scan units). */
interface Budget { left: number; whole: number; scan: number; scanWhole: number }
/** Tokenizes the job's next lines while `budget.left` characters remain (a line costs its length
 *  and one); true when every line is done. */
function advance(job: Job, budget: Budget): boolean {
  let state: LineState = job.states.length ? job.states[job.states.length - 1]! : null;
  for (let index = job.states.length; index < job.lines.length; index++) {
    if (budget.left <= 0 || budget.scan <= 0) return false;
    const line = job.lines[index]!;
    if (job.max && line.length >= job.max) {
      // @pierre/diffs' tokenizeMaxLineLength: Shiki paints such a line plain and starts the next one in the
      // state before it (tokenizeWithTheme).
      job.states.push(state);
      job.runs.push([{ text: line, cls: '' }]);
      budget.left -= 1;
      continue;
    }
    // A slow grammar's characters, and a line outside ASCII (Unicode \b
    // forms), count more; the line's one-time work (chunks parsed, regexes compiled on first use) is
    // charged too, and a line whose compiles would pass what is left stops and goes on next turn.
    const cost = (line.length + 1) * (unicodeText(line) ? UNICODE_SLOW_LANGUAGES[job.lang] ?? 2 : SLOW_LANGUAGES[job.lang] ?? 1);
    // A line that starts its turn may do half a turn of one-time work whatever its own cost: each try
    // that stops tokenizes the line again from its start, so a long line (one past a whole turn, say)
    // then takes a few turns on a grammar's first use, not one per regex.
    const limit = budget.left >= budget.whole ? Math.max(budget.left - cost, budget.whole * FIRST_LINE_WORK) : budget.left - cost;
    const before = grammarWork();
    const result = tokenizeLineWithin(line, job.lang, state, limit, job.progress, budget.scan, budget.scan >= budget.scanWhole);
    budget.left -= grammarWork() - before;
    budget.scan -= turnWork();
    // A line stopped part way (a compile over the limit, or the turn's regex work) goes on next turn from its progress;
    // one stopped before its first scan starts again from its state.
    if (!result || 'progress' in result) { job.progress = result?.progress ?? null; budget.left = Math.min(budget.left, 0); return false; }
    job.progress = null;
    budget.left -= cost;
    state = result.state;
    const runs: Token[] = [];
    for (const piece of result.pieces) {
      const cls = classOf(piece.scopes), last = runs[runs.length - 1];
      if (last && last.cls === cls) last.text += piece.text; else runs.push({ text: piece.text, cls });
    }
    job.states.push(state);
    job.runs.push(runs);
  }
  return true;
}
function finish(job: Job): Token[] {
  jobs.delete(job.key);
  documents.unshift({ lang: job.lang, max: job.max, lines: job.lines, states: job.states, runs: job.runs });
  if (documents.length > 6) documents.pop();
  const tokens: Token[] = [];
  const push = (value: string, cls: Cls) => {
    const last = tokens[tokens.length - 1];
    if (last && last.cls === cls) last.text += value; else tokens.push({ text: value, cls });
  };
  job.runs.forEach((line, index) => {
    if (index) push('\n', '');
    for (const run of line) push(run.text, run.cls);
  });
  if (results.size > 64) results.delete(results.keys().next().value!);
  results.set(job.key, tokens);
  return tokens;
}
/** Runs `work` within what is left of this turn's budget. */
function withinTurn(work: (budget: Budget) => boolean): boolean {
  if (!slicing) return work({ left: Infinity, whole: Infinity, scan: Infinity, scanWhole: Infinity });
  if (turnLeft <= 0 || turnScan <= 0) return false;
  const budget = { left: turnLeft, whole: turnWhole, scan: turnScan, scanWhole: turnScanWhole };
  const done = work(budget);
  turnLeft = budget.left;
  turnScan = budget.scan;
  return done;
}

/** One background turn (the root's highlightSlice mutation): the most recently asked texts go on
 *  for up to `chars` characters; a text nobody asked for in STALE_TURNS turns is dropped. */
export function highlightSlice(chars = SLICE_BUDGET_CHARS, scan = SLICE_SCAN_UNITS): { finished: boolean; pending: boolean } {
  for (const job of [...jobs.values()]) if (turn - job.asked > STALE_TURNS) jobs.delete(job.key);
  const budget = { left: chars, whole: chars, scan, scanWhole: scan };
  let finished = false;
  for (const job of [...jobs.values()].reverse()) {
    if (budget.left <= 0 || budget.scan <= 0) break;
    if (advance(job, budget)) { finish(job); finished = true; }
  }
  return { finished, pending: jobs.size > 0 };
}

/** @pierre/diffs' tokenizeMaxLineLength, its default and what T3 Code's DiffWorkerPoolProvider.tsx sets: in the
 *  Files panel and diffs a line this long or longer is not tokenized (lineTokens). Chat code blocks and search
 *  lines go through Shiki with no limit. */
export const TOKENIZE_MAX_LINE_LENGTH = 1_000;
/** The reference's tokens for `text`, or null when no grammar here covers the language, the text is
 *  too long, or (slicing) its tokens are still being computed. With `maxLineLength`, a line that long or
 *  longer paints plain (TOKENIZE_MAX_LINE_LENGTH). */
export function shikiTokens(text: string, nameOrPath: string, maxLineLength = 0): Token[] | null {
  if (text.length > SHIKI_MAX_CHARS) return null;
  const lang = shikiLanguage(nameOrPath);
  if (!lang) return null;
  const key = textKey(lang, maxLineLength, text);
  const known = results.get(key);
  if (known) return known.map(token => ({ ...token }));
  let job = jobs.get(key);
  if (job) { jobs.delete(key); job.asked = turn; } else job = newJob(text, lang, maxLineLength, key);
  jobs.set(key, job); // the most recently asked text is sliced first
  let tokens: Token[] | null = null;
  try {
    if (withinTurn(budget => advance(job!, budget))) tokens = finish(job);
  } catch {
    // A grammar that throws paints as plain text, as the reference falls back to "text".
    jobs.delete(key);
    tokens = [{ text, cls: '' }];
    results.set(key, tokens);
  }
  if (!tokens) { highlightDeferrals++; return null; }
  return tokens.map(token => ({ ...token }));
}
