// A code block draws one text per token, each with its offset for find and citation marks (markdown.contract
// CodeBlock), and a token's find parts by theirs (thread-find.contract FindToken). The runner bounds the list steps one
// evaluation takes (runner/src/vm.rs MAX_LIST_STEPS, LLP 1017.003 D3): map, filter and join take a step an item, indexOf
// one an item up to the one it finds, slice one an item it keeps. Summing the tokens before each one in Contract took
// about 5N steps for the last of N, so a coloured 1,500-line block of about 26,000 tokens trapped (IterationLimit) and
// poisoned the runner; FindToken summed the find parts before each part twice, past the bound at about 6,550 parts.
// Both offsets are counted once by their producers (CodeToken `at` in TypeScript, FindPart `at` in
// data/src/markdown_find.rs), and a token is handed only its own lines' find parts. Counted counts the steps these
// expressions take.
import { expect, test } from 'bun:test';
import { contractComponent, contractFns } from '../support/contract-action-fixture';
import { codeTokens } from '../../src/highlight/timeline-highlight';
import { tokensOf } from '../../src/highlight/highlights';
import { literalFindRanges } from '../../src/timeline/thread-find';
import { lineRecords } from '../support/highlights-fixture';

const MAX_LIST_STEPS = 65_536;
let steps = 0;
/** A list whose walks count the steps the runner charges for them (runner/src/vm/list_call.rs). */
class Counted<T> extends Array<T> {
  override map<U>(f: (value: T, index: number, array: T[]) => U): U[] { steps += this.length; return super.map(f); }
  override filter(f: (value: T, index: number, array: T[]) => unknown): T[] { steps += this.length; return super.filter(f); }
  override indexOf(item: T, from?: number): number { const found = super.indexOf(item, from); steps += found < 0 ? this.length : found + 1; return found; }
  override slice(start?: number, end?: number): T[] { const kept = super.slice(start, end); steps += kept.length; return kept; }
  override join(separator?: string): string { steps += this.length; return super.join(separator); }
}
const sum = (xs: number[]) => { steps += xs.length; return xs.reduce((total, x) => total + x, 0); };

test('each token carries where it starts in its code, and the lines it starts and ends on', () => {
  expect(codeTokens([{ text: 'const ', cls: 'kw' }, { text: 'a', cls: 'var' }, { text: ' = 1;\n', cls: '' }, { text: '𝑥', cls: 'var' }, { text: '\n\nb', cls: '' }])
    .map(token => [token.id, token.at, token.line, token.lastLine]))
    .toEqual([['0', 0, 0, 0], ['1', 6, 0, 0], ['2', 7, 0, 0], ['3', 13, 1, 1], ['4', 15, 1, 3]]);
});

test('a token of a block of 26,000 takes its offset within the runner\'s list steps', () => {
  const block = { id: 'b0', kind: 'code', text: 'x'.repeat(26_000), href: 'ts', findParts: Counted.from([{ id: '0', text: 'x'.repeat(26_000), find: 0, at: 0 }]), findLines: Counted.from<number>([]) };
  const tokens = Counted.from(codeTokens(Array.from({ length: 26_000 }, () => ({ text: 'x', cls: 'var' }))));
  const hl = { id: '0', code: block.text, icon: 'file-ts', tokens };
  const fns = { ...contractFns('src/timeline/markdown.contract', { max: Math.max, min: Math.min }), ...contractFns('src/timeline/thread-find.contract', { sum }) };
  const codeBlock = contractComponent('src/timeline/markdown.contract', 'CodeBlock');
  const scope = { ...fns, max: Math.max, min: Math.min, sum, block, code: [hl], hl, token: tokens[tokens.length - 1], wrapDefault: true, ms: -1, me: -1 };
  steps = 0;
  const args = codeBlock.callArguments('FindToken(value=token.text', { flipped: false }, scope) as { at: number };
  expect(args.at).toBe(25_999);
  expect(steps).toBeLessThanOrEqual(MAX_LIST_STEPS);
});

