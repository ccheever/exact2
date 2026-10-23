// The reference colours code with Shiki 4.2 (TextMate grammars run by
// vscode-textmate, the pierre-light / pierre-dark themes). This is that tokenizer, as small as it
// can be: the grammars and themes are data (grammar.ts, generated from the reference's
// own packages, regexes converted ahead of time as Shiki's JavaScript engine converts them), and
// this file runs them with vscode-textmate's rules (MIT; see LICENSE-T3): one line at a time with
// the line's "\n", the earliest match wins (ties to the first pattern), begin/end and begin/while
// rules stack, \A / \G variants, end back-references, nested captures, the loop guards that stop
// a line when a rule does not advance, and the theme trie's specificity for colours and font styles.
// Every grammar of Shiki's bundle is here (but the eight its generator leaves out for
// their licences, LICENSE-T3: each is an empty grammar, so what embeds one paints that part plain), in one rule space; each
// grammar's rules, scope names and regexes are a chunk (grammar-<n>.ts) parsed the first
// time a rule of it is needed, so a language costs nothing until it is highlighted.

import { GRAMMAR_CHUNKS, GRAMMAR_INDEX } from './grammar';

/** A converted regex: source, flags, hidden captures, capture transfers, clip-search strategy
 *  (oniguruma-to-es's EmulatedRegExp options), a back-reference marker, or four \A/\G variants
 *  (in the data a variant equal to an earlier one is that one's index; load() resolves it), and
 *  `u`, its form with Oniguruma's Unicode \b where that differs from the ASCII one (see exec).
 *  `o` marks an end / while regex made from one begin match (its back-references resolved). */
interface Rx { s: string; f: string; h?: number[]; t?: [number, number[]][]; c?: 1; b?: 1; v?: Rx[]; u?: Rx; o?: 1; re?: RegExp }
interface Capture { n?: number; c?: number; r?: number }
type Captures = Record<string, Capture>;
interface Rule { i?: number; n?: number; c?: number; m?: number; mc?: Captures; b?: number; bc?: Captures; e?: number; ec?: Captures; w?: number; wc?: Captures; l?: 1; p?: number[] }
/** A theme rule: selector, foreground ('' for none) and, when the rule sets one, its fontStyle. */
interface ThemeData { fg: string; rules: ([string, string] | [string, string, string])[] }
interface Index {
  roots: Record<string, number>; aliases: Record<string, string>; injections: Record<string, [string, number][]>;
  /** Per chunk, the first rule id, scope name index and regex index it holds. */
  chunks: [number, number, number][]; counts: [number, number, number]; themes: { light: ThemeData; dark: ThemeData };
}
let parsedIndex: Index | null = null;
const index = (): Index => parsedIndex ??= JSON.parse(GRAMMAR_INDEX) as Index;
const own = (table: object, key: string) => Object.prototype.hasOwnProperty.call(table, key);
/** The loaded rules, scope names and regexes, by their global ids (holes until their chunk loads). */
const RULES: Rule[] = [], NAMES: string[] = [], REGEXES: Rx[] = [];
const loadedChunks = new Set<number>();
/** The one-time work done so far (chunks parsed, regexes compiled), in the slicer's unit: characters
 *  tokenized at TypeScript's speed in Hermes (about 70,000 a second). Hermes compiles cpp's regexes,
 *  the costliest ASCII ones here, at about 0.4 µs a source character, so a regex costs one unit per 30
 *  of its characters, and four more per Unicode property (a Unicode \b form has four classes of four,
 *  each built when compiled: asm's 280 compile in about 430 ms); a chunk parses at one unit per 400
 *  characters. */
let work = 0;
export const grammarWork = (): number => work;
/** Parses one grammar's chunk into the tables. */
function load(chunk: number): void {
  if (loadedChunks.has(chunk)) return;
  loadedChunks.add(chunk);
  const [ruleAt, nameAt, regexAt] = index().chunks[chunk]!;
  const text = GRAMMAR_CHUNKS[chunk]!();
  work += text.length / 400;
  const data = JSON.parse(text) as { r: Rule[]; n: string[]; x: (Rx & { v?: (Rx | number)[] })[] };
  data.r.forEach((one, at) => { RULES[ruleAt + at] = one; });
  data.n.forEach((one, at) => { NAMES[nameAt + at] = one; });
  data.x.forEach((one, at) => {
    if (one.v) { const variants = one.v; one.v = variants.map(variant => typeof variant === 'number' ? variants[variant] as Rx : variant); }
    REGEXES[regexAt + at] = one as Rx;
  });
}
/** The rule with this id, its grammar's chunk loaded on first use. */
function ruleOf(id: number): Rule {
  const known = RULES[id];
  if (known) return known;
  const chunks = index().chunks;
  let low = 0, high = chunks.length - 1;
  while (low < high) { const mid = (low + high + 1) >> 1; if (chunks[mid]![0] <= id) low = mid; else high = mid - 1; }
  load(low);
  return RULES[id]!;
}
/** How many grammar chunks have been parsed (tests). */
export const loadedGrammarChunks = (): number => loadedChunks.size;

