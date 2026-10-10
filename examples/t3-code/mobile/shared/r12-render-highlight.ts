// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r12-render-highlight.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r12-render: the shared highlighter's Shiki path. For a language the reference's Shiki 4.2
// grammars cover here (r12-render-grammar.ts: typescript, tsx, javascript, jsx, json, jsonc,
// shellscript, python, css, html, markdown, yaml, toml, rust, go, swift; r12-render-grammar-more.ts:
// c, java, kotlin, csharp, xml, diff, docker, make, ruby) the tokens are what the reference paints:
// r12-render-textmate.ts runs the grammar, and each run's pierre-light / pierre-dark colour pair
// names the synColor class that paints it (markdown.contract), with "+i" when the theme sets italic
// (synSlant). Chat code blocks, the diff panel, the Files panel, the attachment code preview and
// content-search lines all reach this through timeline-highlight.ts's highlight().
//
// shiki-residuals, cost: Hermes runs the slowest grammars here at about 70,000 characters a second,
// so once the app turns slicing on (startHighlightTurn), one answer tokenizes at most
// SLICE_BUDGET_CHARS (at most about 50 ms; a data source has no clock, so the budget is counted in
// characters). A text that does not finish keeps the heuristic tokenizer's colours (shikiTokens
// answers null) and goes on in highlightSlice turns, which the root drives while a resource reports
// highlightPending(); its Shiki tokens replace the heuristic ones when it is done.
// A text that extends a recently tokenized one (a streaming code block, the same file again)
// resumes from the last unchanged line's state.

import { colorOf, grammarId, styleOf, tokenizeLine, type LineState } from './r12-render-textmate';
import type { Cls, Token } from './timeline-highlight';

/** A longer text keeps the heuristic tokenizer (decision U15: the reference has no limit). */
export const SHIKI_MAX_CHARS = 1_000_000;
/** The characters one data-module turn tokenizes: at most about 50 ms in the pinned Hermes for the slowest
 *  grammars here (typescript, html; measurements in the shiki-residuals task record). */
export const SLICE_BUDGET_CHARS = 2_500;
/** A text nobody asked for again within this many turns is dropped (about five seconds of slices). */
const STALE_TURNS = 300;

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

// @pierre/diffs getFiletypeFromFileName, for the names and extensions whose grammar is here.
const EXTENSIONS: Record<string, string> = {
  sh: 'zsh', bash: 'zsh', zsh: 'zsh', css: 'css', go: 'go', html: 'html', htm: 'html', js: 'javascript', mjs: 'javascript',
  cjs: 'javascript', json: 'json', jsonc: 'jsonc', jsx: 'jsx', md: 'markdown', markdown: 'markdown', py: 'python', pyw: 'python',
  pyi: 'python', rs: 'rust', swift: 'swift', toml: 'toml', ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'tsx',
  yaml: 'yaml', yml: 'yml',
  // shiki-residuals
  c: 'c', cs: 'csharp', diff: 'diff', patch: 'diff', Dockerfile: 'dockerfile', dockerfile: 'dockerfile', java: 'java', kt: 'kotlin',
  Makefile: 'makefile', mk: 'makefile', makefile: 'makefile', rb: 'ruby', rbw: 'ruby', rake: 'ruby', gemspec: 'ruby', jbuilder: 'ruby',
  builder: 'ruby', rabl: 'ruby', arb: 'ruby', ru: 'ruby', podspec: 'ruby', Gemfile: 'ruby', Rakefile: 'ruby', Guardfile: 'ruby',
  Capfile: 'ruby', Berksfile: 'ruby', Brewfile: 'ruby', Vagrantfile: 'ruby', Thorfile: 'ruby', Appraisals: 'ruby', Dangerfile: 'ruby',
  xml: 'xml',
};
/** The grammar for a fence language (Shiki's id or alias) or a file path (its name or extension), or ''. */
export function shikiLanguage(nameOrPath: string): string {
  const raw = nameOrPath.trim();
  if (!raw) return '';
  if (!raw.includes('/') && !raw.includes('.')) {
    const id = grammarId(raw) || grammarId(raw.toLowerCase());
    if (id || !Object.prototype.hasOwnProperty.call(EXTENSIONS, raw) || raw[0] === raw[0]!.toLowerCase()) return id;
  }
  const base = raw.split(/[/\\]/).pop() ?? raw;
  if (/^[A-Z]/.test(base) && Object.prototype.hasOwnProperty.call(EXTENSIONS, base)) return grammarId(EXTENSIONS[base]!);
  const compound = /\.([^/\\]+\.[^/\\]+)$/.exec(raw)?.[1];
  if (compound && Object.prototype.hasOwnProperty.call(EXTENSIONS, compound)) return grammarId(EXTENSIONS[compound]!);
  const simple = /\.([^.]+)$/.exec(raw)?.[1] ?? '';
  return Object.prototype.hasOwnProperty.call(EXTENSIONS, simple) ? grammarId(EXTENSIONS[simple]!) : '';
}

