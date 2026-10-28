// The reference's code colours (highlights.ts): @pierre/highlights' lexers, run here under Bun's WebAssembly as the
// data crate runs them (highlights-fixture.ts), their token types as colour classes (highlights-tables.ts), and each
// class's ink, slant and weight (syntax-ink.contract) as t3-light and pierre-dark paint the types.
import { afterEach, describe, expect, test } from 'bun:test';
import { HIGHLIGHT_MAX_CHARS, highlightAsk, highlightPending, highlightTokens, lexerOf, resetHighlights, tokensOf } from '../../src/highlight/highlights';
import { TYPE_CLASSES } from '../../src/highlight/highlights-tables';
import { messageCodeBlocks } from '../../src/highlight/timeline-highlight';
import { contractFn } from '../support/contract-fn-fixture';
import { highlighted, lineRecords, settleHighlights } from '../support/highlights-fixture';

const pairs = (text: string, language: string) => highlighted(text, language).map(token => [token.text, token.cls]);
afterEach(() => resetHighlights());

describe('the lexers paint each language as the reference does', () => {
  test('TypeScript: declaration keywords are keywords, as the oracle shows `function`', () => {
    expect(pairs('export const fold = true;', 'ts')).toEqual([['export const ', 'kw'], ['fold ', 'var'], ['= ', 'op'], ['true', 'num'], [';', 'punct']]);
    expect(pairs('export async function countLines(path: string) {}', 'typescript').slice(0, 4))
      .toEqual([['export async function ', 'kw'], ['countLines', 'fn'], ['(', 'punct'], ['path', 'param']]);
  });
  test('Rust, Python, a shell command and JSON', () => {
    expect(pairs('fn main() { let x: u32 = 1; }', 'rust')).toEqual([['fn ', 'kw'], ['main', 'fn'], ['() { ', 'punct'], ['let ', 'kw'], ['x', 'var'],
      [': ', 'punct'], ['u32 ', 'type'], ['= ', 'op'], ['1', 'num'], ['; }', 'punct']]);
    expect(pairs('def f(a):\n    return a  # c', 'py')).toEqual([['def ', 'kw'], ['f', 'fn'], ['(', 'punct'], ['a', 'param'], ['):', 'punct'], ['\n', ''],
      ['    ', 'punct'], ['return ', 'kw'], ['a  ', 'var'], ['# c', 'com']]);
    expect(pairs('git commit -m "msg" --amend', 'bash')).toEqual([['git ', 'fn'], ['commit -m ', 'var'], ['"msg" ', 'str'], ['--amend', 'var']]);
    expect(pairs('{"name": 2}', 'json')).toEqual([['{', 'punct'], ['"name"', 'var'], [': ', 'punct'], ['2', 'num'], ['}', 'punct']]);
  });
  test('Markdown: headings weigh 700, list markers are grey, bold and emphasis keep their styles', () => {
    expect(pairs('# Title\n\n- item one\n1. second\n**bold** and *em*', 'md')).toEqual([['# Title', 'title'], ['\n', ''], ['\n', ''], ['- ', 'punct'],
      ['item one\n', ''], ['1. ', 'punct'], ['second\n', ''], ['**bold**', 'strong'], [' and ', ''], ['*em*', 'em']]);
  });
  test('a diff', () => {
    expect(pairs('-old\n+new', 'diff')).toEqual([['-', 'kw'], ['old', 'del'], ['\n', ''], ['+', 'kw'], ['new', 'ins']]);
  });
});

