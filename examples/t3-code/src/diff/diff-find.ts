// @pierre/diffs CodeViewSearch (T3 Code patch at 9c161462c9, MIT, LICENSE-T3).
import type { TextRange } from '../timeline/thread-find';
export type DiffFindOptions = { caseSensitive: boolean; wholeWord: boolean; regex: boolean };
const WORD_SEPARATORS = "`~!@#$%^&*()-=+[{]}\\|;:'\",.<>/?";
const separator = (text: string, index: number) => index < 0 || index >= text.length || text.charCodeAt(index) <= 32 || text.charCodeAt(index) === 127 || WORD_SEPARATORS.includes(text[index]!);
export function diffFindRanges(text: string, query: string, options: DiffFindOptions): TextRange[] {
  if (!query) return [];
  let pattern: RegExp;
  try { pattern = new RegExp(options.regex ? query : query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), options.caseSensitive ? 'g' : 'gi'); }
  catch { return []; }
  const ranges: TextRange[] = [];
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null && ranges.length < 100_000) {
    const start = match.index, end = start + match[0].length;
    if (start === end) { pattern.lastIndex++; continue; }
    if (!options.wholeWord || separator(text, start - 1) && separator(text, end)) ranges.push({ start, end });
  }
  return ranges;
}
export type FindPart = { id: string; text: string; find: number };
/** Split syntax runs at search boundaries while preserving every other run field. */
export function markFindSegments<T extends { id: string; text: string }>(segments: readonly T[], ranges: readonly TextRange[], active: TextRange | null): (T & { find: number })[] {
  let at = 0;
  return segments.flatMap(segment => {
    const start = at; at += segment.text.length;
    const cuts = [...new Set([start, at, ...ranges.flatMap(range => [Math.max(start, Math.min(at, range.start)), Math.max(start, Math.min(at, range.end))])])].sort((a, b) => a - b);
    return cuts.slice(1).map((end, index) => {
      const begin = cuts[index]!, marked = ranges.some(range => begin >= range.start && end <= range.end);
      return { ...segment, id: `${segment.id}:${index}`, text: segment.text.slice(begin - start, end - start), find: marked ? active && begin >= active.start && end <= active.end ? 2 : 1 : 0 };
    });
  });
}

export type DiffFindMatch = TextRange & { path: string; side: 'additions' | 'deletions'; line: number; oldLine: number; context: boolean };
export type DiffFindLine = { change: 'context' | 'add' | 'delete'; oldLineNumber: number | null; newLineNumber: number | null; content: string };
export class DiffFind {
  scope = ''; open = false; query = ''; caseSensitive = false; wholeWord = false; regex = false; focus = 0; navigation = 0; index = -1;
  matches: DiffFindMatch[] = [];
  reset(scope: string): void { if (scope !== this.scope) { this.scope = scope; this.matches = []; this.index = -1; this.query = ''; this.open = false; } }
  action(op: string, value: string): void {
    if (op === 'open') { this.open = true; this.focus++; }
    else if (op === 'close') { this.open = false; this.query = ''; this.matches = []; this.index = -1; }
    else if (op === 'query') { this.query = value; this.index = -1; }
    else if (op === 'case') this.caseSensitive = !this.caseSensitive;
    else if (op === 'word') this.wholeWord = !this.wholeWord;
    else if (op === 'regex') this.regex = !this.regex;
    else if (this.matches.length && (op === 'next' || op === 'previous')) this.index = ((this.index + (op === 'next' ? 1 : -1)) % this.matches.length + this.matches.length) % this.matches.length;
    this.navigation++;
  }
  search(files: readonly { path: string; lines: readonly DiffFindLine[] }[]): void {
    const previous = this.matches[this.index];
    this.matches = [];
    if (this.open) outer: for (const file of files) for (const line of file.lines) {
      for (const range of diffFindRanges(line.content, this.query, this)) {
        this.matches.push({ ...range, path: file.path, side: line.change === 'delete' ? 'deletions' : 'additions',
          line: (line.change === 'delete' ? line.oldLineNumber : line.newLineNumber) ?? 0, oldLine: line.oldLineNumber ?? 0, context: line.change === 'context' });
        if (this.matches.length >= 100_000) break outer;
      }
    }
    const same = previous ? this.matches.findIndex(match => match.path === previous.path && match.side === previous.side && match.line === previous.line && match.start === previous.start) : -1;
    this.index = same >= 0 ? same : Math.min(Math.max(this.index, 0), this.matches.length - 1);
  }
  get active(): DiffFindMatch | null { return this.matches[this.index] ?? null; }
  view(target = '') { return { open: this.open, query: this.query, focus: this.focus, navigation: this.navigation, count: `${this.matches.length ? this.index + 1 : 0}/${this.matches.length}`, target,
    caseSensitive: this.caseSensitive, wholeWord: this.wholeWord, regex: this.regex, empty: this.matches.length === 0 }; }
  mark<T extends { id: string; text: string }>(segments: readonly T[], path: string, side: string, line: number): (T & { find: number })[] {
    const matches = this.matches.filter(match => match.path === path && (match.context && side === 'deletions' ? match.oldLine === line : match.line === line) && (match.side === side || match.context));
    const active = this.active;
    return markFindSegments(segments, matches, active && matches.includes(active) ? active : null);
  }
}

type SearchRow = { id: string; path: string; kind: string; line: number; side: string; leftLine: number; rightLine: number;
  segments: { id: string; text: string }[]; leftSegments: { id: string; text: string }[]; rightSegments: { id: string; text: string }[] };
export function markDiffRows<T extends SearchRow>(items: readonly T[], find: DiffFind): { items: T[]; target: string } {
  const active = find.active;
  const target = active ? items.find(item => item.path === active.path && (item.kind === 'line' && item.line === active.line && (item.side === active.side || active.context) || item.kind === 'split' && (active.side === 'deletions' ? item.leftLine : item.rightLine) === active.line))?.id ?? '' : '';
  return { items: items.map(item => ({ ...item, segments: find.mark(item.segments, item.path, item.side, item.line),
    leftSegments: find.mark(item.leftSegments, item.path, 'deletions', item.leftLine), rightSegments: find.mark(item.rightSegments, item.path, 'additions', item.rightLine) })), target };
}
const focused = new WeakMap<object, { slot: 'panel' | 'pr'; reference: string }>();
export const focusDiffFind = (client: object, slot: 'panel' | 'pr' | null, reference = ''): void => { if (slot) focused.set(client, { slot, reference }); else focused.delete(client); };
export const focusedDiffFind = (client: object) => focused.get(client) ?? null;

/** Reveal a hydrated unchanged line between hunks before asking the list to materialize it. */
export function findGap(file: { hunks: readonly { newStart: number; lines: readonly { kind: string }[] }[] }, line: number): { gap: number; through: number } | null {
  let next = 1;
  for (const [gap, hunk] of file.hunks.entries()) {
    const count = hunk.lines.filter(line => line.kind !== 'deletion').length, start = hunk.newStart + (count === 0 ? 1 : 0);
    if (line >= next && line < start) return { gap, through: line - next + 1 };
    next = start + count;
  }
  return line >= next ? { gap: file.hunks.length, through: line - next + 1 } : null;
}
