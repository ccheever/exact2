// A code block draws one text per token, each with its offset for find and citation marks (markdown.contract
// CodeBlock). The runner bounds the list steps one evaluation takes (runner/src/vm.rs MAX_LIST_STEPS, LLP 1017.003 D3):
// each run of a map or filter callback is a step, and each item join prints. Summing the tokens before each one in
// Contract took N + 2k steps for the k-th of N tokens, so a coloured 1,500-line block of about 26,000 tokens trapped
// (IterationLimit) and poisoned the runner. The offsets are counted once in TypeScript (CodeToken `at`).
import { expect, test } from 'bun:test';
import { contractComponent, contractFns } from '../support/contract-action-fixture';
import { codeTokens } from '../../src/highlight/timeline-highlight';

const MAX_LIST_STEPS = 65_536;
let steps = 0;
/** A list whose map, filter and join count the steps the runner charges for them. */
class Counted<T> extends Array<T> {
  override map<U>(f: (value: T, index: number, array: T[]) => U): U[] { steps += this.length; return super.map(f); }
  override filter(f: (value: T, index: number, array: T[]) => unknown): T[] { steps += this.length; return super.filter(f); }
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