/** The Shiki language id a grammar here has for a language name or alias, or ''. */
export function grammarId(lang: string): string {
  const table = index(), id = own(table.aliases, lang) ? table.aliases[lang]! : lang;
  return own(table.roots, id) ? id : '';
}

// ── Regexes ─────────────────────────────────────────────────────────────────────────────────
interface Match { start: number; end: number; indices: ([number, number] | undefined)[] }
/** While a line runs under a limit (tokenizeLineWithin): the work when it began, how many grammar
 *  regexes it has compiled, and how much one-time work it may do. */
let lineWork = 0, lineCompiled = 0, workLimit = Infinity;
const OVER_LIMIT = new Error('over the line\'s work limit');
/** Shiki stops a line after 500 ms (`tokenizeTimeLimit`, @shikijs/primitive;
 *  vscode-textmate's _tokenizeString checks the time before each scan, keeps the tokens made so far and gives the next line
 *  the stack where it stopped). A data module has no clock, so time is counted as regex work: a search costs the characters
 *  it scanned (from its start to its match, or to the line's end) times its pattern's length. Measured in the pinned Hermes
 *  over whole lines, about 1,500,000 units a millisecond where it costs most (a template-dense cpp line; minified JavaScript
 *  and CSS run at that rate or faster). Over a measured corpus (205,144 lines) no line under 2,000 characters comes within
 *  1/8 of the limit; the lines that reach it are minified bundles of 19,667 characters or more. */
export const SCAN_UNITS_PER_MS = 1_500_000;
/** Shiki's tokenizeTimeLimit, in scan units. */
export const LINE_SCAN_LIMIT = 500 * SCAN_UNITS_PER_MS;
let scanned = 0;
/** The scan work done so far (scan units). */
export const scanWork = (): number => scanned;
/** A search whose rest of the line is longer than this is charged to the turn at its whole rest times its pattern before it
 *  runs: cpp's function-definition regexes (11,675 characters) traverse the rest of a template-dense line on every search,
 *  whatever they match (about 46 ms for 2,900 characters), which the characters to the match do not show. */
const LONG_REST = 400;
/** While a line runs under a turn's budget (tokenizeLineWithin): the turn's regex work so far (the long-rest searches' charges;
 *  the slicer's character budget bounds the rest), its cap, and whether a search ran this turn. A search that would pass the cap, after another search of the
 *  turn, suspends the line before it runs (OVER_TURN): the line goes on next turn from the scan it was in, whose finished
 *  searches its memo keeps. */
let turnSpent = 0, turnCap = Infinity, turnSearched = false;
const OVER_TURN = new Error('over the turn\'s regex work');
/** The turn's regex work charged by the last tokenizeLineWithin call (scan units). */
export const turnWork = (): number => turnSpent;
/** The entry's RegExp, compiled once and kept on the entry. A line stops before compiling a grammar
 *  regex that would pass its limit, once it has compiled one: those stay compiled, so each try gets
 *  further and a line retried from its start always finishes. A regex made from a begin match (`o`)
 *  never stops a line, since a retry makes it again; its compile is counted as work all the same. */
function compile(rx: Rx): RegExp {
  if (!rx.re) {
    const cost = 1 + rx.s.length / 30 + 4 * (rx.s.split('\\p{').length - 1);
    if (!rx.o) {
      if (lineCompiled > 0 && work - lineWork + cost > workLimit) throw OVER_LIMIT;
      lineCompiled++;
    }
    work += cost;
    rx.re = new RegExp(rx.s, rx.f + 'dg');
  }
  return rx.re;
}
function variant(index: number, allowA: boolean, allowG: boolean): Rx {
  const rx = REGEXES[index]!;
  return rx.v ? rx.v[(allowA ? 2 : 0) + (allowG ? 1 : 0)]! : rx;
}
/** Whether the line being tokenized has a character outside ASCII: only there does Oniguruma's
 *  Unicode \b differ from the faster ASCII one. */
