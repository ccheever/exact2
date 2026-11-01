import { describe, expect, test } from 'bun:test';
import { codeAt, composerMarkdown, emptyFenceAt } from '../../src/composer/composer-markdown';
import { codeLanguageEntry, codeLanguageLabel, CODE_BLOCK_LANGUAGES, languageOfInfoString, withInfoStringLanguage } from '../../src/composer/composer-code-languages';
import { fileLanguage } from '../../src/highlight/highlights';

// The Markdown field paints the source (LLP 1045); these blocks drive key
// handling and the controls outside the field.
describe('source-backed composer Markdown', () => {
  test('headings require whitespace and leave PR tokens literal', () => {
    const source = '# Heading\n## two\n###### six\n#1234\n####### seven';
    const { blocks } = composerMarkdown(source);
    expect(blocks.map(block => block.kind)).toEqual(['heading', 'heading', 'heading', 'paragraph', 'paragraph']);
    expect(blocks.map(block => 'contentStart' in block ? source.slice(block.start, block.contentStart) : '')).toEqual(['# ', '## ', '###### ', '', '']);
  });
  test('nested lists preserve source numbers, whitespace and task state without nesting other blocks', () => {
    const source = '* first\n  003) second\n\t- [X] task\n- # literal heading\n> - literal list';
    const { blocks } = composerMarkdown(source);
    expect(blocks.map(block => block.kind)).toEqual(['list', 'list', 'list', 'list', 'quote']);
    expect(blocks.map(block => block.kind === 'list' ? [block.list, block.marker, block.checked] : [])).toEqual([
      ['bullet', '*', false], ['ordered', '003)', false], ['task', '-', true], ['bullet', '-', false], [],
    ]);
    expect(blocks.map(block => 'contentStart' in block ? source.slice(block.start, block.contentStart) : '')).toEqual(['* ', '  003) ', '\t- [X] ', '- ', '> ']);
  });
  test('rules win over a bullet and a task needs whitespace before its content', () => {
    const { blocks } = composerMarkdown('---\n* * *\n___\n- [ ] task\n- [x]word');
    expect(blocks.map(block => block.kind)).toEqual(['rule', 'rule', 'rule', 'list', 'list']);
    expect(blocks.map(block => block.kind === 'list' ? block.list : '')).toEqual(['', '', '', 'task', 'bullet']);
  });
  test('UTF-16 offsets preserve emoji and CRLF', () => {
    const source = '😀 **bold**\r\n# Heading\r\nplain';
    expect(composerMarkdown(source).blocks.map(block => [block.kind, block.start, block.end])).toEqual([
      ['paragraph', 0, 12], ['heading', 13, 23], ['paragraph', 24, 29],
    ]);
  });
  test('fences preserve exact info/closing source', () => {
    const source = '```ts title=x\n**literal** @alice\n````  \nafter';
    const { blocks } = composerMarkdown(source);
    const code = blocks[0];
    expect(code?.kind).toBe('code');
    if (code?.kind !== 'code') throw new Error('expected parsed code');
    expect(code.info).toBe('ts title=x');
    expect(source.slice(code.body.start, code.body.end)).toBe('**literal** @alice');
    expect(source.slice(code.close?.start, code.close?.end)).toBe('````  ');
    expect(blocks.map(block => block.kind)).toEqual(['code', 'paragraph']);
  });
  test('empty and unclosed code bodies have valid ranges; two identical fences remain separate occurrences', () => {
    // No body line: the closing line's start is not inside the code.
    const empty = composerMarkdown('```ts\n```');
    expect(empty.blocks[0]).toMatchObject({ kind: 'code', body: null, close: { start: 6, end: 9 } });
    expect(codeAt(empty.blocks, { start: 6, end: 6 })).toBeNull();
    expect(emptyFenceAt(empty.blocks, { start: 6, end: 6 })?.start).toBe(0);
    expect(emptyFenceAt(empty.blocks, { start: 6, end: 7 })).toBeNull();
    expect(codeAt(composerMarkdown('```ts\n\n```').blocks, { start: 6, end: 6 })?.body).toEqual({ start: 6, end: 6 });
    const unclosed = composerMarkdown('~~~py\nx');
    expect(codeAt(unclosed.blocks, { start: 6, end: 7 })?.close).toBeNull();
    const two = composerMarkdown('```\nx\n```\n```\nx\n```');
    expect(codeAt(two.blocks, { start: 4, end: 15 })).toBeNull();
    expect(codeAt(two.blocks, { start: 4, end: 5 })?.start).toBe(0);
    expect(codeAt(two.blocks, { start: 14, end: 15 })?.start).toBe(10);
  });
  test('disabled rich text leaves the full draft ordinary', () => {
    const value = '# heading\n**bold**\n```ts\ncode\n```';
    const { blocks } = composerMarkdown(value, { styling: false });
    expect(blocks.every(block => block.kind === 'paragraph')).toBe(true);
  });
});

describe('code fence language picker', () => {
  test('all 40 reference choices and aliases use the canonical label', () => {
    expect(CODE_BLOCK_LANGUAGES).toHaveLength(40);
    expect(['ts', 'JS', 'c++', 'sh', 'text', 'assembly', 'S', 'unknown'].map(codeLanguageLabel))
      .toEqual(['TypeScript', 'JavaScript', 'C++', 'Bash', 'Plain text', 'Assembly', 'Assembly', 'unknown']);
  });
  test('plain-text fence words in any case read as Plain text, metadata aside', () => {
    const entries = ['Text', 'TEXT', 'plaintext', 'txt'].map((language) =>
      codeLanguageEntry(languageOfInfoString(` ${language} title=notes.txt`)));
    expect(entries).toEqual(Array.from({ length: 4 }, () => ({ id: '', label: 'Plain text' })));
  });
  test('assembly files highlight as asm, not the library\'s riscv', () => {
    expect(['a.asm', 'a.ASM', 'a.s', 'a.S', 'a.nasm', 'a.NASM'].map(fileLanguage)).toEqual(Array(6).fill('asm'));
  });
  test('a language change preserves info-string metadata and plain text keeps its disambiguator', () => {
    expect(withInfoStringLanguage('  ts title=x  ', 'python')).toBe('  python title=x  ');
    expect(withInfoStringLanguage('  ts title=x  ', '')).toBe('  text title=x  ');
    expect(withInfoStringLanguage('  ts  ', '')).toBe('');
  });
});
