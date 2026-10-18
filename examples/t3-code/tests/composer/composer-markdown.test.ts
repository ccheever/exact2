import { describe, expect, test } from 'bun:test';
import { codeAt, composerMarkdown, type ComposerAtom } from '../../src/composer/composer-markdown';
import { codeLanguageLabel, CODE_BLOCK_LANGUAGES, withInfoStringLanguage } from '../../src/composer/composer-code-languages';

const parsed = (value: string) => composerMarkdown(value).decorations;
describe('source-backed composer Markdown', () => {
  test('headings require whitespace and leave PR tokens literal', () => {
    const source = '# Heading\n## two\n###### six\n#1234\n####### seven';
    const { decorations, blocks } = composerMarkdown(source);
    expect(decorations.value).toBe(source);
    expect(blocks.map(block => block.kind)).toEqual(['heading', 'heading', 'heading', 'paragraph', 'paragraph']);
    expect(decorations.blocks.map(block => [block.name, source.slice(block.start, block.end)])).toEqual([
      ['cmp-h1', '# Heading'], ['cmp-h2', '## two'], ['cmp-h6', '###### six'],
    ]);
    expect(decorations.hidden.map(range => source.slice(range.start, range.end))).toEqual(['# ', '## ', '###### ']);
  });
  test('nested lists preserve source numbers, whitespace and task state without nesting other blocks', () => {
    const source = '* first\n  003) second\n\t- [X] task\n- # literal heading\n> - literal list';
    const { decorations, blocks } = composerMarkdown(source);
    expect(blocks.map(block => block.kind)).toEqual(['list', 'list', 'list', 'list', 'quote']);
    expect(decorations.blocks.map(block => [block.name, block.marker])).toEqual([
      ['cmp-list-depth0', 'bullet'], ['cmp-list-depth1', 'source'], ['cmp-list-depth1', 'checkbox:on'], ['cmp-list-depth0', 'bullet'], ['cmp-quote', ''],
    ]);
    expect(decorations.hidden.map(range => source.slice(range.start, range.end))).toEqual(['* ', '  003) ', '\t- [X] ', '- ', '> ']);
    expect(decorations.spans).toEqual([]);
  });
  test('rules win over a bullet and a task needs whitespace before its content', () => {
    const { blocks, decorations } = composerMarkdown('---\n* * *\n___\n- [ ] task\n- [x]word');
    expect(blocks.map(block => block.kind)).toEqual(['rule', 'rule', 'rule', 'list', 'list']);
    expect(decorations.blocks.map(block => block.marker)).toEqual(['rule', 'rule', 'rule', 'checkbox:off', 'bullet']);
    expect(decorations.hidden.at(-1)).toMatchObject({ start: 25, end: 27 });
  });
  test('inline nesting retains original delimiters and reveals the complete matching pair', () => {
    const source = '**bold *italic* `code`** ~~gone~~';
    const decorations = parsed(source);
    expect(decorations.spans.map(span => [span.name, source.slice(span.start, span.end)])).toEqual([
      ['cmp-bold', 'bold *italic* `code`'], ['cmp-italic', 'italic'], ['cmp-code-inline', 'code'], ['cmp-strike', 'gone'],
    ]);
    expect(decorations.hidden.filter(range => range.reveal?.start === 0).map(range => [range.start, range.end, range.reveal?.end])).toEqual([[0, 2, 24], [22, 24, 24]]);
    expect(decorations.value).toBe(source);
  });
  test('unclosed outer syntax stays literal while a closed inner span survives', () => {
    const source = '**open *inner*';
    expect(parsed(source).spans.map(span => [span.name, source.slice(span.start, span.end)])).toEqual([['cmp-italic', 'inner']]);
    expect(parsed('before\\*literal* and `open').spans).toEqual([]);
    expect(parsed('identifier__init__value').spans).toEqual([]);
  });
  test('literal path-query paste and chip source are excluded before inline parsing', () => {
    const source = 'src/**name**.ts **bold**';
    const literal = [{ start: 0, end: 15 }];
    expect(composerMarkdown(source, { literal }).decorations.spans.map(span => source.slice(span.start, span.end))).toEqual(['bold']);
    const value = '[src__x__.ts](src__x__.ts) **yes**';
    const atoms: ComposerAtom[] = [{ name: 'cmp-mention', start: 0, end: 24, label: 'src__x__.ts' }];
    expect(composerMarkdown(value, { atoms }).decorations.atoms).toEqual(atoms);
    expect(composerMarkdown(value, { atoms }).decorations.spans.map(span => value.slice(span.start, span.end))).toEqual(['yes']);
  });
  test('UTF-16 offsets preserve emoji, CRLF and raw source outside styled ranges', () => {
    const source = '😀 **bold**\r\n# Heading\r\nplain';
    const { decorations } = composerMarkdown(source);
    expect(decorations.spans).toEqual([{ name: 'cmp-bold', start: 5, end: 9 }]);
    expect(decorations.blocks[0]).toMatchObject({ start: 13, end: 23 });
    expect(decorations.value).toBe(source);
  });
  test('fences preserve exact info/closing source and code suppresses inline marks and chips', () => {
    const source = '```ts title=x\n**literal** @alice\n````  \nafter';
    const atom: ComposerAtom = { name: 'cmp-mention', start: 25, end: 31, label: 'Alice' };
    const { decorations, blocks } = composerMarkdown(source, { atoms: [atom] });
    const code = blocks[0];
    expect(code?.kind).toBe('code');
    if (code?.kind !== 'code') throw new Error('expected parsed code');
    expect(code.info).toBe('ts title=x');
    expect(source.slice(code.body.start, code.body.end)).toBe('**literal** @alice');
    expect(source.slice(code.close?.start, code.close?.end)).toBe('````  ');
    expect(decorations.spans).toEqual([]);
    expect(decorations.atoms).toEqual([]);
    expect(decorations.hidden.map(range => source.slice(range.start, range.end))).toEqual(['```ts title=x', '````  ']);
  });
  test('empty and unclosed code bodies have valid ranges; two identical fences remain separate occurrences', () => {
    const empty = composerMarkdown('```ts\n```');
    expect(codeAt(empty.blocks, { start: 6, end: 6 })?.body).toEqual({ start: 6, end: 6 });
    const unclosed = composerMarkdown('~~~py\nx');
    expect(codeAt(unclosed.blocks, { start: 6, end: 7 })?.close).toBeNull();
    const two = composerMarkdown('```\nx\n```\n```\nx\n```');
    expect(codeAt(two.blocks, { start: 4, end: 15 })).toBeNull();
    expect(codeAt(two.blocks, { start: 4, end: 5 })?.start).toBe(0);
    expect(codeAt(two.blocks, { start: 14, end: 15 })?.start).toBe(10);
  });
  test('disabled rich text leaves the full draft ordinary', () => {
    const value = '# heading\n**bold**\n```ts\ncode\n```';
    const { decorations, blocks } = composerMarkdown(value, { styling: false });
    expect(decorations).toEqual({ value, spans: [], blocks: [], hidden: [], atoms: [] });
    expect(blocks.every(block => block.kind === 'paragraph')).toBe(true);
  });
});

describe('code fence language picker', () => {
  test('all 39 reference choices and aliases use the canonical label', () => {
    expect(CODE_BLOCK_LANGUAGES).toHaveLength(39);
    expect(['ts', 'JS', 'c++', 'sh', 'text', 'unknown'].map(codeLanguageLabel)).toEqual(['TypeScript', 'JavaScript', 'C++', 'Bash', 'Plain text', 'unknown']);
  });
  test('a language change preserves info-string metadata and plain text keeps its disambiguator', () => {
    expect(withInfoStringLanguage('  ts title=x  ', 'python')).toBe('  python title=x  ');
    expect(withInfoStringLanguage('  ts title=x  ', '')).toBe('  text title=x  ');
    expect(withInfoStringLanguage('  ts  ', '')).toBe('');
  });
});