let unicodeLine = false;
function exec(given: Rx, line: string, from: number): Match | null {
  const rx = unicodeLine && given.u ? given.u : given;
  const re = compile(rx);
  // clip_search: the regex sees the line from `from` on (its \G became ^), offsets added back.
  const offset = rx.c && from ? from : 0, text = offset ? line.slice(offset) : line;
  re.lastIndex = from - offset;
  let bound = 0;
  if (turnCap !== Infinity) {
    const rest = line.length - from + 1;
    bound = rest > LONG_REST ? rest * rx.s.length : 0;
    if (turnSearched && turnSpent + bound > turnCap) throw OVER_TURN;
    turnSearched = true;
    turnSpent += bound;
  }
  const found = re.exec(text) as (RegExpExecArray & { indices?: ([number, number] | undefined)[] }) | null;
  const units = ((found ? found.index + offset : line.length) - from + 1) * rx.s.length;
  scanned += units;
  if (!found || !found.indices) return null;
  let indices = found.indices.map(at => at && offset ? [at[0] + offset, at[1] + offset] as [number, number] : at);
  if (rx.h || rx.t) {
    // EmulatedRegExp: hidden captures drop out; a transferred capture lands on its target group.
    const hidden = new Set(rx.h ?? []), transferTo = new Map<number, number>();
    for (const [to, from] of rx.t ?? []) for (const group of from) transferTo.set(group, to);
    const mapped: ([number, number] | undefined)[] = [indices[0]], numbers: (number | null)[] = [0];
    for (let group = 1; group < indices.length; group++) {
      if (hidden.has(group)) numbers.push(null); else { numbers.push(mapped.length); mapped.push(indices[group]); }
      const to = transferTo.get(group);
      if (to !== undefined && indices[group]) { const target = numbers[to]; if (target) mapped[target] = indices[group]; }
    }
    indices = mapped;
  }
  return { start: found.index + offset, end: found.index + offset + found[0].length, indices };
}
// vscode-textmate escapes a captured back-reference for Oniguruma; these are the characters a
// JavaScript unicode-mode pattern lets (and needs) a backslash before.
const escapeRx = (text: string) => text.replace(/[\\^$.*+?()[\]{}|\/-]/g, character => character === '-' ? '\\x2d' : '\\' + character);
/** End / while regexes made from begin matches, by the regex and its resolved source: a text's `**` or
 *  `r#"` is compiled once, as Shiki's JavaScript engine keeps compiled patterns by source. */
const resolved = new Map<string, Rx>();
/** The end / while regex `index` with its back-references (U+E000+n, see the generator) resolved from the begin match. */
function resolveBackrefs(index: number, line: string, begin: Match): Rx {
  const rx = REGEXES[index]!;
  const resolve = (source: string) => source.replace(/[\ue000-\ue0ff]/g, mark => {
    const at = begin.indices[mark.charCodeAt(0) - 0xe000];
    return at ? escapeRx(line.slice(at[0], at[1])) : '';
  });
  // Every form (the \A / \G variants, the Unicode \b one) holds the same back-references as the first.
  const key = `${index}\u0000${resolve((rx.v ? rx.v[0]! : rx).s)}`;
  const known = resolved.get(key);
  if (known) return known;
  const swap = (one: Rx): Rx => ({ s: resolve(one.s), f: one.f, o: 1, ...(one.h ? { h: one.h } : {}), ...(one.t ? { t: one.t } : {}), ...(one.c ? { c: one.c } : {}), ...(one.u ? { u: swap(one.u) } : {}) });
  const made: Rx = rx.v ? { s: '', f: '', o: 1, v: rx.v.map(swap) } : swap(rx);
  if (resolved.size >= 2000) resolved.clear();
  resolved.set(key, made);
  return made;
}

// ── Rules and scanners ──────────────────────────────────────────────────────────────────────
/** The match / begin rules a rule's patterns reach, includes and pattern-only rules expanded. */
const patternCache = new Map<number, number[]>();
function patterns(id: number): number[] {
  const known = patternCache.get(id);
  if (known) return known;
  const out: number[] = [], seen = new Set<number>();
  const visit = (ruleId: number) => {
    if (ruleId < 0 || seen.has(ruleId)) return;
    const rule = ruleOf(ruleId);
    if (rule.i !== undefined) { visit(rule.i); return; }
    if (rule.m !== undefined || rule.b !== undefined) { out.push(ruleId); return; }
    seen.add(ruleId);
    for (const child of rule.p ?? []) visit(child);
  };
  for (const child of ruleOf(id).p ?? []) visit(child);
  patternCache.set(id, out);
  return out;
}

interface Frame {
  rule: number; parent: Frame | null; enter: number; anchor: number; eol: boolean;
  end: Rx | null; names: string[]; content: string[];
}
const END = -1;
/** A regex's last search on the current line: the leftmost match from `from` stays the answer for
 *  any later start up to that match (none in between matched), unless the regex is sticky.
 *  Each line keeps its own (LineProgress.memo), which a capture's retokenizing (another string) no longer
 *  clears: when it did, cpp's function-definition regexes searched the rest of a template-dense line again at every
 *  template (2,960 characters: 6.2 s in Hermes; now 0.25 s, the same tokens). */
