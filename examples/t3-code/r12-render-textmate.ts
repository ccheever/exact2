// Lane r12-render: the reference colours code with Shiki 4.2 (TextMate grammars run by
// vscode-textmate, the pierre-light / pierre-dark themes). This is that tokenizer, as small as it
// can be: the grammars and themes are data (r12-render-grammar.ts, generated from the reference's
// own packages, regexes converted ahead of time as Shiki's JavaScript engine converts them), and
// this file runs them with vscode-textmate's rules (MIT; see LICENSE-T3): one line at a time with
// the line's "\n", the earliest match wins (ties to the first pattern), begin/end and begin/while
// rules stack, \A / \G variants, end back-references, nested captures, the loop guards that stop
// a line when a rule does not advance, and the theme trie's specificity for colours and font styles.
// shiki-residuals: the grammars come in groups (one generated file each, tools/grammar/gen-grammar.mjs),
// each with its own rule ids; a group's data is parsed the first time one of its languages is used.

import { GRAMMAR_DATA, GRAMMAR_LANGUAGES } from './r12-render-grammar';
import { GRAMMAR_DATA as MORE_DATA, GRAMMAR_LANGUAGES as MORE_LANGUAGES } from './r12-render-grammar-more';

/** A converted regex: source, flags, hidden captures, capture transfers, clip-search strategy
 *  (oniguruma-to-es's EmulatedRegExp options), a back-reference marker, or four \A/\G variants. */
interface Rx { s: string; f: string; h?: number[]; t?: [number, number[]][]; c?: 1; b?: 1; v?: Rx[]; re?: RegExp }
interface Capture { n?: number; c?: number; r?: number }
type Captures = Record<string, Capture>;
interface Rule { i?: number; n?: number; c?: number; m?: number; mc?: Captures; b?: number; bc?: Captures; e?: number; ec?: Captures; w?: number; wc?: Captures; l?: 1; p?: number[] }
/** A theme rule: selector, foreground ('' for none) and, when the rule sets one, its fontStyle. */
interface ThemeData { fg: string; rules: ([string, string] | [string, string, string])[] }
interface Data {
  roots: Record<string, number>; aliases: Record<string, string>; injections: Record<string, [string, number][]>;
  names: string[]; rules: Rule[]; regexes: Rx[]; themes?: { light: ThemeData; dark: ThemeData };
}
interface Group { source: string; languages: Set<string>; parsed: Data | null; patterns: Map<number, number[]>; injections: Map<string, Injection[]> }
const GROUPS: Group[] = [[GRAMMAR_DATA, GRAMMAR_LANGUAGES], [MORE_DATA, MORE_LANGUAGES]].map(([source, languages]) =>
  ({ source: source!, languages: new Set(languages!.split(' ')), parsed: null, patterns: new Map(), injections: new Map() }));
const parse = (group: Group): Data => group.parsed ??= JSON.parse(group.source) as Data;
/** The group the line being tokenized belongs to (set by tokenizeLine). */
let current: Group = GROUPS[0]!;
const data = (): Data => parse(current);
const groupOf = (lang: string): Group | undefined => GROUPS.find(group => group.languages.has(lang));

/** The Shiki language id a grammar here has for a language name or alias, or ''. */
export function grammarId(lang: string): string {
  const group = groupOf(lang);
  if (!group) return '';
  const table = parse(group), id = table.aliases[lang] ?? lang;
  return table.roots[id] !== undefined ? id : '';
}

