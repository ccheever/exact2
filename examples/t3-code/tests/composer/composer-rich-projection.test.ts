import { describe, expect, test } from 'bun:test';
import { composerRichProjection } from '../../src/composer/composer-rich-projection';

const project = (value: string, styling = true, literal = false) => composerRichProjection({ value, owner: 'thread-one', styling, literal });
describe('composer source projection', () => {
  test('a code header carries the source signature and its info-string range', () => {
    const source = '😀 text\n```ts title=x\nconst n = 1\n```';
    const result = project(source);
    expect(result.owner).toBe('thread-one');
    expect(result.value).toBe(source);
    expect(result.headers).toEqual([{ start: 8, end: 21, infoStart: 11, infoEnd: 21, info: 'ts title=x', language: 'typescript', label: 'TypeScript' }]);
    expect(result).not.toHaveProperty('decorations');
  });
  test('admission ranges distinguish hidden fence source from its editable body and preserve UTF16 offsets', () => {
    const source = '😀\n```ts\nx\n```\n- item';
    const result = project(source);
    expect(result.blocks).toEqual([
      { kind: 'paragraph', start: 0, end: 2, contentStart: 0, bodyStart: 0, bodyEnd: 2, bodyLine: true },
      { kind: 'code', start: 3, end: 14, contentStart: 3, bodyStart: 9, bodyEnd: 10, bodyLine: true },
      { kind: 'list', start: 15, end: 21, contentStart: 17, bodyStart: 15, bodyEnd: 21, bodyLine: true },
    ]);
    // Without rich text only a list line typed as text is a block: its Tab indents (listIndentForTab).
    expect(project(source, false).blocks.map(block => block.kind)).toEqual(['paragraph', 'paragraph', 'paragraph', 'paragraph', 'list']);
    // A fence closed on the next line is projected where its body would be, and says it has no body line.
    expect(project('```ts\n```').blocks).toEqual([{ kind: 'code', start: 0, end: 9, contentStart: 0, bodyStart: 6, bodyEnd: 6, bodyLine: false }]);
    expect(project('```ts\n\n```').blocks[0]).toMatchObject({ bodyStart: 6, bodyEnd: 6, bodyLine: true });
  });
  test('rich-text preference decides code blocks while literal answer mode also excludes chips', () => {
    const source = '**bold** $tool \n```ts\nconst n = 1; $inside \n```';
    const rich = project(source);
    expect([rich.mode, rich.chips.length, rich.headers.length]).toEqual(['rich', 1, 1]);
    const plain = project(source, false);
    expect([plain.mode, plain.chips.length, plain.headers.length]).toEqual(['plain', 2, 0]);
    const literal = project(source, true, true);
    expect([literal.mode, literal.chips.length, literal.headers.length]).toEqual(['literal', 0, 0]);
  });
  test('reference inventory preserves inline and escaped source semantics independently of paint', () => {
    const context = '[Thread](t3-context://v1/thread/one)';
    const value = '` $inside `\n`' + context + '`\n\\' + context + '\n> - ' + context + '\n- > ' + context;
    const input = { owner: 'thread-one', value, styling: true, literal: false };
    const view = composerRichProjection(input);
    // Whole-answer literal mode excludes references.
    expect(view.chips.map(chip => chip.source)).toEqual(['$inside', context, context, context, context]);
    expect(view.owner).toBe(input.owner);
    expect(view.value).toBe(value);
    expect(composerRichProjection({ ...input, literal: true }).chips).toEqual([]);
    const candidate = view.chips[0];
    if (!candidate) throw new Error('source reference must exist');
    expect(composerRichProjection({ ...input, chips: [{ ...candidate, source: '$stale' }] }).chips).toEqual([]);
  });
});