let memoLine = '';
type Memo = Map<Rx, { from: number; match: Match | null }>;
let memo: Memo = new Map();
function search(rx: Rx, line: string, pos: number): Match | null {
  if (line !== memoLine) { memo = new Map(); memoLine = line; }
  const known = memo.get(rx);
  if (known && known.from <= pos && !rx.c && !rx.f.includes('y') && (known.match === null || known.match.start >= pos)) return known.match;
  const match = exec(rx, line, pos);
  memo.set(rx, { from: pos, match });
  return match;
}
function scanRule(frame: Frame, line: string, pos: number, first: boolean, anchor: number): { rule: number; match: Match } | null {
  const rule = ruleOf(frame.rule);
  const list = patterns(frame.rule);
  const allowA = first, allowG = pos === anchor;
  let best: { rule: number; match: Match } | null = null;
  const consider = (ruleId: number, rx: Rx) => {
    const match = search(rx, line, pos);
    if (match && (!best || match.start < best.match.start)) best = { rule: ruleId, match };
  };
  const end = frame.end && rule.e !== undefined ? (frame.end.v ? frame.end.v[(allowA ? 2 : 0) + (allowG ? 1 : 0)]! : frame.end) : null;
  if (end && !rule.l) consider(END, end);
  for (const id of list) {
    if (best && (best as { match: Match }).match.start === pos) break;
    const target = ruleOf(id);
    consider(id, variant(target.m ?? target.b!, allowA, allowG));
  }
  if (end && rule.l && !(best && (best as { match: Match }).match.start === pos)) consider(END, end);
  return best;
}

// ── Injections (the root grammar's `injections`, vscode-textmate's matcher.ts) ──────────────
type Matcher = (scopes: string[]) => boolean;
interface Injection { matcher: Matcher; rule: number; priority: number }
const scopesMatch = (scope: string, selector: string) => scope === selector || scope.length > selector.length && scope.startsWith(selector) && scope[selector.length] === '.';
function nameMatcher(identifiers: string[], scopes: string[]): boolean {
  if (scopes.length < identifiers.length) return false;
  let last = 0;
  return identifiers.every(identifier => {
    for (let index = last; index < scopes.length; index++) if (scopesMatch(scopes[index]!, identifier)) { last = index + 1; return true; }
    return false;
  });
}
function createMatchers(selector: string): { matcher: Matcher; priority: number }[] {
  const tokens = selector.match(/([LR]:|[\w.:][\w.:-]*|[,|\-()])/g) ?? [];
  let at = 0, token: string | null = tokens[0] ?? null;
  const next = () => { at++; token = tokens[at] ?? null; };
  const isIdentifier = (value: string | null) => !!value && /[\w.:]+/.test(value);
  const results: { matcher: Matcher; priority: number }[] = [];
  function operand(): Matcher | null {
    if (token === '-') { next(); const negated = operand(); return scopes => !!negated && !negated(scopes); }
    if (token === '(') { next(); const inner = expression(); if ((token as string | null) === ')') next(); return inner; }
    if (isIdentifier(token)) { const identifiers: string[] = []; do { identifiers.push(token!); next(); } while (isIdentifier(token)); return scopes => nameMatcher(identifiers, scopes); }
    return null;
  }
  function conjunction(): Matcher { const list: Matcher[] = []; let one = operand(); while (one) { list.push(one); one = operand(); } return scopes => list.every(m => m(scopes)); }
  function expression(): Matcher {
    const list: Matcher[] = [];
    let one: Matcher | null = conjunction();
    while (one) {
      list.push(one);
      if (token === '|' || token === ',') { do next(); while (token === '|' || token === ','); } else break;
      one = conjunction();
    }
    return scopes => list.some(m => m(scopes));
  }
  while (token !== null) {
    let priority = 0;
    if (token.length === 2 && token[1] === ':') { priority = token[0] === 'R' ? 1 : token[0] === 'L' ? -1 : 0; next(); }
    results.push({ matcher: conjunction(), priority });
    if (token !== ',') break;
    next();
  }
  return results;
}
const injectionCache = new Map<string, Injection[]>();
function injectionsOf(lang: string): Injection[] {
  let known = injectionCache.get(lang);
  if (!known) {
    known = [];
    for (const [selector, rule] of own(index().injections, lang) ? index().injections[lang]! : []) for (const { matcher, priority } of createMatchers(selector)) known.push({ matcher, rule, priority });
    known.sort((a, b) => a.priority - b.priority);
    injectionCache.set(lang, known);
  }
  return known;
}
let rootLang = '';
function scan(frame: Frame, line: string, pos: number, first: boolean, anchor: number): { rule: number; match: Match } | null {
  const found = scanRule(frame, line, pos, first, anchor);
  const injections = injectionsOf(rootLang);
  if (!injections.length) return found;
  let best: { rule: number; match: Match; priority: number } | null = null;
  for (const injection of injections) {
    if (!injection.matcher(frame.content)) continue;
    const injected = ruleOf(injection.rule);
    const list = injected.m !== undefined || injected.b !== undefined ? [injection.rule] : patterns(injection.rule);
    for (const id of list) {
      const target = ruleOf(id);
      const match = search(variant(target.m ?? target.b!, first, pos === anchor), line, pos);
      if (match && (!best || match.start < best.match.start)) best = { rule: id, match, priority: injection.priority };
      if (best && best.match.start === pos) break;
    }
    if (best && best.match.start === pos) break;
  }
  if (!best) return found;
  if (!found) return best;
  return best.match.start < found.match.start || best.priority === -1 && best.match.start === found.match.start ? best : found;
}