// ── Regexes ─────────────────────────────────────────────────────────────────────────────────
interface Match { start: number; end: number; indices: ([number, number] | undefined)[] }
/** The entry's RegExp, compiled once and kept on the entry (back-reference variants are fresh entries). */
const compile = (rx: Rx): RegExp => rx.re ??= new RegExp(rx.s, rx.f + 'dg');
function variant(index: number, allowA: boolean, allowG: boolean): Rx {
  const rx = data().regexes[index]!;
  return rx.v ? rx.v[(allowA ? 2 : 0) + (allowG ? 1 : 0)]! : rx;
}
function exec(rx: Rx, line: string, from: number): Match | null {
  const re = compile(rx);
  // clip_search: the regex sees the line from `from` on (its \G became ^), offsets added back.
  const offset = rx.c && from ? from : 0, text = offset ? line.slice(offset) : line;
  re.lastIndex = from - offset;
  const found = re.exec(text) as (RegExpExecArray & { indices?: ([number, number] | undefined)[] }) | null;
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
/** An end / while regex with its back-references (U+E000+n, see the generator) resolved from the begin match. */
function resolveBackrefs(rx: Rx, line: string, begin: Match): Rx {
  const swap = (one: Rx): Rx => ({ s: one.s.replace(/[\ue000-\ue0ff]/g, mark => {
    const at = begin.indices[mark.charCodeAt(0) - 0xe000];
    return at ? escapeRx(line.slice(at[0], at[1])) : '';
  }), f: one.f, ...(one.h ? { h: one.h } : {}), ...(one.t ? { t: one.t } : {}), ...(one.c ? { c: one.c } : {}) });
  return rx.v ? { s: '', f: '', v: rx.v.map(swap) } : swap(rx);
}

// ── Rules and scanners ──────────────────────────────────────────────────────────────────────
/** The match / begin rules a rule's patterns reach, includes and pattern-only rules expanded. */
function patterns(id: number): number[] {
  const patternCache = current.patterns;
  const known = patternCache.get(id);
  if (known) return known;
  const out: number[] = [], seen = new Set<number>();
  const visit = (ruleId: number) => {
    if (ruleId < 0 || seen.has(ruleId)) return;
    const rule = data().rules[ruleId]!;
    if (rule.i !== undefined) { visit(rule.i); return; }
    if (rule.m !== undefined || rule.b !== undefined) { out.push(ruleId); return; }
    seen.add(ruleId);
    for (const child of rule.p ?? []) visit(child);
  };
  for (const child of data().rules[id]!.p ?? []) visit(child);
  patternCache.set(id, out);
  return out;
}

interface Frame {
  rule: number; parent: Frame | null; enter: number; anchor: number; eol: boolean;
  end: Rx | null; names: string[]; content: string[];
}
const END = -1;
/** A regex's last search on the current line: the leftmost match from `from` stays the answer for
 *  any later start up to that match (none in between matched), unless the regex is sticky. */
let memoLine = '';
const memo = new Map<Rx, { from: number; match: Match | null }>();
function search(rx: Rx, line: string, pos: number): Match | null {
  if (line !== memoLine) { memo.clear(); memoLine = line; }
  const known = memo.get(rx);
  if (known && known.from <= pos && !rx.c && !rx.f.includes('y') && (known.match === null || known.match.start >= pos)) return known.match;
  const match = exec(rx, line, pos);
  memo.set(rx, { from: pos, match });
  return match;
}
function scanRule(frame: Frame, line: string, pos: number, first: boolean, anchor: number): { rule: number; match: Match } | null {
  const rule = data().rules[frame.rule]!;
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
    const target = data().rules[id]!;
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
function injectionsOf(lang: string): Injection[] {
  const injectionCache = current.injections;
  let known = injectionCache.get(lang);
  if (!known) {
    known = [];
    for (const [selector, rule] of data().injections[lang] ?? []) for (const { matcher, priority } of createMatchers(selector)) known.push({ matcher, rule, priority });
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
    const own = data().rules[injection.rule]!;
    const list = own.m !== undefined || own.b !== undefined ? [injection.rule] : patterns(injection.rule);
    for (const id of list) {
      const target = data().rules[id]!;
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
  const name = data().names[id]!;
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
      tokenizeString(line.slice(0, at[1]), first && at[0] === 0, at[0], inner, out, false);
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
  for (let node: Frame | null = frame; node; node = node.parent) if (data().rules[node.rule]!.w !== undefined) whiles.push(node);
  for (let node = whiles.pop(); node; node = whiles.pop()) {
    const rule = data().rules[node.rule]!;
    const base = node.end ?? data().regexes[rule.w!]!;
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

/** vscode-textmate's _tokenizeString: the frame after the line (or capture) is tokenized. */
function tokenizeString(line: string, first: boolean, pos: number, frame: Frame, out: Output, lineStart: boolean): Frame {
  const length = line.length;
  let anchor = -1;
  if (lineStart) ({ frame, pos, anchor, first } = checkWhile(line, first, frame, out));
  for (;;) {
    const found = scan(frame, line, pos, first, anchor);
    if (!found) { out.produce(frame.content, length); return frame; }
    const { match } = found;
    const advanced = match.end > pos;
    if (found.rule === END) {
      const popped = frame, rule = data().rules[popped.rule]!;
      out.produce(frame.content, match.start);
      const ending: Frame = { ...frame, content: frame.names };
      handleCaptures(line, first, ending, out, rule.ec, match);
      out.produce(ending.content, match.end);
      frame = popped.parent!;
      anchor = popped.anchor;
      if (!advanced && popped.enter === pos) { frame = ending; out.produce(frame.content, length); return frame; }
    } else {
      const rule = data().rules[found.rule]!;
      out.produce(frame.content, match.start);
      const before = frame;
      const names = [...frame.content, ...scopeName(rule.n, line, match)];
      let pushed: Frame = { rule: found.rule, parent: frame, enter: pos, anchor, eol: match.end === length, end: null, names, content: names };
      if (rule.b !== undefined) {
        handleCaptures(line, first, pushed, out, rule.bc, match);
        out.produce(pushed.content, match.end);
        anchor = match.end;
        pushed = { ...pushed, content: [...names, ...scopeName(rule.c, line, match)] };
        const closing = rule.e ?? rule.w;
        if (closing !== undefined) {
          const rx = data().regexes[closing]!;
          pushed.end = rx.b ? resolveBackrefs(rx, line, match) : rx;
        }
        frame = pushed;
        if (!advanced && hasSameRule(before, pushed)) { frame = before; out.produce(frame.content, length); return frame; }
      } else {
        handleCaptures(line, first, pushed, out, rule.mc, match);
        out.produce(pushed.content, match.end);
        if (!advanced) { frame = before.parent ?? before; out.produce(frame.content, length); return frame; }
      }
    }
    if (match.end > pos) { pos = match.end; first = false; }
  }
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
  const source = parse(GROUPS[0]!).themes![name];
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
/** One line (without its "\n") after `state`: its scoped pieces and the state after it. */
export function tokenizeLine(text: string, lang: string, state: LineState): { pieces: Piece[]; state: Frame } {
  const first = state === null;
  rootLang = lang;
  current = groupOf(lang) ?? GROUPS[0]!;
  let frame: Frame;
  if (state === null) {
    const root = data().roots[lang]!, top = data().rules[root]!;
    const names = scopeName(top.n, '', { start: 0, end: 0, indices: [] });
    frame = { rule: root, parent: null, enter: -1, anchor: -1, eol: false, end: null, names, content: names };
  } else {
    frame = state;
    // vscode-textmate resets every frame's enter and anchor positions before each line after the first.
    for (let node: Frame | null = frame; node; node = node.parent) { node.enter = -1; node.anchor = -1; }
  }
  const line = text + '\n';
  const out = new Output(line, text.length);
  frame = tokenizeString(line, first, 0, frame, out, true);
  return { pieces: out.pieces, state: frame };
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
