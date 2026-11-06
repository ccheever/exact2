import { afterEach, describe, expect, test } from 'bun:test';
import { lineTokens, overlay } from '../../src/highlight/timeline-diff-syntax';
import { parsePatch } from '../../src/diff/diff';
import { resetHighlights } from '../../src/highlight/highlights';
import { settleHighlights } from '../support/highlights-fixture';

/** One side's line tokens once the lexers have answered for it. */
const lexed = (lines: string[], path: string) => { lineTokens(lines, path); settleHighlights(); return lineTokens(lines, path); };
afterEach(() => resetHighlights());

describe('changes panel syntax', () => {
  test('each side of a hunk is coloured as the reference colours fold.ts', () => {
    const [line] = lexed(['export const fold = true;'], 'src/timeline/fold.ts');
    expect(line!.map(token => [token.text, token.cls])).toEqual([['export const ', 'kw'], ['fold ', 'var'], ['= ', 'op'], ['true', 'num'], [';', 'punct']]);
    expect(lexed(['/* a', ' b */ x'], 'a.ts').map(tokens => tokens.map(token => token.cls))).toEqual([['com'], ['com', 'var']]);
    expect(lineTokens(['plain', ''], 'notes.unknown')).toEqual([[{ text: 'plain', cls: '' }], []]);
  });
  test('word marks split at token boundaries and keep both properties', () => {
    const segments = [{ id: '0', text: 'export const ', mark: false, syntax: '' }, { id: '1', text: 'fold', mark: true, syntax: '' }, { id: '2', text: ' = 1;', mark: false, syntax: '' }];
    const [tokens] = lexed(['export const fold = 1;'], 'a.ts');
    expect(overlay(segments, tokens!).map(segment => [segment.text, segment.mark, segment.syntax])).toEqual([
      ['export const ', false, 'kw'], ['fold', true, 'var'], [' ', false, 'var'], ['= ', false, 'op'], ['1', false, 'num'], [';', false, 'punct']]);
  });
  test('a parsed TypeScript patch keeps its hunks intact for the overlay', () => {
    const files = parsePatch('diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-export const rows = 1;\n+export const rows = 2;\n');
    expect(files[0]!.hunks[0]!.lines.map(line => line.kind)).toEqual(['deletion', 'addition']);
  });
});