// ── Scopes and tokens ───────────────────────────────────────────────────────────────────────
function scopeName(id: number | undefined, line: string, match: Match): string[] {
  if (id === undefined) return [];
  const name = NAMES[id]!;
  const resolved = /\$\d+|\$\{\d+:\/(?:downcase|upcase)\}/.test(name) ? name.replace(/\$(\d+)|\$\{(\d+):\/(downcase|upcase)\}/g, (whole: string, plain: string, group: string, command: string) => {
    const at = match.indices[Number(plain ?? group)];
    if (!at) return whole;
    const text = line.slice(at[0], at[1]).replace(/^\.+/, '');
    return command === 'downcase' ? text.toLowerCase() : command === 'upcase' ? text.toUpperCase() : text;
  }) : name;
  return resolved.split(' ').filter(Boolean);
}
export interface Piece { text: string; scopes: string[] }
class Output {
  pieces: Piece[] = []; at = 0;
  constructor(readonly line: string, readonly limit: number) {}
  produce(scopes: string[], end: number) {
    const stop = Math.min(end, this.limit);
    if (stop <= this.at) return;
    this.pieces.push({ text: this.line.slice(this.at, stop), scopes });
    this.at = stop;
  }
}

function handleCaptures(line: string, first: boolean, frame: Frame, out: Output, captures: Captures | undefined, match: Match) {
  if (!captures) return;
  const local: { scopes: string[]; end: number }[] = [];
  const maxEnd = match.indices[0]![1];
  for (let group = 0; group < match.indices.length; group++) {
    const capture = captures[String(group)], at = match.indices[group];
    if (!capture || !at || at[1] === at[0]) continue;
    if (at[0] > maxEnd) break;
    while (local.length && local[local.length - 1]!.end <= at[0]) { const top = local.pop()!; out.produce(top.scopes, top.end); }
    out.produce(local.length ? local[local.length - 1]!.scopes : frame.content, at[0]);
    if (capture.r !== undefined) {
      const names = [...frame.content, ...scopeName(capture.n, line, match)];
      const content = [...names, ...scopeName(capture.c, line, match)];
      const inner: Frame = { rule: capture.r, parent: frame, enter: at[0], anchor: -1, eol: false, end: null, names, content };
      tokenizeString(line.slice(0, at[1]), first && at[0] === 0, at[0], inner, out);
      continue;
    }
    const names = scopeName(capture.n, line, match);
    if (names.length) local.push({ scopes: [...(local.length ? local[local.length - 1]!.scopes : frame.content), ...names], end: at[1] });
  }
  while (local.length) { const top = local.pop()!; out.produce(top.scopes, top.end); }
}

function checkWhile(line: string, first: boolean, frame: Frame, out: Output): { frame: Frame; pos: number; anchor: number; first: boolean } {
  let anchor = frame.eol ? 0 : -1, pos = 0;
  const whiles: Frame[] = [];
  for (let node: Frame | null = frame; node; node = node.parent) if (ruleOf(node.rule).w !== undefined) whiles.push(node);
  for (let node = whiles.pop(); node; node = whiles.pop()) {
    const rule = ruleOf(node.rule);
    const base = node.end ?? REGEXES[rule.w!]!;
    const match = exec(base.v ? base.v[(first ? 2 : 0) + (pos === anchor ? 1 : 0)]! : base, line, pos);
    if (!match) { frame = node.parent!; break; }
    out.produce(node.content, match.start);
    handleCaptures(line, first, node, out, rule.wc, match);
    out.produce(node.content, match.end);
    anchor = match.end;
    if (match.end > pos) { pos = match.end; first = false; }
  }
  return { frame, pos, anchor, first };
}

function hasSameRule(before: Frame, after: Frame): boolean {
  for (let node: Frame | null = before; node && node.enter === after.enter; node = node.parent) if (node.rule === after.rule) return true;
  return false;
}