/** A tokenized text: per line, the state after it and its class runs (classes resolved as lines finish). */
interface Document { lang: string; lines: string[]; states: LineState[]; runs: Token[][] }
const documents: Document[] = [];
const results = new Map<string, Token[]>();
/** A text still being tokenized: its lines, the states and runs so far, the turn it was last asked in. */
interface Job extends Document { key: string; asked: number }
const jobs = new Map<string, Job>();

// ── Turns and slices ────────────────────────────────────────────────────────────────────────
let slicing = false, turnLeft = Infinity, turn = 0;
/** The app calls this at the start of each answer: from then on a turn tokenizes at most `budget` characters. */
export function startHighlightTurn(budget = SLICE_BUDGET_CHARS): void {
  slicing = true;
  turnLeft = budget;
  turn++;
}
/** Tests: back to unsliced, everything tokenized at once, and no queued work. */
export function resetHighlightSlicing(): void {
  slicing = false; turnLeft = Infinity; turn = 0; jobs.clear(); results.clear(); documents.length = 0;
}
/** A text waiting for its Shiki tokens paints the heuristic colours up to this length and plain text
 *  above it: the TypeScript heuristic takes 16 ms for 20,000 characters but 1.2 s for 200,000 in Hermes. */
export const HEURISTIC_MAX_CHARS = 20_000;
/** Whether shikiTokens deferred this text to background turns. */
export function shikiDeferred(text: string, nameOrPath: string): boolean {
  const lang = shikiLanguage(nameOrPath);
  return !!lang && jobs.has(`${lang}\u0000${text}`);
}
/** Whether a text is waiting for its Shiki tokens (the root keeps slicing while this holds). */
export const highlightPending = (): boolean => jobs.size > 0;
/** How many times shikiTokens answered "not yet": a cache of highlight results checks it. */
export let highlightDeferrals = 0;

function newJob(text: string, lang: string, key: string): Job {
  const lines = text.split(/\r\n|\r|\n/);
  // Resume after the longest run of unchanged leading lines among the recent documents and jobs.
  let base: Document | null = null, shared = 0;
  const consider = (doc: Document, done: number) => {
    if (doc.lang !== lang) return;
    let same = 0;
    while (same < lines.length && same < done && doc.lines[same] === lines[same]) same++;
    if (same > shared) { shared = same; base = doc; }
  };
  for (const doc of documents) consider(doc, doc.lines.length);
  for (const job of jobs.values()) consider(job, job.states.length);
  const from = base as Document | null;
  return { key, lang, lines, asked: turn, states: from ? from.states.slice(0, shared) : [], runs: from ? from.runs.slice(0, shared) : [] };
}
/** Tokenizes the job's next lines while `budget.left` characters remain (a line costs its length
 *  and one); true when every line is done. */
function advance(job: Job, budget: { left: number }): boolean {
  let state: LineState = job.states.length ? job.states[job.states.length - 1]! : null;
  for (let index = job.states.length; index < job.lines.length; index++) {
    if (budget.left <= 0) return false;
    budget.left -= job.lines[index]!.length + 1;
    const result = tokenizeLine(job.lines[index]!, job.lang, state);
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
  documents.unshift({ lang: job.lang, lines: job.lines, states: job.states, runs: job.runs });
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
function withinTurn(work: (budget: { left: number }) => boolean): boolean {
  if (!slicing) return work({ left: Infinity });
  if (turnLeft <= 0) return false;
  const budget = { left: turnLeft };
  const done = work(budget);
  turnLeft = budget.left;
  return done;
}

/** One background turn (the root's highlightSlice mutation): the most recently asked texts go on
 *  for up to `chars` characters; a text nobody asked for in STALE_TURNS turns is dropped. */
export function highlightSlice(chars = SLICE_BUDGET_CHARS): { finished: boolean; pending: boolean } {
  for (const job of [...jobs.values()]) if (turn - job.asked > STALE_TURNS) jobs.delete(job.key);
  const budget = { left: chars };
  let finished = false;
  for (const job of [...jobs.values()].reverse()) {
    if (budget.left <= 0) break;
    if (advance(job, budget)) { finish(job); finished = true; }
  }
  return { finished, pending: jobs.size > 0 };
}

/** The reference's tokens for `text`, or null when no grammar here covers the language, the text is
 *  too long, or (slicing) its tokens are still being computed. */
export function shikiTokens(text: string, nameOrPath: string): Token[] | null {
  if (text.length > SHIKI_MAX_CHARS) return null;
  const lang = shikiLanguage(nameOrPath);
  if (!lang) return null;
  const key = `${lang}\u0000${text}`;
  const known = results.get(key);
  if (known) return known.map(token => ({ ...token }));
  let job = jobs.get(key);
  if (job) { jobs.delete(key); job.asked = turn; } else job = newJob(text, lang, key);
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
