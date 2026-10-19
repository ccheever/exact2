import { describe, expect, test } from 'bun:test';
import { composerRichProjection, MAX_HIGHLIGHTED_CODE_BLOCK_LENGTH } from '../../src/composer/composer-rich-projection';
import { resetHighlightSlicing } from '../../src/highlight/shiki-highlight';

const project = (value: string, styling = true, literal = false) => composerRichProjection({ value, owner: 'thread-one', styling, literal, skills: {}, literalRanges: [] });
describe('composer decoration producer', () => {
  test('real Shiki tokens retain source positions and the same signature as their language header', () => {
    resetHighlightSlicing();
    const source = '😀 text\n```ts title=x\nconst n = 1\n```';
    const result = project(source);
    expect(result.owner).toBe('thread-one');
    expect(result.value).toBe(source);
    expect(result.headers).toEqual([{ start: 8, end: 21, infoStart: 11, infoEnd: 21, info: 'ts title=x', language: 'typescript', label: 'TypeScript' }]);
    expect(result.decorations.spans.filter(span => span.name.startsWith('cmp-syn-')).map(span => [span.name, source.slice(span.start, span.end)])).toContainEqual(['cmp-syn-decl', 'const']);
    expect(result.decorations.spans.filter(span => source.slice(span.start, span.end) === '1').map(span => span.name)).toContain('cmp-syn-num');
  });
  test('admission ranges distinguish hidden fence source from its editable body and preserve UTF16 offsets', () => {
    const source = '😀\n```ts\nx\n```\n- item';
    const result = project(source);
    expect(result.blocks).toEqual([
      { kind: 'paragraph', start: 0, end: 2, contentStart: 0, bodyStart: 0, bodyEnd: 2 },
      { kind: 'code', start: 3, end: 14, contentStart: 3, bodyStart: 9, bodyEnd: 10 },
      { kind: 'list', start: 15, end: 21, contentStart: 17, bodyStart: 15, bodyEnd: 21 },
    ]);
    expect(project(source, false).blocks.every(block => block.kind === 'paragraph')).toBe(true);
  });
  test('20,000 source characters are highlighted and larger blocks stay ordinary code', () => {
    resetHighlightSlicing();
    const source = (length: number) => '```ts\n' + '1'.repeat(length) + '\n```';
    expect(project(source(MAX_HIGHLIGHTED_CODE_BLOCK_LENGTH)).decorations.spans.length).toBeGreaterThan(0);
    const large = project(source(MAX_HIGHLIGHTED_CODE_BLOCK_LENGTH + 1));
    expect(large.decorations.spans).toEqual([]);
    expect(large.decorations.blocks[0]?.name).toBe('cmp-code-block');
    expect(large.headers[0]?.label).toBe('TypeScript');
  });
  test('rich-text preference affects syntax while literal answer mode also excludes chips', () => {
    const source = '**bold** $tool \n```ts\nconst n = 1; $inside \n```';
    expect(project(source).chips).toHaveLength(1);
    const plain = project(source, false);
    expect(plain.decorations.spans).toEqual([]);
    expect(plain.decorations.blocks).toEqual([]);
    expect(plain.decorations.atoms).toHaveLength(2);
    const literal = project(source, true, true);
    expect(literal.decorations).toEqual({ value: source, spans: [], blocks: [], hidden: [], atoms: [] });
    expect(literal.chips).toEqual([]);
  });
  test('reference inventory preserves inline and escaped source semantics independently of paint', () => {
    const context = '[Thread](t3-context://v1/thread/one)';
    const value = '` $inside `\n`' + context + '`\n\\' + context + '\n> - ' + context + '\n- > ' + context;
    const input = { owner: 'thread-one', value, styling: true, literal: false, skills: {}, literalRanges: [{ start: 0, end: value.length }] };
    const view = composerRichProjection(input);
    // Inline literal ranges protect formatting; whole-answer literal mode excludes references.
    expect(view.chips.map(chip => chip.source)).toEqual(['$inside', context, context, context, context]);
    expect(view.owner).toBe(input.owner);
    expect(view.value).toBe(value);
    expect(composerRichProjection({ ...input, literal: true }).chips).toEqual([]);
    const candidate = view.chips[0];
    if (!candidate) throw new Error('source reference must exist');
    expect(composerRichProjection({ ...input, chips: [{ ...candidate, source: '$stale' }] }).chips).toEqual([]);
  });
});