/** Where a line's (or a capture's) tokenizing stands: vscode-textmate's _tokenizeString loop variables. */
interface Cursor { frame: Frame; pos: number; anchor: number; first: boolean }
/** One scan of vscode-textmate's _tokenizeString loop at the cursor: false when the line (or capture) is done. */
function scanStep(line: string, cursor: Cursor, out: Output): boolean {
  const length = line.length, pos = cursor.pos;
  let frame = cursor.frame;
  const found = scan(frame, line, pos, cursor.first, cursor.anchor);
  if (!found) { out.produce(frame.content, length); return false; }
  const { match } = found;
  const advanced = match.end > pos;
  if (found.rule === END) {
    const popped = frame, rule = ruleOf(popped.rule);
    out.produce(frame.content, match.start);
    const ending: Frame = { ...frame, content: frame.names };
    handleCaptures(line, cursor.first, ending, out, rule.ec, match);
    out.produce(ending.content, match.end);
    cursor.frame = frame = popped.parent!;
    cursor.anchor = popped.anchor;
    if (!advanced && popped.enter === pos) { cursor.frame = ending; out.produce(ending.content, length); return false; }
  } else {
    const rule = ruleOf(found.rule);
    out.produce(frame.content, match.start);
    const before = frame;
    const names = [...frame.content, ...scopeName(rule.n, line, match)];
    let pushed: Frame = { rule: found.rule, parent: frame, enter: pos, anchor: cursor.anchor, eol: match.end === length, end: null, names, content: names };
    if (rule.b !== undefined) {
      handleCaptures(line, cursor.first, pushed, out, rule.bc, match);
      out.produce(pushed.content, match.end);
      cursor.anchor = match.end;
      pushed = { ...pushed, content: [...names, ...scopeName(rule.c, line, match)] };
      const closing = rule.e ?? rule.w;
      if (closing !== undefined) {
        const rx = REGEXES[closing]!;
        pushed.end = rx.b ? resolveBackrefs(closing, line, match) : rx;
      }
      cursor.frame = pushed;
      if (!advanced && hasSameRule(before, pushed)) { cursor.frame = before; out.produce(before.content, length); return false; }
    } else {
      handleCaptures(line, cursor.first, pushed, out, rule.mc, match);
      out.produce(pushed.content, match.end);
      if (!advanced) { cursor.frame = before.parent ?? before; out.produce(cursor.frame.content, length); return false; }
    }
  }
  if (match.end > pos) { cursor.pos = match.end; cursor.first = false; }
  return true;
}
/** vscode-textmate's _tokenizeString with no time limit (a capture's retokenizing): the frame after it. */
function tokenizeString(line: string, first: boolean, pos: number, frame: Frame, out: Output): Frame {
  const cursor: Cursor = { frame, pos, anchor: -1, first };
  while (scanStep(line, cursor, out));
  return cursor.frame;
}