describe('the classes take t3-light and pierre-dark', () => {
  test('each token type has a class, and each class an ink, slant and weight', async () => {
    const color = await contractFn('syntax-ink.contract', 'synColor', ['cls']), slant = await contractFn('syntax-ink.contract', 'synSlant', ['cls']);
    const weight = await contractFn('syntax-ink.contract', 'synWeight', ['cls']);
    expect(TYPE_CLASSES).toHaveLength(73);
    for (const cls of new Set(TYPE_CLASSES)) expect(color(cls)).toMatch(/^light-dark\(#[0-9a-f]{6}, #[0-9a-f]{6}\)$/);
    // readableSyntaxTheme.ts: pierre-light darkened to 4.5:1 (6:1 for #1ca1c7, #636363, #1a85d4); the task's inks.
    expect(['', 'kw', 'type', 'fn', 'var', 'label', 'param', 'punct', 'const', 'str', 'esc', 'regex', 'num', 'op', 'com', 'tag', 'attr', 'link'].map(cls => color(cls)))
      .toEqual(['#0a0a0a/#fafafa', '#b12251/#ff678d', '#912ba7/#d568ea', '#693acf/#9d6afb', '#8f4e17/#ffa359', '#7e540e/#ffab16', '#4b4b4b/#a3a3a3',
        '#4b4b4b/#636363', '#735b02/#ffd452', '#106a2b/#5ecc71', '#0b685b/#61d5c0', '#0d6971/#64d1db', '#0b5267/#68cdf2', '#006781/#08c0ef',
        '#5c5c5c/#737373', '#a43d23/#ff855e', '#0e6944/#60d199', '#00609f/#009fff'].map(pair => `light-dark(${pair.replace('/', ', ')})`));
    expect(['title', 'strong', 'em', 'predict', 'kw'].map(cls => [slant(cls), weight(cls)]))
      .toEqual([['normal', 700], ['normal', 700], ['italic', 400], ['italic', 400], ['normal', 400]]);
  });
});

describe('a text reaches the lexers through the root', () => {
  test('it paints plain until the pump stores its tokens, and a longer text of it keeps them meanwhile', () => {
    expect(highlightTokens('const a = 1;', 'ts')).toBeNull();
    expect(highlightPending()).toBe(true);
    settleHighlights();
    expect(highlightTokens('const a = 1;', 'ts')!.map(token => token.cls)).toEqual(['kw', 'var', 'op', 'num', 'punct']);
    // A streaming block grows: its new text waits, and paints the lexed part's tokens and the rest plain.
    expect(highlightTokens('const a = 1;\nlet b', 'ts')!.map(token => [token.text, token.cls]).slice(-2)).toEqual([[';', 'punct'], ['\nlet b', '']]);
  });
  test('a language without a lexer, or a text past the limit, paints plain and asks nothing', () => {
    expect(['console', 'abap', 'text', '', 'notes.unknown'].map(lexerOf)).toEqual([0, 0, 0, 0, 0]);
    expect(highlightTokens('$ ls', 'console')).toBeNull();
    expect(highlightTokens('x'.repeat(HIGHLIGHT_MAX_CHARS + 1), 'ts')).toBeNull();
    expect(highlightPending()).toBe(false);
  });
  test('fence words read lower-cased, gitignore as ini, and paths by @pierre/diffs\' file types', () => {
    expect([lexerOf('Python'), lexerOf('src/app.tsx'), lexerOf('Dockerfile'), lexerOf('README.md')].every(id => id > 0)).toBe(true);
    expect(lexerOf('Python')).toBe(lexerOf('py'));
    expect(lexerOf('gitignore')).toBe(lexerOf('ini'));
  });
  test('the most recently asked texts go first, and a text nobody asks for again is dropped', () => {
    highlightTokens('let a = 1;', 'ts');
    highlightTokens('let b = 2;', 'ts');
    expect(highlightAsk().requests.map(request => request.text)).toEqual(['let b = 2;', 'let a = 1;']);
    // Rounds whose lexers never answer ask again, until no view has asked for the texts in 60 rounds.
    let last = highlightAsk();
    for (let round = 2; round < 61; round++) last = highlightAsk();
    expect(last.requests).toEqual([]);
    expect(highlightPending()).toBe(false);
  });
});

describe('line records become tokens', () => {
  test('line breaks are plain, and a line at the limit or longer is one plain token', () => {
    const text = 'const a = 1;\nconst b = 2;';
    const records = lineRecords(lexerOf('ts'), text);
    expect(tokensOf(text, records).map(token => token.text).join('')).toBe(text);
    expect(tokensOf(text, records, 12).map(token => [token.text, token.cls])).toEqual([['const a = 1;\n', ''], ['const b = 2;', '']]);
    expect(tokensOf(text, records, 13).map(token => token.cls)).toContain('kw');
  });
  test('code blocks carry their exact text for the renderer to match', () => {
    const markdown = 'Intro\n\n```ts\nconst a = 1;\n```\n\n```\nplain\n```\n';
    messageCodeBlocks(markdown);
    settleHighlights();
    const blocks = messageCodeBlocks(markdown);
    expect(blocks.map(block => block.code)).toEqual(['const a = 1;', 'plain']);
    expect(blocks[0]!.tokens.map(token => token.cls)).toEqual(['kw', 'var', 'op', 'num', 'punct']);
    expect(blocks[1]!.tokens).toEqual([{ id: '0', text: 'plain', cls: '' }]);
  });
});
