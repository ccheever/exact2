// A code block draws one text per token, each with its offset for find and citation marks (markdown.contract
// CodeBlock), and a token's find parts by theirs (thread-find.contract FindToken). The runner bounds the list steps one
// evaluation takes (runner/src/vm.rs MAX_LIST_STEPS, LLP 1017.003 D3): map, filter, indexOf, slice and join each take
// a step an item they walk. Summing the tokens before each one in Contract took about 5N steps for the last of N, so a
// coloured 1,500-line block of about 26,000 tokens trapped (IterationLimit) and poisoned the runner; FindToken summed
// the find parts before each part twice, past the bound at about 6,550 parts. Both offsets are counted once by their
// producers (CodeToken `at` in TypeScript, FindPart `at` in data/src/markdown_find.rs). Counted here counts the walks
// these expressions take.
import { expect, test } from 'bun:test';
import { contractComponent, contractFns } from '../support/contract-action-fixture';
import { codeTokens } from '../../src/highlight/timeline-highlight';

const MAX_LIST_STEPS = 65_536;
let steps = 0;
/** A list whose map, filter and join count the steps the runner charges for them. */
class Counted<T> extends Array<T> {
  override map<U>(f: (value: T, index: number, array: T[]) => U): U[] { steps += this.length; return super.map(f); }
  override filter(f: (value: T, index: number, array: T[]) => unknown): T[] { steps += this.length; return super.filter(f); }
  override indexOf(item: T, from?: number): number { steps += this.length; return super.indexOf(item, from); }
  override slice(start?: number, end?: number): T[] { steps += this.length; return super.slice(start, end); }
  override join(separator?: string): string { steps += this.length; return super.join(separator); }
}

test('each token carries where it starts in its code', () => {
  expect(codeTokens([{ text: 'const ', cls: 'kw' }, { text: 'a', cls: 'var' }, { text: ' = 1;\n', cls: '' }, { text: '𝑥', cls: 'var' }]).map(token => [token.id, token.at]))
    .toEqual([['0', 0], ['1', 6], ['2', 7], ['3', 13]]);
});

test('a token of a block of 26,000 takes its offset within the runner\'s list steps', () => {
  const block = { id: 'b0', kind: 'code', text: 'x'.repeat(26_000), href: 'ts', findParts: [] };
  const tokens = Counted.from(codeTokens(Array.from({ length: 26_000 }, () => ({ text: 'x', cls: 'var' }))));
  const hl = { id: '0', code: block.text, icon: 'file-ts', tokens };
  const fns = contractFns('src/timeline/markdown.contract', { max: Math.max, min: Math.min });
  const codeBlock = contractComponent('src/timeline/markdown.contract', 'CodeBlock');
  const scope = { ...fns, max: Math.max, min: Math.min, block, code: [hl], hl, token: tokens[tokens.length - 1], wrapDefault: true, ms: -1, me: -1 };
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