// ── Theme ───────────────────────────────────────────────────────────────────────────────────
// fontStyle as vscode-textmate keeps it: -1 not set, else bits (1 italic, 2 bold, 4 underline, 8 strikethrough).
interface TrieRule { depth: number; parents: string[] | null; fg: string; fs: number }
class TrieNode {
  children = new Map<string, TrieNode>();
  constructor(public main: TrieRule, public withParents: TrieRule[]) {}
  insert(depth: number, scope: string, parents: string[] | null, fg: string, fs: number) {
    if (scope === '') {
      if (parents === null) { this.main.depth = Math.max(this.main.depth, depth); if (fg) this.main.fg = fg; if (fs !== -1) this.main.fs = fs; return; }
      const same = this.withParents.find(rule => strArrCmp(rule.parents, parents) === 0);
      if (same) { same.depth = Math.max(same.depth, depth); if (fg) same.fg = fg; if (fs !== -1) same.fs = fs; return; }
      this.withParents.push({ depth, parents, fg: fg || this.main.fg, fs: fs !== -1 ? fs : this.main.fs });
      return;
    }
    const dot = scope.indexOf('.'), head = dot < 0 ? scope : scope.slice(0, dot), tail = dot < 0 ? '' : scope.slice(dot + 1);
    let child = this.children.get(head);
    if (!child) { child = new TrieNode({ ...this.main }, this.withParents.map(rule => ({ ...rule }))); this.children.set(head, child); }
    child.insert(depth + 1, tail, parents, fg, fs);
  }
  match(scope: string): TrieRule[] {
    if (scope !== '') {
      const dot = scope.indexOf('.'), head = dot < 0 ? scope : scope.slice(0, dot), tail = dot < 0 ? '' : scope.slice(dot + 1);
      const child = this.children.get(head);
      if (child) return child.match(tail);
    }
    return [...this.withParents, this.main].sort(bySpecificity);
  }
}
function strcmp(a: string, b: string) { return a < b ? -1 : a > b ? 1 : 0; }
function strArrCmp(a: string[] | null, b: string[] | null): number {
  if (a === null && b === null) return 0;
  if (!a) return -1;
  if (!b) return 1;
  if (a.length !== b.length) return a.length - b.length;
  for (let index = 0; index < a.length; index++) { const result = strcmp(a[index]!, b[index]!); if (result) return result; }
  return 0;
}
function bySpecificity(a: TrieRule, b: TrieRule): number {
  if (a.depth !== b.depth) return b.depth - a.depth;
  const pa = a.parents ?? [], pb = b.parents ?? [];
  let ia = 0, ib = 0;
  for (;;) {
    if (pa[ia] === '>') ia++;
    if (pb[ib] === '>') ib++;
    if (ia >= pa.length || ib >= pb.length) break;
    const diff = pb[ib]!.length - pa[ia]!.length;
    if (diff) return diff;
    ia++; ib++;
  }
  return pb.length - pa.length;
}
const matchesScope = (scope: string, pattern: string) => scope === pattern || scope.startsWith(pattern) && scope[pattern.length] === '.';
function parentsMatch(path: string[], parents: string[] | null): boolean {
  if (!parents || !parents.length) return true;
  let at = path.length - 2;
  for (let index = 0; index < parents.length; index++) {
    let pattern = parents[index]!, must = false;
    if (pattern === '>') { if (index === parents.length - 1) return false; pattern = parents[++index]!; must = true; }
    while (at >= 0) { if (matchesScope(path[at]!, pattern)) break; if (must) return false; at--; }
    if (at < 0) return false;
    at--;
  }
  return true;
}
/** vscode-textmate's parseTheme: a fontStyle string as bits ('' and 'normal' are 0). */
function fontStyleBits(style: string | undefined): number {
  if (style === undefined) return -1;
  let bits = 0;
  for (const word of style.split(' ')) bits |= word === 'italic' ? 1 : word === 'bold' ? 2 : word === 'underline' ? 4 : word === 'strikethrough' ? 8 : 0;
  return bits;
}
export interface Style { fg: string; italic: boolean }
interface Theme { root: TrieNode; fg: string; cache: Map<string, Style> }
const themes = new Map<string, Theme>();
function theme(name: 'light' | 'dark'): Theme {
  let built = themes.get(name);
  if (built) return built;
  const source = index().themes[name];
  const parsed = source.rules.map(([selector, fg, style], index) => {
    const segments = selector.split(' ').filter(Boolean);
    const scope = segments[segments.length - 1] ?? '';
    const parents = segments.length > 1 ? segments.slice(0, -1).reverse() : null;
    return { scope, parents, fg, fs: fontStyleBits(style), index };
  }).sort((a, b) => strcmp(a.scope, b.scope) || strArrCmp(a.parents, b.parents) || a.index - b.index);
  const root = new TrieNode({ depth: 0, parents: null, fg: '', fs: -1 }, []);
  for (const rule of parsed) root.insert(0, rule.scope, rule.parents, rule.fg, rule.fs);
  built = { root, fg: source.fg, cache: new Map() };
  themes.set(name, built);
  return built;
}
/** The foreground and font style a scope path takes: each pushed scope's best rule overrides what it sets. */
export function styleOf(scopes: string[], name: 'light' | 'dark'): Style {
  const t = theme(name), key = scopes.join(' ');
  const known = t.cache.get(key);
  if (known) return known;
  let fg = t.fg, fs = 0;
  for (let depth = 0; depth < scopes.length; depth++) {
    const path = scopes.slice(0, depth + 1);
    const rule = t.root.match(scopes[depth]!).find(candidate => parentsMatch(path, candidate.parents));
    if (rule && rule.fg) fg = rule.fg;
    if (rule && rule.fs !== -1) fs = rule.fs;
  }
  const style = { fg, italic: (fs & 1) === 1 };
  t.cache.set(key, style);
  return style;
}
/** The foreground a scope path takes. */
export const colorOf = (scopes: string[], name: 'light' | 'dark'): string => styleOf(scopes, name).fg;

/** The tokenizer's state between lines (opaque); null before the first line. */
export type LineState = Frame | null;
/** Whether a line has a character outside ASCII (its regexes then run in their Unicode \b forms). */
export const unicodeText = (text: string): boolean => /[^\x00-\x7f]/.test(text);
/** A line being tokenized: its text with "\n", where its scan stands, its output so far, the scan work it
 *  has done; opaque to callers, who hand it back to tokenizeLineWithin to go on. */