test('a code token places its find parts within the runner\'s list steps', () => {
  // A query matched 3,300 times in one block: 6,600 parts, a match and the text after it each.
  let at = 0;
  const parts = Counted.from(Array.from({ length: 6_600 }, (_, i) => {
    const part = { id: String(i), text: i % 2 ? ' = ' : 'needle', find: i % 2 ? 0 : 1, at };
    at += part.text.length;
    return part;
  }));
  const last = parts[parts.length - 2]!, findToken = contractComponent('src/timeline/thread-find.contract', 'FindToken');
  const scope = { ...contractFns('src/timeline/thread-find.contract', { max: Math.max, min: Math.min }), max: Math.max, min: Math.min, parts, part: last, value: 'needle', at: last.at };
  steps = 0;
  expect(findToken.drawn('text value.slice(', {}, scope)).toBe(true);
  expect(findToken.attribute('text value.slice(', 'id', {}, scope)).toBe('');
  expect(steps).toBeLessThanOrEqual(MAX_LIST_STEPS);
});

test('a token of a 26,000-token block a query matches on every line walks its own lines\' find parts', () => {
  // The stand-in's 1,500-line block, lexed as the app lexes it, and "stream" found once on each line: 3,001 parts.
  const text = Array.from({ length: 1_500 }, (_, i) => `export function stream${i}(value: number): number { return value * ${i} + base; } // ${i}`).join('\n');
  const tokens = Counted.from(codeTokens(tokensOf(text, lineRecords(72, text))));
  expect(tokens.length).toBeGreaterThan(26_000);
  // markdown_find.rs find_parts and find_lines: the parts between the matches' ends, each line's part, then the last.
  const ranges = literalFindRanges(text, 'stream');
  const cuts = [...new Set([0, text.length, ...ranges.flatMap(range => [range.start, range.end])])].sort((a, b) => a - b);
  const parts = Counted.from(cuts.slice(1).map((end, i) => ({ id: String(i), text: text.slice(cuts[i], end), find: ranges.some(range => cuts[i]! >= range.start && end <= range.end) ? 1 : 0, at: cuts[i]! })));
  const lines: number[] = [];
  for (let at = 0, part = 0; at <= text.length; at = text.indexOf('\n', at) + 1 || text.length + 1) {
    while (part + 1 < parts.length && parts[part + 1]!.at <= at) part++;
    lines.push(part);
  }
  lines.push(parts.length - 1);
  expect(parts.length).toBe(3_001);
  const block = { id: 'b0', kind: 'code', text, href: 'ts', findParts: parts, findLines: Counted.from(lines) };
  const hl = { id: '0', code: text, icon: 'file-ts', tokens };
  const fns = { ...contractFns('src/timeline/markdown.contract', { max: Math.max, min: Math.min }), ...contractFns('src/timeline/thread-find.contract', { sum }) };
  const codeBlock = contractComponent('src/timeline/markdown.contract', 'CodeBlock');
  const findToken = contractComponent('src/timeline/thread-find.contract', 'FindToken');
  let most = 0, handed = 0;
  for (const token of [tokens[0]!, tokens[Math.floor(tokens.length / 2)]!, tokens[tokens.length - 1]!]) {
    const scope = { ...fns, max: Math.max, min: Math.min, sum, block, code: [hl], hl, token, wrapDefault: true, ms: -1, me: -1 };
    steps = 0;
    const args = codeBlock.callArguments('FindToken(value=token.text', { flipped: false }, scope) as { value: string; at: number; parts: typeof parts };
    most = Math.max(most, steps);
    handed = Math.max(handed, args.parts.length);
    // The parts it is handed draw the whole token.
    const drawn = args.parts.filter(part => findToken.drawn('text value.slice(', {}, { ...args, part, max: Math.max, min: Math.min }))
      .map(part => args.value.slice(Math.max(0, part.at - args.at), Math.min(args.value.length, part.at + part.text.length - args.at))).join('');
    expect(drawn).toBe(token.text);
  }
  // Two one-item slices summed and the token's parts: 3 parts and 7 steps at most, where every token walked all 3,001.
  expect(handed).toBeLessThanOrEqual(3);
  expect(most).toBeLessThanOrEqual(7);
});