export interface LineProgress { readonly text: string; readonly lang: string; readonly unicode: boolean; readonly cursor: Cursor; readonly out: Output; readonly memo: Memo; units: number; stopped: boolean }
/** A tokenized line: its scoped pieces, the state after it, and whether it stopped at the time limit (the rest plain). */
export interface LineResult { pieces: Piece[]; state: Frame; stopped: boolean }
function startLine(text: string, lang: string, state: LineState): LineProgress {
  const first = state === null;
  rootLang = lang;
  unicodeLine = unicodeText(text);
  let frame: Frame;
  if (state === null) {
    const root = index().roots[lang]!, top = ruleOf(root);
    const names = scopeName(top.n, '', { start: 0, end: 0, indices: [] });
    frame = { rule: root, parent: null, enter: -1, anchor: -1, eol: false, end: null, names, content: names };
  } else {
    frame = state;
    // vscode-textmate resets every frame's enter and anchor positions before each line after the first.
    for (let node: Frame | null = frame; node; node = node.parent) { node.enter = -1; node.anchor = -1; }
  }
  const line = text + '\n';
  const out = new Output(line, text.length);
  // vscode-textmate's clock starts after the line's while checks.
  return { text: line, lang, unicode: unicodeLine, cursor: checkWhile(line, first, frame, out), out, memo: new Map(), units: 0, stopped: false };
}
/** Runs the line's scans: true when it is done (to its end, or stopped at LINE_SCAN_LIMIT with the rest plain), false when
 *  the turn's regex work ran out (OVER_TURN): the progress is then as it was before the scan it was in, which goes on next
 *  turn. A compile over the work limit (OVER_LIMIT) leaves it the same way and is thrown on. */
function runLine(progress: LineProgress): boolean {
  rootLang = progress.lang;
  unicodeLine = progress.unicode;
  const { cursor, out, text } = progress, base = scanned;
  for (;;) {
    if (progress.units + scanned - base > LINE_SCAN_LIMIT) {
      progress.units += scanned - base;
      progress.stopped = true;
      out.produce([], text.length);
      return true;
    }
    const saved = { ...cursor }, count = out.pieces.length, at = out.at;
    memoLine = text;
    memo = progress.memo;
    let more: boolean;
    try { more = scanStep(text, cursor, out); } catch (error) {
      Object.assign(cursor, saved); out.pieces.length = count; out.at = at;
      progress.units += scanned - base;
      if (error === OVER_TURN) return false;
      throw error;
    }
    if (!more) { progress.units += scanned - base; return true; }
  }
}
const resultOf = (progress: LineProgress): LineResult =>
  ({ pieces: progress.out.pieces, state: progress.cursor.frame, stopped: progress.stopped });

/** One line (without its "\n") after `state`: its scoped pieces, the state after it, and whether it stopped at Shiki's time
 *  limit (LINE_SCAN_LIMIT): then the pieces before the stop keep their scopes and the rest of the line is plain. */
export function tokenizeLine(text: string, lang: string, state: LineState): LineResult {
  const progress = startLine(text, lang, state);
  runLine(progress);
  return resultOf(progress);
}

/** tokenizeLine within a turn. `limit` bounds the line's one-time work (regexes compiled on first
 *  use): a compile that would pass it stops the line, the regexes compiled so far stay compiled. `scanCap`
 *  bounds the turn's regex work (turnWork), a search allowed past it only as the turn's first (`fresh`). Either stop answers
 *  the line's progress, which a later call continues from (`progress`; `state` is then unused), or null when the line stopped
 *  before its first scan (a while check): it starts again from `state` (a line only reads its start state). So a line that
 *  first meets a big list (asm's 280 Unicode forms, about 430 ms) or a template-dense cpp line spreads over turns. */
export function tokenizeLineWithin(text: string, lang: string, state: LineState, limit: number, progress: LineProgress | null = null, scanCap = Infinity, fresh = true): LineResult | { progress: LineProgress } | null {
  lineWork = work; lineCompiled = 0; workLimit = limit;
  turnSpent = 0; turnCap = scanCap; turnSearched = !fresh;
  let current = progress;
  try {
    current ??= startLine(text, lang, state);
    return runLine(current) ? resultOf(current) : { progress: current };
  } catch (error) {
    if (error === OVER_LIMIT || error === OVER_TURN) return current ? { progress: current } : null;
    throw error;
  } finally { workLimit = Infinity; turnCap = Infinity; }
}

/** The scoped pieces of `code` in a language this table holds (one piece per run, "\n" included). */
export function tokenizeCode(code: string, name: string): Piece[] {
  const lang = grammarId(name);
  if (!lang) return [{ text: code, scopes: [] }];
  const pieces: Piece[] = [];
  let state: LineState = null;
  const lines = code.split(/\r\n|\r|\n/);
  lines.forEach((text, index) => {
    const result = tokenizeLine(text, lang, state);
    state = result.state;
    for (const piece of result.pieces) pieces.push(piece);
    if (index < lines.length - 1) pieces.push({ text: '\n', scopes: [] });
  });
  return pieces;
}
